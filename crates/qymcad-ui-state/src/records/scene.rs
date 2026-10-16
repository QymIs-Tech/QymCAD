//! Imports, the clipboard, the camera, the help window, the parameters of a command, the blocks of the scene, references and snapshots.

use super::*;

/// AN IMPORT WAITING TO BE PLACED: the curves have been read, but where they go has not been said yet.
/// Together with the drawing points that came from the same import, this is one unfinished action.
#[derive(Clone, Default)]
pub struct PendingImport {
    /// the curves read, their source and the file name
    pub curves: Option<(Vec<qymcad_core::geom::ProfEdge>, Option<Id>, String)>,
    /// the points to be drawn once it has been placed
    pub draw_pts: Option<Vec<Point2>>,
}

impl PendingImport {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// A FILE READ AND LAID IN, WAITING FOR ITS UNITS AND SCALE. The bodies are already in the document inside an open
/// edit, so the answer shows on screen as it is given; Enter closes the edit, Esc rolls it back.
pub struct ImportScale {
    /// The file's name, for the window's line.
    pub file: String,
    /// The format's name: the key the unit last chosen for it is remembered under.
    pub format: String,
    /// The file carries no unit (STL, OBJ, PLY): the window offers units, not only a factor.
    pub unitless: bool,
    /// The factor asked for, from the file's own numbers.
    pub factor: f64,
    /// The factor the bodies in the document are at now.
    pub applied: f64,
    /// The sides of the box of everything that came in, as the file has it.
    pub span: [f64; 3],
    /// The body of every mesh that came in: the document scales a piece itself, from the factor it stands at.
    pub meshes: Vec<Id>,
    /// Every solid that came in: its body, and the shape as read.
    pub solids: Vec<(Id, qymcad_kernel::Shape)>,
    /// Every component that came in with the file's tree: its id, and where the file places it.
    pub places: Vec<(Id, [f64; 12])>,
    /// The factor the import stood at when it was asked about again from its node; `None` for a file just read. The
    /// window then applies a new factor rather than bringing a file in, and Esc puts this one back.
    pub again: Option<f64>,
}

/// THE SKETCH WORKING SESSION: which one is open for editing and which one was the last. Kept apart,
/// "open" could be cleared while "last" still held a different sketch — and the next command took hold
/// of the wrong one.
#[derive(Clone, Copy, Default)]
pub struct SketchSession {
    /// the sketch open for editing (None means not in sketch mode)
    pub editing: Option<Id>,
    /// the last one worked on (for commands that act "on the last sketch")
    pub last: Option<Id>,
    /// THE KEY OF THE OPEN SKETCH WHEN IT WAS OPENED (`Project::sketch_key`): closed with the same key, it changed
    /// nothing the bodies on it are built from, and nothing is rebuilt (`rebuild_after_sketch`)
    pub opened: u64,
}

impl SketchSession {
    /// No sketch open.
    pub const CLOSED: SketchSession = SketchSession { editing: None, last: None, opened: 0 };
}

/// THE CLIPBOARD: sketch geometry and tree nodes are different things, but they share one state of "what has been copied".
#[derive(Clone, Default)]
pub struct Clipboard {
    pub geom: Option<qymcad_core::model::GeomClip>,
    pub geom_pending: Option<(Vec<Id>, bool)>,
    /// THE PLACE OF A PASTE IS AWAITED: a ghost follows the pointer, a click places it. A copy or a cut waits for its
    /// base point alone and fills the clipboard (`geom_pending`); nothing follows it until a paste.
    pub geom_place: bool,
    pub tree: Option<TreeClip>,
    pub tree_multi: Option<(Vec<Id>, bool)>,
    pub os_ping: bool,
}

impl Clipboard {
    /// A COPY (or with `cut`, a cut) OF THE GEOMETRY `eids` taken: its base point is awaited; a paste waiting before it
    /// goes.
    pub fn arm_copy(&mut self, eids: Vec<Id>, cut: bool) {
        (self.geom_pending, self.geom_place) = (Some((eids, cut)), false);
    }

