//! Wire protocol for the pairing exchange (issue #21). One JSON object per
//! line over a plain TCP connection — mirrors the newline-delimited-JSON
//! convention `omarchy_kids_common::transport` already uses for the
//! agentd socket (see docs/agent-protocol.md "Wire format").
//!
//! Security model: SPAKE2 (password-authenticated key exchange) over the
//! pairing code, so the code itself never needs to be strong enough to
//! resist offline brute force — an on-path or LAN attacker who captures the
//! whole exchange still can't recover the code or the derived key. Only the
//! payload *after* the handshake is confidential, sent AEAD-encrypted with
//! a key derived from the SPAKE2 shared secret via HKDF (domain-separated
//! with `HKDF_INFO` below, on top of what the spake2 crate's own transcript
//! hash already provides — cheap extra hygiene).
//!
//! The `spake2` crate has no independent security audit (see its own
//! README) — accepted for this project's threat model (a parental-control
//! tool defending against a casual LAN observer, not a nation-state), but
//! worth remembering if this ever needs to be re-justified.

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use hkdf::Hkdf;
use omarchy_kids_common::transport::{read_line_bounded, MAX_LINE_BYTES};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use std::io::{BufRead, Write};
use std::net::TcpStream;

/// The child (server) side "creates a random code" per the spake2 crate's
/// own description of this use case. Fixed, non-secret identity strings —
/// just domain separation between the two sides, not part of the secret.
const ID_CONTROL_CENTER: &[u8] = b"omarchy-kids-control-center";
const ID_CHILD: &[u8] = b"omarchy-kids-child";
const HKDF_INFO: &[u8] = b"omarchy-kids-pairing v1 aead-key";
const NONCE_LEN: usize = 12;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// Client -> server, first message: which session, plus the client's
    /// SPAKE2 message.
    Hello {
        v: u8,
        sid: String,
        spake_msg: String,
    },
    /// Server -> client: the server's SPAKE2 message.
    SpakeMsg { spake_msg: String },
    /// Either direction, once both sides hold the derived key.
    Encrypted { nonce: String, ciphertext: String },
    /// Server -> client: sid didn't match, or decryption failed (wrong
    /// code) — either way the exchange is aborted, nothing is written.
    Error { reason: String },
}

/// The plaintext of every `Encrypted` message, once decrypted.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SecurePayload {
    /// Client -> server: the Control Center's own freshly generated SSH
    /// public key — see the vault note's "Kernentscheidung": the private
    /// key never leaves the machine it was generated on, encrypted or not.
    Pubkey { pubkey: String },
    /// Server -> client: confirms the key was installed, and what to
    /// connect to. `fingerprint` is shown to the parent for a final visual
    /// check, on top of (not instead of) the SPAKE2 authentication.
    /// `username` is the child account `serve` ran as — without it the
    /// Control Center has no way to know which account to SSH into, since
    /// the child's username is chosen freely during Omarchy's own account
    /// setup and was never otherwise transmitted (found missing during
    /// issue #29's real end-to-end verification: pairing itself worked,
    /// but the paired key was then unusable because nothing recorded whose
    /// authorized_keys it lived in).
    ///
    /// `ssh_host_public_key` is the child machine's own real sshd host key
    /// (issue #33): SPAKE2 authenticates *this pairing exchange*, but
    /// without also transmitting the host key through it, nothing ties that
    /// authenticated exchange to the specific SSH host the parent will
    /// later actually connect to — the first real SSH connection was pure
    /// `StrictHostKeyChecking=accept-new` TOFU, unrelated to pairing trust.
    /// Sent as the full public-key line (not just its fingerprint) so the
    /// Control Center can pin it directly into a known_hosts file before
    /// ever connecting, mechanically, rather than showing the parent a
    /// second fingerprint to eyeball.
    Confirm {
        hostname: String,
        ssh_port: u16,
        fingerprint: String,
        username: String,
        ssh_host_public_key: String,
    },
    /// Client -> server: the parent confirmed the fingerprint matches.
    Ack { confirmed: bool },
}

