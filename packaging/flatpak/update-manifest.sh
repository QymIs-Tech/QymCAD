#!/usr/bin/env bash
# Point the Flatpak manifest at a release and regenerate the list of crate sources.
#
# TWO THINGS HAVE TO MOVE TOGETHER: the tag with its commit, and cargo-sources.json. The second is the list
# of every crate in the lock file, written out as declared sources - the build sandbox has no network, so
# a crate that is not on that list is a build that stops halfway with a message about being offline.
#
#   packaging/flatpak/update-manifest.sh v0.1.0-dev.20260908
set -euo pipefail

TAG=${1:-}
if [ -z "$TAG" ]; then
    echo "usage: $0 <release tag, e.g. v0.1.0-dev.20260908>" >&2
    exit 2
fi
REPO=${QYMCAD_REPO:-QymIs-Tech/QymCAD}
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)

commit=$(gh api "repos/${REPO}/git/ref/tags/${TAG}" -q '.object.sha')
if [ -z "$commit" ]; then
    echo "!!! ${REPO} has no tag ${TAG}" >&2
    exit 1
fi
# An annotated tag points at a tag object, which points at the commit; a lightweight one points at the
# commit directly. Ask what this one is rather than assuming.
kind=$(gh api "repos/${REPO}/git/ref/tags/${TAG}" -q '.object.type')
if [ "$kind" = "tag" ]; then
    commit=$(gh api "repos/${REPO}/git/tags/${commit}" -q '.object.sha')
fi

sed -i -e "s|^        tag: .*|        tag: ${TAG}|" -e "s|^        commit: .*|        commit: ${commit}|" "$HERE/tech.qymis.cad.yml"

# --- the crates, as declared sources ---
#
# The generator is a script from the flatpak-builder-tools repository. It is not vendored here: it reads
# Cargo.lock and writes JSON, and a copy of somebody else's tool goes stale where nobody looks.
GEN=${FLATPAK_CARGO_GENERATOR:-}
if [ -z "$GEN" ]; then
    echo "!!! set FLATPAK_CARGO_GENERATOR to the path of flatpak-cargo-generator.py" >&2
    echo "!!!   git clone https://github.com/flatpak/flatpak-builder-tools" >&2
    echo "!!!   FLATPAK_CARGO_GENERATOR=.../cargo/flatpak-cargo-generator.py $0 $TAG" >&2
    exit 1
fi
python3 "$GEN" "$ROOT/Cargo.lock" -o "$HERE/cargo-sources.json"

# THE PICTURES ARE NOT TOUCHED HERE. Their addresses are pinned in the metainfo to the commit that
# added them, once: they do not change from release to release, and naming the tag closed a circle -
# an address would name the tag, and the tag has to stand on a commit whose file already carries that
# address. See the comment in tech.qymis.cad.metainfo.xml.

echo ">>> the manifest now points at ${TAG} (${commit})"
grep -E '^        (tag|commit):' "$HERE/tech.qymis.cad.yml"
echo ">>> crate sources: $(python3 -c "import json,sys; print(len(json.load(open('$HERE/cargo-sources.json'))))")"
