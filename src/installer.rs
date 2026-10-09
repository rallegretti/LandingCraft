//! Managed installations: `<install dir>/<app id>/` holds the payload (an
//! unpacked release folder, an AppImage, or on macOS an `.app` bundle) plus a
//! `craftapp.json` manifest.
//! Paths in the manifest are relative to the app folder, so the whole install
//! folder can be moved.
//!
//! Every temporary entry the installer creates starts with `.lc-`, so leftovers
//! from an interrupted install can be recognised and removed safely.

use std::fs::{self, File};
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::releases::{self, Asset, Release, now};
use crate::settings::{Format, write_atomic};

pub const MANIFEST: &str = "craftapp.json";
const TMP: &str = ".lc-";

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub id: String,
    pub version: Version,
    pub format: Format,
    /// Unpacked folder, AppImage file or `.app` bundle, relative to the app folder.
    pub payload: String,
    /// Executable to launch, relative to the app folder.
    pub exe: String,
    pub asset: String,
    pub sha256: String,
    pub installed: u64,
}

impl Manifest {
    pub fn exe_path(&self, base: &Path) -> PathBuf {
        base.join(&self.id).join(&self.exe)
    }
}

pub fn read_manifest(base: &Path, id: &str) -> Option<Manifest> {
    let bytes = fs::read(base.join(id).join(MANIFEST)).ok()?;
    let m: Manifest = serde_json::from_slice(&bytes).ok()?;
    (m.id == id && is_plain_relative(Path::new(&m.exe)) && is_single_name(&m.payload)).then_some(m)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    Downloading,
    Verifying,
    Unpacking,
    Finishing,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::Downloading => "Downloading",
            Stage::Verifying => "Verifying",
            Stage::Unpacking => "Unpacking",
            Stage::Finishing => "Finishing",
        }
    }
}

pub const CANCELLED: &str = "Cancelled";

/// Download, verify and install `asset` from `release`, replacing any earlier
/// managed version of the app only once the new one is fully in place.
pub fn install(
    base: &Path,
    id: &str,
    release: &Release,
    asset: &Asset,
    format: Format,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Stage, u64, u64),
) -> Result<Manifest, String> {
    let expected = expected_sha256(release, asset)?;
    let dir = base.join(id);
    fs::create_dir_all(&dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
    let previous = read_manifest(base, id);

    let part = dir.join(format!("{TMP}download.part"));
    let result = (|| {
        let actual = download(asset, &part, cancel, progress)?;
        progress(Stage::Verifying, 0, 1);
        if actual != expected {
            return Err(format!("download of {} is corrupt (checksum mismatch)", asset.name));
        }
        match format {
            Format::Tarball => place_tarball(&dir, id, &part, cancel, progress),
            Format::AppImage => place_appimage(&dir, &asset.name, &part),
            #[cfg(target_os = "macos")]
            Format::Dmg => place_dmg(&dir, id, &part, cancel, progress),
            #[cfg(not(target_os = "macos"))]
            Format::Dmg => Err("disk images can only be installed on macOS".to_owned()),
        }
    })();
    let _ = fs::remove_file(&part);
    let (payload, exe) = match result {
        Ok(placed) => placed,
        Err(e) => {
            // A failed or cancelled first install leaves no empty app folder behind.
            if previous.is_none() {
                let _ = fs::remove_dir(&dir);
            }
            return Err(e);
        }
    };

    progress(Stage::Finishing, 0, 1);
    let manifest = Manifest {
        id: id.to_owned(),
        version: release.version.clone(),
        format,
        payload,
        exe,
        asset: asset.name.clone(),
        sha256: expected,
        installed: now(),
    };
    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    write_atomic(&dir.join(MANIFEST), &json).map_err(|e| format!("couldn't write the manifest: {e}"))?;

    if let Some(prev) = previous
        && prev.payload != manifest.payload
    {
        remove_any(&dir.join(&prev.payload));
    }
    clean_leftovers(&dir);
    Ok(manifest)
}

/// Remove a managed app. Refuses unless the folder carries this app's manifest.
pub fn uninstall(base: &Path, id: &str) -> Result<(), String> {
    if read_manifest(base, id).is_none() {
        return Err(format!("{} is not managed by the launcher", base.join(id).display()));
    }
    fs::remove_dir_all(base.join(id)).map_err(|e| format!("couldn't remove {}: {e}", base.join(id).display()))
}

/// Startup tidy-up for one app folder: remove `.lc-*` leftovers from an
/// interrupted install, then the folder itself if that leaves it empty (an
/// install that was cut off before anything was placed).
pub fn tidy(dir: &Path) {
    clean_leftovers(dir);
    let _ = fs::remove_dir(dir); // only succeeds when empty
}

/// Delete `.lc-*` leftovers from an interrupted install.
pub fn clean_leftovers(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(TMP) {
            // A disk image left mounted by an interrupted install is unmounted
            // first, so nothing on it is touched.
            #[cfg(target_os = "macos")]
            if entry.file_name() == MOUNT {
                crate::macos::detach_dmg(&entry.path());
                let _ = fs::remove_dir(entry.path());
                continue;
            }
            remove_any(&entry.path());
        }
    }
}

