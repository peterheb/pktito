# The Windows setup program for one architecture, from any checkout, a fresh
# clone included: the README's "Windows builds" steps in order.
#
#     scripts/build-windows.ps1 [-Arch x64|arm64] [-QnnSdk DIR] [-Config FILE ...]
#
# Needs rustup's stable-x86_64-pc-windows-gnullvm toolchain (with the
# aarch64-pc-windows-gnullvm target for arm64), llvm-mingw's bin on PATH and
# the Tauri CLI (cargo install tauri-cli). -QnnSdk (default $env:QNN_SDK)
# puts the NPU's QNN files in an arm64 build (qnn-windows.ps1); without it
# the arm64 build is CPU only. -Config adds Tauri config files, such as a
# signing command's. The setup program lands in
# src-tauri\target\<triple>\release\bundle\nsis.
#
# tauri-bundler 2.10.4 adds WebView2Loader.dll to an installer only for
# targets ending in -gnu, so the build names it as a resource, and
# tauri-build checks resources before it copies the DLL into the target's
# folder: in a fresh folder the first build stopped on the DLL not being
# there (a fresh clone, 2026-11-05). So it is copied in first, from the
# webview2-com-sys that Cargo.lock names, which carries Microsoft's loader
# for each architecture.
param(
    [ValidateSet('x64', 'arm64')] [string]$Arch = 'x64',
    [string]$QnnSdk = $env:QNN_SDK,
    [string[]]$Config = @()
)
$ErrorActionPreference = 'Stop'

$triple = @{ x64 = 'x86_64-pc-windows-gnullvm'; arm64 = 'aarch64-pc-windows-gnullvm' }[$Arch]
$Config = @($Config | ForEach-Object { (Resolve-Path $_).Path })
$tauri = Join-Path $PSScriptRoot '..\src-tauri' | Resolve-Path
Set-Location $tauri
$env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-gnullvm'

$meta = cargo metadata --format-version 1 --locked | ConvertFrom-Json
if ($LASTEXITCODE) { throw 'cargo metadata failed' }
$sys = @($meta.packages | Where-Object name -eq 'webview2-com-sys')[0]
if (-not $sys) { throw 'Cargo.lock has no webview2-com-sys' }
$release = "target\$triple\release"
New-Item -ItemType Directory -Force $release | Out-Null
Copy-Item (Join-Path (Split-Path $sys.manifest_path) "$Arch\WebView2Loader.dll") $release

# a file, not --config's inline JSON, whose quotes Windows PowerShell 5.1
# drops on the way to a native program
$loader = "target\$triple\webview2-loader.conf.json"
Set-Content -Encoding ascii $loader ('{"bundle":{"resources":{"' + "target/$triple/release/WebView2Loader.dll" + '":"WebView2Loader.dll"}}}')
$configs = @($loader)
if ($Arch -eq 'arm64' -and $QnnSdk) {
    & (Join-Path $PSScriptRoot 'qnn-windows.ps1') -Sdk $QnnSdk
    $env:QNN_SDK = $QnnSdk
    $configs += 'windows\qnn.conf.json'
}
$configs += $Config

$cargoArgs = @('tauri', 'build', '--target', $triple, '--bundles', 'nsis')
foreach ($c in $configs) { $cargoArgs += '--config', $c }
cargo @cargoArgs
exit $LASTEXITCODE
