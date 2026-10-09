//! Desktop integration, one implementation per platform: opening web links,
//! choosing a folder, and showing a folder in the file manager. The XDG
//! Desktop Portal over D-Bus on Linux (`portal.rs`), Cocoa on macOS (`macos.rs`),
//! the Windows shell on Windows (`win.rs`).

use std::path::{Path, PathBuf};

/// Label for the button that shows a folder.
pub const SHOW_FOLDER: &str = if cfg!(target_os = "macos") {
    "Show in Finder"
} else if cfg!(windows) {
    "Show in Explorer"
} else {
    "Show in file manager"
};
/// The file manager, for messages ("Couldn't open …").
pub const FILE_MANAGER: &str = if cfg!(target_os = "macos") {
    "Finder"
} else if cfg!(windows) {
    "File Explorer"
} else {
    "the file manager"
};

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    use super::*;

    pub fn open_uri(url: &str) -> Result<(), String> {
        crate::portal::open_uri(url).map_err(|e| e.to_string())
    }

    pub fn pick_folder(title: &str, start: &Path) -> Result<Option<PathBuf>, String> {
        crate::portal::pick_folder(title, start).map_err(|e| e.to_string())
    }

    pub fn show_folder(path: &Path) -> Result<(), String> {
        crate::portal::show_folder(path).map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "macos")]
use crate::macos as imp;

#[cfg(windows)]
use crate::win as imp;

/// Not implemented elsewhere yet. Callers fall back to copying the link or path.
#[cfg(not(any(unix, windows)))]
mod imp {
    use super::*;

    const UNSUPPORTED: &str = "not supported on this platform yet";

    pub fn open_uri(_url: &str) -> Result<(), String> {
        Err(UNSUPPORTED.to_owned())
    }

    pub fn pick_folder(_title: &str, _start: &Path) -> Result<Option<PathBuf>, String> {
        Err(UNSUPPORTED.to_owned())
    }

    pub fn show_folder(_path: &Path) -> Result<(), String> {
        Err(UNSUPPORTED.to_owned())
    }
}

/// Ask the desktop to open a web link.
pub fn open_uri(url: &str) -> Result<(), String> {
    imp::open_uri(url)
}

/// Show the desktop's folder chooser. Blocks until the user picks a folder
/// (`Ok(Some)`) or cancels (`Ok(None)`), so call it from a worker thread.
pub fn pick_folder(title: &str, start: &Path) -> Result<Option<PathBuf>, String> {
    imp::pick_folder(title, start)
}

/// Open a folder in the file manager.
pub fn show_folder(path: &Path) -> Result<(), String> {
    imp::show_folder(path)
}
