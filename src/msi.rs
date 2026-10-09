//! Windows Installer: reading the apps' MSI packages, and installing and
//! removing them through msi.dll, the API msiexec itself uses, so no helper
//! program is started.
//!
//! The apps' MSIs install for all users, into Program Files, so Windows asks
//! for administrator approval each time. Meanwhile the installer shows its own
//! small progress window, whose Cancel button rolls the change back.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use windows::Win32::System::ApplicationInstallationAndServicing::{
    INSTALLLEVEL_DEFAULT, INSTALLSTATE_ABSENT, INSTALLSTATE_DEFAULT, INSTALLSTATE_LOCAL, INSTALLUILEVEL_BASIC, MSIDBOPEN_READONLY, MSIHANDLE,
    MsiCloseHandle, MsiConfigureProductExW, MsiDatabaseOpenViewW, MsiGetComponentPathW, MsiInstallProductW,
    MsiOpenDatabaseW, MsiQueryProductStateW, MsiRecordGetFieldCount, MsiRecordGetStringW, MsiSetInternalUI,
    MsiViewExecute, MsiViewFetch,
};
use windows::core::{HSTRING, PWSTR};

use crate::installer::CANCELLED;

const ERROR_NO_MORE_ITEMS: u32 = 259;

/// What the launcher needs from an app's MSI.
#[derive(Debug, PartialEq, Eq)]
pub struct Package {
    pub product_code: String,
    pub upgrade_code: String,
    /// The Windows Installer component that holds `<id>.exe`, which says where it was installed.
    pub exe_component: String,
}

/// A turn at Windows Installer, which runs one installation at a time (the
/// launcher's own included), so installs and removals queue for it.
pub struct Turn(#[allow(dead_code)] MutexGuard<'static, ()>);

pub fn queue() -> Turn {
    static QUEUE: Mutex<()> = Mutex::new(());
    Turn(QUEUE.lock().unwrap_or_else(|e| e.into_inner()))
}

struct Handle(MSIHANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { MsiCloseHandle(self.0) };
    }
}

fn check(code: u32, what: &str) -> Result<(), String> {
    match code {
        0 => Ok(()),
        c => Err(format!("{what}: {}", std::io::Error::from_raw_os_error(c as i32))),
    }
}

fn record_string(record: &Handle, field: u32) -> Result<String, String> {
    let mut len = 0u32;
    check(unsafe { MsiRecordGetStringW(record.0, field, None, Some(&mut len)) }, "couldn't read the package")?;
    let mut buf = vec![0u16; len as usize + 1];
    let mut cap = len + 1;
    check(
        unsafe { MsiRecordGetStringW(record.0, field, Some(PWSTR(buf.as_mut_ptr())), Some(&mut cap)) },
        "couldn't read the package",
    )?;
    Ok(String::from_utf16_lossy(&buf[..cap as usize]))
}

/// Run a query on an MSI database and return every row's fields as text.
fn query(db: &Handle, sql: &str) -> Result<Vec<Vec<String>>, String> {
    let mut view = MSIHANDLE::default();
    check(unsafe { MsiDatabaseOpenViewW(db.0, &HSTRING::from(sql), &mut view) }, "couldn't read the package")?;
    let view = Handle(view);
    check(unsafe { MsiViewExecute(view.0, MSIHANDLE::default()) }, "couldn't read the package")?;
    let mut rows = Vec::new();
    loop {
        let mut record = MSIHANDLE::default();
        match unsafe { MsiViewFetch(view.0, &mut record) } {
            ERROR_NO_MORE_ITEMS => return Ok(rows),
            code => check(code, "couldn't read the package")?,
        }
        let record = Handle(record);
        let fields = unsafe { MsiRecordGetFieldCount(record.0) };
        rows.push((1..=fields).map(|f| record_string(&record, f)).collect::<Result<_, _>>()?);
    }
}

