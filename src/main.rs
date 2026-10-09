//! LandingCraft — a native launcher for the ArtCraft Crafting Apps.

mod app;
mod catalog;
mod detect;
mod gpu;
mod links;
mod releases;
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

    eframe::run_native(
        "LandingCraft",
        options,
        Box::new(|cc| Ok(Box::new(app::Launcher::new(cc)))),
    )
}
