#!/usr/bin/env bash
# Renders the PNG variants of ticknode.svg. Needs rsvg-convert (librsvg).
#
# rsvg-convert ignores vector-effect="non-scaling-stroke", so every size gets its own copy of the
# mark with the outline width set in viewBox units: a 2-unit-per-pixel stroke clipped to the inside
# of each shape leaves exactly 1 px. PNGs cannot follow a theme, so they use the light-theme outline.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
src="$root/assets/logo/ticknode.svg"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT

render() { # <size> <output>
    local width; width="$(awk -v s="$1" 'BEGIN { printf "%.4f", 128 / s }')"
    sed -e '/@media/d' -e 's/ vector-effect="non-scaling-stroke"//' \
        -e "s/stroke-width=\"2\"/stroke-width=\"$width\"/" "$src" > "$tmp/mark.svg"
    rsvg-convert -w "$1" -h "$1" "$tmp/mark.svg" -o "$2"
    echo "$2"
}

render 128 "$root/ha-addon/ticknode/icon.png"
render 256 "$root/ha-addon/ticknode/logo.png"
render 32 "$root/ticknode-editor/assets/favicon-32.png"
render 180 "$root/ticknode-editor/assets/apple-touch-icon.png"
for s in 16 32 48 64 128 256 512; do render "$s" "$root/assets/logo/ticknode-$s.png"; done
cp "$src" "$root/ticknode-editor/assets/favicon.svg"
