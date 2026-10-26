# pktito as an MSIX package, for the Microsoft Store: the release build's
# two exes, unchanged, with src-tauri/windows/msix/AppxManifest.xml.
#
#     scripts/msix.ps1 -Arch x64 [-Exes DIR] [-Out FILE] [-Identity store|test|sideload]
#     scripts/msix.ps1 -Bundle a.msix,b.msix -Out pktito.msixbundle
#
# DIR holds pktito-app.exe, pktito-cli.exe and WebView2Loader.dll (default:
# src-tauri's target/release, or target/aarch64-pc-windows-gnullvm/release
# for arm64). The version is Cargo.toml's with a fourth part of 0, which the
# Store keeps for itself.
#
# -Identity store uses the name and publisher Partner Center gives the app
# (Product identity), below; the Store signs that package itself. -Identity
# test (the default) is unsigned, marked so by Microsoft's OID for unsigned
# packages, for trying a package on this computer, from an administrator's
# PowerShell:  Add-AppxPackage -AllowUnsigned pktito_0.9.2.0_x64.msix
# -Identity sideload is the test package's name with our code signing
# certificate's subject as publisher, to be signed with it (sign-ours.bat)
# and installed by anyone, no administrator needed. Neither is the Store's
# package, so either may be installed beside it.
#
# -Bundle joins packages of each architecture into one .msixbundle, which
# the Store takes as a single upload.
param(
    [ValidateSet('x64', 'arm64')] [string]$Arch = 'x64',
    [string]$Exes,
    [string]$Out,
    [ValidateSet('store', 'test', 'sideload')] [string]$Identity = 'test',
    [string[]]$Bundle
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$tauri = Join-Path $root 'src-tauri'

# the Store's values, from Partner Center's Product identity page: the app
# reserved as Pktito by Peter Hebert. Name and Publisher are still to copy
# from there; until then, placeholders that make a package the Store would
# refuse. The Store wants the reserved name as the DisplayName.
$store = @{
    Name = 'TODO.Pktito'
    Publisher = 'CN=TODO'
    Display = 'Pktito'
    PublisherDisplay = 'Peter Hebert'
}
$test = @{
    Name = 'pktito.Test'
    Publisher = 'CN=pktito test, OID.2.25.311729368913984317654407730594956997722=1'
    Display = 'pktito (test)'
    PublisherDisplay = 'pktito (unsigned test)'
}

# makeappx from the newest Windows SDK here
$sdk = "${env:ProgramFiles(x86)}\Windows Kits\10\bin"
$makeappx = Get-ChildItem "$sdk\10.*\x64\makeappx.exe" | Sort-Object FullName | Select-Object -Last 1
if (-not $makeappx) { throw "no makeappx.exe under $sdk (the Windows SDK)" }

if ($Bundle) {
    if (-not $Out) { throw '-Bundle needs -Out' }
    $dir = Join-Path ([IO.Path]::GetTempPath()) "pktito-bundle-$PID"
    New-Item -ItemType Directory $dir | Out-Null
    try {
        $Bundle | ForEach-Object { Copy-Item $_ $dir }
        & $makeappx bundle /o /d $dir /p $Out
        if ($LASTEXITCODE) { throw "makeappx bundle failed ($LASTEXITCODE)" }
    } finally { Remove-Item -Recurse -Force $dir }
    return
}

$version = (Select-String -Path "$tauri\Cargo.toml" -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value + '.0'
if (-not $Exes) {
    $Exes = if ($Arch -eq 'arm64') { "$tauri\target\aarch64-pc-windows-gnullvm\release" } else { "$tauri\target\release" }
}
$sideload = @{
    Name = 'pktito.Test'
    Publisher = 'CN=Peter Hebert, O=Peter Hebert, L=Raleigh, S=nc, C=US'
    Display = 'pktito (test)'
    PublisherDisplay = 'pktito (test)'
}
$id = @{ store = $store; test = $test; sideload = $sideload }[$Identity]
if (-not $Out) { $Out = Join-Path $root "pktito_${version}_$Arch$(if ($Identity -ne 'store') { "-$Identity" }).msix" }

$layout = Join-Path ([IO.Path]::GetTempPath()) "pktito-msix-$PID"
New-Item -ItemType Directory "$layout\Assets" | Out-Null
try {
    foreach ($f in 'pktito-app.exe', 'pktito-cli.exe', 'WebView2Loader.dll') {
        if (-not (Test-Path "$Exes\$f")) { throw "no $f in $Exes" }
        Copy-Item "$Exes\$f" $layout
    }
    foreach ($f in 'Square44x44Logo.png', 'Square150x150Logo.png', 'StoreLogo.png') {
        Copy-Item "$tauri\icons\$f" "$layout\Assets"
    }
    Copy-Item "$tauri\icons\model\48x48.png" "$layout\Assets\model.png"
    # licenses beside the exes, as the setup program installs them
    New-Item -ItemType Directory "$layout\licenses" | Out-Null
    Copy-Item "$root\ui\fonts\Archivo-OFL.txt", "$root\ui\fonts\CommitMono-OFL.txt" "$layout\licenses"
    Copy-Item "$root\vendor\pktito\LICENSE" "$layout\licenses\pktito-LICENSE.txt"

    (Get-Content -Raw "$tauri\windows\msix\AppxManifest.xml") `
        -replace '@NAME@', $id.Name -replace '@PUBLISHER@', $id.Publisher `
        -replace '@PUBLISHER_DISPLAY@', $id.PublisherDisplay -replace '@DISPLAY@', $id.Display `
        -replace '@VERSION@', $version -replace '@ARCH@', $Arch |
        Set-Content -Encoding utf8 "$layout\AppxManifest.xml"

    & $makeappx pack /o /d $layout /p $Out
    if ($LASTEXITCODE) { throw "makeappx pack failed ($LASTEXITCODE)" }
} finally { Remove-Item -Recurse -Force $layout }
