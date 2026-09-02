use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

pub struct DesktopEntry {
    pub name: String,
    pub icon: String,
    pub exec: String,
}

/// Desktop ids are conventionally reverse-DNS-style dotted identifiers
/// (`org.kde.gcompris`) or a single dash/underscore-separated token
/// (`tuxpaint-fullscreen`) — reject anything that could escape the intended
/// `<dir>/<id>.desktop` join (path separators, `.`/`..` components, NUL/
/// control characters) before it ever reaches a filesystem lookup (see
/// issue #38). Every caller that accepts a desktop id from outside this
/// process (wrapper CLI args, agentd request payloads) must call this first.
pub fn is_valid_desktop_id(desktop_id: &str) -> bool {
    if desktop_id.is_empty() || desktop_id == "." || desktop_id == ".." {
        return false;
    }
    desktop_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Standard XDG application dirs, most-specific first — matches what
/// `gtk-launch`/desktop-file lookups already use, so a desktop id that shows
/// up in the launcher resolves the same way here.
///
/// Kiosk-launch resolution deliberately does **not** consult
/// `~/.local/share/applications` (issue #35): that directory is
/// child-writable, and nothing in the current tier line-ups needs a
/// user-installed desktop entry — only the system-wide, root-installed
/// directories are trusted for resolving what a launch/unlock/tier-switch
/// request is allowed to point at.
pub fn find_desktop_file(desktop_id: &str) -> Option<PathBuf> {
    if !is_valid_desktop_id(desktop_id) {
        return None;
    }
    let dirs = [
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/usr/share/applications"),
    ];

    dirs.into_iter()
        .map(|dir| dir.join(format!("{desktop_id}.desktop")))
        .find(|p| p.is_file())
}

pub fn parse_desktop_entry(path: &Path) -> Result<DesktopEntry> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

    let mut in_main_section = false;
    let mut name = None;
    let mut icon = None;
    let mut exec = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_main_section = line == "[Desktop Entry]";
            continue;
        }
        if !in_main_section {
            continue;
        }
        if let Some(v) = line.strip_prefix("Name=") {
            name.get_or_insert_with(|| v.to_string());
        } else if let Some(v) = line.strip_prefix("Icon=") {
            icon.get_or_insert_with(|| v.to_string());
        } else if let Some(v) = line.strip_prefix("Exec=") {
            exec.get_or_insert_with(|| v.to_string());
        }
    }

    Ok(DesktopEntry {
        name: name.unwrap_or_else(|| desktop_id_from_path(path)),
        icon: icon.unwrap_or_default(),
        exec: exec.ok_or_else(|| anyhow!("no Exec= line in {}", path.display()))?,
    })
}

fn desktop_id_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("app")
        .to_string()
}

/// Turns a desktop entry's `Exec=` line into an argv, stripping field codes
/// (`%f`, `%F`, `%u`, `%U`, `%i`, `%c`, `%k`, ...) per the Desktop Entry
/// Specification. Quoting support is deliberately minimal (double quotes with
/// backslash escapes) — Exec= lines in practice don't use the fuller POSIX
/// shell grammar the spec technically allows.
pub fn exec_to_argv(exec: &str) -> Vec<String> {
    split_shell_words(exec)
        .into_iter()
        .filter(|tok| !(tok.len() == 2 && tok.starts_with('%')))
        .map(|tok| strip_field_codes(&tok))
        .filter(|tok| !tok.is_empty())
        .collect()
}

fn split_shell_words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => in_quotes = !in_quotes,
            '\\' if in_quotes => {
                if let Some(next) = chars.next() {
                    cur.push(next);
                }
            }
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn strip_field_codes(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            chars.next(); // drop the code character itself, e.g. the 'f' in "%f"
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_exec() {
        assert_eq!(exec_to_argv("gcompris-qt"), vec!["gcompris-qt"]);
    }

    #[test]
    fn exec_with_args_and_field_code() {
        assert_eq!(
            exec_to_argv("tuxpaint --fullscreen=native %f"),
            vec!["tuxpaint", "--fullscreen=native"]
        );
    }

    #[test]
    fn quoted_exec() {
        assert_eq!(
            exec_to_argv(r#"app "--title=Hello World""#),
            vec!["app", "--title=Hello World"]
        );
    }

    #[test]
    fn valid_desktop_ids() {
        assert!(is_valid_desktop_id("org.kde.gcompris"));
        assert!(is_valid_desktop_id("tuxpaint-fullscreen"));
        assert!(is_valid_desktop_id("some_app.v2"));
    }

    #[test]
    fn rejects_path_traversal_and_control_chars() {
        assert!(!is_valid_desktop_id(""));
        assert!(!is_valid_desktop_id("."));
        assert!(!is_valid_desktop_id(".."));
        assert!(!is_valid_desktop_id("../../etc/passwd"));
        assert!(!is_valid_desktop_id("foo/bar"));
        assert!(!is_valid_desktop_id("foo\0bar"));
        assert!(!is_valid_desktop_id("foo\nbar"));
        assert!(!is_valid_desktop_id("foo bar"));
    }

    #[test]
    fn find_desktop_file_rejects_invalid_id_before_lookup() {
        assert_eq!(find_desktop_file("../../etc/passwd"), None);
    }
}
