//! Colours, fonts and small custom-painted widgets shared by every page.

use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Rect,
    Response, Sense, Stroke, StrokeKind, Ui, Vec2, epaint::RectShape, text::LayoutJob,
};

pub const BG: Color32 = Color32::from_rgb(0x11, 0x12, 0x16);
pub const SIDEBAR: Color32 = Color32::from_rgb(0x0b, 0x0c, 0x0f);
pub const SURFACE: Color32 = Color32::from_rgb(0x17, 0x18, 0x1d);
pub const SURFACE_HI: Color32 = Color32::from_rgb(0x1f, 0x20, 0x26);
pub const BORDER: Color32 = Color32::from_rgb(0x2a, 0x2c, 0x33);
pub const TEXT: Color32 = Color32::from_rgb(0xf3, 0xf3, 0xf5);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x9b, 0x9d, 0xa6);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x63, 0x65, 0x6e);
/// The light blue of "craft" on getartcraft.com.
pub const BRAND: Color32 = Color32::from_rgb(0x74, 0xa3, 0xff);
pub const OK: Color32 = Color32::from_rgb(0x4a, 0xde, 0x80);

pub fn heading(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("heading".into()))
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}
pub fn body(size: f32) -> FontId {
    FontId::proportional(size)
}
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::empty();
    let add = |fonts: &mut FontDefinitions, name: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add(&mut fonts, "space-grotesk-bold", include_bytes!("../assets/fonts/SpaceGrotesk-Bold.ttf"));
    add(&mut fonts, "plex-sans", include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf"));
    add(&mut fonts, "plex-sans-semibold", include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf"));
    add(&mut fonts, "plex-mono", include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf"));

    let fam = |names: &[&str]| names.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    fonts.families.insert(FontFamily::Proportional, fam(&["plex-sans", "plex-mono"]));
    fonts.families.insert(FontFamily::Monospace, fam(&["plex-mono", "plex-sans"]));
    fonts
        .families
        .insert(FontFamily::Name("heading".into()), fam(&["space-grotesk-bold", "plex-sans-semibold"]));
    fonts
        .families
        .insert(FontFamily::Name("semibold".into()), fam(&["plex-sans-semibold", "plex-sans"]));
    ctx.set_fonts(fonts);

    ctx.global_style_mut(|style| {
        let v = &mut style.visuals;
        *v = egui::Visuals::dark();
        v.panel_fill = BG;
        v.window_fill = SURFACE;
        v.extreme_bg_color = Color32::from_rgb(0x0d, 0x0e, 0x11);
        v.override_text_color = Some(TEXT);
        v.selection.bg_fill = BRAND.gamma_multiply(0.45);
        v.selection.stroke = Stroke::new(1.0, BRAND);
        v.hyperlink_color = BRAND;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, TEXT_FAINT);
        v.widgets.active.bg_stroke = Stroke::new(1.0, BRAND);
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.scroll.bar_width = 6.0;
        style.spacing.scroll.floating = true;
        style.text_styles.insert(egui::TextStyle::Body, body(14.0));
        style.text_styles.insert(egui::TextStyle::Button, mono(12.0));
        style.text_styles.insert(egui::TextStyle::Monospace, mono(13.0));
    });
}

/// Text colour that reads well on top of `bg`.
pub fn on_color(bg: Color32) -> Color32 {
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let l = 0.2126 * lin(bg.r()) + 0.7152 * lin(bg.g()) + 0.0722 * lin(bg.b());
    if l > 0.33 { Color32::from_rgb(0x10, 0x10, 0x12) } else { Color32::WHITE }
}

/// Lighter variant of an accent, for text on the dark background.
pub fn tint(c: Color32) -> Color32 {
    c.lerp_to_gamma(Color32::WHITE, 0.25)
}

pub fn spaced(text: &str, font: FontId, color: Color32, spacing: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        egui::TextFormat { font_id: font, color, extra_letter_spacing: spacing, ..Default::default() },
    );
    job
}

/// Small uppercase monospace label, like the section labels on the website.
pub fn eyebrow(ui: &mut Ui, text: &str, color: Color32) {
    let job = spaced(&text.to_uppercase(), mono(11.0), color, 1.2);
    ui.label(job);
}

