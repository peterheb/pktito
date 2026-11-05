# pktito as an MSIX package, for the Microsoft Store: the setup program's
# exes, unchanged, with src-tauri/windows/msix/AppxManifest.xml.
#
#     scripts/msix.ps1 -Setup FILE [-Identity store|test|sideload] [-Out FILE] [-Version V]
#     scripts/msix.ps1 [-Exes DIR] [-Identity ...] [-Out FILE] [-Version V]
#     scripts/msix.ps1 -Bundle a.msix,b.msix [-Out FILE]
#
# -Setup is a release's signed setup program (pktito_0.9.2_x64-setup.exe):
# 7-Zip takes pktito-app.exe, pktito-cli.exe and WebView2Loader.dll out of
# it, so the Store's package holds the very files the download installs.
# -Exes is a folder holding the three instead (default: a build's,
# src-tauri\target\release), for a test. The package's architecture is
# pktito-app.exe's. An Arm64 setup built with Qualcomm's QNN libraries
# (scripts/qnn-windows.ps1) has them beside the exes, and their notice
# files among the licenses: they go into the package as they are, all of
# them or none.
#
# -Identity:
# - store: the name and publisher Partner Center gives the app (Product
#   identity), below. The Store signs the package itself; the exes must
#   carry our signature already, as every file of our code does.
# - test (the default): unsigned, marked so by Microsoft's OID for unsigned
#   packages. An administrator's PowerShell installs it:
#       Add-AppxPackage -AllowUnsigned pktito_0.9.2.0_x64-test.msix
# - sideload: published by our code signing certificate's subject, to be
#   signed with that certificate, after which it installs without an
#   administrator. The exes must be signed, as for store.
# Neither test identity is the Store's package, so either installs beside
# it, and beside the setup program's copy.
#
# The version is Cargo.toml's with a fourth part of 0, which the Store keeps
# for itself; -Version sets another, as a test package's must rise for
# Windows to install it over the last and draw its icons afresh.
#
# -Bundle joins one package of each architecture into a .msixbundle, which
# the Store takes as a single upload, with the packages' version.
param(
    [string]$Setup,
    [string]$Exes,
    [string]$Out,
    [ValidateSet('store', 'test', 'sideload')] [string]$Identity = 'test',
    [string[]]$Bundle,
    [string]$Version
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$tauri = Join-Path $root 'src-tauri'

# the Store's values, from Partner Center's Product identity page (Store ID
# 9MW58B2T1ZVW, package family PeterHebert.Pktito_ff8200zn03b0w). The Store
# wants the reserved name as the DisplayName.
$identities = @{
    store = @{
        Name = 'PeterHebert.Pktito'
        Publisher = 'CN=888FC3C8-DF5F-4348-8575-767E3FF54C42'
        Display = 'Pktito'
        PublisherDisplay = 'Peter Hebert'
    }
    test = @{
        Name = 'pktito.Test'
        Publisher = 'CN=pktito test, OID.2.25.311729368913984317654407730594956997722=1'
        Display = 'pktito (test)'
        PublisherDisplay = 'pktito (unsigned test)'
    }
    sideload = @{
        Name = 'pktito.Test'
        Publisher = 'CN=Peter Hebert, O=Peter Hebert, L=Raleigh, S=nc, C=US'
        Display = 'pktito (test)'
        PublisherDisplay = 'pktito (test)'
    }
}

# makeappx and makepri from the newest Windows SDK here
$sdk = "${env:ProgramFiles(x86)}\Windows Kits\10\bin"
$makeappx = Get-ChildItem "$sdk\10.*\x64\makeappx.exe" | Sort-Object FullName | Select-Object -Last 1
if (-not $makeappx) { throw "no makeappx.exe under $sdk (the Windows SDK)" }
$makepri = Join-Path $makeappx.DirectoryName 'makepri.exe'

$work = Join-Path ([IO.Path]::GetTempPath()) "pktito-msix-$PID"
New-Item -ItemType Directory $work | Out-Null
try {
    if ($Bundle) {
        # the bundle's version is its packages', which must agree
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $versions = foreach ($p in $Bundle) {
            $zip = [IO.Compression.ZipFile]::OpenRead((Resolve-Path $p))
            try {
                $r = New-Object IO.StreamReader $zip.GetEntry('AppxManifest.xml').Open()
                ([xml]$r.ReadToEnd()).Package.Identity.Version
                $r.Dispose()
            } finally { $zip.Dispose() }
        }
        $v = @($versions | Sort-Object -Unique)
        if ($v.Count -ne 1) { throw "the packages' versions differ: $($v -join ', ')" }
        if (-not $Out) { $Out = Join-Path $root "pktito_$($v[0]).msixbundle" }
        $Bundle | ForEach-Object { Copy-Item $_ $work }
        & $makeappx bundle /o /bv $v[0] /d $work /p $Out
        if ($LASTEXITCODE) { throw "makeappx bundle failed ($LASTEXITCODE)" }
        return
    }

    $files = 'pktito-app.exe', 'pktito-cli.exe', 'WebView2Loader.dll'
    # the NPU's, where the build has them (src-tauri/windows/qnn.conf.json)
    $npu = 'QnnHtp.dll', 'QnnSystem.dll', 'QnnHtpV73Stub.dll', 'libQnnHtpV73Skel.so', 'libqnnhtpv73.cat',
        'licenses\QNN_NOTICE.txt', 'licenses\QNN_NOTICE_WINDOWS.txt', 'licenses\QNN_VERSION.txt'
    if ($Setup) {
        $7z = (Get-Command 7z -EA SilentlyContinue).Source
        if (-not $7z) { $7z = "$env:ProgramFiles\7-Zip\7z.exe" }
        if (-not (Test-Path $7z)) { throw 'no 7z.exe (7-Zip), which takes the exes out of the setup program' }
        $Exes = "$work\setup"
        & $7z x -y "-o$Exes" $Setup @files | Out-Null
        if ($LASTEXITCODE) { throw "7-Zip could not read $Setup ($LASTEXITCODE)" }
        & $7z x -y "-o$Exes" $Setup @npu | Out-Null
    } elseif (-not $Exes) {
        $Exes = "$tauri\target\release"
    }
    foreach ($f in $files) {
        if (-not (Test-Path "$Exes\$f")) { throw "no $f in $Exes" }
    }
    $here = @($npu | Where-Object { Test-Path "$Exes\$_" })
    if ($here.Count -and $here.Count -ne $npu.Count) {
        throw "$Exes has some of the NPU's files and not all: $($npu | Where-Object { $_ -notin $here })"
    }
    if ($Identity -ne 'test') {
        foreach ($f in 'pktito-app.exe', 'pktito-cli.exe') {
            $s = Get-AuthenticodeSignature "$Exes\$f"
            if ($s.Status -ne 'Valid') { throw "$f is not signed ($($s.Status)): -Identity $Identity takes the release's signed exes" }
        }
    }

    # the architecture pktito-app.exe is built for: its PE header's machine
    $pe = [IO.File]::ReadAllBytes("$Exes\pktito-app.exe")
    $machine = [BitConverter]::ToUInt16($pe, [BitConverter]::ToInt32($pe, 0x3c) + 4)
    $arch = @{ 0x8664 = 'x64'; 0xaa64 = 'arm64' }[[int]$machine]
    if (-not $arch) { throw ('pktito-app.exe is for machine 0x{0:x4}, neither x64 nor arm64' -f $machine) }

    $version = if ($Version) { $Version } else {
        (Select-String -Path "$tauri\Cargo.toml" -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value + '.0'
    }
    $id = $identities[$Identity]
    if (-not $Out) { $Out = Join-Path $root "pktito_${version}_$arch$(if ($Identity -ne 'store') { "-$Identity" }).msix" }

    $layout = "$work\layout"
    New-Item -ItemType Directory "$layout\Assets", "$layout\licenses" | Out-Null
    $files | ForEach-Object { Copy-Item "$Exes\$_" $layout }
    foreach ($f in 'Square44x44Logo.png', 'Square150x150Logo.png', 'StoreLogo.png') {
        Copy-Item "$tauri\icons\$f" "$layout\Assets"
    }
    Copy-Item "$tauri\icons\model\48x48.png" "$layout\Assets\model.png"
    # the taskbar's and Start's icon at each size they draw it, unplated:
    # with only Square44x44Logo, Windows scales it onto a plate of the
    # accent color, whose edge showed round the icon's corners (the
    # 2026-10-26 test package, in the taskbar)
    Copy-Item "$tauri\windows\msix\Assets\*.png" "$layout\Assets"
    # licenses beside the exes, as the setup program installs them
    Copy-Item "$root\ui\fonts\Archivo-OFL.txt", "$root\ui\fonts\CommitMono-OFL.txt" "$layout\licenses"
    Copy-Item "$root\vendor\pktito\LICENSE" "$layout\licenses\pktito-LICENSE.txt"
    $here | ForEach-Object { Copy-Item "$Exes\$_" "$layout\$_" }

    (Get-Content -Raw "$tauri\windows\msix\AppxManifest.xml") `
        -replace '@NAME@', $id.Name -replace '@PUBLISHER@', $id.Publisher `
        -replace '@PUBLISHER_DISPLAY@', $id.PublisherDisplay -replace '@DISPLAY@', $id.Display `
        -replace '@VERSION@', $version -replace '@ARCH@', $arch |
        Set-Content -Encoding utf8 "$layout\AppxManifest.xml"

    # the resource index, by which Windows finds the icons' sized forms
    # (Assets\*.targetsize-*); without it only the plain file names count,
    # and the fringe stayed
    & $makepri createconfig /cf "$work\priconfig.xml" /dq en-US /o | Out-Null
    & $makepri new /pr $layout /cf "$work\priconfig.xml" /of "$layout\resources.pri" /o | Out-Null
    if ($LASTEXITCODE) { throw "makepri failed ($LASTEXITCODE)" }

    & $makeappx pack /o /d $layout /p $Out
    if ($LASTEXITCODE) { throw "makeappx pack failed ($LASTEXITCODE)" }
} finally { Remove-Item -Recurse -Force $work }
