#!/usr/bin/env bash
# Assemble QymCAD into a macOS .app and pack it into a disk image. Runs after `cargo build --release` on the machine it is
# packing for - Apple Silicon or Intel - with OCCT installed at $OCCT_ROOT (built from source - see
# .github/workflows/release.yml).
#
# WHAT MAKES A MAC BUNDLE DIFFERENT. On Linux `linuxdeploy` gathers the shared libraries and rewrites
# their paths; on Windows the DLLs simply sit beside the executable and are found there. macOS does
# neither: a dylib carries the path it was BUILT at, baked into whatever loads it, so a copied library is
# looked for where it used to live and the program dies on start. `install_name_tool` rewrites those
# paths to `@rpath`, and `@rpath` is pointed at the bundle's own Frameworks directory.
set -euo pipefail

BIN=target/release/qymcad
[ -x "$BIN" ] || { echo "!!! no $BIN - run cargo build --release first"; exit 1; }
OCCT_ROOT=${OCCT_ROOT:?set OCCT_ROOT to the OCCT installation}

# THE PROCESSOR, in the package name and in the notes. Two packages go up side by side, and a person
# holding an Intel mac must be able to tell theirs from the file name alone. CI names it (MACOS_ARCH);
# by hand it is the machine the script runs on, which is the machine cargo just built for.
ARCH=${MACOS_ARCH:-$(uname -m)}
case "$ARCH" in
    arm64)  MACHINE="Apple Silicon" ;;
    x86_64) MACHINE="Intel" ;;
    *) echo "!!! unknown macOS architecture: $ARCH (expected arm64 or x86_64)"; exit 1 ;;
esac

# THE NAME. A tag names the package itself; anything else carries the commit, so two builds three days
# apart cannot share a file name and a report can always be traced to one of them.
if [ -n "${QYMCAD_VERSION:-}" ]; then
    NAME="qymcad-${QYMCAD_VERSION#v}"
else
    VER=$(grep -m1 '^version' Cargo.toml | sed 's/[^0-9.]//g')
    git config --global --add safe.directory "$PWD" 2>/dev/null || true
    SHA=$(git rev-parse --short=9 HEAD 2>/dev/null || true)
    NAME="qymcad-${VER:-0.0.0}${SHA:+-dev.$SHA}"
fi

APP=dist/QymCAD.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"
cp "$BIN" "$APP/Contents/MacOS/qymcad"
cp assets/icons/macos/qymcad.icns "$APP/Contents/Resources/"

# The licence and the notices travel with the binary: AGPL asks for the licence text to accompany the
# program, LGPL-2.1 (OCCT) for the notice.
cp LICENSE "$APP/Contents/Resources/LICENSE.txt"
cp THIRD-PARTY-NOTICES.md "$APP/Contents/Resources/"

VER_PLIST=$(grep -m1 '^version' Cargo.toml | sed 's/[^0-9.]//g')
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>QymCAD</string>
    <key>CFBundleDisplayName</key><string>QymCAD</string>
    <key>CFBundleIdentifier</key><string>tech.qymis.cad</string>
    <key>CFBundleExecutable</key><string>qymcad</string>
    <key>CFBundleIconFile</key><string>qymcad</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${VER_PLIST:-0.0.0}</string>
    <key>CFBundleVersion</key><string>${VER_PLIST:-0.0.0}</string>
    <key>LSMinimumSystemVersion</key><string>12.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# --- the kernel libraries, with their paths rewritten to live inside the bundle ---
