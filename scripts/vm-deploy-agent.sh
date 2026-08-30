#!/bin/bash
# Builds the agent/ Rust workspace and deploys it to a dev child VM created
# by vm-recreate.sh (or manually per docs/dev-vm-setup.md): copies the
# binaries to /usr/bin, installs the agentd systemd --user unit, and starts
# it. Fully non-interactive — uses the fixed dev-only password (see
# vm-recreate.sh's header comment) to answer `sudo` over SSH via `sudo -S`
# through a forced pseudo-tty (`ssh -tt`), since plain `sudo` over a
# non-interactive SSH session refuses to read a password at all (see
# docs/dev-vm-setup.md's "Known gotchas").
#
# Usage: vm-deploy-agent.sh <host-or-ip> [ssh-key]
set -euo pipefail

HOST="${1:?Usage: vm-deploy-agent.sh <host-or-ip> [ssh-key]}"
KEY="${2:-$HOME/.ssh/omarchy_kids_dev}"
CHILD_USER="devchild"
CHILD_PASSWORD="omarchy-kids-dev"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
AGENT_DIR="$REPO_ROOT/agent"

ssh_opts=(-o BatchMode=yes -o StrictHostKeyChecking=accept-new -i "$KEY")

echo "==> Building agent/ (release)"
(cd "$AGENT_DIR" && cargo build --release --workspace)

echo "==> Copying binaries to $HOST:/tmp"
scp "${ssh_opts[@]}" \
  "$AGENT_DIR/target/release/omarchy-kids-agent" \
  "$AGENT_DIR/target/release/omarchy-kids-agentd" \
  "$AGENT_DIR/target/release/omarchy-kids-pairing" \
  "$AGENT_DIR/target/release/omarchy-kids-override-helper" \
  "$AGENT_DIR/target/release/omarchy-kids-repair-helper" \
  "$AGENT_DIR/target/release/omarchy-kids-run" \
  "$AGENT_DIR/packaging/systemd/omarchy-kids-agentd.service" \
  "$CHILD_USER@$HOST:/tmp/"

echo "==> Installing to /usr/bin (sudo via known dev password)"
ssh -tt "${ssh_opts[@]}" "$CHILD_USER@$HOST" \
  "echo '$CHILD_PASSWORD' | sudo -S install -m755 /tmp/omarchy-kids-agent /tmp/omarchy-kids-agentd /tmp/omarchy-kids-pairing /tmp/omarchy-kids-override-helper /tmp/omarchy-kids-repair-helper /tmp/omarchy-kids-run /usr/bin/"

# Without lingering, agentd's `graphical-session.target` binding gets torn
# down the moment this (or any) `ssh -tt` session ends, since a fresh
# unattended VM (via vm-recreate.sh) has nobody actually logged into the
# graphical session to keep it alive — found 2026-08-31: agentd started
# fine, then systemd stopped it ~11s later the moment the enabling SSH
# session's pseudo-tty closed. `enable-linger` keeps the user's systemd
# instance (and anything bound to it) running independent of login sessions.
echo "==> Enabling lingering so agentd survives this SSH session ending"
ssh -tt "${ssh_opts[@]}" "$CHILD_USER@$HOST" \
  "echo '$CHILD_PASSWORD' | sudo -S loginctl enable-linger $CHILD_USER"

echo "==> Installing + starting the agentd systemd --user unit"
ssh "${ssh_opts[@]}" "$CHILD_USER@$HOST" bash -s <<'EOF'
set -e
mkdir -p ~/.config/systemd/user
cp /tmp/omarchy-kids-agentd.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now omarchy-kids-agentd
sleep 1
systemctl --user is-active omarchy-kids-agentd
omarchy-kids-agent status --json
EOF

echo "==> Done."
