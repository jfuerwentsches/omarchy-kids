# Branding + initial tier switch (issue #19). Sourced by
# omarchy-kids-bootstrap, not run standalone.

# xml_escape <text>
xml_escape() {
  local s="$1"
  s="${s//&/&amp;}"
  s="${s//</&lt;}"
  s="${s//>/&gt;}"
  s="${s//\"/&quot;}"
  printf '%s' "$s"
}

# render_name_banner_via_transcode <target_path> <child_name>
#
# Fallback for when `omarchy ascii` isn't available (see render_name_banner
# below). The underlying file is just a block-art text file rendered by the
# screensaver (see logo.txt), producible from an image via
# `omarchy-transcode-ascii`. This reproduces `omarchy branding screensaver
# image`'s conversion non-interactively: render the child's name as an SVG
# banner, rasterize it, then transcode it. Verified by hand in the dev VM
# (2026-08-29) — also handles umlauts/non-Latin names, which the FIGlet font
# `omarchy ascii` uses reportedly does not (per the manual: "letters and
# spaces only").
render_name_banner_via_transcode() {
  local target="$1" child_name="$2"
  local svg_path png_path escaped_name
  svg_path="$(mktemp --suffix=.svg)"
  png_path="$(mktemp --suffix=.png)"
  escaped_name="$(xml_escape "$child_name")"

  cat > "$svg_path" <<EOF
<svg xmlns="http://www.w3.org/2000/svg" width="800" height="200">
  <rect width="800" height="200" fill="black"/>
  <text x="400" y="130" font-family="sans-serif" font-weight="bold" font-size="90" fill="white" text-anchor="middle">$escaped_name</text>
</svg>
EOF

  rsvg-convert "$svg_path" -o "$png_path"
  omarchy-transcode-ascii "$png_path" "$target" --mode block --invert

  rm -f "$svg_path" "$png_path"
}

# render_name_banner <child_user> <child_name>
#
# The vault note's plan ("omarchy ascii \"<name>\" -> screensaver.txt")
# matches the public manual (omarchy.org/manual/branding) but NOT the
# Omarchy 4.0.1 CLI actually installed in the dev VM: `omarchy ascii`
# exits 127 ("Unknown Omarchy command"), no such binary or FIGlet font
# exists anywhere under /usr/share/omarchy, and the package is up to date
# (verified 2026-08-29, `pacman -Qu` empty). Apparent doc/package mismatch
# upstream, not something to chase further here — this tries the documented
# command first (so it starts working for free once/if it ships) and falls
# back to a manual SVG-render + transcode otherwise, which is verified
# working today.
render_name_banner() {
  local child_user="$1" child_name="$2"
  local home branding_dir target ascii_output

  home="$(getent passwd "$child_user" | cut -d: -f6)"
  branding_dir="$home/.config/omarchy/branding"
  target="$branding_dir/screensaver.txt"
  install -d -o "$child_user" -g "$child_user" "$branding_dir"

  if ascii_output="$(omarchy ascii "$child_name" 2>/dev/null)" && [[ -n $ascii_output ]]; then
    printf '%s\n' "$ascii_output" > "$target"
  else
    render_name_banner_via_transcode "$target" "$child_name"
  fi

  chown "$child_user:$child_user" "$target"
  chmod 644 "$target"
}

# apply_branding_and_tier <child_user> <child_name> <tier> [theme]
apply_branding_and_tier() {
  local child_user="$1" child_name="$2" tier="$3" theme="${4:-}"

  render_name_banner "$child_user" "$child_name"

  # Not yet packaged (see tiers/README.md "Status") — assumes
  # omarchy-kids-set-tier is on the child account's PATH, same assumption
  # the rest of the dev workflow already makes (docs/dev-vm-setup.md).
  if [[ -n $theme ]]; then
    sudo -u "$child_user" omarchy-kids-set-tier "$tier" "$theme"
  else
    sudo -u "$child_user" omarchy-kids-set-tier "$tier"
  fi
}
