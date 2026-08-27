/// Background daemon (systemd user service) running in the parent/admin account.
/// Owns time-budget enforcement, the app-wrapper event socket, the PIN/polkit
/// override path, and the SQLite usage log. See docs for the full design.
fn main() {
    todo!("listen on the local unix socket, enforce time budgets, write the SQLite log");
}