pub fn disk_usage(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else { return 0 };
    if meta.is_dir() {
        fs::read_dir(path)
            .map(|rd| rd.flatten().map(|e| disk_usage(&e.path())).sum())
            .unwrap_or(0)
    } else {
        meta.len()
    }
}

// ------------------------------------------------------------------ download

fn expected_sha256(release: &Release, asset: &Asset) -> Result<String, String> {
    if let Some(hex) = asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:"))
        && is_sha256_hex(hex)
    {
        return Ok(hex.to_ascii_lowercase());
    }
    // Older releases: fall back to the published SHA256SUMS.txt.
    let sums = release
        .assets
        .iter()
        .find(|a| a.name == "SHA256SUMS.txt")
        .ok_or_else(|| format!("{} has no published checksum, so it won't be installed", asset.name))?;
    let mut resp = releases::agent()
        .get(&sums.url)
        .call()
        .map_err(|e| format!("couldn't fetch checksums: {e}"))?;
    if resp.status().as_u16() != 200 {
        return Err(format!("couldn't fetch checksums (HTTP {})", resp.status().as_u16()));
    }
    let mut text = String::new();
    resp.body_mut()
        .as_reader()
        .take(1024 * 1024)
        .read_to_string(&mut text)
        .map_err(|e| format!("couldn't read checksums: {e}"))?;
    text.lines()
        .filter_map(|l| l.split_once(char::is_whitespace))
        .find(|(_, name)| name.trim().trim_start_matches('*') == asset.name)
        .map(|(hex, _)| hex.to_ascii_lowercase())
        .filter(|hex| is_sha256_hex(hex))
        .ok_or_else(|| format!("{} is missing from SHA256SUMS.txt, so it won't be installed", asset.name))
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Stream the asset to `dest`, hashing as it arrives. Returns the SHA-256 hex.
fn download(
    asset: &Asset,
    dest: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Stage, u64, u64),
) -> Result<String, String> {
    let resp = releases::agent()
        .get(&asset.url)
        .call()
        .map_err(|e| format!("couldn't start the download: {e}"))?;
    if resp.status().as_u16() != 200 {
        return Err(format!("download failed (HTTP {})", resp.status().as_u16()));
    }
    let mut reader = resp.into_body().into_reader();
    let mut file = File::create(dest).map_err(|e| format!("couldn't write {}: {e}", dest.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    progress(Stage::Downloading, 0, asset.size);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.to_owned());
        }
        let n = reader.read(&mut buf).map_err(|e| format!("download interrupted: {e}"))?;
        if n == 0 {
            break;
        }
        done += n as u64;
        if done > asset.size {
            return Err("the server sent more data than expected".to_owned());
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| format!("couldn't save the download: {e}"))?;
        progress(Stage::Downloading, done, asset.size);
    }
    if done != asset.size {
        return Err(format!("download ended early ({done} of {} bytes)", asset.size));
    }
    file.sync_all().map_err(|e| format!("couldn't save the download: {e}"))?;
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

// ------------------------------------------------------------------- placing

