//! Parent-curated allow-list of desktop ids `Unlock` may grant (issue #32).
//!
//! `Unlock` (unlike `Override`) reaches agentd with no interactive
//! authentication at all — a request over SSH from the Control Center and a
//! request from an arbitrary local child-uid process are indistinguishable
//! at the socket layer (see docs/security-threat-model.md). This allow-list
//! is the accepted mitigation: whatever the caller, `Unlock` can only ever
//! grant access to a desktop id the parent has explicitly listed here, never
//! an arbitrary one. `Override` (already gated by a live polkit admin
//! authentication at the moment it's used) is exempt — see `handlers.rs`.

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
struct RawAllowlist {
    #[serde(default)]
    unlockable: Vec<String>,
}

/// An absent file means an empty allow-list (secure default: nothing is
/// unlockable until the parent curates a list), not an error — this file
/// won't exist at all until the parent (or a future Control Center control)
/// creates it.
pub fn load(path: &Path) -> Vec<String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(_) => return Vec::new(),
    };
    match toml::from_str::<RawAllowlist>(&raw) {
        Ok(list) => list.unlockable,
        Err(e) => {
            eprintln!(
                "agentd: {} is malformed, treating as empty allow-list: {e:#}",
                path.display()
            );
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn missing_file_is_an_empty_list() {
        assert_eq!(load(Path::new("/nonexistent/unlockable-apps.toml")), Vec::<String>::new());
    }

    #[test]
    fn parses_a_real_file() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, r#"unlockable = ["org.kde.gcompris", "org.kde.ktuberling"]"#).unwrap();
        assert_eq!(
            load(f.path()),
            vec!["org.kde.gcompris".to_string(), "org.kde.ktuberling".to_string()]
        );
    }

    #[test]
    fn malformed_file_is_an_empty_list_not_a_crash() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "not valid toml {{{{").unwrap();
        assert_eq!(load(f.path()), Vec::<String>::new());
    }
}
