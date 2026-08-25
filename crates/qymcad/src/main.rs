//! QymCAD - a desktop CAD application (egui plus wgpu). Pure GUI.
//!
//! All the logic (import, operations, IR, post-processor, verify) lives in the headless crates
//! core/io/post/verify; this is only the window. `gui` builds a `Project` and assembles the
//! program through `Project::build_program`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(test)]
mod comment_ratchet;
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

use std::process::ExitCode;

fn main() -> ExitCode {
    match gui::launch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("failed to start QymCAD: {e}");
            ExitCode::FAILURE
        }
    }
}
