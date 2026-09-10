#!/usr/bin/env bash
# Fill the winget manifests for a release and lay them out the way winget-pkgs expects.
#
# WHY TEMPLATES RATHER THAN FILES KEPT UP TO DATE BY HAND. Three of the values - the version, the checksum
# and the product code - are known only once a release exists, and a manifest carrying invented ones is
# worse than none: it is a file that looks finished and installs the wrong thing. So the repository keeps
# the shape, and this writes the copy that gets submitted.
#
# The result lands in dist/winget/manifests/q/QymIsTech/QymCAD/<version>/ - the exact path microsoft's
# repository uses, so submitting is a copy of one directory.
#
#   packaging/winget/update-manifests.sh v0.1.0
set -euo pipefail

TAG=${1:-}
if [ -z "$TAG" ]; then
    echo "usage: $0 <release tag, e.g. v0.1.0>" >&2
    exit 2
fi
REPO=${QYMCAD_REPO:-QymIs-Tech/QymCAD}
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)

relver=${TAG#v}
file="qymcad-${relver}-x64.msi"

read -r sha date < <(gh api "repos/${REPO}/releases/tags/${TAG}" \
    -q ".assets[] | select(.name == \"${file}\") | \"\(.digest) \(.created_at)\"" | sed 's/^sha256://')
if [ -z "${sha:-}" ]; then
    echo "!!! the release ${TAG} of ${REPO} has no asset named ${file}" >&2
    echo "!!! the MSI is built by packaging/win/build-msi.ps1 - is it attached to the release?" >&2
    exit 1
fi
date=${date%%T*}

# THE VERSION AN MSI COMPARES BY, and the product code computed from it. Both rules live in
# packaging/win/build-msi.ps1; repeating them here is deliberate and guarded - the manifest must name the
# code the installer really carries, and the installer is built on a different machine, in a different
# language, hours earlier.
msiver=$(python3 - "$relver" <<'PY'
import re, sys
v = sys.argv[1]
m = re.fullmatch(r'(\d+)\.(\d+)\.(\d+)-dev\.(\d{4})(\d{2})(\d{2})', v)
if m:
    print(f"{m[1]}.{m[2]}.{m[3]}{m[5]}{m[6]}")
else:
    m = re.match(r'(\d+)\.(\d+)\.(\d+)', v)
    if not m:
        sys.exit(f"the version '{v}' is not a shape an MSI can compare")
    print(f"{m[1]}.{m[2]}.{m[3]}")
PY
)
product=$(python3 -c "
import uuid, sys
print(str(uuid.uuid5(uuid.UUID('DF949703-CA05-535F-80FD-8404D9704F33'), sys.argv[1])).upper())
" "$msiver")

OUT="$ROOT/dist/winget/manifests/q/QymIsTech/QymCAD/${relver}"
rm -rf "$OUT"
mkdir -p "$OUT"
for src in version locale.en-US installer; do
    case "$src" in
        version)      dst="QymIsTech.QymCAD.yaml" ;;
        locale.en-US) dst="QymIsTech.QymCAD.locale.en-US.yaml" ;;
        installer)    dst="QymIsTech.QymCAD.installer.yaml" ;;
    esac
    sed -e "s|@VERSION@|${relver}|g" \
        -e "s|@TAG@|${relver}|g" \
        -e "s|@SHA256@|${sha}|g" \
        -e "s|@PRODUCTCODE@|${product}|g" \
        -e "s|@RELEASEDATE@|${date}|g" \
        "$HERE/templates/${src}.yaml" > "$OUT/$dst"
done

# NOTHING UNFILLED LEAVES THIS SCRIPT. A placeholder that survives into a submitted manifest is a package
# that installs a file nobody checked.
if grep -rn '@[A-Z0-9]*@' "$OUT"; then
    echo "!!! a placeholder was left unfilled - see the lines above" >&2
    exit 1
fi

echo ">>> ${OUT}"
ls -1 "$OUT"
echo ">>> product code ${product} (msi version ${msiver}), checksum ${sha}"
echo ">>> to submit: copy that directory into a fork of microsoft/winget-pkgs and open a pull request"
