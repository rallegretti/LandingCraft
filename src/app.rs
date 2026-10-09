use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Layout, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, TextureHandle, Ui, UiBuilder, Vec2, pos2, vec2,
};
use serde::{Deserialize, Serialize};

use crate::catalog::{self, APPS, CraftApp};
use crate::detect::{self, Processes};
use crate::theme::{self, ButtonKind, button, eyebrow, pill};
use crate::{gpu, links};

const STORAGE_KEY: &str = "landingcraft";

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
enum Page {
    #[default]
    All,
    Installed,
    App(usize),
    Settings,
}

#[derive(Serialize, Deserialize, Default)]
struct Saved {
    page: Page,
    /// User-provided executable per app id, overriding auto-detection.
    custom_paths: HashMap<String, String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Detected,
    Custom,
}

struct Install {
    path: PathBuf,
    source: Source,
}

struct Toast {
    text: String,
    until: f64,
}

struct GpuInfo {
    active: eframe::wgpu::AdapterInfo,
    all: Vec<eframe::wgpu::AdapterInfo>,
}

pub struct Launcher {
    saved: Saved,
    icons: Vec<TextureHandle>,
    glow: TextureHandle,
    installs: Vec<Option<Install>>,
    processes: Processes,
    path_edit: Vec<String>,
    toasts: Vec<Toast>,
    link_failures: (Sender<(String, String)>, Receiver<(String, String)>),
    gpu: Option<GpuInfo>,
}

