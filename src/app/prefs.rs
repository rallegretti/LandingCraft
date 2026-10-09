//! Settings & about page.

use eframe::egui::{self, RichText, Sense, Ui, Vec2, vec2};

use super::Launcher;
use super::widgets::{ago, bytes, dim, kv, section};
use crate::catalog::{self, APPS};
use crate::settings::{self, Format};
use crate::theme::{self, ButtonKind, button, eyebrow};
use crate::{gpu, releases};

impl Launcher {
    pub(super) fn settings_page(&mut self, ui: &mut Ui) {
        ui.add_space(36.0);
        eyebrow(ui, "Settings & about", theme::TEXT_FAINT);
        ui.label(RichText::new("LandingCraft").font(theme::heading(40.0)));
        dim(
            ui,
            format!(
                "Version {} · a native launcher for the Crafting Apps, written in Rust and drawn with {}.",
                env!("CARGO_PKG_VERSION"),
                gpu::API
            ),
        );
        ui.add_space(26.0);

        section(ui, "Installations", |ui| self.installations(ui));
        section(ui, "Package format", |ui| self.format_choice(ui));
        section(ui, "Updates", |ui| self.update_prefs(ui));
        section(ui, "Window", |ui| self.window_prefs(ui));
        section(ui, "Graphics", |ui| self.graphics(ui));
        section(ui, "About", |ui| {
            dim(
                ui,
                "App names, descriptions and icons come from the ArtCraft Crafting Apps page. Releases are \
                 downloaded from github.com/storytold over HTTPS (rustls with pure-Rust cryptography) and checked \
                 against GitHub's SHA-256 digests. Interface type is Space Grotesk and IBM Plex, both under the \
                 SIL Open Font License 1.1.",
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if button(ui, "Crafting Apps ↗", ButtonKind::Outline, false).clicked() {
                    self.open_url(catalog::APPS_PAGE.into());
                }
                if button(ui, "Discord ↗", ButtonKind::Outline, false).clicked() {
                    self.open_url(catalog::DISCORD.into());
                }
                if button(ui, "GitHub ↗", ButtonKind::Outline, false).clicked() {
                    self.open_url(catalog::GITHUB_ORG.into());
                }
            });
        });
        ui.add_space(40.0);
    }

    fn installations(&mut self, ui: &mut Ui) {
        let count = self.managed.iter().filter(|m| m.is_some()).count();
        let total: u64 = self.managed_size.iter().sum();
        dim(
            ui,
            "Every app the launcher installs lives in its own folder here. Changing the location moves \
             installed apps along with it.",
        );
        ui.add_space(8.0);
        kv(ui, "Location", &settings::display_path(self.base()));
        kv(
            ui,
            "Installed",
            &format!("{count} app{} · {}", if count == 1 { "" } else { "s" }, bytes(total)),
        );
        ui.add_space(10.0);

        let locked = self.moving || self.jobs.iter().any(Option::is_some);
        ui.horizontal(|ui| {
            let resp = button(ui, "Change location…", ButtonKind::Outline, false);
            if resp.clicked() && !locked {
                self.pick_install_dir();
            }
            if button(ui, crate::desktop::SHOW_FOLDER, ButtonKind::Ghost, false).clicked() {
                let dir = self.base().to_path_buf();
                self.show_folder(&dir);
            }
            let default_dir = settings::default_install_dir();
            if self.settings.install_dir != default_dir
                && button(ui, &format!("Reset to {}", settings::display_path(&default_dir)), ButtonKind::Ghost, false).clicked()
                && !locked
            {
                self.request_install_dir(settings::default_install_dir());
            }
        });
        if locked {
            ui.label(RichText::new("Unavailable while apps are installing or moving.").font(theme::body(12.5)).color(theme::TEXT_FAINT));
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let edit = egui::TextEdit::singleline(&mut self.dir_edit)
                .font(theme::mono(13.0))
                .desired_width((ui.available_width() - 120.0).max(200.0))
                .margin(vec2(10.0, 8.0));
            let resp = ui.add(edit);
            let submit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (button(ui, "Use folder", ButtonKind::Ghost, false).clicked() || submit) && !locked {
                match settings::parse_user_path(&self.dir_edit) {
                    Some(p) => self.request_install_dir(p),
                    None => self.toast(format!("Enter an absolute path, e.g. {}", settings::EXAMPLE_DIR)),
                }
            }
        });
    }

    fn format_choice(&mut self, ui: &mut Ui) {
        let formats = Format::available();
        if formats.len() < 2 {
            // Nothing to choose: describe the one format the apps ship in here.
            match formats.first() {
                Some(Format::Dmg) => dim(
                    ui,
                    "Crafting Apps for macOS are published as disk images. The launcher opens each one out of \
                     sight, copies the app inside into its own folder with its signature intact, and closes the \
                     image again.",
                ),
                Some(Format::Zip) => dim(
                    ui,
                    "Crafting Apps for Windows are published as portable zip files. The launcher unpacks each one \
                     into its own folder, and the apps keep their settings in your user profile, so updates and \
                     moves never touch them.",
                ),
                _ => dim(ui, format!("The launcher can't install Crafting Apps on {} yet.", releases::platform())),
            }
            return;
        }
        ui.horizontal(|ui| {
            for &format in formats {
                let kind = if self.settings.format == format { ButtonKind::Solid(theme::BRAND) } else { ButtonKind::Outline };
                if button(ui, format.label(), kind, false).clicked() && self.settings.format != format {
                    self.settings.format = format;
                    self.save_settings();
                }
            }
        });
        ui.add_space(6.0);
        match self.settings.format {
            Format::Tarball => dim(
                ui,
                "Each app is unpacked into its own folder. Starts fastest and needs nothing else on the system.",
            ),
            Format::Dmg | Format::Zip => {}
            Format::AppImage => {
                dim(ui, "Each app is kept as a single self-contained .AppImage file.");
                #[cfg(all(unix, not(target_os = "macos")))]
                if crate::detect::has_fuse2() {
                    dim(ui, "FUSE 2 is available, so AppImages mount directly.");
                } else {
                    dim(
                        ui,
                        "FUSE 2 (libfuse.so.2) wasn't found, so AppImages will unpack to a temporary folder each \
                         time they start. That works, but starts more slowly.",
                    );
                }
            }
        }
        ui.label(
            RichText::new(
                "Applies to new installs and updates. To convert an installed app, choose Reinstall in its More menu. \
                 If a release lacks the chosen format, the other one is used.",
            )
            .font(theme::body(12.5))
            .color(theme::TEXT_FAINT),
        );
    }

    fn update_prefs(&mut self, ui: &mut Ui) {
        let mut on = self.settings.check_on_startup;
        if ui.checkbox(&mut on, "Check for updates automatically").changed() {
            self.settings.check_on_startup = on;
            self.save_settings();
        }
        dim(
            ui,
            "Only stable releases are offered: drafts, pre-releases and tags such as -rc, -beta or -nightly are \
             skipped. Checks happen at startup and every few hours while the launcher is open.",
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let label = if self.checking > 0 { "Checking…" } else { "↻ Check now" };
            if button(ui, label, ButtonKind::Outline, false).clicked() {
                self.check_updates();
            }
            if let Some(t) = self.last_checked() {
                ui.label(RichText::new(format!("Last checked {}", ago(t))).color(theme::TEXT_FAINT));
            }
        });
        let errors: Vec<String> = APPS
            .iter()
            .zip(&self.release_errors)
            .filter_map(|(a, e)| e.as_ref().map(|e| format!("{}: {e}", a.name())))
            .collect();
        for e in errors.iter().take(3) {
            ui.label(RichText::new(e).font(theme::body(12.5)).color(theme::TEXT_FAINT));
        }
    }

    fn window_prefs(&mut self, ui: &mut Ui) {
        let mut native = self.settings.native_title_bar;
        let (label, about) = if cfg!(target_os = "macos") {
            (
                "Use the standard macOS title bar",
                "By default the launcher draws its own title bar beneath the window buttons, so it looks the same \
                 everywhere. Turn this on to use the standard macOS title bar instead.",
            )
        } else if cfg!(windows) {
            (
                "Use the standard Windows title bar",
                "By default the launcher draws its own title bar, so it looks the same everywhere. Turn this on to \
                 use the standard Windows title bar and window frame instead.",
            )
        } else {
            (
                "Use the desktop's title bar",
                "By default the launcher draws its own title bar, so it looks the same everywhere. Turn this on to \
                 use your desktop's instead. Only do this if your desktop draws title bars for apps: KDE Plasma, \
                 Xfce and most X11 window managers do, GNOME on Wayland doesn't.",
            )
        };
        if ui.checkbox(&mut native, label).changed() {
            self.settings.native_title_bar = native;
            self.save_settings();
        }
        dim(ui, about);
        if native != !self.custom_frame {
            ui.label(
                RichText::new("Takes effect the next time LandingCraft starts.")
                    .font(theme::body(12.5))
                    .color(theme::TEXT_FAINT),
            );
        }
    }

    fn graphics(&mut self, ui: &mut Ui) {
        let Some(gpu) = &self.gpu else {
            dim(ui, "Renderer information unavailable.");
            return;
        };
        kv(ui, "Backend", &format!("{:?}", gpu.active.backend));
        kv(ui, "Device", &gpu.active.name);
        kv(ui, "Type", device_type(gpu.active.device_type));
        let driver = format!("{} {}", gpu.active.driver, gpu.active.driver_info);
        kv(ui, "Driver", driver.trim());
        ui.add_space(10.0);
        ui.label(RichText::new(format!("{} devices on this system", gpu::API)).font(theme::semibold(14.0)));
        for info in &gpu.all {
            let active = info.name == gpu.active.name && info.device == gpu.active.device;
            ui.horizontal(|ui| {
                let c = if active { theme::OK } else { theme::TEXT_FAINT };
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 3.5, c);
                ui.label(RichText::new(&info.name).color(if active { theme::TEXT } else { theme::TEXT_DIM }));
                ui.label(RichText::new(device_type(info.device_type)).font(theme::mono(11.0)).color(theme::TEXT_FAINT));
            });
        }
        ui.add_space(8.0);
        dim(
            ui,
            format!(
                "No vendor or power class is preferred: the first hardware device {} reports is \
                 used. To choose another, start the launcher with {}=<part of the device name>.",
                gpu::DEVICE_SOURCE,
                gpu::GPU_ENV
            ),
        );
    }
}

fn device_type(t: eframe::wgpu::DeviceType) -> &'static str {
    use eframe::wgpu::DeviceType::*;
    match t {
        DiscreteGpu => "discrete GPU",
        IntegratedGpu => "integrated GPU",
        VirtualGpu => "virtual GPU",
        Cpu => "software",
        Other => "other",
    }
}
