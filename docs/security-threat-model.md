# Security threat model

Written closing out issue #39, after implementing the fixes for the
2026-08-30 security audit (issues #32–#38 — see the
[Security & Hardening project board](https://github.com/users/jfuerwentsches/projects/1)).
Describes the trust boundaries that actually exist in the code today, what
each one defends against, and — as importantly — which gaps were fixed
outright versus accepted as residual risk and why. Companion to
`docs/agent-protocol.md` (wire formats/component split) and
`docs/dev-vm-setup.md` (how to actually exercise any of this).

## Trust boundaries

### 1. Child process ↔ agentd (Unix socket)

`agentd` runs as the child's own systemd `--user` service (see
`docs/agent-protocol.md`'s "Account topology") — it is not a separate,
more-privileged account. Every legitimate caller reachable today
(a local app, `omarchy-kids-override-helper` after `pkexec --user <child>`,
and Control Center's SSH-invoked `omarchy-kids-agent` — see boundary 2)
connects to this socket **as the child's own uid**. `SO_PEERCRED` can tell
agentd "this connection's peer uid is N", but N is the same for a
legitimate call and a hypothetical malicious one running as the child, so
it cannot be used to gate anything here — this was confirmed in practice
while implementing #32: `SetTier`'s only real caller
(`tiers/omarchy-kids-set-tier`, invoked via `sudo -u <child_user>` from
`setup-wizard/bootstrap/lib/branding-tier.sh`) turned out to run at the
child's own uid too, the same as `Unlock`.

**Accepted residual risk:** a malicious process running as the child
account can reach every request `agentd` accepts, including `Unlock` and
`SetTier`, with no caller-identity check beyond "is on this Unix socket."
Closing this completely would require a separate system account for the
SSH-key/privileged path, distinct from the child's own session account —
a materially bigger architectural change than this audit's scope, and not
undertaken here.

**Mitigation actually implemented — validate content, not the caller:**
- `Unlock` may only grant a desktop id on a parent-curated allow-list
  (`~/.config/omarchy-kids/unlockable-apps.toml`, `agent/agentd/src/
  allowlist.rs`) — an absent or empty file means *nothing* is unlockable
  via this path, a secure default. `Override` (the PIN/`pkexec` path) is
  exempt: it only ever runs after a real, live polkit `auth_admin`
  authentication happened moments earlier (see boundary 3), a stronger
  in-the-moment check the allow-list would only duplicate.
- `SetTier` only accepts a `tier` from a small, hardcoded enumeration
  (`KNOWN_TIERS` in `agent/agentd/src/handlers.rs` — just `"mini"` today,
  matching the "Current focus" in root `CLAUDE.md`) and rejects any `apps`
  tile whose `desktop_id` doesn't resolve to a real, installed `.desktop`
  file. This can't stop a malicious child-uid process from switching to a
  *different, still-installed* tier's own app list once a second tier
  ships, but it does stop fabricating tiles with an attacker-chosen
  `Exec=` line — the actually dangerous part of an unauthenticated
  `SetTier`.
- Every desktop id accepted from outside `agentd` (via `Unlock`, `SetTier`,
  or the wrapper) is validated by `omarchy_kids_common::desktop::
  is_valid_desktop_id` before ever reaching a filesystem path join (issue
  #38) — rejects path separators, `.`/`..`, and control characters.
  `find_desktop_file` also no longer consults the child-writable
  `~/.local/share/applications` at all (issue #35) — only the two
  system-wide, root-installed application directories are trusted.
- `agent/agentd/src/main.rs`'s socket now bounds message size (64 KiB,
  `omarchy_kids_common::transport::MAX_LINE_BYTES`), caps concurrent
  connections (32), and applies a 10s idle timeout per connection (issue
  #36) — hardening against a hostile/misbehaving local peer, independent
  of the residual-risk question above.

**Not mitigated, accepted as-is:** the SQLite usage log
(`~/.local/share/omarchy-kids/usage.sqlite3`) and `config.toml` are also
child-writable, for the same one-account reason. A malicious child-uid
process could open the DB file directly (bypassing `agentd`'s API
entirely) and alter usage history or delete security-event rows. No
mitigation was implemented for this — it's downstream of the same
accepted architectural limitation above, and the usage log's purpose
(transparency for the parent, not tamper-proof auditing against the
child) doesn't currently justify the cost of a different storage design.

### 2. Control Center ↔ agent (SSH)

Control Center's `AgentClient` runs `ssh ... omarchy-kids-agent <args>`
against a `command=`-restricted `authorized_keys` entry installed in the
**child's own** account during pairing (see boundary 4) — so, per boundary
1 above, an authenticated SSH call and a local child-uid call are
indistinguishable to `agentd`. This is exactly why boundary 1's mitigation
had to be content-based rather than caller-based: there is no cryptographic
signal `agentd` could check to tell them apart even in principle, short of
the bigger account-separation redesign noted above.

What SSH *does* verify here: that the caller holds the Control Center's
private key, and (since issue #33) that the host being connected to is the
one that came through the authenticated pairing exchange — see boundary 4.

### 3. Override (`pkexec`)

`omarchy-kids-agent override <app>` runs
`pkexec --user <child> omarchy-kids-override-helper ...`. The polkit action
(`agent/packaging/polkit/net.omarchykids.agent.policy`) requires
`auth_admin`, which on a correctly set up child computer means the
*separate* parent/admin account must authenticate — not the child's own,
non-admin account. This is a real, cryptographically-enforced boundary
(polkit + PAM), unlike boundaries 1/2 above: the helper process still ends
up running as the child (`--user <child>`), but only after that
authentication succeeded. This is why `handle_unlock` exempts
`via_override` requests from the boundary-1 allow-list — the authorization
already happened, more strongly, immediately before.

Depends on account topology the setup wizard is responsible for (the child
account must not be in `wheel`/the polkit admin group) — enforcing that is
out of this package's scope, per the polkit policy file's own comment.

### 4. Pairing exchange (SPAKE2 + host-key binding)

`agent/pairing` authenticates the pairing exchange itself via SPAKE2 over
the pairing code (see `agent/pairing/src/proto.rs`'s module doc for the
crypto rationale, including the accepted lack of an independent security
audit for the `spake2` crate). Two gaps closed by this audit:

- **Issue #34 — key installed before confirmation.** Previously, the
  Control Center's public key was written to the child's
  `authorized_keys` *before* the parent's fingerprint confirmation (`Ack`)
  was even received — a declined or dropped connection still left the key
  installed. Fixed: `handle_connection` (`agent/pairing/src/main.rs`) now
  only validates the key and computes its fingerprint before sending
  `Confirm`; the actual `write_to_authorized_keys` call happens strictly
  after a received `Ack { confirmed: true }`.
- **Issue #33 — the first real SSH connection was pure TOFU.** SPAKE2
  authenticates the pairing exchange, but nothing tied that authenticated
  exchange to the specific SSH host the parent would connect to
  afterward — `StrictHostKeyChecking=accept-new` trusted whatever key
  answered first. Fixed: `serve` now reads the child's real sshd host key
  (`/etc/ssh/ssh_host_ed25519_key.pub`, falling back to
  `ssh_host_rsa_key.pub`) and sends it through the already-authenticated
  channel as `Confirm::ssh_host_public_key`. Control Center's
  `AgentClient` (`control/core/src/agent_client.cpp`) pins this key into a
  per-call, process-local known_hosts file (`PinnedKnownHosts`) and
  connects with `StrictHostKeyChecking=yes` against it — the trusted key
  is the one that came through pairing, not whichever one answers.
  Falls back to the old `accept-new` behavior only for a host paired
  before this field existed (`sshHostPublicKey` empty); re-pairing picks
  up a pinned key.
- **Issue #37 — one bad connection ended the whole pairing window.**
  `serve` previously bound, accepted exactly one TCP connection, and
  exited — a malformed message or a stalled/dropped connection forced the
  parent to reopen pairing from scratch. Fixed: the listener is now bound
  once for the whole window, and a failed pairing *attempt* is logged and
  the accept loop continues (mDNS broadcast stays live throughout) until
  either a successful pairing or the overall timeout.
- **Issue #36 — unbounded reads.** `proto::read_message` now uses the same
  bounded-line reader as the agentd socket (`omarchy_kids_common::
  transport::read_line_bounded`, 64 KiB cap) instead of an unbounded
  `read_line()`.

### 5. Other accepted residual risks (recorded elsewhere, cross-referenced here)

- **`librespot`'s `persist_credentials: true`** (not part of this audit;
  documented 2026-08-27 in the private vault note "Omarchy Kids -
  Architekturuebersicht"): the Spotify Connect credential blob handed over
  during Zeroconf pairing is cached on the child computer so playback
  survives independently of the parent computer after the first pairing.
  Accepted trade-off: a reusable Spotify session credential (not a
  plaintext account password) sits permanently on the child computer —
  physical access to the child machine could hijack the Connect session.
  Audio itself always streams directly between the child computer and
  Spotify's servers, never relayed through the parent computer.

## What was deliberately *not* built

- **A separate SSH-key/privileged system account distinct from the child's
  session account.** This is the one change that would let `agentd`
  actually distinguish an SSH/Control-Center caller (or a `SetTier`/PIN
  caller) from an arbitrary local child-uid process. Not undertaken —
  content validation (boundary 1) was judged proportionate for a
  parental-control tool's threat model (a technically curious child on
  their own kiosk machine, not a hostile third party with local code
  execution as a stepping stone to something else).
- **Tamper-evident/append-only usage logging.** The SQLite usage log stays
  plain, child-writable SQLite; no signing, no separate privileged writer.

## Test coverage added alongside these fixes

- `agent/agentd/src/handlers.rs` — allow-list enforcement
  (`unlock_is_permitted`) and known-tier validation, as pure-function unit
  tests (no real socket/filesystem needed).
- `agent/agentd/src/allowlist.rs` — missing file → empty list, malformed
  file → empty list (not a crash), a real file parses correctly.
- `agent/common/src/desktop.rs` — `is_valid_desktop_id` accepts real ids
  and rejects path traversal/control characters/whitespace;
  `find_desktop_file` rejects an invalid id before ever touching the
  filesystem.
- `agent/common/src/transport.rs` — `read_line_bounded`: normal lines,
  CRLF stripping, clean EOF, an oversized line (both with and without a
  terminator), a line exactly at the cap, and reading a second line after
  the first on the same reader.
- `control/tests/host_registry_test.cpp` — `sshHostPublicKey` round-trips
  through `hosts.toml`, and a pre-#33 entry with no such key in its TOML
  table loads as an empty string rather than failing to parse.

**Deliberately not covered by an automated test, consistent with
`control/`'s existing testing philosophy** (GUI/SSH code is verified
against the real dev VM, not unit tested — see root `CLAUDE.md`'s
"Status" log): `PairingDialog`'s stdout-parsing contract, and
`AgentClient`'s actual SSH host-key pinning behavior end-to-end (that the
pinned known_hosts file really does cause `ssh` to reject a
different/spoofed host key). Both should be re-verified against the dev
VM the next time pairing or the dashboard's SSH path is touched, per
`docs/dev-vm-setup.md`.

**Not done:** an explicit "hostile local client" test throwing raw garbage
bytes at the agentd socket end-to-end (as opposed to the unit-level
`read_line_bounded` coverage above, which already exercises the same
boundary in isolation). Left as a follow-up rather than blocking this
batch — `read_line_bounded`'s unit tests already cover the actual
size-bounding logic that would matter here.
