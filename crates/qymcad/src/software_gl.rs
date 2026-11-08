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

/// FIRST THING IN A START ON THE PROCESSOR: Mesa is loaded before anything asks for OpenGL.
pub(crate) fn load_if_asked() {
    if this_start() == ThisStart::OnTheSystem {
        return;
    }
    if let Some(folder) = folder() {
        load(&folder.join("opengl32.dll"));
    }
}

/// Whether a start that found no adapter is followed by one more start, rather than by the message.
pub(crate) fn starts_again() -> bool {
    folder().is_some_and(|folder| next(carried(&folder), this_start()) == Next::StartAgainOnTheProcessor)
}

/// THE START ON THE PROCESSOR, with the arguments of this one. False when it could not be started, and then the
/// person is told why the window did not open.
pub(crate) fn start_again_on_the_processor() -> bool {
    let Ok(program) = std::env::current_exe() else { return false };
    starts_again() && std::process::Command::new(program).args(std::env::args_os().skip(1)).env(ASKED, "1").spawn().is_ok()
}

#[cfg(windows)]
fn load(library: &Path) {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut std::ffi::c_void;
    }
    let wide: Vec<u16> = library.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: a NUL-terminated wide path. The library stays loaded for the life of the process, which is the point.
    // A failure leaves the system's OpenGL, and the start ends with the message about the missing adapter.
    unsafe { LoadLibraryW(wide.as_ptr()) };
}

#[cfg(not(windows))]
fn load(_library: &Path) {}

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
