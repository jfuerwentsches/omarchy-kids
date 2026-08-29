//! Pairing exchange (issues #21-#24): hands the Control Center's SSH public
//! key to the child account and installs it as a `command=`-restricted
//! `authorized_keys` entry — the actual mechanism behind the wizard's
//! pairing step. Protocol details and the security rationale are in
//! `proto.rs`; see the vault note "Omarchy Kids - Implementierung
//! Setup-Wizard" ("Tracking — Vorschlag Pairing-Protokoll") for the
//! full design writeup this implements.
//!
//! Two subcommands, two very different lifetimes:
//! - `serve` (child side) is meant to be invoked by the wizard for exactly
//!   one pairing attempt, then exit — not a standing daemon.
//! - `pair` is a reference client exercising the same protocol, standing in
//!   for the real Control Center (which doesn't exist yet — see root
//!   CLAUDE.md "Status"). This is the concrete answer to issue #24's
//!   "Control-Center-side pairing contract": whatever eventually implements
//!   Control Center's pairing UI needs to speak exactly what this does.

mod code;
mod mdns;
mod proto;
mod qr;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use proto::{Message, SecurePayload};
use serde_json::json;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command as OsCommand;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Parser)]
#[command(name = "omarchy-kids-pairing")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Child side: open a time-boxed pairing window (mDNS broadcast + QR
    /// code), accept exactly one pairing attempt, then exit. Run as the
    /// child account (writes to its own ~/.ssh/authorized_keys).
    Serve {
        #[arg(long, default_value_t = 7420)]
        port: u16,
        #[arg(long, default_value_t = 10)]
        timeout_minutes: u32,
        /// Included in the Confirm payload so the client knows what port
        /// to actually SSH to afterward — decoupled from the pairing
        /// port itself.
        #[arg(long, default_value_t = 22)]
        ssh_port: u16,
        /// Write the QR code as an SVG here (the real wizard UI would show
        /// this on screen). Always prints a terminal-friendly QR too.
        #[arg(long)]
        qr_svg: Option<PathBuf>,
        #[arg(long)]
        no_mdns: bool,
    },
    /// Reference/test pairing client — NOT the real Control Center. See
    /// the module doc comment above.
    Pair {
        #[arg(long)]
        host: String,
        #[arg(long)]
        port: u16,
        #[arg(long)]
        sid: String,
        #[arg(long)]
        code: String,
        #[arg(long, default_value = "omarchy-kids-control-center")]
        comment: String,
        /// Where to write the freshly generated keypair
        /// (<path> and <path>.pub) — this is the Control Center's own
        /// key, generated locally; only the .pub half ever crosses the
        /// network.
        #[arg(long)]
        key_out: PathBuf,
        /// Skip the fingerprint confirmation prompt and confirm
        /// automatically. For scripting/testing — a real Control Center
        /// should show the fingerprint to the parent and only pass this
        /// once they've actually confirmed it matches the child's screen.
        #[arg(long, default_value_t = false)]
        yes: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve {
            port,
            timeout_minutes,
            ssh_port,
            qr_svg,
            no_mdns,
        } => serve(port, timeout_minutes, ssh_port, qr_svg, no_mdns),
        Command::Pair {
            host,
            port,
            sid,
            code,
            comment,
            key_out,
            yes,
        } => pair(&host, port, &sid, &code, &comment, &key_out, yes),
    }
}

fn local_hostname() -> Result<String> {
    Ok(std::fs::read_to_string("/proc/sys/kernel/hostname")
        .context("reading hostname")?
        .trim()
        .to_string())
}

fn local_ipv4() -> Result<String> {
    for iface in if_addrs::get_if_addrs().context("enumerating network interfaces")? {
        if iface.is_loopback() {
            continue;
        }
        if let std::net::IpAddr::V4(addr) = iface.ip() {
            return Ok(addr.to_string());
        }
    }
    bail!("no non-loopback IPv4 address found")
}