    /// An unfinished copy or paste of geometry put down.
    pub fn drop_geom(&mut self) {
        (self.geom_pending, self.geom_place) = (None, false);
    }
}

/// The orbit camera for the software 3D view (the projection comes from `Settings::projection`).
#[derive(Clone, Copy)]
pub struct Cam3 {
    pub yaw: f64,
    pub pitch: f64,
    pub scale: f32,
    pub target: [f64; 3],
    pub init: bool,
    /// The scale the view was last framed at. The wheel's limits are taken from it, so a model of 38 m and one of
    /// 0.1 mm both zoom on from where they were framed.
    pub fit: f32,
    /// THE TILT about the line of sight, in radians: turned by the tilt of the Gesture layout, back to nothing with any
    /// standard view of the cube.
    pub roll: f64,
}

impl Default for Cam3 {
    fn default() -> Self {
        // Front isometric: the camera sits at +X -Y +Z (front = -Y towards the viewer), the usual choice.
        // +Y runs up and away, so the "top" of a sketch on the XY plane matches the "top" in 3D.
        Self { yaw: -0.7, pitch: 0.6, scale: 4.0, target: [0.0; 3], init: false, fit: 4.0, roll: 0.0 }
    }
}

impl Cam3 {
    pub fn basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let dir = [self.pitch.cos() * self.yaw.cos(), self.pitch.cos() * self.yaw.sin(), self.pitch.sin()];
        let fwd = [-dir[0], -dir[1], -dir[2]];
        // TOP AND BOTTOM VIEWS (fwd parallel to Z): world Z as the "up" gives a zero cross product, so the
        // basis degenerates and the picture collapses. In that case world Y is taken as the reference
        // "up" (the ViewCube's top no longer breaks the view).
        let ref_up = if fwd[2].abs() > 0.999 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
        let right = v_norm(v_cross(fwd, ref_up));
        let up = v_norm(v_cross(right, fwd));
        if self.roll == 0.0 {
            return (right, up, fwd);
        }
        let (c, sn) = (self.roll.cos(), self.roll.sin());
        let turn = |a: [f64; 3], b: [f64; 3], ka: f64, kb: f64| [a[0] * ka + b[0] * kb, a[1] * ka + b[1] * kb, a[2] * ka + b[2] * kb];
        (turn(right, up, c, sn), turn(up, right, c, -sn), fwd)
    }
}

/// THE STATE OF THE HELP WINDOW, gathered out of the pile of window flags.
///
/// Four fields that only ever move together - open, which article, where Back leads, what is typed into
/// the search. In the pile they were four unrelated names among fifteen, and nothing said they belonged
/// to one another.
#[derive(Default)]
pub struct HelpWin {
    pub open: bool,
    pub article: String,
    pub back: Vec<String>,
    pub query: String,
}

impl HelpWin {
    /// Step back to the article visited before this one, if there is one.
    pub fn go_back(&mut self) {
        if let Some(prev) = self.back.pop() {
            self.article = prev;
        }
    }

    /// Whether there is somewhere to go back to.
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }
}

/// One editable dimension of the active Part command: the label + the key (for `feat_dims`) + the current value +
/// the input buffer (a number OR an expression like `w/2+3`). An on-screen field like the sketch dimensions have.
#[derive(Clone)]
pub struct CmdParam {
    /// A CATALOGUE KEY rather than a finished phrase: the field's label must follow a change of language by itself,
    /// without the command being recreated. The key is stable; the translation is taken at the moment of display.
    pub label: LabelKey,
    /// The dimension's key. A STRING rather than a `&'static str`: a variable fillet has as many fields as there
    /// are vertices in the table, and each has a key of its own of the form `at{vertex descriptor}`. That cannot be
    /// expressed as a list of constants, and a second field mechanism for one tool would fork into copies what
    /// already works: the popup, the expressions, the parametrics, Enter and Esc.
    pub key: String,
    pub val: f64,
    pub txt: String,
    pub lo: f64,
    pub hi: f64,
    /// ZERO IS NO VALUE for this field, though both signs are: a face pushed by 0 is the body unchanged.
    pub nonzero: bool,
    /// WHERE THIS FIELD LIVES IN SPACE. `None` means the command's shared popup; a point means a small window of
    /// its own at that place (a radius at a vertex is shown AT THE VERTEX, otherwise six identical fields in a
    /// column cannot be told apart).
    pub at: Option<[f64; 3]>,
}

impl CmdParam {
    /// The KEY of the field's label. The words are chosen by whoever draws it - a record of the
    /// interface holds keys and never reaches for the dictionary.
    pub fn label_key(&self) -> &'static str {
        self.label.0
    }

    /// Change the label on the fly (on a chamfer the second leg becomes an angle when the mode changes).
    pub fn set_label(&mut self, key: &'static str) {
        self.label = LabelKey(key);
    }

    pub fn new(label: &'static str, key: &str, val: f64, lo: f64, hi: f64) -> Self {
        Self { label: LabelKey(label), key: key.to_string(), val, txt: format!("{val:.2}"), lo, hi, at: None, nonzero: false }
    }

    /// A field AT THE GEOMETRY: the same thing, but with a place of its own in space.
    pub fn at(mut self, p: [f64; 3]) -> Self {
        self.at = Some(p);
        self
    }

    /// A field for which zero is no value.
    pub fn nonzero(mut self) -> Self {
        self.nonzero = true;
        self
    }
}