fn place_appimage(dir: &Path, name: &str, part: &Path) -> Result<(String, String), String> {
    #[cfg(unix)]
    fs::set_permissions(part, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    swap_into_place(dir, part, name)?;
    Ok((name.to_owned(), name.to_owned()))
}

fn place_tarball(
    dir: &Path,
    id: &str,
    part: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Stage, u64, u64),
) -> Result<(String, String), String> {
    let staging = dir.join(format!("{TMP}staging"));
    remove_any(&staging);
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;

    let result = (|| {
        let top = unpack(part, &staging, cancel, progress)?;
        let root = staging.join(&top);
        let exe = [Path::new("bin").join(id), PathBuf::from(id)]
            .into_iter()
            .find(|rel| crate::detect::is_executable(&root.join(rel)))
            .ok_or_else(|| format!("the archive has no {id} executable"))?;
        swap_into_place(dir, &root, &top)?;
        Ok((top.clone(), Path::new(&top).join(exe).to_string_lossy().into_owned()))
    })();
    remove_any(&staging);
    result
}

/// Where a disk image is mounted while its app is copied out.
#[cfg(target_os = "macos")]
const MOUNT: &str = ".lc-mount";

/// Mount the disk image, copy its `.app` bundle out, and unmount it. The
/// bundle is kept whole, so its code signature stays valid.
#[cfg(target_os = "macos")]
fn place_dmg(
    dir: &Path,
    id: &str,
    part: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Stage, u64, u64),
) -> Result<(String, String), String> {
    use crate::macos;

    let mount = dir.join(MOUNT);
    let staging = dir.join(format!("{TMP}staging"));
    remove_any(&staging);
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    fs::create_dir_all(&mount).map_err(|e| e.to_string())?;

    progress(Stage::Unpacking, 0, 2);
    let copied = macos::attach_dmg(part, &mount, cancel).and_then(|()| {
        let app = macos::find_bundle(&mount, id).ok_or_else(|| "the disk image has no app in it".to_owned())?;
        let name = app.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        progress(Stage::Unpacking, 1, 2);
        macos::copy_bundle(&app, &staging.join(&name), cancel)?;
        Ok(name)
    });
    // Unmount even when attaching reported failure or was cancelled part-way.
    macos::detach_dmg(&mount);
    let _ = fs::remove_dir(&mount);

    let result = copied.and_then(|name| {
        progress(Stage::Unpacking, 2, 2);
        let exe = macos::bundle_executable(&staging.join(&name)).ok_or_else(|| format!("{name} has no executable"))?;
        swap_into_place(dir, &staging.join(&name), &name)?;
        Ok((name.clone(), Path::new(&name).join(exe).to_string_lossy().into_owned()))
    });
    remove_any(&staging);
    result
}

/// Move `src` to `dir/name`. An existing entry of that name (a reinstall of the
/// same version) is moved aside first and deleted only after the swap.
fn swap_into_place(dir: &Path, src: &Path, name: &str) -> Result<(), String> {
    let target = dir.join(name);
    let aside = dir.join(format!("{TMP}old"));
    remove_any(&aside);
    let had_old = fs::symlink_metadata(&target).is_ok();
    if had_old {
        fs::rename(&target, &aside).map_err(|e| format!("couldn't replace {}: {e}", target.display()))?;
    }
    if let Err(e) = fs::rename(src, &target) {
        if had_old {
            let _ = fs::rename(&aside, &target);
        }
        return Err(format!("couldn't move the new version into place: {e}"));
    }
    remove_any(&aside);
    Ok(())
}

/// Counts compressed bytes consumed, for unpack progress.
struct Counting<R> {
    inner: R,
    count: std::rc::Rc<std::cell::Cell<u64>>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count.set(self.count.get() + n as u64);
        Ok(n)
    }
}

