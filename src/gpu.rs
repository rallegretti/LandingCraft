//! Vulkan-only wgpu setup with vendor-neutral adapter selection.
//!
//! The adapter is chosen without regard to vendor or power class: the first
//! hardware Vulkan device (in the order the system Vulkan loader reports them)
//! that can present to the window is used. Set `LANDINGCRAFT_GPU` to a
//! case-insensitive substring of a device name to pick a specific one.

use std::sync::Arc;

use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu;

pub const GPU_ENV: &str = "LANDINGCRAFT_GPU";

pub fn configuration() -> WgpuConfiguration {
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = wgpu::Backends::VULKAN;
    setup.power_preference = wgpu::PowerPreference::None;
    setup.native_adapter_selector = Some(Arc::new(select_adapter));

    WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

fn select_adapter(
    adapters: &[wgpu::Adapter],
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, String> {
    let usable: Vec<&wgpu::Adapter> = adapters
        .iter()
        .filter(|a| a.get_info().backend == wgpu::Backend::Vulkan)
        .filter(|a| surface.is_none_or(|s| a.is_surface_supported(s)))
        .collect();

    if usable.is_empty() {
        return Err("No Vulkan device that can present to this window was found. \
                    Make sure a Vulkan driver for your GPU is installed."
            .to_owned());
    }

    if let Ok(wanted) = std::env::var(GPU_ENV)
        && !wanted.trim().is_empty()
    {
        let wanted = wanted.to_lowercase();
        if let Some(a) = usable
            .iter()
            .find(|a| a.get_info().name.to_lowercase().contains(&wanted))
        {
            return Ok(chosen(a));
        }
        eprintln!("{GPU_ENV}={wanted:?} matched no Vulkan device; using the default choice");
    }

    // Prefer real hardware over software rasterisers (lavapipe, SwiftShader),
    // but otherwise keep the loader's order.
    let pick = usable
        .iter()
        .find(|a| a.get_info().device_type != wgpu::DeviceType::Cpu)
        .unwrap_or(&usable[0]);
    Ok(chosen(pick))
}

fn chosen(adapter: &wgpu::Adapter) -> wgpu::Adapter {
    let info = adapter.get_info();
    eprintln!("LandingCraft: rendering with Vulkan on {} ({:?})", info.name, info.device_type);
    adapter.clone()
}