/// Read the product, upgrade and executable component codes from the MSI at
/// `path` for the app `id`.
pub fn read_package(path: &Path, id: &str) -> Result<Package, String> {
    let mut db = MSIHANDLE::default();
    check(
        unsafe { MsiOpenDatabaseW(&HSTRING::from(path.as_os_str()), MSIDBOPEN_READONLY, &mut db) },
        "couldn't open the installer package",
    )?;
    let db = Handle(db);
    let properties = query(&db, "SELECT `Property`, `Value` FROM `Property`")?;
    let property = |name: &str| {
        properties
            .iter()
            .find(|row| row[0] == name)
            .map(|row| row[1].clone())
            .ok_or_else(|| format!("the installer package has no {name}"))
    };
    // File names are stored as "SHORT~1.EXE|long-name.exe", or just the one name.
    let exe = format!("{id}.exe");
    let files = query(
        &db,
        "SELECT `File`.`FileName`, `Component`.`ComponentId` FROM `File`, `Component` \
         WHERE `File`.`Component_` = `Component`.`Component`",
    )?;
    let exe_component = files
        .iter()
        .find(|row| row[0].rsplit('|').next().is_some_and(|name| name.eq_ignore_ascii_case(&exe)))
        .map(|row| row[1].clone())
        .ok_or_else(|| format!("the installer package has no {exe}"))?;
    Ok(Package { product_code: property("ProductCode")?, upgrade_code: property("UpgradeCode")?, exe_component })
}

/// What Windows Installer's result codes mean for the launcher.
fn outcome(code: u32) -> Result<(), String> {
    match code {
        // Done, or done once Windows restarts (never forced: REBOOT=ReallySuppress).
        0 | 1641 | 3010 => Ok(()),
        // Cancelled in the installer's window, or administrator approval was declined.
        1602 | 1223 => Err(CANCELLED.to_owned()),
        1618 => Err("Windows is installing something else; try again when it's finished".to_owned()),
        c => Err(format!("Windows Installer failed: {}", std::io::Error::from_raw_os_error(c as i32))),
    }
}

/// Install the MSI at `package`. `reinstall` reinstalls a product that's
/// already installed (the same package again) instead of leaving it as it is.
pub fn install(_turn: &Turn, package: &Path, reinstall: bool) -> Result<(), String> {
    let mut properties = "REBOOT=ReallySuppress".to_owned();
    if reinstall {
        properties.push_str(" REINSTALL=ALL REINSTALLMODE=vamus");
    }
    unsafe { MsiSetInternalUI(INSTALLUILEVEL_BASIC, None) };
    outcome(unsafe { MsiInstallProductW(&HSTRING::from(package.as_os_str()), &HSTRING::from(properties)) })
}

/// Remove an installed product, as Settings › Apps would.
pub fn uninstall(_turn: &Turn, product_code: &str) -> Result<(), String> {
    unsafe { MsiSetInternalUI(INSTALLUILEVEL_BASIC, None) };
    let code = unsafe {
        MsiConfigureProductExW(&HSTRING::from(product_code), INSTALLLEVEL_DEFAULT, INSTALLSTATE_ABSENT, &HSTRING::from("REBOOT=ReallySuppress"))
    };
    // 1605: not installed (any more), which is what was wanted.
    if code == 1605 { Ok(()) } else { outcome(code) }
}

pub fn is_installed(product_code: &str) -> bool {
    let state = unsafe { MsiQueryProductStateW(&HSTRING::from(product_code)) };
    state == INSTALLSTATE_DEFAULT
}

