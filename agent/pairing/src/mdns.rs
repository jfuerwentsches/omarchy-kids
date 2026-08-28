//! Child-side mDNS broadcast for pairing discovery (issue #22). Discovery
//! only — no secret ever goes in a TXT record. Only active while `serve`
//! (main.rs) is actually listening for a pairing attempt, never as a
//! standing background service: the hub note's "SSH is the only open
//! service" principle stays true outside this deliberately time-boxed
//! window (see the vault note's "Warum ein eigener Listener" section).

use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceInfo};

pub const SERVICE_TYPE: &str = "_omarchy-kids-pairing._tcp.local.";

pub struct Broadcast {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Broadcast {
    pub fn start(hostname: &str, sid: &str, port: u16) -> Result<Self> {
        let daemon = ServiceDaemon::new().context("starting the mDNS daemon")?;
        let service_hostname = format!("{hostname}.local.");
        let properties = [("v", "1"), ("sid", sid)];

        let info = ServiceInfo::new(
            SERVICE_TYPE,
            hostname,
            &service_hostname,
            "",
            port,
            &properties[..],
        )
        .context("building mDNS service info")?
        .enable_addr_auto();

        let fullname = info.get_fullname().to_string();
        daemon
            .register(info)
            .context("registering the mDNS service")?;

        Ok(Self { daemon, fullname })
    }
}

impl Drop for Broadcast {
    fn drop(&mut self) {
        // Best-effort: the pairing window is over either way once `serve`
        // exits, but withdrawing the announcement promptly avoids a stale
        // entry lingering in other hosts' mDNS caches.
        let _ = self.daemon.unregister(&self.fullname);
    }
}
