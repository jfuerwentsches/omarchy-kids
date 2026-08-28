//! Pairing code + session ID generation. One code serves both the typed
//! (mDNS-discovered) and scanned (QR) paths — SPAKE2 (see proto.rs) is
//! secure even for a short, human-enterable code, so there's no need for
//! two different lengths depending on transport.

use rand::Rng;

/// Crockford-style alphabet, minus visually ambiguous characters (0/O,
/// 1/I/L) — meant to be read off a screen and typed on another device.
const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// 8 chars from a 32-symbol alphabet is 40 bits — not enough to resist
/// offline brute force on its own, but SPAKE2 doesn't need that: an
/// attacker gets exactly one online guess per connection attempt (see
/// proto.rs), and `serve` handles exactly one connection per process (see
/// main.rs) before exiting.
pub fn generate_pairing_code() -> String {
    let mut rng = rand::rng();
    let chars: String = (0..8)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect();
    format!("{}-{}", &chars[0..4], &chars[4..8])
}

/// Non-secret — just lets a client (and a human reading two screens at
/// once) confirm it's talking to the session it thinks it is, when more
/// than one child machine on the LAN might be pairing at the same time.
pub fn generate_session_id() -> String {
    let mut rng = rand::rng();
    (0..6)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Codes are generated with hyphens for display but typed/scanned without
/// them being significant — normalize before feeding into SPAKE2 so
/// "AB20-DKZ2" and "ab20dkz2" are the same password to both sides.
pub fn normalize_code(code: &str) -> String {
    code.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(char::to_uppercase)
        .collect()
}
