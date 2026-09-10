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

# THE LIBRARIES THAT ARE OPENED BY NAME AT RUN TIME, and are therefore invisible to `ldd`.
#
# Reported behaviour: the AppImage check of appimage.github.io ran the package under firejail and it
# panicked with "Library libxkbcommon-x11.so could not be loaded."
#
# linuxdeploy gathers what the ELF header asks for. The window stack does not ask: `xkbcommon-dl`,
# `x11-dl` and `wayland-sys` open their libraries with `dlopen` by plain name, so none of them appears in
# `ldd` and none of them was packed. Measured on the built binary: `ldd` names libX11, libxcb, libXau and
# libXdmcp - and not one of libxkbcommon, libxkbcommon-x11, libXcursor, libXrandr, libXi or libwayland-*.
# On a machine that happens to have them installed the package starts; on a bare one it dies on the first.
#
# WHAT IS DELIBERATELY NOT HERE: libGL, libEGL, libvulkan and libwayland-client. Those are the system's
# side of the graphics stack and must come from the system - a copy carried inside the package talks to a
# compositor or a driver it was not built against.
#
# Reported behaviour: the package built with libwayland-client inside died on a Wayland desktop with
# "WGPU error: Failed to create surface for any enabled backend: {}". Measured on that very package: with
# all three wayland libraries inside it fails; with libwayland-client.so.0 alone deleted the window opens.
# Its two neighbours, libwayland-cursor and libwayland-egl, turned out to be dead weight in both
# directions - a machine running a compositor already has them, and a machine without one never reaches
# the Wayland path at all - so they are gone too.
DLOPENED=(
    libxkbcommon.so.0
    libxkbcommon-x11.so.0
    libXcursor.so.1
    libXrandr.so.2
    libXi.so.6
)

# THE LIBRARIES THAT MUST NEVER BE CARRIED, whatever the reason looks like at the time.
#
# This is the graphics and compositor part of the AppImage excludelist, the list the format keeps because
# these particular libraries have to be the host's own. `--library` FORCES a file in and walks straight
# past that list, which is how libwayland-client got inside: linuxdeploy would have refused it on its own.
# So the request is checked against this before it is made.
#
# The whole wayland group is here, not only the one that broke: without libwayland-client there is no
# conversation with a compositor for the other two to take part in, and a machine that has a compositor
# has all three already.
NEVER_CARRY=(
    libwayland-client.so.0
    libwayland-cursor.so.0
    libwayland-egl.so.1
    libGL.so.1
    libEGL.so.1
    libGLX.so.0
    libgbm.so.1
    libdrm.so.2
    libvulkan.so.1
    libX11.so.6
    libxcb.so.1
)
for lib in "${DLOPENED[@]}"; do
    for banned in "${NEVER_CARRY[@]}"; do
        if [ "$lib" = "$banned" ]; then
            echo "!!! $lib is asked for by --library, and that library has to be the host's own"
            echo "!!! carrying it inside the package breaks the machines that do have it - see NEVER_CARRY above"
            exit 1
        fi
    done
done

LIB_ARGS=()
missing=()
for lib in "${DLOPENED[@]}"; do
    # the loader's own answer, not a guessed path: `ldconfig -p` says where this machine keeps it
    path=$(ldconfig -p | awk -v n="$lib" '$1 == n && $0 ~ /x86-64/ { print $NF; exit }')
    if [ -z "$path" ] || [ ! -e "$path" ]; then
        missing+=("$lib")
        continue
    fi
    LIB_ARGS+=(--library "$path")
done
if [ ${#missing[@]} -ne 0 ]; then
    echo "!!! these libraries are opened at run time and are not in the builder image: ${missing[*]}"
    echo "!!! add their packages to packaging/linux/Dockerfile - a package built without them dies on a bare machine"
    exit 1
fi

echo ">>> linuxdeploy: the icons, the .so files from ldd (/opt/occt/lib among them), plus ${#LIB_ARGS[@]} halves of the dlopened ones"
linuxdeploy \
    --appdir "$APPDIR" \
    --executable "$APPDIR/usr/bin/qymcad" \
    --desktop-file packaging/linux/qymcad.desktop \
    "${ICON_ARGS[@]}" \
    "${LIB_ARGS[@]}" \
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

# --- THE PACKAGE IS OPENED AND LOOKED INSIDE, before anybody downloads it ---
#
# The list above is only a request; whether linuxdeploy honoured it is a different question, and the
# difference shows up on somebody else's machine. So the finished package is unpacked and every library
# that gets opened by name is looked for by hand. This is the check that would have caught the reported
# failure at build time instead of in a stranger's terminal.
echo ">>> checking what actually ended up inside the package"
CHECK=/tmp/appimage-check
rm -rf "$CHECK"
mkdir -p "$CHECK"
( cd "$CHECK" && "$OUT" --appimage-extract >/dev/null )
absent=()
for lib in "${DLOPENED[@]}"; do
    find "$CHECK/squashfs-root" -name "$lib" -print -quit | grep -q . || absent+=("$lib")
done
if [ ${#absent[@]} -ne 0 ]; then
    echo "!!! the package is missing libraries it opens by name at run time: ${absent[*]}"
    echo "!!! it would start only on a machine that happens to have them - that is the bug this check exists for"
    exit 1
fi

# ...and the other direction, which is the one that actually shipped broken. A library can arrive without
# being asked for - dragged in as somebody else's dependency - so the finished package is searched for the
# ones that must be the host's own.
carried=()
for lib in "${NEVER_CARRY[@]}"; do
    find "$CHECK/squashfs-root" -name "$lib" -print -quit | grep -q . && carried+=("$lib")
done
if [ ${#carried[@]} -ne 0 ]; then
    echo "!!! the package carries libraries that have to be the host's own: ${carried[*]}"
    echo "!!! this is what makes a package start here and die on somebody else's desktop"
    exit 1
fi
rm -rf "$CHECK"
echo ">>> DONE: $OUT"
