#!/bin/bash
# Automates the pairing round trip against an already-provisioned dev VM
# (child account created, omarchy-kids-agent/omarchy-kids-pairing on PATH,
# UFW allowing SSH — i.e. past omarchy-kids-bootstrap, see docs/dev-vm-setup.md's
# "Quick reference" and setup-wizard/README.md). A fast, scriptable stand-in
# for manually running `omarchy-kids-pairing serve` on the child, reading the
# pairing code off a screenshot, and typing `pair` by hand on the host: this
# runs the reference `pair` CLI with --yes (see agent/pairing/src/main.rs —
# "For scripting/testing", NOT the real Control Center GUI) and checks the
# machine-readable `PAIR_RESULT:` JSON line plus a live `agent status` call
# through the freshly installed restricted key, instead of eyeballing a
# `virsh screenshot`.
#
# Does NOT replace the full from-ISO end-to-end test in
# docs/agent-protocol.md's "End-to-end verification" section (first boot →
# wizard → pairing, driven via virsh screenshot/send-key) — that's the
# periodic check that the whole chain still wires together, including the
# tty1 gum form. This only re-exercises the pairing protocol itself, so it's
# safe to run on every iteration without redoing the account/tier setup.
#
# Usage: vm-pairing-smoke-test.sh <child_host> <child_user>
# Example: vm-pairing-smoke-test.sh 192.168.122.109 fine
set -euo pipefail

CHILD_HOST="${1:?Usage: vm-pairing-smoke-test.sh <child_host> <child_user>}"
CHILD_USER="${2:?Usage: vm-pairing-smoke-test.sh <child_host> <child_user>}"

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

SERVE_OUT="$WORK_DIR/serve.out"
KEY_OUT="$WORK_DIR/control-center-key"

echo "vm-pairing-smoke-test: starting 'omarchy-kids-pairing serve' on $CHILD_USER@$CHILD_HOST..." >&2
# Serve is a one-shot listener — accepts exactly one attempt, then exits (see
# serve's own doc comment in agent/pairing/src/main.rs). --no-mdns because
# this test dials in with an explicit host/port/sid/code anyway, and mDNS on
# a libvirt NAT bridge is one more thing that can flake in a scripted run.
ssh "$CHILD_USER@$CHILD_HOST" "omarchy-kids-pairing serve --timeout-minutes 2 --no-mdns" \
  >"$SERVE_OUT" 2>&1 &
SERVE_PID=$!

for _ in $(seq 1 50); do
  grep -q "^Session:" "$SERVE_OUT" 2>/dev/null && break
  sleep 0.2
done

PAIR_CODE="$(grep "^Pairing code:" "$SERVE_OUT" | awk '{print $NF}')"
SESSION_ID="$(grep "^Session:" "$SERVE_OUT" | awk '{print $NF}')"
PORT="$(grep "^Port:" "$SERVE_OUT" | awk '{print $NF}')"

if [[ -z $PAIR_CODE || -z $SESSION_ID || -z $PORT ]]; then
  echo "vm-pairing-smoke-test: FAILED — couldn't read pairing code/session/port from 'serve' within 10s. Output so far:" >&2
  cat "$SERVE_OUT" >&2
  exit 1
fi

echo "vm-pairing-smoke-test: dialing in (host=$CHILD_HOST port=$PORT sid=$SESSION_ID)..." >&2
PAIR_OUT="$(omarchy-kids-pairing pair \
  --host "$CHILD_HOST" --port "$PORT" --sid "$SESSION_ID" --code "$PAIR_CODE" \
  --key-out "$KEY_OUT" --yes)"
wait "$SERVE_PID" || true

echo "$PAIR_OUT" >&2

RESULT_JSON="$(grep "^PAIR_RESULT:" <<<"$PAIR_OUT" | sed 's/^PAIR_RESULT: //')"
if [[ -z $RESULT_JSON ]]; then
  echo "vm-pairing-smoke-test: FAILED — 'pair' produced no PAIR_RESULT line" >&2
  exit 1
fi

USERNAME="$(jq -r '.username' <<<"$RESULT_JSON")"
SSH_PORT="$(jq -r '.ssh_port' <<<"$RESULT_JSON")"

echo "vm-pairing-smoke-test: verifying the newly paired key can reach omarchy-kids-agent..." >&2
if ssh -i "$KEY_OUT" -p "$SSH_PORT" -o StrictHostKeyChecking=accept-new -o BatchMode=yes \
     "$USERNAME@$CHILD_HOST" status >/dev/null; then
  echo "vm-pairing-smoke-test: PASSED" >&2
else
  echo "vm-pairing-smoke-test: FAILED — pairing reported success but 'agent status' over the new key didn't" >&2
  exit 1
fi