fn serve(
    port: u16,
    timeout_minutes: u32,
    ssh_port: u16,
    qr_svg: Option<PathBuf>,
    no_mdns: bool,
) -> Result<()> {
    let pairing_code = code::generate_pairing_code();
    let sid = code::generate_session_id();
    let hostname = local_hostname()?;
    // Resolved once and reused for both the printed line and the QR payload
    // below — the manual/QR fallback entry path is unusable without the
    // parent being able to read this off the child's screen (it was
    // previously only encoded in the QR image, never printed as text).
    let host = local_ipv4().unwrap_or_else(|_| hostname.clone());

    println!("Host:         {host}");
    println!("Port:         {port}");
    println!("Pairing code: {pairing_code}");
    println!("Session:      {sid}");
    println!(
        "Waiting up to {timeout_minutes} minute(s) for the parent's Control Center to pair..."
    );

    let _broadcast = if no_mdns {
        None
    } else {
        Some(mdns::Broadcast::start(&hostname, &sid, port).context("starting mDNS broadcast")?)
    };

    let exp = (SystemTime::now() + Duration::from_secs(timeout_minutes as u64 * 60))
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;
    let qr_payload = qr::QrPayload {
        v: 1,
        host: host.clone(),
        port,
        sid: sid.clone(),
        code: pairing_code.clone(),
        exp,
    };
    if let Some(path) = &qr_svg {
        qr::write_svg(&qr_payload, path)?;
        println!("QR code written to {}", path.display());
    }
    println!("{}", qr::render_unicode(&qr_payload)?);

    let stream = accept_with_timeout(port, timeout_minutes)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;

    match handle_connection(stream, &pairing_code, &sid, &hostname, ssh_port) {
        Ok(()) => {
            println!("Pairing succeeded.");
            Ok(())
        }
        Err(e) => {
            eprintln!("Pairing failed: {e:#}");
            Err(e)
        }
    }
}

