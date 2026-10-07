#!/usr/bin/env bash
# Assemble QymCAD into a macOS .app and zip it. Runs after `cargo build --release` on an Apple Silicon
# machine, with OCCT installed at $OCCT_ROOT (built from source - see .github/workflows/release.yml).
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
# followed), and `zip -y` stores them as links instead of expanding them again in the archive.
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

# SIGNED, BUT NOT BY APPLE, AND SAID SO IN BOTH LANGUAGES. A certificate Apple did not issue proves the
# bundle is whole, not who made it, so the system still marks the download as quarantined and refuses to
# open it the first time.
#
# THE RIGHT-CLICK IS NOT THE WAY ANY MORE. These notes used to say "Control-click and choose Open", the
# advice that worked for years. Reported behaviour: on a current macOS it does nothing - the same refusal
# appears, and the program went to the Bin instead. Recent releases block code that carries no signature
# from an identified developer in every condition, and the Control-click exception went with them.
#
# What is left is the mark itself: `xattr -cr` clears the quarantine attribute, and the program then opens
# by an ordinary double click. It is done once per download - an extended attribute stays cleared, a
# restart does not bring it back - so the steps are written out plainly, for a person who has never opened
# a terminal.
cat > dist/README.txt <<'TXT'
QymCAD - build for macOS (Apple Silicon).

FIRST RUN. The build is signed, but not with a certificate Apple issued, and macOS marks everything
downloaded from the internet as "quarantined": it will refuse to open the app, saying Apple cannot
check it or that it is damaged, and offer to move it to the Bin. It is not damaged. The mark has to
be cleared, once.

  1. Unpack the archive.

  2. Open Terminal: Command+Space, type "Terminal", press Enter.

  3. Type this into it, with a space at the end. Do NOT press Enter yet:

        xattr -cr 

  4. Drag QymCAD.app into the Terminal window - the path fills itself in. Now press Enter.
     Nothing is printed in reply; that is how it should be.

  5. Open QymCAD.app with an ordinary double click.

The mark is gone for good on this copy: a restart does not bring it back. A build downloaded anew
has to be cleared the same way.

Requires macOS 12 or newer, an Apple Silicon machine.
TXT

cat > dist/ПРОЧТИ.txt <<'TXT'
QymCAD - сборка для macOS (Apple Silicon).

ПЕРВЫЙ ЗАПУСК. Сборка подписана, но не сертификатом, выданным Apple, а macOS помечает всё скачанное
из интернета «карантином»: она откажется открыть программу, сказав, что Apple не может её проверить
или что она повреждена, и предложит переместить её в Корзину. Она не повреждена. Метку нужно снять,
один раз.

  1. Распакуйте архив.

  2. Откройте Терминал: Command+Пробел, наберите «Терминал», Enter.

  3. Наберите в нём вот это, с пробелом в конце. Enter пока НЕ нажимайте:

        xattr -cr 

  4. Перетащите QymCAD.app мышью прямо в окно Терминала — путь подставится сам. Вот теперь Enter.
     В ответ ничего не напечатается, так и должно быть.

  5. Откройте QymCAD.app обычным двойным щелчком.

Метка снята навсегда для этой копии: перезагрузка её не вернёт. Сборку, скачанную заново, придётся
освободить так же.

Требуется macOS 12 или новее, компьютер на Apple Silicon.
TXT

ZIP="dist/$NAME-macos-arm64.zip"
rm -f "$ZIP"
( cd dist && zip -r -y -q "$(basename "$ZIP")" QymCAD.app README.txt ПРОЧТИ.txt )
echo ">>> DONE: $ZIP ($(du -h "$ZIP" | cut -f1))"
