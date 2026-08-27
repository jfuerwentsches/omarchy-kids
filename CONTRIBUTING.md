# Contributing

This is a monorepo covering a few different tech stacks. Pick the folder for the area you're touching:

| Folder | Stack | Build |
|---|---|---|
| `agent/` | Rust (2021 edition) | `cd agent && cargo build` |
| `control/` | C++20 / Qt6 / CMake | `cmake -B control/build -S control -G Ninja && cmake --build control/build` |
| `tiers/`, `setup-wizard/` | shell | no build step |
| `quickshell-plugin/` | QML | loaded by Quickshell directly |

## Issues

Use `area:agent`, `area:control-center`, `area:tiers`, `area:quickshell-plugin`, `area:setup-wizard` labels so things stay navigable despite the single repo.

## Versioning

The project ships one version tag across all components — agent and control center are always released together, so there's no cross-component compatibility matrix to track.
