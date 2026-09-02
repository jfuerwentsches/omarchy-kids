# Packaging

Omarchy Kids packages follow one path convention:

- real files live under `/usr/lib/omarchy-kids/<component>/`
- user-facing entry points get a symlink from `/usr/bin/`

That matters for more than tidiness. `tiers/omarchy-kids-set-tier` resolves
its shipped tier directories as siblings of its own resolved path, and
`setup-wizard/first-boot/omarchy-kids-setup-wizard` discovers tiers by
calling that same binary via `command -v` + `readlink -f`. If the script
only existed in `/usr/bin`, those sibling lookups would break. The same
pattern keeps future packaged components from reinventing their own
ad-hoc path layout.

Current packages:

- `agent/packaging/PKGBUILD` is the canonical example.
- `tiers/packaging/PKGBUILD` installs `omarchy-kids-set-tier` plus the
  tier directories under `/usr/lib/omarchy-kids/tiers/`, with a
  `/usr/bin/omarchy-kids-set-tier` symlink.
- `control/packaging/PKGBUILD` installs `omarchy-kids-control` to
  `/usr/bin/` and its systemd user units to `/usr/lib/systemd/user/`.
- `quickshell-plugin/packaging/PKGBUILD` installs the plugin files to
  `/usr/share/omarchy-kids/quickshell-plugin/` and a user-run enable helper
  to `/usr/bin/`.
- `setup-wizard/packaging/PKGBUILD` installs the wizard scripts under
  `/usr/lib/omarchy-kids/setup-wizard/` and symlinks their entry points
  into `/usr/bin/`.

When adding a new package, copy the relevant `post_install()` /
`post_upgrade()` / `post_remove()` pattern from
`agent/packaging/PKGBUILD` and adjust only the install paths and service
names needed for the new component.
