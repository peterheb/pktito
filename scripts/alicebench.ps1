# AliceBench v1 on Windows: scripts/alicebench.sh's, which says what it is
# and how its chapters were made; the two keep the same list.
#
#     powershell -ExecutionPolicy Bypass -File alicebench.ps1 [DIR]
#
# DIR keeps the chapters (default %LOCALAPPDATA%\alicebench-v1); PKTITO_CLI
# names the pktito-cli to time, else the one on the PATH (the app's installer
# puts it there).
param([string]$Dir = (Join-Path $env:LOCALAPPDATA 'alicebench-v1'))
$ErrorActionPreference = 'Stop'

$base = 'https://models.pktito.net/alicebench/v1'
$chapters = @'
wonderland_ch_01.flac ad3326f63b5371090294c38a80467dcd288bc309b9def8547c550429844b6a31 420b1701aa22b14ae761476d9394889dbb32307becd4efd389cebfbffa923128
wonderland_ch_02.flac 7507934bbf636c7ef5a3089eba6f5aec3848528ac1150b987ad80840472c3f97 9b9c182116e0c1e302298cf35a6f95d568ee28ef37d8c6aa2500a4445816da05
wonderland_ch_03.flac 919e8f75289091fce57dde9c0e1947132ad42e88095b8921696d0a3fccd0e50d 502be4285a107012dfcd86373e6623aa01ba0f842f2b189c8c4e794b07fc8a29
wonderland_ch_04.flac f575db797db9a0db3af94f1a560b191fcd485e9cc212c19da4ebdf9f2af8e20e 2eb60b8d7451ffaf2edec1153bc251051f3b1348cc6e7fd3fef2b64ed0231354
wonderland_ch_05.flac 57d3ec5801fd93782f8d5e6f193a45f11f06ffb5e8358c88b9aecf5bbb2780b5 3fbcb15dbb16a47449618d283e2e1de0b96477ee32279fd3b179e33b535a0643
wonderland_ch_06.flac cffb9569b7e875e2588bcd92c0fcc38bdb15be232595d54a57c25ca5fcc6133f f0170d1b5162a1a7efb6492e991eb17782753d8785357ab2bb9fc0f0fe93e1bb
wonderland_ch_07.flac 59fbba17032e80ae188837a4f77b399fa97817597bf57925952f28e3cd7c731a 4c5aff13c3eafa3c080d1ca38bf48ed02ff474989c94ef0ad78a0e1b5cb9d9a4
wonderland_ch_08.flac e47086d07d2f7e73488550531b2eb4c7e20c7761e806a68f0f544a5dc8d51d5a d0f40e98ee30f34a4e1efb66f7608c4b74199d4f50e25ada12f3c0fde6fb1548
wonderland_ch_09.flac a0ec98e0bcb2cc84502faaffdcede67d309f27cc846d8ec18b00160c4c322f0e 8d4180ce60b8c69b368a29953adf7fb5bd6f8b0972ec7c8498ab5717b786b8b5
wonderland_ch_10.flac 463975d3aaedb928618ef1b16bb4ede8640c4a75d0d92b209fcc9c8bd6832aa5 a480673ea85f2728d719c92eddecacce14757b7c073aaa95f946714734ff3ac3
wonderland_ch_11.flac 6ef258f362737f4976e06713624dde9aff46a8a72728a5ecf2c70ef907384f59 a770e46c49181c3511d2863201cb3507b9ff6dc149576fcf5dbd5cd43658c7fc
wonderland_ch_12.flac 922986f0c8f45fe6465daf990b832d664f24b8fdc159c45082ea449abac64ff9 9a2d63b4ff85f2df4ed5daa9f01853df7e3f692259b02b01a223f80ae49c2624
'@ -split "`r?`n" | Where-Object { $_ } | ForEach-Object {
    $name, $file, $transcript = $_ -split ' '
    [pscustomobject]@{ Name = $name; File = $file; Transcript = $transcript }
}

$cli = $env:PKTITO_CLI
if (-not $cli) { $cli = (Get-Command pktito-cli -ErrorAction SilentlyContinue).Source }
if (-not $cli) { Write-Error 'alicebench: no pktito-cli; set PKTITO_CLI to one' }

New-Item -ItemType Directory -Force $Dir | Out-Null
foreach ($c in $chapters) {
    $f = Join-Path $Dir $c.Name
    if ((Test-Path $f) -and (Get-FileHash $f -Algorithm SHA256).Hash -eq $c.File) { continue }
    Write-Host "alicebench: downloading $($c.Name)"
    $ProgressPreference = 'SilentlyContinue'    # Invoke-WebRequest's bar slows it many times over
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$($c.Name)" -OutFile "$f.part"
    if ((Get-FileHash "$f.part" -Algorithm SHA256).Hash -ne $c.File) { Write-Error "alicebench: $($c.Name) isn't AliceBench v1's" }
    Move-Item -Force "$f.part" $f
}

& $cli download parakeet-tdt-0.6b-v2 | Out-Null
if ($LASTEXITCODE) { exit 1 }
$files = $chapters | ForEach-Object { Join-Path $Dir $_.Name }
$json = & $cli bench -m parakeet-tdt-0.6b-v2 --json @files
$json | Set-Content (Join-Path $Dir 'result.json')
$r = $json | Out-String | ConvertFrom-Json

$differ = 0
foreach ($f in $r.files) {
    $ref = ($chapters | Where-Object Name -eq $f.file).Transcript
    $ok = if ($f.transcript_sha256 -eq $ref) { '' } else { $differ++; '  transcript differs' }
    '{0,7:F1}x  {1}{2}' -f $f.x_real_time, $f.file, $ok
}
# the compiler, as alicebench.sh gives it, when pktito-cli names one
$compiler = if ($r.compiler) { ", $($r.compiler)" } else { '' }
'AliceBench v1: {0:F1}x real time (pktito-cli {1}, {2}, {3}, {4} threads{5})' -f $r.x_real_time, $r.pktito_cli, $r.model, $r.backend, $r.threads, $compiler
if (@($r.files).Count -ne 12) { "  only $(@($r.files).Count) of the 12 chapters were timed"; exit 1 }
if ($differ) { "  $differ of the 12 transcripts differ from the reference: not what pktito should compute"; exit 1 }
'  all 12 transcripts are the reference''s'
