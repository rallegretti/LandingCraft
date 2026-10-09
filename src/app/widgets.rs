//! UI pieces shared between pages: app actions, the More menu, progress bars.

use eframe::egui::{self, Align2, Color32, CornerRadius, Rect, RichText, Sense, Stroke, Ui, vec2};

use super::{Dialog, Launcher, Page};
use crate::catalog::APPS;
use crate::theme::{self, ButtonKind, button};

pub const DANGER: Color32 = Color32::from_rgb(0xef, 0x5b, 0x5b);

pub fn bytes(n: u64) -> String {
    let n = n as f64;
    if n >= 1e9 {
        format!("{:.1} GB", n / 1e9)
    } else if n >= 1e6 {
        format!("{:.0} MB", n / 1e6)
    } else if n >= 1e3 {
        format!("{:.0} kB", n / 1e3)
    } else {
        format!("{n} B")
    }
}

pub fn ago(unix: u64) -> String {
    let secs = crate::releases::now().saturating_sub(unix);
    match secs {
        0..=59 => "just now".to_owned(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86_399 => format!("{} h ago", secs / 3600),
        _ => format!("{} days ago", secs / 86_400),
    }
}

/// `2026-10-08T14:52:50Z` → `2026-10-08`.
pub fn date(iso: &str) -> &str {
    iso.get(..10).unwrap_or(iso)
}

pub fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
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

pub fn kv(ui: &mut Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(96.0, 18.0), Sense::hover());
        theme::text_at(ui, r.left_center(), Align2::LEFT_CENTER, &key.to_uppercase(), theme::mono(11.0), theme::TEXT_FAINT);
        // Long values (paths) are cut off with an ellipsis instead of widening the panel.
        ui.add(egui::Label::new(value).truncate()).on_hover_text(value);
    });
}

pub fn dim(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(theme::TEXT_DIM));
}

/// Thin progress bar with a label, used while installing.
pub fn progress_bar(ui: &mut Ui, width: f32, fraction: Option<f32>, label: &str, accent: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 30.0), Sense::hover());
    let p = ui.painter();
    let track = Rect::from_min_size(egui::pos2(rect.left(), rect.bottom() - 5.0), vec2(rect.width(), 5.0));
    p.rect_filled(track, CornerRadius::same(3), theme::BORDER);
    let fill_w = match fraction {
        Some(f) => track.width() * f.clamp(0.0, 1.0),
        None => {
            // Indeterminate: a sliding segment.
            let t = (ui.input(|i| i.time) * 0.8).fract() as f32;
            ui.ctx().request_repaint();
            let seg = track.width() * 0.3;
            let x = track.left() + (track.width() + seg) * t - seg;
            let r = Rect::from_min_max(egui::pos2(x.max(track.left()), track.top()), egui::pos2((x + seg).min(track.right()), track.bottom()));
            p.rect_filled(r, CornerRadius::same(3), accent);
            0.0
        }
    };
    if fill_w > 0.0 {
        p.rect_filled(Rect::from_min_size(track.min, vec2(fill_w, track.height())), CornerRadius::same(3), accent);
    }
    // Clip to the bar so a long label can never run into neighbouring buttons.
    ui.painter().with_clip_rect(rect).text(
        egui::pos2(rect.left(), rect.top() + 9.0),
        Align2::LEFT_CENTER,
        label,
        theme::mono(11.0),
        theme::TEXT_DIM,
    );
}

/// Something chosen from an app's More menu, applied after the menu closes.
enum MenuAction {
    Open,
    Install,
    Check,
    ShowFolder,
    Releases,
    Details,
    Uninstall,
}

