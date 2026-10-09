//! The page for a single app.

use eframe::egui::{self, Align, Color32, CornerRadius, Layout, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};

use super::widgets::{bytes, date, dim, progress_bar};
use super::{Dialog, Launcher, Page};
use crate::catalog::{self, APPS, CraftApp};
use crate::settings::display_path;
use crate::theme::{self, ButtonKind, button, eyebrow, pill};

impl Launcher {
    pub(super) fn app_page(&mut self, ui: &mut Ui, idx: usize) {
        let app: &'static CraftApp = &APPS[idx];
        let full = ui.available_rect_before_wrap();

        // Hero wash across the top of the page plus a large faded icon.
        let hero = Rect::from_min_size(full.min - vec2(40.0, 0.0), vec2(full.width() + 80.0, 420.0));
        theme::paint_glow(ui, &self.glow, hero, 0, app.accent.gamma_multiply(0.38));
        let ghost = Rect::from_min_size(pos2(full.right() - 230.0, full.top() + 40.0), Vec2::splat(250.0));
        let show_ghost = full.width() > 860.0;
        let text_w = if show_ghost { (full.width() - 280.0).min(680.0) } else { full.width().min(680.0) };
        if show_ghost {
            egui::Image::new((self.icons[idx].id(), ghost.size()))
                .corner_radius(56)
                .tint(Color32::WHITE.gamma_multiply(0.10))
                .paint_at(ui, ghost);
        }

        ui.add_space(22.0);
        if button(ui, "← All apps", ButtonKind::Ghost, false).clicked() {
            self.ui_state.page = Page::All;
        }
        ui.add_space(18.0);

        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(120.0), Sense::hover());
            ui.painter().rect_filled(r.translate(vec2(0.0, 8.0)).expand(2.0), CornerRadius::same(28), Color32::from_black_alpha(70));
            egui::Image::new((self.icons[idx].id(), r.size())).corner_radius(26).paint_at(ui, r);
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.add_space(4.0);
                eyebrow(ui, &format!("{:02} · {}", idx + 1, app.category), theme::tint(app.accent));
                let mut job = egui::text::LayoutJob::default();
                let font = theme::heading(50.0);
                job.append(app.prefix, 0.0, egui::TextFormat::simple(font.clone(), theme::TEXT));
                job.append("Craft", 0.0, egui::TextFormat::simple(font, theme::tint(app.accent)));
                ui.label(job);
                ui.horizontal(|ui| {
                    let color = match app.stage {
                        catalog::Stage::EarlyAlpha => theme::tint(app.accent),
                        catalog::Stage::InDevelopment => theme::TEXT_DIM,
                    };
                    pill(ui, app.stage.label(), color, true);
                    self.status_pill(ui, idx, false);
                    let platforms = if app.web { "macOS · Windows · Linux · Web" } else { "macOS · Windows · Linux" };
                    pill(ui, platforms, theme::TEXT_DIM, false);
                });
            });
        });

        ui.add_space(22.0);
        ui.scope(|ui| {
            ui.set_max_width(text_w);
            ui.label(RichText::new(app.tagline).font(theme::semibold(19.0)));
            ui.add_space(2.0);
            ui.label(RichText::new(app.summary).font(theme::body(15.0)).color(theme::TEXT_DIM));
        });
        ui.add_space(20.0);

        ui.horizontal(|ui| {
            if let Some((label, fraction)) = self.job_label(idx, false) {
                progress_bar(ui, 320.0, fraction, &label, theme::tint(app.accent));
                ui.add_space(8.0);
            }
            self.actions(ui, idx, true);
            self.more_menu(ui, idx, true);
            ui.add_space(8.0);
            if let Some(url) = app.website()
                && button(ui, "Website ↗", ButtonKind::Outline, true).clicked()
            {
                self.open_url(url);
            }
            if button(ui, "Source ↗", ButtonKind::Outline, true).clicked() {
                self.open_url(app.repo());
            }
        });

        ui.add_space(44.0);
        eyebrow(ui, "02 / Highlights", theme::TEXT_FAINT);
        ui.label(RichText::new("What's inside").font(theme::heading(28.0)));
        ui.add_space(10.0);
        highlights(ui, app);

        ui.add_space(40.0);
        eyebrow(ui, "03 / Install", theme::TEXT_FAINT);
        ui.label(RichText::new("On this machine").font(theme::heading(28.0)));
        ui.add_space(10.0);
        self.install_panel(ui, idx);
        ui.add_space(48.0);
    }

    fn install_panel(&mut self, ui: &mut Ui, idx: usize) {
        let app = &APPS[idx];
        panel(ui, |ui| {
            // What is installed.
            if let Some(m) = self.managed[idx].clone() {
                ui.horizontal(|ui| {
                    pill(ui, &format!("Installed v{}", m.version), theme::OK, true);
                    pill(ui, m.format.label(), theme::TEXT_DIM, false);
                    dim(ui, format!("{} · managed by the launcher", bytes(self.managed_size[idx])));
                });
                ui.add_space(4.0);
                let dir = m.folder(self.base());
                ui.label(RichText::new(display_path(&m.exe_path(self.base()))).font(theme::mono(13.0)));
                if self.launch_path(idx).is_none() {
                    ui.label(RichText::new("The executable is missing. Reinstall from the More menu.").color(super::widgets::DANGER));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if button(ui, crate::desktop::SHOW_FOLDER, ButtonKind::Outline, false).clicked() {
                        self.show_folder(&dir);
                    }
                    let can_remove = !self.busy(idx) && !self.running(idx);
                    let resp = button(ui, "Uninstall…", ButtonKind::Ghost, false);
                    let resp = if self.running(idx) { resp.on_hover_text(format!("Close {} first", app.name())) } else { resp };
                    if resp.clicked() && can_remove {
                        self.dialog = Some(Dialog::Uninstall(idx));
                    }
                });
            } else if let Some(ext) = &self.external[idx] {
                ui.horizontal(|ui| {
                    pill(ui, "Installed", theme::TEXT_DIM, true);
                    dim(ui, if ext.custom { "path set by you" } else { "found outside the launcher" });
                });
                ui.add_space(4.0);
                ui.label(RichText::new(display_path(&ext.path)).font(theme::mono(13.0)));
                ui.add_space(4.0);
                dim(
                    ui,
                    format!(
                        "The launcher can't update this copy. Install it with the launcher to get automatic updates in {}.",
                        display_path(self.base())
                    ),
                );
            } else {
                pill(ui, "Not installed", theme::TEXT_DIM, true);
                ui.add_space(4.0);
                let text = match self.choice(idx).map(|c| c.format) {
                    Some(crate::settings::Format::Msi) => format!(
                        "Installing downloads the latest stable {} MSI, verified against its published checksum, and \
                         runs it: Windows asks for permission, then installs {} in Program Files.",
                        crate::releases::platform(),
                        app.name()
                    ),
                    format => format!(
                        "Installing downloads the latest stable {} {} into {}, verified against its published checksum.",
                        crate::releases::platform(),
                        format.unwrap_or(self.settings.format).label().to_lowercase(),
                        display_path(&self.base().join(app.id))
                    ),
                };
                dim(ui, text);
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(10.0);

            // Latest release.
            ui.label(RichText::new("Latest stable release").font(theme::semibold(14.0)));
            let latest = self.choice(idx).map(|c| (c.release.clone(), c.asset.size, c.format));
            match latest {
                Some((release, size, format)) => {
                    let fallback = if format != self.settings.format {
                        format!(" (no {} in this release)", self.settings.format.label())
                    } else {
                        String::new()
                    };
                    dim(
                        ui,
                        format!(
                            "v{} · published {} · {} {}{}",
                            release.version,
                            date(&release.published),
                            bytes(size),
                            format.label(),
                            fallback
                        ),
                    );
                    if !release.notes.trim().is_empty() {
                        egui::CollapsingHeader::new(RichText::new("Release notes").color(theme::TEXT_DIM))
                            .id_salt(("notes", idx))
                            .show(ui, |ui| {
                                let notes: String = release.notes.chars().take(6000).collect();
                                ui.label(RichText::new(notes).font(theme::mono(12.0)).color(theme::TEXT_DIM));
                                if button(ui, "View on GitHub ↗", ButtonKind::Ghost, false).clicked() {
                                    self.open_url(release.page.clone());
                                }
                            });
                    }
                }
                None if self.checking > 0 => dim(ui, "Checking GitHub…"),
                None => {
                    let why = self.release_errors[idx]
                        .clone()
                        .unwrap_or_else(|| format!("no stable {} build has been published", crate::releases::platform()));
                    ui.label(RichText::new(format!("Unavailable: {why}")).color(theme::TEXT_DIM));
                }
            }
            if let Some(err) = self.release_errors[idx].clone().filter(|_| self.releases[idx].is_some()) {
                ui.label(RichText::new(format!("Last check failed: {err}")).font(theme::body(12.5)).color(theme::TEXT_FAINT));
            }

            ui.add_space(10.0);
            egui::CollapsingHeader::new(RichText::new("Advanced").color(theme::TEXT_DIM))
                .id_salt(("advanced", idx))
                .show(ui, |ui| self.advanced(ui, idx));
        });
    }

    fn advanced(&mut self, ui: &mut Ui, idx: usize) {
        let app = &APPS[idx];
        ui.add_space(4.0);
        ui.label(RichText::new("Use another copy").font(theme::semibold(14.0)));
        dim(ui, "Point at an executable installed some other way. Used only while the launcher doesn't manage this app.");
        ui.horizontal(|ui| {
            let edit = egui::TextEdit::singleline(&mut self.path_edit[idx])
                .hint_text(if cfg!(target_os = "macos") {
                    format!("e.g. /Applications/{}.app", app.name())
                } else if cfg!(windows) {
                    format!(r"e.g. C:\Program Files\{}\{}.exe", app.name(), app.id)
                } else {
                    format!("e.g. ~/Applications/{}.AppImage", app.id)
                })
                .font(theme::mono(13.0))
                .desired_width((ui.available_width() - 190.0).max(200.0))
                .margin(vec2(10.0, 8.0));
            let resp = ui.add(edit);
            let submit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if button(ui, "Use path", ButtonKind::Outline, false).clicked() || submit {
                self.set_custom_path(idx);
            }
            if self.settings.custom_paths.contains_key(app.id) && button(ui, "Clear", ButtonKind::Ghost, false).clicked() {
                self.path_edit[idx].clear();
                self.set_custom_path(idx);
            }
        });

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Build from source").font(theme::semibold(14.0)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(ui, "Copy", ButtonKind::Ghost, false).clicked() {
                    ui.ctx().copy_text(app.build_command());
                    self.toast("Build commands copied to the clipboard");
                }
            });
        });
        egui::Frame::new().fill(theme::SIDEBAR).corner_radius(8).inner_margin(14).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(app.build_command()).font(theme::mono(12.5)).color(theme::TEXT_DIM));
        });
    }
}

fn panel(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(12)
        .inner_margin(22)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

fn highlights(ui: &mut Ui, app: &CraftApp) {
    let w = ui.available_width();
    let gap = 14.0;
    let cols = if w > 900.0 { 3 } else if w > 560.0 { 2 } else { 1 };
    let card_w = (w - gap * (cols as f32 - 1.0)) / cols as f32;
    for (row_i, row) in app.highlights.chunks(cols).enumerate() {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (col_i, hl) in row.iter().enumerate() {
                let n = row_i * cols + col_i + 1;
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .corner_radius(12)
                    .inner_margin(18)
                    .show(ui, |ui| {
                        ui.set_width(card_w - 38.0);
                        ui.set_min_height(92.0);
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;
                            ui.label(theme::spaced(&format!("{n:02}"), theme::mono(11.0), theme::tint(app.accent), 1.0));
                            ui.label(RichText::new(hl.title).font(theme::semibold(15.5)));
                            ui.label(RichText::new(hl.body).font(theme::body(13.5)).color(theme::TEXT_DIM));
                        });
                    });
            }
        });
        ui.add_space(gap - ui.spacing().item_spacing.y);
    }
}
