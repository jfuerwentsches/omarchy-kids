# Parent/admin account creation and child-account topology enforcement
# (issue #16). Sourced by omarchy-kids-bootstrap, not run standalone.
#
# Account topology (see docs/agent-protocol.md "Account topology"):
# agentd/agent/omarchy-kids-run all run as the CHILD's own account, which
# must therefore not be a polkit admin. A separate account carries admin
# rights instead, so the PIN/polkit override path (agent issue #9) has
# something meaningful to authenticate against. Naming convention: follows
# every other omarchy-kids-* binary/account name in this repo.
# shellcheck disable=SC2034  # consumed by omarchy-kids-bootstrap, which sources this file
OMARCHY_KIDS_ADMIN_USER_DEFAULT="omarchy-kids-parent"

# create_parent_admin_account <child_user> <admin_user> <admin_password>
#
# Creates (or updates) the admin account and makes sure the child's own,
# Omarchy-created account is not in `wheel` — Omarchy's own first-boot
# flow makes the primary user a sudoer by default, which is exactly the
# membership this function has to undo.
create_parent_admin_account() {
  local child_user="$1" admin_user="$2" admin_password="$3"

  if ! id "$admin_user" >/dev/null 2>&1; then
    useradd -m -G wheel -s /bin/bash "$admin_user"
  elif ! id -nG "$admin_user" | tr ' ' '\n' | grep -qx wheel; then
    usermod -aG wheel "$admin_user"
  fi
  echo "$admin_user:$admin_password" | chpasswd

  if id -nG "$child_user" | tr ' ' '\n' | grep -qx wheel; then
    gpasswd -d "$child_user" wheel >/dev/null
    echo "omarchy-kids-bootstrap: removed '$child_user' from wheel (child accounts must not be polkit admins)" >&2
  fi
}

# grant_getty_lockdown_sudo <child_user>
#
# omarchy-kids-set-tier (tiers/omarchy-kids-set-tier) masks the unused VTs
# via `sudo systemctl mask --now getty@tty2..6.service` — the actual
# access-control boundary for the kiosk tiers (see root CLAUDE.md). That
# call runs as whichever account invokes the tier switch, which after this
# script's wheel fix-up above is the child account with NO sudo rights at
# all, and even before that fix-up a non-interactive sudo call has no tty to
# prompt on. Either way the mask step would silently no-op (the script
# swallows its result with `|| true`), quietly disabling VT lockdown.
#
# Fixes it the same way Omarchy's own first-boot setup already does for
# other commands (see its create_user(): "omarchy ships narrow %wheel
# NOPASSWD rules for specific commands") — a narrow NOPASSWD grant for
# exactly this command, not a return to broad admin rights.
#
# Filename sorts last on purpose (verified against a real conflict in the
# dev VM: a pre-existing `devchild ALL=(ALL) ALL` in /etc/sudoers.d/04_devchild
# alphabetically outranked an earlier "00-" filename here and silently
# shadowed this NOPASSWD grant — sudoers uses last-match-wins per exact
# command line, regardless of which rule is narrower). A "zz-" prefix keeps
# this grant authoritative no matter what other rules exist for the account.
grant_getty_lockdown_sudo() {
  local child_user="$1"
  local sudoers_file=/etc/sudoers.d/zz-omarchy-kids-getty-lockdown
  local getty_units="getty@tty2.service getty@tty3.service getty@tty4.service getty@tty5.service getty@tty6.service"
  local tmp_file

  tmp_file="$(mktemp)"
  {
    echo "$child_user ALL=(root) NOPASSWD: /usr/bin/systemctl mask --now $getty_units"
    echo "$child_user ALL=(root) NOPASSWD: /usr/bin/systemctl unmask $getty_units"
  } > "$tmp_file"

  visudo -c -f "$tmp_file" >/dev/null
  install -m 440 "$tmp_file" "$sudoers_file"
  rm -f "$tmp_file"
}

# generate_admin_password
#
# Phase 1 kept a non-interactive fallback for direct bootstrap invocations
# in the dev VM. The real first-boot form now prompts for this password
# interactively, but the direct bootstrap path still accepts
# --admin-password and can generate one when no caller provides it.
generate_admin_password() {
  openssl rand -base64 18
}
