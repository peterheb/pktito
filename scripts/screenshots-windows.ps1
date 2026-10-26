# The site's standard screenshot, Windows' two: the window at its first
# size, LibriVox's Alice's Adventures in Wonderland, chapter 1, transcribed
# by the multilingual model and scrolled past the recording's LibriVox
# notice, so that "Alice's Adventures in Wonderland by Lewis Carroll." is
# the first row; once in light and once in dark, as Windows draws the
# window: its rounded corners and border, on the site's own page color.
#
#     scripts/screenshots-windows.ps1 [-App EXE] [-Out DIR]
#
# writes DIR\windows-light.png and DIR\windows-dark.png (default
# design\screenshots; pktito-site reads these names). EXE is the release
# build's pktito-app.exe (default src-tauri\target\release). The choices
# are the Mac's (scripts/screenshots-mac.sh, design/README.md "Imagery"):
# the multilingual model, the MP3 as LibriVox publishes it (Media
# Foundation gives the Mac's rows once MP3s are cut to where their sound
# starts), and a scroll of three one-line rows, 43 px each.
#
# How it differs from the Mac's:
# - The scroll goes through WebView2's debugging port, open on 127.0.0.1
#   for this run only: the window takes no injected mouse input (the 0.9.2
#   test, 2026-10-24).
# - The capture is the screen inside the window's frame, with a window of
#   the site's page color behind it (tokens.css --ground: #f4f6f9 light,
#   #0b1220 dark), so the corners Windows rounds off show the page, not
#   the desktop.
# - The display should be at 100% scaling, as the 9950X's is, so a row is
#   43 px; at more, the shot is bigger and the scroll still three rows.
#
# It quits pktito (every copy: one already open would be handed the file),
# sets its settings for the shot (the model, no saved window size, and
# playback, the view and the times at their defaults: 1x, full volume,
# sentences, seconds) and puts them back after, and puts the app theme
# (AppsUseLightTheme) back as it was.
param(
    [string]$App,
    [string]$Out
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
if (-not $App) { $App = "$root\src-tauri\target\release\pktito-app.exe" }
if (-not $Out) { $Out = "$root\design\screenshots" }
$data = "$env:APPDATA\net.pktito.app"
$SCROLL = 129
$PORT = 9233
$ground = @{ light = '#f4f6f9'; dark = '#0b1220' }
$mp3Url = 'https://archive.org/download/alice_in_wonderland_librivox/wonderland_ch_01_64kb.mp3'
$mp3Sha = 'fca40613cd198eb2b3e3b36e087b6cbaf22be8d143951d744670973a3c26d0d1'
$cache = "$env:LOCALAPPDATA\pktito-screenshots"
$mp3 = "$cache\wonderland_ch_01_64kb.mp3"

if (-not (Test-Path $App)) { throw "no ${App}: cargo tauri build --no-bundle first, or give -App" }
if (-not (Test-Path "$data\models\parakeet-tdt-0.6b-v3.pkf")) { throw "the multilingual model isn't downloaded: pktito-cli download v3" }
New-Item -ItemType Directory -Force $Out, $cache | Out-Null
if (-not (Test-Path $mp3) -or (Get-FileHash $mp3).Hash -ne $mp3Sha) {
    Invoke-WebRequest -UseBasicParsing $mp3Url -OutFile "$mp3.part"
    if ((Get-FileHash "$mp3.part").Hash -ne $mp3Sha) { throw "the chapter isn't LibriVox's" }
    Move-Item -Force "$mp3.part" $mp3
}

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class Shot {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint f);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr SendMessageTimeout(IntPtr h, uint m, IntPtr w, string l, uint f, uint t, out IntPtr r);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out R r, int n);
  public struct R { public int L, T, Ri, B; }
  // the window as drawn, without the invisible resize border: DWMWA_EXTENDED_FRAME_BOUNDS
  public static R Frame(IntPtr h) { R r; DwmGetWindowAttribute(h, 9, out r, 16); return r; }
  // above every window not topmost, or back among them
  public static void Top(IntPtr h, bool on) { SetWindowPos(h, new IntPtr(on ? -1 : -2), 0, 0, 0, 0, 0x13); }
  // tells running programs the app theme changed, as Settings does
  public static void ThemeChanged() { IntPtr r; SendMessageTimeout(new IntPtr(0xffff), 0x1a, IntPtr.Zero, "ImmersiveColorSet", 2, 3000, out r); }
}
"@
[Shot]::SetProcessDPIAware() | Out-Null

$personalize = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'
$themeWas = (Get-ItemProperty $personalize -EA SilentlyContinue).AppsUseLightTheme
$settingsWas = [IO.File]::ReadAllBytes("$data\settings.json")
$utf8 = New-Object Text.UTF8Encoding $false

function Quit-App {
    Get-Process pktito-app -EA SilentlyContinue | Stop-Process -Force
    for ($i = 0; $i -lt 50 -and (Get-Process pktito-app -EA SilentlyContinue); $i++) { Start-Sleep -Milliseconds 100 }
}

