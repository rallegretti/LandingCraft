//! Opening web links through the XDG Desktop Portal (`org.freedesktop.portal.OpenURI`),
//! spoken to directly over D-Bus with the pure-Rust `zbus` crate. This avoids
//! shelling out to `xdg-open` or any other helper program.

use std::collections::HashMap;
use std::sync::mpsc::Sender;

use zbus::zvariant::Value;

/// Opens `url` on a background thread. If the portal is unavailable, the URL is
/// sent back on `failed` so the UI can offer it on the clipboard instead.
pub fn open(url: String, failed: Sender<(String, String)>) {
    std::thread::spawn(move || {
        if let Err(err) = open_blocking(&url) {
            let _ = failed.send((url, err.to_string()));
        }
    });
}

fn open_blocking(url: &str) -> zbus::Result<()> {
    let conn = zbus::blocking::Connection::session()?;
    let options: HashMap<&str, Value<'_>> = HashMap::new();
    conn.call_method(
        Some("org.freedesktop.portal.Desktop"),
        "/org/freedesktop/portal/desktop",
        Some("org.freedesktop.portal.OpenURI"),
        "OpenURI",
        &("", url, options),
    )?;
    Ok(())
}
