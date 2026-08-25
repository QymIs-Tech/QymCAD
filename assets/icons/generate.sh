#!/usr/bin/env bash
# Generating the icon set from assets/logo.png for Windows/.ico, Linux/AppImage and macOS/.icns.
# Needs ImageMagick (magick) and python3. Run from the root of the repository: assets/icons/generate.sh
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

SRC=assets/logo.png
OUT=assets/icons
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$OUT/windows" "$OUT/macos" "$OUT/linux"

# master square PNGs (Lanczos downscale, transparent background)
for s in 16 24 32 48 64 128 256 512 1024; do
  magick "$SRC" -resize ${s}x${s} -background none -gravity center -extent ${s}x${s} -strip "$WORK/$s.png"
done

# Linux (hicolor / AppImage)
for s in 16 24 32 48 64 128 256 512; do cp "$WORK/$s.png" "$OUT/linux/${s}x${s}.png"; done

# Windows multi-res .ico
magick "$WORK/16.png" "$WORK/24.png" "$WORK/32.png" "$WORK/48.png" \
       "$WORK/64.png" "$WORK/128.png" "$WORK/256.png" "$OUT/windows/qymcad.ico"

# macOS .icns (ImageMagick has no ICNS encoder, so the container is assembled by hand out of PNG chunks)
python3 - "$WORK" "$OUT/macos/qymcad.icns" <<'PY'
import struct, sys, os
work, out = sys.argv[1], sys.argv[2]
chunks = [("icp4",16),("icp5",32),("icp6",64),("ic07",128),("ic08",256),
          ("ic09",512),("ic10",1024),("ic11",32),("ic12",64),("ic13",256),("ic14",512)]
body = b""
for ostype, size in chunks:
    data = open(os.path.join(work, f"{size}.png"), "rb").read()
    body += ostype.encode("ascii") + struct.pack(">I", len(data)+8) + data
open(out, "wb").write(b"icns" + struct.pack(">I", len(body)+8) + body)
PY

echo "done: $OUT/{windows/qymcad.ico, linux/*.png, macos/qymcad.icns}"
