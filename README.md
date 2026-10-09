<p align="center">
  <img src="packaging/landingcraft.svg" width="96" alt="LandingCraft icon">
</p>

<h1 align="center">LandingCraft</h1>

<p align="center">
  A native launcher for the <a href="https://getartcraft.com/apps">ArtCraft Crafting Apps</a>.<br>
  Install, update, open and remove every Crafting App from one window.
</p>

<p align="center">
  <b>Pure Rust</b> · <b>Vulkan</b> / <b>Metal</b> · <b>Linux</b> · <b>macOS</b> · <b>Windows</b> · stable releases only · checksum-verified installs
</p>

![The LandingCraft library, with three apps installed and an update available for VectorCraft](docs/screenshots/library.webp)

## Contents

- [The apps](#the-apps)
- [A quick tour](#a-quick-tour)
- [Settings](#settings)
- [How installs work](#how-installs-work)
- [Getting started](#getting-started)
- [macOS](#macos)
- [Windows](#windows)
- [Linux releases](#linux-releases)
- [Publishing a release](#publishing-a-release)
- [Built in Rust](#built-in-rust)
- [Development](#development)
- [Credits](#credits)
- [License](#license)

## The apps

| | App | What it is |
| --- | --- | --- |
| <img src="assets/icons/photocraft.webp" width="32"> | **PhotoCraft** | Image editor |
| <img src="assets/icons/vectorcraft.webp" width="32"> | **VectorCraft** | Vector illustration |
| <img src="assets/icons/filmcraft.webp" width="32"> | **FilmCraft** | Video editor |
| <img src="assets/icons/lightcraft.webp" width="32"> | **LightCraft** | Photo library and raw developer |
| <img src="assets/icons/pdfcraft.webp" width="32"> | **PdfCraft** | PDF workbench |
| <img src="assets/icons/effectcraft.webp" width="32"> | **EffectCraft** | Motion graphics and VFX |
| <img src="assets/icons/designcraft.webp" width="32"> | **DesignCraft** | Page layout and publishing |
| <img src="assets/icons/soundcraft.webp" width="32"> | **SoundCraft** | Audio workstation |
| <img src="assets/icons/cadcraft.webp" width="32"> | **CADCraft** | CAD and drafting |

Each app has its own accent colour, taken from its icon background. Its card, page and buttons all use it.

## A quick tour

### The library

The home screen has a card for every app. Each card shows the app's category, a one-line description,
its release stage, and either the installed version or the latest available one. The sidebar marks
installed apps with a ring, apps with an update with a coloured dot, and running apps with a pulsing
green dot. **Installed** in the sidebar shows only the apps on this machine.

### Installing and updating

**Install** downloads the newest stable release with a build for your OS and CPU. Progress shows on
the card, in the sidebar and on the app's page, and the download can be cancelled at any time. When a
newer release comes out, **Update** appears on the app, and a banner offers **Update all**.

![Installing FilmCraft at 47%, with an update banner for VectorCraft](docs/screenshots/installing.webp)

### App pages and the More menu

Each app has a page with its summary, six highlights and supported platforms, plus links to its
website and source code. The **More (•••)** menu, on both the page and the card, holds everything else:
Update or Reinstall, Check for updates, Releases on GitHub, Show in file manager, and Uninstall.

![The VectorCraft page with an update to v0.7.0 available and the More menu open](docs/screenshots/app-page.webp)

### On this machine

The bottom of each app page shows what's installed: version, package format, disk use and
executable path. It also shows the latest stable release, with its release notes. Under **Advanced**
you can point the launcher at a copy you installed some other way, or copy the commands to build the
app from source.

![The "On this machine" panel for PhotoCraft v0.5.0](docs/screenshots/install-panel.webp)

### Uninstalling

Uninstall always asks first and says exactly which folder it will delete. It removes only that app's
folder in the install location, never documents you've made. An app started from the launcher can't
be uninstalled, updated or moved until you close it.

![The uninstall confirmation for PhotoCraft](docs/screenshots/uninstall.webp)

## Settings

![Settings: installations, package format, updates and window](docs/screenshots/settings.webp)

- **Installations.** Apps install to `~/.craftapps` by default (on macOS, `~/Applications/Crafting Apps`,
  so Spotlight and Launchpad find them; on Windows, `%LOCALAPPDATA%\Programs\Crafting Apps`).
  **Change location…** opens your desktop's folder picker, or you can type a path. Installed apps move with the folder: renamed on the
  same drive, or copied then deleted across drives. If anything fails partway, the apps already moved
  are put back.

  ![Confirming a move of three apps from ~/.craftapps to ~/Apps/craft](docs/screenshots/move.webp)

- **Package format.** **Tarball** (the default) unpacks each app into its own folder and starts fastest.
  **AppImage** keeps each app as one self-contained file, and runs it with FUSE 2 when available, or by
  unpacking first. The choice applies to new installs and updates. **Reinstall** converts an app that's
  already installed. If a release lacks the chosen format, the other one is used. On macOS the apps ship
  only as disk images and on Windows as portable zips, so there's nothing to choose (see [macOS](#macos)
  and [Windows](#windows)).
- **Updates.** The launcher checks at startup and every six hours while open, and you can turn this
  off. **Check now** runs a check straight away.
- **Window.** The launcher draws its own title bar so it looks the same on every desktop (see
  [Built in Rust](#built-in-rust)). On desktops that draw title bars for apps, such as KDE Plasma,
  Xfce and most X11 window managers, **Use the desktop's title bar** switches to the native one from the
  next start. On macOS the system's window buttons stay, over the launcher's bar, and **Use the standard
  macOS title bar** brings back the usual one. On Windows, **Use the standard Windows title bar** does the same.
- **Graphics.** Shows the GPU in use and lists every Vulkan (or, on macOS, Metal) device the system offers.

## How installs work

```
~/.craftapps/
  photocraft/
    craftapp.json                      # manifest: version, format, checksum, executable
    photocraft-0.5.0-linux-x86_64/     # unpacked tarball (or a single .AppImage file, PhotoCraft.app on macOS,
                                       # or photocraft-0.5.0-windows-x64-portable\ on Windows)
  vectorcraft/
    ...
```

- **Stable releases only.** A release qualifies only if GitHub doesn't mark it as a draft or pre-release
  *and* its tag is a plain `X.Y.Z` version. The tag check matters: some release candidates
  (`v0.1.1-rc.5`) were published without the pre-release flag. Tags with `-rc`, `-beta`, `-nightly` or
  `+build` suffixes are always skipped.
- **Verified downloads.** Every file is checked against GitHub's SHA-256 digest for that asset, or the
  release's `SHA256SUMS.txt` for older releases. The launcher won't install a release that has no
  checksum.
- **Safe replacement.** An update is downloaded and unpacked next to the current version and swapped in
  only when complete. A failed or cancelled update leaves the working copy untouched, and interrupted
  downloads are cleaned up the next time the launcher starts. Archives may contain only files, folders
  and links that stay inside the app's own folder.
- **Movable.** Paths in each manifest are relative, so the whole install folder can be relocated.
- **Copies installed elsewhere** are still found and can be opened. The launcher looks on `$PATH` and in
  `~/.local/bin`, `~/.cargo/bin`, `~/Applications`, `~/AppImages`, `~/Downloads`, `~/bin` and `/opt`.
  On macOS it looks for `<App>.app` in `/Applications` and `~/Applications`. On Windows it looks for
  `<app>.exe` on `PATH`, in Program Files (where the apps' MSIs install), and in unzipped release folders in
  your Downloads, Desktop and `Apps` folders. Its own managed copy always takes priority.

Settings are stored in `~/.config/landingcraft/settings.json`, and the release-list cache in
`~/.cache/landingcraft/releases.json`. On macOS they're in `~/Library/Application Support/landingcraft/`
and `~/Library/Caches/landingcraft/`, and on Windows in `%APPDATA%\landingcraft\` and
`%LOCALAPPDATA%\landingcraft\`.

## Getting started

You need Linux or Windows 10/11 with a Vulkan driver for at least one GPU, or macOS 11 or later. To build,
you also need [Rust](https://rustup.rs). Nothing else is required on Linux and macOS, because the launcher
compiles no C or C++ code; Windows builds need MinGW-w64's tools too (see [Windows](#windows)).
The scripts work the same way on Linux and macOS; on a Mac, `install.sh` puts `LandingCraft.app` in
`~/Applications` (see [macOS](#macos)). On Windows, run `build.sh` and `run.sh` from Git Bash.

```bash
./build.sh     # optimised release build for this CPU
./run.sh       # rebuilds if the sources changed, then launches
./install.sh   # adds LandingCraft to your application menu
```

| Script | Options |
| --- | --- |
| `build.sh` | `--portable` for a binary that runs on any x86-64 machine, `--debug` for faster compiles, `--clean` |
| `run.sh` | `--gpu NAME` to choose a GPU, `--list-gpus`, `--no-build` |
| `install.sh` | `--prefix DIR` (default `~/.local`, or `/usr/local` as root; on macOS `~/Applications`), `--uninstall` |

A few details:
- **Finding Rust:** `build.sh` finds `cargo` even when `~/.cargo/bin` isn't on your `PATH`.
- **Slow drives:** if the project lives on an NTFS, FAT, exFAT or FUSE drive, build output goes to
  `~/.cache/landingcraft/target` (`~/Library/Caches/landingcraft/target` on macOS), because Cargo's many
  small files are slow on those. Setting `CARGO_TARGET_DIR` overrides this.
- **What `install.sh` adds:** the binary, a desktop entry (`packaging/landingcraft.desktop`) and an icon
  (`packaging/landingcraft.svg`). `--uninstall` removes just those three and leaves your installed apps
  alone.

## macOS

The launcher runs natively on Apple silicon and Intel Macs, drawn with Metal. Mac-only code lives in
`src/macos.rs` and `packaging/macos/`, and Linux builds don't compile any of it.

**Installing Crafting Apps.** The apps publish `<app>-<version>-macos-universal.dmg`. The launcher
downloads and checksums it like any other release, mounts it read-only out of sight (no Finder window,
nothing on the desktop), copies the `.app` out with `ditto` so its code signature and notarization stay
intact, and unmounts it. Updates, moves and uninstalls then work as on Linux. Downloads made by the launcher
aren't quarantined, so apps open without a Gatekeeper prompt.

**Building and signing the launcher.** `packaging/macos/package.sh` builds a universal binary, assembles
`LandingCraft.app`, signs it, and wraps it in a disk image, all under `target/macos/`:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin   # once, for universal builds
packaging/macos/package.sh                                   # ad-hoc signed: runs on this Mac only
packaging/macos/package.sh --sign "Developer ID Application: Your Name (TEAMID)"
packaging/macos/package.sh --sign "Developer ID Application: Your Name (TEAMID)" --notarize PROFILE
```

| Option | Effect |
| --- | --- |
| `--sign IDENTITY` | Sign the app and disk image with a Developer ID, with the hardened runtime and a secure timestamp. Also read from `MACOS_SIGN_IDENTITY` |
| `--notarize PROFILE` | Submit the disk image to Apple's notary service, wait, and staple the ticket. `PROFILE` is a keychain profile saved once with `xcrun notarytool store-credentials`. Also read from `MACOS_NOTARY_PROFILE` |
| `--native` | Build for this Mac's CPU only (quicker; for local use) |
| `--app-only` | Stop after the `.app` |
| `--bundle-id ID` | Bundle identifier (default `io.github.rallegretti.landingcraft`) |

Signing needs the Xcode command line tools (`xcode-select --install`). The app icon,
`packaging/macos/AppIcon.icns`, is rendered from `packaging/landingcraft.svg` by
`packaging/macos/make-icon.swift`. Rerun that script if the SVG changes.

## Windows

The launcher runs on Windows 10 and 11, drawn with Vulkan like on Linux, so it needs the Vulkan driver
that comes with the GPU's own driver (NVIDIA, AMD and Intel all include one). Without it, the launcher
says so in a message box and quits: that includes most virtual machines. Windows-only code lives in
`src/win.rs` and `packaging/windows/`, and other platforms don't compile any of it.

**Installing Crafting Apps.** The apps publish `<app>-<version>-windows-<arch>-portable.zip` alongside
their MSIs. The launcher installs the zip, which it can manage like a tarball: checksummed, unpacked
into its own folder under the same rules (plus no names Windows treats specially, such as `CON` or
`a:b`), and swapped in when complete. The zips ship a `portable.txt` that makes an app keep its
settings beside its executable, where the next update would replace them, so the launcher removes it:
apps it installs keep their settings in your profile, as MSI installs do. Links, the folder picker and
**Show in Explorer** go through the Windows shell.

**Building.** Use Rust's GNU toolchain, which needs no Visual Studio, plus
[MinGW-w64](https://winlibs.com) for its `dlltool` (which the `windows` crates need) and `windres`
(which embeds the icon and version info, see `build.rs`). The scripts run in Git Bash, which comes
with [Git for Windows](https://gitforwindows.org), and find WinLibs where winget installs it:

```bash
winget install Rustlang.Rustup BrechtSanders.WinLibs.POSIX.MSVCRT   # once
rustup default stable-x86_64-pc-windows-gnu                         # once
./run.sh                                                            # build and launch
packaging/windows/package.sh                                        # the release zip
```

`packaging/windows/package.sh` builds `landingcraft-<version>-windows-<arch>.zip` under
`target/windows/`, holding `landingcraft.exe` with the README and licence. The exe links the C runtime
statically, so it needs only DLLs that come with Windows, and opens no console window. The script
checks both before zipping. It isn't code-signed, so Windows SmartScreen asks before the first run.
The app icon, `packaging/windows/landingcraft.ico`, is made from `packaging/macos/AppIcon.icns` by
`packaging/windows/make-icon.py`. Rerun that script if the icon changes.

## Linux releases

`packaging/linux/package.sh` builds the two Linux release files, under `target/linux/`:

- `landingcraft-<version>-linux-<arch>.tar.gz`: a plain `bin/` + `share/` tree with the binary, desktop
  entry, icon, README and licence. Unpack it anywhere and run `bin/landingcraft`.
- `landingcraft-<version>-linux-<arch>.AppImage`: one file to download, mark executable and run.

By default it builds inside an Ubuntu 22.04 container (Docker or Podman), so the binary needs only glibc
2.35 and runs on Ubuntu 22.04+, Debian 12+ and other distros of that age or newer, the same baseline as
the Crafting Apps. A release build made directly on a newer distro would refuse to start on older ones,
and `build.sh`'s default is tuned for the building machine's CPU, so neither is suitable for publishing.

```bash
packaging/linux/package.sh                 # container build: the files to publish
packaging/linux/package.sh --host          # build on this machine instead (runs only on glibc >= its own)
packaging/linux/package.sh --no-appimage   # tarball only
```

The container uses the same Rust version as your machine and keeps its toolchain and crates in
`target/linux/`, so later builds are quick. The AppImage is made with
[appimagetool](https://github.com/AppImage/appimagetool) (from `$APPIMAGETOOL` or your `PATH`, otherwise
downloaded once). It embeds the static type-2 runtime, so it doesn't need libfuse2.

## Publishing a release

`release.sh` uploads built files to the GitHub release for the version in `Cargo.toml` (tag
`v<version>`). Each platform builds and uploads its own files from its own machine, at the same pushed
commit. The release stays a draft, visible only to you, until `--publish` creates the tag and makes it
public:

```bash
gh auth login                                              # once per machine
packaging/macos/package.sh --sign "…" --notarize PROFILE   # on the Mac
./release.sh target/macos/landingcraft-<version>-macos-universal.dmg
packaging/linux/package.sh                                 # on Linux
./release.sh target/linux/landingcraft-<version>-linux-x86_64.{tar.gz,AppImage}
packaging/windows/package.sh                               # on Windows, in Git Bash
./release.sh target/windows/landingcraft-<version>-windows-x64.zip
./release.sh --publish                                     # when every platform is up
```

A platform can also be added after publishing: check out the release's tag (`git checkout v<version>`),
build, and upload as usual. The script refuses uncommitted or unpushed code, a commit other than the
release's, files not named for the current version, a disk image that isn't notarized, and replacing a
file that's already published. Bump `version` in `Cargo.toml` for each new release.

## Built in Rust

The whole launcher is Rust, and at runtime it starts no other programs except the Crafting Apps you open.
The one exception is on macOS, where installing a disk image runs the system's own `hdiutil` and `ditto`.

| Area | How |
| --- | --- |
| Interface | [egui](https://github.com/emilk/egui) via eframe and winit, with fonts and icons bundled |
| Graphics | [wgpu](https://wgpu.rs) with **only the Vulkan backend** compiled in (on macOS, **only Metal**) |
| GPU choice | No vendor or power class is preferred: the first hardware device the Vulkan loader (or Metal) lists is used. Override it with `LANDINGCRAFT_GPU=<part of name>` or `./run.sh --gpu` |
| HTTPS | [ureq](https://github.com/algesten/ureq) and rustls with the pure-Rust **RustCrypto** provider (no `ring`/`aws-lc` C or assembly), trusting your system's CA certificates |
| Archives and checksums | `flate2` (miniz_oxide), `tar`, `sha2`; on Windows also `zip`, deflate only, through the same miniz_oxide |
| Desktop integration | [zbus](https://github.com/dbus2/zbus) talks to the XDG Desktop Portal (opening links, folder picker) and the file manager directly over D-Bus, instead of running `xdg-open`. On macOS, Cocoa's `NSWorkspace` and `NSOpenPanel` through [objc2](https://github.com/madsmtm/objc2), the bindings winit already uses. On Windows, the shell's `ShellExecuteW` and `IFileOpenDialog` through the [windows](https://github.com/microsoft/windows-rs) crate wgpu already uses |
| Title bar | Drawn by the launcher. winit's own Wayland title bars would run `dbus-send`, `gsettings` and `fc-match` |

The release binary links only against the standard C runtime (`libc`, `libm`, `libgcc_s`). The Vulkan
loader and the Wayland or X11 libraries are loaded from the system when the app starts. On macOS it links
only against system frameworks, and on Windows only against DLLs that come with Windows (the C runtime is
linked in statically, and the Vulkan loader is loaded at startup).

**Note:** the RustCrypto TLS provider is still alpha software. It's the only all-Rust option today,
because the mature alternatives include C and assembly.

## Development

```bash
cargo test
```

The end-to-end installer test downloads about 120 MB from GitHub. It cancels a first install, installs
PdfCraft as a tarball, converts it to an AppImage, moves it to another folder, then uninstalls it. On
macOS it installs the disk image instead (about 70 MB), and on Windows the portable zip (about 55 MB).
Run it on request:

```bash
LC_TEST_DIR=/tmp/lc-test cargo test --release -- --ignored --nocapture
```

The screenshots in this README come from the optional `screenshot` feature. The launcher saves an image
of its own window, so nothing else on screen is ever captured:

```bash
cargo build --features screenshot
LANDINGCRAFT_SCREENSHOT=out.png LANDINGCRAFT_DEMO=menu:vectorcraft target/debug/landingcraft
```

### Source layout

| Path | Purpose |
| --- | --- |
| `src/catalog.rs` | App data: names, summaries, highlights, accents |
| `src/app.rs` | Launcher state and actions: install, update, uninstall, move |
| `src/app/` | Pages and dialogs: library, app page, settings, title bar, shared widgets |
| `src/releases.rs` | GitHub release lookup, stable filtering, choosing the right file |
| `src/installer.rs` | Download, verify, unpack, swap in, uninstall, move installs |
| `src/settings.rs` | Install folder, package format and other settings |
| `src/theme.rs` | Colours, fonts and the custom-painted buttons, pills and glows |
| `src/detect.rs` | Finding copies installed elsewhere, and launching apps |
| `src/gpu.rs` | Single-backend wgpu setup (Vulkan, or Metal on macOS) and GPU selection |
| `src/desktop.rs` | Opening links, picking folders and showing folders, one implementation per platform |
| `src/portal.rs` | Linux: desktop portal and file manager over D-Bus |
| `src/macos.rs` | macOS: Cocoa integration, `.app` bundles and disk images |
| `src/win.rs` | Windows: shell integration (links, folder picker, Explorer) and the startup error message |
| `src/screenshot.rs` | The development-only screenshot feature |
| `packaging/` | Desktop entry and app icon |
| `packaging/macos/` | `Info.plist`, `AppIcon.icns`, and the scripts that build, sign, notarize and install the Mac app |
| `packaging/linux/` | The script that builds the Linux release tarball and AppImage |
| `packaging/windows/` | The script that builds the Windows release zip, and the app icon with the script that makes it |
| `build.rs` | Windows: embeds the icon and version info in the exe |
| `docs/screenshots/` | Images used in this README |

## Credits

- **Apps:** names, descriptions and icons belong to the ArtCraft team. See
  [getartcraft.com/apps](https://getartcraft.com/apps) and [github.com/storytold](https://github.com/storytold).
- **Fonts:** [Space Grotesk](https://github.com/floriankarsten/space-grotesk) and
  [IBM Plex](https://github.com/IBM/plex), both under the SIL Open Font License 1.1
  (see [`assets/fonts/OFL.txt`](assets/fonts/OFL.txt)).

## License

LandingCraft's own code and files are dedicated to the public domain under
[CC0 1.0 Universal](LICENSE). You can copy, modify and distribute them, even commercially, without
asking permission.

That dedication doesn't cover material by others included in this repository:

- **The Crafting Apps' names, descriptions and icons** (`assets/icons/`, and as they appear in
  `docs/screenshots/`) belong to the ArtCraft team.
- **The bundled fonts** (`assets/fonts/`) remain under the SIL Open Font License 1.1.
