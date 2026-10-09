//! GitHub release lookup over pure-Rust HTTPS.
//!
//! TLS is rustls with the RustCrypto provider (no C or assembly crypto), and
//! certificates are checked against the system's CA store.

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use semver::Version;
use serde::{Deserialize, Serialize};
use ureq::tls::{Certificate, RootCerts, TlsConfig, TlsProvider};

use crate::settings::{self, Format};

pub const OWNER: &str = "storytold";

/// One HTTP client shared by every request.
pub fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let native = rustls_native_certs::load_native_certs();
        let roots = if native.certs.is_empty() {
            // No readable system store: fall back to Mozilla's bundled roots.
            RootCerts::WebPki
        } else {
            let certs: Vec<Certificate<'static>> = native
                .certs
                .iter()
                .map(|c| Certificate::from_der(c.as_ref()).to_owned())
                .collect();
            RootCerts::new_with_certs(&certs)
        };
        let tls = TlsConfig::builder()
            .provider(TlsProvider::Rustls)
            .root_certs(roots)
            .unversioned_rustls_crypto_provider(Arc::new(rustls_rustcrypto::provider()))
            .build();
        ureq::Agent::config_builder()
            .tls_config(tls)
            .user_agent(concat!("LandingCraft/", env!("CARGO_PKG_VERSION")))
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .build()
            .into()
    })
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub url: String,
    /// `sha256:<hex>` as reported by GitHub, when available.
    pub digest: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Release {
    pub tag: String,
    pub version: Version,
    pub published: String,
    pub page: String,
    pub notes: String,
    pub assets: Vec<Asset>,
}

/// GitHub's JSON, reduced to the fields we read.
#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
    html_url: String,
    body: Option<String>,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    digest: Option<String>,
}

/// A release counts as stable only if GitHub doesn't mark it draft/pre-release
/// *and* its tag is a plain `X.Y.Z` version. The tag check matters: some
/// release candidates (e.g. `v0.1.1-rc.5`) are published without the
/// pre-release flag. Build metadata (`+nightly…`) is rejected too.
fn stable(r: GhRelease) -> Option<Release> {
    if r.draft || r.prerelease {
        return None;
    }
    let version = Version::parse(r.tag_name.strip_prefix('v').unwrap_or(&r.tag_name)).ok()?;
    if !version.pre.is_empty() || !version.build.is_empty() {
        return None;
    }
    Some(Release {
        tag: r.tag_name,
        version,
        published: r.published_at.unwrap_or_default(),
        page: r.html_url,
        notes: r.body.unwrap_or_default(),
        assets: r
            .assets
            .into_iter()
            .map(|a| Asset { name: a.name, size: a.size, url: a.browser_download_url, digest: a.digest })
            .collect(),
    })
}

pub fn arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => other,
    }
}

/// The CPU as Windows release files name it: x64, x86 or arm64.
pub fn windows_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        other => other,
    }
}

/// "Linux x86_64", "macOS" (the Mac builds are universal), "Windows x64", and so on.
pub fn platform() -> String {
    match std::env::consts::OS {
        "macos" => "macOS".to_owned(),
        "windows" => format!("Windows {}", windows_arch()),
        _ => format!("Linux {}", arch()),
    }
}

/// The release file for `format` on this OS and CPU, if the format exists here.
pub fn asset_name(id: &str, version: &Version, format: Format) -> Option<String> {
    if !Format::available().contains(&format) {
        return None;
    }
    Some(match format {
        Format::Tarball => format!("{id}-{version}-linux-{}.tar.gz", arch()),
        Format::AppImage => format!("{id}-{version}-linux-{}.AppImage", arch()),
        Format::Dmg => format!("{id}-{version}-macos-universal.dmg"),
        Format::Zip => format!("{id}-{version}-windows-{}-portable.zip", windows_arch()),
    })
}

/// What to install: the newest stable release with a build for this OS and
/// CPU, in the preferred format if that release has it, otherwise another one.
pub struct Choice<'a> {
    pub release: &'a Release,
    pub asset: &'a Asset,
    pub format: Format,
}