impl Launcher {
    /// Progress text and fraction for a running job. `compact` (cards) keeps the
    /// text short enough to sit beside the Cancel button.
    pub(super) fn job_label(&self, idx: usize, compact: bool) -> Option<(String, Option<f32>)> {
        let job = self.jobs[idx].as_ref()?;
        if job.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Some(("Cancelling…".to_owned(), None));
        }
        let label = match job.stage {
            crate::installer::Stage::Downloading if compact => {
                format!("Downloading · {:.0}%", job.fraction() * 100.0)
            }
            crate::installer::Stage::Downloading => format!(
                "Downloading v{} · {} of {}",
                job.version,
                bytes(job.done),
                bytes(job.total)
            ),
            _ if job.uninstalling => "Removing…".to_owned(),
            crate::installer::Stage::Unpacking => format!("Unpacking · {:.0}%", job.fraction() * 100.0),
            s => format!("{}…", s.label()),
        };
        let fraction = matches!(job.stage, crate::installer::Stage::Downloading | crate::installer::Stage::Unpacking)
            .then(|| job.fraction());
        Some((label, fraction))
    }

    /// Main buttons for an app: Open / Install / Update, or Cancel while busy.
    pub(super) fn actions(&mut self, ui: &mut Ui, idx: usize, large: bool) {
        let app = &APPS[idx];
        if self.jobs[idx].is_some() {
            let cancellable = self.jobs[idx].as_ref().is_some_and(|j| !j.uninstalling);
            if cancellable && button(ui, "Cancel", ButtonKind::Ghost, large).clicked() {
                self.cancel(idx);
            }
            return;
        }

        let installed = self.is_installed(idx);
        let update = self.update_for(idx);
        let latest = self.latest_version(idx);

        if installed {
            if button(ui, "Open", ButtonKind::Solid(app.accent), large).clicked() {
                self.launch(idx);
            }
            if let Some(v) = update {
                let label = if large { format!("Update to v{v}") } else { "Update".to_owned() };
                let resp = button(ui, &label, ButtonKind::Outline, large);
                let resp = if self.running(idx) { resp.on_hover_text(format!("Close {} to update it", app.name())) } else { resp };
                if resp.clicked() {
                    self.install(idx);
                }
            }
        } else if let Some(v) = latest {
            let label = if large { format!("Install v{v}") } else { "Install".to_owned() };
            if button(ui, &label, ButtonKind::Solid(app.accent), large).clicked() {
                self.install(idx);
            }
        } else if self.checking > 0 {
            ui.label(theme::spaced("CHECKING…", theme::mono(11.0), theme::TEXT_FAINT, 1.0));
        } else {
            let resp = button(ui, "Retry", ButtonKind::Outline, large).on_hover_text(
                self.release_errors[idx].clone().unwrap_or_else(|| "Release information isn't available".to_owned()),
            );
            if resp.clicked() {
                self.check_updates();
            }
        }
    }

    /// The More (three dots) button and its menu.
    pub(super) fn more_menu(&mut self, ui: &mut Ui, idx: usize, large: bool) {
        let resp = theme::dots_button(ui, large).on_hover_text("More actions");
        #[cfg(feature = "screenshot")]
        if large && self.demo_menu == Some(idx) {
            egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&resp));
        }
        let app = &APPS[idx];
        let managed = self.managed[idx].is_some();
        let installed = self.is_installed(idx);
        let busy = self.busy(idx);
        let running = self.running(idx);
        let choice = self.choice(idx).map(|c| (c.release.version.clone(), c.format));
        let on_details = self.ui_state.page == Page::App(idx);

        let mut action = None;
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(220.0);
            let mut item = |ui: &mut Ui, label: &str, enabled: bool, a: MenuAction| {
                if ui.add_enabled(enabled, egui::Button::new(label).frame(false)).clicked() {
                    action = Some(a);
                }
            };
            if installed {
                item(ui, "Open", !busy, MenuAction::Open);
            }
            if !on_details {
                item(ui, "Details", true, MenuAction::Details);
            }
            ui.separator();
            if let Some((v, format)) = &choice {
                let label = match &self.managed[idx] {
                    Some(m) if *v > m.version => format!("Update to v{v}"),
                    Some(m) if m.format != *format => format!("Reinstall as {}", format.label()),
                    Some(_) => format!("Reinstall v{v}"),
                    None if installed => format!("Install v{v} with the launcher"),
                    None => format!("Install v{v}"),
                };
                item(ui, &label, !busy && !(managed && running), MenuAction::Install);
            }
            item(ui, "Check for updates", self.checking == 0, MenuAction::Check);
            item(ui, "Releases on GitHub ↗", true, MenuAction::Releases);
            if managed {
                item(ui, crate::desktop::SHOW_FOLDER, true, MenuAction::ShowFolder);
                ui.separator();
                let label = if running { "Uninstall… (close the app first)" } else { "Uninstall…" };
                item(ui, label, !busy && !running, MenuAction::Uninstall);
            }
        });

        match action {
            Some(MenuAction::Open) => self.launch(idx),
            Some(MenuAction::Install) => self.install(idx),
            Some(MenuAction::Check) => self.check_updates(),
            Some(MenuAction::Releases) => self.open_url(app.releases()),
            Some(MenuAction::Details) => self.ui_state.page = Page::App(idx),
            Some(MenuAction::ShowFolder) => {
                let dir = self.base().join(app.id);
                self.show_folder(&dir);
            }
            Some(MenuAction::Uninstall) => self.dialog = Some(Dialog::Uninstall(idx)),
            None => {}
        }
    }

    /// One-line status. `compact` (cards) leaves the update to the Update button.
    pub(super) fn status_pill(&self, ui: &mut Ui, idx: usize, compact: bool) {
        let app = &APPS[idx];
        if self.running(idx) {
            theme::pill(ui, "Running", theme::OK, true);
        } else if let Some(v) = self.update_for(idx).filter(|_| !compact) {
            theme::pill(ui, &format!("v{v} available"), theme::tint(app.accent), true);
        } else if let Some(m) = &self.managed[idx] {
            theme::pill(ui, &format!("v{}", m.version), theme::TEXT_DIM, true);
        } else if self.external[idx].is_some() {
            theme::pill(ui, "Installed", theme::TEXT_DIM, true);
        } else {
            let v = self.latest_version(idx).map_or_else(|| format!("v{}", app.version), |v| format!("v{v}"));
            ui.label(theme::spaced(&v, theme::mono(11.0), theme::TEXT_FAINT, 0.6));
        }
    }
}

pub fn danger_button(ui: &mut Ui, label: &str) -> egui::Response {
    button(ui, label, ButtonKind::Solid(DANGER), true)
}
