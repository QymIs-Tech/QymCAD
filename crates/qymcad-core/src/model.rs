//! Project model: geometry, the tool library and the feature timeline.
//!
//! `Project` is serialised into `*.qcad` (RON). Operations reference contours by index and tools by
//! number instead of copying them. `build_program` collects every enabled operation, in order, into a
//! single `Program` for the post-processor.

use crate::geom::{Bbox, Contour, Point2};
use serde::{Deserialize, Serialize};

/// THE UNITS THE DOCUMENT IS WRITTEN IN. A property of the drawing, not of any one operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Units {
    #[default]
    Mm,
    Inch,
}

/// Stable identifier of an entity (contour, mesh and so on). Operations reference entities by `Id`
/// rather than by array index, so a reference survives insertion, removal and reordering.
pub type Id = u64;

/// A body of the document: geometry, its faces, its name and its visibility in one place.
///
/// `mesh` is the tessellation for the screen and for export, `faces` are the recognised faces (they
/// survive a reload and live in the bundle next to the mesh), `visible` says whether to draw it in 3D.
/// The live B-rep (`Shape`) is deliberately not part of this: it is not serialised and is rebuilt by the
/// kernel on demand.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub id: Id,
    pub name: String,
    pub mesh: crate::geom::Mesh,
    #[serde(default)]
    pub faces: Vec<crate::geom::MeshFace>,
    #[serde(default = "crate::model::yes")]
    pub visible: bool,
    /// A sheet (surface) rather than a solid. A sheet has no volume, so it has no mass, no material and
    /// no toolpath, and the "one part is one body" rule does not count it either. The kernel sets this
    /// flag on rebuild and it is stored in the file: opening a document does not rebuild, and knowing
    /// that a body is a surface is needed immediately.
    #[serde(default)]
    pub sheet: bool,
}

pub(crate) fn yes() -> bool {
    true
}

/// Geometry accuracy is a property of the document, not a setting of the application.
///
/// It decides both the picture on screen and what goes into an STL. As an application setting the same
/// file would export differently for two people, each of them convinced the program is at fault. As a
/// document property it travels with the file: jewellery needs one accuracy and a machine frame another,
/// and the choice belongs to whoever drew it, not to whoever opened it.
///
/// Stored as steps rather than as a number: deflection in millimetres is a quantity the user has no
/// reason to reason about and every chance of mistyping. A step also survives a change of the formula
/// underneath it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum GeomQuality {
    /// Fast and coarse: rough estimates, heavy assemblies.
    Draft,
    /// Normal: the behaviour the application had before this setting existed (K = 0.0015).
    #[default]
    Normal,
    /// Fine: small parts, printing, export to a customer.
    Fine,
}

impl GeomQuality {
    /// Fraction of the bounding-box diagonal used as the tessellation deflection.
    ///
    /// `Normal` has to reproduce the previous behaviour byte for byte: introducing the setting must not
    /// silently change how already-drawn projects look.
    pub fn deflection_k(self) -> f64 {
        match self {
            GeomQuality::Draft => 0.006,
            GeomQuality::Normal => 0.0015,
            GeomQuality::Fine => 0.0004,
        }
    }

    /// Catalogue key for the label; the core holds no words of its own.
    pub fn label_key(self) -> &'static str {
        match self {
            GeomQuality::Draft => "quality-draft",
            GeomQuality::Normal => "quality-normal",
            GeomQuality::Fine => "quality-fine",
        }
    }

    /// All steps in order, so the properties window does not repeat the list of its own.
    pub fn all() -> [GeomQuality; 3] {
        [GeomQuality::Draft, GeomQuality::Normal, GeomQuality::Fine]
    }
}

/// Document properties: title, author, version, comment.
///
/// Every field is free text, deliberately: one person's version is `1.2` and another's is `rev. B`, and
/// an imposed format would only be worked around in the comment field. The application does not
/// interpret these fields, it stores and displays them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocMeta {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub comment: String,
    /// When the document was created (ISO-8601, UTC). Empty means it has never been saved.
    ///
    /// Filled once, on the first save, and never touched again: when a document was started is a fact,
    /// not a property of the latest write.
    #[serde(default)]
    pub created: String,
    /// WHICH BUILD WROTE THIS FILE. Filled by the program on every save, never typed by a person - the
    /// field above named `version` is theirs ("rev. B"), this one is ours.
    ///
    /// The reason is a report: the format changes with no backward compatibility, so "it does not open"
    /// is answered by knowing what wrote it. Without this the answer costs a conversation, and the
    /// person rarely remembers.
    #[serde(default)]
    pub saved_by: String,
}

impl DocMeta {
    /// Whether anything at all is filled in; decides whether the summary is shown.
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.author.is_empty() && self.version.is_empty() && self.comment.is_empty()
    }
}

/// WHERE AN ELEMENT STOOD WHEN A REFERENCE WAS MADE TO IT - the witness that repairs the reference when
/// the numbering shifts under it.
///
/// It used to be the triple `(u32, [f64; 3], [f64; 3])`, and there was no way to tell the second array from
/// the third without going to find the doc comment on the field: for an edge they are the midpoint and the
/// direction, for a face the centre and the normal. Three named fields say it at the point of use.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ElemSnapshot {
    /// The element's id as it was stored in the reference.
    pub id: u32,
    /// WHERE: an edge's midpoint, a face's centre - in the local space of the source body.
    pub at: [f64; 3],
    /// WHICH WAY: an edge's direction, a face's normal - in the same space.
    pub dir: [f64; 3],
}

