//! LandingCraft — a native launcher for the ArtCraft Crafting Apps.

mod app;
mod catalog;
mod detect;
mod gpu;
mod installer;
mod portal;
mod releases;
#[cfg(feature = "screenshot")]
mod screenshot;
mod settings;
mod theme;

use eframe::egui;

fn main() -> eframe::Result {
    let accents: Vec<egui::Color32> = catalog::APPS.iter().map(|a| a.accent).collect();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LandingCraft")
            .with_app_id("landingcraft")
            .with_inner_size([1240.0, 820.0])
            .with_min_inner_size([760.0, 560.0])
            .with_icon(theme::window_icon(&accents)),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: gpu::configuration(),
        ..Default::default()
    };
    #[cfg(feature = "screenshot")]
    let options = match screenshot::window_height() {
        Some(h) => eframe::NativeOptions { viewport: options.viewport.with_inner_size([1240.0, h]), ..options },
        None => options,
    };

    eframe::run_native(
        "LandingCraft",
        options,
        Box::new(|cc| Ok(Box::new(app::Launcher::new(cc)))),
    )
}
