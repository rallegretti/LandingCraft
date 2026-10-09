//! macOS integration: opening links, choosing folders and showing folders in
//! Finder through Cocoa (the objc2 bindings winit already uses), plus `.app`
//! bundles and the `.dmg` disk images the Crafting Apps ship in.
//!
//! Disk images are mounted with `hdiutil` and the bundle is copied out with
//! `ditto`. Both ship with macOS; nothing else on this platform starts a
//! helper program.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use dispatch2::run_on_main;
use objc2_app_kit::{NSModalResponse, NSModalResponseOK, NSOpenPanel, NSWorkspace};
use objc2_foundation::{NSString, NSURL, NSUserDefaults};

// ------------------------------------------------------------------ desktop

/// Ask the system to open a web link in the default browser.
pub fn open_uri(url: &str) -> Result<(), String> {
    let url = NSURL::URLWithString(&NSString::from_str(url)).ok_or("not a valid link")?;
    if NSWorkspace::sharedWorkspace().openURL(&url) { Ok(()) } else { Err("no app could open it".to_owned()) }
}

/// Open a folder in Finder.
pub fn show_folder(path: &Path) -> Result<(), String> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    if NSWorkspace::sharedWorkspace().openURL(&url) { Ok(()) } else { Err("Finder couldn't open it".to_owned()) }
}

/// Show the standard folder chooser. Blocks until the user picks a folder
/// (`Ok(Some)`) or cancels (`Ok(None)`), so call it off the main thread.
///
/// The panel must be created on the main thread. It's shown modeless, with a
/// completion handler, so it never runs a nested event loop inside winit's.
pub fn pick_folder(title: &str, start: &Path) -> Result<Option<PathBuf>, String> {
    let (tx, rx) = mpsc::channel();
    let (title, start) = (title.to_owned(), start.to_string_lossy().into_owned());
    run_on_main(move |mtm| {
        let panel = NSOpenPanel::openPanel(mtm);
        panel.setCanChooseDirectories(true);
        panel.setCanChooseFiles(false);
        panel.setAllowsMultipleSelection(false);
        panel.setCanCreateDirectories(true);
        panel.setMessage(Some(&NSString::from_str(&title)));
        panel.setPrompt(Some(&NSString::from_str("Choose")));
        panel.setDirectoryURL(Some(&NSURL::fileURLWithPath(&NSString::from_str(&start))));
        let chooser = panel.clone();
        let done = RcBlock::new(move |response: NSModalResponse| {
            let picked = (response == NSModalResponseOK)
                .then(|| chooser.URLs().firstObject())
                .flatten()
                .and_then(|url| url.path())
                .map(|p| PathBuf::from(p.to_string()));
            let _ = tx.send(picked);
        });
        panel.beginWithCompletionHandler(&done);
    });
    rx.recv().map_err(|_| "the folder chooser closed unexpectedly".to_owned())
}

/// What double-clicking a title bar does, as set in System Settings › Desktop & Dock.
pub enum TitleDoubleClick {
    Zoom,
    Minimize,
    Nothing,
}

pub fn title_double_click() -> TitleDoubleClick {
    let action = NSUserDefaults::standardUserDefaults().stringForKey(&NSString::from_str("AppleActionOnDoubleClick"));
    match action.map(|a| a.to_string()).as_deref() {
        Some("Minimize") => TitleDoubleClick::Minimize,
        Some("None") => TitleDoubleClick::Nothing,
        _ => TitleDoubleClick::Zoom,
    }
}

// ------------------------------------------------------------------ bundles

pub fn is_bundle(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("app")) && path.is_dir()
}