impl Launcher {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);

        let saved: Saved = cc
            .storage
            .and_then(|s| eframe::get_value(s, STORAGE_KEY))
            .unwrap_or_default();

        let icons = APPS
            .iter()
            .map(|app| {
                let img = image::load_from_memory_with_format(app.icon_webp, image::ImageFormat::WebP)
                    .expect("bundled icon decodes")
                    .to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                let opts = egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear));
                cc.egui_ctx.load_texture(app.id, color, opts)
            })
            .collect();

        let gpu = cc.wgpu_render_state.as_ref().map(|rs| GpuInfo {
            active: rs.adapter.get_info(),
            all: rs.available_adapters.iter().map(|a| a.get_info()).collect(),
        });

        let path_edit = APPS
            .iter()
            .map(|a| saved.custom_paths.get(a.id).cloned().unwrap_or_default())
            .collect();

        let mut launcher = Self {
            saved,
            icons,
            glow: theme::glow_texture(&cc.egui_ctx),
            installs: Vec::new(),
            processes: Processes::default(),
            path_edit,
            toasts: Vec::new(),
            link_failures: channel(),
            gpu,
        };
        launcher.rescan();
        launcher
    }

    fn rescan(&mut self) {
        self.installs = APPS
            .iter()
            .map(|app| {
                if let Some(p) = self.saved.custom_paths.get(app.id)
                    && detect::is_executable(&PathBuf::from(p))
                {
                    return Some(Install { path: PathBuf::from(p), source: Source::Custom });
                }
                detect::find(app.id).map(|path| Install { path, source: Source::Detected })
            })
            .collect();
    }

    fn installed_count(&self) -> usize {
        self.installs.iter().filter(|i| i.is_some()).count()
    }

    fn toast(&mut self, ctx: &egui::Context, text: impl Into<String>) {
        let now = ctx.input(|i| i.time);
        self.toasts.push(Toast { text: text.into(), until: now + 4.0 });
    }

    fn open_url(&self, url: String) {
        links::open(url, self.link_failures.0.clone());
    }

    fn launch(&mut self, ctx: &egui::Context, idx: usize) {
        let app = &APPS[idx];
        let Some(install) = &self.installs[idx] else { return };
        let path = install.path.clone();
        match self.processes.launch(app.id, &path) {
            Ok(()) => self.toast(ctx, format!("Starting {}…", app.name())),
            Err(e) => self.toast(ctx, format!("Couldn't start {}: {e}", app.name())),
        }
    }

    /// Primary action for an app: open it if installed, otherwise go get it.
    fn primary_action(&mut self, ui: &mut Ui, idx: usize, large: bool) {
        let app = &APPS[idx];
        if self.installs[idx].is_some() {
            if button(ui, "Open", ButtonKind::Solid(app.accent), large).clicked() {
                self.launch(ui.ctx(), idx);
            }
        } else if button(ui, "Get", ButtonKind::Outline, large)
            .on_hover_text("Open the latest release on GitHub")
            .clicked()
        {
            self.open_url(app.releases());
        }
    }

    // ----------------------------------------------------------------- sidebar

    fn sidebar(&mut self, ui: &mut Ui) {
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
            let status = if self.processes.running(app.id) > 0 {
                "running"
            } else if self.installs[i].is_some() {
                "installed"
            } else {
                ""
            };
            self.nav_item(ui, Some(i), &app.name(), status, Page::App(i), app.accent);
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
        let selected = self.saved.page == page;
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
        match trailing {
            "running" => {
                let pulse = (ui.input(|i| i.time) * 3.0).sin() as f32 * 0.5 + 0.5;
                ui.painter().circle_filled(right - vec2(3.0, 0.0), 3.5, theme::OK.gamma_multiply(0.55 + 0.45 * pulse));
                ui.ctx().request_repaint();
            }
            "installed" => {
                ui.painter().circle_stroke(right - vec2(3.0, 0.0), 3.0, Stroke::new(1.5, theme::TEXT_FAINT));
            }
            "" => {}
            n => {
                theme::text_at(ui, right, Align2::RIGHT_CENTER, n, theme::mono(11.0), theme::TEXT_FAINT);
            }
        }

        if resp.clicked() {
            self.saved.page = page;
        }
    }

    // -------------------------------------------------------------- home page

    fn home(&mut self, ui: &mut Ui, installed_only: bool) {
        let max_w = ui.available_width();
        ui.add_space(36.0);
        eyebrow(ui, "Seven apps · Open source · Pure Rust", theme::TEXT_FAINT);
        ui.add_space(6.0);

        let mut job = egui::text::LayoutJob::default();
        let fmt = |c| egui::TextFormat { font_id: theme::heading(46.0), color: c, ..Default::default() };
        if installed_only {
            job.append("Your ", 0.0, fmt(theme::TEXT));
            job.append("crafts", 0.0, fmt(theme::BRAND));
            job.append(".", 0.0, fmt(theme::TEXT));
        } else {
            job.append("Seven apps. One ", 0.0, fmt(theme::TEXT));
            job.append("craft", 0.0, fmt(theme::BRAND));
            job.append(".", 0.0, fmt(theme::TEXT));
        }
        ui.label(job);
        ui.add_space(4.0);

        let sub = if installed_only {
            "Crafting Apps found on this machine. Open one, or point the launcher at an \
             executable from an app's page."
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
            if button(ui, "↻ Rescan", ButtonKind::Ghost, true)
                .on_hover_text("Search this machine for installed apps again")
                .clicked()
            {
                self.rescan();
                let n = self.installed_count();
                self.toast(ui.ctx(), format!("Found {n} of {} apps installed", APPS.len()));
            }
        });
        ui.add_space(34.0);

        let indices: Vec<usize> = (0..APPS.len())
            .filter(|&i| !installed_only || self.installs[i].is_some())
            .collect();

        if indices.is_empty() {
            self.empty_state(ui);
            return;
        }

        let gap = 16.0;
        let cols = ((max_w + gap) / (280.0 + gap)).floor().clamp(1.0, 4.0) as usize;
        let card_w = (max_w - gap * (cols as f32 - 1.0)) / cols as f32;
        let card_h = 248.0;

        for row in indices.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for &i in row {
                    self.card(ui, i, vec2(card_w, card_h));
                }
            });
            ui.add_space(gap - ui.spacing().item_spacing.y);
        }
        ui.add_space(40.0);
    }

    fn empty_state(&mut self, ui: &mut Ui) {
        egui::Frame::new()
            .fill(theme::SURFACE)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .corner_radius(12)
            .inner_margin(28)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("No Crafting Apps found yet").font(theme::heading(22.0)));
                ui.add_space(2.0);
                ui.label(
                    RichText::new(
                        "The launcher looks on your PATH, in ~/.local/bin, ~/.cargo/bin, ~/Applications, \
                         ~/Downloads and /opt for executables and AppImages named after each app.",
                    )
                    .color(theme::TEXT_DIM),
                );
                ui.add_space(12.0);
                if button(ui, "Browse all apps", ButtonKind::Solid(theme::BRAND), true).clicked() {
                    self.saved.page = Page::All;
                }
            });
    }

    fn card(&mut self, ui: &mut Ui, idx: usize, size: Vec2) {
        let app = &APPS[idx];
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let hover = ui.ctx().animate_bool(resp.id, resp.hovered() || resp.contains_pointer());
        let radius = CornerRadius::same(14);

        if !ui.is_rect_visible(rect) {
            return;
        }

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
        p.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, theme::BORDER.lerp_to_gamma(app.accent, hover * 0.8)),
            StrokeKind::Inside,
        );

        // Child UIs inside the card don't advance the row's cursor.
        let inner = rect.shrink(20.0);
        {
            let ui = &mut ui.new_child(UiBuilder::new().max_rect(inner).layout(Layout::top_down(Align::Min)));
            ui.horizontal(|ui| {
                let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
                // Lift the icon slightly on hover.
                let icon_rect = icon_rect.translate(vec2(0.0, -2.0 * hover));
                egui::Image::new((self.icons[idx].id(), icon_rect.size()))
                    .corner_radius(14)
                    .paint_at(ui, icon_rect);
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

        // Footer: status on the left, action on the right.
        let footer = Rect::from_min_max(pos2(inner.left(), inner.bottom() - 30.0), inner.max);
        {
            let ui = &mut ui.new_child(UiBuilder::new().max_rect(footer).layout(Layout::left_to_right(Align::Center)));
            let running = self.processes.running(app.id);
            if running > 0 {
                pill(ui, "Running", theme::OK, true);
            } else if self.installs[idx].is_some() {
                pill(ui, "Installed", theme::TEXT_DIM, true);
            } else {
                ui.label(theme::spaced(&format!("v{}", app.version), theme::mono(11.0), theme::TEXT_FAINT, 0.6));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.primary_action(ui, idx, false);
                if button(ui, "Details", ButtonKind::Ghost, false).clicked() {
                    self.saved.page = Page::App(idx);
                }
            });
        }

        if resp.clicked() {
            self.saved.page = Page::App(idx);
        }
    }

    // --------------------------------------------------------------- app page

    fn app_page(&mut self, ui: &mut Ui, idx: usize) {
        let app: &'static CraftApp = &APPS[idx];
        let full = ui.available_rect_before_wrap();

        // Hero wash across the whole top of the page plus a large faded icon.
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
            self.saved.page = Page::All;
        }
        ui.add_space(18.0);

        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(120.0), Sense::hover());
            ui.painter().rect_filled(
                r.translate(vec2(0.0, 8.0)).expand(2.0),
                CornerRadius::same(28),
                Color32::from_black_alpha(70),
            );
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
                    pill(ui, &format!("v{}", app.version), theme::TEXT_DIM, false);
                    let platforms = if app.web { "macOS · Windows · Linux · Web" } else { "macOS · Windows · Linux" };
                    pill(ui, platforms, theme::TEXT_DIM, false);
                    let running = self.processes.running(app.id);
                    if running > 0 {
                        let label = if running == 1 { "Running".to_owned() } else { format!("{running} running") };
                        pill(ui, &label, theme::OK, true);
                    }
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
            self.primary_action(ui, idx, true);
            if self.installs[idx].is_some() && button(ui, "Releases", ButtonKind::Outline, true).clicked() {
                self.open_url(app.releases());
            }
            if button(ui, "Website ↗", ButtonKind::Outline, true).clicked() {
                self.open_url(app.website());
            }
            if button(ui, "Source ↗", ButtonKind::Outline, true).clicked() {
                self.open_url(app.repo());
            }
        });

        ui.add_space(44.0);
        eyebrow(ui, "02 / Highlights", theme::TEXT_FAINT);
        ui.label(RichText::new("What's inside").font(theme::heading(28.0)));
        ui.add_space(10.0);
        self.highlights(ui, app);

        ui.add_space(40.0);
        eyebrow(ui, "03 / Install", theme::TEXT_FAINT);
        ui.label(RichText::new("On this machine").font(theme::heading(28.0)));
        ui.add_space(10.0);
        self.install_panel(ui, idx);
        ui.add_space(48.0);
    }

    fn highlights(&self, ui: &mut Ui, app: &CraftApp) {
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

    fn install_panel(&mut self, ui: &mut Ui, idx: usize) {
        let app = &APPS[idx];
        egui::Frame::new()
            .fill(theme::SURFACE)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .corner_radius(12)
            .inner_margin(22)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                match &self.installs[idx] {
                    Some(install) => {
                        ui.horizontal(|ui| {
                            pill(ui, "Installed", theme::OK, true);
                            let how = match install.source {
                                Source::Detected => "found automatically",
                                Source::Custom => "set by you",
                            };
                            ui.label(RichText::new(how).color(theme::TEXT_FAINT));
                        });
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(detect::display_path(&install.path))
                                .font(theme::mono(13.0))
                                .color(theme::TEXT),
                        )
                        .on_hover_text(install.path.display().to_string());
                    }
                    None => {
                        pill(ui, "Not found", theme::TEXT_DIM, true);
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!(
                                "Download the Linux AppImage or tarball from the releases page into ~/Applications \
                                 or ~/Downloads, install the .deb/.rpm, or build it from source. Files named \
                                 {id}, {id}-*.AppImage or {id}-*/{id} are picked up automatically.",
                                id = app.id
                            ))
                            .color(theme::TEXT_DIM),
                        );
                    }
                }

                ui.add_space(16.0);
                ui.label(RichText::new("Executable path").font(theme::semibold(14.0)));
                ui.horizontal(|ui| {
                    let edit = egui::TextEdit::singleline(&mut self.path_edit[idx])
                        .hint_text(format!("e.g. ~/Applications/{}-{}-linux-x86_64.AppImage", app.id, app.version))
                        .font(theme::mono(13.0))
                        .desired_width((ui.available_width() - 190.0).max(200.0))
                        .margin(vec2(10.0, 8.0));
                    let resp = ui.add(edit);
                    let submit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if button(ui, "Use path", ButtonKind::Solid(app.accent), false).clicked() || submit {
                        self.set_custom_path(ui.ctx(), idx);
                    }
                    if self.saved.custom_paths.contains_key(app.id)
                        && button(ui, "Clear", ButtonKind::Ghost, false).clicked()
                    {
                        self.saved.custom_paths.remove(app.id);
                        self.path_edit[idx].clear();
                        self.rescan();
                    }
                });

                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Build from source").font(theme::semibold(14.0)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if button(ui, "Copy", ButtonKind::Ghost, false).clicked() {
                            ui.ctx().copy_text(app.build_command());
                            self.toast(ui.ctx(), "Build commands copied to the clipboard");
                        }
                    });
                });
                egui::Frame::new()
                    .fill(theme::SIDEBAR)
                    .corner_radius(8)
                    .inner_margin(14)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(app.build_command()).font(theme::mono(12.5)).color(theme::TEXT_DIM));
                    });
            });
    }

    fn set_custom_path(&mut self, ctx: &egui::Context, idx: usize) {
        let app = &APPS[idx];
        let raw = self.path_edit[idx].trim().to_owned();
        if raw.is_empty() {
            self.saved.custom_paths.remove(app.id);
            self.rescan();
            return;
        }
        let expanded = match (raw.strip_prefix("~/"), std::env::var("HOME")) {
            (Some(rest), Ok(home)) => format!("{home}/{rest}"),
            _ => raw.clone(),
        };
        if detect::is_executable(&PathBuf::from(&expanded)) {
            self.saved.custom_paths.insert(app.id.to_owned(), expanded);
            self.rescan();
            self.toast(ctx, format!("{} will launch from {raw}", app.name()));
        } else {
            self.toast(ctx, format!("{raw} isn't an executable file"));
        }
    }

    // ---------------------------------------------------------- settings page

    fn settings(&mut self, ui: &mut Ui) {
        ui.add_space(36.0);
        eyebrow(ui, "Settings & about", theme::TEXT_FAINT);
        ui.label(RichText::new("LandingCraft").font(theme::heading(40.0)));
        ui.label(
            RichText::new(format!(
                "Version {} · a native launcher for the Crafting Apps, written in Rust and drawn with Vulkan.",
                env!("CARGO_PKG_VERSION")
            ))
            .color(theme::TEXT_DIM),
        );
        ui.add_space(26.0);

        section(ui, "Graphics", |ui| {
            match &self.gpu {
                Some(gpu) => {
                    kv(ui, "Backend", &format!("{:?}", gpu.active.backend));
                    kv(ui, "Device", &gpu.active.name);
                    kv(ui, "Type", device_type(gpu.active.device_type));
                    let driver = format!("{} {}", gpu.active.driver, gpu.active.driver_info);
                    kv(ui, "Driver", driver.trim());
                    ui.add_space(10.0);
                    ui.label(RichText::new("Vulkan devices on this system").font(theme::semibold(14.0)));
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
                    ui.label(
                        RichText::new(format!(
                            "No vendor or power class is preferred: the first hardware device the Vulkan loader \
                             reports is used. To choose another, start the launcher with {}=<part of the device name>.",
                            gpu::GPU_ENV
                        ))
                        .color(theme::TEXT_DIM),
                    );
                }
                None => {
                    ui.label(RichText::new("Renderer information unavailable.").color(theme::TEXT_DIM));
                }
            }
        });

        section(ui, "App discovery", |ui| {
            ui.label(
                RichText::new(
                    "Each app is looked for on your PATH, then in ~/.local/bin, ~/.cargo/bin, ~/Applications, \
                     ~/AppImages, ~/Downloads, ~/bin and /opt — as a binary named after the app, a versioned \
                     AppImage, or an unpacked release folder. A path set on an app's page always wins.",
                )
                .color(theme::TEXT_DIM),
            );
            ui.add_space(8.0);
            if button(ui, "↻ Rescan now", ButtonKind::Outline, false).clicked() {
                self.rescan();
                let n = self.installed_count();
                self.toast(ui.ctx(), format!("Found {n} of {} apps installed", APPS.len()));
            }
        });

        section(ui, "About", |ui| {
            ui.label(
                RichText::new(
                    "App names, descriptions and icons come from the ArtCraft Crafting Apps page. Interface type \
                     is Space Grotesk and IBM Plex, both under the SIL Open Font License 1.1.",
                )
                .color(theme::TEXT_DIM),
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

    // ------------------------------------------------------------------ toasts

    fn show_toasts(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        self.toasts.retain(|t| t.until > now);
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(Align2::RIGHT_BOTTOM, vec2(-20.0, -20.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                for t in &self.toasts {
                    let fade = ((t.until - now) as f32 / 0.4).clamp(0.0, 1.0);
                    egui::Frame::new()
                        .fill(theme::SURFACE_HI.gamma_multiply(fade))
                        .stroke(Stroke::new(1.0, theme::BORDER.gamma_multiply(fade)))
                        .corner_radius(8)
                        .inner_margin(vec2(14.0, 10.0))
                        .shadow(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha((90.0 * fade) as u8) })
                        .show(ui, |ui| {
                            ui.label(RichText::new(&t.text).color(theme::TEXT.gamma_multiply(fade)));
                        });
                }
            });
        ctx.request_repaint();
    }
}

fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(12)
        .inner_margin(22)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).font(theme::heading(20.0)));
            ui.add_space(6.0);
            add(ui);
        });
    ui.add_space(14.0);
}

