#!/bin/bash
# Pairs a dev child VM for real use with Control Center's dashboard — unlike
# vm-pairing-smoke-test.sh (which throws its key away, it's a protocol smoke
# test), this writes a durable keypair under
# ~/.local/share/omarchy-kids-control/keys/ and prints a ready-to-paste
# [[hosts]] block for ~/.config/omarchy-kids-control/hosts.toml.
#
# Opens/closes the pairing port's UFW rule itself (over SSH, sudo via the
# fixed dev password — see vm-recreate.sh's header comment) since a fresh
# dev VM has no wizard to do it, unlike production (docs/agent-protocol.md's
# "UFW rule around the pairing window").
#
# Usage: vm-pair-for-dashboard.sh <host-or-ip> <name-for-hosts-toml> [ssh-key]
set -euo pipefail

CHILD_HOST="${1:?Usage: vm-pair-for-dashboard.sh <host-or-ip> <name> [ssh-key]}"
NAME="${2:?Usage: vm-pair-for-dashboard.sh <host-or-ip> <name> [ssh-key]}"
KEY="${3:-$HOME/.ssh/omarchy_kids_dev}"
CHILD_USER="devchild"
CHILD_PASSWORD="omarchy-kids-dev"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PAIRING_BIN="$REPO_ROOT/agent/target/release/omarchy-kids-pairing"
KEY_OUT="$HOME/.local/share/omarchy-kids-control/keys/$NAME"

ssh_opts=(-o BatchMode=yes -o StrictHostKeyChecking=accept-new -i "$KEY")

echo "==> Opening the pairing port (7420/tcp) on $CHILD_HOST"
ssh -tt "${ssh_opts[@]}" "$CHILD_USER@$CHILD_HOST" \
  "echo '$CHILD_PASSWORD' | sudo -S ufw allow 7420/tcp"

echo "==> Starting 'omarchy-kids-pairing serve' on the child"
ssh "${ssh_opts[@]}" "$CHILD_USER@$CHILD_HOST" \
  "nohup omarchy-kids-pairing serve --timeout-minutes 5 > /tmp/pairing-serve.log 2>&1 < /dev/null &"
sleep 2

SERVE_LOG=$(ssh "${ssh_opts[@]}" "$CHILD_USER@$CHILD_HOST" "cat /tmp/pairing-serve.log")
PAIR_CODE="$(grep '^Pairing code:' <<<"$SERVE_LOG" | awk '{print $NF}')"
SESSION_ID="$(grep '^Session:' <<<"$SERVE_LOG" | awk '{print $NF}')"
PORT="$(grep '^Port:' <<<"$SERVE_LOG" | awk '{print $NF}')"

if [[ -z $PAIR_CODE || -z $SESSION_ID || -z $PORT ]]; then
  echo "!! Couldn't read pairing code/session/port. serve output:" >&2
  echo "$SERVE_LOG" >&2
  exit 1
fi

echo "==> Pairing (sid=$SESSION_ID)"
mkdir -p "$(dirname "$KEY_OUT")"
rm -f "$KEY_OUT" "$KEY_OUT.pub"
PAIR_OUT="$("$PAIRING_BIN" pair --host "$CHILD_HOST" --port "$PORT" --sid "$SESSION_ID" \
  --code "$PAIR_CODE" --key-out "$KEY_OUT" --yes)"
echo "$PAIR_OUT" >&2

RESULT_JSON="$(grep '^PAIR_RESULT:' <<<"$PAIR_OUT" | sed 's/^PAIR_RESULT: //')"
if [[ -z $RESULT_JSON ]]; then
  echo "!! 'pair' produced no PAIR_RESULT line" >&2
  exit 1
fi

FINGERPRINT="$(jq -r '.fingerprint' <<<"$RESULT_JSON")"
USERNAME="$(jq -r '.username' <<<"$RESULT_JSON")"
SSH_PORT="$(jq -r '.ssh_port' <<<"$RESULT_JSON")"

echo "==> Closing the pairing port again"
ssh -tt "${ssh_opts[@]}" "$CHILD_USER@$CHILD_HOST" \
  "echo '$CHILD_PASSWORD' | sudo -S ufw delete allow 7420/tcp" || true

echo "==> Verifying"
timeout 8 ssh -o BatchMode=yes -i "$KEY_OUT" -p "$SSH_PORT" "$USERNAME@$CHILD_HOST" status --json

cat <<EOF

Paired. Add this to ~/.config/omarchy-kids-control/hosts.toml:

[[hosts]]
name = "$NAME"
hostname = "$CHILD_HOST"
ssh_port = $SSH_PORT
username = "$USERNAME"
key_path = "$KEY_OUT"
fingerprint = "$FINGERPRINT"
paired_at = "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
EOF