/// Root of the project (the document).
///
/// Bodies are one record per body (`bodies: Vec<Body>`): geometry, faces, name and visibility together.
/// Contours still live as a pair of arrays, `contours` and `contour_ids` — algorithms address them by
/// index while references use the stable `Id`. Geometry is mutated through the `add_*`, `set_*` and
/// `remove_*` methods.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Project {
    pub units: Units,
    /// Document properties: author, title, version, comment. They travel with the file because they
    /// describe the document, not the application: a colleague who receives it sees whose it is.
    #[serde(default)]
    pub meta: DocMeta,
    /// Geometry accuracy of this document (see [`GeomQuality`]). A document property: one file has to
    /// produce the same STL for two different people.
    #[serde(default)]
    pub geom_quality: GeomQuality,
    /// Stable id generator (the last id handed out).
    #[serde(default)]
    pub next_id: Id,
    /// Geometry name table: descriptor to structural name (which feature produced a face, in what role,
    /// from which entity). Stored in the document, because a descriptor held by a reference has to mean
    /// the same thing on the next open. See `names::NameTable`.
    #[serde(default)]
    pub names: crate::names::NameTable,
    /// The last assembly solve did not converge (conflicting or incompatible mates); drives the warning
    /// in the interface. Not serialised, since it is recomputed on every regenerate.
    #[serde(skip)]
    pub mates_conflict: bool,
    /// Pointer drag: the component, a point on it (in its local space) and where that point is led (in
    /// world space).
    ///
    /// Lives only for the duration of the drag and is never written to the file. It sits on the project
    /// rather than being passed to a separate call so that the goal lands in the same problem as the
    /// mates: everything is solved at once and the mechanism moves as a chain. Solving in two steps,
    /// mates first and drag second, makes a nested mate lag by one frame.
    #[serde(skip)]
    pub drag_pull: Option<(Id, [f64; 3], [f64; 3])>,
    /// Which mates are in conflict, by id, rather than a single flag saying a conflict exists somewhere.
    ///
    /// A derived field, not written to the file (like `mates_conflict`); the solve fills it in. It exists
    /// so the panel can tell the truth about each individual mate: without it both conflicting mates read
    /// "0 degrees of freedom, fully constrained" in green next to a red conflict warning.
    pub mates_violated: Vec<Id>,
    /// Contours of the document together with their ids, provenance and nesting — one consistent
    /// structure instead of four parallel lists (the reasoning is in `model::contours`).
    #[serde(default)]
    pub contours: contours::Contours,
    #[serde(default)]
    pub bodies: Vec<Body>,
    /// Bodies added by import (STL/STEP). They have no timeline node yet are valid rather than orphaned,
    /// so pruning can tell an import from a mesh whose node was deleted. Persisted.
    #[serde(default)]
    pub imported_bodies: std::collections::HashSet<Id>,

    /// Manually assigned part colours (RGB), keyed by the lineage root of a body: the colour belongs to
    /// the part rather than to a mesh index, so it stays stable across operations. No entry means the
    /// default palette (`default_part_color`).
    #[serde(default)]
    pub part_colors: std::collections::HashMap<Id, [u8; 3]>,
    /// Colours a file gives single faces of a part (RGB), by the persistent id of the face, keyed like `part_colors`
    /// by the lineage root of a body. A face with no entry takes the part's colour.
    #[serde(default)]
    pub face_colors: std::collections::HashMap<Id, Vec<(u32, [u8; 3])>>,
    /// The colours of the triangles of a piece of a mesh whose file colours faces one by one, keyed like `part_colors`:
    /// the distinct colours, and for every triangle the place of its colour among them.
    #[serde(default)]
    pub tri_colors: std::collections::HashMap<Id, (Vec<[u8; 3]>, Vec<u8>)>,
    /// Construction tree: user-defined work planes.
    #[serde(default)]
    pub planes: Vec<WorkPlane>,
    /// Sketches: a grouping layer above contours (one DXF/SVG import is one sketch). The contours
    /// themselves live flat in `contours`; a sketch holds their ids.
    #[serde(default)]
    pub sketches: Vec<Sketch>,
    /// Feature timeline: one ordered list of nodes — sketches, datum planes, part features. References use
    /// stable ids. This is the canonical source of build order and of model truth; bodies are rebuilt from
    /// it by `regenerate`.
    #[serde(default)]
    pub timeline: Vec<crate::feature::FeatureNode>,
    /// Components and parts: containers of the build tree that hold sketches and features.
    #[serde(default)]
    pub components: Vec<crate::feature::Component>,
    /// Root component of the document (an assembly with `parent == None`). Every node and component sits
    /// inside it or inside its descendants. Zero means it has not been created yet; `ensure_root` assigns
    /// the id.
    #[serde(default)]
    pub root: Id,
    /// Active context: new timeline nodes land in it. `None` is normalised to `root` (see `active_ctx`).
    #[serde(default)]
    pub active_component: Option<Id>,
    /// Datum points (the `DatumPoint` timeline node references one by id).
    #[serde(default)]
    pub datum_points: Vec<DatumPoint>,
    /// Datum axes (the `DatumAxis` timeline node references one by id).
    #[serde(default)]
    pub datum_axes: Vec<DatumAxis>,
    /// Mate connectors: coordinate frames placed on components.
    #[serde(default)]
    pub connectors: Vec<crate::feature::MateConnector>,
    /// Joints between components; they drive placement in the assembly pass of `regenerate`.
    #[serde(default)]
    pub joints: Vec<crate::feature::Joint>,
    /// Bodies that existed and were deleted.
    ///
    /// The document has to remember these by id, or "the body was deleted" is indistinguishable from
    /// "the body is not built yet". The difference matters: unbuilt geometry is a temporary state (the
    /// document was opened from a bundle and the live B-rep is raised on demand), and a mate on it has to
    /// come back to life by itself, whereas a deleted body never returns and a mate on it has to go red.
    ///
    /// This list is what lets mates survive the disappearance of geometry.
    #[serde(default)]
    pub dead_bodies: Vec<Id>,
    /// Groups: sets of components rigidly fixed relative to each other.
    ///
    /// Not a joint kind but a constraint — a group has no connectors at all. Anything that does not move
    /// relative to its neighbours is better collected into one group than joined by a rigid mate between
    /// every pair. On an imported assembly of dozens of parts that is the difference between one action
    /// and dozens.
    #[serde(default)]
    pub mate_constraints: Vec<crate::feature::MateConstraint>,
    /// Relations between mates: gear, rack and pinion, screw, linear. Neither a joint nor a constraint —
    /// they tie two already existing degrees of freedom together by a constant factor, which is why they
    /// live in a list of their own.
    #[serde(default)]
    pub relations: Vec<crate::feature::MateRelation>,
    /// External references: controlled top-down design. A cross-component reference is allowed only when
    /// it is declared as an explicit `ExternalRef`; otherwise component isolation blocks it. Source
    /// geometry is resolved into the consumer's local space through `world_transform`. Declaring it makes
    /// the dependency explicit, enumerable and breakable.
    #[serde(default)]
    pub external_refs: Vec<crate::feature::ExternalRef>,
    /// Named driving dimensions: a dimension of a skeleton sketch exposed as a global parameter.
    #[serde(default)]
    pub named_dims: Vec<NamedDim>,
    /// Embedded import originals (dxf/svg/stl), kept for re-import and comparison. The bytes live in the
    /// bundle separately (`sources/<id>.<ext>`), not in `document.ron`.
    #[serde(default)]
    pub sources: Vec<SourceFile>,
    /// Named project parameters (`w = 50`, `d = w/2`): parametric dimensions. Dimensional constraints can
    /// reference them from an expression.
    #[serde(default)]
    pub parameters: Vec<Param>,
    /// Parametric dimension expressions by entity id: `id -> {dimension name -> expression}` (`height`,
    /// `down`, `angle`, `radius` and so on; for mates, `angle`, `offset` and `offset2` by joint id). When
    /// an expression is present and evaluates, it overrides the stored number during regenerate, keeping
    /// the value associative to `parameters`. Absent means the stored number is used. Ids are unique
    /// (`alloc_id`), so entities cannot collide here.
    #[serde(default)]
    pub feat_dims: std::collections::HashMap<Id, std::collections::HashMap<String, String>>,
    /// Geometric snapshots of the edges a feature selected (fillet, chamfer): `feature id -> [(edge id,
    /// midpoint, direction)]` in the local space of the source body, taken at creation time. If the stored
    /// edge id no longer resolves on rebuild (the topology above it in the timeline changed), the
    /// reference is repaired from the snapshot by looking for the current edge with the nearest midpoint
    /// and direction. Deliberately conservative: only an unambiguous match repairs, otherwise the
    /// reference stays broken.
    #[serde(default)]
    pub edge_refs: std::collections::HashMap<Id, Vec<ElemSnapshot>>,
    /// Face snapshots (centre and normal in the local space of the source body): the same idea as
    /// `edge_refs`, but for references to faces.
    ///
    /// Without them a reference stored in a file misses as soon as a face gains a name instead of a
    /// positional number, and the feature that held it goes red with "face not found". A positional
    /// number is not an address, and names appear exactly when naming is improved, so a reference needs a
    /// witness that does not depend on numbering.
    #[serde(default)]
    pub face_refs: std::collections::HashMap<Id, Vec<ElemSnapshot>>,
    /// How many times the geometric fallback had to repair a reference.
    ///
    /// The fallback looks an element up by a snapshot of its place, that is, by similarity. It is needed
    /// and it stays, but every time it fires it means the by-name link broke. Without counting them there
    /// is no way to claim naming works: a "36 of 36 by name" figure describes one feature and says
    /// nothing about the document as a whole.
    ///
    /// The counter lives in memory only (never written to the file) and grows only in the geometric
    /// branches of resolution. It is atomic and shared between clones because regeneration runs on a
    /// background thread, and a counter that did not survive cloning would lie exactly where the work
    /// happens.
    #[serde(skip)]
    pub snap_rebinds: std::sync::Arc<std::sync::atomic::AtomicU32>,
    /// Faces of bodies from the B-rep of the last rebuild (`body id -> faces`). Derived from the kernel
    /// and not saved (regenerate restores it). Needed to resolve face references by persistent id, so that
    /// a sketch on a face keeps holding the same face across rebuilds.
    #[serde(skip)]
    pub regen_faces: std::collections::HashMap<Id, Vec<crate::geom::MeshFace>>,
    /// Edges of bodies from the B-rep of the last rebuild (`body id -> edges`). Derived from the kernel
    /// (`Kernel::edges`) and not saved. Used to resolve axis connectors by persistent edge id
    /// (`AnchorRef::EdgeMid`).
    #[serde(skip)]
    pub regen_edges: std::collections::HashMap<Id, Vec<crate::geom::MeshEdge>>,
    /// Nodes whose feature failed to build in the last regenerate (`node id -> error`). A modifier may
    /// have fallen back to pass-through (output is a copy of the input), so the body is valid while the
    /// feature was in fact not applied; the tree marks such nodes. Derived from regenerate, not saved, and
    /// cleared on success or deletion.
    #[serde(skip)]
    pub regen_errors: std::collections::HashMap<Id, crate::errors::CoreError>,
    /// Nodes that built, but not all of what was asked (`node id -> what was left out`): a rounding that could not
    /// take some of its edges and did the rest. Shown yellow with the words, not red. Derived, not saved.
    #[serde(skip)]
    pub regen_warnings: std::collections::HashMap<Id, crate::errors::CoreError>,
    /// Rollback bar: build only the first N timeline nodes; nodes at index N and beyond are suppressed and
    /// their bodies are neither built nor shown. `None` builds everything. Saved with the project.
    #[serde(default)]
    pub rollback: Option<usize>,
    /// THE REBUILDS OF SKETCHES HELD BACK while a tool lays many elements (`Project::batch`). Not a fact of the
    /// document.
    #[serde(skip)]
    pub held_rebuilds: HeldRebuilds,
}