pub fn latest_compatible<'a>(id: &str, releases: &'a [Release], preferred: Format) -> Option<Choice<'a>> {
    let mut sorted: Vec<&Release> = releases.iter().collect();
    sorted.sort_by(|a, b| b.version.cmp(&a.version));
    sorted.into_iter().find_map(|release| {
        let others = Format::available().iter().copied().filter(|&f| f != preferred);
        std::iter::once(preferred).chain(others).find_map(|format| {
            let name = asset_name(id, &release.version, format)?;
            let asset = release.assets.iter().find(|a| a.name == name)?;
            Some(Choice { release, asset, format })
        })
    })
}

/// Cached release list for one app, revalidated with its ETag so unchanged
/// lists aren't downloaded again. (Anonymous requests still count toward
/// GitHub's 60-per-hour limit, which is why checks are spaced out.)
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Cached {
    pub etag: Option<String>,
    pub checked: u64,
    pub releases: Vec<Release>,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn cache_path() -> std::path::PathBuf {
    settings::cache_dir().join("releases.json")
}

pub fn load_cache() -> HashMap<String, Cached> {
    std::fs::read(cache_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_cache(cache: &HashMap<String, Cached>) {
    if let Ok(json) = serde_json::to_vec(cache) {
        let _ = settings::write_atomic(&cache_path(), &json);
    }
}

/// Fetch the stable releases of `storytold/<id>`, reusing `previous` on 304.
pub fn fetch(id: &str, previous: Option<&Cached>) -> Result<Cached, String> {
    let url = format!("https://api.github.com/repos/{OWNER}/{id}/releases?per_page=50");
    let mut req = agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(etag) = previous.and_then(|p| p.etag.as_deref()) {
        req = req.header("If-None-Match", etag);
    }
    let mut resp = req.call().map_err(|e| format!("couldn't reach GitHub: {e}"))?;
    let status = resp.status().as_u16();
    let header = |name: &str| resp.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_owned);
    let (etag, remaining, reset) = (header("etag"), header("x-ratelimit-remaining"), header("x-ratelimit-reset"));

    match status {
        304 => {
            let mut cached = previous.cloned().unwrap_or_default();
            cached.checked = now();
            Ok(cached)
        }
        200 => {
            let mut body = String::new();
            resp.body_mut()
                .as_reader()
                .take(8 * 1024 * 1024)
                .read_to_string(&mut body)
                .map_err(|e| format!("couldn't read GitHub's reply: {e}"))?;
            let list: Vec<GhRelease> =
                serde_json::from_str(&body).map_err(|e| format!("unexpected reply from GitHub: {e}"))?;
            Ok(Cached { etag, checked: now(), releases: list.into_iter().filter_map(stable).collect() })
        }
        403 | 429 if remaining.as_deref() == Some("0") => {
            let wait = reset
                .and_then(|r| r.parse::<u64>().ok())
                .map(|reset| reset.saturating_sub(now()) / 60 + 1);
            Err(match wait {
                Some(m) => format!("GitHub's hourly request limit was reached; try again in about {m} min"),
                None => "GitHub's hourly request limit was reached; try again later".to_owned(),
            })
        }
        404 => Err(format!("no release list found for {OWNER}/{id}")),
        s => Err(format!("GitHub replied with HTTP {s}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gh(tag: &str, prerelease: bool, assets: &[&str]) -> GhRelease {
        GhRelease {
            tag_name: tag.to_owned(),
            draft: false,
            prerelease,
            published_at: None,
            html_url: String::new(),
            body: None,
            assets: assets
                .iter()
                .map(|n| GhAsset { name: (*n).to_owned(), size: 1, browser_download_url: String::new(), digest: None })
                .collect(),
        }
    }

    #[test]
    fn only_plain_versions_are_stable() {
        assert!(stable(gh("v0.5.0", false, &[])).is_some());
        assert!(stable(gh("0.5.0", false, &[])).is_some());
        // Real case: published as a normal release, but it's a release candidate.
        assert!(stable(gh("v0.1.1-rc.5", false, &[])).is_none());
        assert!(stable(gh("v0.6.0-nightly.20261008", false, &[])).is_none());
        assert!(stable(gh("v0.6.0+nightly", false, &[])).is_none());
        assert!(stable(gh("nightly", false, &[])).is_none());
        assert!(stable(gh("v0.6.0", true, &[])).is_none());
        let mut draft = gh("v0.6.0", false, &[]);
        draft.draft = true;
        assert!(stable(draft).is_none());
    }

    #[test]
    #[cfg(all(unix, not(target_os = "macos")))]
    fn picks_newest_compatible_release() {
        let a = arch();
        let releases: Vec<Release> = [
            gh("v0.10.0", false, &["x-0.10.0-windows-x64.msi"]),
            gh("v0.9.0", false, &[&format!("x-0.9.0-linux-{a}.AppImage")]),
            gh("v0.2.0", false, &[&format!("x-0.2.0-linux-{a}.tar.gz"), &format!("x-0.2.0-linux-{a}.AppImage")]),
            gh("v0.11.0-rc.1", false, &[&format!("x-0.11.0-rc.1-linux-{a}.tar.gz")]),
        ]
        .into_iter()
        .filter_map(stable)
        .collect();

        // 0.10.0 has no Linux build; 0.9.0 only has an AppImage, so the tarball preference falls back.
        let c = latest_compatible("x", &releases, Format::Tarball).unwrap();
        assert_eq!((c.release.version.to_string(), c.format), ("0.9.0".to_owned(), Format::AppImage));
        let c = latest_compatible("x", &releases, Format::AppImage).unwrap();
        assert_eq!(c.release.version.to_string(), "0.9.0");
        assert!(latest_compatible("y", &releases, Format::Tarball).is_none());
        assert!(latest_compatible("x", &releases, Format::Dmg).is_some(), "an unavailable preference falls back");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn picks_newest_mac_release() {
        let a = arch();
        let releases: Vec<Release> = [
            gh("v0.10.0", false, &[&format!("x-0.10.0-linux-{a}.tar.gz"), "x-0.10.0-windows-x64.msi"]),
            gh("v0.9.0", false, &["x-0.9.0-macos-universal.dmg", "x-cli-0.9.0-macos-universal.zip"]),
            gh("v0.8.0", false, &["x-0.8.0-macos-universal.dmg"]),
            gh("v0.11.0-rc.1", false, &["x-0.11.0-rc.1-macos-universal.dmg"]),
        ]
        .into_iter()
        .filter_map(stable)
        .collect();

        // 0.10.0 has no Mac build. A Linux format left in the settings still finds the disk image.
        for preferred in [Format::Dmg, Format::Tarball] {
            let c = latest_compatible("x", &releases, preferred).unwrap();
            assert_eq!((c.release.version.to_string(), c.format), ("0.9.0".to_owned(), Format::Dmg));
            assert_eq!(c.asset.name, "x-0.9.0-macos-universal.dmg");
        }
        assert!(latest_compatible("y", &releases, Format::Dmg).is_none());
    }

    #[test]
    #[cfg(windows)]
    fn picks_newest_windows_release() {
        let w = windows_arch();
        let other = if w == "arm64" { "x64" } else { "arm64" };
        let releases: Vec<Release> = [
            gh("v0.10.0", false, &["x-0.10.0-macos-universal.dmg", &format!("x-0.10.0-windows-{w}.msi")]),
            gh("v0.9.0", false, &[&format!("x-0.9.0-windows-{other}-portable.zip")]),
            gh("v0.8.0", false, &[&format!("x-0.8.0-windows-{w}.msi"), &format!("x-0.8.0-windows-{w}-portable.zip")]),
            gh("v0.11.0-rc.1", false, &[&format!("x-0.11.0-rc.1-windows-{w}-portable.zip")]),
        ]
        .into_iter()
        .filter_map(stable)
        .collect();

        // 0.10.0 only has an MSI, which the launcher can't manage; 0.9.0 is for another CPU.
        // A Linux format left in the settings still finds the zip.
        for preferred in [Format::Zip, Format::Tarball] {
            let c = latest_compatible("x", &releases, preferred).unwrap();
            assert_eq!((c.release.version.to_string(), c.format), ("0.8.0".to_owned(), Format::Zip));
            assert_eq!(c.asset.name, format!("x-0.8.0-windows-{w}-portable.zip"));
        }
        assert!(latest_compatible("y", &releases, Format::Zip).is_none());
    }
}
