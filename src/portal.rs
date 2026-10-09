//! Desktop integration over D-Bus with the pure-Rust `zbus` crate: opening web
//! links and choosing folders through the XDG Desktop Portal, and revealing
//! folders through the file manager. No helper programs are spawned.

use std::collections::HashMap;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedValue, Value};

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

/// Ask the desktop to open a web link.
pub fn open_uri(url: &str) -> zbus::Result<()> {
    let conn = Connection::session()?;
    let options: HashMap<&str, Value<'_>> = HashMap::new();
    conn.call_method(Some(PORTAL), PORTAL_PATH, Some("org.freedesktop.portal.OpenURI"), "OpenURI", &("", url, options))?;
    Ok(())
}

/// Show the portal's folder chooser. Blocks until the user picks a folder
/// (`Ok(Some)`) or cancels (`Ok(None)`).
pub fn pick_folder(title: &str, start: &Path) -> zbus::Result<Option<PathBuf>> {
    let conn = Connection::session()?;
    let sender = conn
        .unique_name()
        .map(|n| n.trim_start_matches(':').replace('.', "_"))
        .unwrap_or_default();
    let token = format!("landingcraft{}", std::process::id());
    let request_path = format!("{PORTAL_PATH}/request/{sender}/{token}");

    // Subscribe before calling, so the reply can't be missed.
    let request = Proxy::new(&conn, PORTAL, request_path.as_str(), "org.freedesktop.portal.Request")?;
    let mut responses = request.receive_signal("Response")?;

    let mut start_bytes = start.as_os_str().as_encoded_bytes().to_vec();
    start_bytes.push(0);
    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert("directory", Value::from(true));
    options.insert("modal", Value::from(true));
    options.insert("current_folder", Value::from(start_bytes));
    conn.call_method(Some(PORTAL), PORTAL_PATH, Some("org.freedesktop.portal.FileChooser"), "OpenFile", &("", title, options))?;

    let Some(msg) = responses.next() else { return Ok(None) };
    let (code, results): (u32, HashMap<String, OwnedValue>) = msg.body().deserialize()?;
    if code != 0 {
        return Ok(None);
    }
    let uris = results
        .get("uris")
        .and_then(|v| Vec::<String>::try_from(v.clone()).ok())
        .unwrap_or_default();
    Ok(uris.first().and_then(|u| file_uri_to_path(u)))
}

/// Open a folder in the user's file manager.
pub fn show_folder(path: &Path) -> zbus::Result<()> {
    let conn = Connection::session()?;
    let uri = path_to_file_uri(path);
    conn.call_method(
        Some("org.freedesktop.FileManager1"),
        "/org/freedesktop/FileManager1",
        Some("org.freedesktop.FileManager1"),
        "ShowFolders",
        &(vec![uri.as_str()], ""),
    )?;
    Ok(())
}

fn path_to_file_uri(path: &Path) -> String {
    let mut out = String::from("file://");
    for &b in path.as_os_str().as_encoded_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(PathBuf::from(std::ffi::OsString::from_vec(out)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uris_round_trip() {
        let p = Path::new("/home/me/My Apps/ünï");
        assert_eq!(file_uri_to_path(&path_to_file_uri(p)).as_deref(), Some(p));
        assert_eq!(file_uri_to_path("file:///tmp/a%20b").as_deref(), Some(Path::new("/tmp/a b")));
        assert_eq!(file_uri_to_path("https://x"), None);
    }
}