/// The sketches whose rebuild is held back, and how deep the batches go.
#[derive(Clone, Debug, Default)]
pub struct HeldRebuilds {
    depth: u32,
    sketches: std::collections::BTreeSet<usize>,
    /// the sketches whose solve is held back too (`Project::solve_sketch_held`)
    solves: std::collections::BTreeSet<usize>,
}

/// Definition of a datum point. `at` is derived whenever the definition is not `Manual`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[derive(Default)]
pub enum PointDef {
    /// Coordinates given by hand (`at` = x/y/z, parametric through `feat_dim`).
    #[default]
    Manual,
    /// At a vertex of a body (an endpoint of a persistent edge). Associative: the point travels with the
    /// vertex when the source is rebuilt. `end = false` is the start of the edge, `true` is the end.
    AtVertex { body: Id, edge: u32, end: bool },
}

/// A datum point: a named point in 3D. Planes, sketches and joints reference one by id.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatumPoint {
    #[serde(default)]
    pub id: Id,
    pub name: String,
    pub at: [f64; 3],
    /// Parametric definition: when it is not `Manual`, `at` is resolved during regenerate.
    #[serde(default)]
    pub def: PointDef,
}

impl Default for DatumPoint {
    fn default() -> Self {
        Self { id: 0, name: "name-datum-point".into(), at: [0.0; 3], def: PointDef::Manual }
    }
}

/// Definition of a datum axis. `origin` and `dir` are derived whenever the definition is not `Manual`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AxisDef {
    /// Given by hand: the coordinates are the definition.
    Manual { origin: [f64; 3], dir: [f64; 3] },
    /// Through two datum points `a` and `b`: origin = a, dir = norm(b - a), parametric in their coordinates.
    TwoPoints { a: Id, b: Id },
    /// Along a straight edge of a body (persistent edge id). Associative: the axis travels with the edge.
    FromEdge { body: Id, edge: u32 },
    /// Along the axis of a cylindrical or conical face of a body (persistent face id). Associative.
    FromFace { body: Id, face: u32 },
}

impl Default for AxisDef {
    fn default() -> Self {
        AxisDef::Manual { origin: [0.0; 3], dir: [0.0, 0.0, 1.0] }
    }
}

/// A datum axis: a point and a direction (normalised at the point of use).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatumAxis {
    #[serde(default)]
    pub id: Id,
    pub name: String,
    /// Coordinates are derived and are never written to the file.
    ///
    /// Storing `origin` and `dir` next to the definition and saving both means that for a non-`Manual`
    /// definition the fields hold a stale truth whenever regenerate has not reached them yet, with nothing
    /// to tell stale from current. The single source of truth is `def`; the coordinates live in memory
    /// only (`#[serde(skip)]`) and are filled by resolution. `Manual` keeps its coordinates inside the
    /// definition, because there they are the definition.
    #[serde(skip)]
    pub(crate) origin_cache: [f64; 3],
    #[serde(skip)]
    pub(crate) dir_cache: [f64; 3],
    #[serde(default)]
    pub def: AxisDef,
}

impl DatumAxis {
    /// A manual axis, where the coordinates are the definition. This is the only way to build an axis with
    /// specific coordinates from outside the core: the resolution cache is private on purpose.
    pub fn manual(name: impl Into<String>, origin: [f64; 3], dir: [f64; 3]) -> Self {
        Self { id: 0, name: name.into(), origin_cache: origin, dir_cache: dir, def: AxisDef::Manual { origin, dir } }
    }

    /// A parametric axis from a definition (two points, an edge, a face). The coordinates appear after
    /// resolution during regenerate; before that they honestly do not exist, rather than existing in a
    /// stale form.
    pub fn from_def(name: impl Into<String>, def: AxisDef) -> Self {
        Self { id: 0, name: name.into(), origin_cache: [0.0; 3], dir_cache: [0.0, 0.0, 1.0], def }
    }

    /// Origin of the axis: from the definition for `Manual`, otherwise from the resolution cache.
    pub fn origin(&self) -> [f64; 3] {
        match self.def {
            AxisDef::Manual { origin, .. } => origin,
            _ => self.origin_cache,
        }
    }

    /// Direction of the axis (unit): from the definition for `Manual`, otherwise from the resolution cache.
    pub fn dir(&self) -> [f64; 3] {
        match self.def {
            AxisDef::Manual { dir, .. } => dir,
            _ => self.dir_cache,
        }
    }

    /// Set coordinates by hand. This changes the definition to `Manual`: keeping the coordinates next to a
    /// parametric definition would create a second source of truth.
    pub fn set_manual(&mut self, origin: [f64; 3], dir: [f64; 3]) {
        self.def = AxisDef::Manual { origin, dir };
    }

    /// Store the result of resolving the definition (for non-`Manual` axes only). Test-facing wrapper.
    pub fn set_resolved_for_test(&mut self, origin: [f64; 3], dir: [f64; 3]) {
        self.set_resolved(origin, dir);
    }

    pub(crate) fn set_resolved(&mut self, origin: [f64; 3], dir: [f64; 3]) {
        self.origin_cache = origin;
        self.dir_cache = dir;
    }
}

impl Default for DatumAxis {
    fn default() -> Self {
        Self { id: 0, name: "name-datum-axis".into(), origin_cache: [0.0; 3], dir_cache: [0.0, 0.0, 1.0], def: AxisDef::default() }
    }
}

/// A named model parameter: `name = expr` evaluated into `value`. An expression may reference other
/// parameters (see `crate::expr`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    /// The expression (`50`, `w/2`, `2*pi*r`). Empty or unparsable leaves `value` untouched.
    pub expr: String,
    /// Last evaluated value (a cache for the solver and the interface).
    pub value: f64,
}

/// What exactly a named driver points at.
///
/// One enum instead of two mechanisms: a sketch dimension is addressed by a set of entities, a feature
/// parameter by a timeline node and a key. Separate fields for the two would mean writing the rules
/// (name already taken, renaming, breadcrumbs, listing) twice, and the two copies would drift apart.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DimTarget {
    /// A sketch dimension, identified by the entities it is attached to (points, a circle, an arc,
    /// depending on the dimension kind). Order does not matter: the comparison is set-based.
    ///
    /// A pair of points would only cover a linear dimension between two points, leaving angle, diameter,
    /// distance to a line, arc length and tangent gap with nowhere in the model to hold a name.
    Sketch { sketch: Id, refs: Vec<Id> },
    /// A feature parameter: a timeline node and a key (`height`, `radius`, `thickness` and so on) — the
    /// same keys `feat_dims` uses to override numbers with expressions.
    ///
    /// Features need names as much as sketches do: any model parameter should be nameable and referable
    /// from any formula.
    Feature { node: Id, key: String },
}

impl Default for DimTarget {
    fn default() -> Self {
        DimTarget::Sketch { sketch: 0, refs: Vec::new() }
    }
}

/// A named driving dimension: the value of a sketch dimension or of a feature parameter is exposed as a
/// global parameter `name`, so a skeleton sketch of an assembly can drive part dimensions through
/// expressions. Bound by stable ids, not by index.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NamedDim {
    pub name: String,
    pub target: DimTarget,
}

/// Embedded original of an imported file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SourceFile {
    pub id: Id,
    /// Original file name, for example `frame.dxf`.
    pub name: String,
    /// Extension, used for the path inside the bundle, for example `dxf`.
    pub ext: String,
    /// Raw file bytes. Not serialised into `document.ron`; they live under `sources/`.
    #[serde(skip)]
    pub data: Vec<u8>,
}

/// Definition of a datum plane. The `origin` and `normal` of the plane are derived: regenerate resolves
/// them from this definition. `Manual` means they were given by hand.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[derive(Default)]
pub enum PlaneDef {
    /// Given by hand as coordinates (`origin`, `normal` and `rot` are edited directly).
    #[default]
    Manual,
    /// Offset from a base plane (XY, XZ, YZ) by `dist` along its normal (parametric through the `dist`
    /// feature dimension).
    OffsetBase { base: crate::feature::BasePlane, dist: f64 },
    /// Offset from a face of body `body` (persistent `face` id) by `dist` along the face normal.
    OffsetFace { body: Id, face: crate::feature::FaceKey, dist: f64 },
    /// Offset from another datum plane `plane` by `dist` along its normal (parametric through the `dist`
    /// feature dimension).
    OffsetPlane { plane: Id, dist: f64 },
    /// Where a face stood whose body was deleted: fixed like `Manual`, and a sketch standing on it is red with the
    /// reason until it is put on another face or plane.
    FaceGone,
    /// Where a datum plane stood that was deleted: the same, with its own reason.
    PlaneGone,
}

