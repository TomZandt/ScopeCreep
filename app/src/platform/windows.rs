//! The app on Windows.
use std::ffi::OsStr;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use winit::window::WindowAttributes;

/// A child started from the app must not open a console window (the app has none: `windows_subsystem`).
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The folder the app remembers its settings in: `%APPDATA%\ScopeCreep`. None without `APPDATA`.
pub fn settings_folder() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").filter(|v| !v.is_empty())?;
    Some(PathBuf::from(base).join("ScopeCreep"))
}

/// The Python to run the Resolve helper under: the first that exists of [`python_candidates`]. With none, the
/// first candidate, so the Colour Setup line names where Resolve's own would be.
///
/// Resolve on Windows documents its scripting for a Python the user installs; the helper itself finds the
/// scripting module (`%PROGRAMDATA%\Blackmagic Design\DaVinci Resolve\Support\Developer\Scripting\Modules`) and
/// `fusionscript.dll` (in Resolve's install folder) when that Python cannot import it by itself.
pub fn resolve_python() -> PathBuf {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
    let candidates = python_candidates(
        var("ProgramFiles").as_deref(),
        var("SystemRoot").as_deref(),
        var("LOCALAPPDATA").as_deref(),
        var("PATH").as_deref(),
    );
    // `symlink_metadata`, not `exists`: the Python launcher and Store installs are app execution aliases,
    // reparse points that `exists` may not follow.
    candidates
        .iter()
        .find(|p| std::fs::symlink_metadata(p).is_ok())
        .or(candidates.first())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("python.exe"))
}

/// Where a Python able to run the helper may be, best first: a Python bundled with Resolve (as Linux's
/// ResolvePython, should Resolve ship one on Windows), the Python launcher `py.exe` (installed with python.org's
/// Python, it runs the newest Python 3 and passes the arguments on), then `python.exe` on `PATH`. The
/// `WindowsApps` folder goes last: its `python.exe` may be Windows' stand-in that only offers the Store.
fn python_candidates(
    program_files: Option<&OsStr>,
    system_root: Option<&OsStr>,
    local_app_data: Option<&OsStr>,
    path: Option<&OsStr>,
) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let program_files =
        program_files.map_or_else(|| PathBuf::from(r"C:\Program Files"), PathBuf::from);
    found.push(program_files.join(r"Blackmagic Design\DaVinci Resolve\ResolvePython.exe"));
    found.push(
        system_root
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
            .join("py.exe"),
    );
    if let Some(local) = local_app_data {
        found.push(PathBuf::from(local).join(r"Programs\Python\Launcher\py.exe"));
    }
    let (store, plain): (Vec<PathBuf>, Vec<PathBuf>) = path
        .map(|p| {
            std::env::split_paths(p)
                .filter(|d| !d.as_os_str().is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .partition(|d| d.ends_with("WindowsApps"));
    found.extend(plain.iter().map(|d| d.join("python.exe")));
    found.extend(store.iter().map(|d| d.join("py.exe")));
    found.extend(store.iter().map(|d| d.join("python.exe")));
    found
}

/// The job object every Resolve helper is put in. The app holds its only handle, never inheritable, and never
/// closes it, so Windows closes it when the app's process ends however it ends, and kill-on-close then ends
/// every helper in it: the job object is Windows' parent-death signal.
fn helper_job() -> io::Result<HANDLE> {
    // A HANDLE is a pointer, which a static cannot hold; it is kept as its address.
    static JOB: OnceLock<Result<usize, i32>> = OnceLock::new();
    let job = JOB.get_or_init(|| {
        // SAFETY: no security attributes (so the handle is not inheritable) and no name; the information
        // block is a valid JOBOBJECT_EXTENDED_LIMIT_INFORMATION of the size given.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(io::Error::last_os_error().raw_os_error().unwrap_or(0));
            }
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 {
                return Err(io::Error::last_os_error().raw_os_error().unwrap_or(0));
            }
            Ok(job as usize)
        }
    });
    match job {
        Ok(address) => Ok(*address as HANDLE),
        Err(code) => Err(io::Error::from_raw_os_error(*code)),
    }
}

/// Start the Resolve helper's process so it dies with the app, even when the app is killed or crashes: it goes
/// into the app's kill-on-close job object ([`helper_job`]). No console window. It holds nothing the app
/// opened, since Windows passes a child only the handles marked inheritable and the standard library marks only
/// its pipes. A helper that cannot be put in the job is killed at once, and the error returned.
///
/// The helper is put in the job just after it starts; an app killed in those microseconds would leave it
/// running until it next reads its closed stdin. The helper starts no processes of its own in that time.
pub fn spawn_tied_to_app(command: &mut Command) -> io::Result<Child> {
    command.creation_flags(CREATE_NO_WINDOW);
    let job = helper_job().map_err(|e| {
        io::Error::new(e.kind(), format!("no job object to tie it to the app: {e}"))
    })?;
    let mut child = command.spawn()?;
    // SAFETY: both handles are open: the job is never closed, the child's handle is owned by `child`.
    if unsafe { AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) } == 0 {
        let e = io::Error::last_os_error();
        let _ = child.kill();
        let _ = child.wait();
        return Err(io::Error::new(
            e.kind(),
            format!("it could not be tied to the app: {e}"),
        ));
    }
    Ok(child)
}

/// Start the system's opener on `url` (the default browser for a web link). `rundll32` takes the URL as it is,
/// where `cmd /C start` would split it at every `&`.
pub fn open_url(url: &str) -> std::io::Result<Child> {
    Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
}

/// The window's platform attributes: none needed. winit makes the app per-monitor DPI aware, so the Window's
/// scale factor follows the display it is on.
pub fn window_attributes(attributes: WindowAttributes) -> WindowAttributes {
    attributes
}

/// The graphics backend: DirectX 12. `WGPU_BACKEND` still overrides it, for trying another.
pub const GRAPHICS_BACKENDS: wgpu::Backends = wgpu::Backends::DX12;

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn candidates(path: &str) -> Vec<PathBuf> {
        python_candidates(
            Some(OsStr::new(r"D:\Apps")),
            Some(OsStr::new(r"C:\WINDOWS")),
            Some(OsStr::new(r"C:\Users\c\AppData\Local")),
            Some(&OsString::from(path)),
        )
    }

    #[test]
    fn resolve_bundled_python_comes_first_then_the_launcher() {
        let c = candidates("");
        assert_eq!(
            c,
            [
                PathBuf::from(r"D:\Apps\Blackmagic Design\DaVinci Resolve\ResolvePython.exe"),
                PathBuf::from(r"C:\WINDOWS\py.exe"),
                PathBuf::from(r"C:\Users\c\AppData\Local\Programs\Python\Launcher\py.exe"),
            ]
        );
    }

    #[test]
    fn python_on_path_comes_before_the_windows_apps_stand_in() {
        let c =
            candidates(r"C:\Users\c\AppData\Local\Microsoft\WindowsApps;C:\Python313;;C:\Tools");
        assert_eq!(
            &c[3..],
            [
                PathBuf::from(r"C:\Python313\python.exe"),
                PathBuf::from(r"C:\Tools\python.exe"),
                PathBuf::from(r"C:\Users\c\AppData\Local\Microsoft\WindowsApps\py.exe"),
                PathBuf::from(r"C:\Users\c\AppData\Local\Microsoft\WindowsApps\python.exe"),
            ]
        );
    }
}
