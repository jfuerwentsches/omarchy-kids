# setup-wizard

One-time first-boot setup on the child computer, hooking into Omarchy 4's deferred first-boot provisioning. Implementation plan: vault note "Omarchy Kids - Implementierung Setup-Wizard"; progress tracked on the [Pairing & Setup Wizard project](https://github.com/users/jfuerwentsches/projects/3) (issues #16-#29).

## Status

**Phase 1 (bootstrap logic, no UI) is implemented and verified end-to-end in the dev VM (2026-08-29)** — see `bootstrap/`:

- `bootstrap/omarchy-kids-bootstrap` — main entry point, run as root:
  ```
  sudo omarchy-kids-bootstrap --child-user <linux-username> --child-name "<display name>" --tier <tier> [--theme <theme-id>] [--admin-user <name>] [--admin-password <password>]
  ```
- `bootstrap/lib/branding-tier.sh` — renders the child's name as a screensaver banner and calls `omarchy-kids-set-tier <tier> [theme]` (issue #19). The vault note's original plan (`omarchy ascii "<name>"`) doesn't match the real Omarchy 4.0.1 CLI — there's no text-to-banner command, only the interactive `omarchy branding screensaver <image|text|reset>`. This reproduces its `image` mode non-interactively: render an SVG with the name, rasterize with `rsvg-convert`, transcode with `omarchy-transcode-ascii --mode block --invert` (verified by hand first, see git history).
- `bootstrap/lib/account.sh` — creates the separate parent/admin account (default name `omarchy-kids-parent`) and removes the child's own, Omarchy-created account from `wheel` if it's a member (issue #16). No interactive password prompt yet — pass `--admin-password` or let it generate and print one once; the real parent-facing form (issue #27) replaces this. Also installs a narrow NOPASSWD sudoers grant (`/etc/sudoers.d/zz-omarchy-kids-getty-lockdown`) for exactly the getty-mask/unmask commands `omarchy-kids-set-tier` needs for its VT lockdown step — without it, that step silently no-ops once the child loses `wheel` (or even non-interactively before that, since a tty-less sudo call has nothing to prompt on), quietly disabling the kiosk's actual access-control boundary. Filename deliberately sorts last: sudoers is last-match-wins per exact command line, and the dev VM had a pre-existing broader `fine ALL=(ALL) ALL` rule (in `/etc/sudoers.d/04_fine`, a dev-only convenience — see `docs/dev-vm-setup.md`) that silently shadowed an earlier "00-"-prefixed filename here during testing.
- `bootstrap/lib/ssh-key.sh` — generates the agent's ed25519 keypair and installs a `command=`-restricted `authorized_keys` entry for the child account; stashes a root-only copy of the private half for the pairing step to pick up later (issue #17).
- UFW's SSH allow rule is deliberately **not** duplicated here — `omarchy-kids-agent`'s package `post_install` already does it (see `agent/packaging/PKGBUILD`, `docs/agent-protocol.md`). The bootstrap script only logs a warning if UFW is active without it (issue #18).

Not yet done: the actual first-boot hook / parent-facing form (issue #27, blocked on researching Omarchy 4's provisioning hook API, issue #26), and the pairing step (mDNS broadcast + QR fallback, issues #21-#25). Also not done: packaging `tiers/` (see its README) — the dev-VM verification above worked around this by copying the tier data alongside a manually placed `omarchy-kids-set-tier` on `PATH`, since the script resolves sibling tier directories relative to its own (symlink-resolved) location.