fn accept_with_timeout(port: u16, timeout_minutes: u32) -> Result<TcpStream> {
    let listener = TcpListener::bind(("0.0.0.0", port))
        .with_context(|| format!("binding the pairing listener on port {port}"))?;
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + Duration::from_secs(timeout_minutes as u64 * 60);

    loop {
        match listener.accept() {
            Ok((stream, _addr)) => {
                stream.set_nonblocking(false)?;
                return Ok(stream);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    bail!("timed out waiting for a pairing connection ({timeout_minutes} min)");
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn handle_connection(
    stream: TcpStream,
    pairing_code: &str,
    sid: &str,
    hostname: &str,
    ssh_port: u16,
) -> Result<()> {
    let mut reader = proto::read_stream(&stream);
    let mut writer = stream;

    let hello = proto::read_message(&mut reader)?;
    let Message::Hello {
        v,
        sid: client_sid,
        spake_msg: client_msg_b64,
    } = hello
    else {
        bail!("expected a Hello message first");
    };
    if v != 1 {
        proto::write_message(
            &mut writer,
            &Message::Error {
                reason: "unsupported protocol version".into(),
            },
        )?;
        bail!("client spoke protocol version {v}, expected 1");
    }
    if client_sid != sid {
        proto::write_message(
            &mut writer,
            &Message::Error {
                reason: "unknown_session".into(),
            },
        )?;
        bail!("client asked for session '{client_sid}', this listener is '{sid}'");
    }

    let (spake, server_msg) = proto::spake_start_server(pairing_code);
    proto::write_message(
        &mut writer,
        &Message::SpakeMsg {
            spake_msg: base64_encode(&server_msg),
        },
    )?;

    let client_msg = base64_decode(&client_msg_b64)?;
    let shared_key = spake
        .finish(&client_msg)
        .map_err(|e| anyhow!("SPAKE2 exchange failed: {e:?}"))?;

    let payload_msg = proto::read_message(&mut reader)?;
    let payload = match proto::decrypt(&shared_key, &payload_msg) {
        Ok(p) => p,
        Err(e) => {
            // Deliberately vague to the peer (don't confirm/deny *why* it
            // failed over the wire) — the detail is only logged locally.
            proto::write_message(
                &mut writer,
                &Message::Error {
                    reason: "decryption failed".into(),
                },
            )?;
            return Err(e).context("decrypting the client's payload (likely a wrong code)");
        }
    };
    let SecurePayload::Pubkey { pubkey } = payload else {
        bail!("expected a Pubkey payload");
    };

    let fingerprint = install_pubkey(&pubkey)?;
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .context("cannot determine the current user (USER/LOGNAME unset) to report to the Control Center")?;

    let confirm = proto::encrypt(
        &shared_key,
        &SecurePayload::Confirm {
            hostname: hostname.to_string(),
            ssh_port,
            fingerprint: fingerprint.clone(),
            username,
        },
    )?;
    proto::write_message(&mut writer, &confirm)?;
    println!("Installed key, fingerprint: {fingerprint}");

    let ack_msg = proto::read_message(&mut reader)?;
    let ack = proto::decrypt(&shared_key, &ack_msg)?;
    match ack {
        SecurePayload::Ack { confirmed: true } => {
            println!("Client confirmed the fingerprint.");
            Ok(())
        }
        SecurePayload::Ack { confirmed: false } => {
            bail!("client did not confirm the fingerprint (key was still installed above)")
        }
        _ => bail!("expected an Ack payload"),
    }
}

/// Validates the incoming public key line and appends it to the child's
/// `authorized_keys` with the `command=` restriction (see
/// docs/agent-protocol.md — this is the actual security boundary).
/// Idempotent: re-pairing with the same key doesn't duplicate the line.
fn install_pubkey(pubkey: &str) -> Result<String> {
    let pubkey = pubkey.trim();
    if pubkey.contains('\n') || pubkey.contains('\r') {
        bail!("public key contains embedded newlines, refusing to install it");
    }
    if pubkey.len() > 4096 {
        bail!("public key is implausibly long, refusing to install it");
    }
    let known_prefix = ["ssh-ed25519 ", "ssh-rsa ", "ecdsa-sha2-"];
    if !known_prefix.iter().any(|p| pubkey.starts_with(p)) {
        bail!("public key doesn't start with a recognized algorithm, refusing to install it");
    }

    // The strongest validation available: if ssh-keygen can't parse it, it
    // isn't a real public key. Also gives us the fingerprint for free.
    let tmp = tempfile_with_contents(pubkey)?;
    let output = OsCommand::new("ssh-keygen")
        .arg("-lf")
        .arg(tmp.path())
        .output()
        .context("running ssh-keygen -lf")?;
    if !output.status.success() {
        bail!(
            "ssh-keygen rejected the public key: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let fingerprint_line = String::from_utf8_lossy(&output.stdout);
    let fingerprint = fingerprint_line
        .split_whitespace()
        .find(|tok| tok.starts_with("SHA256:"))
        .ok_or_else(|| anyhow!("could not parse fingerprint from ssh-keygen output"))?
        .to_string();

    let home = std::env::var("HOME").context("HOME is not set")?;
    let ssh_dir = std::path::Path::new(&home).join(".ssh");
    std::fs::create_dir_all(&ssh_dir)?;
    let authorized_keys = ssh_dir.join("authorized_keys");

    let existing = std::fs::read_to_string(&authorized_keys).unwrap_or_default();
    if existing.contains(pubkey) {
        return Ok(fingerprint); // already paired with this exact key
    }

    let restricted_entry = format!(
        "command=\"/usr/bin/omarchy-kids-agent\",no-agent-forwarding,no-X11-forwarding,no-port-forwarding,restrict {pubkey}\n"
    );
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&authorized_keys)?;
    file.write_all(restricted_entry.as_bytes())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&authorized_keys, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(fingerprint)
}

fn tempfile_with_contents(contents: &str) -> Result<tempfile::NamedTempFile> {
    let mut f = tempfile::NamedTempFile::new()?;
    f.write_all(contents.as_bytes())?;
    f.flush()?;
    Ok(f)
}

/// Reference pairing client (see the module doc comment) — generates its
/// own keypair, runs the same protocol `handle_connection` speaks, and
/// auto-confirms the fingerprint (a real Control Center would show it to
/// the parent and wait for a click instead).
fn pair(
    host: &str,
    port: u16,
    sid: &str,
    code: &str,
    comment: &str,
    key_out: &PathBuf,
    yes: bool,
) -> Result<()> {
    if key_out.exists() {
        bail!(
            "{} already exists — refusing to overwrite an existing key",
            key_out.display()
        );
    }
    let status = OsCommand::new("ssh-keygen")
        .args(["-t", "ed25519", "-N", "", "-C", comment, "-f"])
        .arg(key_out)
        .status()
        .context("running ssh-keygen")?;
    if !status.success() {
        bail!("ssh-keygen failed");
    }
    let pubkey = std::fs::read_to_string(format!("{}.pub", key_out.display()))?
        .trim()
        .to_string();

    let stream =
        TcpStream::connect((host, port)).with_context(|| format!("connecting to {host}:{port}"))?;
    let mut reader = proto::read_stream(&stream);
    let mut writer = stream;

    let (spake, client_msg) = proto::spake_start_client(code);
    proto::write_message(
        &mut writer,
        &Message::Hello {
            v: 1,
            sid: sid.to_string(),
            spake_msg: base64_encode(&client_msg),
        },
    )?;

    let reply = proto::read_message(&mut reader)?;
    let server_msg = match reply {
        Message::SpakeMsg { spake_msg } => base64_decode(&spake_msg)?,
        Message::Error { reason } => bail!("server rejected pairing: {reason}"),
        _ => bail!("expected a SpakeMsg reply"),
    };
    let shared_key = spake
        .finish(&server_msg)
        .map_err(|e| anyhow!("SPAKE2 exchange failed: {e:?}"))?;

    let pubkey_msg = proto::encrypt(&shared_key, &SecurePayload::Pubkey { pubkey })?;
    proto::write_message(&mut writer, &pubkey_msg)?;

    let confirm_msg = proto::read_message(&mut reader)?;
    let confirm = match proto::decrypt(&shared_key, &confirm_msg) {
        Ok(SecurePayload::Confirm {
            hostname,
            ssh_port,
            fingerprint,
            username,
        }) => (hostname, ssh_port, fingerprint, username),
        Ok(_) => bail!("expected a Confirm payload"),
        Err(e) => bail!("decrypting the server's confirmation failed (wrong code?): {e:#}"),
    };
    let (hostname, ssh_port, fingerprint, username) = confirm;

    println!("Paired with {hostname} (SSH port {ssh_port}).");
    println!("Key fingerprint: {fingerprint}");

    let confirmed = if yes {
        println!("(--yes passed, confirming without prompting.)");
        true
    } else {
        print!("Does this match the fingerprint shown on the child's screen? [y/N] ");
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
    };

    let ack = proto::encrypt(&shared_key, &SecurePayload::Ack { confirmed })?;
    proto::write_message(&mut writer, &ack)?;

    if !confirmed {
        // The server already installed the key before sending its Confirm
        // message (see handle_connection/install_pubkey) — declining here
        // tells it the parent didn't accept, but doesn't undo that install.
        // Revoking it needs a separate, already-trusted channel (there
        // isn't one yet at this point), so this is a known follow-up, not
        // something this client can fix from here.
        bail!(
            "fingerprint not confirmed — pairing aborted (note: the key was already installed \
             on the child; it is not automatically removed)"
        );
    }

    println!("Try: ssh -i {} -p {ssh_port} {username}@{hostname}", key_out.display());
    println!(
        "PAIR_RESULT: {}",
        json!({
            "hostname": hostname,
            "ssh_port": ssh_port,
            "fingerprint": fingerprint,
            "key_path": key_out.to_string_lossy(),
            "username": username,
        })
    );

    Ok(())
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.encode(bytes)
}

fn base64_decode(s: &str) -> Result<Vec<u8>> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.decode(s).context("decoding base64")
}
