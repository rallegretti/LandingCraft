//! Development aid (cargo feature `screenshot`): with
//! `LANDINGCRAFT_SCREENSHOT=<file.png>` set, the launcher saves an image of its
//! own window after `LANDINGCRAFT_SCREENSHOT_DELAY` seconds (default 4) and quits.
//! `LANDINGCRAFT_SCREENSHOT_HEIGHT` opens a taller window, to capture more of a page.
//! `LANDINGCRAFT_DEMO=<action>:<arg>` (see `Launcher::demo_action`) sets up a state to capture.

pub fn window_height() -> Option<f32> {
    std::env::var("LANDINGCRAFT_SCREENSHOT_HEIGHT").ok().and_then(|h| h.parse().ok())
}

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;

pub fn tick(ctx: &egui::Context) {
    let Some(path) = std::env::var_os("LANDINGCRAFT_SCREENSHOT") else { return };
    static REQUESTED: AtomicBool = AtomicBool::new(false);
    let delay: f64 = std::env::var("LANDINGCRAFT_SCREENSHOT_DELAY")
        .ok()
        .and_then(|d| d.parse().ok())
        .unwrap_or(4.0);

    if ctx.input(|i| i.time) > delay && !REQUESTED.swap(true, Ordering::Relaxed) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
    }
    let shots: Vec<_> = ctx.input(|i| {
        i.raw
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
            .collect()
    });
    for img in shots {
        let [w, h] = img.size;
        let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
            if let Err(e) = buf.save(&path) {
                eprintln!("screenshot: {e}");
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}