/// A READY PIECE OF THE SCENE BUFFER FOR ONE BODY.
///
/// THE KEY IS SPLIT IN TWO, AND THAT IS THE WHOLE POINT. `shape` is about the body's FORM and appearance (the
/// mesh, the highlight, the ghost state, the colour, the display settings, the cutting plane); `at` is where the
/// body stands. The position used to be part of one shared key, and MOVING a part declared the block stale
/// entirely: a drag rebuilt 63 blocks out of 138 (the driven side was a gantry subassembly), and rebuilding a
/// block means clipping every triangle by the section plane, world normals and three colours per triangle.
///
/// A move changes NEITHER THE FORM NOR THE COLOUR. If the part is oriented as before and merely shifted, the
/// ready vertices need only have the difference added - one addition per vertex instead of the whole
/// manufacture. A rotation and a section cannot be carried over that way (the colour is computed from the WORLD
/// normal and the section cuts in world space), so there the block is built anew.
pub struct SceneBlock {
    /// The key of the FORM and appearance - everything except the position.
    pub shape: u64,
    /// The position these vertices already stand at.
    pub at: [f64; 12],
    /// The number these vertices name: the body's row in the look table the last time the block was used.
    pub body: u32,
    /// SHARED, NOT OWNED OUTRIGHT: the frame takes the block as it is, without copying a single vertex.
    ///
    /// Every rebuild of the scene used to glue all the blocks into one vector purely to hand it to the
    /// upload - a second full copy of the scene, 739 MB of it on the reference engine, at the worst possible
    /// moment. Now the pieces are uploaded one after another, each straight from its block.
    ///
    /// A move still shifts the vertices in place: after the frame has been drawn the block is the only owner,
    /// so `Arc::make_mut` copies nothing.
    /// ALL the geometry of the body. It used to be split into opaque and translucent here, which tied the
    /// DATA to the look: a body turning into a ghost had to be rebuilt to move its vertices into the other
    /// half. The pass is chosen when drawing, from the look table.
    ///
    /// Several parts only when the body is too big for one buffer of the card (see `BLOCK_VERTEX_CAP`);
    /// ordinarily there is exactly one.
    pub parts: Vec<BlockPart>,
}

/// ONE PIECE OF A BODY as the card takes it: the vertices, and the triangles as indices into them.
///
/// A VERTEX OF THE BLOCK IS A VERTEX OF THE MESH, one for one. Each used to be written out three times over -
/// once per triangle that touches it - because the colour and the normal made neighbours differ; now the
/// vertex carries only a position and the number of its body, so the triangles simply point at it.
/// Measured on a closed mesh: about three times fewer vertices, and 4 bytes of index against 20 of vertex.
///
/// The exception is the vertices BORN OF A SECTION: clipping a triangle by the plane makes points that are in
/// no mesh. They are appended at the end of the block and indexed in order.
#[derive(Clone)]
pub struct BlockPart {
    pub verts: std::sync::Arc<Vec<GpuVert>>,
    pub idx: std::sync::Arc<Vec<u32>>,
}

/// HOW BIG ONE PART OF A BLOCK MAY GROW, in vertices and in indices.
///
/// An indexed draw reads from ONE buffer, so a part must fit inside a single piece of the scene, and a piece
/// may not exceed the device's limit on a buffer. The smallest such limit met in practice is 256 MB: 4.19
/// million vertices at 20 bytes is 84 MB, and 16.7 million indices at 4 bytes is 67 MB, so both stay well
/// inside it on any device. A body bigger than that is cut into parts when the block is built.
pub const BLOCK_VERTEX_CAP: usize = 1 << 22;
/// The companion of `BLOCK_VERTEX_CAP` for the triangles: a mesh has about twice as many triangles as
/// vertices, so the indices are the number that runs out first.
pub const BLOCK_INDEX_CAP: usize = 1 << 24;

/// The extent of an extrude or a cut (in place of the magic numbers 0 to 3). The discriminants are kept as the
/// former u8 in case of outside places, but the comparisons go by variant.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtentMode {
    #[default]
    Length, // to a length (one side)
    Symmetric,
    TwoSided, // two sides (the "down" field is active)
    Through,  // through all (operations on a body only)
}

