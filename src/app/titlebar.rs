//! The launcher's own window frame: title bar, window buttons and resize
//! edges. Drawn by the app so the window looks and behaves the same whether or
//! not the desktop draws title bars (on Wayland, GNOME and others leave it to
//! the app).

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, Id, LayerId, Order, PointerButton, Pos2, RawInput, Rect,
    ResizeDirection, Sense, Stroke, StrokeKind, Ui, ViewportCommand, pos2, vec2,
};

use super::{Launcher, Page};
use crate::catalog::APPS;
use crate::theme;

pub const HEIGHT: f32 = 38.0;
const BUTTON_W: f32 = 46.0;
/// Thickness of the invisible resize strips along the window edges.
const EDGE: f32 = 5.0;
/// Length of each arm of the L-shaped corner zones that resize in two directions.
const CORNER: f32 = 14.0;
const CLOSE_HOVER: Color32 = Color32::from_rgb(0xd9, 0x3b, 0x3b);

#[derive(Clone, Copy)]
enum WindowButton {
    Minimize,
    Maximize,
    Close,
}

/// Once a move or resize is handed to the compositor (`StartDrag`,
/// `BeginResize`), the compositor owns the pointer and the button release
/// usually never reaches the app. egui keeps a drag alive until it sees a
/// release, so it would treat the next press as part of the old drag. Moving
/// the window again would then take two attempts, which is easy to hit after
/// snapping it to a screen edge or maximising it. This supplies the missing
/// release before the next frame.
#[derive(Default)]
pub struct Handoff {
    pending: bool,
}

impl Handoff {
    pub fn begin(&mut self) {
        self.pending = true;
    }

    pub fn patch(&mut self, raw: &mut RawInput, pos: Pos2) {
        if std::mem::take(&mut self.pending) {
            let release = egui::Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            };
            // The pointer is the compositor's now; drop the stale hover too.
            raw.events.splice(0..0, [release, egui::Event::PointerGone]);
        }
    }
}

fn maximized(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}

