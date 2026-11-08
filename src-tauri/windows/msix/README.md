# pktito in the Microsoft Store

The Store edition is the setup program's exes, unchanged, in an MSIX
package: `AppxManifest.xml` here, packed by `scripts/msix.ps1` from a
release's signed setup program. Nothing is compiled differently. The exes
ask Windows at run time whether they run from a package (`store_edition`
in main.rs, by `GetCurrentPackageFullName`) and where their data is
(`data_in_package`), and the page reads both from `status`.

## What packaging changes

|                          | Setup program                           | Store package                                                                                            |
| ------------------------ | --------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Installed to             | `%LOCALAPPDATA%\pktito`                 | `C:\Program Files\WindowsApps\…`, read-only                                                              |
| Settings, models, recent | `%APPDATA%\net.pktito.app`              | the package's own copy of that folder, under its `LocalCache`, unless the folder was there first (below) |
| Uninstalling             | leaves the data folder                  | deletes the package's own copy, models with it                                                           |
| `.pkf` as "pktito model" | registry, hooks.nsh                     | `windows.fileTypeAssociation`                                                                            |
| `pktito-cli` on the PATH | path.ps1 edits the user's PATH          | an execution alias, in a folder on every user's PATH (below)                                             |
| Updates                  | a new setup program                     | the Store                                                                                                |
| Signing                  | our certificate: the exes and the setup | the same signed exes; the Store signs the package                                                        |

### Where the data is

Windows redirects only a folder the package makes. Where the setup
program's copy made `%APPDATA%\net.pktito.app` first, the Store's copy
reads and writes that real folder: the same models, settings and recent
list as the other copy, and uninstalling it leaves them. (The signed test
package on the 9950X, 2026-10-26: its recent transcript went into the real
folder, and its `LocalCache` held nothing of pktito's.) So the app asks
where its data is, not only where it came from:

- `data_in_package`: the package's own copy is there, or no folder is there
  yet and the first write will make one in the package. Then the model menu
  has no Show in File Explorer, since Explorer outside the package can't
  show that folder, and Privacy says uninstalling deletes the data.
- Otherwise the menu and Privacy are the setup program's, which are right
  for a shared folder.
- `store`: About says "Microsoft Store" whenever the app runs from the
  package.

Open a model file… and a saved transcript's Show in File Explorer stay in
both cases: transcripts are saved in the user's own folders, which aren't
redirected. The site's privacy policy, which the listing links, says the
Store edition's data goes when it is uninstalled; it needs the shared case
too.

`runFullTrust` is the one restricted capability, which every desktop
program in a package declares; Partner Center asks for a line saying why
("a Win32 desktop app"). `unvirtualizedResources` would make the folder
always shared. It was tested and does that (2026-10-23), but the Store
often refuses it, and nothing here needs it.

### The command line

`pktito-cli` is an app execution alias: Windows puts it in
`%LOCALAPPDATA%\Microsoft\WindowsApps`, which is on every user's PATH, and
runs it inside the package, where it sees the window's models and
settings. Through it, a transcript of test.wav came out byte for byte the
setup program's CLI's (2026-10-26).

- The alias belongs to the package's one Application, and names
  pktito-cli.exe and a console subsystem. The Certification Kit fails a
  package of two Applications (below). makeappx refuses a console alias
  unless its Application allows many instances. With the window open, two
  transcripts at once came out the setup program's CLI's, and the window
  still opened once (0.9.2.4 on the 9950X, 2026-10-26).
- With both editions installed, the Store's alias comes first on the PATH.
- Over SSH (armlet, 2026-10-23), Windows refused the alias while the app
  ran in the desktop session. For SSH, services and other accounts, the
  setup program's CLI is the one to use.
- A user can turn the alias off: Settings, Apps, Advanced app settings, App
  execution aliases.

## The window's runtime

WebView2 is not something a package can install. Windows 11 has it, as does
Windows 10 updated through Edge. The manifest's minimum is Windows 10 1809.

## Two ways into the Store

1. **The MSIX (this):** the Store hosts, signs and updates it, and a user
   installs it with no prompt.
2. **The setup program as a Win32 listing:**
   - The Store links to our signed setup by a versioned URL (policy 10.2.9).
   - It must install silently (`/S`) and offline, and must not download the
     app.
   - The Store doesn't update it, so each release is a new submission. The
     app doesn't update itself.
   - Behavior is identical to the download from pktito.net.

The MSIX is the better fit: updates come through the Store, which the setup
program can't offer. Partner Center can't turn one kind of product into
the other, so the MSIX needs a product made as an "MSIX or PWA app". Once
listed, the app is in winget's `msstore` source too
(`winget install <Store ID> -s msstore`).

## Making a package

```powershell
# a release: the signed setup programs, a package of each, then the bundle
scripts\msix.ps1 -Setup pktito_0.9.2_x64-setup.exe -Identity store
scripts\msix.ps1 -Setup pktito_0.9.2_arm64-setup.exe -Identity store
scripts\msix.ps1 -Bundle pktito_0.9.2.0_x64.msix,pktito_0.9.2.0_arm64.msix

# a test, from a build (cargo tauri build --no-bundle, and pktito-cli)
scripts\msix.ps1
scripts\msix.ps1 -Exes src-tauri\target\aarch64-pc-windows-gnullvm\release
```

The Store and sideload identities take only signed exes. A test package
installs from an administrator's PowerShell:
`Add-AppxPackage -AllowUnsigned pktito_0.9.2.0_x64-test.msix`. A sideload
package, signed with the certificate that signs the setup program, installs
with no prompt. Either has its own identity, so it sits beside the setup
program's copy and beside a Store install. Installed again, a test package
needs a higher `-Version`, or Windows keeps the last one's icons.

## Before the first submission

- Partner Center: the product is an "MSIX or PWA app", Store ID
  9MW58B2T1ZVW. Its identity is msix.ps1's store identity (copied from
  Product identity, 2026-11-05), and its Display must be the reserved name,
  Pktito, exactly.
