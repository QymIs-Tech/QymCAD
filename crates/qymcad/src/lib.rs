//! QymCAD - a desktop CAD application (egui plus wgpu), as a library.
//!
//! The executable is a thin `main` over [`run`]. Everything else stays private to the crate: the acceptance checks
//! reach the program through its session alone, so they touch it the way a person does and nothing else.

use std::process::ExitCode;

pub use gui::session::{pos2, vec2, Chooser, Datum, Document, FaceKinds, Feature, Gizmo, Inspection, JointInfo, Key, Kept, Kind, Machine, Modifiers, Parameter, Part, Picture, PointerButton, Pos2, Rect, Session, SketchInfo, SketchPick, Solid, Vec2, Widget};

/// START THE PROGRAM: open the window and run it until it closes; a start that fails is told to the person.
pub fn run() -> ExitCode {
    match gui::launch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("failed to start QymCAD: {e}");
            // A WINDOWED BUILD HAS NOWHERE TO PRINT: on Windows there is no console behind the program, so
            // the line above reached nobody. What a person saw was a window that blinked and closed.
            match crate::diagnostics::start_failure() {
                Some(f) => crate::start_notice::tell_the_person(&f),
                // the failure happened somewhere that did not record itself - say the little that is known
                None => crate::start_notice::tell_the_person(&crate::diagnostics::StartFailure { reason: e.to_string(), report: None, no_adapter: false }),
            }
            ExitCode::FAILURE
        }
    }
}


#[cfg(test)]
mod comment_ratchet;
#[cfg(test)]
mod dependency_ratchet;
mod god_object_ratchet;
// The macOS bundling script, run here with the mac-only tools stubbed out. Unix only: it is a shell script.
#[cfg(all(test, unix))]
mod packaging_macos;
// The Linux packaging script, read rather than run: what it must carry is decided by the dependency tree.
#[cfg(test)]
mod packaging_linux;
// The AUR package description: three versions that must agree, and a checksum that must be real.
#[cfg(test)]
mod packaging_aur;
// The winget manifest and the MSI it describes: one product code computed in two languages.
#[cfg(test)]
mod packaging_winget;
// The Flatpak manifest: one application id in six places, and a build that reaches no network.
#[cfg(test)]
mod packaging_flatpak;
// The release run itself: every package built is handed over, and the publishing job waits for them all.
#[cfg(test)]
mod packaging_release;
// Every build of the kernel keeps the checks OCCT compiles out of a Release by default.
#[cfg(test)]
mod packaging_occt;
// One reverse-DNS name for the program, in the four places that cannot share a constant.
#[cfg(test)]
mod app_identity;
mod build_info;
mod crash;
mod diagnostics;
/// THE DICTIONARY LIVES IN ITS OWN CRATE NOW. Re-exported under the old name because 2159 places say
/// `crate::i18n::tr`, and rewriting every one of them would change no meaning at all.
pub(crate) use qymcad_i18n as i18n;
/// The colour schemes live in their own crate now; the old name is kept so nothing else has to change.
pub(crate) use qymcad_scheme as palette;
pub(crate) use qymcad_help as help;
pub(crate) use qymcad_help::map as help_map;
mod command_catalog;
mod templates;
mod gui;
mod parts_library;
mod viewport_gpu;
mod start_notice;
mod system;
mod wide_signature_ratchet;


