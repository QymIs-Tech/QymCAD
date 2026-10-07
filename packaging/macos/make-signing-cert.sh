#!/usr/bin/env bash
# Make the self-signed code-signing certificate the macOS build is signed with, and print it in the form
# the release workflow takes it: the `MACOS_CERT_P12` secret holds the base64 of the .p12, and
# `MACOS_CERT_PASSWORD` the password it was exported with.
#
# SELF-SIGNED IS NOT APPLE-SIGNED. macOS does not know this certificate, so Gatekeeper still asks the person
# to allow the program once. What the signature gives is a sealed bundle - nothing in it can be changed
# without the signature breaking - and one identity across releases, under which permissions granted to one
# version stay granted to the next.
#
# Run it once, on any machine with openssl; keep the .p12 and the password somewhere safe. A new certificate
# is a new identity, and every permission granted under the old one is asked again.
set -euo pipefail

CN_NAME=${CN_NAME:-QymCAD}
DAYS_VALID=3650
OUT=${OUT:-.}
KEY_FILE="$OUT/qymcad-signing.key"
CRT_FILE="$OUT/qymcad-signing.crt"
P12_FILE="$OUT/qymcad-signing.p12"
B64_FILE="$OUT/qymcad-signing.p12.b64"

command -v openssl >/dev/null || { echo "!!! openssl is not installed" >&2; exit 1; }

if [ -z "${P12_PASSWORD:-}" ]; then
    read -r -s -p "Password for the .p12: " P12_PASSWORD
    echo
    read -r -s -p "The same again: " P12_PASSWORD_CONFIRM
    echo
    [ "$P12_PASSWORD" = "$P12_PASSWORD_CONFIRM" ] || { echo "!!! the passwords differ" >&2; exit 1; }
fi

openssl req -x509 -newkey rsa:2048 -nodes \
    -keyout "$KEY_FILE" \
    -out "$CRT_FILE" \
    -days "$DAYS_VALID" \
    -subj "/CN=${CN_NAME}" \
    -addext "keyUsage = critical, digitalSignature" \
    -addext "extendedKeyUsage = critical, codeSigning"

# THE OLD CIPHERS, OR THE KEYCHAIN CANNOT READ IT. OpenSSL 3 and newer encrypt a .p12 with AES and PBKDF2
# by default, and `security import` on macOS refuses that file with "MAC verification failed during PKCS12
# import (wrong password?)" - measured with OpenSSL 4.0.3, the password being right. `-legacy` writes
# 3DES/RC2, which it reads. LibreSSL, the `openssl` macOS ships, writes those by default and has no such
# flag.
LEGACY=()
if openssl pkcs12 -help 2>&1 | grep -q -- '-legacy'; then
    LEGACY=(-legacy)
fi
# The `+` form, because the bash macOS ships (3.2) calls an empty array unbound under `set -u`.
openssl pkcs12 -export ${LEGACY[@]+"${LEGACY[@]}"} \
    -out "$P12_FILE" \
    -inkey "$KEY_FILE" \
    -in "$CRT_FILE" \
    -passout env:P12_PASSWORD

# `base64 -w 0` is GNU only: the macOS one has no `-w` and wraps nothing anyway. `tr` gives one line on both.
base64 < "$P12_FILE" | tr -d '\n' > "$B64_FILE"

if command -v pbcopy >/dev/null; then
    pbcopy < "$B64_FILE"
    echo ">>> the base64 is on the clipboard"
elif command -v xclip >/dev/null; then
    xclip -selection clipboard < "$B64_FILE"
    echo ">>> the base64 is on the clipboard"
fi

# The key is inside the .p12 already; a second, unencrypted copy beside it is only a thing to leak.
rm -f "$KEY_FILE"

echo ">>> $P12_FILE   - keep it, with its password"
echo ">>> $B64_FILE   - the value of the MACOS_CERT_P12 secret"
echo ">>> $CRT_FILE   - the public certificate, safe to publish"
echo ">>> gh secret set MACOS_CERT_P12 < $B64_FILE"
echo ">>> gh secret set MACOS_CERT_PASSWORD"
