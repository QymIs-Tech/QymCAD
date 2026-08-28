//! QymCAD - a desktop CAD application (egui plus wgpu). Pure GUI.
//!
//! All the logic (import, operations, IR, post-processor, verify) lives in the headless crates
//! core/io/post/verify; this is only the window. `gui` builds a `Project` and assembles the
//! program through `Project::build_program`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(test)]
mod comment_ratchet;
#[cfg(test)]
mod dependency_ratchet;
// The macOS bundling script, run here with the mac-only tools stubbed out. Unix only: it is a shell script.
#[cfg(all(test, unix))]
mod packaging_macos;
mod build_info;
mod crash;
mod diagnostics;
mod i18n;
mod palette;
mod help;
mod help_map;
mod command_catalog;
mod templates;
mod gui;
mod library;
mod parts_library;
mod viewport_gpu;
mod start_notice;

use std::process::ExitCode;

fn main() -> ExitCode {
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
