# Build the portable ZIP for Windows 7/8.x (x64). Runs after building for x86_64-win7-windows-msvc.
#
# Packages the application binary, OCCT libraries, the combase.dll export forwarder,
# Visual C++ runtime DLLs, application assets, and documentation.
$ErrorActionPreference = "Stop"

$bin = if ($env:QYMCAD_BIN) { $env:QYMCAD_BIN } else { "target\x86_64-win7-windows-msvc\release\qymcad.exe" }
if (-not (Test-Path $bin)) {
    $bin = "target\release\qymcad.exe"
}
if (-not (Test-Path $bin)) { throw "no binary at $bin - run cargo build for x86_64-win7-windows-msvc first" }

# A tag names the package itself; anything else is named by manifest version and commit hash.
if ($env:QYMCAD_VERSION) {
    $name = "qymcad-" + ($env:QYMCAD_VERSION -replace '^v', '')
} else {
    $ver = (Select-String -Path Cargo.toml -Pattern '^version' | Select-Object -First 1).Line -replace '[^0-9.]', ''
    if (-not $ver) { $ver = "0.0.0" }
    $sha = (git rev-parse --short=9 HEAD 2>$null)
    $name = if ($sha) { "qymcad-$ver-dev.$sha" } else { "qymcad-$ver" }
}

$out = "dist\qymcad-win7"
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force -Path $out | Out-Null
Copy-Item $bin "$out\qymcad.exe"

# --- combase.dll export forwarder ---
#
# Windows 7 lacks combase.dll (introduced in Windows 8). Basic COM functions live in ole32.dll.
# The PE loader resolves exported forwarders directly into ole32.dll.
$fwdSrc = "packaging\win\combase_forwarder.c"
$fwdDll = "$out\combase.dll"
if (Test-Path $fwdSrc) {
    Write-Host ">>> Compiling combase.dll forwarder..."
    if (Get-Command clang -ErrorAction SilentlyContinue) {
        $obj = "$out\combase.obj"
        & clang --target=x86_64-pc-windows-msvc -c $fwdSrc -o $obj
        & lld-link /dll /machine:x64 /entry:DllMainCRTStartup $obj /out:$fwdDll
        Remove-Item $obj -ErrorAction SilentlyContinue
    } elseif (Get-Command cl.exe -ErrorAction SilentlyContinue) {
        & cl.exe /nologo /LD /O2 /Fe:$fwdDll $fwdSrc /link /MACHINE:X64 /NOENTRY
        Remove-Item "$out\combase.obj", "$out\combase.lib", "$out\combase.exp" -ErrorAction SilentlyContinue
    } else {
        Write-Warning "neither clang nor cl.exe was found - combase.dll forwarder was not built"
    }
}

# --- OCCT libraries ---
$occtBin = Join-Path $env:OCCT_ROOT "bin"
if (-not (Test-Path $occtBin)) { throw "no OCCT binaries at $occtBin" }
$dlls = Get-ChildItem "$occtBin\*.dll"
if ($dlls.Count -eq 0) { throw "$occtBin holds no DLLs" }
Write-Host ">>> OCCT libraries: $($dlls.Count)"
Copy-Item $dlls.FullName $out

# --- Visual C++ runtime ---
$redist = Get-ChildItem "C:\Program Files*\Microsoft Visual Studio\*\*\VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT" -Directory -ErrorAction SilentlyContinue |
          Sort-Object FullName | Select-Object -Last 1
if ($redist) {
    Write-Host ">>> Visual C++ runtime from $($redist.FullName)"
    Copy-Item "$($redist.FullName)\*.dll" $out
} else {
    Write-Warning "the Visual C++ runtime was not found - the archive will need it installed on the machine"
}

# --- assets, license, and notices ---
if (Test-Path "assets") {
    Copy-Item "assets" -Destination "$out\assets" -Recurse
}
Copy-Item LICENSE "$out\LICENSE.txt"
Copy-Item THIRD-PARTY-NOTICES.md $out

# --- user instructions and platform notes ---
@"
QymCAD - portable build for Windows 7 SP1 / 8 / 8.1 / 10 / 11 (x64).
To run: qymcad.exe (all required runtime DLLs are included in this directory).

System Requirements:
- Windows 7 SP1 (64-bit), Windows 8/8.1 (64-bit), or Windows 10/11 (64-bit).
- On Windows 7: Service Pack 1 and KB2999226 (Universal CRT) or Visual C++ Redistributable are required.
- GPU with OpenGL 3.3+ / DirectX 11 support.

Notice:
This build provides compatibility with legacy Windows versions (starting with Windows 7 SP1).
Operation on Windows 7 and 8 is provided as-is without guarantee of full stability or frequent updates.
For Windows 10/11, the primary release build is recommended.
"@ | Set-Content -Encoding UTF8 "$out\README.txt"

$zip = "dist\$name-win7-x64.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path "$out\*" -DestinationPath $zip
Write-Host ">>> DONE: $zip ($([math]::Round((Get-Item $zip).Length / 1MB, 1)) MB)"
