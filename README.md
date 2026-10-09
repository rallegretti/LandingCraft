# LandingCraft

A native desktop launcher for the seven [ArtCraft Crafting Apps](https://getartcraft.com/apps):
PhotoCraft, VectorCraft, FilmCraft, LightCraft, PdfCraft, EffectCraft and DesignCraft.
It works like a small Creative Cloud–style hub. Browse the apps, see which are installed, open them,
or jump to their releases, website and source.

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

You need a Vulkan driver for at least one GPU. Nothing else is required at build time. No C/C++ code is
compiled, and the binary links only against libc. At runtime it loads the system Vulkan loader and
Wayland or X11 libraries.

## What it does

- **Library**: cards for every app with its icon, category, tagline, release stage and version. Each
  app's accent colour is taken from its icon background and used throughout its card and page.
- **App pages**: summary, six highlights, platform list, and Open / Get / Website / Source actions.
- **Discovery**: finds installed apps on `$PATH`, in `~/.local/bin`, `~/.cargo/bin`, `~/Applications`,
  `~/AppImages`, `~/Downloads`, `~/bin` and `/opt`. It accepts a binary named after the app, a
  `<app>-*.AppImage`, or an unpacked `<app>-*/` release folder. You can also set an explicit
  executable on each app's page. That setting is saved and takes priority over discovery.
- **Launching**: starts the app detached in its own process group and shows it as *Running* until it exits.
- **Links**: opens URLs through the XDG Desktop Portal over D-Bus (`zbus`) rather than by running
  `xdg-open`. If no portal answers, the link is copied to the clipboard.

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
| `src/app.rs` | The UI: sidebar, library grid, app pages, settings |
| `src/theme.rs` | Colours, fonts and the custom-painted buttons, pills and glows |
| `src/detect.rs` | Installed-app discovery and process launching |
| `src/gpu.rs` | Vulkan-only wgpu setup and adapter selection |
| `src/links.rs` | Opening URLs via the desktop portal |
| `assets/icons` | App icons from getartcraft.com |
| `assets/fonts` | Space Grotesk and IBM Plex (SIL OFL 1.1, see `assets/fonts/OFL.txt`) |