/// A work plane (a construction element): an origin, a normal and a rotation about that normal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkPlane {
    /// Stable id; the timeline node references the plane by it. Zero means it has not been assigned yet
    /// (`rebuild_timeline` hands one out).
    #[serde(default)]
    pub id: Id,
    pub name: String,
    pub origin: [f64; 3],
    pub normal: [f64; 3],
    /// Rotation of the plane's X axis about its normal, in degrees.
    pub rot_deg: f64,
    /// The tilt of a plane defined by an offset, applied to the normal its definition gives (`Manual` ignores it).
    #[serde(default)]
    pub tilt: crate::feature::PlaneTilt,
    /// Parametric definition: when it is not `Manual`, `origin` and `normal` are resolved during
    /// regenerate.
    #[serde(default)]
    pub def: PlaneDef,
}

impl Default for WorkPlane {
    fn default() -> Self {
        Self { id: 0, name: "name-plane".into(), origin: [0.0; 3], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, tilt: Default::default(), def: PlaneDef::Manual }
    }
}

impl WorkPlane {
    /// Build a plane from a recognised mesh face.
    pub fn from_face(name: impl Into<String>, face: &crate::geom::MeshFace) -> Self {
        Self { id: 0, name: name.into(), origin: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, rot_deg: 0.0, tilt: Default::default(), def: PlaneDef::Manual }
    }
}

/// Edge-name translation map for a single rebuild pass: body to "old number to new name".
///
/// Edge names are inherited down the timeline: a resulting body carries the edges of its inputs under the
/// same names. The map of a body therefore travels into its descendants, so any reference saved under the
/// previous scheme (a chamfer, a fillet) is translated exactly, through a known mapping, instead of being
/// looked up as a similar edge.
pub(crate) type EdgeRenames = std::collections::HashMap<Id, std::collections::HashMap<u32, u32>>;

mod assembly;
pub use assembly::{ExportMesh, ExportNode, FileGroup, ImportNode};
pub mod contours;
mod regen;
pub use regen::{ArrayAxis, BodyOp, ChamferShape, CombineSpan, ExtrudeSpan, HoleTool, RevolveAxis, RevolveTurn};
mod tess;
mod timeline;
mod sketch;
pub use sketch::{LoftEncoded, LoftSectionsEncoded};
mod drag;
mod round;
pub use sketch::{ChamferLegs, CornerAt, CornerBlend, CornerCut, CornerTool, ExtendAsk, ExtendSides, ExtendStop, ExtendStops, EndKept, FilletBy, FilletSize, LineExtension, LinesCrossing, TextSpec};
pub(crate) mod comp_pattern;
pub use comp_pattern::{CompPattern, CompPatternKind};
mod projection;
mod hash_ser;

impl Project {}

/// Tessellate a sketch into several contours: each circle becomes its own contour, while lines and arcs are
/// collected into loops through their shared points. Construction entities are skipped.
fn remap_point_id(s: &mut Sketch, from: Id, to: Id) {
    let r = |id: &mut Id| {
        if *id == from {
            *id = to;
        }
    };
    for e in &mut s.entities {
        match &mut e.kind {
            EntityKind::Line { a, b } => {
                r(a);
                r(b);
            }
            EntityKind::Arc { center, a, b, .. } => {
                r(center);
                r(a);
                r(b);
            }
            EntityKind::Circle { center, .. } => r(center),
            EntityKind::Ellipse { c, ma, mi } => {
                r(c);
                r(ma);
                r(mi);
            }
        }
    }
    for c in &mut s.constraints {
        match c {
            Constraint::Fixed { p } => r(p),
            Constraint::Horizontal { a, b } | Constraint::Vertical { a, b } | Constraint::Orientation { a, b, .. } | Constraint::Coincident { a, b } | Constraint::Distance { a, b, .. } => {
                r(a);
                r(b);
            }
            Constraint::Parallel { a, b, c, d } | Constraint::Perpendicular { a, b, c, d } | Constraint::Equal { a, b, c, d } | Constraint::Collinear { a, b, c, d } => {
                r(a);
                r(b);
                r(c);
                r(d);
            }
            Constraint::Angle { a, b, c, .. } => {
                r(a);
                r(b);
                r(c);
            }
            Constraint::Midpoint { p, a, b } => {
                r(p);
                r(a);
                r(b);
            }
            Constraint::Tangent { a, b, c, .. } => {
                r(a);
                r(b);
                r(c);
            }
            Constraint::Symmetric { a, b, la, lb } => {
                r(a);
                r(b);
                r(la);
                r(lb);
            }
            Constraint::PointOnLine { p, a, b } => {
                r(p);
                r(a);
                r(b);
            }
            Constraint::DistancePL { p, a, b, .. } => {
                r(p);
                r(a);
                r(b);
            }
            Constraint::EdgeDistance { c1, c2, .. } => {
                r(c1);
                r(c2);
            }
            Constraint::Diameter { c, .. } => r(c),
            Constraint::EqualRadius { c1, c2 } => {
                r(c1);
                r(c2);
            }
            Constraint::CircleTangent { c1, c2, .. } => {
                r(c1);
                r(c2);
            }
            Constraint::PointOnCircle { p, c } => {
                r(p);
                r(c);
            }
            Constraint::Concentric { c1, c2 } => {
                r(c1);
                r(c2);
            }
            Constraint::ArcLength { c, a, b, .. } => {
                r(c);
                r(a);
                r(b);
            }
            Constraint::AngleLines { a, b, c, d, .. } => {
                r(a);
                r(b);
                r(c);
                r(d);
            }
        }
    }
    for sp in &mut s.splines {
        for id in &mut sp.points {
            r(id);
        }
    }
}

/// A VERTEX OF A BODY: where it stands and the name the bridge gave it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexSpot {
    pub at: [f64; 3],
    pub name: crate::names::VertexName,
}

/// Serialise a project into a RON string.
/// Inheritance of the edge translation map: the body of a node carries the edges of its inputs under their
/// names, so the "old number to new name" mapping applies to it as well.
///
/// A collision means refusing to translate. Positional numbers are counted within a body, so "edge 5" of the
/// base and "edge 5" of the tool are different edges. When the inputs give one number two different names,
/// there is no honest translation: the key is dropped entirely and the reference stays as it was rather than
/// being translated at random. Silently picking one of the two put a fillet on the wrong edge — an R10
/// landed on an edge that only allows 1 mm.
fn inherit_edge_renames(emap: &mut EdgeRenames, out: Id, inputs: &[Id]) {
    let mut inherited: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut clash: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for inp in inputs {
        let Some(m) = emap.get(inp) else { continue };
        for (k, v) in m {
            match inherited.get(k) {
                Some(prev) if prev != v => {
                    clash.insert(*k);
                }
                _ => {
                    inherited.insert(*k, *v);
                }
            }
        }
    }
    for k in &clash {
        inherited.remove(k);
    }
    if !inherited.is_empty() {
        let e = emap.entry(out).or_default();
        for (k, v) in inherited {
            e.insert(k, v);
        }
    }
}

/// WHO IS WRITING THE FILE, as one line: `QymCAD 0.1.0 (a8f629971, 2026-08-25)`.
///
/// The core is a library and has no build of its own to name; the application sets this once at startup
/// and everything that saves picks it up. Passing it down as an argument instead would thread a
/// parameter through every writing path for a value that never changes within a run.
static PRODUCER: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Called once by the application at startup. A second call is ignored rather than refused: the value
/// is the same for the whole run, and a save must never fail over a diagnostic string.
pub fn set_producer(line: &str) {
    let _ = PRODUCER.set(line.to_string());
}

/// The producer line, empty when nobody set it (a headless test, another program using the library).
pub fn producer() -> String {
    PRODUCER.get().cloned().unwrap_or_default()
}

pub fn to_ron(project: &Project) -> Result<String, String> {
    // Through the file schema rather than directly: the on-disk format no longer mirrors the layout of the
    // model.
    let doc = crate::doc_file::DocumentFile::from_model(project);
    ron::ser::to_string_pretty(&doc, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())
}

/// Load a project from a RON string.
pub fn from_ron(s: &str) -> Result<Project, String> {
    // The recursion limit is lifted, or a large selection makes the file unreadable.
    //
    // A set of selected edges is written as a ladder of `Union(Union(...))`: 52 edges give 51 levels of
    // nesting and the reader hits its own limit. The file still saves and then never opens again — selecting
    // 52 edges for a fillet and saving loses access to the project. Writing and reading have to agree, so
    // the limit is lifted here while the ladder is flattened separately.
    let opts = ron::Options::default().without_recursion_limit();
    let doc: crate::doc_file::DocumentFile = opts.from_str(s).map_err(|e| e.to_string())?;
    let mut p = doc.into_model();
    p.ensure_ids();
    p.names.rebuild_index(); // The reverse index is derived (`serde(skip)`); without it names get duplicated.
    Ok(p)
}

