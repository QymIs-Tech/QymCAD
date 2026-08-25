//! `qymcad-cam`: the post-processor engine.
//!
//! It takes the toolpath intermediate representation together with options and returns G-code text. The
//! reference post is Mach3, implemented natively in [`mach3`]. A scripting layer is the next stage on top of
//! this interface.

pub mod mach3;

use qymcad_core::ir::Program;
use qymcad_core::model::PostKind;

/// Generate G-code for the chosen controller.
pub fn post_for(program: &Program, kind: PostKind, opts: &PostOptions) -> String {
    let dialect = match kind {
        PostKind::Mach3 => mach3::MACH3,
        PostKind::Grbl => mach3::GRBL,
        PostKind::LinuxCnc => mach3::LINUXCNC,
    };
    mach3::emit(program, &dialect, opts)
}

/// Post-processor options.
#[derive(Clone, Debug)]
pub struct PostOptions {
    pub output_comments: bool,
    pub output_header: bool,
    pub output_line_numbers: bool,
    pub axis_precision: u8,
    pub feed_precision: u8,
    pub spindle_decimals: u8,
    pub output_tool_length_offset: bool,
    pub translate_drill_cycles: bool,
}

impl Default for PostOptions {
    /// The defaults of the baseline post-processor.
    fn default() -> Self {
        Self {
            output_comments: true,
            output_header: true,
            output_line_numbers: false,
            axis_precision: 3,
            feed_precision: 3,
            spindle_decimals: 1,
            output_tool_length_offset: true,
            translate_drill_cycles: false,
        }
    }
}

/// Run a post over a program. To be implemented with the scripting layer.
pub fn run_post(_script: &str, _program: &Program, _opts: &PostOptions) -> Result<String, String> {
    Err("post-not-implemented".into())
}
