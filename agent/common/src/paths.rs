use std::path::PathBuf;

/// Panics if `HOME`/`XDG_RUNTIME_DIR` are unset rather than guessing a UID —
/// wrong guesses on a multi-user box are worse than a clear startup failure.
fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME must be set"))
}

pub fn runtime_dir() -> PathBuf {
    PathBuf::from(
        std::env::var_os("XDG_RUNTIME_DIR")
            .expect("XDG_RUNTIME_DIR must be set (pam_systemd should set this for any login, including SSH)"),
    )
}

/// Directory holding the agentd socket. Created (0700) by agentd on startup.
pub fn socket_dir() -> PathBuf {
    runtime_dir().join("omarchy-kids")
}

pub fn socket_path() -> PathBuf {
    socket_dir().join("agentd.sock")
}

pub fn config_dir() -> PathBuf {
    home().join(".config/omarchy-kids")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn launcher_apps_path() -> PathBuf {
    config_dir().join("launcher-apps.json")
}

pub fn data_dir() -> PathBuf {
    home().join(".local/share/omarchy-kids")
}

pub fn db_path() -> PathBuf {
    data_dir().join("usage.sqlite3")
}
