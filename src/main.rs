//! LandingCraft — a native launcher for the ArtCraft Crafting Apps.

// Release builds on Windows are GUI programs, so no console window opens with them.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod catalog;
mod desktop;
mod detect;
mod gpu;
mod installer;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(all(unix, not(target_os = "macos")))]
mod portal;
mod releases;
#[cfg(feature = "screenshot")]
mod screenshot;
mod settings;
mod theme;
#[cfg(windows)]
mod win;

use eframe::egui;

fn main() -> eframe::Result {
    // The launcher draws its own title bar unless the user asked for the desktop's.
    let native_title_bar = settings::Settings::load().native_title_bar;

    let viewport = egui::ViewportBuilder::default()
        .with_title("LandingCraft")
        .with_app_id("landingcraft")
        .with_inner_size([1240.0, 820.0])
        .with_min_inner_size([760.0, 560.0])
        .with_icon(theme::window_icon());
    // The window frame stays on, so the system keeps the traffic-light buttons,
    // resizing and the shadow. The launcher's own bar is drawn underneath them,
    // in place of the system title bar.
    #[cfg(target_os = "macos")]
    let viewport = viewport
        .with_fullsize_content_view(!native_title_bar)
        .with_titlebar_shown(native_title_bar)
        .with_title_shown(native_title_bar);
    #[cfg(not(target_os = "macos"))]
    let viewport = viewport.with_decorations(native_title_bar);

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: gpu::configuration(),
        ..Default::default()
    };
    #[cfg(feature = "screenshot")]
    let options = match screenshot::window_height() {
        Some(h) => eframe::NativeOptions { viewport: options.viewport.with_inner_size([1240.0, h]), ..options },
        None => options,
    };

    let result = eframe::run_native(
        "LandingCraft",
        options,
        Box::new(|cc| Ok(Box::new(app::Launcher::new(cc)))),
    );
    #[cfg(windows)]
    if let Err(e) = &result {
        match e {
            eframe::Error::Wgpu(_) => win::startup_error(&format!("{}

({e})", gpu::NO_DEVICE)),
            _ => win::startup_error(&e.to_string()),
        }
    }
    result
}
