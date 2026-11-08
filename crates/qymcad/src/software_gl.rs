//! DRAWING ON THE PROCESSOR WHERE THE SYSTEM OFFERS NOTHING TO DRAW WITH.
//!
//! Windows 10 offers an adapter on the processor by itself (WARP, through DX12). Windows 7 has no DX12, and its own
//! `opengl32.dll` is OpenGL 1.1, below the 3.3 the GL path of wgpu needs. Reported behaviour: on a machine with no
//! graphics driver the program stopped at "no graphics adapter" and opened no window. The Windows 7 zip carries Mesa
//! (llvmpipe) in `software-gl\` for that machine.
//!
//! Mesa is not put beside the program. A library named `opengl32.dll` there replaces the system's for every start, and
//! a card whose driver gives OpenGL and no Vulkan would draw on the processor. So the program first starts on what the
//! system has, and only a start that found no adapter starts once more, with Mesa loaded before anything asks for
//! OpenGL. That build delay-loads `opengl32.dll` (build.rs); Windows hands a library already loaded under a name to
//! every later request for that name, wherever it was loaded from.

use std::path::{Path, PathBuf};

/// The folder beside the program that holds Mesa's `opengl32.dll` and `libgallium_wgl.dll`.
pub(crate) const FOLDER: &str = "software-gl";
/// Set on the start that draws on the processor; its absence is the first start.
const ASKED: &str = "QYMCAD_SOFTWARE_GL";

/// Whether the package carries the renderer on the processor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Carried {
    Yes,
    No,
}

/// What this start draws with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ThisStart {
    OnTheSystem,
    OnTheProcessor,
}

/// What follows a start that found no adapter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Next {
    StartAgainOnTheProcessor,
    TellThePerson,
}

/// ONE MORE START, NEVER TWO: a start on the processor that found no adapter either is told, not repeated.
pub(crate) fn next(carried: Carried, this: ThisStart) -> Next {
    if carried == Carried::Yes && this == ThisStart::OnTheSystem {
        Next::StartAgainOnTheProcessor
    } else {
        Next::TellThePerson
    }
}

fn folder() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join(FOLDER))
}

fn carried(folder: &Path) -> Carried {
    if folder.join("opengl32.dll").is_file() {
        Carried::Yes
    } else {
        Carried::No
    }
}

fn this_start() -> ThisStart {
    if std::env::var_os(ASKED).is_some() {
        ThisStart::OnTheProcessor
    } else {
        ThisStart::OnTheSystem
    }
}

/// What loading Mesa came to, kept for the journal of the run, which is not open yet when Mesa is loaded.
static LOADED: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// FIRST THING IN A START ON THE PROCESSOR: Mesa is loaded before anything asks for OpenGL.
pub(crate) fn load_if_asked() {
    if this_start() == ThisStart::OnTheSystem {
        return;
    }
    let said = match folder() {
        Some(folder) => load(&folder.join("opengl32.dll")),
        None => "the folder of the program is unknown".to_string(),
    };
    let _ = LOADED.set(said);
}

/// The line for the journal of a start on the processor: what loading Mesa came to.
pub(crate) fn journal_line() -> Option<String> {
    LOADED.get().map(|said| format!("{FOLDER}: {said}"))
}

/// Whether a start that found no adapter is followed by one more start, rather than by the message.
pub(crate) fn starts_again() -> bool {
    folder().is_some_and(|folder| next(carried(&folder), this_start()) == Next::StartAgainOnTheProcessor)
}

/// THE START ON THE PROCESSOR, with the arguments of this one, or why it could not be started.
///
/// THE NEW START GETS NO HANDLE OF THIS ONE. Measured on Windows 7: with the streams inherited, a program started from a
/// console did not start again, and the person was told only of the missing adapter; started from Explorer it did.
/// With the streams of the new start closed both start again and draw. A failure that remains is said with the message.
pub(crate) fn start_again_on_the_processor() -> Result<(), String> {
    let program = std::env::current_exe().map_err(|e| format!("the program does not know its own file: {e}"))?;
    std::process::Command::new(&program)
        .args(std::env::args_os().skip(1))
        .env(ASKED, "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|e| format!("{} did not start again on the processor: {e}", program.display()))
}

/// MESA AND ITS LIBRARY FROM ONE FOLDER, AND NO WINDOW OF THE SYSTEM ON A FAILURE.
///
/// A plain LoadLibraryW looks for the libraries a library imports beside the program, not beside that library.
/// Measured on Windows 7: Mesa's opengl32.dll from software-gl\ failed for its libgallium_wgl.dll, and the system put up
/// "libgallium_wgl.dll is missing" in the middle of the start. LOAD_WITH_ALTERED_SEARCH_PATH starts the search in the
/// folder of the library; SEM_FAILCRITICALERRORS turns the window into an error code for the journal.
#[cfg(windows)]
fn load(library: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;
    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x8;
    const SEM_FAILCRITICALERRORS: u32 = 0x1;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryExW(name: *const u16, file: *mut std::ffi::c_void, flags: u32) -> *mut std::ffi::c_void;
        fn SetThreadErrorMode(mode: u32, old: *mut u32) -> i32;
    }
    let wide: Vec<u16> = library.as_os_str().encode_wide().chain([0]).collect();
    let mut before = 0u32;
    // SAFETY: a NUL-terminated wide path and a valid place for the old mode. The library stays loaded for the life of
    // the process, which is the point; the error mode of the thread is put back as it was.
    let loaded = unsafe {
        SetThreadErrorMode(SEM_FAILCRITICALERRORS, &mut before);
        let module = LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH);
        let code = std::io::Error::last_os_error();
        SetThreadErrorMode(before, std::ptr::null_mut());
        if module.is_null() {
            Err(code)
        } else {
            Ok(())
        }
    };
    match loaded {
        Ok(()) => format!("loaded {}", library.display()),
        // a failure leaves the system's OpenGL, and the start ends with the message about the missing adapter
        Err(e) => format!("could not load {}: {e}", library.display()),
    }
}

#[cfg(not(windows))]
fn load(library: &Path) -> String {
    format!("not loaded {}: there is no such library off Windows", library.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_start_with_no_adapter_starts_again_on_the_processor() {
        assert_eq!(next(Carried::Yes, ThisStart::OnTheSystem), Next::StartAgainOnTheProcessor);
    }

    #[test]
    fn a_start_on_the_processor_with_no_adapter_is_told_not_repeated() {
        assert_eq!(next(Carried::Yes, ThisStart::OnTheProcessor), Next::TellThePerson);
    }

    #[test]
    fn a_package_without_the_renderer_tells_at_once() {
        assert_eq!(next(Carried::No, ThisStart::OnTheSystem), Next::TellThePerson);
    }
}
