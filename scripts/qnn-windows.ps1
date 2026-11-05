# Qualcomm's QNN libraries for the Windows on Arm build, where its encoder
# runs on a Snapdragon X's NPU (the qnn fork's core/qnn.c, build.rs with
# QNN_SDK): copied out of the SDK into src-tauri\target\qnn, git-ignored
# like the target folder, for src-tauri\windows\qnn.conf.json, which puts
# them beside pktito-app.exe in the setup program.
#
#     scripts/qnn-windows.ps1 [-Sdk DIR]
#
# DIR is QAIRT's SDK (default $env:QNN_SDK), 2.51.0 for the catalog's
# graphs (model.rs, Graph). Then, with QNN_SDK set to the same DIR:
#
#     cargo tauri build --target aarch64-pc-windows-gnullvm --config windows\qnn.conf.json
#
# The DLLs are QNN's HTP backend, QnnSystem, which it loads to read a
# compiled graph, and the stub for each Hexagon the catalog has a graph
# for on Windows (73, the X1E80100's; 81, the X2 Elite's), with that
# Hexagon's skel and its catalog file (.cat), which the NPU's driver checks
# the skel against: snappy's Snapdragon X Elite ran the catalog's graph
# with V73's alone beside the exe, from an installed MSIX too (2026-11-05);
# no X2 has run V81's yet. Not the compiler, QnnHtpPrepare.dll, as the
# graphs come compiled on a PC. With them go the SDK's notice files for the open-source
# code in them, as the SDK has them, and its version, for About's licenses:
# QTI's license (its "Third Party Notices") leaves those licenses to the
# app, and the files unaltered. QNN_NOTICE.txt is the one the Android app
# ships; NOTICE_WINDOWS.txt is the SDK's for its Windows builds.
param([string]$Sdk = $env:QNN_SDK)
$ErrorActionPreference = 'Stop'
if (-not $Sdk) { throw 'qnn-windows.ps1 -Sdk DIR, or QNN_SDK: QAIRT''s SDK' }
$out = Join-Path $PSScriptRoot '..\src-tauri\target\qnn'
Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $out | Out-Null
$lib = Join-Path $Sdk 'lib\aarch64-windows-msvc'
Copy-Item "$lib\QnnHtp.dll", "$lib\QnnSystem.dll" $out
foreach ($v in 73, 81) {
    Copy-Item "$lib\QnnHtpV${v}Stub.dll" $out
    Copy-Item "$Sdk\lib\hexagon-v$v\unsigned\libQnnHtpV${v}Skel.so", "$Sdk\lib\hexagon-v$v\unsigned\libqnnhtpv$v.cat" $out
}
Copy-Item "$Sdk\QNN_NOTICE.txt", "$Sdk\NOTICE_WINDOWS.txt" $out
$version = (Select-String -Path "$Sdk\sdk.yaml" -Pattern '^version:\s*(.+)$').Matches[0].Groups[1].Value.Trim()
Set-Content -NoNewline -Encoding ascii "$out\VERSION" $version
Get-ChildItem $out | ForEach-Object { '{0,10}  {1}' -f $_.Length, $_.Name }
