# LandingCraft

A native desktop launcher for the seven [ArtCraft Crafting Apps](https://getartcraft.com/apps):
PhotoCraft, VectorCraft, FilmCraft, LightCraft, PdfCraft, EffectCraft and DesignCraft.
It works like a small Creative Cloud–style hub. It installs the latest stable release of each app,
keeps them up to date, opens them and uninstalls them, all from one place.

## Build and run

```bash
./build.sh   # optimised release build for this CPU
./run.sh     # rebuilds if sources changed, then launches
```

`build.sh` finds `cargo` even when `~/.cargo/bin` isn't on `PATH`, and builds with
`-C target-cpu=native`. Use `--portable` for a binary that runs on other x86-64 machines, or
`--debug` for faster compiles. When the project sits on an NTFS/FAT/exFAT or FUSE mount, build
output goes to `~/.cache/landingcraft/target` instead of `./target`, because Cargo's many small
files are slow on those filesystems. Setting `CARGO_TARGET_DIR` overrides this. `run.sh` rebuilds
with the same options as the last `build.sh`. It also accepts `--gpu NAME`, `--list-gpus` and
`--no-build`. Both scripts take `--help`.

To install it for your user, with an application-menu entry and icon:

```bash
./install.sh               # binary to ~/.local/bin, menu entry and icon under ~/.local/share
./install.sh --uninstall   # removes those three files; installed Crafting Apps stay
```

`--prefix /usr/local` (run as root) installs system-wide instead. The menu entry comes from
`packaging/landingcraft.desktop`, and its file name matches the window's Wayland app ID
(`landingcraft`), which is how desktops connect the running window to its icon
(`packaging/landingcraft.svg`).

You need a Vulkan driver for at least one GPU. Nothing else is required at build time. No C/C++ code is
compiled, and the binary links only against libc. At runtime it loads the system Vulkan loader and
Wayland or X11 libraries. HTTPS uses rustls with the pure-Rust RustCrypto provider and your system's CA
certificates.

## What it does

- **Library**: cards for every app with its icon, category, tagline, release stage and version. Each
  app's accent colour is taken from its icon background and used throughout its card and page.
- **Install, update, uninstall**: **Install** downloads the newest stable release that has a build for
  this OS and CPU. **Update** appears when a newer one is published, and **Update all** handles every app
  at once. The More (•••) menu on each card and page has Reinstall, Check for updates, Show in file
  manager and Uninstall.
- **Launching**: starts the app detached in its own process group and shows it as *Running* until it
  exits. A running app can't be updated, uninstalled or moved.
- **Other copies**: apps installed some other way are still found and can be opened. That covers `$PATH`,
  `~/.local/bin`, `~/.cargo/bin`, `~/Applications`, `~/AppImages`, `~/Downloads`, `~/bin` and `/opt`.
  You can also point an app at a specific executable under **Advanced** on its page. The launcher's own
  copy always takes priority.
- **Links and folders**: URLs open through the XDG Desktop Portal over D-Bus (`zbus`), not `xdg-open`.
  The same goes for the folder picker. Folders open in the file manager via `org.freedesktop.FileManager1`.

## Installations

Apps live in **`~/.craftapps`** by default, one folder per app:

```
~/.craftapps/
  photocraft/
    craftapp.json                      # manifest: version, format, checksum, executable
    photocraft-0.5.0-linux-x86_64/     # unpacked tarball (or a single .AppImage file)
```

- **Stable only.** A release qualifies only if GitHub doesn't mark it as a draft or pre-release *and* its
  tag is a plain `X.Y.Z` version. The tag check matters because some release candidates
  (`v0.1.1-rc.5`) were published without the pre-release flag. Tags with `-rc`, `-beta`, `-nightly`
  or `+build` suffixes are always skipped.
- **Verified.** Each download is checked against GitHub's SHA-256 digest for the asset, or the release's
  `SHA256SUMS.txt` for older releases. Without a checksum, the launcher won't install the release.
- **Safe replacement.** The new version is downloaded and unpacked next to the old one and swapped in
  only once it's complete, so a failed or cancelled update leaves the working copy alone. Archives may
  contain only files, folders and links that stay inside the app's folder.
- **Settings → Installations** changes the folder with your desktop's folder picker, or you can type a
  path. Installed apps move with it: renamed on the same drive, or copied then deleted across drives.
  If any move fails, the apps already moved are put back.
- **Settings → Package format** chooses between **Tarball** (default: unpacked folder, fastest start)
  and **AppImage** (one file per app). It applies to new installs and updates, and **Reinstall** converts
  an existing app. AppImages run with FUSE 2 when it's present, and otherwise by extracting first.

Launcher settings live in `~/.config/landingcraft/settings.json`, and the release-list cache in
`~/.cache/landingcraft/releases.json`. Updates are checked at startup and every six hours, and the
automatic checks can be turned off.

## Window

The launcher draws its own title bar, with drag-to-move, double-click to maximise, minimise/maximise/close,
and resizing from the edges and corners. That way it looks the same on every desktop. This matters on
Wayland, where GNOME and some other compositors don't draw title bars for apps, and winit's built-in
title bars would mean running helper programs (`dbus-send`, `gsettings`, `fc-match`). On a desktop that
does draw title bars, such as KDE Plasma, Xfce or most X11 window managers, **Settings → Window → Use the
desktop's title bar** switches to the native one from the next start.

## Graphics

Rendering goes through `wgpu` with **only the Vulkan backend** compiled in. Adapter choice ignores vendor
and power class. The launcher uses the first hardware device the Vulkan loader lists that can present
to the window. To pick another one, set `LANDINGCRAFT_GPU` to part of its name:

```bash
LANDINGCRAFT_GPU=radv cargo run --release
```

**Settings & about** shows the active device and lists every Vulkan device found.

## Layout

| Path | Purpose |
| --- | --- |
| `src/catalog.rs` | App data: names, summaries, highlights, accents, versions |
| `src/app.rs` | Launcher state and actions (install, update, uninstall, move) |
| `src/app/` | Pages and dialogs: library, app detail, settings, title bar, shared widgets |
| `src/releases.rs` | GitHub release lookup over HTTPS, stable filtering, asset choice |
| `src/installer.rs` | Download, verify, unpack, swap in, uninstall, move installs |
| `src/settings.rs` | Install folder, package format and other persisted settings |
| `src/theme.rs` | Colours, fonts and the custom-painted buttons, pills and glows |
| `src/detect.rs` | Finding copies installed elsewhere, and process launching |
| `src/gpu.rs` | Vulkan-only wgpu setup and adapter selection |
| `src/portal.rs` | Desktop portal and file manager over D-Bus |
| `assets/icons` | App icons from getartcraft.com |
| `packaging/` | Desktop entry and scalable app icon, installed by `install.sh` |
| `assets/fonts` | Space Grotesk and IBM Plex (SIL OFL 1.1, see `assets/fonts/OFL.txt`) |

## Tests

```bash
cargo test
```

The end-to-end installer test downloads about 120 MB from GitHub. It installs PdfCraft as a tarball,
switches it to an AppImage, moves it, then uninstalls it. Run it on request:

```bash
LC_TEST_DIR=/tmp/lc-test cargo test --release -- --ignored --nocapture
```
