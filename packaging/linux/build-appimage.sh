#!/usr/bin/env bash
# Build QymCAD into an AppImage. Runs INSIDE the image from packaging/linux/Dockerfile (see justfile ->
# pkg-linux). The sources are mounted at /src and the result is left in /dist.
set -euo pipefail

cd /src

# a target directory of its own, so that a mounted host target (another toolchain, another distribution) is
# never mixed into this build
export CARGO_TARGET_DIR=/tmp/target
export OCCT_INCLUDE_DIR=${OCCT_INCLUDE_DIR:-/opt/occt/include/opencascade}
export OCCT_LIB_DIR=${OCCT_LIB_DIR:-/opt/occt/lib}
export LD_LIBRARY_PATH=${OCCT_LIB_DIR}:${LD_LIBRARY_PATH:-}
export APPIMAGE_EXTRACT_AND_RUN=1

echo ">>> cargo build --release (qymcad)"
cargo build --release --locked --bin qymcad

BIN=$CARGO_TARGET_DIR/release/qymcad
[ -x "$BIN" ] || { echo "!!! the binary did not build: $BIN"; exit 1; }

# --- assemble the AppDir ---
APPDIR=/tmp/AppDir
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
cp "$BIN" "$APPDIR/usr/bin/qymcad"

# The licence and the third-party notices TRAVEL WITH THE BINARY, in the place a Linux program keeps them.
# AGPL-3.0 asks for the licence text to accompany the program, and LGPL-2.1 (OCCT) for the notice.
mkdir -p "$APPDIR/usr/share/doc/qymcad"
cp LICENSE "$APPDIR/usr/share/doc/qymcad/LICENSE"
cp THIRD-PARTY-NOTICES.md "$APPDIR/usr/share/doc/qymcad/"

# the icons: the real logo as a ready set of PNGs in several sizes (assets/icons/linux, made from
# assets/logo.png). linuxdeploy files each one under usr/share/icons/hicolor/<WxH>/apps/qymcad.png by its
# SIZE and makes the .DirIcon; --icon-filename=qymcad brings the names in line with Icon=qymcad in the
# .desktop file.
ICON_ARGS=()
for s in 16 32 48 64 128 256 512; do
    ICON_ARGS+=(--icon-file "assets/icons/linux/${s}x${s}.png")
done

echo ">>> linuxdeploy: the icons, plus the dependent .so files (/opt/occt/lib among them) gathered from ldd"
linuxdeploy \
    --appdir "$APPDIR" \
    --executable "$APPDIR/usr/bin/qymcad" \
    --desktop-file packaging/linux/qymcad.desktop \
    "${ICON_ARGS[@]}" \
    --icon-filename qymcad \
    --output appimage

# КАК ЗОВЁТСЯ ПАКЕТ.
#
# Тег называет себя сам: собрали v0.1.0-dev.20260826 — файл так и зовётся. Всё остальное зовётся
# номером из манифеста плюс коммит. Без этого две сборки, разошедшиеся на три дня, получали бы одно имя,
# и три файла в папке загрузок опять стали бы тремя неизвестными.
#
# QYMCAD_VERSION ставит процесс сборки, когда собирает тег; ведущее `v` отбрасывается.
package_name() {
    if [ -n "${QYMCAD_VERSION:-}" ]; then
        printf "qymcad-%s" "${QYMCAD_VERSION#v}"
        return
    fi
    ver=$(grep -m1 '^version' Cargo.toml | sed 's/[^0-9.]//g')
    # Репозиторий, примонтированный в контейнер, принадлежит другому пользователю, и git отказывается
    # его читать, пока ему не сказать, что каталог свой. Без этой строки суффикс молча выходил бы пустым.
    git config --global --add safe.directory "$PWD" 2>/dev/null || true
    sha=$(git rev-parse --short=9 HEAD 2>/dev/null || true)
    if [ -n "$sha" ]; then
        printf "qymcad-%s-dev.%s" "${ver:-0.0.0}" "$sha"
    else
        printf "qymcad-%s" "${ver:-0.0.0}"
    fi
}

mkdir -p /dist
OUT="/dist/$(package_name)-x86_64.AppImage"
mv qymcad*.AppImage "$OUT" 2>/dev/null || mv ./*.AppImage "$OUT"
chmod +x "$OUT"
echo ">>> DONE: $OUT"
