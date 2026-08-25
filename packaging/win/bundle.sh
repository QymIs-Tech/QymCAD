#!/usr/bin/env bash
# Build the portable ZIP for Windows. Runs under MSYS2/UCRT64 (see .github/workflows/release.yml): puts
# qymcad.exe next to every mingw DLL it depends on (OCCT TK*.dll, gcc/stdc++, tbb and so on) -> archive.
set -euo pipefail

BIN=target/release/qymcad.exe
[ -f "$BIN" ] || { echo "!!! no $BIN - run cargo build --release first"; exit 1; }

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

ZIP="$(package_name)-win64.zip"

OUT=dist/qymcad
rm -rf "$OUT"; mkdir -p "$OUT"
cp "$BIN" "$OUT/"

# The dependent DLLs from the MSYS2 prefix (the system ones in C:\Windows\* are NOT carried: everyone has
# them). BOTH PREFIXES ARE MATCHED: builds run in ucrt64, but a local build in an older mingw64 shell must
# not silently produce an archive with no DLLs in it - one that unpacks and refuses to start is worse than
# one that fails to build.
echo ">>> collecting the DLLs by ldd"
ldd "$BIN" | awk '/(ucrt64|mingw64)\/bin/ {print $3}' | sort -u | while read -r dll; do
    [ -f "$dll" ] && cp -v "$dll" "$OUT/"
done

# A SHORT NOTE IN BOTH LANGUAGES. The person who unpacks this archive may read either, and a note in a
# language they do not read is the same as no note at all.
cat > "$OUT/README.txt" <<'TXT'
QymCAD - portable build for Windows (x64).
To run: qymcad.exe (every DLL it needs is right here - nothing to install).
Requires Windows 10/11 x64.
TXT

cat > "$OUT/ПРОЧТИ.txt" <<'TXT'
QymCAD - переносимая сборка для Windows (x64).
Запуск: qymcad.exe (все нужные DLL лежат рядом - ничего ставить не надо).
Требуется Windows 10/11 x64.
TXT

# The licence and the third-party notices TRAVEL WITH THE BINARY. AGPL-3.0 asks for the licence text to
# accompany the program, and LGPL-2.1 (OCCT) for the notice; a link in a repository the person may never
# open does not count as either.
cp LICENSE "$OUT/LICENSE.txt"
cp THIRD-PARTY-NOTICES.md "$OUT/"

mkdir -p dist
( cd dist && zip -r -q "$ZIP" qymcad )
echo ">>> DONE: dist/$ZIP"