/// The executable inside an `.app` bundle, relative to the bundle: the one
/// `Info.plist` names, or else the only one in `Contents/MacOS`.
pub fn bundle_executable(app: &Path) -> Option<PathBuf> {
    let dir = app.join("Contents").join("MacOS");
    let names: Vec<String> = fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter(|e| crate::detect::is_executable(&e.path()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let declared = fs::read_to_string(app.join("Contents").join("Info.plist"))
        .ok()
        .and_then(|plist| plist_string(&plist, "CFBundleExecutable"));
    let name = match declared {
        Some(d) if names.contains(&d) => d,
        _ if names.len() == 1 => names.into_iter().next()?,
        _ => return None,
    };
    Some(Path::new("Contents").join("MacOS").join(name))
}

/// The program to start for `path`: a bundle's executable, or `path` itself.
pub fn launch_target(path: &Path) -> PathBuf {
    match is_bundle(path).then(|| bundle_executable(path)).flatten() {
        Some(exe) => path.join(exe),
        None => path.to_path_buf(),
    }
}

/// A `<string>` value from an XML property list. (Binary plists return `None`;
/// `bundle_executable` then falls back to the folder's contents.)
fn plist_string(xml: &str, key: &str) -> Option<String> {
    let after_key = &xml[xml.find(&format!("<key>{key}</key>"))?..];
    let value = after_key[after_key.find("<string>")? + "<string>".len()..].split("</string>").next()?;
    let value = value.trim();
    (!value.is_empty() && !value.contains(['<', '/'])).then(|| value.to_owned())
}

// --------------------------------------------------------------- disk images

/// Mount a disk image read-only at `mount` (an existing empty folder), out of
/// sight: not shown in Finder or on the desktop. The checksum has already been
/// verified, so hdiutil's own verification pass is skipped.
pub fn attach_dmg(image: &Path, mount: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let mut cmd = Command::new("/usr/bin/hdiutil");
    cmd.args(["attach", "-readonly", "-nobrowse", "-noautoopen", "-noverify", "-quiet", "-mountpoint"])
        .arg(mount)
        .arg(image);
    run(&mut cmd, cancel).map_err(|e| explain("couldn't open the disk image", e))
}

/// Unmount a disk image mounted by `attach_dmg`, forcibly if it's busy.
pub fn detach_dmg(mount: &Path) {
    let never = AtomicBool::new(false);
    for force in [false, true] {
        let mut cmd = Command::new("/usr/bin/hdiutil");
        cmd.args(["detach", "-quiet"]);
        if force {
            cmd.arg("-force");
        }
        if run(cmd.arg(mount), &never).is_ok() {
            return;
        }
    }
}

/// The app bundle at the top of a mounted disk image: `<id>.app` (any case),
/// or the only bundle there. Symlinks are ignored.
pub fn find_bundle(mount: &Path, id: &str) -> Option<PathBuf> {
    let bundles: Vec<PathBuf> = fs::read_dir(mount)
        .ok()?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .filter(|p| is_bundle(p))
        .collect();
    let named = bundles
        .iter()
        .find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(id)));
    match named {
        Some(p) => Some(p.clone()),
        None if bundles.len() == 1 => bundles.into_iter().next(),
        None => None,
    }
}

/// Copy a bundle exactly (links, permissions, extended attributes and the
/// code signature) with `ditto`, the copier macOS itself uses for bundles.
pub fn copy_bundle(from: &Path, to: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let mut cmd = Command::new("/usr/bin/ditto");
    cmd.arg(from).arg(to);
    run(&mut cmd, cancel).map_err(|e| explain("couldn't copy the app out of the disk image", e))
}

/// Prefix a tool's error with what was being done, leaving a cancellation as is.
fn explain(doing: &str, err: String) -> String {
    if err == crate::installer::CANCELLED { err } else { format!("{doing}: {err}") }
}

/// Run a system tool to completion, killing it if `cancel` is set. On failure,
/// returns the tool's own message.
fn run(cmd: &mut Command, cancel: &AtomicBool) -> Result<(), String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(crate::installer::CANCELLED.to_owned());
        }
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    if status.success() {
        return Ok(());
    }
    let mut err = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = std::io::Read::read_to_string(&mut stderr, &mut err);
    }
    let err = err.trim();
    Err(if err.is_empty() { status.to_string() } else { err.to_owned() })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>PdfCraft</string>
	<key>CFBundleExecutable</key>
	<string>pdfcraft-bin</string>
