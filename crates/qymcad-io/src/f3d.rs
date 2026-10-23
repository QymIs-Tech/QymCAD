//! F3D: a parametric CAD archive. The bodies in it are kept in a proprietary B-rep form, and decoding that
//! form in place is its own large work; the open `cadmpeg` converter already turns it into STEP, which the
//! document's own exact reader opens. The converter is looked up on PATH, run once per file, and its absence
//! is a refusal of its own, said before the reading starts, not an error in the file.

use std::path::{Path, PathBuf};

/// THE CONVERTER, as it is named in the search path.
pub const CONVERTER: &str = "cadmpeg";

/// THE TWO FILES OF ONE CONVERSION: the archive read and the file written, both whole paths.
pub struct InOut {
    pub input: String,
    pub output: String,
}

/// The name the converter carries as a program: Windows names its executables with the extension.
fn program() -> &'static str {
    if cfg!(windows) {
        "cadmpeg.exe"
    } else {
        CONVERTER
    }
}

/// THE CONVERTER LOOKED UP in the `search` value of a PATH-like variable: the first directory that holds the
/// program. A directory that is gone or unreadable is passed over, as a search path may list several.
pub fn find_in(search: &std::ffi::OsStr) -> Option<PathBuf> {
    std::env::split_paths(search).map(|dir| dir.join(program())).find(|full| full.is_file())
}

/// THE CONVERTER ON THE PATH OF THIS PROCESS.
pub fn find_converter() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|search| find_in(&search))
}

/// CONVERT `files.input` INTO `files.output` with the converter at `converter`; the output extension selects
/// the format (`... .step` gives STEP). `Err` carries the key of the refusal, and after the `#` the
/// converter's exit status and the last line it said, apart by the unit separator (`i18n::name` reads them
/// into `$v` and `$w`). A failing converter has said why on its own stream; its last line is the one a person
/// reads first.
pub fn convert_with(converter: &Path, files: &InOut) -> Result<(), String> {
    let run = std::process::Command::new(converter).arg("convert").arg(&files.input).arg("-o").arg(&files.output).arg("--force").output();
    match run {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let said = String::from_utf8_lossy(&out.stderr);
            let said = said.lines().next_back().unwrap_or_default().trim();
            Err(format!("io-f3d-convert-failed#{}\u{1f}{said}", out.status))
        }
        Err(e) => Err(format!("io-f3d-run-failed#{e}")),
    }
}

/// CONVERT `files.input` into `files.output` with the converter of this process's PATH. `Err` names a
/// converter that is not installed as its own kind of refusal: the way back is an installation, not a
/// different file.
pub fn convert_to_step(files: &InOut) -> Result<(), String> {
    let Some(converter) = find_converter() else { return Err("io-f3d-converter-missing".into()) };
    convert_with(&converter, files)
}
