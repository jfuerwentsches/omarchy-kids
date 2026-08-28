# SSH prep for the agent's pairing step (issue #17, revised 2026-08-29).
# Sourced by omarchy-kids-bootstrap, not run standalone.

# prepare_agent_ssh_dir <child_user>
#
# Originally this generated a keypair on the child and stashed the private
# half for pairing to transfer (issue #21's threat-model review changed
# that — see the vault note's "Kernentscheidung", 29.08.2026, and
# agent/pairing/). The keypair now belongs to the Control Center: it
# generates its own locally, and only the PUBLIC half ever crosses the
# network — no private key in transit, encrypted or not. There is
# therefore nothing to generate or stash here anymore; this just makes
# sure ~/.ssh exists with the right permissions and an empty
# authorized_keys, ready for `omarchy-kids-pairing serve` (run later, as
# this same child account, during the wizard's actual pairing step) to
# append the `command=`-restricted entry once a Control Center pairs.
prepare_agent_ssh_dir() {
  local child_user="$1"
  local child_home ssh_dir authorized_keys

  child_home="$(getent passwd "$child_user" | cut -d: -f6)"
  ssh_dir="$child_home/.ssh"
  authorized_keys="$ssh_dir/authorized_keys"

  install -d -m 700 -o "$child_user" -g "$child_user" "$ssh_dir"
  [[ -f $authorized_keys ]] || install -m 600 -o "$child_user" -g "$child_user" /dev/null "$authorized_keys"
}
