# omarchy-kids

A configuration layer on top of [Omarchy](https://omarchy.org) that grows with a child — age-tiered desktop profiles plus tooling for parental controls and screen time. Not a fork: Omarchy is installed normally, this project layers config, a background agent, and a control center on top.

**Status:** early concept / development environment setup. Not usable yet.

## How it fits together

- **Child computer:** runs Omarchy plus this project's config layer and a local agent.
- **Parent computer:** runs a Quickshell plugin + control center that talks to the agent over SSH.

See `docs/` for the architecture write-up once it lands (ported from the project's private design notes, stripped of any personal details).

## Layout

| Folder | Stack | Purpose |
|---|---|---|
| `tiers/` | shell/config | Age-tier config layer: Hyprland config, Quickshell modules, wallpaper/branding per tier, `omarchy-kids-set-tier`. |
| `agent/` | Rust | `omarchy-kids-agent` (CLI) + `omarchy-kids-agentd` (daemon), app wrapper. Runs on the child computer. |
| `control/` | C++ / Qt | Core library + GUI (Qt) + TUI. Runs on the parent computer, talks to the agent over SSH. |
| `quickshell-plugin/` | QML | Parent-side headerbar plugin: online/offline status, opens the control center. |
| `setup-wizard/` | — | One-time first-boot setup on the child computer (name, initial tier, pairing). |
| `docs/` | — | Architecture docs, roadmap. |
| `website/` | HTML/CSS | Landing page for omarchy-kids.com (English + German). |

See [`docs/packaging.md`](docs/packaging.md) for the packaging path convention shared by `agent/`, `tiers/`, `control/`, `quickshell-plugin/`, and `setup-wizard/`.

## License

MIT — see `LICENSE`.