fn kv(ui: &mut Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(90.0, 18.0), Sense::hover());
        theme::text_at(ui, r.left_center(), Align2::LEFT_CENTER, &key.to_uppercase(), theme::mono(11.0), theme::TEXT_FAINT);
        ui.label(value);
    });
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

impl eframe::App for Launcher {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.processes.poll();
        if APPS.iter().any(|a| self.processes.running(a.id) > 0) {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        while let Ok((url, err)) = self.link_failures.1.try_recv() {
            ctx.copy_text(url.clone());
            self.toast(ctx, format!("Couldn't open a browser ({err}). Link copied: {url}"));
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("sidebar")
            .exact_size(240.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::SIDEBAR)
                    .inner_margin(egui::Margin { left: 8, right: 8, top: 0, bottom: 0 })
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::no_frame()
            .frame(egui::Frame::new().fill(theme::BG))
            .show(ui, |ui| {
                let page = self.saved.page;
                ScrollArea::vertical()
                    .id_salt(match page {
                        Page::App(i) => i + 10,
                        Page::All => 0,
                        Page::Installed => 1,
                        Page::Settings => 2,
                    })
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .inner_margin(egui::Margin { left: 40, right: 40, top: 0, bottom: 0 })
                            .show(ui, |ui| match page {
                                Page::All => self.home(ui, false),
                                Page::Installed => self.home(ui, true),
                                Page::App(i) => self.app_page(ui, i.min(APPS.len() - 1)),
                                Page::Settings => self.settings(ui),
                            });
                    });
            });

        let ctx = ui.ctx().clone();
        self.show_toasts(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.saved);
    }
}
