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
wonderland_ch_01.flac 8ac53c4d56166846351d58f3f6a26e603f10e0b1ce6e7a3b22f106e0a754c4c7 420b1701aa22b14ae761476d9394889dbb32307becd4efd389cebfbffa923128
wonderland_ch_02.flac 5210f2d89843a94a42555228406ca75672ff1ebb4731c0d07b75c5125cc2f4dc 9b9c182116e0c1e302298cf35a6f95d568ee28ef37d8c6aa2500a4445816da05
wonderland_ch_03.flac 3a7bc0fd543b47daa9eccaef002be75716c5fd59cae9ea1390247b3eb1895808 502be4285a107012dfcd86373e6623aa01ba0f842f2b189c8c4e794b07fc8a29
wonderland_ch_04.flac b0cc8de92e8875d4ce85a3f647aa77e17faea65de6ca490d7fff756bba228e8a 2eb60b8d7451ffaf2edec1153bc251051f3b1348cc6e7fd3fef2b64ed0231354
wonderland_ch_05.flac dc2d1865e72aef0023da36bfdaedb824313611961306e2c409592636f7127ca6 3fbcb15dbb16a47449618d283e2e1de0b96477ee32279fd3b179e33b535a0643
wonderland_ch_06.flac 56248ff5002a7d20fd149bc1ccc2f2117cc330d2940d2fb79f707a28f8e18347 f0170d1b5162a1a7efb6492e991eb17782753d8785357ab2bb9fc0f0fe93e1bb
wonderland_ch_07.flac a75e21ed3ad05f9b2d12cb4208577ae8e4f8872892ec4478446542ab1b7009a9 4c5aff13c3eafa3c080d1ca38bf48ed02ff474989c94ef0ad78a0e1b5cb9d9a4
wonderland_ch_08.flac c89cd244315f2ffa5cbcf6509891d7e510d30092e9bfaf69d4d09bb803f3970f d0f40e98ee30f34a4e1efb66f7608c4b74199d4f50e25ada12f3c0fde6fb1548
wonderland_ch_09.flac d8c0a03d1d3482d26244484e9c4a5361bb50e80cf2e1b3b11939fe16c164bb14 8d4180ce60b8c69b368a29953adf7fb5bd6f8b0972ec7c8498ab5717b786b8b5
wonderland_ch_10.flac 721f1aa64865cb18082a88c66ff542cdcdd0628f794c4b35fbbc839a837858bc a480673ea85f2728d719c92eddecacce14757b7c073aaa95f946714734ff3ac3
wonderland_ch_11.flac 09d0a325f913069b53ed23f02dd8f17f76027518c0a0b89f1e14b10f5ec13d69 a770e46c49181c3511d2863201cb3507b9ff6dc149576fcf5dbd5cd43658c7fc
wonderland_ch_12.flac c7a4a9d9cd8fd1aae1d4abbc05289920074dfa9f845650e4a5773e959c997852 9a2d63b4ff85f2df4ed5daa9f01853df7e3f692259b02b01a223f80ae49c2624
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
'AliceBench v1: {0:F1}x real time (pktito-cli {1}, {2}, {3}, {4} threads)' -f $r.x_real_time, $r.pktito_cli, $r.model, $r.backend, $r.threads
if (@($r.files).Count -ne 12) { "  only $(@($r.files).Count) of the 12 chapters were timed"; exit 1 }
if ($differ) { "  $differ of the 12 transcripts differ from the reference: not what pktito should compute"; exit 1 }
'  all 12 transcripts are the reference''s'
