//! The global tool library, kept in the OS configuration directory.
//!
//! It is stored separately from projects; a project embeds COPIES of what it uses. "Import from the
//! library" means copying a tool into `project.tools`.

use std::path::PathBuf;

use qymcad_core::tool::{Tool, ToolLibrary, ToolType};

fn config_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("tech", "qymis", "qym-cad").map(|d| d.config_dir().to_path_buf())
}

/// Path to the global tool library file.
pub fn tool_library_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("tools.ron"))
}

/// Load the global library (or the default set when there is no file).
pub fn load_tool_library() -> ToolLibrary {
    if let Some(p) = tool_library_path() {
        if let Ok(s) = std::fs::read_to_string(&p) {
            if let Ok(lib) = ron::from_str::<ToolLibrary>(&s) {
                return lib;
            }
        }
    }
    default_library()
}

/// Save the global library into the configuration directory.
pub fn save_tool_library(lib: &ToolLibrary) -> Result<(), String> {
    let dir = config_dir().ok_or_else(|| crate::i18n::tr("io-no-config-dir"))?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let s = ron::ser::to_string_pretty(lib, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("tools.ron"), s).map_err(|e| e.to_string())
}

fn t(number: u32, name: &str, kind: ToolType, diameter: f64, corner_radius: f64, flutes: u32, v_angle: Option<f64>) -> Tool {
    Tool { number, name: name.into(), kind, diameter, corner_radius, flutes, v_angle }
}

/// The starting set of tools, used when no library exists yet.
fn default_library() -> ToolLibrary {
    ToolLibrary {
        tools: vec![
            t(1, &crate::i18n::tr("cam-tool-endmill-6"), ToolType::FlatEnd, 6.0, 0.0, 2, None),
            t(2, &crate::i18n::tr("cam-tool-endmill-3"), ToolType::FlatEnd, 3.0, 0.0, 2, None),
            t(3, &crate::i18n::tr("cam-tool-ballnose-6"), ToolType::BallNose, 6.0, 3.0, 2, None),
            t(4, &crate::i18n::tr("cam-tool-vbit-60"), ToolType::VBit, 6.0, 0.0, 1, Some(60.0)),
            t(5, &crate::i18n::tr("cam-tool-drill-5"), ToolType::Drill, 5.0, 0.0, 2, None),
        ],
    }
}
