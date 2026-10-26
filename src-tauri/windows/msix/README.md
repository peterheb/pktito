# pktito in the Microsoft Store

The Store edition is the setup program's two exes, unchanged, in an MSIX
package (`AppxManifest.xml` here, packed by `scripts/msix.ps1`). The exes
tell they are packaged at run time (`store_edition` in main.rs, which asks
`GetCurrentPackageFullName`), and the page reads that as `status.store`.
Nothing is compiled differently.

## What packaging changes

| | Setup program | Store package |
|---|---|---|
| Installed to | `%LOCALAPPDATA%\pktito` | `C:\Program Files\WindowsApps\…`, read-only |
| Settings, models, recent | `%APPDATA%\net.pktito.app` | the same path, which Windows redirects into the package's own `LocalCache` |
| Uninstalling | leaves the data folder | deletes it, models with it |
| `.pkf` as "pktito model" | registry, hooks.nsh | `windows.fileTypeAssociation` |
| `pktito-cli` on the PATH | path.ps1 edits the user's PATH | an execution alias, which Windows puts on the PATH |
| Updates | a new setup program | the Store |
| Signing | our certificate, every exe and the setup | the Store signs the package |

What the app does about it (f264534):
- The redirected folder can't be shown in Explorer from outside the package,
  so the model menu has no Show in File Explorer. Open a model file… stays,
  and so does a saved transcript's Show in File Explorer: transcripts are
  saved in the user's own folders, which aren't redirected.
- Privacy says that uninstalling deletes the data. The site's privacy policy
  must say so too, since the listing links it.
- About says "Microsoft Store".

Writes outside AppData are not redirected: a transcript saved next to its
recording lands there, as from the setup program's copy.

Windows redirects only a folder the package makes. Where the setup
program's copy ran first, `%APPDATA%\net.pktito.app` is already there, and
the Store's copy reads and writes that real folder: same models, settings
and recent list, and uninstalling it leaves them. This was found with a
signed test package on the 9950X (2026-10-26): its recent transcript went
into the real folder, and its `LocalCache` held nothing of pktito's. So
`status.store` says where the app came from, not where its data is. In
that case Privacy's "uninstalling deletes them" is wrong, and the hidden
Show in File Explorer would work.

No restricted capability beyond `runFullTrust`, which every desktop program
in a package declares; Partner Center asks for a line saying why ("a Win32
desktop app"). `unvirtualizedResources` would keep AppData shared with the
setup program's copy. It was tested and does that (2026-10-23), but the Store
often refuses it, and nothing here needs it.

## The window's runtime

WebView2 is not something a package can install. Windows 11 has it, as does
Windows 10 updated through Edge. The manifest's minimum is Windows 10 1809.

## Two ways into the Store

1. **The MSIX (this):** the Store hosts, signs and updates it, and a user
   installs it with no prompt. The data is per package and goes on uninstall.
2. **The setup program as a Win32 listing:**
   - The Store links to our signed setup by a versioned URL (policy 10.2.9).
   - It must install silently (`/S`) and offline, and must not download the
     app.
   - The Store doesn't update it, so each release is a new submission. The
     app doesn't update itself.
   - Behaviour is identical to the download from pktito.net.

The MSIX is the better fit: updates come through the Store, which the setup
program can't offer, and uninstalling takes the models with it.

## Making a package

```powershell
# from the release build (cargo tauri build --no-bundle, plus pktito-cli)
scripts\msix.ps1 -Arch x64                    # unsigned test package
scripts\msix.ps1 -Arch arm64
scripts\msix.ps1 -Arch x64 -Identity store    # Partner Center's identity
scripts\msix.ps1 -Bundle a.msix,b.msix -Out pktito_0.9.2.0.msixbundle
```

A test package installs from an administrator's PowerShell:
`Add-AppxPackage -AllowUnsigned pktito_0.9.2.0_x64-test.msix`. It has its
own identity, so it sits beside the setup program's copy and beside a Store
install.

## Before the first submission

- Reserve the name in Partner Center, and copy its Package/Identity/Name and
  Publisher into `$store` in msix.ps1.
- The listing needs:
  - screenshots and a description;
  - the privacy policy URL (pktito.net/privacy);
  - an age rating questionnaire;
  - the runFullTrust justification.
- Run the Windows App Certification Kit on the package (it needs an
  administrator).
- Version: Cargo.toml's, with a fourth part of 0, which the Store keeps for
  itself.

## The icons

`Assets/` holds the taskbar and Start icon, `Square44x44Logo`, at each
target size Windows draws it, in the unplated and light-unplated forms.
Without them, Windows scales the 44 px logo onto a plate of the accent
colour, which shows round the icon's rounded corners. They are
`icons/icon.png` resized with Lanczos:

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