#
# THE LINKS ARE KEPT AS LINKS, and that is the difference between an 80 MB download and a third of it.
# OCCT installs every module under three names - `libTKernel.dylib` -> `libTKernel.7.9.dylib` ->
# `libTKernel.7.9.3.dylib`, one file and two links to it. A plain `cp` follows each link and writes THREE
# full copies: 144 files for 48 modules, measured on the runner. Only one of the three is ever loaded -
# the name written into the dependencies - so the other two were pure weight.
#
# `-RP` copies links as links on both toolchains (POSIX: with -R and no -H/-L, a link is copied, not
# followed), here and again into the disk image's folder below.
echo ">>> gathering the kernel libraries"
cp -RP "$OCCT_ROOT"/lib/*.dylib "$APP/Contents/Frameworks/"
n=$(find "$APP/Contents/Frameworks" -type f -name '*.dylib' | wc -l | tr -d ' ')
links=$(find "$APP/Contents/Frameworks" -type l -name '*.dylib' | wc -l | tr -d ' ')
[ "$n" -gt 0 ] || { echo "!!! no dylibs found under $OCCT_ROOT/lib"; exit 1; }
echo ">>> libraries: $n, links to them: $links"

# The executable looks for them beside itself, one directory up and into Frameworks. This one is NOT
# allowed to fail quietly: without the rpath every library below is unreachable and the program does not
# start at all, so a swallowed error here would ship as a green build.
install_name_tool -add_rpath "@executable_path/../Frameworks" "$APP/Contents/MacOS/qymcad"

# What a file depends on, one path per line. The header line - the file's own name - is dropped.
#
# NOT A PIPELINE ENDING IN `grep`. This script runs under `set -o pipefail`, and `grep` returns 1 when it
# matches nothing: a bundle whose paths are ALREADY `@rpath` (which is what OCCT built by CMake produces)
# made the whole script end right here, silently, six minutes into a build.
deps_of() {
    otool -L "$@" | awk 'NR>1 {print $1}'
}

# Each library must both ANNOUNCE itself by @rpath and LOOK FOR its neighbours the same way: the OCCT
# modules depend on one another, and a path left pointing at the build machine is a start-up failure on
# anybody else's.
rewritten=0
rewrite_deps() {
    local file=$1 dep
    while IFS= read -r dep; do
        case "$dep" in
            "$OCCT_ROOT"/*)
                install_name_tool -change "$dep" "@rpath/$(basename "$dep")" "$file" 2>/dev/null || true
                rewritten=$((rewritten + 1))
                ;;
        esac
    done <<< "$(deps_of "$file")"
}

for lib in "$APP"/Contents/Frameworks/*.dylib; do
    # A link is the same file under another name: writing through it would set the id of the real file to
    # whichever name came last, and the loop would undo its own work two times out of three.
    if [ -L "$lib" ]; then
        continue
    fi
    install_name_tool -id "@rpath/$(basename "$lib")" "$lib" 2>/dev/null || true
    rewrite_deps "$lib"
done
rewrite_deps "$APP/Contents/MacOS/qymcad"
echo ">>> paths rewritten to @rpath: $rewritten"

# NOT A PATH LEFT POINTING HOME. A single dependency still naming the build machine means the program
# starts nowhere but here, and it says so only on somebody else's computer.
#
# THE CHECK IS NOT `grep -q`. Under `set -o pipefail` an early-exiting `grep -q` kills `otool` with a
# broken pipe, the pipeline reports that failure, and the `if` reads it as "nothing found" - the sentinel
# passed a bundle in which every path still named the build machine. Read it all, then look.
left=$(otool -L "$APP/Contents/MacOS/qymcad" "$APP"/Contents/Frameworks/*.dylib | grep "$OCCT_ROOT" || true)
if [ -n "$left" ]; then
    echo "!!! a library still points at the build machine:"
    printf '%s\n' "$left" | head -5 || true
    exit 1
fi

# --- the signature, made LAST: any change to a signed file breaks it ---
#
# THE BUNDLE IS SEALED AS A WHOLE. The linker signs the executable by itself, and that signature says the
# file belongs to a bundle whose resources it vouches for - yet the bundle around it was never signed.
# `codesign --verify` on such an .app answers "code has no resources but signature indicates they must be
# present", measured on the CI package, and macOS reads a broken signature as a damaged download.
#
# THE CERTIFICATE. `MACOS_SIGN_P12` is the .p12 made by `make-signing-cert.sh`, `MACOS_SIGN_P12_PASSWORD_FILE`
# the file holding its password; without them the signature is ad hoc, which still seals the bundle.
#
# `rcodesign`, NOT `codesign`, FOR THE CERTIFICATE. `codesign` takes an identity only from a keychain and only
# when the system trusts it: a self-signed one is listed as CSSMERR_TP_NOT_TRUSTED and refused with "no
# identity found", measured. Trusting it would mean declaring a home-made certificate trusted on the
# signing machine. `rcodesign` reads the .p12 itself and needs neither. It signs the nested libraries
# before the bundle, as sealing requires, and writes the requirement `identifier "tech.qymis.cad" and
# certificate root = H"..."` - the same for every version signed with that certificate, which is what
# lets the system tell a new release from a stranger.
#
# No hardened runtime: under it the loader takes a library only from the same Apple team, and a self-signed
# certificate has no team, so not one OCCT module would load. No timestamp: Apple's timestamp service is
# for Apple-issued certificates.
#
# WRITABLE FIRST. OCCT as Homebrew installs it is read-only (0444), `cp` keeps that, and `rcodesign` stops
# on the first library with "Permission denied".
chmod -R u+w "$APP"
if [ -n "${MACOS_SIGN_P12:-}" ]; then
    echo ">>> signing with the certificate in $MACOS_SIGN_P12"
    rcodesign sign --timestamp-url none \
        --p12-file "$MACOS_SIGN_P12" --p12-password-file "${MACOS_SIGN_P12_PASSWORD_FILE:?the .p12 needs its password file}" \
        "$APP"
else
    echo ">>> no certificate given (MACOS_SIGN_P12): signing ad hoc"
    find "$APP/Contents/Frameworks" -type f -name '*.dylib' -print0 |
        xargs -0 codesign --force --timestamp=none --sign -
    codesign --force --timestamp=none --sign - "$APP"
fi

# A signature that does not verify is the "damaged" message again, found by whoever downloads it.
codesign --verify --deep --strict --verbose "$APP"

# SIGNED, BUT NOT BY APPLE, AND SAID SO IN EVERY LANGUAGE OF THE PROGRAM. A certificate Apple did not issue proves the
# bundle is whole, not who made it, so the system refuses to open the download the first time: "Apple could
# not verify QymCAD is free of malware", with "Done" and "Move to Bin".
#
# THE WAY PAST IT IS IN SYSTEM SETTINGS, NOT IN A TERMINAL. Once the bundle carries a signature that
# verifies, Privacy & Security lists the refused program with an "Open Anyway" button; one press, the
# password, and the program opens from then on by an ordinary double click. Reported behaviour: the
# Terminal steps the notes used to give (`xattr -cr`) are outdated - they were the way round a BROKEN
# signature, which the system called damaged and offered no button for. The Control-click-and-Open
# exception is gone as well on a current macOS.
#
# ONE NOTE PER LANGUAGE OF THE PROGRAM, TAKEN FROM THE LANGUAGE ITSELF. Each folder of i18n/ is a language and
# keeps its note in `macos/`, named in that language so it is found at a glance.
# The notes go into one folder, README, so the window shows one icon for them however many languages there
# are: a language added to i18n/ with its note reaches the image with no edit here, in settings.py or in the
# picture. A language with no note, or with two, stops the build - its reader would find nothing, or two.
#
# The one word that differs between the two builds - the processor - is written `{ $machine }` in a note,
# the way a substitution is written in the catalogue beside it. It holds no `*`, `?` or `[`, so as a pattern
# it matches only itself, on the bash 3.2 a mac ships as well as on a newer one.
placeholder='{ $machine }'
NOTES=dist/README
rm -rf "$NOTES"
mkdir -p "$NOTES"
for lang in i18n/*/; do
    notes=("${lang}"macos/*.txt)
    if [ "${#notes[@]}" -ne 1 ] || [ ! -f "${notes[0]}" ]; then
        echo "!!! ${lang}macos/ must hold one note for the disk image, a .txt named in its language"
        exit 1
    fi
    name=$(basename "${notes[0]}")
    [ ! -e "$NOTES/$name" ] || { echo "!!! two languages name their note $name - ${lang}macos/"; exit 1; }
    text=$(<"${notes[0]}")
    printf '%s\n' "${text//$placeholder/$MACHINE}" > "$NOTES/$name"
done
echo ">>> notes: $(find "$NOTES" -type f | wc -l | tr -d ' ')"

# THE DISK IMAGE, the form a mac download is expected in. Opened, it shows one window drawn for it: the
# program on the left, Applications on the right, an arrow from one to the other, and under them the folder
# of notes beside a picture of the way to "Open Anyway". It holds the signed bundle - made after the check
# above, so nothing unverified goes in - and the notes, because the first launch is refused the same way for the copy dragged out of it. The
# mounted volume carries the program's icon.
#
# THE ONLY PACKAGE. A zip of the same bundle beside it would be a second file per processor to choose
# between on the download page, and nothing takes a zip that the image does not serve.
#
# `dmgbuild`, NOT A FINDER SCRIPT. The window's layout - its size, the background, where each icon stands
# - lives in a `.DS_Store` file inside the image. The usual way to make one is to mount the image and have
# Finder arrange it through AppleScript, which needs a logged-in session with Finder automation allowed;
# `dmgbuild` writes that file itself and runs headless. The places and the picture both come from
# packaging/macos/dmg/settings.py, see there and draw_background.py beside it.
#
# Neither the staging copy (`cp -RP`) nor the image breaks the seal: `codesign --verify --deep --strict`
# passes on the QymCAD bundle inside the mounted image, measured on a release build with 67 OCCT libraries,
# and a signed test bundle starts from the image with its libraries found through the links.
DMG="dist/$NAME-macos-$ARCH.dmg"
STAGE=dist/dmg
rm -rf "$STAGE" "$DMG"
mkdir -p "$STAGE"
cp -RP "$APP" "$NOTES" "$STAGE/"
command -v dmgbuild >/dev/null || { echo "!!! dmgbuild is not installed: python3 -m pip install --require-hashes -r packaging/macos/dmg/requirements.txt"; exit 1; }
# The layout lies beside this script, wherever the script is run from.
ART="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/dmg"
dmgbuild -s "$ART/settings.py" -D stage="$STAGE" -D art="$ART" QymCAD "$DMG"
rm -rf "$STAGE"
[ -f "$DMG" ] || { echo "!!! dmgbuild reported success and made no $DMG"; exit 1; }
echo ">>> DONE: $DMG ($(du -h "$DMG" | cut -f1))"
