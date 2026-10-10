# Build the portable ZIP for Windows. Runs after `cargo build --release` on a windows runner with MSVC.
#
# It replaces bundle.sh, which collected the dependent DLLs by running `ldd` - a tool that exists in MSYS2
# and nowhere else. Under MSVC the set of libraries to carry is known rather than discovered: the OCCT
# DLLs we built ourselves, and the three redistributable Visual C++ runtime files.
$ErrorActionPreference = "Stop"

$bin = "target\release\qymcad.exe"
if (-not (Test-Path $bin)) { throw "no $bin - run cargo build --release first" }

# THE NAME. A tag names the package itself; anything else is named by the manifest version plus the commit,
# or two builds three days apart share a file name and a report cannot be traced to either.
if ($env:QYMCAD_VERSION) {
    $name = "qymcad-" + ($env:QYMCAD_VERSION -replace '^v', '')
} else {
    $ver = (Select-String -Path Cargo.toml -Pattern '^version' | Select-Object -First 1).Line -replace '[^0-9.]', ''
    if (-not $ver) { $ver = "0.0.0" }
    $sha = (git rev-parse --short=9 HEAD 2>$null)
    $name = if ($sha) { "qymcad-$ver-dev.$sha" } else { "qymcad-$ver" }
}

$out = "dist\qymcad"
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force -Path $out | Out-Null
Copy-Item $bin $out

# --- the OCCT libraries, from the build we made ourselves ---
$occtBin = Join-Path $env:OCCT_ROOT "bin"
if (-not (Test-Path $occtBin)) { throw "no OCCT binaries at $occtBin" }
$dlls = Get-ChildItem "$occtBin\*.dll"
if ($dlls.Count -eq 0) { throw "$occtBin holds no DLLs" }
Write-Host ">>> OCCT libraries: $($dlls.Count)"
Copy-Item $dlls.FullName $out

# --- the Visual C++ runtime ---
#
# OCCT is built against the dynamic runtime, so those three files have to travel with it. They are
# redistributable by their own licence, and shipping them means nobody has to install anything first.
$redist = Get-ChildItem "C:\Program Files*\Microsoft Visual Studio\*\*\VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT" -Directory -ErrorAction SilentlyContinue |
          Sort-Object FullName | Select-Object -Last 1
if ($redist) {
    Write-Host ">>> Visual C++ runtime from $($redist.FullName)"
    Copy-Item "$($redist.FullName)\*.dll" $out
} else {
    # Not fatal on purpose: the archive is still worth having, and this says plainly what is missing from
    # it rather than producing something that fails on a stranger's machine without explanation.
    Write-Warning "the Visual C++ runtime was not found - the archive will need it installed on the machine"
}

# --- the licence and the notices travel with the binary ---
Copy-Item LICENSE "$out\LICENSE.txt"
Copy-Item THIRD-PARTY-NOTICES.md $out

# A SHORT NOTE IN BOTH LANGUAGES. Whoever unpacks this may read either, and a note in a language they do
# not read is the same as no note at all.
@"
QymCAD - portable build for Windows (x64).
To run: qymcad.exe (every DLL it needs is right here - nothing to install).
Requires Windows 10/11 x64.
"@ | Set-Content -Encoding UTF8 "$out\README.txt"

@"
QymCAD - переносимая сборка для Windows (x64).
Запуск: qymcad.exe (все нужные DLL лежат рядом - ничего ставить не надо).
Требуется Windows 10/11 x64.
"@ | Set-Content -Encoding UTF8 "$out\ПРОЧТИ.txt"

$zip = "dist\$name-win64.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path "$out\*" -DestinationPath $zip
Write-Host ">>> DONE: $zip  ($([math]::Round((Get-Item $zip).Length / 1MB, 1)) MB)"