# one JavaScript expression in the window, by the debugging port; its value
function Evaluate([string]$expr) {
    $page = (Invoke-RestMethod "http://127.0.0.1:$PORT/json") | Where-Object type -eq 'page' | Select-Object -First 1
    $ws = New-Object Net.WebSockets.ClientWebSocket
    $none = [Threading.CancellationToken]::None
    $ws.ConnectAsync([Uri]$page.webSocketDebuggerUrl, $none).Wait()
    $msg = @{ id = 1; method = 'Runtime.evaluate'; params = @{ expression = $expr; returnByValue = $true } } | ConvertTo-Json -Compress -Depth 5
    $bytes = $utf8.GetBytes($msg)
    $ws.SendAsync((New-Object 'ArraySegment[byte]' -ArgumentList (, $bytes)), 'Text', $true, $none).Wait()
    $buf = New-Object byte[] 65536; $text = ''
    do {
        $r = $ws.ReceiveAsync((New-Object 'ArraySegment[byte]' -ArgumentList (, $buf)), $none).Result
        $text += $utf8.GetString($buf, 0, $r.Count)
    } until ($r.EndOfMessage)
    $ws.Dispose()
    ($text | ConvertFrom-Json).result.result.value
}

$backdrop = $null
try {
    Quit-App
    $s = [IO.File]::ReadAllText("$data\settings.json") | ConvertFrom-Json
    $s.model = 'parakeet-tdt-0.6b-v3'
    $s.model_file = $null
    foreach ($k in @{ speed = 1.0; volume = 1.0; captions = $false; frames = $false }.GetEnumerator()) {
        $s.prefs | Add-Member -Force -NotePropertyName $k.Key -NotePropertyValue $k.Value
    }
    $s.PSObject.Properties.Remove('window')
    [IO.File]::WriteAllText("$data\settings.json", ($s | ConvertTo-Json -Depth 10), $utf8)

    $started = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $argsWas = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$PORT"
    $proc = Start-Process $App -ArgumentList "`"$mp3`"" -PassThru
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $argsWas
    # done once the recent list's newest is this chapter, finished since
    $done = $false
    for ($i = 0; $i -lt 120 -and -not $done; $i++) {
        Start-Sleep 1
        $items = @(Get-Content -Raw "$data\recent\index.json" -EA SilentlyContinue | ConvertFrom-Json)
        $done = $items.Count -and $items[0].name -eq 'wonderland_ch_01_64kb.mp3' -and $items[0].finished -ge $started
    }
    if (-not $done) { throw 'the chapter was not transcribed within 2 minutes' }
    Start-Sleep 1

    # scroll the transcript, the scrolling box under the window's lower middle
    $top = Evaluate @"
(() => {
  let e = document.elementFromPoint(innerWidth / 2, innerHeight * 0.7);
  while (e && !(e.scrollHeight > e.clientHeight && /auto|scroll/.test(getComputedStyle(e).overflowY))) e = e.parentElement;
  if (!e) return -1;
  e.scrollTop += $SCROLL;
  return e.scrollTop;
})()
"@
    if ($top -ne $SCROLL) { throw "the transcript scrolled to $top, not $SCROLL" }

    $h = (Get-Process -Id $proc.Id).MainWindowHandle
    $f = [Shot]::Frame($h)
    $w = $f.Ri - $f.L; $ht = $f.B - $f.T
    $backdrop = New-Object Windows.Forms.Form -Property @{
        FormBorderStyle = 'None'; ShowInTaskbar = $false; StartPosition = 'Manual'
        Bounds = New-Object Drawing.Rectangle ($f.L - 48), ($f.T - 48), ($w + 96), ($ht + 96)
    }
    $backdrop.Show()
    foreach ($look in 'light', 'dark') {
        Set-ItemProperty $personalize AppsUseLightTheme ([int]($look -eq 'light'))
        [Shot]::ThemeChanged()
        $backdrop.BackColor = [Drawing.ColorTranslator]::FromHtml($ground[$look])
        [Shot]::Top($backdrop.Handle, $true); [Shot]::Top($h, $true)
        [Shot]::SetForegroundWindow($h) | Out-Null
        # the theme's change, and the scrollbar's fading
        for ($i = 0; $i -lt 20; $i++) { [Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 100 }
        $bm = New-Object Drawing.Bitmap $w, $ht
        $g = [Drawing.Graphics]::FromImage($bm)
        $g.CopyFromScreen($f.L, $f.T, 0, 0, $bm.Size)
        $bm.Save("$Out\windows-$look.png", [Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose(); $bm.Dispose()
        "$Out\windows-$look.png"
    }
} finally {
    if ($backdrop) { $backdrop.Close() }
    Quit-App
    [IO.File]::WriteAllBytes("$data\settings.json", $settingsWas)
    if ($null -eq $themeWas) { Remove-ItemProperty $personalize AppsUseLightTheme -EA SilentlyContinue }
    else { Set-ItemProperty $personalize AppsUseLightTheme $themeWas }
    [Shot]::ThemeChanged()
}