pub fn read_message(reader: &mut impl BufRead) -> Result<Message> {
    let line = read_line_bounded(reader, MAX_LINE_BYTES)
        .context("reading from peer")?
        .ok_or_else(|| anyhow!("connection closed before a message was received"))?;
    serde_json::from_str(&line).context("parsing message")
}

pub fn write_message(writer: &mut impl Write, msg: &Message) -> Result<()> {
    let line = serde_json::to_string(msg)?;
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

/// Derives the AEAD key from the raw SPAKE2 shared secret via HKDF-Expand.
fn derive_aead_key(spake_key: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, spake_key);
    let mut key = [0u8; 32];
    hk.expand(HKDF_INFO, &mut key)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    key
}

pub fn encrypt(spake_key: &[u8], payload: &SecurePayload) -> Result<Message> {
    let key = derive_aead_key(spake_key);
    let cipher = ChaCha20Poly1305::new((&key).into());
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);

    let plaintext = serde_json::to_vec(payload)?;
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_slice())
        .map_err(|_| anyhow!("encryption failed"))?;

    Ok(Message::Encrypted {
        nonce: B64.encode(nonce_bytes),
        ciphertext: B64.encode(ciphertext),
    })
}

/// Fails (rather than panics/returns garbage) on a wrong pairing code: a
/// mismatched SPAKE2 key makes decryption fail AEAD authentication, which
/// is exactly what we want — no separate "is the code right" check needed.
pub fn decrypt(spake_key: &[u8], msg: &Message) -> Result<SecurePayload> {
    let Message::Encrypted { nonce, ciphertext } = msg else {
        bail!("expected an encrypted message, got something else");
    };
    let key = derive_aead_key(spake_key);
    let cipher = ChaCha20Poly1305::new((&key).into());
    let nonce_bytes: [u8; NONCE_LEN] = B64
        .decode(nonce)
        .context("decoding nonce")?
        .try_into()
        .map_err(|_| anyhow!("nonce has the wrong length"))?;
    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = B64.decode(ciphertext).context("decoding ciphertext")?;

    let plaintext = cipher
        .decrypt(&nonce, ciphertext.as_slice())
        .map_err(|_| anyhow!("decryption failed (wrong pairing code, or corrupted message)"))?;
    Ok(serde_json::from_slice(&plaintext)?)
}

/// Server side: starts SPAKE2 as "B", ready to respond once the client's
/// message arrives. Normalizes the code itself (rather than trusting every
/// caller to) — both sides MUST derive the same password bytes, and a
/// server passing the hyphenated display form while the client normalizes
/// its typed/scanned copy is a silent, hard-to-debug pairing failure (hit
/// exactly this while testing).
pub fn spake_start_server(code: &str) -> (Spake2<Ed25519Group>, Vec<u8>) {
    let code = crate::code::normalize_code(code);
    Spake2::<Ed25519Group>::start_b(
        &Password::new(code.as_bytes()),
        &Identity::new(ID_CONTROL_CENTER),
        &Identity::new(ID_CHILD),
    )
}

/// Client side: starts SPAKE2 as "A". See `spake_start_server` on why
/// normalization happens here rather than at the call site.
pub fn spake_start_client(code: &str) -> (Spake2<Ed25519Group>, Vec<u8>) {
    let code = crate::code::normalize_code(code);
    Spake2::<Ed25519Group>::start_a(
        &Password::new(code.as_bytes()),
        &Identity::new(ID_CONTROL_CENTER),
        &Identity::new(ID_CHILD),
    )
}

pub fn read_stream(stream: &TcpStream) -> std::io::BufReader<TcpStream> {
    std::io::BufReader::new(stream.try_clone().expect("cloning the stream handle"))
}
