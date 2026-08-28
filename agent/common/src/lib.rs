//! Shared types between `omarchy-kids-agent` (CLI), `omarchy-kids-agentd`
//! (session daemon), `omarchy-kids-run` (app wrapper) and
//! `omarchy-kids-override-helper` (polkit-gated unlock helper).

pub mod config;
pub mod desktop;
pub mod paths;
pub mod protocol;
pub mod transport;