impl Launcher {
    pub(super) fn title_bar(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        let ctx = ui.ctx().clone();
        let is_max = maximized(&ctx);

        // The whole bar moves the window; buttons added afterwards take precedence.
        let bar = ui.interact(rect, Id::new("titlebar"), Sense::click_and_drag());
        if bar.double_clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_max));
        } else if bar.drag_started_by(PointerButton::Primary) {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            self.handoff.begin();
        }

        let title = match self.ui_state.page {
            Page::App(i) => format!("LandingCraft · {}", APPS[i.min(APPS.len() - 1)].name()),
            Page::Settings => "LandingCraft · Settings".to_owned(),
            Page::Installed => "LandingCraft · Installed".to_owned(),
            Page::All => "LandingCraft".to_owned(),
        };
        let p = ui.painter();
        p.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, theme::BORDER));
        p.text(rect.center(), Align2::CENTER_CENTER, title, theme::body(13.0), theme::TEXT_DIM);

        let mut x = rect.right();
        for button in [WindowButton::Close, WindowButton::Maximize, WindowButton::Minimize] {
            let r = Rect::from_min_max(pos2(x - BUTTON_W, rect.top()), pos2(x, rect.bottom() - 1.0));
            x -= BUTTON_W;
            let resp = ui.interact(r, Id::new(("window-button", button as u8)), Sense::click());
            let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
            let (bg, fg) = match button {
                WindowButton::Close => (CLOSE_HOVER.gamma_multiply(hover), theme::TEXT_DIM.lerp_to_gamma(Color32::WHITE, hover)),
                _ => (Color32::WHITE.gamma_multiply(0.07 * hover), theme::TEXT_DIM.lerp_to_gamma(theme::TEXT, hover)),
            };
            let p = ui.painter();
            p.rect_filled(r, CornerRadius::ZERO, bg);
            paint_glyph(p, r.center(), button, is_max, fg);
            let resp = resp.on_hover_text(match button {
                WindowButton::Minimize => "Minimize",
                WindowButton::Maximize if is_max => "Restore",
                WindowButton::Maximize => "Maximize",
                WindowButton::Close => "Close",
            });
            if resp.clicked() {
                ctx.send_viewport_cmd(match button {
                    WindowButton::Minimize => ViewportCommand::Minimized(true),
                    WindowButton::Maximize => ViewportCommand::Maximized(!is_max),
                    WindowButton::Close => ViewportCommand::Close,
                });
            }
        }
    }

    /// Invisible strips along the edges that resize the window, plus a hairline
    /// border so the window stands out against dark backgrounds. Called last, so
    /// the strips take precedence over anything underneath.
    pub(super) fn window_edges(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if maximized(&ctx) {
            return;
        }
        let r = ui.max_rect();
        ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("window-border")))
            .rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);

        use ResizeDirection as D;
        let (l, t, rt, b) = (r.left(), r.top(), r.right(), r.bottom());
        let band = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(pos2(x0, y0), pos2(x1, y1));
        // Every zone is a thin band along the border; each corner is an L of two
        // bands, so nothing inside the window (like the Close button) is covered.
        let zones = [
            (band(l, t, l + CORNER, t + EDGE), D::NorthWest, CursorIcon::ResizeNorthWest),
            (band(l, t, l + EDGE, t + CORNER), D::NorthWest, CursorIcon::ResizeNorthWest),
            (band(rt - CORNER, t, rt, t + EDGE), D::NorthEast, CursorIcon::ResizeNorthEast),
            (band(rt - EDGE, t, rt, t + CORNER), D::NorthEast, CursorIcon::ResizeNorthEast),
            (band(l, b - EDGE, l + CORNER, b), D::SouthWest, CursorIcon::ResizeSouthWest),
            (band(l, b - CORNER, l + EDGE, b), D::SouthWest, CursorIcon::ResizeSouthWest),
            (band(rt - CORNER, b - EDGE, rt, b), D::SouthEast, CursorIcon::ResizeSouthEast),
            (band(rt - EDGE, b - CORNER, rt, b), D::SouthEast, CursorIcon::ResizeSouthEast),
            (band(l + CORNER, t, rt - CORNER, t + EDGE), D::North, CursorIcon::ResizeNorth),
            (band(l + CORNER, b - EDGE, rt - CORNER, b), D::South, CursorIcon::ResizeSouth),
            (band(l, t + CORNER, l + EDGE, b - CORNER), D::West, CursorIcon::ResizeWest),
            (band(rt - EDGE, t + CORNER, rt, b - CORNER), D::East, CursorIcon::ResizeEast),
        ];
        for (i, (zone, dir, cursor)) in zones.into_iter().enumerate() {
            // Sensing clicks too makes the band own the whole press, so nothing
            // underneath (such as the Close button) can take it as a click.
            let resp = ui.interact(zone, Id::new(("resize", i)), Sense::click_and_drag());
            if !resp.hovered() {
                continue;
            }
            ctx.set_cursor_icon(cursor);
            if ctx.input(|inp| inp.pointer.primary_pressed()) {
                ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
                self.handoff.begin();
            }
        }
    }
}