</dict>
</plist>"#;

    #[test]
    fn reads_plist_strings() {
        assert_eq!(plist_string(PLIST, "CFBundleExecutable").as_deref(), Some("pdfcraft-bin"));
        assert_eq!(plist_string(PLIST, "CFBundleName").as_deref(), Some("PdfCraft"));
        assert_eq!(plist_string(PLIST, "CFBundleIdentifier"), None);
        assert_eq!(plist_string("bplist00\u{1}\u{2}", "CFBundleExecutable"), None);
    }

    /// A minimal bundle: `<dir>/<name>.app` with the given executables.
    fn fake_bundle(dir: &Path, name: &str, exes: &[&str], plist: Option<&str>) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let app = dir.join(format!("{name}.app"));
        let macos = app.join("Contents").join("MacOS");
        fs::create_dir_all(&macos).unwrap();
        for exe in exes {
            fs::write(macos.join(exe), "#!/bin/sh\nsleep 0.2\n").unwrap();
            fs::set_permissions(macos.join(exe), fs::Permissions::from_mode(0o755)).unwrap();
        }
        fs::write(macos.join("notes.txt"), "not executable").unwrap();
        if let Some(p) = plist {
            fs::write(app.join("Contents").join("Info.plist"), p).unwrap();
        }
        app
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("landingcraft-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_a_bundles_executable() {
        let dir = scratch("bundle");
        let one = fake_bundle(&dir, "One", &["one"], None);
        assert_eq!(bundle_executable(&one), Some(PathBuf::from("Contents/MacOS/one")));
        assert_eq!(launch_target(&one), one.join("Contents/MacOS/one"));
        assert!(crate::detect::is_executable(&one), "a bundle counts as executable");

        let two = fake_bundle(&dir, "Two", &["helper", "pdfcraft-bin"], Some(PLIST));
        assert_eq!(bundle_executable(&two), Some(PathBuf::from("Contents/MacOS/pdfcraft-bin")));

        let unclear = fake_bundle(&dir, "Unclear", &["a", "b"], None);
        assert_eq!(bundle_executable(&unclear), None);
        assert!(!crate::detect::is_executable(&unclear));
        assert_eq!(launch_target(&unclear), unclear);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mounts_and_copies_a_disk_image() {
        let dir = scratch("dmg");
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        fake_bundle(&src, "Fake", &["fake"], None);
        std::os::unix::fs::symlink("/Applications", src.join("Applications")).unwrap();
        let image = dir.join("fake.dmg");
        let ok = Command::new("/usr/bin/hdiutil")
            .args(["create", "-quiet", "-fs", "HFS+", "-format", "UDZO", "-volname", "Fake", "-srcfolder"])
            .arg(&src)
            .arg(&image)
            .status()
            .unwrap()
            .success();
        assert!(ok, "hdiutil create");

        let mount = dir.join("mnt");
        fs::create_dir_all(&mount).unwrap();
        let never = AtomicBool::new(false);
        attach_dmg(&image, &mount, &never).expect("attach");
        let app = find_bundle(&mount, "fake").expect("bundle in the image");
        assert_eq!(app.file_name().unwrap(), "Fake.app");
        let copy = dir.join("Fake.app");
        let copied = copy_bundle(&app, &copy, &never);
        detach_dmg(&mount);
        copied.expect("copy");
        assert!(fs::read_dir(&mount).unwrap().next().is_none(), "unmounted");
        assert_eq!(bundle_executable(&copy), Some(PathBuf::from("Contents/MacOS/fake")));
        assert!(crate::detect::is_executable(&copy.join("Contents/MacOS/fake")));

        let cancelled = AtomicBool::new(true);
        assert_eq!(attach_dmg(&image, &mount, &cancelled).unwrap_err(), crate::installer::CANCELLED);
        detach_dmg(&mount);
        let _ = fs::remove_dir_all(&dir);
    }
}
