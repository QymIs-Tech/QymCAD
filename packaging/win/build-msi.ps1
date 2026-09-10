# Build the MSI installer for Windows, from the same folder the portable zip is made of.
#
# Runs after packaging/win/bundle.ps1 on a windows runner. WiX 5 is a dotnet tool, and the version is
# part of the instruction:
#   dotnet tool install --global wix --version 5.0.2
#
# Without the version dotnet takes the newest, and the newest stops with "WIX7015: You must accept the
# Open Source Maintenance Fee (OSMF) EULA to use WiX Toolset v7". That fee arrived in v6 and is asked of
# everyone earning money from the toolset. Version 5 predates it and reads this same file: the namespace
# below is http://wixtoolset.org/schemas/v4/wxs, which v5 speaks natively.
$ErrorActionPreference = "Stop"

$payload = "dist\qymcad"
if (-not (Test-Path "$payload\qymcad.exe")) { throw "no $payload\qymcad.exe - run packaging/win/bundle.ps1 first" }

# --- the version, in the four numbers an MSI understands ---
#
# MSI compares versions as a.b.c.d with each part below 65536, and IGNORES the fourth entirely. A tag like
# v0.1.0-dev.20260908 has to become something in that shape, or every dev build would look like the same
# version to Windows and upgrades would silently do nothing.
#
# The date is what carries the ordering, so it is folded into the third part: 0.1.0-dev.20260908 becomes
# 0.1.60908 - the year is dropped and the month-day pair fits under the limit. A tagged release keeps its
# own three numbers.
if ($env:QYMCAD_VERSION) { $tag = $env:QYMCAD_VERSION -replace '^v', '' }
else {
    $ver = (Select-String -Path Cargo.toml -Pattern '^version' | Select-Object -First 1).Line -replace '[^0-9.]', ''
    $tag = if ($ver) { $ver } else { "0.0.0" }
}
if ($tag -match '^(\d+)\.(\d+)\.(\d+)-dev\.(\d{4})(\d{2})(\d{2})$') {
    $msiVersion = "{0}.{1}.{2}{3}{4}" -f $Matches[1], $Matches[2], $Matches[3], $Matches[5], $Matches[6]
} elseif ($tag -match '^(\d+)\.(\d+)\.(\d+)') {
    $msiVersion = "{0}.{1}.{2}" -f $Matches[1], $Matches[2], $Matches[3]
} else {
    throw "the version '$tag' is not a shape an MSI can compare"
}

# --- the product code, computed rather than generated ---
#
# Windows tells one version from another by the product code, and it MUST differ between versions or an
# upgrade turns into "the same thing is already installed". A fresh random GUID at every build would do
# that - but then nobody knows the code afterwards, and winget wants it in the manifest to recognise what
# it installed. Computing it from the upgrade code and the version gives both: different per version, and
# knowable without opening the MSI.
$upgradeCode = "DF949703-CA05-535F-80FD-8404D9704F33"
function New-DeterministicGuid([string]$namespace, [string]$name) {
    # RFC 4122 name-based UUID, version 5 (SHA-1), the same value Python's uuid5 gives for these inputs
    $ns = [guid]$namespace
    $b = $ns.ToByteArray()
    # .NET lays the first three fields out little-endian; the RFC wants them big-endian
    foreach ($r in @(@(0,3), @(4,5), @(6,7))) { [array]::Reverse($b, $r[0], $r[1] - $r[0] + 1) }
    $sha = [System.Security.Cryptography.SHA1]::Create()
    $hash = $sha.ComputeHash($b + [System.Text.Encoding]::UTF8.GetBytes($name))
    $g = $hash[0..15]
    $g[6] = ($g[6] -band 0x0F) -bor 0x50   # version 5
    $g[8] = ($g[8] -band 0x3F) -bor 0x80   # RFC 4122 variant
    foreach ($r in @(@(0,3), @(4,5), @(6,7))) { [array]::Reverse($g, $r[0], $r[1] - $r[0] + 1) }
    return ([guid][byte[]]$g).ToString().ToUpper()
}
$productCode = New-DeterministicGuid $upgradeCode $msiVersion

$name = "qymcad-$tag-x64.msi"
$out = "dist\$name"
Write-Host ">>> MSI $msiVersion, product code $productCode"

wix build packaging\win\qymcad.wxs `
    -arch x64 `
    -d "Version=$msiVersion" `
    -d "ProductCode=$productCode" `
    -d "PayloadDir=$((Resolve-Path $payload).Path)" `
    -d "IconPath=$((Resolve-Path 'assets\icons\windows\qymcad.ico').Path)" `
    -o $out
if ($LASTEXITCODE -ne 0) { throw "wix build failed" }

# THE PRODUCT CODE IS WRITTEN DOWN BESIDE THE PACKAGE. The winget manifest needs it, and reading it back
# out of an MSI needs Windows - so the build that knows it says so, once, in a file.
"$productCode" | Set-Content -Encoding ASCII "$out.productcode"
Write-Host ">>> DONE: $out  ($([math]::Round((Get-Item $out).Length / 1MB, 1)) MB)"
