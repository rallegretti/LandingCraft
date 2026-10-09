//! Finding installed Crafting Apps on disk and launching them.

use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn is_executable(path: &Path) -> bool {
    path.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Directories that might contain an AppImage or an unpacked release.
fn extra_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(h) = home() {
        for sub in [
            ".local/bin",
            ".cargo/bin",
            "Applications",
            "AppImages",
            ".local/share/applications/appimages",
            "Downloads",
            "bin",
        ] {
            dirs.push(h.join(sub));
        }
    }
    dirs.push(PathBuf::from("/opt"));
    dirs
}

/// Look for an app's executable. Order: `$PATH`, well-known user directories,
/// `/opt/<id>/`, then `<id>-*.AppImage` files or unpacked `<id>-*` release
/// folders in the same directories.
pub fn find(id: &str) -> Option<PathBuf> {
    let path_dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();

    let dirs: Vec<PathBuf> = path_dirs.into_iter().chain(extra_dirs()).collect();

    for dir in &dirs {
        for candidate in [dir.join(id), dir.join(id).join(id), dir.join(id).join("bin").join(id)] {
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
            let exe = if path.is_dir() {
                [path.join(id), path.join("bin").join(id)]
                    .into_iter()
                    .find(|p| is_executable(p))
            } else if name.ends_with(".appimage") && is_executable(&path) {
                Some(path)
            } else {
                None
            };
            let Some(exe) = exe else { continue };
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().is_none_or(|(t, _)| modified > *t) {
                best = Some((modified, exe));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Tracks child processes started by the launcher so cards can show "Running".
#[derive(Default)]
pub struct Processes {
    children: HashMap<&'static str, Vec<Child>>,
}

impl Processes {
    pub fn launch(&mut self, id: &'static str, exe: &Path) -> std::io::Result<()> {
        let mut cmd = Command::new(exe);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // Own process group, so closing the launcher doesn't take the app down with it.
            .process_group(0);
        if let Some(dir) = home() {
            cmd.current_dir(dir);
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
}

pub fn display_path(path: &Path) -> String {
    if let Some(h) = home()
        && let Ok(rest) = path.strip_prefix(&h)
    {
        return Path::new("~").join(rest).display().to_string();
    }
    path.display().to_string()
}