#[cfg(test)]
mod rot_axis_tests {
    use super::tess::rot_about_axis;
    use crate::feature::apply12;
    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9)
    }
    #[test]
    fn rot_z_90_maps_x_to_y() {
        let m = rot_about_axis([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 90.0);
        assert!(close(apply12(&m, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
    }
    #[test]
    fn rot_x_90_maps_y_to_z() {
        let m = rot_about_axis([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 90.0);
        assert!(close(apply12(&m, [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]));
    }
    #[test]
    fn rot_about_axis_through_point_fixes_point() {
        let p = [5.0, 3.0, 2.0];
        let m = rot_about_axis(p, [0.0, 0.0, 1.0], 37.0);
        assert!(close(apply12(&m, p), p), "a point on the axis must not move");
    }
    #[test]
    fn rot_diagonal_120_cycles_axes() {
        // 120 degrees about (1,1,1): X to Y to Z to X.
        let m = rot_about_axis([0.0, 0.0, 0.0], [1.0, 1.0, 1.0], 120.0);
        assert!(close(apply12(&m, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]), "X must map to Y about the diagonal");
    }
    #[test]
    fn degenerate_axis_falls_back_to_z() {
        let m = rot_about_axis([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 90.0);
        assert!(close(apply12(&m, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]), "a degenerate axis must fall back to Z");
    }
}

#[cfg(test)]
mod ellipse_intersect_tests {
    use super::{circle_ellipse_pts, ellipse_axes, line_ellipse_roots, seg_ellipse_t};

    // Ellipse: centre (0,0), major semi-axis 4 along +X, minor semi-axis 2 along +Y.
    fn axes() -> (f64, f64, f64, f64) {
        ellipse_axes(0.0, 0.0, 4.0, 0.0, 0.0, 2.0)
    }

    #[test]
    fn horizontal_line_through_center_hits_major_vertices() {
        let (ux, uy, ma, mi) = axes();
        // The line y = 0, x = t*10 - 5, where t in 0..1 covers x in [-5,5].
        let mut ts = seg_ellipse_t([-5.0, 0.0], [5.0, 0.0], [0.0, 0.0], [ux, uy], ma, mi);
        ts.sort_by(|a, b| a.total_cmp(b));
        assert_eq!(ts.len(), 2, "two intersection points");
        let xs: Vec<f64> = ts.iter().map(|t| -5.0 + t * 10.0).collect();
        assert!((xs[0] + 4.0).abs() < 1e-6 && (xs[1] - 4.0).abs() < 1e-6, "vertices at plus and minus a: {xs:?}");
    }

    #[test]
    fn vertical_line_through_center_hits_minor_vertices() {
        let (ux, uy, ma, mi) = axes();
        let roots = line_ellipse_roots([0.0, -5.0], [0.0, 5.0], [0.0, 0.0], [ux, uy], ma, mi);
        let mut ys: Vec<f64> = roots.iter().map(|t| -5.0 + t * 10.0).collect();
        ys.sort_by(|a, b| a.total_cmp(b));
        assert_eq!(ys.len(), 2);
        assert!((ys[0] + 2.0).abs() < 1e-6 && (ys[1] - 2.0).abs() < 1e-6, "vertices at plus and minus b: {ys:?}");
    }

    #[test]
    fn tangent_and_miss() {
        let (ux, uy, ma, mi) = axes();
        // The tangent y = 2 gives a double root (the discriminant is near zero); y = 3 misses entirely.
        let far = seg_ellipse_t([-5.0, 3.0], [5.0, 3.0], [0.0, 0.0], [ux, uy], ma, mi);
        assert!(far.is_empty(), "y = 3 must not intersect the ellipse");
    }

    #[test]
    fn rotated_ellipse_major_axis_45deg() {
        // Major axis along (1,1) with its endpoint at (3,3), so major = sqrt(18); the minor axis is perpendicular, length 1.
        let (ux, uy, ma, mi) = ellipse_axes(0.0, 0.0, 3.0, 3.0, -0.5_f64.sqrt(), 0.5_f64.sqrt());
        // The major axis itself, the line from (-4,-4) to (4,4), meets the curve at the vertices at plus and minus (3,3).
        let mut roots = line_ellipse_roots([-4.0, -4.0], [4.0, 4.0], [0.0, 0.0], [ux, uy], ma, mi);
        roots.sort_by(|a, b| a.total_cmp(b));
        assert_eq!(roots.len(), 2, "two vertices: {roots:?}");
        assert!((roots[0] - 0.125).abs() < 1e-6 && (roots[1] - 0.875).abs() < 1e-6, "vertices at plus and minus (3,3): {roots:?}");
    }

    #[test]
    fn circle_ellipse_four_points() {
        let (ux, uy, ma, mi) = axes(); // a=4,b=2
                                       // A circle of radius 3 at the centre: |E(t)| = 3 when 16*cos^2 + 4*sin^2 = 9, that is 12*cos^2 = 5, giving four symmetric points.
        let pts = circle_ellipse_pts([0.0, 0.0], 3.0, [0.0, 0.0], [ux, uy], ma, mi);
        assert_eq!(pts.len(), 4, "a circle of r = 3 must meet the ellipse four times: {pts:?}");
        for (x, y) in &pts {
            assert!(((x * x + y * y).sqrt() - 3.0).abs() < 1e-4, "the point must lie on the circle r = 3");
            assert!((x * x / 16.0 + y * y / 4.0 - 1.0).abs() < 1e-4, "the point must lie on the ellipse");
        }
    }

    #[test]
    fn circle_ellipse_no_intersection() {
        let (ux, uy, ma, mi) = axes();
        // A circle of r = 1 lies entirely inside (the minor semi-axis is 2), so there are no intersections.
        assert!(circle_ellipse_pts([0.0, 0.0], 1.0, [0.0, 0.0], [ux, uy], ma, mi).is_empty());
    }
}

#[cfg(test)]
mod drive_joint_tests {
    use super::Project;
    use crate::feature::{AnchorRef, JointKind};

    // base (grounded) -j1- mid -j2- tip: the parent edge of each is the joint that places it.
    fn chain() -> (Project, [u64; 5]) {
        let mut p = Project::default();
        let base = p.add_component("base");
        let mid = p.add_component("mid");
        let tip = p.add_component("tip");
        let j1 = {
            let a = p.add_connector(base, AnchorRef::Origin);
            let b = p.add_connector(mid, AnchorRef::Origin);
            p.add_joint(a, b, JointKind::Revolute)
        };
        let j2 = {
            let a = p.add_connector(mid, AnchorRef::Origin);
            let b = p.add_connector(tip, AnchorRef::Origin);
            p.add_joint(a, b, JointKind::Slider)
        };
        p.set_grounded(base, true);
        (p, [base, mid, tip, j1, j2])
    }

    #[test]
    fn grounded_root_has_no_drive_joint() {
        let (p, [base, ..]) = chain();
        assert_eq!(p.drive_joint_for(base), None, "a grounded root is not driven by a joint");
    }

    #[test]
    fn each_component_driven_by_its_tree_parent_edge() {
        let (p, [_, mid, tip, j1, j2]) = chain();
        assert_eq!(p.drive_joint_for(mid), Some(j1), "mid is placed by j1");
        assert_eq!(p.drive_joint_for(tip), Some(j2), "tip is placed by j2, deeper in the tree");
    }

    #[test]
    fn floating_component_without_joints_has_none() {
        let (mut p, _) = chain();
        let free = p.add_component("free");
        assert_eq!(p.drive_joint_for(free), None, "a component with no joints is free (six degrees of freedom), not driven");
    }

    #[test]
    fn no_grounded_seeds_min_id_as_root() {
        // With nothing grounded the seed is the smallest id in the graph (as in `place_tree`), so the root is not driven and the second one is.
        let mut p = Project::default();
        let a = p.add_component("a");
        let b = p.add_component("b");
        let j = {
            let ca = p.add_connector(a, AnchorRef::Origin);
            let cb = p.add_connector(b, AnchorRef::Origin);
            p.add_joint(ca, cb, JointKind::Revolute)
        };
        // `a` was created first, so it has the smaller id and becomes the seed root.
        assert_eq!(p.drive_joint_for(a), None, "the seed root (smallest id) is not driven");
        assert_eq!(p.drive_joint_for(b), Some(j), "the second component is driven by the joint");
    }

    #[test]
    fn joint_frame_at_grounded_origin_is_identity() {
        // Origin connectors plus a grounded base at zero give an identity motion frame.
        let (p, [base, ..]) = chain();
        let f = p.joint_frame(p.joints[0].id, base).expect("the joint frame exists");
        let id = crate::feature::PLACE_IDENTITY;
        assert!((0..12).all(|i| (f[i] - id[i]).abs() < 1e-9), "a frame at zero must be the identity");
    }

    #[test]
    fn solve_joints_clamps_free_slot_to_limits() {
        // A revolute angle above its upper limit and a slider offset below its lower one are clamped to the
        // boundaries. What is clamped is the specified value (`drive`): the reading is a fact about where the
        // body stands, and correcting it by a limit would misreport that.
        let (mut p, [_base, _mid, _tip, j1, j2]) = chain();
        // j1 (revolute, angle slot 0): range 0..90 degrees, specifying 120 must clamp to 90.
        {
            let jj = p.joints.iter_mut().find(|x| x.id == j1).unwrap();
            jj.limit_min[0] = Some(0.0);
            jj.limit_max[0] = Some(90.0);
            jj.drive[0] = Some(120.0);
        }
        // j2 (slider, offset slot 1): minimum 10 mm, specifying 3 mm must clamp to 10.
        {
            let jj = p.joints.iter_mut().find(|x| x.id == j2).unwrap();
            jj.limit_min[1] = Some(10.0);
            jj.drive[1] = Some(3.0);
        }
        p.solve_joints();
        let a = p.joints.iter().find(|x| x.id == j1).unwrap().drive[0].expect("the specified value stays specified");
        let o = p.joints.iter().find(|x| x.id == j2).unwrap().drive[1].expect("the specified value stays specified");
        assert!((a - 90.0).abs() < 1e-9, "the angle must clamp to the maximum of 90 degrees (was {a})");
        assert!((o - 10.0).abs() < 1e-9, "the offset must clamp to the minimum of 10 mm (was {o})");
    }

    #[test]
    fn clamp_slot_ignores_unset_bounds() {
        // With no limits the value passes through unchanged; with one bound set only that bound clamps.
        let (p, _) = chain();
        let j = &p.joints[0];
        assert_eq!(j.clamp_slot(0, 42.0), 42.0, "no limits, no change");
    }

    // root (assembly) - sub (subassembly) - leaf (part): an assembly holds a part.
    fn nested_tree() -> (Project, super::Id, super::Id, super::Id) {
        let mut p = Project::default();
        let root = p.ensure_root();
        p.set_active_component(Some(root));
        let sub = p.add_assembly("sub");
        p.set_active_component(Some(sub));
        let leaf = p.add_part("leaf");
        (p, root, sub, leaf)
    }

    #[test]
    fn ancestor_child_of_lifts_leaf_to_context_child() {
        // root - sub - leaf: the direct child of root containing leaf is sub, not leaf itself.
        let (p, root, sub, leaf) = nested_tree();
        assert_eq!(p.ancestor_child_of(root, leaf), Some(sub), "the leaf is lifted to the subassembly, the child of the context");
        assert_eq!(p.ancestor_child_of(sub, leaf), Some(leaf), "the leaf is already a direct child of sub");
        assert_eq!(p.ancestor_child_of(leaf, root), None, "root is outside the subtree of leaf");
    }

    #[test]
    fn connector_frame_in_owner_cs_when_owner_is_subassembly() {
        // The connector belongs to subassembly `sub` while the anchor geometry lives in a leaf body inside
        // `leaf`. The frame has to come back in the coordinate system of `sub`, multiplied by
        // `relative_transform(leaf, sub)`.
        use crate::feature::{AnchorRef, FeatureKind, FeatureNode};
        let tr = |x: f64| [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let (mut p, _root, sub, leaf) = nested_tree();
        p.set_component_transform(sub, tr(10.0)); // world(sub) = +10 along X
        p.set_component_transform(leaf, tr(5.0)); // world(leaf) = +15 along X, so rel(leaf, sub) = +5
                                                  // Body A belongs to leaf and has one edge whose midpoint is [1,0,0] in leaf local space.
        let body = p.alloc_id();
        let nid = p.alloc_id();
        p.timeline.push(FeatureNode { id: nid, name: "b".into(), kind: FeatureKind::Box3 { dx: 1.0, dy: 1.0, dz: 1.0, body }, parent: Some(leaf), dirty: false, suppressed: false });
        p.regen_edges.insert(body, vec![crate::geom::MeshEdge { id: 7, mid: [1.0, 0.0, 0.0], dir: [1.0, 0.0, 0.0], a: [0.5, 0.0, 0.0], b: [1.5, 0.0, 0.0], ..Default::default() }]);
        assert_eq!(p.body_owner(body), Some(leaf), "the owner of the body is the leaf");
        // The connector is owned by subassembly sub (the placement owner) while its anchor is an edge of the leaf body.
        let cid = p.add_connector(sub, AnchorRef::EdgeMid(body, 7));
        let conn = p.connector(cid).unwrap().clone();
        assert_eq!(p.connector_geom_owner(&conn), Some(leaf), "the geometry resolves in the leaf");
        let fr = p.connector_frame(&conn).unwrap();
        // The midpoint [1,0,0] in leaf space becomes [1+5,0,0] in sub space.
        assert!((fr.origin[0] - 6.0).abs() < 1e-9 && fr.origin[1].abs() < 1e-9 && fr.origin[2].abs() < 1e-9, "frame in sub space: origin={:?}", fr.origin);
    }

    #[test]
    fn global_joint_visible_at_root_not_local_only() {
        // A joint between two parts inside subassembly `sub` has `sub` as its home and is visible only in
        // that context by default. With `global` set it is also visible at the root, for global control,
        // while still belonging to its home.
        use crate::feature::{AnchorRef, JointKind};
        let mut p = Project::default();
        let root = p.ensure_root();
        p.set_active_component(Some(root));
        let sub = p.add_assembly("sub");
        p.set_active_component(Some(sub));
        let a = p.add_part("a");
        let b = p.add_part("b");
        let ca = p.add_connector(a, AnchorRef::Origin);
        let cb = p.add_connector(b, AnchorRef::Origin);
        let jid = p.add_joint(ca, cb, JointKind::Slider);
        assert_eq!(p.joint_home(&p.joints[0]), Some(sub), "the home of the joint is the subassembly");
        // By default: visible in sub, not at the root.
        assert!(p.joint_in_context(&p.joints[0], sub), "visible in its own home");
        assert!(!p.joint_in_context(&p.joints[0], root), "not at the root by default");
        // With `global`: visible both at the root and in its home.
        p.joints.iter_mut().find(|x| x.id == jid).unwrap().global = true;
        assert!(p.joint_in_context(&p.joints[0], root), "global makes it visible at the root");
        assert!(p.joint_in_context(&p.joints[0], sub), "global keeps it visible in its home as well");
    }

    #[test]
    fn rigid_face_mate_no_spurious_flip() {
        // Two opposing faces (+X on A, -X on B) with a rigid mate and B flipped meet face to face without
        // turning the body over. Aligning the full frames gives 180 degrees about X, from an arbitrary
        // in-plane basis; the shortest turn is zero.
        use crate::feature::{apply12, apply12_dir, AnchorRef, FaceKey, FeatureKind, FeatureNode, JointKind};
        let mut p = Project::default();
        let root = p.ensure_root();
        p.set_active_component(Some(root));
        let pa = p.add_part("A");
        let pb = p.add_part("B");
        let body_of = |p: &mut Project, part: super::Id| {
            let b = p.alloc_id();
            let n = p.alloc_id();
            p.timeline.push(FeatureNode { id: n, name: "b".into(), kind: FeatureKind::Box3 { dx: 1.0, dy: 1.0, dz: 1.0, body: b }, parent: Some(part), dirty: false, suppressed: false });
            b
        };
        let ba = body_of(&mut p, pa);
        let bb = body_of(&mut p, pb);
        // B starts shifted by +5 along X without rotation, and its face points at -X, towards the face of A.
        p.set_component_transform(pb, [1.0, 0.0, 0.0, 5.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        let key = |c: [f64; 3], n: [f64; 3]| FaceKey { index: 0, centroid: c, normal: n, id: 0 };
        let ca = p.add_connector(pa, AnchorRef::FaceCenter(ba, key([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])));
        let cb = p.add_connector(pb, AnchorRef::FaceCenter(bb, key([0.0, 0.0, 0.0], [-1.0, 0.0, 0.0])));
        p.connectors.iter_mut().find(|c| c.id == cb).unwrap().flip = true; // Face to face means flipping B.
        p.add_joint(ca, cb, JointKind::Rigid);
        p.set_grounded(pa, true);
        p.solve_joints(); // Placement goes through the single solver.
        let wb = p.world_transform(pb);
        // Not turned over: local +Y stays world +Y (the old formula gave -Y, that is 180 degrees about X).
        let y = apply12_dir(&wb, [0.0, 1.0, 0.0]);
        assert!(y[1] > 0.999 && y[0].abs() < 1e-6 && y[2].abs() < 1e-6, "B must not be turned over: +Y -> {:?}", y);
        // The face of B lands on the plane of the face of A (x = 0), with its normal pointing at -X, towards A.
        let face_c = apply12(&wb, [0.0, 0.0, 0.0]);
        let face_n = apply12_dir(&wb, [-1.0, 0.0, 0.0]);
        assert!(face_c[0].abs() < 1e-9, "the face of B must lie on the plane x = 0: {:?}", face_c);
        assert!((face_n[0] + 1.0).abs() < 1e-9, "the normal of the face of B must point at -X: {:?}", face_n);
    }

    #[test]
    fn rigid_face_mate_coplanar_no_180_flip() {
        // The symmetric case: the face of B points the same way in world space as the face of A (+X for
        // both), and the expected result is coplanarity through the smallest movement. Forcing face to face
        // (`flip = true`) used to turn the body by 180 degrees; taking the nearest normal sign instead makes
        // the faces coplanar without turning the body.
        use crate::feature::{apply12, apply12_dir, AnchorRef, FaceKey, FeatureKind, FeatureNode, JointKind};
        let mut p = Project::default();
        let root = p.ensure_root();
        p.set_active_component(Some(root));
        let pa = p.add_part("A");
        let pb = p.add_part("B");
        let body_of = |p: &mut Project, part: super::Id| {
            let b = p.alloc_id();
            let n = p.alloc_id();
            p.timeline.push(FeatureNode { id: n, name: "b".into(), kind: FeatureKind::Box3 { dx: 1.0, dy: 1.0, dz: 1.0, body: b }, parent: Some(part), dirty: false, suppressed: false });
            b
        };
        let ba = body_of(&mut p, pa);
        let bb = body_of(&mut p, pb);
        // B is shifted by +5 along X without rotation, and its selected face points at +X, as the face of A does.
        p.set_component_transform(pb, [1.0, 0.0, 0.0, 5.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        let key = |c: [f64; 3], n: [f64; 3]| FaceKey { index: 0, centroid: c, normal: n, id: 0 };
        let ca = p.add_connector(pa, AnchorRef::FaceCenter(ba, key([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])));
        let cb = p.add_connector(pb, AnchorRef::FaceCenter(bb, key([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])));
        p.connectors.iter_mut().find(|c| c.id == cb).unwrap().flip = true; // The application forces the flip for face to face.
        p.add_joint(ca, cb, JointKind::Rigid);
        p.set_grounded(pa, true);
        p.solve_joints(); // Placement goes through the single solver.
        let wb = p.world_transform(pb);
        // Not turned over: local +X stays world +X (the old behaviour gave -X, a 180 degree turn).
        let x = apply12_dir(&wb, [1.0, 0.0, 0.0]);
        assert!(x[0] > 0.999, "B must not turn by 180 degrees: +X -> {:?}", x);
        let y = apply12_dir(&wb, [0.0, 1.0, 0.0]);
        assert!(y[1] > 0.999, "B keeps its original orientation: +Y -> {:?}", y);
        // The face of B became coplanar with the plane of the face of A (x = 0).
        let face_c = apply12(&wb, [0.0, 0.0, 0.0]);
        assert!(face_c[0].abs() < 1e-9, "the face of B must be coplanar with the plane x = 0: {:?}", face_c);
    }

    #[test]
    fn rigid_face_mate_flip_toggle_forces_opposite_side() {
        // Toggling the mating side of a face-to-face mate turns the body to the opposite side (face to face
        // through 180 degrees) instead of aligning it coplanar. Same setup as without the flip, but with the
        // flip local +X turns into -X.
        use crate::feature::{apply12_dir, AnchorRef, FaceKey, FeatureKind, FeatureNode, JointKind};
        let mut p = Project::default();
        let root = p.ensure_root();
        p.set_active_component(Some(root));
        let pa = p.add_part("A");
        let pb = p.add_part("B");
        let body_of = |p: &mut Project, part: super::Id| {
            let b = p.alloc_id();
            let n = p.alloc_id();
            p.timeline.push(FeatureNode { id: n, name: "b".into(), kind: FeatureKind::Box3 { dx: 1.0, dy: 1.0, dz: 1.0, body: b }, parent: Some(part), dirty: false, suppressed: false });
            b
        };
        let ba = body_of(&mut p, pa);
        let bb = body_of(&mut p, pb);
        p.set_component_transform(pb, [1.0, 0.0, 0.0, 5.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        let key = |c: [f64; 3], n: [f64; 3]| FaceKey { index: 0, centroid: c, normal: n, id: 0 };
        let ca = p.add_connector(pa, AnchorRef::FaceCenter(ba, key([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])));
        let cb = p.add_connector(pb, AnchorRef::FaceCenter(bb, key([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])));
        let jid = p.add_joint(ca, cb, JointKind::Rigid);
        // Flipping the side goes through the core entry point. Writing the `flip` field directly is not
        // enough: the solver chooses the side and overwrites the same field, so an explicit flip has to
        // request the opposite side and mark it as decided.
        p.flip_joint_side(jid);
        p.set_grounded(pa, true);
        p.solve_joints(); // Placement goes through the single solver.
        let wb = p.world_transform(pb);
        let x = apply12_dir(&wb, [1.0, 0.0, 0.0]);
        assert!(x[0] < -0.999, "a flip must give the opposite side: +X -> {:?}", x);
    }

    #[test]
    fn cylindrical_edge_mate_keeps_axial_position() {
        // Edge to edge makes the axes coaxial but does not snap the midpoints together: the position along
        // the axis is preserved, or a large body would be yanked to the midpoint of someone else's edge.
        use crate::feature::{apply12, apply12_dir, AnchorRef, FeatureKind, FeatureNode, JointKind};
        let mut p = Project::default();
        let root = p.ensure_root();
        p.set_active_component(Some(root));
        let pa = p.add_part("A");
        let pb = p.add_part("B");
        let body_of = |p: &mut Project, part: super::Id| {
            let b = p.alloc_id();
            let n = p.alloc_id();
            p.timeline.push(FeatureNode { id: n, name: "b".into(), kind: FeatureKind::Box3 { dx: 1.0, dy: 1.0, dz: 1.0, body: b }, parent: Some(part), dirty: false, suppressed: false });
            b
        };
        let ba = body_of(&mut p, pa);
        let bb = body_of(&mut p, pb);
        let edge = |mid: [f64; 3]| vec![crate::geom::MeshEdge { id: 1, mid, dir: [0.0, 0.0, 1.0], a: [mid[0], mid[1], mid[2] - 1.0], b: [mid[0], mid[1], mid[2] + 1.0], ..Default::default() }];
        p.regen_edges.insert(ba, edge([0.0, 0.0, 0.0]));
        p.regen_edges.insert(bb, edge([0.0, 0.0, 0.0]));
        // B is shifted by +3 and +4 perpendicular to the axis and by +10 along it.
        p.set_component_transform(pb, [1.0, 0.0, 0.0, 3.0, 0.0, 1.0, 0.0, 4.0, 0.0, 0.0, 1.0, 10.0]);
        let ca = p.add_connector(pa, AnchorRef::EdgeMid(ba, 1));
        let cb = p.add_connector(pb, AnchorRef::EdgeMid(bb, 1));
        // Preserving the position along the axis is a cylindrical mate, not a rigid one.
        //
        // A rigid mate with a special case inside placement that avoided pulling along the axis was a
        // workaround for the fact that an edge anchor sits at the midpoint: edges of different lengths have
        // different midpoints, and a rigid mate honestly pulled the body by the difference. Curing that by
        // weakening the definition of "rigid" makes the mate kind unpredictable. The correct answer: when
        // the position along the axis is not specified, the mate is cylindrical.
        p.add_joint(ca, cb, JointKind::Cylindrical);
        p.set_grounded(pa, true);
        p.solve_joints(); // Placement goes through the single solver.
        let wb = p.world_transform(pb);
        let mid = apply12(&wb, [0.0, 0.0, 0.0]); // World midpoint of the edge of B.
                                                 // A tolerance of 1e-6 mm is picometres: a numeric solver converges to a tolerance, and demanding a
                                                 // bit-exact zero from it is meaningless.
        assert!(
            mid[0].abs() < 1e-6 && mid[1].abs() < 1e-6 && (mid[2] - 10.0).abs() < 1e-6,
            "the axes must be coaxial (the perpendicular offset removed) with the position along the axis kept at z = 10: {:?}",
            mid
        );
        let x = apply12_dir(&wb, [1.0, 0.0, 0.0]);
        assert!((x[0] - 1.0).abs() < 1e-6, "no spurious rotation: +X -> {:?}", x);
    }
}

#[cfg(test)]
mod library_extract_insert_tests {
    //! Extracting a product (`subproject_of`) and inserting one (`graft`): a cross-project clone of a subtree.
    use super::{Id, Project};
    use crate::feature::{ComponentKind, FeatureKind};
    use crate::geom::{Contour, Point2};

    // Source: a root assembly plus an active part holding a square sketch and an extrude, with a feature dimension.
    fn source_with_part() -> (Project, Id, Id) {
        let mut p = Project::default();
        let part = p.new_document(); // Root assembly plus one active part.
        let sq = vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)];
        let sk = p.add_sketch("Section", vec![Contour::closed(sq)], None);
        p.add_sketch_node(sk, "Sketch");
        let body = p.add_extrude(sk, 5.0);
        p.feat_dims.entry(body).or_default().insert("height".into(), "L".into());
        (p, part, body)
    }

    fn timeline_kinds(p: &Project, owner: Id) -> Vec<&'static str> {
        p.timeline
            .iter()
            .filter(|n| n.parent == Some(owner))
            .map(|n| match n.kind {
                FeatureKind::Sketch { .. } => "sketch",
                FeatureKind::Extrude { .. } => "extrude",
                _ => "other",
            })
            .collect()
    }

    #[test]
    fn subproject_of_makes_minimal_project_with_one_part() {
        let (p, part, _) = source_with_part();
        let out = p.subproject_of(part).expect("the extraction succeeded");
        // Exactly one part under the root assembly.
        let parts: Vec<Id> = out.components.iter().filter(|c| c.kind == ComponentKind::Part).map(|c| c.id).collect();
        assert_eq!(parts.len(), 1, "one part in the product");
        assert!(out.components.iter().any(|c| c.id == out.root && c.kind == ComponentKind::Assembly && c.parent.is_none()), "a root assembly must exist");
        assert_eq!(out.components.iter().find(|c| c.id == parts[0]).unwrap().parent, Some(out.root), "the part must sit under the product root");
        // Timeline of the part: sketch plus extrude.
        assert_eq!(timeline_kinds(&out, parts[0]), vec!["sketch", "extrude"], "sketch and extrude must be carried over");
        // The feature dimension is carried over under the new body id.
        let new_body = out.timeline.iter().find_map(|n| n.kind.body()).expect("the body exists");
        assert_eq!(out.feat_dims.get(&new_body).and_then(|m| m.get("height")).map(String::as_str), Some("L"), "the height=L feature dimension must be preserved");
    }

    #[test]
    fn subproject_of_root_is_none() {
        let (p, _, _) = source_with_part();
        assert!(p.subproject_of(p.root).is_none(), "the document root must not be saved as a product");
    }

    #[test]
    fn graft_inserts_first_class_part_with_disjoint_ids() {
        let (src, part, _) = source_with_part();
        let qpart = src.subproject_of(part).expect("extraction");

        let mut host = Project::default();
        host.new_document();
        let before_next = host.next_id;
        let host_ids: std::collections::HashSet<Id> = host.components.iter().map(|c| c.id).chain(host.timeline.iter().map(|n| n.id)).collect();

        let inserted = host.graft(&qpart, host.root, &|n| n.to_string()).expect("insertion");
        // The inserted node is a first-class part under the host root.
        let ins = host.components.iter().find(|c| c.id == inserted).expect("the inserted component exists");
        assert_eq!(ins.kind, ComponentKind::Part, "what was inserted must be a part");
        assert_eq!(ins.parent, Some(host.root), "under the host root");
        // The timeline of the part is carried over.
        assert_eq!(timeline_kinds(&host, inserted), vec!["sketch", "extrude"], "the inserted part must have the sketch and the extrude");
        // Every new id is disjoint from the host ids and greater than the previous `next_id`.
        let new_comp_ids: Vec<Id> = host.components.iter().map(|c| c.id).filter(|id| !host_ids.contains(id)).collect();
        assert!(!new_comp_ids.is_empty());
        for id in &new_comp_ids {
            assert!(*id > before_next, "the new id {id} must exceed the previous next_id {before_next}");
            assert!(!host_ids.contains(id), "the new id {id} must not collide with the host ids");
        }
        // The feature dimension reached the body of the inserted part.
        let new_body = host.timeline.iter().filter(|n| n.parent == Some(inserted)).find_map(|n| n.kind.body()).expect("the body was inserted");
        assert_eq!(host.feat_dims.get(&new_body).and_then(|m| m.get("height")).map(String::as_str), Some("L"), "the feature dimension must survive insertion");
    }

    #[test]
    fn graft_into_part_is_rejected() {
        let (src, part, _) = source_with_part();
        let qpart = src.subproject_of(part).expect("extraction");
        let mut host = Project::default();
        let hpart = host.new_document(); // The active part.
        assert!(host.graft(&qpart, hpart, &|n| n.to_string()).is_none(), "a part inside a part is not allowed, so inserting into a part is refused");
    }

    #[test]
    fn graft_twice_no_id_collision() {
        let (src, part, _) = source_with_part();
        let qpart = src.subproject_of(part).expect("extraction");
        let mut host = Project::default();
        host.new_document();
        let a = host.graft(&qpart, host.root, &|n| n.to_string()).expect("first insertion");
        let b = host.graft(&qpart, host.root, &|n| n.to_string()).expect("second insertion");
        assert_ne!(a, b, "two insertions must give different roots");
        // Every component id is unique.
        let ids: Vec<Id> = host.components.iter().map(|c| c.id).collect();
        let uniq: std::collections::HashSet<Id> = ids.iter().copied().collect();
        assert_eq!(ids.len(), uniq.len(), "no component id collisions after two insertions");
    }
}

/// The one element a search finds, or none when it finds none or several: a match by place is proof only when nothing
/// else stands there.
fn the_one<T>(mut found: impl Iterator<Item = T>) -> Option<T> {
    let one = found.next()?;
    found.next().is_none().then_some(one)
}

#[cfg(test)]
mod name_map_tests {
    use super::Project;
    use crate::geom::{MeshEdge, MeshFace, Point3};

    fn face(id: u32, at: [f64; 3], normal: [f64; 3]) -> MeshFace {
        MeshFace { triangles: Vec::new(), normal, centroid: Point3::new(at[0], at[1], at[2]), area: 1.0, id }
    }

    /// NAMES ARE TRANSLATED BY PLACE ONLY WHERE ONE ELEMENT STANDS THERE: the wall of a tube and the wall of its bore
    /// share a centroid on the axis and an averaged normal, so a face of the deleted body standing at that spot has two
    /// candidates on the source body. Taking the first gave the bore's reference to the outer wall; with two candidates
    /// the reference is left untranslated and goes red honestly, as `resolve_face_id` does on an ambiguous snapshot.
    #[test]
    fn a_name_is_not_translated_onto_one_of_two_elements_at_its_place() {
        let mut p = Project::default();
        p.regen_faces.insert(1, vec![face(10, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0]), face(11, [3.0, 0.0, 0.0], [1.0, 0.0, 0.0])]);
        p.regen_faces.insert(2, vec![face(20, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0]), face(21, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0]), face(22, [3.0, 0.0, 0.0], [1.0, 0.0, 0.0])]);
        let edge = |id: u32, mid: [f64; 3]| MeshEdge { id, mid, dir: [0.0, 0.0, 1.0], ..MeshEdge::default() };
        p.regen_edges.insert(1, vec![edge(30, [1.0, 0.0, 0.0]), edge(31, [2.0, 0.0, 0.0])]);
        p.regen_edges.insert(2, vec![edge(40, [1.0, 0.0, 0.0]), edge(41, [1.0, 0.0, 0.0]), edge(42, [2.0, 0.0, 0.0])]);
        let map = p.geom_name_map(1, 2);
        assert_eq!(map.get(&11), Some(&22), "the face alone at its place was not translated");
        assert_eq!(map.get(&31), Some(&42), "the edge alone at its place was not translated");
        assert_eq!(map.get(&10), None, "a face with two candidates at its place was translated onto one of them");
        assert_eq!(map.get(&30), None, "an edge with two candidates at its place was translated onto one of them");
    }
}

mod sketch_types;
pub use sketch_types::*;
mod curves;
pub use curves::*;
mod sketch_ops;
mod params;
mod bodies;
mod refs;
mod features;
mod components;
mod keys;