/// Unpack a release tarball into `dest`. Every entry must sit under one
/// top-level folder, and only regular files, folders and symlinks that stay
/// inside the archive are accepted. Returns the top-level folder name.
fn unpack(
    archive: &Path,
    dest: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Stage, u64, u64),
) -> Result<String, String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    let total = file.metadata().map_or(0, |m| m.len());
    let count = std::rc::Rc::new(std::cell::Cell::new(0));
    let reader = Counting { inner: io::BufReader::new(file), count: count.clone() };
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(reader));
    tar.set_preserve_permissions(false);
    tar.set_unpack_xattrs(false);

    let bad = |why: &str| format!("the archive looks unsafe ({why}), so it wasn't installed");
    let mut top: Option<String> = None;
    for entry in tar.entries().map_err(|e| e.to_string())? {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.to_owned());
        }
        let mut entry = entry.map_err(|e| format!("couldn't read the archive: {e}"))?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        if !is_plain_relative(&path) {
            return Err(bad("an entry points outside its folder"));
        }
        let first = path
            .components()
            .find_map(|c| match c {
                Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                _ => None,
            })
            .ok_or_else(|| bad("an entry has no name"))?;
        match &top {
            None => top = Some(first),
            Some(t) if *t == first => {}
            Some(_) => return Err(bad("more than one top-level folder")),
        }

        use tar::EntryType as T;
        match entry.header().entry_type() {
            T::Regular | T::Directory => {}
            T::Symlink => {
                let target = entry.link_name().map_err(|e| e.to_string())?.ok_or_else(|| bad("empty link"))?;
                if !link_stays_inside(&path, &target) {
                    return Err(bad("a link points outside the app"));
                }
            }
            other => return Err(bad(&format!("unsupported entry type {other:?}"))),
        }
        entry.unpack_in(dest).map_err(|e| format!("couldn't unpack {}: {e}", path.display()))?;
        progress(Stage::Unpacking, count.get().min(total), total);
    }
    top.filter(|t| dest.join(t).is_dir()).ok_or_else(|| bad("no top-level folder"))
}

fn is_plain_relative(p: &Path) -> bool {
    p.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
        && p.components().any(|c| matches!(c, Component::Normal(_)))
}

fn is_single_name(s: &str) -> bool {
    let p = Path::new(s);
    p.components().count() == 1 && matches!(p.components().next(), Some(Component::Normal(_)))
}

/// Resolve a relative symlink target against the link's folder and check it
/// never climbs above the archive's top-level folder.
fn link_stays_inside(link: &Path, target: &Path) -> bool {
    if target.is_absolute() {
        return false;
    }
    let mut depth: i32 = link.components().filter(|c| matches!(c, Component::Normal(_))).count() as i32 - 1;
    for c in target.components() {
        match c {
            Component::Normal(_) => depth += 1,
            Component::ParentDir => {
                depth -= 1;
                if depth < 1 {
                    return false;
                }
            }
            Component::CurDir => {}
            _ => return false,
        }
    }
    depth >= 1
}

fn remove_any(path: &Path) {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => {
            let _ = fs::remove_dir_all(path);
        }
        Ok(_) => {
            let _ = fs::remove_file(path);
        }
        Err(_) => {}
    }
}

// --------------------------------------------------------- moving the folder

/// Move every managed app from `old` to `new`. Renames when both are on the
/// same filesystem, otherwise copies and then deletes. On failure, apps that
/// were already moved are put back, so installs never end up split.
pub fn move_installs(old: &Path, new: &Path, ids: &[&str], progress: &mut dyn FnMut(usize)) -> Result<(), String> {
    fs::create_dir_all(new).map_err(|e| format!("couldn't create {}: {e}", new.display()))?;
    let probe = new.join(format!("{TMP}write-test"));
    File::create(&probe).map_err(|e| format!("{} isn't writable: {e}", new.display()))?;
    let _ = fs::remove_file(&probe);

    let to_move: Vec<&str> = ids.iter().copied().filter(|id| read_manifest(old, id).is_some()).collect();
    for id in &to_move {
        let dest = new.join(id);
        let empty_dir = fs::read_dir(&dest).map(|mut d| d.next().is_none()).unwrap_or(false);
        if empty_dir {
            let _ = fs::remove_dir(&dest);
        } else if fs::symlink_metadata(&dest).is_ok() {
            return Err(format!("{} already exists; move or remove it first", dest.display()));
        }
    }

    let mut moved: Vec<&str> = Vec::new();
    for (i, id) in to_move.iter().enumerate() {
        progress(i);
        if let Err(e) = move_dir(&old.join(id), &new.join(id)) {
            for back in moved {
                let _ = move_dir(&new.join(back), &old.join(back));
            }
            return Err(format!("couldn't move {id}: {e}"));
        }
        moved.push(id);
    }
    Ok(())
}

