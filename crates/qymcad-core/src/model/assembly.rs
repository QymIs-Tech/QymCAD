//! Assemblies: components, the tree, mates and external references.
//!
//! The split is mechanical: a method belongs here because it touches the fields of this subsystem alone, not
//! because of what it is called. Names are chosen by hand, so searching by name finds only what is expected.

use super::*;
use super::tess::*; // 2D sketch geometry: profiles, tessellation, region analysis.

impl Project {}

// --- Geometry of mate anchors ---
//
// A mate connector is a full coordinate system on a body, and resolving it from a geometry reference (a face,
// an edge, a vertex, a base plane) is the assembly's work rather than the general core's. This section is
// closed by call: the outside needs `connector_matrix` and `connector_frame`, and the rest is their
// machinery.

impl Project {}

/// ONE NODE OF AN IMPORTED TREE, under the name the file gives it: a part made of one body, or a group of nodes
/// (a subassembly in the document), or both. `place` is where it stands in its parent, a 3x4 row-major
/// placement; `solid` is the body's place among the file's bodies, which is how an `Import` node finds it again
/// in the embedded source; `color` is the colour of its part, where the file gives one. `repeat_of` marks an
/// occurrence of a product met before: it carries no body of its own and comes in as a clone of the part holding
/// that body.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportNode {
    pub name: String,
    pub place: [f64; 12],
    pub body: Option<Id>,
    pub solid: u32,
    pub color: Option<[u8; 3]>,
    /// Colours of single faces of the body, by the face's persistent id, as sRGB bytes.
    pub face_colors: Vec<(u32, [u8; 3])>,
    /// The colour of every triangle of a piece of a mesh, where the file colours its faces one by one; empty otherwise.
    pub tri_colors: Vec<[u8; 3]>,
    pub repeat_of: Option<u32>,
    /// The body is a piece of a mesh, not a solid: it comes in as a `MeshPiece` rather than an `Import`.
    pub mesh: bool,
    pub children: Vec<ImportNode>,
}

impl Default for ImportNode {
    fn default() -> Self {
        Self {
            name: String::new(),
            place: crate::feature::PLACE_IDENTITY,
            body: None,
            solid: 0,
            color: None,
            face_colors: Vec::new(),
            tri_colors: Vec::new(),
            repeat_of: None,
            mesh: false,
            children: Vec::new(),
        }
    }
}

impl ImportNode {
    /// A node with neither a body nor anything below it would come in as an empty subassembly.
    pub fn carries_something(&self) -> bool {
        self.body.is_some() || self.repeat_of.is_some() || self.children.iter().any(ImportNode::carries_something)
    }
}

/// A GROUP OF A FILE a piece stands in - a glTF node, a 3MF object or an AMF constellation that holds others: its
/// number in the file, its name, and its place in the group above it.
#[derive(Clone, Debug, PartialEq)]
pub struct FileGroup {
    pub index: usize,
    pub name: String,
    pub place: [f64; 12],
}

/// ONE NODE OF A TREE GOING OUT to an exact file: a subassembly (no body) or a part carrying its body, under its name,
/// where it stands in its parent. `same_as` points at an earlier node whose product this one repeats - a clone goes
/// out as a second occurrence of its original, not as a copy. `color` is the part's colour.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportNode {
    pub name: String,
    pub parent: Option<usize>,
    pub place: [f64; 12],
    pub body: Option<Id>,
    pub same_as: Option<usize>,
    pub color: Option<[u8; 3]>,
    /// Colours of single faces of the part's body, by the face's persistent id, as sRGB bytes.
    pub face_colors: Vec<(u32, [u8; 3])>,
}

/// THE MESH OF ONE BODY GOING OUT in a tree, in the part's own coordinates. `tri_colors` holds a colour for every
/// triangle when the faces carry colours of their own, and is empty when the part's colour covers it all.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportMesh {
    pub body: Id,
    pub mesh: crate::geom::Mesh,
    pub tri_colors: Vec<Option<[u8; 3]>>,
}

mod tree;
mod components;
mod joints;
mod drive;
mod hierarchy;
mod anchors;
mod mate_rules;
