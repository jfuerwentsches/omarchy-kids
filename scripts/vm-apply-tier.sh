#!/bin/bash
# Rsyncs tiers/ to a dev child VM and runs omarchy-kids-set-tier there —
# the step that turns the bare infrastructure vm-recreate.sh/
# vm-deploy-agent.sh/vm-pair-for-dashboard.sh produce into an actual locked-
# down kiosk. Kept as a separate, explicit step rather than folded into
# vm-recreate.sh: the two test scenarios (a) "try things on a finished kids
# computer" and (b) "walk through creating a new one, incl. pairing" want
# different starting points — see docs/dev-vm-setup.md's "Fast path" /
# `vm-snapshot.sh`'s `fresh-boot` (pre-tier) vs `kiosk-ready` (post-tier)
# snapshots.
#
# Non-interactive `sudo` for the getty-masking step inside
# omarchy-kids-set-tier: pre-authenticates with `sudo -S -v` over the same
# `ssh -tt` session so the script's own bare `sudo systemctl mask` calls
# reuse the cached credential (see docs/dev-vm-setup.md's "Known gotchas"
# for why plain `sudo` over SSH otherwise fails silently — that script's
# `|| true` means a failed mask wouldn't even error out, just silently not
# apply the VT lockdown).
#
# Usage: vm-apply-tier.sh <host-or-ip> [tier] [ssh-key]
set -euo pipefail

HOST="${1:?Usage: vm-apply-tier.sh <host-or-ip> [tier] [ssh-key]}"
TIER="${2:-mini}"
KEY="${3:-$HOME/.ssh/omarchy_kids_dev}"
CHILD_USER="devchild"
CHILD_PASSWORD="omarchy-kids-dev"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Syncing tiers/ to $HOST"
rsync -a --delete -e "ssh -i $KEY -o StrictHostKeyChecking=accept-new" \
  "$REPO_ROOT/tiers/" "$CHILD_USER@$HOST:~/omarchy-kids-tiers/"

echo "==> Applying tier '$TIER'"
ssh -tt -o StrictHostKeyChecking=accept-new -i "$KEY" "$CHILD_USER@$HOST" \
  "chmod +x ~/omarchy-kids-tiers/omarchy-kids-set-tier && echo '$CHILD_PASSWORD' | sudo -S -v && ~/omarchy-kids-tiers/omarchy-kids-set-tier $TIER"

echo "==> Verifying VT lockdown + launcher plugin"
ssh -i "$KEY" "$CHILD_USER@$HOST" \
  "systemctl is-enabled getty@tty2; ls ~/.config/omarchy/plugins/"

echo "==> Done. Consider: scripts/vm-snapshot.sh save kiosk-ready"
