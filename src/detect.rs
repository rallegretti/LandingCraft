//! Finding Crafting Apps installed outside the launcher, and launching apps.

use std::collections::HashMap;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::settings::home;

#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// A file with an execute bit. On macOS, an `.app` bundle with a findable
/// executable counts too. On Windows, an `.exe` file.
pub fn is_executable(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    if crate::macos::is_bundle(path) {
        return crate::macos::bundle_executable(path).is_some();
    }
    #[cfg(unix)]
    let runnable = |m: &std::fs::Metadata| m.permissions().mode() & 0o111 != 0;
    #[cfg(windows)]
    let runnable = |_: &std::fs::Metadata| path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe"));
    #[cfg(not(any(unix, windows)))]
    let runnable = |_: &std::fs::Metadata| true;
    path.metadata().map(|m| m.is_file() && runnable(&m)).unwrap_or(false)
}

/// The file name of an app's executable: `<id>`, or `<id>.exe` on Windows.
#[cfg(not(target_os = "macos"))]
fn exe_name(id: &str) -> String {
    format!("{id}{}", std::env::consts::EXE_SUFFIX)
}

/// macOS: look for `<id>.app` (any case) in `/Applications` and `~/Applications`.
#[cfg(target_os = "macos")]
pub fn find(id: &str) -> Option<PathBuf> {
    let wanted = format!("{id}.app");
    [PathBuf::from("/Applications"), home().join("Applications")].iter().find_map(|dir| {
        std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(&wanted))
            .map(|e| e.path())
            .find(|p| is_executable(p))
    })
}

/// Directories that might contain an AppImage or an unpacked release.
#[cfg(all(not(target_os = "macos"), not(windows)))]
fn extra_dirs() -> Vec<PathBuf> {
    let h = home();
    let mut dirs: Vec<PathBuf> = [".local/bin", ".cargo/bin", "Applications", "AppImages", "Downloads", "bin"]
        .iter()
        .map(|sub| h.join(sub))
        .collect();
    dirs.push(PathBuf::from("/opt"));
    dirs
}

/// Directories that might contain an app installed from its MSI (Program Files)
/// or an unzipped portable release.
#[cfg(windows)]
fn extra_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
        .iter()
        .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
        .collect();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Programs"));
    }
    let h = home();
    dirs.extend(["Downloads", "Desktop", "Apps", "bin", ".cargo/bin"].iter().map(|sub| h.join(sub)));
    dirs.dedup();
    dirs
}

/// Look for an app installed by other means. Order: `$PATH`, well-known user
/// directories, `/opt/<id>/`, then `<id>-*.AppImage` files or unpacked
/// `<id>-*` release folders in the same directories. On Windows the well-known
/// places are Program Files (`<App>\<id>.exe`, where the MSI puts it) and the
/// user's Downloads, Desktop and Apps folders, and the executable is `<id>.exe`.
#[cfg(not(target_os = "macos"))]
pub fn find(id: &str) -> Option<PathBuf> {
    let path_dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();

    let dirs: Vec<PathBuf> = path_dirs.into_iter().chain(extra_dirs()).collect();

    let exe = exe_name(id);
    for dir in &dirs {
        for candidate in [dir.join(&exe), dir.join(id).join(&exe), dir.join(id).join("bin").join(&exe)] {
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }

    // Versioned AppImages and release folders: pick the most recently modified match.
    let prefix = format!("{id}-");
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for dir in &dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !name.starts_with(&prefix) && name != format!("{id}.appimage") {
                continue;
            }
            let path = entry.path();
            let found = if path.is_dir() {
                // Explorer's "Extract all" nests the zip's folder in one of the same name.
                [path.join(&exe), path.join("bin").join(&exe), path.join(entry.file_name()).join(&exe)]
                    .into_iter()
                    .find(|p| is_executable(p))
            } else if name.ends_with(".appimage") && is_executable(&path) {
                Some(path)
            } else {
                None
            };
            let Some(found) = found else { continue };
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().is_none_or(|(t, _)| modified > *t) {
                best = Some((modified, found));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Classic AppImages mount themselves with libfuse 2. Without it they can still
/// run by extracting to a temporary folder first (slower to start).
#[cfg(all(unix, not(target_os = "macos")))]
pub fn has_fuse2() -> bool {
    [
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/lib/x86_64-linux-gnu",
        "/lib/aarch64-linux-gnu",
        "/usr/lib64",
        "/usr/lib",
        "/lib64",
        "/lib",
    ]
    .iter()
    .any(|dir| Path::new(dir).join("libfuse.so.2").exists())
}

/// Tracks child processes started by the launcher so cards can show "Running".
#[derive(Default)]
pub struct Processes {
    children: HashMap<&'static str, Vec<Child>>,
}

impl Processes {
    pub fn launch(&mut self, id: &'static str, exe: &Path) -> std::io::Result<()> {
        // An `.app` bundle starts its executable directly, so it can be tracked like any other child.
        #[cfg(target_os = "macos")]
        let exe = &crate::macos::launch_target(exe);
        let mut cmd = Command::new(exe);
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).current_dir(home());
        // Own process group, so closing the launcher doesn't take the app down with it.
        #[cfg(unix)]
        cmd.process_group(0);
        // Likewise on Windows, where a console's Ctrl+C would otherwise reach it too.
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let is_appimage = exe.extension().is_some_and(|e| e.eq_ignore_ascii_case("appimage"));
            if is_appimage && !has_fuse2() {
                cmd.env("APPIMAGE_EXTRACT_AND_RUN", "1");
            }
        }
        let child = cmd.spawn()?;
        self.children.entry(id).or_default().push(child);
        Ok(())
    }

    /// Forget children that have exited.
    pub fn poll(&mut self) {
        for list in self.children.values_mut() {
            list.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
        }
    }

    pub fn running(&self, id: &str) -> usize {
        self.children.get(id).map_or(0, Vec::len)
    }

    pub fn any_running(&self) -> bool {
        self.children.values().any(|v| !v.is_empty())
    }
}