/// Where a product's component was installed: for the executable's
/// component, the executable itself.
pub fn component_path(product_code: &str, component: &str) -> Option<PathBuf> {
    let mut buf = vec![0u16; 32_768];
    let mut len = buf.len() as u32;
    let state = unsafe {
        MsiGetComponentPathW(
            &HSTRING::from(product_code),
            &HSTRING::from(component),
            Some(PWSTR(buf.as_mut_ptr())),
            Some(&mut len),
        )
    };
    (state == INSTALLSTATE_LOCAL).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::ApplicationInstallationAndServicing::{MSIDBOPEN_CREATE, MsiDatabaseCommit};

    fn exec(db: &Handle, sql: &str) {
        let mut view = MSIHANDLE::default();
        assert_eq!(unsafe { MsiDatabaseOpenViewW(db.0, &HSTRING::from(sql), &mut view) }, 0, "{sql}");
        let view = Handle(view);
        assert_eq!(unsafe { MsiViewExecute(view.0, MSIHANDLE::default()) }, 0, "{sql}");
    }

    /// An MSI database with just the tables read_package looks at, laid out
    /// like the apps' packages (WiX writes short|long file names).
    fn make_package(path: &Path) {
        let _ = std::fs::remove_file(path);
        let mut db = MSIHANDLE::default();
        assert_eq!(unsafe { MsiOpenDatabaseW(&HSTRING::from(path.as_os_str()), MSIDBOPEN_CREATE, &mut db) }, 0);
        let db = Handle(db);
        exec(&db, "CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL, `Value` LONGCHAR NOT NULL PRIMARY KEY `Property`)");
        exec(&db, "CREATE TABLE `Component` (`Component` CHAR(72) NOT NULL, `ComponentId` CHAR(38) PRIMARY KEY `Component`)");
        exec(
            &db,
            "CREATE TABLE `File` (`File` CHAR(72) NOT NULL, `Component_` CHAR(72) NOT NULL, `FileName` CHAR(255) NOT NULL \
             PRIMARY KEY `File`)",
        );
        for sql in [
            "INSERT INTO `Property` (`Property`, `Value`) VALUES ('ProductCode', '{11111111-2222-3333-4444-555555555555}')",
            "INSERT INTO `Property` (`Property`, `Value`) VALUES ('UpgradeCode', '{EA00C367-7270-4C50-9D8B-A7A5C88AD27F}')",
            "INSERT INTO `Property` (`Property`, `Value`) VALUES ('ALLUSERS', '1')",
            "INSERT INTO `Component` (`Component`, `ComponentId`) VALUES ('App', '{E724C1C6-091F-4E21-A685-80D089FDBE21}')",
            "INSERT INTO `Component` (`Component`, `ComponentId`) VALUES ('Cli', '{0A0A0A0A-0000-0000-0000-000000000000}')",
            "INSERT INTO `File` (`File`, `Component_`, `FileName`) VALUES ('CliExe', 'Cli', 'X-CLI.EXE|x-cli.exe')",
            "INSERT INTO `File` (`File`, `Component_`, `FileName`) VALUES ('AppExe', 'App', 'X.EXE|X.exe')",
        ] {
            exec(&db, sql);
        }
        assert_eq!(unsafe { MsiDatabaseCommit(db.0) }, 0);
    }

    #[test]
    fn reads_the_codes_the_launcher_needs() {
        let path = std::env::temp_dir().join(format!("landingcraft-test-{}.msi", std::process::id()));
        make_package(&path);
        assert_eq!(
            read_package(&path, "x"),
            Ok(Package {
                product_code: "{11111111-2222-3333-4444-555555555555}".to_owned(),
                upgrade_code: "{EA00C367-7270-4C50-9D8B-A7A5C88AD27F}".to_owned(),
                exe_component: "{E724C1C6-091F-4E21-A685-80D089FDBE21}".to_owned(),
            })
        );
        assert!(read_package(&path, "y").is_err_and(|e| e.contains("no y.exe")));
        assert!(read_package(&path.with_extension("missing"), "x").is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unknown_products_are_not_installed() {
        assert!(!is_installed("{11111111-2222-3333-4444-555555555555}"));
        assert_eq!(component_path("{11111111-2222-3333-4444-555555555555}", "{E724C1C6-091F-4E21-A685-80D089FDBE21}"), None);
        let turn = queue();
        assert_eq!(uninstall(&turn, "{11111111-2222-3333-4444-555555555555}"), Ok(()), "nothing to remove is fine");
        assert_eq!(outcome(1602), Err(CANCELLED.to_owned()));
        assert!(outcome(1603).is_err_and(|e| e.starts_with("Windows Installer failed")));
    }
}
