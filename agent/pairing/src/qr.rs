//! QR fallback payload (issue #21/#23) for when mDNS discovery doesn't
//! work. No key material — same principle as the mDNS broadcast: only
//! enough to open the pairing connection, the actual secret exchange still
//! goes through the SPAKE2-authenticated channel in proto.rs. Carries the
//! pairing code (so the parent doesn't have to type it after scanning) and
//! a short validity window.

use anyhow::{Context, Result};
use qrcode::render::svg;
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub struct QrPayload {
    pub v: u8,
    pub host: String,
    pub port: u16,
    pub sid: String,
    pub code: String,
    /// Unix timestamp. Checked by the client (a real Control Center would
    /// refuse to even try connecting past this), and separately enforced
    /// server-side by `serve` simply not listening anymore after its own
    /// timeout — belt and suspenders, not two sources of truth: the
    /// server's listener lifetime is authoritative, this is just so a
    /// stale scanned code fails fast with a clear reason instead of a
    /// generic connection-refused.
    pub exp: i64,
}

pub fn write_svg(payload: &QrPayload, path: &Path) -> Result<()> {
    let json = serde_json::to_string(payload)?;
    let code = QrCode::new(json.as_bytes()).context("encoding pairing payload as a QR code")?;
    let svg_xml = code.render::<svg::Color>().min_dimensions(300, 300).build();
    std::fs::write(path, svg_xml).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Terminal-friendly rendering, for the dev VM / headless testing where
/// there's no screen to scan from — not meant for the real kiosk UI.
pub fn render_unicode(payload: &QrPayload) -> Result<String> {
    let json = serde_json::to_string(payload)?;
    let code = QrCode::new(json.as_bytes()).context("encoding pairing payload as a QR code")?;
    Ok(code
        .render::<qrcode::render::unicode::Dense1x2>()
        .quiet_zone(true)
        .build())
}
