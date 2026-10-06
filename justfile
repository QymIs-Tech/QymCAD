# QymCAD - development and packaging commands.
# `just` = casey/just (Arch: pacman -S just). The list of commands: `just --list`.
#
# THE DEV WORKFLOW DOES NOT CHANGE: `cargo run` / `cargo test` in the dev profile, as always.
# Packages are built SEPARATELY (release) and do not disturb the dev environment.

set shell := ["bash", "-uc"]

# run the application in dev mode (as usual)
dev:
    cargo run --bin qymcad

# run the workspace tests
test:
    cargo test

# check translation coverage and list missing keys (`just i18n` or `just i18n uk`)
i18n lang="":
    python3 tools/i18n.py {{lang}}

# --- Linux AppImage (locally, through Docker; OCCT 7.9.3 from source, glibc 2.35) ---
IMG := "qymcad-appimage-builder"

# once (and after every edit of the Dockerfile): build the builder image
pkg-linux-image:
    docker build -t {{IMG}} packaging/linux

# build the AppImage -> dist/qymcad-<ver>-x86_64.AppImage
pkg-linux: pkg-linux-image
    mkdir -p dist
    docker run --rm \
        -v "$PWD":/src -v "$PWD/dist":/dist \
        {{IMG}} bash packaging/linux/build-appimage.sh
    @echo "-> the files are in ./dist"

# --- Windows portable zip ---
# The main path is GitHub Actions (.github/workflows/release.yml): push a vX.Y.Z tag, or Actions -> Run workflow.
# Locally (in a Win10/11 VM with Visual Studio Build Tools) it goes like this - the kernel is built from
# source first, see packaging/README.md:
#   $env:OCCT_INCLUDE_DIR = "C:\occt\include\opencascade"; $env:OCCT_ROOT = "C:\occt"
#   cargo build --release --bin qymcad; pwsh -File packaging/win/bundle.ps1

# --- Windows 7 portable zip (locally or CI) ---
pkg-win7:
    python3 packaging/win/bundle-win7.py

# clear out what packaging produced
clean-dist:
    rm -rf dist