- The listing:
  - a description;
  - screenshots of at least 1366×768 (the site's standard shots, 682×832,
    are too small);
  - the privacy policy URL (pktito.net/privacy);
  - the age rating questionnaire;
  - the runFullTrust justification;
  - certification notes, saying the first run downloads a model of about
    650 MB from models.pktito.net.
- The Windows App Certification Kit on the bundle (it needs an
  administrator).
- Version: Cargo.toml's, with a fourth part of 0, which the Store keeps for
  itself.

## Tested

A sideload bundle, 0.9.2.3, packed from this branch's signed setup
programs (2026-10-26):

- **armlet, Arm64, as a fresh computer** (its own data folders set aside
  for the run):
  - the bundle installed its Arm64 package with no prompt;
  - with no data folder anywhere, the window offered the models to download
    and said `data_in_package`;
  - a model and settings written inside the package went to its own copy,
    and the real `%APPDATA%\net.pktito.app` stayed absent;
  - through the alias, `pktito-cli` saw that model in use and transcribed
    test.wav;
  - the window was ready, with no Show in File Explorer, and gave the
    Store's Privacy sentence;
  - uninstalling took the package, the alias and the package's data with
    the model, and left nothing in the real folders.
- **The 9950X, x64, with the setup program's folder already there:**
  - the package shared the folder;
  - `data_in_package` was false: the menu kept Show in File Explorer and
    Privacy gave the setup program's sentence;
  - with the package's own copy made for the run, both turned as on armlet;
  - through the alias, test.wav's transcript was byte for byte the setup
    program's CLI's.

The window was driven through WebView2's debugging port, from PowerShell
started in the package (`Invoke-CommandInDesktopPackage`), which passes
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` on to the app as the package's own
activation does not. Not yet run: Windows 10.

### The Certification Kit

Kit 10.0.26100.7705 on the 0.9.2.3 bundle's x64 package, from an
administrator's PowerShell on the 9950X (2026-10-26): 21 tests passed, two
optional ones failed and one gave a warning. Microsoft's page on the kit's
tests for desktop apps says the optional ones are "informational only and
will not be used to evaluate your app during Microsoft Store onboarding".

- **Application count** (optional): "The Microsoft Store does not support a
  package containing more than one applications defined in the manifest."
  0.9.2.3 gave the CLI an Application of its own. In 0.9.2.4 the alias is
  the one Application's (above).
- **Blocked executables** (optional) is a test for Windows in S mode, where
  starting programs is restricted:
  - `CreateProcessW` is how pktito starts ffmpeg, where the user has one,
    for a format it doesn't read itself (decode.rs). It starts nothing else
    on Windows.
  - `ShellExecuteW` opens a link in the browser.
  - `cmd.exe` is Rust's standard library's, for running a batch file, which
    pktito never does.
  - `REg` and `basH` are bytes of pktito-app.exe's machine code that spell
    them: an address's offset, and part of Tauri's `deny-basename`.
- **DPI awareness** (a warning): pktito-cli.exe declares none. It is a
  console program, with no window to scale; pktito-app.exe passed.

The 0.9.6.0 bundle passed the kit (2026-11-05). The one test it failed was
Blocked executables, the optional one 0.9.2.4 failed too.

## The icons

`Assets/` holds the taskbar and Start icon, `Square44x44Logo`, at each
target size Windows draws it, in the unplated and light-unplated forms.
Windows finds them by the package's resource index, `resources.pri`, which
msix.ps1 makes with makepri. Without the two, Windows scales the 44 px logo
onto a plate of the accent color, which shows round the icon's rounded
corners. They are `icons/icon.png` resized with Lanczos:

```sh
python3 -c "
from PIL import Image
src = Image.open('src-tauri/icons/icon.png').convert('RGBA')
for n in (16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 256):
    im = src.resize((n, n), Image.LANCZOS)
    for alt in ('altform-unplated', 'altform-lightunplated'):
        im.save(f'src-tauri/windows/msix/Assets/Square44x44Logo.targetsize-{n}_{alt}.png')
"
```
