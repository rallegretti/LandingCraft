//! Confirmation dialogs.

use eframe::egui::{self, RichText, Ui};

use super::widgets::{bytes, danger_button, dim};
use super::{Dialog, Launcher};
use crate::catalog::APPS;
use crate::settings::display_path;
use crate::theme::{self, ButtonKind, button};

enum Choice {
    Confirm,
    Cancel,
}

impl Launcher {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &self.dialog else { return };
        let mut choice = None;
        let busy_moving = matches!(dialog, Dialog::Move { progress: Some(_), .. });

        let modal = egui::Modal::new(egui::Id::new("dialog"))
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER))
                    .corner_radius(14)
                    .inner_margin(26),
            )
            .show(ctx, |ui| {
                ui.set_width(440.0);
                match dialog {
                    Dialog::Uninstall(idx) => {
                        let app = &APPS[*idx];
                        let version = self.managed[*idx].as_ref().map(|m| format!(" v{}", m.version)).unwrap_or_default();
                        ui.label(RichText::new(format!("Uninstall {}{version}?", app.name())).font(theme::heading(22.0)));
                        ui.add_space(6.0);
                        dim(
                            ui,
                            format!(
                                "This deletes {} ({}). Documents you made with {} are not touched.",
                                display_path(&self.base().join(app.id)),
                                bytes(self.managed_size[*idx]),
                                app.name()
                            ),
                        );
                        ui.add_space(18.0);
                        buttons(ui, &mut choice, |ui| danger_button(ui, "Uninstall"));
                    }
                    Dialog::Move { to, apps, bytes: size, progress } => {
                        let n = apps.len();
                        ui.label(RichText::new("Move installed apps?").font(theme::heading(22.0)));
                        ui.add_space(6.0);
                        dim(
                            ui,
                            format!(
                                "{n} app{} ({}) will move from {} to {}. Moving to another drive copies the files, \
                                 which can take a minute.",
                                if n == 1 { "" } else { "s" },
                                bytes(*size),
                                display_path(self.base()),
                                display_path(to)
                            ),
                        );
                        ui.add_space(18.0);
                        match progress {
                            Some(done) => {
                                let name = apps.get(*done).map(|&i| APPS[i].name()).unwrap_or_default();
                                let label = format!("Moving {name} ({} of {n})…", (done + 1).min(n));
                                super::widgets::progress_bar(ui, 440.0, None, &label, theme::BRAND);
                            }
                            None => buttons(ui, &mut choice, |ui| {
                                button(ui, "Move apps", ButtonKind::Solid(theme::BRAND), true)
                            }),
                        }
                    }
                }
            });

        if modal.should_close() && !busy_moving {
            choice = Some(Choice::Cancel);
        }
        match (choice, &self.dialog) {
            (Some(Choice::Cancel), _) => self.dialog = None,
            (Some(Choice::Confirm), Some(Dialog::Uninstall(idx))) => {
                let idx = *idx;
                self.dialog = None;
                self.uninstall(idx);
            }
            (Some(Choice::Confirm), Some(Dialog::Move { .. })) => self.start_move(),
            _ => {}
        }
    }
}

fn buttons(ui: &mut Ui, choice: &mut Option<Choice>, confirm: impl FnOnce(&mut Ui) -> egui::Response) {
    ui.horizontal(|ui| {
        if confirm(ui).clicked() {
            *choice = Some(Choice::Confirm);
        }
        if button(ui, "Cancel", ButtonKind::Ghost, true).clicked() {
            *choice = Some(Choice::Cancel);
        }
    });
}