fn paint_glyph(p: &egui::Painter, c: egui::Pos2, button: WindowButton, is_max: bool, color: Color32) {
    let s = Stroke::new(1.2, color);
    match button {
        WindowButton::Minimize => {
            p.hline((c.x - 5.0)..=(c.x + 5.0), c.y + 0.5, s);
        }
        WindowButton::Maximize if is_max => {
            // Restore: two overlapping squares.
            let back = Rect::from_center_size(c + vec2(1.5, -1.5), vec2(8.0, 8.0));
            let front = Rect::from_center_size(c + vec2(-1.0, 1.0), vec2(8.0, 8.0));
            p.line_segment([back.left_top() + vec2(2.0, 0.0), back.right_top()], s);
            p.line_segment([back.right_top(), back.right_bottom() - vec2(0.0, 2.0)], s);
            p.rect_stroke(front, CornerRadius::ZERO, s, StrokeKind::Middle);
        }
        WindowButton::Maximize => {
            p.rect_stroke(Rect::from_center_size(c, vec2(10.0, 10.0)), CornerRadius::ZERO, s, StrokeKind::Middle);
        }
        WindowButton::Close => {
            let d = 5.0;
            p.line_segment([c + vec2(-d, -d), c + vec2(d, d)], s);
            p.line_segment([c + vec2(-d, d), c + vec2(d, -d)], s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs one headless frame with a draggable bar and reports whether a drag started.
    fn frame(ctx: &egui::Context, t: f64, events: Vec<egui::Event>, handoff: &mut Handoff) -> bool {
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            time: Some(t),
            events,
            ..Default::default()
        };
        let pos = ctx.input(|i| i.pointer.latest_pos()).unwrap_or_default();
        handoff.patch(&mut raw, pos);
        let mut started = false;
        let mut out = ctx.run_ui(raw, |ui| {
            let bar = ui.interact(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 40.0)), Id::new("bar"), Sense::click_and_drag());
            started |= bar.drag_started_by(PointerButton::Primary);
        });
        out.textures_delta.clear(); // headless: nothing uploads the font atlas
        started
    }

    fn press(x: f32) -> egui::Event {
        egui::Event::PointerButton { pos: pos2(x, 20.0), button: PointerButton::Primary, pressed: true, modifiers: Default::default() }
    }

    fn moved(x: f32) -> egui::Event {
        egui::Event::PointerMoved(pos2(x, 20.0))
    }

    /// Drag the bar, let the "compositor" swallow the release, then try again.
    fn second_drag_starts(use_handoff: bool) -> bool {
        let ctx = egui::Context::default();
        let mut h = Handoff::default();
        frame(&ctx, 0.0, vec![moved(100.0)], &mut h);
        frame(&ctx, 0.1, vec![press(100.0)], &mut h);
        assert!(frame(&ctx, 0.2, vec![moved(140.0)], &mut h), "first drag starts");
        if use_handoff {
            h.begin(); // what title_bar() does when it sends StartDrag
        }
        // The compositor moves the window; the app sees the pointer leave and
        // come back, but never the release.
        frame(&ctx, 0.3, vec![egui::Event::PointerGone], &mut h);
        frame(&ctx, 2.0, vec![moved(100.0)], &mut h);
        frame(&ctx, 2.1, vec![press(100.0)], &mut h);
        frame(&ctx, 2.2, vec![moved(140.0)], &mut h)
    }

    #[test]
    fn handoff_lets_the_next_drag_start() {
        assert!(second_drag_starts(true));
    }

    /// Documents the egui behaviour the workaround exists for. If egui ever
    /// ends drags on its own here, this fails and Handoff can be removed.
    #[test]
    fn without_handoff_the_next_drag_is_swallowed() {
        assert!(!second_drag_starts(false));
    }

    /// Pressing the resize band over the Close button, followed by the injected
    /// release, must not count as clicking Close.
    fn close_clicked_after_edge_press(band_sense: Sense) -> bool {
        let ctx = egui::Context::default();
        let mut h = Handoff::default();
        let mut clicked = false;
        let mut run = |t: f64, events: Vec<egui::Event>, h: &mut Handoff| {
            let mut raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
                time: Some(t),
                events,
                ..Default::default()
            };
            let pos = ctx.input(|i| i.pointer.latest_pos()).unwrap_or_default();
            h.patch(&mut raw, pos);
            let mut out = ctx.run_ui(raw, |ui| {
                let close = ui.interact(Rect::from_min_size(pos2(354.0, 0.0), vec2(46.0, 38.0)), Id::new("close"), Sense::click());
                clicked |= close.clicked();
                ui.interact(Rect::from_min_size(pos2(386.0, 0.0), vec2(14.0, 5.0)), Id::new("band"), band_sense);
            });
            out.textures_delta.clear();
        };
        let at = pos2(395.0, 2.0);
        run(0.0, vec![egui::Event::PointerMoved(at)], &mut h);
        run(0.1, vec![egui::Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Default::default() }], &mut h);
        h.begin(); // BeginResize was sent on that press
        run(0.2, vec![], &mut h);
        run(0.3, vec![], &mut h);
        clicked
    }

    #[test]
    fn edge_press_never_clicks_the_button_underneath() {
        assert!(!close_clicked_after_edge_press(Sense::click_and_drag()));
    }
}