impl ExtentMode {
    pub fn symmetric(self) -> bool {
        matches!(self, ExtentMode::Symmetric)
    }
    pub fn two_sided(self) -> bool {
        matches!(self, ExtentMode::TwoSided)
    }
    pub fn through(self) -> bool {
        matches!(self, ExtentMode::Through)
    }
    /// Restore the mode from a saved feature's flags (through takes priority, then symmetry, then two-sided).
    pub fn from_extent(reach: qymcad_core::feature::Reach, down: f64, through: bool) -> Self {
        if through {
            ExtentMode::Through
        } else if reach == qymcad_core::feature::Reach::BothWays {
            ExtentMode::Symmetric
        } else if down.abs() > 1e-9 {
            ExtentMode::TwoSided
        } else {
            ExtentMode::Length
        }
    }
}

/// The TREE clipboard: what was copied or cut (by stable Id) and which of the two it was.
#[derive(Clone, Copy, PartialEq)]
pub enum TreeClip {
    /// A sketch node (by the sketch's Id).
    Sketch { sid: Id, cut: bool },
    /// A component - a Part or a subassembly (by the component's Id).
    Component { id: Id, cut: bool },
    /// A node of a part's timeline (by the node's Id): pasting opens its tool with its values, to be placed anew.
    Feature { nid: Id },
}

/// A reference for the dimension tool: a point or a straight line (a line or an axis). A dimension between two
/// references: point to point, point to line (a perpendicular), line to a parallel line, or to an axis.
#[derive(Clone, Copy, PartialEq)]
pub enum DimRef {
    Point(Id),
    Line(Id, Id),
}

/// A base edge for building tangents (a tangent arc or a tangent circle): either a straight line (with ends a
/// and b) or a circle or arc (a centre plus a radius).
#[derive(Clone, Copy, PartialEq)]
pub enum EdgeRef {
    Line { a: Id, b: Id },
    Circle { center: Id, r: f64 },
}

/// The sketch's diagnostics in one piece: the degrees of freedom and the redundancy, the free points, THE ARGUING
/// SET of constraints and the redundant ones. Computed in a single pass in `sketch_diag`.
/// The diagnostics of one sketch and what they were computed for.
pub struct SketchStatus {
    pub si: usize,
    /// `sketch_shape_key` at the time
    pub shape: u64,
    /// `sketch_place_key` at the time
    pub place: u64,
    pub diag: SketchDiag,
}

#[derive(Clone, Default)]
pub struct SketchDiag {
    /// (the degrees of freedom, the redundancy)
    pub dof: (i32, i32),
    /// per sketch point: whether that point can move
    pub free: Vec<bool>,
    /// the indexes of the constraints that contradict one another TOGETHER - any one of the set may be removed
    pub conflicts: std::collections::HashSet<usize>,
    /// the indexes of the constraints whose removal does not raise the degrees of freedom (consistent redundancy)
    pub redundant: std::collections::HashSet<usize>,
}

/// AN UNDO SNAPSHOT IS ONE STATE OF THE DOCUMENT.
///
/// A snapshot used to be assembled from three pieces (the project, the faces and the visibility), while the live
/// B-rep was not part of it at all and was restored by a separate mechanism. While the state is smeared about, an
/// undo regularly under-rolls something: after Ctrl+Z the mesh showed the old geometry while the kernel's cache
/// held the new one. The faces and the visibility now live in the body itself, and a snapshot is the whole project.
#[derive(Clone)]
pub struct Snapshot {
    pub project: Project,
}

/// AN UNFINISHED DRAWING: which shape is being finished right now.
///
/// AN ENUMERATION RATHER THAN THREE `Option`s: exactly ONE THING is ever being drawn, and the type must say so.
/// While these were three independent fields, "a rectangle and an ellipse at once" remained a possible state -
/// held off by discipline in the code rather than by the type system.
#[derive(Clone, Default, PartialEq)]
pub enum PlacingShape {
    #[default]
    None,
    /// a rectangle: two corners and the entities it created
    Rect { a: Point2, b: Point2, ids: Vec<Id> },
    /// a rectangle drawn from the centre: the centre stays put while the width and height are typed,
    /// the corner under the pointer follows them
    RectCenter { center: Point2, corner: Point2, ids: Vec<Id> },
    /// a polygon by its inscribed or circumscribed circle
    Poly(Id),
    /// an ellipse: the entity and its centre
    Ellipse(Id, Point2),
    /// a rectangle reopened by a double click on a side: its width and height dimensions, edited in place
    RectDims(qymcad_core::model::RectDims),
}
