//! Windows integration: opening links, choosing folders and showing folders in
//! File Explorer through the Windows shell (the windows crate wgpu already
//! uses), plus the message shown when the launcher can't start.
//!
//! Nothing here starts a helper program: links and folders go to
//! `ShellExecuteW`, and the folder chooser is the shell's own `IFileOpenDialog`.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::ERROR_CANCELLED;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog, IShellItem,
    SHCreateItemFromParsingName, SIGDN_FILESYSPATH, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW, SW_SHOWNORMAL};
use windows::core::{HRESULT, HSTRING, w};

/// COM for the current thread, for as long as this lives. The shell needs it,
/// and it may already be set up (winit does on the UI thread).
struct Com {
    owned: bool,
}

impl Com {
    fn init() -> Self {
        // S_FALSE (already initialised this way) still needs balancing; a
        // different threading model already in place doesn't, and works too.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        Self { owned: hr.is_ok() }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.owned {
            unsafe { CoUninitialize() };
        }
    }
}

/// Hand `target` to the shell with `verb`. ShellExecute reports success as a
/// value above 32.
fn shell_execute(verb: &HSTRING, target: &str) -> Result<(), String> {
    let _com = Com::init();
    let r = unsafe { ShellExecuteW(None, verb, &HSTRING::from(target), None, None, SW_SHOWNORMAL) };
    if r.0 as usize > 32 { Ok(()) } else { Err(std::io::Error::last_os_error().to_string()) }
}

/// Ask Windows to open a web link in the default browser. Only http(s) links
/// are passed on, since the shell would also run programs and open files.
pub fn open_uri(url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err("not a web link".to_owned());
    }
    shell_execute(&HSTRING::from("open"), url)
}

/// Open a folder in File Explorer.
pub fn show_folder(path: &Path) -> Result<(), String> {
    if !path.is_dir() {
        return Err("the folder doesn't exist".to_owned());
    }
    shell_execute(&HSTRING::from("explore"), &path.to_string_lossy())
}

/// Show the standard folder chooser. Blocks until the user picks a folder
/// (`Ok(Some)`) or cancels (`Ok(None)`), so call it off the UI thread: the
/// dialog runs its own message loop on the calling thread.
pub fn pick_folder(title: &str, start: &Path) -> Result<Option<PathBuf>, String> {
    let _com = Com::init();
    let result = unsafe {
        (|| -> windows::core::Result<PathBuf> {
            let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            dialog.SetOptions(dialog.GetOptions()? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)?;
            dialog.SetTitle(&HSTRING::from(title))?;
            if let Ok(folder) = SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(start.as_os_str()), None) {
                let _ = dialog.SetFolder(&folder);
            }
            dialog.Show(None)?;
            let name = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = PathBuf::from(OsString::from_wide(name.as_wide()));
            CoTaskMemFree(Some(name.0 as *const _));
            Ok(path)
        })()
    };
    match result {
        Ok(path) => Ok(Some(path)),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => Ok(None),
        Err(e) => Err(e.message()),
    }
}

/// Release builds have no console, so a failure to start would otherwise go
/// unseen: say what went wrong in a message box.
///
/// The box gets a thread of its own. The event loop that just ended leaves a
/// quit message in this thread's queue, which would close the box at once.
pub fn startup_error(message: &str) {
    let text = HSTRING::from(format!("LandingCraft couldn't start.\n\n{message}"));
    let _ = std::thread::spawn(move || unsafe { MessageBoxW(None, &text, w!("LandingCraft"), MB_OK | MB_ICONERROR) }).join();
}
