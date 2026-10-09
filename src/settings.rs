//! Launcher settings, stored as JSON in `$XDG_CONFIG_HOME/landingcraft/settings.json`
//! (on macOS, `~/Library/Application Support/landingcraft/settings.json`; on Windows,
//! `%APPDATA%\landingcraft\settings.json`).
//!
//! These are written immediately on every change (not on exit like the UI state),
//! because the install location must never get out of step with where apps are.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// `<app>-<version>-linux-<arch>.tar.gz`, unpacked into the install folder.
    Tarball,
    /// `<app>-<version>-linux-<arch>.AppImage`, kept as a single executable file.
    AppImage,
    /// `<app>-<version>-macos-universal.dmg`, whose `.app` bundle is copied out.
    Dmg,
    /// `<app>-<version>-windows-<arch>-portable.zip`, unpacked into the install folder.
    Zip,
    /// `<app>-<version>-windows-<arch>.msi`, run by Windows Installer, which puts the app in
    /// Program Files.
    Msi,
}

impl Default for Format {
    fn default() -> Self {
        Format::available().first().copied().unwrap_or(Format::Tarball)
    }
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Tarball => "Tarball",
            Format::AppImage => "AppImage",
            Format::Dmg => "Disk image",
            Format::Zip => "Archive",
            Format::Msi => "MSI",
        }
    }

    /// The formats the Crafting Apps publish for this OS, the default first.
    /// Empty where the launcher can't install apps yet.
    pub fn available() -> &'static [Format] {
        if cfg!(target_os = "macos") {
            &[Format::Dmg]
        } else if cfg!(windows) {
            &[Format::Msi, Format::Zip]
        } else if cfg!(unix) {
            &[Format::Tarball, Format::AppImage]
        } else {
            &[]
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Settings {
    /// Folder that holds every app the launcher manages, one subfolder per app.
    pub install_dir: PathBuf,
    /// Package format used for new installs and updates.
    pub format: Format,
    pub check_on_startup: bool,
    /// Let the desktop draw the title bar instead of the launcher. Only useful
    /// where the desktop draws title bars for apps; read once at startup.
    pub native_title_bar: bool,
    /// User-provided executable per app id, used when the launcher doesn't manage the app.
    pub custom_paths: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            install_dir: default_install_dir(),
            format: Format::default(),
            check_on_startup: true,
            native_title_bar: false,
            custom_paths: HashMap::new(),
        }
    }
}

pub fn home() -> PathBuf {
    // On Windows, USERPROFILE first: shells such as Git Bash set HOME too, not always to the profile.
    let vars = if cfg!(windows) { ["USERPROFILE", "HOME"] } else { ["HOME", "USERPROFILE"] };
    vars.iter()
        .find_map(|v| std::env::var_os(v).filter(|p| !p.is_empty()))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// `~/.craftapps`. On macOS, `~/Applications/Crafting Apps` instead, so that
/// Spotlight and Launchpad find the installed apps. On Windows,
/// `%LOCALAPPDATA%\Programs\Crafting Apps`, where per-user programs go.
pub fn default_install_dir() -> PathBuf {
    #[cfg(windows)]
    return local_app_data().join("Programs").join("Crafting Apps");
    #[cfg(not(windows))]
    if cfg!(target_os = "macos") {
        home().join("Applications").join("Crafting Apps")
    } else {
        home().join(".craftapps")
    }
}

/// `%LOCALAPPDATA%` (Windows), falling back to its usual place in the profile.
#[cfg(windows)]
fn local_app_data() -> PathBuf {
    known_folder("LOCALAPPDATA", r"AppData\Local")
}

#[cfg(windows)]
fn known_folder(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(fallback))
}

#[cfg(all(not(target_os = "macos"), not(windows)))]
fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(fallback))
}

#[cfg(all(not(target_os = "macos"), not(windows)))]
pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("landingcraft")
}

#[cfg(all(not(target_os = "macos"), not(windows)))]
pub fn cache_dir() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache").join("landingcraft")
}

/// `%APPDATA%\landingcraft`, which roams with the user's profile.
#[cfg(windows)]
pub fn config_dir() -> PathBuf {
    known_folder("APPDATA", r"AppData\Roaming").join("landingcraft")
}

/// `%LOCALAPPDATA%\landingcraft`, which stays on this machine.
#[cfg(windows)]
pub fn cache_dir() -> PathBuf {
    local_app_data().join("landingcraft")
}

#[cfg(target_os = "macos")]
pub fn config_dir() -> PathBuf {
    home().join("Library/Application Support/landingcraft")
}

#[cfg(target_os = "macos")]
pub fn cache_dir() -> PathBuf {
    home().join("Library/Caches/landingcraft")
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read(settings_path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        write_atomic(&settings_path(), &json)
    }
}

/// Write via a temporary file and rename, so readers never see a half-written file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// `~`-relative form of a path for display.
pub fn display_path(path: &Path) -> String {
    match path.strip_prefix(home()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Ok(rest) => Path::new("~").join(rest).display().to_string(),
        Err(_) => path.display().to_string(),
    }
}

/// Expand a leading `~` and require an absolute path.
pub fn parse_user_path(raw: &str) -> Option<PathBuf> {
    let raw = raw.trim();
    let tilde = raw.strip_prefix("~/").or_else(|| if cfg!(windows) { raw.strip_prefix("~\\") } else { None });
    let path = if raw == "~" {
        home()
    } else if let Some(rest) = tilde {
        home().join(rest)
    } else {
        PathBuf::from(raw)
    };
    path.is_absolute().then_some(path)
}

/// An example absolute path, for hints.
pub const EXAMPLE_DIR: &str = if cfg!(windows) { r"D:\Apps\craft" } else { "~/Apps/craft" };
