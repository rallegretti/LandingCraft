//! Sidebar, library grid and app cards.

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder,
    Vec2, pos2, vec2,
};

use super::widgets::progress_bar;
use super::{Launcher, Page};
use crate::catalog::{self, APPS};
use crate::theme::{self, ButtonKind, button, eyebrow, pill};

impl Launcher {
    pub(super) fn sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
            let accents: Vec<Color32> = APPS.iter().map(|a| a.accent).collect();
            theme::paint_logo(ui, rect.center(), 18.0, &accents);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new("LandingCraft").font(theme::heading(18.0)));
                ui.label(theme::spaced("ARTCRAFT APPS", theme::mono(9.5), theme::TEXT_FAINT, 1.4));
            });
        });
        ui.add_space(26.0);

        let total = APPS.len().to_string();
        let installed = self.installed_count().to_string();
        self.nav_item(ui, None, "All apps", &total, Page::All, theme::BRAND);
        self.nav_item(ui, None, "Installed", &installed, Page::Installed, theme::BRAND);

        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            eyebrow(ui, "Apps", theme::TEXT_FAINT);
        });
        ui.add_space(2.0);
        for (i, app) in APPS.iter().enumerate() {
            let trailing = if let Some(job) = &self.jobs[i] {
                if job.uninstalling { "…".to_owned() } else { format!("{:.0}%", job.fraction() * 100.0) }
            } else if self.running(i) {
                "running".to_owned()
            } else if self.update_for(i).is_some() {
                "update".to_owned()
            } else if self.is_installed(i) {
                "installed".to_owned()
            } else {
                String::new()
            };
            self.nav_item(ui, Some(i), &app.name(), &trailing, Page::App(i), app.accent);
        }

        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                if button(ui, "Discord", ButtonKind::Ghost, false).clicked() {
                    self.open_url(catalog::DISCORD.into());
                }
                if button(ui, "GitHub", ButtonKind::Ghost, false).clicked() {
                    self.open_url(catalog::GITHUB_ORG.into());
                }
            });
            ui.add_space(4.0);
            self.nav_item(ui, None, "Settings & about", "", Page::Settings, theme::BRAND);
            ui.add_space(4.0);
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(r.x_range().shrink(10.0), r.bottom(), Stroke::new(1.0, theme::BORDER));
        });
    }

    fn nav_item(&mut self, ui: &mut Ui, icon: Option<usize>, label: &str, trailing: &str, page: Page, accent: Color32) {
        let width = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(width, 36.0), Sense::click());
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let selected = self.ui_state.page == page;
        let t = ui.ctx().animate_bool(resp.id, resp.hovered() || selected);

        let bg = rect.shrink2(vec2(6.0, 1.0));
        if t > 0.0 {
            let fill = if selected { theme::SURFACE_HI } else { theme::SURFACE.gamma_multiply(t) };
            ui.painter().rect_filled(bg, CornerRadius::same(6), fill);
        }
        if selected {
            let bar = Rect::from_min_size(pos2(bg.left(), bg.top() + 8.0), vec2(3.0, bg.height() - 16.0));
            ui.painter().rect_filled(bar, CornerRadius::same(2), accent);
        }

        let mut x = bg.left() + 14.0;
        if let Some(i) = icon {
            let r = Rect::from_center_size(pos2(x + 11.0, bg.center().y), Vec2::splat(22.0));
            egui::Image::new((self.icons[i].id(), r.size())).corner_radius(5).paint_at(ui, r);
            x += 32.0;
        }
        let color = if selected { theme::TEXT } else { theme::TEXT_DIM.lerp_to_gamma(theme::TEXT, t) };
        let font = if selected { theme::semibold(14.0) } else { theme::body(14.0) };
        theme::text_at(ui, pos2(x, bg.center().y), Align2::LEFT_CENTER, label, font, color);

        let right = pos2(bg.right() - 12.0, bg.center().y);
        let dot = right - vec2(3.0, 0.0);
        match trailing {
            "running" => {
                let pulse = (ui.input(|i| i.time) * 3.0).sin() as f32 * 0.5 + 0.5;
                ui.painter().circle_filled(dot, 3.5, theme::OK.gamma_multiply(0.55 + 0.45 * pulse));
                ui.ctx().request_repaint();
            }
            "update" => {
                ui.painter().circle_filled(dot, 3.5, theme::tint(accent));
            }
            "installed" => {
                ui.painter().circle_stroke(dot, 3.0, Stroke::new(1.5, theme::TEXT_FAINT));
            }
            "" => {}
            n => {
                theme::text_at(ui, right, Align2::RIGHT_CENTER, n, theme::mono(11.0), theme::TEXT_FAINT);
            }
        }

        if resp.clicked() {
            self.ui_state.page = page;
        }
    }

    pub(super) fn home(&mut self, ui: &mut Ui, installed_only: bool) {
        let max_w = ui.available_width();
        ui.add_space(36.0);
        eyebrow(ui, "Seven apps · Open source · Pure Rust", theme::TEXT_FAINT);
        ui.add_space(6.0);

        let mut job = egui::text::LayoutJob::default();
        let fmt = |c| egui::TextFormat { font_id: theme::heading(46.0), color: c, ..Default::default() };
        let (a, b) = if installed_only { ("Your ", "crafts") } else { ("Seven apps. One ", "craft") };
        job.append(a, 0.0, fmt(theme::TEXT));
        job.append(b, 0.0, fmt(theme::BRAND));
        job.append(".", 0.0, fmt(theme::TEXT));
        ui.label(job);
        ui.add_space(4.0);

        let sub = if installed_only {
            "Crafting Apps on this machine. The launcher keeps the ones it installed up to date."
        } else {
            "Image editing, vector illustration, video, photography, PDFs, motion graphics and \
             page layout. Native, open-source apps from the ArtCraft team, built in Rust and free to use."
        };
        ui.scope(|ui| {
            ui.set_max_width(max_w.min(640.0));
            ui.label(RichText::new(sub).font(theme::body(15.5)).color(theme::TEXT_DIM));
        });
        ui.add_space(18.0);

        ui.horizontal(|ui| {
            if button(ui, "Join the Discord", ButtonKind::Solid(Color32::WHITE), true).clicked() {
                self.open_url(catalog::DISCORD.into());
            }
            if button(ui, "Browse on GitHub", ButtonKind::Outline, true).clicked() {
                self.open_url(catalog::GITHUB_ORG.into());
            }
            ui.add_space(4.0);
            let label = if self.checking > 0 { "Checking…" } else { "↻ Check for updates" };
            let resp = button(ui, label, ButtonKind::Ghost, true);
            let resp = match self.last_checked() {
                Some(t) => resp.on_hover_text(format!("Last checked {}", super::widgets::ago(t))),
                None => resp,
            };
            if resp.clicked() {
                self.rescan();
                self.check_updates();
            }
        });

        self.updates_banner(ui);
        ui.add_space(30.0);

        let indices: Vec<usize> = (0..APPS.len()).filter(|&i| !installed_only || self.is_installed(i)).collect();
        if indices.is_empty() {
            self.empty_state(ui);
            return;
        }

        let gap = 16.0;
        let cols = ((max_w + gap) / (290.0 + gap)).floor().clamp(1.0, 4.0) as usize;
        let card_w = (max_w - gap * (cols as f32 - 1.0)) / cols as f32;
        for row in indices.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for &i in row {
                    self.card(ui, i, vec2(card_w, 250.0));
                }
            });
            ui.add_space(gap - ui.spacing().item_spacing.y);
        }
        ui.add_space(40.0);
    }

    fn updates_banner(&mut self, ui: &mut Ui) {
        let updates = self.updates();
        if updates.is_empty() {
            return;
        }
        ui.add_space(22.0);
        egui::Frame::new()
            .fill(theme::BRAND.gamma_multiply(0.08))
            .stroke(Stroke::new(1.0, theme::BRAND.gamma_multiply(0.35)))
            .corner_radius(10)
            .inner_margin(egui::vec2(18.0, 12.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let names: Vec<String> = updates
                        .iter()
                        .map(|&i| format!("{} v{}", APPS[i].name(), self.update_for(i).map(|v| v.to_string()).unwrap_or_default()))
                        .collect();
                    let n = updates.len();
                    ui.label(
                        RichText::new(format!("{n} update{} available", if n == 1 { "" } else { "s" }))
                            .font(theme::semibold(14.5)),
                    );
                    ui.label(RichText::new(names.join(" · ")).color(theme::TEXT_DIM));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let busy = updates.iter().all(|&i| self.jobs[i].is_some());
                        if !busy && button(ui, "Update all", ButtonKind::Solid(theme::BRAND), false).clicked() {
                            self.update_all();
                        }
                    });
                });
            });
    }

    fn empty_state(&mut self, ui: &mut Ui) {
        egui::Frame::new()
            .fill(theme::SURFACE)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .corner_radius(12)
            .inner_margin(28)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Nothing installed yet").font(theme::heading(22.0)));
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!(
                        "Install any Crafting App from its card. The launcher downloads the latest stable release \
                         into {} and keeps it up to date.",
                        crate::settings::display_path(self.base())
                    ))
                    .color(theme::TEXT_DIM),
                );
                ui.add_space(12.0);
                if button(ui, "Browse all apps", ButtonKind::Solid(theme::BRAND), true).clicked() {
                    self.ui_state.page = Page::All;
                }
            });
    }

    fn card(&mut self, ui: &mut Ui, idx: usize, size: Vec2) {
        let app = &APPS[idx];
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        if !ui.is_rect_visible(rect) {
            return;
        }
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let hover = ui.ctx().animate_bool(resp.id, resp.contains_pointer());
        let radius = CornerRadius::same(14);

        let p = ui.painter();
        p.rect_filled(rect, radius, theme::SURFACE.lerp_to_gamma(theme::SURFACE_HI, hover));
        // Accent wash rising from the top edge, stronger on hover.
        theme::paint_glow(
            ui,
            &self.glow,
            Rect::from_min_size(rect.min, vec2(rect.width(), rect.height() * 0.9)),
            radius,
            app.accent.gamma_multiply(0.16 + 0.14 * hover),
        );
        p.rect_stroke(rect, radius, Stroke::new(1.0, theme::BORDER.lerp_to_gamma(app.accent, hover * 0.8)), StrokeKind::Inside);

        // Child UIs inside the card don't advance the row's cursor.
        let inner = rect.shrink(20.0);
        {
            let ui = &mut ui.new_child(UiBuilder::new().max_rect(inner).layout(Layout::top_down(Align::Min)));
            ui.horizontal(|ui| {
                let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
                let icon_rect = icon_rect.translate(vec2(0.0, -2.0 * hover));
                egui::Image::new((self.icons[idx].id(), icon_rect.size())).corner_radius(14).paint_at(ui, icon_rect);
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    let color = match app.stage {
                        catalog::Stage::EarlyAlpha => theme::tint(app.accent),
                        catalog::Stage::InDevelopment => theme::TEXT_DIM,
                    };
                    pill(ui, app.stage.label(), color, false);
                });
            });
            ui.add_space(10.0);
            ui.label(RichText::new(app.name()).font(theme::heading(23.0)));
            ui.add_space(-6.0);
            eyebrow(ui, app.category, theme::tint(app.accent));
            ui.add_space(2.0);
            ui.label(RichText::new(app.tagline).font(theme::body(13.5)).color(theme::TEXT_DIM));
        }

        // Footer: status (or progress) on the left, actions on the right.
        let footer = Rect::from_min_max(pos2(inner.left(), inner.bottom() - 32.0), inner.max);
        {
            let ui = &mut ui.new_child(UiBuilder::new().max_rect(footer).layout(Layout::left_to_right(Align::Center)));
            if let Some((label, fraction)) = self.job_label(idx, true) {
                let w = (footer.width() - 80.0).max(80.0);
                progress_bar(ui, w, fraction, &label, theme::tint(app.accent));
            } else {
                self.status_pill(ui, idx, true);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if self.is_installed(idx) || self.jobs[idx].is_none() {
                    self.more_menu(ui, idx, false);
                }
                self.actions(ui, idx, false);
            });
        }

        if resp.clicked() {
            self.ui_state.page = Page::App(idx);
        }
    }
}
