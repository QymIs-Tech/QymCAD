//! `qymcad-io`: geometry import and project files.

mod amf;
mod dxf_export;
mod dxf_import;
mod formats;
mod gltf;
mod iges_drawing;
mod obj;
mod ply;
mod part_file;
mod project_file;
mod rigid;
mod stl_export;
mod stl_import;
mod svg_export;
mod svg_import;
mod threemf;
mod units;
pub mod xml;

pub use amf::{export_amf, import_amf};
pub use dxf_export::export_dxf;
pub use dxf_import::import_dxf;
pub use formats::{Format, Lands};
pub use gltf::{export_glb, export_glb_tree, import_gltf};
pub use iges_drawing::{read_iges_drawing, IgesDrawing};
pub use obj::{export_obj, import_obj};
pub use ply::{export_ply, import_ply, import_ply_coloured};
pub use part_file::{load_part, load_part_bytes, load_part_manifest, load_part_manifest_bytes, load_part_thumb, load_part_thumb_bytes, save_part, LoadedPart};
pub use project_file::{content_weight, load_project, load_project_with_brep, LoadedProject, save_project, save_project_guarded, save_project_guarded_with_brep, save_project_with_brep};
pub use stl_export::export_stl;
pub use stl_import::{import_stl, import_stl_named};
pub use svg_export::export_svg;
pub use svg_import::import_svg;
pub use threemf::{export_3mf, export_3mf_tree, import_3mf};
pub use units::FileUnit;

use qymcad_core::geom::ProfEdge;

/// A mesh read from a file that names its pieces - an OBJ object, a glTF node, a 3MF or AMF object. The name is
/// empty where the file gives none; the colour, sRGB, is there where the file gives one. `place` is where the piece
/// stands, a 3x4 row-major placement, when the file keeps the piece in coordinates of its own; the identity when
/// the mesh is already where it stands.
pub struct NamedMesh {
    pub name: String,
    pub mesh: qymcad_core::geom::Mesh,
    pub color: Option<[u8; 3]>,
    pub place: [f64; 12],
    /// The colour of every triangle, where the file colours its faces one by one (PLY does); empty where it does not.
    pub tri_colors: Vec<[u8; 3]>,
    /// The groups of the file the piece stands in, from the top down - a glTF node that holds others: its number in
    /// the file, its name and its place in the group above it; `place` is then the piece's place in the last of them.
    /// Empty for a piece at the top of its file, and for every piece of a file that holds no groups.
    pub within: Vec<qymcad_core::model::FileGroup>,
}

/// A NUMBER THAT IS NOT FINITE, refused with its place and its value. Text parses `NaN` and `inf` as numbers and binary
/// floats carry them too: such a corner came into the document without a word, and a mesh with one then stopped the
/// recognition of its body. `key` names the format and the kind of place; the values follow the `#` apart by U+001F,
/// which `qymcad_i18n::name` reads into `$v`, `$w`, ... in order.
pub(crate) fn not_finite(key: &str, values: &[&dyn std::fmt::Display]) -> String {
    let values: Vec<String> = values.iter().map(|v| v.to_string()).collect();
    format!("{key}#{}", values.join("\u{1f}"))
}

/// A NAME A WRITER PUTS WHERE IT HAS NONE is no name: the program that wrote the owner's print head numbers its pieces
/// "empty_2", "empty_3"... in both its glTF and its OBJ. The piece is then named after its file, as an unnamed one is.
/// THE COLOURS OF A PIECE'S TRIANGLES as a piece carries them: a colour per triangle - the piece's own `colour`
/// standing for a triangle that names none - where any triangle shows another than the piece, and nothing where every
/// one shows the piece's. A piece coloured over all round keeps them too: its colour is then shown by no triangle. The
/// readers of glTF and 3MF both give them so.
pub(crate) fn several_colours(per: &[Option<[u8; 3]>], colour: Option<[u8; 3]>) -> Vec<[u8; 3]> {
    let Some(fill) = colour.or_else(|| per.iter().flatten().next().copied()) else { return Vec::new() };
    let all: Vec<[u8; 3]> = per.iter().map(|c| c.unwrap_or(fill)).collect();
    if all.iter().all(|c| *c == fill) {
        return Vec::new();
    }
    all
}

pub(crate) fn authored(name: String) -> String {
    match name.trim().strip_prefix("empty_") {
        Some(n) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => String::new(),
        _ => name,
    }
}

/// A colour component 0..1, sRGB, as a byte.
pub(crate) fn srgb_byte(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The result of importing 2D geometry: exact primitive curves — segments, arcs and circles — rather than a
/// tessellation. The import builds editable sketch entities from them, so a circle stays a circle and a fillet
/// stays an arc instead of becoming thousands of segments. Connectivity and closure are recovered from shared
/// endpoints by deduplicating the points.
#[derive(Clone, Debug, Default)]
pub struct ImportedSketch {
    pub curves: Vec<ProfEdge>,
    /// The kinds of entity the file holds that were not read, each with how many - named to a person, not dropped.
    pub skipped: Vec<(String, usize)>,
}
