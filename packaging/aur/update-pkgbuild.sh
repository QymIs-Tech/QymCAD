#!/usr/bin/env bash
# Point the AUR package at a release: rewrite the version and the checksum in PKGBUILD.
#
# WHY A SCRIPT AND NOT A PAIR OF EDITS BY HAND. Three things have to agree - the version pacman sees, the
# name the file really has on the release page, and the checksum of that file - and they are edited in
# three different lines. Done by hand, the checksum is the one that gets forgotten, and a stale checksum
# does not fail quietly: it fails on somebody else's machine, at install time, with a message about
# corruption.
#
# THE CHECKSUM COMES FROM THE RELEASE ITSELF, not from a downloaded copy: GitHub publishes the digest of
# every asset, so nothing has to be fetched to learn it.
#
#   packaging/aur/update-pkgbuild.sh v0.1.0-dev.20260828
set -euo pipefail

TAG=${1:-}
if [ -z "$TAG" ]; then
    echo "usage: $0 <release tag, e.g. v0.1.0-dev.20260828>" >&2
    exit 2
fi
REPO=${QYMCAD_REPO:-QymIs-Tech/QymCAD}
HERE=$(cd "$(dirname "$0")" && pwd)

relver=${TAG#v}
file="qymcad-${relver}-x86_64.AppImage"

digest=$(gh api "repos/${REPO}/releases/tags/${TAG}" -q ".assets[] | select(.name == \"${file}\") | .digest")
if [ -z "$digest" ]; then
    echo "!!! the release ${TAG} of ${REPO} has no asset named ${file}" >&2
    exit 1
fi
sha=${digest#sha256:}
if [ "$sha" = "$digest" ]; then
    echo "!!! the release publishes the digest as \"${digest}\", which is not a sha256" >&2
    exit 1
fi

# A hyphen is what separates pkgver from pkgrel, so it cannot appear inside pkgver itself.
pkgver=${relver//-/.}

sed -i \
    -e "s/^pkgver=.*/pkgver=${pkgver}/" \
    -e "s/^_relver=.*/_relver=${relver}/" \
    -e "s/^pkgrel=.*/pkgrel=1/" \
    -e "s/^sha256sums=.*/sha256sums=('${sha}')/" \
    "$HERE/PKGBUILD"

# .SRCINFO IS GENERATED, NEVER EDITED. It is what the AUR reads to learn the version and the dependencies,
# and a hand-written one drifts from the PKGBUILD beside it - after which the site shows one version and
# the package builds another.
( cd "$HERE" && makepkg --printsrcinfo > .SRCINFO )

echo ">>> PKGBUILD now points at ${TAG}"
grep -E '^(pkgver|_relver|pkgrel|sha256sums)=' "$HERE/PKGBUILD"
