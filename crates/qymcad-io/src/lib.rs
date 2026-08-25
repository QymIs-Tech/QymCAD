//! `qymcad-io`: geometry import and project files.

mod dxf_export;
mod dxf_import;
mod part_file;
mod project_file;
mod stl_export;
mod stl_import;
mod svg_export;
mod svg_import;

pub use dxf_export::export_dxf;
pub use dxf_import::import_dxf;
pub use part_file::{load_part, load_part_bytes, load_part_manifest, load_part_manifest_bytes, load_part_thumb, load_part_thumb_bytes, save_part, LoadedPart};
pub use project_file::{content_weight, load_project, load_project_with_brep, save_project, save_project_guarded, save_project_guarded_with_brep, save_project_with_brep};
pub use stl_export::export_stl;
pub use stl_import::import_stl;
pub use svg_export::export_svg;
pub use svg_import::import_svg;

use qymcad_core::geom::ProfEdge;

/// The result of importing 2D geometry: exact primitive curves — segments, arcs and circles — rather than a
/// tessellation. The import builds editable sketch entities from them, so a circle stays a circle and a fillet
/// stays an arc instead of becoming thousands of segments. Connectivity and closure are recovered from shared
/// endpoints by deduplicating the points.
#[derive(Clone, Debug, Default)]
pub struct ImportedSketch {
    pub curves: Vec<ProfEdge>,
}
