# Contributing

This is a monorepo covering a few different tech stacks. Pick the folder for the area you're touching:

| Folder | Stack | Build |
|---|---|---|
| `agent/` | Rust (2021 edition) | `cd agent && cargo build` |
| `control/` | C++20 / Qt6 / CMake | `cmake -B control/build -S control -G Ninja && cmake --build control/build` |
| `tiers/`, `setup-wizard/` | shell | no build step |
| `quickshell-plugin/` | QML | loaded by Quickshell directly |

Packaging notes live in [`docs/packaging.md`](docs/packaging.md).

## Issues

Use `area:agent`, `area:control-center`, `area:tiers`, `area:quickshell-plugin`, `area:setup-wizard` labels so things stay navigable despite the single repo.

## Versioning

The project ships one version tag across all components — agent, control center, tiers, quickshell plugin, and setup wizard are always released together, so there is no supported parent/child compatibility matrix to track.

That means `omarchy-kids-*` package updates are intentionally synchronized: if one component moves, the rest move with it. If a deployed machine reports a version mismatch, treat it as a rollout/upgrade warning, not as a steady-state configuration to support long term.
