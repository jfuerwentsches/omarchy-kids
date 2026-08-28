# Agent SSH keypair generation + command=-restricted authorized_keys entry
# (issue #17). Sourced by omarchy-kids-bootstrap, not run standalone.

# install_agent_ssh_key <child_user>
#
# Generates (idempotently) an ed25519 keypair for the child account and
# installs the public half into that account's authorized_keys, restricted
# via command= to omarchy-kids-agent — this is the actual security boundary
# the SSH-based parental-control design rests on (see
# docs/agent-protocol.md), never a full shell.
#
# The private half is also copied to a root-only location: it never needs
# to be readable by the child account, only by whatever later transfers it
# to the parent's Control Center during pairing (see the Pairing track,
# issue #21 for the transfer format/security posture).
install_agent_ssh_key() {
  local child_user="$1"
  local child_home ssh_dir key_path authorized_keys pubkey restricted_entry

  child_home="$(getent passwd "$child_user" | cut -d: -f6)"
  ssh_dir="$child_home/.ssh"
  key_path="$ssh_dir/omarchy-kids-agent_ed25519"
  authorized_keys="$ssh_dir/authorized_keys"

  install -d -m 700 -o "$child_user" -g "$child_user" "$ssh_dir"

  if [[ ! -f $key_path ]]; then
    sudo -u "$child_user" ssh-keygen -q -t ed25519 -N "" \
      -C "omarchy-kids-agent@$(hostname)" -f "$key_path"
  fi

  pubkey="$(<"$key_path.pub")"
  restricted_entry="command=\"/usr/bin/omarchy-kids-agent\",no-agent-forwarding,no-X11-forwarding,no-port-forwarding,restrict $pubkey"

  touch "$authorized_keys"
  if ! grep -qF "$pubkey" "$authorized_keys"; then
    echo "$restricted_entry" >> "$authorized_keys"
  fi
  chmod 600 "$authorized_keys"
  chown "$child_user:$child_user" "$authorized_keys"

  install -d -m 700 /var/lib/omarchy-kids/pairing
  install -m 600 "$key_path" /var/lib/omarchy-kids/pairing/agent_ed25519
}
