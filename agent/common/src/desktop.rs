use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

pub struct DesktopEntry {
    pub name: String,
    pub icon: String,
    pub exec: String,
}

/// Standard XDG application dirs, most-specific first — matches what
/// `gtk-launch`/desktop-file lookups already use, so a desktop id that shows
/// up in the launcher resolves the same way here.
pub fn find_desktop_file(desktop_id: &str) -> Option<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }
    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));

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
}