/// Soft radial glow texture (white, alpha falls off from the top-centre),
/// tinted at paint time to produce per-app accent washes.
pub fn glow_texture(ctx: &egui::Context) -> egui::TextureHandle {
    const N: usize = 128;
    let mut pixels = Vec::with_capacity(N * N);
    for y in 0..N {
        for x in 0..N {
            let dx = (x as f32 + 0.5) / N as f32 - 0.5;
            let dy = (y as f32 + 0.5) / N as f32;
            let d = (dx * dx * 1.6 + dy * dy).sqrt();
            let a = (1.0 - d).clamp(0.0, 1.0).powf(2.2);
            pixels.push(Color32::from_white_alpha((a * 255.0) as u8));
        }
    }
    let image = egui::ColorImage::new([N, N], pixels);
    ctx.load_texture("glow", image, egui::TextureOptions::LINEAR)
}

pub fn paint_glow(ui: &Ui, glow: &egui::TextureHandle, rect: Rect, radius: impl Into<CornerRadius>, color: Color32) {
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    ui.painter()
        .add(RectShape::filled(rect, radius, color).with_texture(glow.id(), uv));
}

#[derive(Clone, Copy)]
pub enum ButtonKind {
    /// Filled with the given colour.
    Solid(Color32),
    /// Transparent with a border.
    Outline,
    /// Text only, highlights on hover.
    Ghost,
}

/// Website-style button: uppercase, letter-spaced monospace label in a square-ish box.
pub fn button(ui: &mut Ui, label: &str, kind: ButtonKind, large: bool) -> Response {
    let font = mono(if large { 12.5 } else { 11.0 });
    let pad = if large { Vec2::new(20.0, 13.0) } else { Vec2::new(12.0, 7.0) };

    let probe = ui
        .painter()
        .layout_job(spaced(&label.to_uppercase(), font.clone(), TEXT, 1.0));
    let size = probe.size() + pad * 2.0;
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(response.id, response.hovered());
        let pressed = response.is_pointer_button_down_on();
        let (fill, stroke, text) = match kind {
            ButtonKind::Solid(c) => {
                let fill = c.lerp_to_gamma(Color32::WHITE, 0.12 * t);
                let fill = if pressed { c.gamma_multiply(0.85) } else { fill };
                (fill, Stroke::NONE, on_color(c))
            }
            ButtonKind::Outline => (
                Color32::WHITE.gamma_multiply(0.06 * t),
                Stroke::new(1.0, BORDER.lerp_to_gamma(TEXT_DIM, t)),
                TEXT,
            ),
            ButtonKind::Ghost => (
                Color32::WHITE.gamma_multiply(0.06 * t),
                Stroke::NONE,
                TEXT_DIM.lerp_to_gamma(TEXT, t),
            ),
        };
        let r = CornerRadius::same(3);
        ui.painter().rect(rect, r, fill, stroke, StrokeKind::Inside);
        let galley = ui
            .painter()
            .layout_job(spaced(&label.to_uppercase(), font, text, 1.0));
        let pos = rect.center() - galley.size() / 2.0;
        ui.painter().galley(pos, galley, text);
    }
    response
}

/// Square "more" button drawn as three dots (no font has a reliable ⋯ glyph).
pub fn dots_button(ui: &mut Ui, large: bool) -> Response {
    let side = if large { 40.0 } else { 26.0 };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(response.id, response.hovered());
        let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&response));
        let fill = Color32::WHITE.gamma_multiply(0.06 * t.max(open as u8 as f32));
        ui.painter().rect(rect, CornerRadius::same(3), fill, Stroke::NONE, StrokeKind::Inside);
        let color = TEXT_DIM.lerp_to_gamma(TEXT, t);
        for dx in [-5.0, 0.0, 5.0] {
            ui.painter().circle_filled(rect.center() + Vec2::new(dx, 0.0), 1.6, color);
        }
    }
    response
}

