//! Windows only: embeds the app icon and version information in
//! landingcraft.exe, so File Explorer, the Start menu, the taskbar and Task
//! Manager show them. Does nothing for other platforms.
//!
//! The resources are compiled with windres (MinGW-w64, which the GNU toolchain
//! needs anyway), or rc.exe for MSVC builds. Without one the build carries on
//! with a warning, unless LANDINGCRAFT_REQUIRE_WINRES is set, as
//! packaging/windows/package.sh does for release builds.
//!
//! There's no application manifest here: MinGW-w64 links in its own default
//! one (run as the invoking user, long paths allowed), and a second would clash.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=packaging/windows/landingcraft.ico");
    println!("cargo:rerun-if-env-changed=LANDINGCRAFT_REQUIRE_WINRES");
    println!("cargo:rerun-if-env-changed=WINDRES");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if let Err(e) = embed_resources() {
        if env::var_os("LANDINGCRAFT_REQUIRE_WINRES").is_some() {
            panic!("couldn't embed the Windows icon and version info: {e}");
        }
        println!("cargo:warning=building without the Windows icon and version info: {e}");
    }
}

fn embed_resources() -> Result<(), String> {
    let out = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR not set")?);
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or("CARGO_MANIFEST_DIR not set")?);
    let v = |part: &str| env::var(format!("CARGO_PKG_VERSION_{part}")).unwrap_or_else(|_| "0".to_owned());
    let (major, minor, patch, version) = (v("MAJOR"), v("MINOR"), v("PATCH"), env::var("CARGO_PKG_VERSION").unwrap_or_default());

    let ico = root.join("packaging/windows/landingcraft.ico").display().to_string().replace('\\', "/");
    let rc = out.join("landingcraft.rc");
    let script = format!(
        r#"LANGUAGE 0x09, 0x01
1 ICON "{ico}"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "FileDescription", "LandingCraft"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "landingcraft"
      VALUE "LegalCopyright", "CC0 1.0 Universal"
      VALUE "OriginalFilename", "landingcraft.exe"
      VALUE "ProductName", "LandingCraft"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    std::fs::write(&rc, script).map_err(|e| e.to_string())?;

    let (tool, args, output): (String, Vec<OsString>, PathBuf) = if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let res = out.join("landingcraft.res");
        ("rc".to_owned(), vec!["/nologo".into(), "/fo".into(), res.clone().into(), rc.into()], res)
    } else {
        // An object file, which GNU ld links like any other.
        let obj = out.join("landingcraft-res.o");
        let tool = env::var("WINDRES").unwrap_or_else(|_| "windres".to_owned());
        let args = ["-i".into(), rc.into(), "-O".into(), "coff".into(), "-o".into(), obj.clone().into()];
        (tool, args.into(), obj)
    };
    let status = Command::new(&tool).args(&args).status().map_err(|e| format!("couldn't run {tool}: {e}"))?;
    if !status.success() {
        return Err(format!("{tool} failed ({status})"));
    }
    println!("cargo:rustc-link-arg-bins={}", output.display());
    Ok(())
}