fn move_dir(from: &Path, to: &Path) -> io::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        // EXDEV: different filesystems.
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            if let Err(e) = copy_tree(from, to) {
                remove_any(to);
                return Err(e);
            }
            fs::remove_dir_all(from)
        }
        Err(e) => Err(e),
    }
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(from)?;
    if meta.file_type().is_symlink() {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(fs::read_link(from)?, to);
        #[cfg(not(unix))]
        return Err(io::Error::other(format!("{} is a symbolic link", from.display())));
    } else if meta.is_dir() {
        fs::create_dir(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
        fs::set_permissions(to, meta.permissions())
    } else {
        fs::copy(from, to).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::releases;

    #[test]
    fn links_must_stay_inside_the_archive() {
        let link = Path::new("app-1.0/lib/libfoo.so");
        assert!(link_stays_inside(link, Path::new("libfoo.so.1")));
        assert!(link_stays_inside(link, Path::new("../share/x")));
        assert!(!link_stays_inside(link, Path::new("../../x")));
        assert!(!link_stays_inside(link, Path::new("/etc/passwd")));
        assert!(!link_stays_inside(Path::new("app-1.0/x"), Path::new("..")));
    }

    #[test]
    fn manifest_paths_are_plain() {
        assert!(is_plain_relative(Path::new("a/bin/a")));
        assert!(!is_plain_relative(Path::new("../a")));
        assert!(!is_plain_relative(Path::new("/a")));
        assert!(is_single_name("a-1.0-linux-x86_64"));
        assert!(!is_single_name("a/b"));
        assert!(!is_single_name(".."));
    }

    /// Full round trip against GitHub (~120 MB of downloads):
    /// `LC_TEST_DIR=/tmp/x cargo test -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn install_switch_move_uninstall() {
        let root = PathBuf::from(std::env::var("LC_TEST_DIR").expect("set LC_TEST_DIR"));
        // Optionally a folder on another filesystem, to exercise copy-then-delete moves.
        let other = std::env::var_os("LC_TEST_DIR2").map(PathBuf::from).unwrap_or_else(|| root.join("other"));
        let (a, b) = (root.join("a"), other.join("b"));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&b);
        let id = "pdfcraft";
        let cached = releases::fetch(id, None).expect("fetch releases");
        assert!(cached.releases.iter().all(|r| r.version.pre.is_empty()));
        let cancel = AtomicBool::new(false);
        let mut log = |s: Stage, d: u64, t: u64| {
            if d == t {
                eprintln!("  {s:?} {d}/{t}");
            }
        };

        let first = releases::latest_compatible(id, &cached.releases, Format::Tarball).expect("compatible release");
        let cancelled = AtomicBool::new(true);
        let r = install(&a, id, first.release, first.asset, Format::Tarball, &cancelled, &mut |_, _, _| {});
        assert_eq!(r.unwrap_err(), CANCELLED);
        assert!(!a.join(id).exists(), "a cancelled first install leaves no folder");

        for &format in Format::available() {
            let c = releases::latest_compatible(id, &cached.releases, format).expect("compatible release");
            assert_eq!(c.format, format);
            eprintln!("installing {} v{} as {format:?}", c.asset.name, c.release.version);
            let m = install(&a, id, c.release, c.asset, format, &cancel, &mut log).expect("install");
            assert_eq!(m.format, format);
            assert!(crate::detect::is_executable(&m.exe_path(&a)), "exe {}", m.exe);
            assert_eq!(read_manifest(&a, id).unwrap().payload, m.payload);
            // Only the manifest and the current payload remain.
            let mut names: Vec<String> =
                fs::read_dir(a.join(id)).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
            names.sort();
            let mut want = vec![MANIFEST.to_owned(), m.payload.clone()];
            want.sort();
            assert_eq!(names, want);
        }

        move_installs(&a, &b, &[id], &mut |_| {}).expect("move");
        assert!(!a.join(id).exists(), "source removed after the move");
        assert!(read_manifest(&a, id).is_none());
        let m = read_manifest(&b, id).expect("moved manifest");
        assert!(crate::detect::is_executable(&m.exe_path(&b)));

        uninstall(&b, id).expect("uninstall");
        assert!(!b.join(id).exists());
        assert!(uninstall(&b, id).is_err(), "refuses folders it doesn't manage");
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&b);
    }
}