/// Rounded pill with a coloured dot, used for status and stage tags.
pub fn pill(ui: &mut Ui, text: &str, color: Color32, dot: bool) -> Response {
    let font = mono(10.5);
    let galley = ui
        .painter()
        .layout_job(spaced(&text.to_uppercase(), font, color, 0.8));
    let dot_w = if dot { 12.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + 16.0 + dot_w, 22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(11), color.gamma_multiply(0.12), Stroke::new(1.0, color.gamma_multiply(0.35)), StrokeKind::Inside);
    if dot {
        p.circle_filled(egui::pos2(rect.left() + 12.0, rect.center().y), 3.0, color);
    }
    p.galley(
        egui::pos2(rect.left() + 8.0 + dot_w, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    response
}

/// Dot colours of the launcher's mark: the accents of the first seven Crafting Apps.
///
/// Fixed on purpose: the mark stands for the launcher, not the current catalog, so it
/// doesn't change as apps are added.
const LOGO_DOTS: [Color32; 7] = [
    Color32::from_rgb(0x2f, 0x7b, 0xf5),
    Color32::from_rgb(0xe7, 0x57, 0x3e),
    Color32::from_rgb(0x8b, 0x5b, 0xf6),
    Color32::from_rgb(0xf3, 0xa6, 0x17),
    Color32::from_rgb(0x13, 0xa4, 0x8a),
    Color32::from_rgb(0xe0, 0x36, 0x90),
    Color32::from_rgb(0x7a, 0xb5, 0x1b),
];

/// The launcher's mark: seven coloured dots in a ring.
pub fn paint_logo(ui: &Ui, center: egui::Pos2, radius: f32) {
    let p = ui.painter();
    p.rect_filled(
        Rect::from_center_size(center, Vec2::splat(radius * 2.0)),
        CornerRadius::same((radius * 0.45) as u8),
        SURFACE_HI,
    );
    let n = LOGO_DOTS.len() as f32;
    for (i, c) in LOGO_DOTS.iter().enumerate() {
        let a = std::f32::consts::TAU * i as f32 / n - std::f32::consts::FRAC_PI_2;
        let pos = center + Vec2::angled(a) * radius * 0.55;
        p.circle_filled(pos, radius * 0.17, *c);
    }
}

/// Window icon generated from the same design as [`paint_logo`].
pub fn window_icon() -> egui::IconData {
    const S: usize = 128;
    let mut rgba = vec![0u8; S * S * 4];
    let n = LOGO_DOTS.len() as f32;
    let dots: Vec<(f32, f32, Color32)> = LOGO_DOTS
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let a = std::f32::consts::TAU * i as f32 / n - std::f32::consts::FRAC_PI_2;
            (64.0 + a.cos() * 36.0, 64.0 + a.sin() * 36.0, *c)
        })
        .collect();
    for y in 0..S {
        for x in 0..S {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Rounded-square background (radius 28) with a 1px anti-aliased edge.
            let qx = (fx - 64.0).abs() - (64.0 - 28.0);
            let qy = (fy - 64.0).abs() - (64.0 - 28.0);
            let outside = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - 28.0;
            let bg_a = (0.5 - outside).clamp(0.0, 1.0);
            let mut col = [0x1f as f32, 0x20 as f32, 0x26 as f32];
            for &(cx, cy, c) in &dots {
                let d = (fx - cx).hypot(fy - cy) - 11.0;
                let cov = (0.5 - d).clamp(0.0, 1.0);
                col[0] += (c.r() as f32 - col[0]) * cov;
                col[1] += (c.g() as f32 - col[1]) * cov;
                col[2] += (c.b() as f32 - col[2]) * cov;
            }
            let i = (y * S + x) * 4;
            rgba[i] = col[0] as u8;
            rgba[i + 1] = col[1] as u8;
            rgba[i + 2] = col[2] as u8;
            rgba[i + 3] = (bg_a * 255.0) as u8;
        }
    }
    egui::IconData { rgba, width: S as u32, height: S as u32 }
}

/// Draws text at `pos` and returns its rect; a thin wrapper to keep painting code short.
pub fn text_at(ui: &Ui, pos: egui::Pos2, anchor: Align2, text: &str, font: FontId, color: Color32) -> Rect {
    ui.painter().text(pos, anchor, text, font, color)
}
