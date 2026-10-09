//! Single-backend wgpu setup with vendor-neutral adapter selection: Vulkan,
//! or Metal on macOS, where Vulkan has no native driver.
//!
//! The adapter is chosen without regard to vendor or power class: the first
//! hardware device (in the order the system reports them) that can present to
//! the window is used. Set `LANDINGCRAFT_GPU` to a case-insensitive substring
//! of a device name to pick a specific one.

use std::sync::Arc;

use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu;

pub const GPU_ENV: &str = "LANDINGCRAFT_GPU";

#[cfg(not(target_os = "macos"))]
const BACKEND: wgpu::Backend = wgpu::Backend::Vulkan;
#[cfg(target_os = "macos")]
const BACKEND: wgpu::Backend = wgpu::Backend::Metal;

/// The graphics API in use, for display.
#[cfg(not(target_os = "macos"))]
pub const API: &str = "Vulkan";
#[cfg(target_os = "macos")]
pub const API: &str = "Metal";

/// What lists the devices, for display ("the first hardware device … reports").
#[cfg(not(target_os = "macos"))]
pub const DEVICE_SOURCE: &str = "the Vulkan loader";
#[cfg(target_os = "macos")]
pub const DEVICE_SOURCE: &str = "macOS";

#[cfg(not(target_os = "macos"))]
const NO_DEVICE: &str = "No Vulkan device that can present to this window was found. \
                         Make sure a Vulkan driver for your GPU is installed.";
#[cfg(target_os = "macos")]
const NO_DEVICE: &str = "No Metal device that can present to this window was found.";

pub fn configuration() -> WgpuConfiguration {
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = wgpu::Backends::from(BACKEND);
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
        .filter(|a| a.get_info().backend == BACKEND)
        .filter(|a| surface.is_none_or(|s| a.is_surface_supported(s)))
        .collect();

    if usable.is_empty() {
        return Err(NO_DEVICE.to_owned());
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
        eprintln!("{GPU_ENV}={wanted:?} matched no {API} device; using the default choice");
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
    eprintln!("LandingCraft: rendering with {API} on {} ({:?})", info.name, info.device_type);
    adapter.clone()
}
