#!/usr/bin/env bash
# Plays a clip into the real synthbass app and records what it outputs, to
# check the live path (JACK, the window, patch loading) against an offline
# `render`, whose numbers it prints first. Nothing reaches the speakers: the
# app's ports are pinned off and linked by hand, and a temporary config
# folder (with a copy of your saved patches) leaves your settings alone.
#
# Usage: scripts/live-render.sh "<patch name>" <out.wav> [clip name or .wav]
set -euo pipefail
usage='usage: scripts/live-render.sh "<patch name>" <out.wav> [clip name or .wav]'
[ $# -ge 2 ] || { echo "$usage" >&2; exit 2; }
repo=$(cd "$(dirname "$0")/.." && pwd)
patch=$1; out=$2; clip=${3:-open-strings}
[[ $clip == */* || $clip == *.wav ]] || clip="$repo/clips/$clip.wav"

# Another synthbass would own the `synthbass:` port names, and its outputs go
# to the speakers.
if pw-link -i | grep -q '^synthbass:'; then
  echo "live-render: close the running Synthbass app first" >&2; exit 1
fi

cargo build --quiet --release -p synthbass --bins --manifest-path "$repo/Cargo.toml"
# The app falls back to the default patch for a name it doesn't know, and
# matches names exactly, so check the exact name against `render`'s table.
offline=$("$repo/target/release/render" --clip "$clip" --patch "$patch")
echo "$offline"
awk -v name="$patch" 'NR > 1 && substr($0, 1, length(name) + 1) == name " " { found = 1 }
  END { exit !found }' <<<"$offline" \
  || { echo "live-render: no patch named exactly \"$patch\"" >&2; exit 1; }

cfg=$(mktemp -d); mkdir -p "$cfg/synthbass"
saved=${XDG_CONFIG_HOME:-$HOME/.config}/synthbass/patches
[ -d "$saved" ] && cp -r "$saved" "$cfg/synthbass/"
toml_name=${patch//\\/\\\\}; toml_name=${toml_name//\"/\\\"}
printf 'last_patch = "%s"\n\n[ports]\ninput = ""\noutput_1 = ""\noutput_2 = ""\n' "$toml_name" \
  > "$cfg/synthbass/settings.toml"
app=; rec=; play=
cleanup() { kill $play $rec $app 2>/dev/null || true; rm -rf "$cfg"; }
trap cleanup EXIT

# Port names vary (pw-play names a mono port output_FL), so look them up.
first_port() {
  for _ in $(seq 50); do
    p=$(pw-link "$1" | grep "^$2:" | head -1 || true)
    [ -n "$p" ] && { echo "$p"; return; }
    sleep 0.1
  done
  echo "live-render: no $2 port appeared" >&2; exit 1
}

XDG_CONFIG_HOME=$cfg "$repo/target/release/synthbass" >/dev/null 2>&1 & app=$!
first_port -i synthbass >/dev/null
pw-record -P '{ node.name=live-render-rec node.autoconnect=false }' \
  --channels 1 --rate 48000 --format f32 "$out" & rec=$!
pw-link synthbass:out_1 "$(first_port -i live-render-rec)"
pw-play -P '{ node.name=live-render-play node.autoconnect=false }' --channels 1 "$clip" & play=$!
pw-link "$(first_port -o live-render-play)" synthbass:in
timeout 120 tail --pid=$play -f /dev/null || true
sleep 0.5
# Let the recorder finish its file before saying it is written.
kill -INT $rec; wait $rec 2>/dev/null || true; rec=
echo "live-render: wrote $out"
