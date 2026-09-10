#!/usr/bin/env bash
# Round the corners of a window capture and lay a shadow under it, for the store listing.
#
# WHY THIS EXISTS. Flathub asks for the window with its native decoration - the title bar, the rounded
# corners and the shadow - and a tiling compositor draws none of the last two. So the shot is taken in a
# nested session that DOES draw a title bar, and the rounding and the shadow are put on here. Nothing is
# invented: the title bar in the picture is a real one, and these two are what the compositor would have
# drawn around it.
#
# WHY A SCRIPT AND NOT A HAND IN AN EDITOR. Five pictures have to look like a set. Done by hand they come
# out with five different shadows, and the difference is visible the moment they stand side by side in a
# store. Done again a year later, after the interface has moved, they come out the same as today.
#
#   packaging/flatpak/frame-screenshots.sh shot-1.png shot-2.png ...
#
# The finished files land in docs/screenshots/ under the same names.
set -euo pipefail

command -v magick >/dev/null || { echo "!!! ImageMagick (magick) is not installed" >&2; exit 1; }
[ $# -gt 0 ] || { echo "usage: $0 <window-capture.png> [...]" >&2; exit 1; }

HERE=$(cd "$(dirname "$0")" && pwd)
OUT="$HERE/../../docs/screenshots"
mkdir -p "$OUT"

for src in "$@"; do
    [ -f "$src" ] || { echo "!!! no such file: $src" >&2; exit 1; }
    name=$(basename "$src")

    # EVERYTHING IS MEASURED FROM THE WIDTH, so a 1000-wide shot and a 2000-wide one for HiDPI come out
    # looking the same rather than one of them with a hairline rounding and a smudge for a shadow.
    w=$(magick identify -format '%w' "$src")
    h=$(magick identify -format '%h' "$src")
    radius=$(( w / 100 )); [ "$radius" -lt 8 ] && radius=8
    blur=$(( w / 80 ));    [ "$blur" -lt 8 ] && blur=8
    drop=$(( w / 160 ));   [ "$drop" -lt 4 ] && drop=4

    # `-compose Over` after the mask is NOT decoration: the operator set for the rounding stays in force,
    # and the merge with the shadow then runs under DstIn too - which throws the picture away and leaves a
    # blank sheet. Measured: without this line the output is an empty image of the right size.
    magick "$src" \
        \( +clone -alpha transparent -background none -fill white \
           -draw "roundrectangle 0,0 $((w-1)),$((h-1)) $radius,$radius" \) \
        -compose DstIn -composite \
        -compose Over \
        \( +clone -background black -shadow "55x${blur}+0+${drop}" \) \
        -reverse -background none -layers merge +repage \
        "$OUT/$name"

    echo ">>> $name: ${w}x${h} -> $(magick identify -format '%wx%h' "$OUT/$name")  (rounding ${radius}, shadow ${blur}/${drop})"
done

echo ">>> the finished pictures are in docs/screenshots/"
