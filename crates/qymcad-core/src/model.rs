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

/// A sketch: a named group of contours (a tree node). It may be the result of an import, in which case
/// `source` references the embedded original.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Sketch {
    pub id: Id,
    pub name: String,
    /// Stable ids of the contours in this sketch (order is for display).
    pub contour_ids: Vec<Id>,
    /// Import source (an id in `Project::sources`) when the sketch was imported.
    #[serde(default)]
    pub source: Option<Id>,
    /// Typed sketch points, shared between entities: moving a point moves every entity that references it,
    /// which is the foundation of dimensions and constraints.
    #[serde(default)]
    pub points: Vec<SketchPoint>,
    /// Typed entities (line, arc, circle) expressed through point ids.
    #[serde(default)]
    pub entities: Vec<SketchEntity>,
    /// Whether the entity chain is closed (used when tessellating into a contour).
    #[serde(default)]
    pub closed: bool,
    /// Geometric constraints and dimensions (solved by the sketch solver).
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    /// Splines (smooth curves through control points).
    #[serde(default)]
    pub splines: Vec<Spline>,
    /// Text notes and annotations. Not geometry: they never reach a profile or a toolpath.
    #[serde(default)]
    pub notes: Vec<Note>,
    /// Parametric text geometry (labels turned into glyph contours). Does reach profiles and toolpaths.
    #[serde(default)]
    pub texts: Vec<SketchText>,
    /// Editable patterns: a source, the layout parameters and the ids of the derived copies.
    #[serde(default)]
    pub patterns: Vec<SketchPattern>,
    /// The rectangles kept as one shape: their sides, corners, centre and construction diagonals, see `SketchRect`.
    #[serde(default)]
    pub rects: Vec<SketchRect>,
    /// Projections of body geometry (driven entities that reference their source).
    #[serde(default)]
    pub projections: Vec<SketchProjection>,
    /// Placement of the sketch in 3D (a plane, a face or a datum). The 2D geometry is lifted through this
    /// frame. Defaults to the global XY plane.
    #[serde(default)]
    pub plane: crate::feature::SketchPlane,
    /// Id of the origin point (0,0), fixed. Zero means it has not been materialised yet — it is created
    /// lazily on the first constraint or dimension that references the origin. Never reaches a profile.
    #[serde(default)]
    pub origin: Id,
    /// Guide points of the X and Y axes (fixed, at (1,0) and (0,1)), so the axes can be picked as lines for
    /// constraints (parallel, point-on-axis, symmetry). Zero means they have not been created.
    #[serde(default)]
    pub axis_pts: [Id; 2],
    /// THE ANCHOR OF THE FRAME OF REFERENCE at (0,0) - the second end of both axis lines.
    ///
    /// SEPARATE FROM `origin`, and that separation is the whole point. `origin` is a point a person may
    /// snap geometry to, and `merge_close_points` glues anything landing on it INTO it - so the centre of
    /// a circle drawn at zero becomes the origin itself. Moving that circle then moved the origin, and
    /// with it both axes, since an axis is the infinite line through the origin and its guide.
    ///
    /// Reported behaviour, in that order: first "a dimension to the X or Y axis comes out crooked" (the
    /// axes had become diagonals - measured on a file where the origin stood at (-23.45, 18.70)), and
    /// then, after the origin was pinned at zero, "I cannot move a circle away from the origin" - because
    /// the pinned point WAS the circle's centre.
    ///
    /// The anchor is nobody's geometry: it is never picked, never glued, never moved. Zero means it has
    /// not been materialised yet.
    #[serde(default)]
    pub frame: Id,
    /// Offset of the sketch origin within its plane (u,v in frame axes), produced by snapping to an edge
    /// or a vertex when the plane was chosen. `None` means the default origin (the projection of the source
    /// origin). Invariant to body placement: u*X + v*Y travels with the frame (see `sketch_frame`).
    #[serde(default)]
    pub origin_uv: Option<Point2>,
    /// THE PARTS THE LAST SOLVE DID NOT REACH: its time ran out before them (`solver::Budget`), and they stand as
    /// they stood until the next solve. Not a fact of the drawing - it is not written to a file.
    #[serde(skip)]
    pub left_unsolved: usize,
    /// WHAT A DRAG UNDER WAY KEEPS from one frame to the next (`drag::DragSession`). Not a fact of the drawing.
    #[serde(skip)]
    pub(crate) drag_session: Option<Box<drag::DragSession>>,
    /// WHAT ITS LOOPS WERE MADE FROM (`round::Laid`): a rebuild makes again only the loops round what changed since.
    /// Not a fact of the drawing.
    #[serde(skip)]
    pub(crate) laid: Option<Box<round::Laid>>,
    /// WHAT ITS CHECKS REMEMBER OF EACH PART (`solver::PartMemo`): a change counts again the parts it touched. Not a
    /// fact of the drawing.
    #[serde(skip)]
    pub(crate) checked: CheckMemo,
    /// THE PLACES OF THE POINTS AND OF THE ENTITIES BY ID (`Sketch::point`, `Sketch::entity`). Not a fact of the drawing.
    #[serde(skip)]
    pub(crate) point_at: IdPlaces,
    #[serde(skip)]
    pub(crate) entity_at: IdPlaces,
}

/// THE PLACE OF EACH ID IN A LIST, a table that checks itself: a place found is taken only where the list still holds
/// that id there; an id not found puts in the tail the list has grown by, and failing that makes the table again. Asked through `Sketch::point` and
/// `Sketch::entity`, it is right however the lists were changed; looked along the lists, a tool laying an element at
/// every corner of 2 000 rectangles looked through them some ten times a corner - 7 s in a release build. Copied, a
/// sketch makes its own anew.
#[derive(Default)]
pub(crate) struct IdPlaces(std::sync::Mutex<std::collections::HashMap<Id, usize>>);

/// THE MEMORY OF THE CHECKS OF A SKETCH, asked through `&Project` (`sketch_checks`, `sketch_conflicts`). Copied, a
/// sketch starts its own empty; printed, it shows nothing - as `IdPlaces`.
#[derive(Default)]
pub(crate) struct CheckMemo(std::sync::Mutex<crate::solver::PartMemo>);

impl Clone for CheckMemo {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for CheckMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CheckMemo")
    }
}

impl CheckMemo {
    pub(crate) fn with<R>(&self, work: impl FnOnce(&mut crate::solver::PartMemo) -> R) -> R {
        let mut memo = self.0.lock().unwrap_or_else(|e| e.into_inner());
        work(&mut memo)
    }
}

/// Printed without its places: a table filled as ids are asked for is no fact of the sketch, and a sketch compared by
/// its print before and after an operation that changed nothing read as changed where only the table had filled.
impl std::fmt::Debug for IdPlaces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IdPlaces")
    }
}

impl Clone for IdPlaces {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl IdPlaces {
    fn find<T>(&self, list: &[T], id: Id, id_of: impl Fn(&T) -> Id) -> Option<usize> {
        let mut places = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&i) = places.get(&id) {
            if list.get(i).is_some_and(|x| id_of(x) == id) {
                return Some(i);
            }
        }
        // most changes add to the end of a list: the new tail is put in first, the whole list only if that is not it
        let known = places.len();
        if known < list.len() {
            for (i, x) in list.iter().enumerate().skip(known) {
                places.insert(id_of(x), i);
            }
            if let Some(&i) = places.get(&id) {
                if list.get(i).is_some_and(|x| id_of(x) == id) {
                    return Some(i);
                }
            }
        }
        *places = list.iter().enumerate().map(|(i, x)| (id_of(x), i)).collect();
        places.get(&id).copied()
    }
}

impl Sketch {
    /// The point of id `id` (`IdPlaces`).
    pub fn point(&self, id: Id) -> Option<&SketchPoint> {
        self.point_at.find(&self.points, id, |p| p.id).map(|i| &self.points[i])
    }

    /// The entity of id `id` (`IdPlaces`).
    pub fn entity(&self, id: Id) -> Option<&SketchEntity> {
        self.entity_at.find(&self.entities, id, |e| e.id).map(|i| &self.entities[i])
    }

    /// System points of the sketch: the origin and the endpoints of the X and Y axes. They cannot be
    /// dragged, deleted or counted as free.
    ///
    /// One source of truth on purpose: spelled out by hand in six places the set drifts apart, some copies
    /// forgetting the origin and others the axes.
    /// THE CONSTRAINTS A CUT CORNER HOLDS ITSELF BY, by their index: the virtual sharp standing on the extension of each
    /// side (`PointOnLine` of a point no geometry ends at), the legs of a symmetric chamfer kept equal on it (`Equal` from
    /// the sharp to the two ends of the cut line), and the arc of a fillet touching the lines it ends on (`Tangent` of a
    /// line to the arc that ends at one of its points). They are the corner's own, as the turn of a rectangle is the
    /// rectangle's: shown, a chamfer of 3 carried two "=" and a point-on-line badge out in the air beside its cut, and a
    /// fillet a tangency badge on each line. A dimension a person measured to the sharp is not among them.
    pub fn corner_holders(&self) -> std::collections::HashSet<usize> {
        // no constraint of a kind a corner holds itself by, no holder - before the points of every entity are gathered:
        // 6 ms on 70 000 segments, and the list of constraints asks every frame
        if !self.constraints.iter().any(|c| matches!(c, Constraint::PointOnLine { .. } | Constraint::Equal { .. } | Constraint::Tangent { .. })) {
            return Default::default();
        }
        let drawn: std::collections::HashSet<Id> = self
            .entities
            .iter()
            .flat_map(|e| match e.kind {
                EntityKind::Line { a, b } => vec![a, b],
                EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
                EntityKind::Circle { center, .. } => vec![center],
                EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
            })
            .collect();
        let cut = |x: Id, y: Id| self.entities.iter().any(|e| matches!(e.kind, EntityKind::Line { a, b } if (a == x && b == y) || (a == y && b == x)));
        self.constraints
            .iter()
            .enumerate()
            .filter(|(_, c)| match **c {
                Constraint::PointOnLine { p, .. } => !drawn.contains(&p) && !self.system_ids().contains(&p),
                Constraint::Equal { a, b, c, d } => a == c && !drawn.contains(&a) && cut(b, d),
                Constraint::Tangent { a, b, c, .. } => {
                    self.entities.iter().any(|e| matches!(e.kind, EntityKind::Arc { center, a: x, b: y, .. } if center == c && [x, y].iter().any(|q| *q == a || *q == b)))
                }
                _ => false,
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// WHETHER THE CIRCLE OR ARC ABOUT `centre` CARRIES A SIZE OF ITS OWN: a radius or a diameter, the length of the arc,
    /// or the chord between its two ends. Where it does, no passive radius is labelled beside it: a fillet sized by its
    /// chord or its arc length showed an R it does not hold by as well.
    pub fn rim_sized(&self, centre: Id) -> bool {
        let chord = |x: Id, y: Id| self.entities.iter().any(|e| matches!(e.kind, EntityKind::Arc { center, a, b, .. } if center == centre && ((a == x && b == y) || (a == y && b == x))));
        self.constraints.iter().any(|c| match *c {
            Constraint::Diameter { c, .. } | Constraint::ArcLength { c, .. } => c == centre,
            Constraint::Distance { a, b, .. } => chord(a, b),
            _ => false,
        })
    }

    /// The points not drawn among the sketch's own: the frame of reference, drawn by the axis marker. The virtual sharp of
    /// a cut corner is drawn, and picked, like any point.
    pub fn unseen_points(&self) -> std::collections::HashSet<Id> {
        self.system_ids().into_iter().collect()
    }

    pub fn system_ids(&self) -> Vec<Id> {
        std::iter::once(self.origin).chain(std::iter::once(self.frame)).chain(self.axis_pts).filter(|id| *id != 0).collect()
    }

    /// PUT THE FRAME OF REFERENCE BACK WHERE IT BELONGS: the origin at (0,0), the axis guides at (1,0)
    /// and (0,1). Returns whether anything had to be moved.
    ///
    /// Reported behaviour: "I cannot put a dimension from the centre of a circle to the X or Y axis - a
    /// crooked one is placed instead." Measured on the reporter's file: the origin stood at
    /// (-23.454944474040893, 18.70469405576201).
    ///
    /// AN AXIS IS THE INFINITE LINE THROUGH THE ORIGIN AND ITS GUIDE. Move the origin and the "X axis"
    /// becomes a diagonal; a distance to it is then measured honestly, to the wrong line, and the number
    /// on the drawing belongs to nothing. The same for the guides.
    ///
    /// Why this is repaired rather than only prevented: `Constraint::Fixed` pins a point to where it IS,
    /// not to zero, so a frame that moved once is locked in the wrong place for good - and documents in
    /// that state already exist. Called on every rebuild of the sketch, so a file opens repaired.
    pub fn pin_frame(&mut self) -> bool {
        // THE ORIGIN IS PINNED HERE TOO, and that is safe only because it is nobody's geometry by the time
        // this runs: `regen_sketch` calls `detach_geometry_from_origin` first, which gives any drawing
        // welded to the origin a point of its own. Pinning a point that geometry sits on is what nailed
        // down a shape drawn at zero. The frame itself stands on `frame`, which is never anybody's.
        // A DOCUMENT WHOSE AXES WERE HUNG ON THE ORIGIN IS REPAIRED HERE.
        //
        // Before the anchor existed, an axis was the line through the ORIGIN and its guide, and every
        // dimension to an axis holds those two ids. Such files exist and open crooked. Any constraint
        // naming the origin together with an axis guide is re-pointed at the anchor: the line it means
        // is the axis, and the axis now stands on the anchor.
        if self.origin != 0 && self.frame != 0 {
            let guides = self.axis_pts;
            let (origin, frame) = (self.origin, self.frame);
            for c in &mut self.constraints {
                let pts = c.points();
                if pts.contains(&origin) && guides.iter().any(|g| *g != 0 && pts.contains(g)) {
                    remap_constraint_point(c, origin, frame);
                }
            }
        }
        let want: [(Id, f64, f64); 4] = [(self.frame, 0.0, 0.0), (self.origin, 0.0, 0.0), (self.axis_pts[0], 1.0, 0.0), (self.axis_pts[1], 0.0, 1.0)];
        let mut moved = false;
        for (id, x, y) in want {
            if id == 0 {
                continue;
            }
            if let Some(p) = self.points.iter_mut().find(|p| p.id == id) {
                if p.x != x || p.y != y {
                    p.x = x;
                    p.y = y;
                    moved = true;
                }
            }
        }
        moved
    }

    /// Driven points (projections of a body). They cannot be dragged or deleted one by one, being derived
    /// from body geometry. Same role as `system_ids`, and asked for in the same places.
    pub fn projected_points(&self) -> std::collections::HashSet<Id> {
        self.projections.iter().flat_map(|p| p.points.iter().copied()).collect()
    }

    /// Driven entities (projections of a body).
    pub fn projected_entities(&self) -> std::collections::HashSet<Id> {
        self.projections.iter().flat_map(|p| p.entities.iter().copied()).collect()
    }

    /// Points that cannot be dragged: the system ones (origin and axes) plus the driven projections.
    pub fn immovable_points(&self) -> std::collections::HashSet<Id> {
        let mut s = self.projected_points();
        s.extend(self.system_ids());
        s
    }

    /// Points a person has pinned with the anchor constraint.
    pub fn pinned_points(&self) -> std::collections::HashSet<Id> {
        self.constraints
            .iter()
            .filter_map(|c| match c {
                Constraint::Fixed { p } => Some(*p),
                _ => None,
            })
            .collect()
    }

    /// WHAT GOES WITH THE POINT `centre` WHEN IT IS DRAGGED, as a circle goes with its centre: the corners of a rectangle
    /// it is the centre of, the two ends of an arc it is the centre of, every point of a slot it is the centre of an end
    /// of (a slot's two arcs are held to one radius, `EqualRadius`), and the axis ends of an ellipse. Left behind, the
    /// drag pulled the centre alone: an arc and a slot were refused the drag, and an ellipse turned and changed shape.
    /// The centre of a fillet - an arc touching its lines (`Tangent`) with no partner of one radius - carries nothing:
    /// the lines it touches hold it. Empty for any other point.
    pub fn carried_with(&self, centre: Id) -> Vec<Id> {
        let rect: Vec<Id> = self.rects.iter().filter(|r| r.centre == centre).flat_map(|r| r.corners).collect();
        if !rect.is_empty() {
            return rect;
        }
        if let Some((ma, mi)) = self.entities.iter().find_map(|e| match e.kind {
            EntityKind::Ellipse { c, ma, mi } if c == centre => Some((ma, mi)),
            _ => None,
        }) {
            return vec![ma, mi];
        }
        let Some(arc) = self.entities.iter().find(|e| matches!(e.kind, EntityKind::Arc { center, .. } if center == centre)) else { return Vec::new() };
        let touches = self.constraints.iter().any(|c| matches!(*c, Constraint::Tangent { c, .. } if c == centre));
        let partner = self.constraints.iter().find_map(|c| match *c {
            Constraint::EqualRadius { c1, c2 } if c1 == centre => Some(c2),
            Constraint::EqualRadius { c1, c2 } if c2 == centre => Some(c1),
            _ => None,
        });
        match (touches, partner) {
            // a slot: every point of the shape the arc is part of, the centres of its arcs with them
            (_, Some(_)) => {
                let mut shape = vec![arc.id];
                let mut k = 0;
                while k < shape.len() {
                    let here = self.entities.iter().find(|e| e.id == shape[k]).map_or(Vec::new(), entity_points);
                    for e in &self.entities {
                        if !shape.contains(&e.id) && entity_points(e).iter().any(|p| here.contains(p)) {
                            shape.push(e.id);
                        }
                    }
                    k += 1;
                }
                let mut out: Vec<Id> = shape.iter().flat_map(|id| self.entities.iter().find(|e| e.id == *id).map_or(Vec::new(), entity_points)).filter(|p| *p != centre).collect();
                out.sort_unstable();
                out.dedup();
                out
            }
            // a fillet: held by the lines it touches
            (true, None) => Vec::new(),
            // an arc of its own: its two ends
            (false, None) => match arc.kind {
                EntityKind::Arc { a, b, .. } => vec![a, b],
                _ => Vec::new(),
            },
        }
    }

    /// EVERYTHING AN EDITING TOOL MUST LEAVE WHERE IT IS: the frame, the driven projections, and whatever
    /// the person pinned.
    ///
    /// Why the pinned ones need naming at all. `Constraint::Fixed` carries no coordinates - it pins the point
    /// to where it IS at the moment of the solve. So a tool that shifts the point first and asks the solver
    /// afterwards gets a constraint satisfied by the NEW place: the pin travels with the drawing instead of
    /// holding it, the glyph stays green and nobody is told. Not the same list as `immovable_points`, which
    /// answers "may this be deleted one by one" - a pinned point is deleted like any other.
    pub fn held_points(&self) -> std::collections::HashSet<Id> {
        let mut s = self.immovable_points();
        s.extend(self.pinned_points());
        s
    }
}

/// A text note in a sketch (an annotation). Not geometry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub x: f64,
    pub y: f64,
    pub text: String,
}

/// WHERE THE OUTLINES OF A TEXT CAME FROM.
///
/// Kept in the document beside the baked glyphs, because a re-bake without it uses whatever font happens to
/// be loaded: open the file again, change the height of a label, and it is silently redrawn in somebody
/// else's typeface. Measured: a label written in Liberation Sans came back in Cantarell, 1.201 wide for its
/// height against 1.379.
///
/// Both the name and the path, on purpose. The name survives a move to another machine, where the file lies
/// elsewhere; the path finds the file at once on this one. The index is the face inside a collection - a
/// `.ttc` holds several, and a variable font its instances.
///
/// Empty means "not recorded": labels written before this was kept. Their drawing is intact, since the
/// glyphs are in the file; what cannot be done to them is an honest re-bake.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct FontRef {
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub index: u32,
}

impl FontRef {
    /// Is anything recorded at all.
    pub fn is_empty(&self) -> bool {
        self.family.is_empty() && self.path.is_empty()
    }
}

/// Parametric sketch text: a label turned into glyph contours, which are real geometry for profiles and
/// toolpaths.
///
/// The parameters (position, height, angle, string) are the source of truth; `glyphs` are the baked glyph
/// polylines in world coordinates. Baking happens in the application, because the font lives there: after
/// an edit to the string or the height the application re-bakes the glyphs and calls `regen_sketch`. The
/// text stays one selectable, movable, editable object, unlike loose contours produced by a one-way
/// conversion.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SketchText {
    pub id: Id,
    pub x: f64,
    pub y: f64,
    pub height: f64,
    #[serde(default)]
    pub angle: f64,
    pub text: String,
    #[serde(default)]
    pub construction: bool,
    /// Baked closed glyph polylines (world coordinates).
    #[serde(default)]
    pub glyphs: Vec<Vec<Point2>>,
    /// The font these glyphs were baked from - see `FontRef`.
    #[serde(default)]
    pub font: FontRef,
}

/// What a body is for the purpose of export (see [`Project::export_kind`]): one policy for STEP and STL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    /// Live B-rep: exported to STEP as exact geometry and to STL as a tessellation at the chosen accuracy.
    Brep,
    /// A mesh with no recipe (an STL import): no B-rep exists at all. It goes into STL; STEP skips it,
    /// because there is nothing exact to write, and the export has to say so.
    MeshOnly,
    /// A body with a recipe but without a live B-rep: a failed regenerate (a red node). The screen shows
    /// the last good geometry and STL exports that same geometry, while STEP will not contain the body at
    /// all. Both exports have to report this; staying silent about it is the defect.
    Stale,
}

/// Pattern kind: linear (step dx, dy) or circular (centre plus sector).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum PatternKind {
    /// Linear pattern: `count` copies along (dx,dy) and `count2` copies along the second direction
    /// (dx2,dy2), forming a grid of rows by columns. `count2 <= 1` makes it one-dimensional; the second
    /// direction is optional.
    Linear {
        dx: f64,
        dy: f64,
        count: u32,
        #[serde(default)]
        dx2: f64,
        #[serde(default)]
        dy2: f64,
        #[serde(default)]
        count2: u32,
    },
    Circular {
        cx: f64,
        cy: f64,
        count: u32,
        total_deg: f64,
    },
}

/// A SKETCH RECTANGLE KEPT AS ONE SHAPE: four sides each held at its turn (`Constraint::Orientation`: the first and the
/// third at the turn of the rectangle, the second and the fourth square to them), and a centre on the middle of its
/// diagonal - drawn as two construction diagonals for a rectangle drawn from its centre. It stays a rectangle until it
/// is broken - a side deleted - and then the record, the centre and the diagonals go and four plain lines are left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SketchRect {
    pub id: Id,
    /// The corners in order round the rectangle.
    pub corners: [Id; 4],
    /// The sides: corner k to corner k + 1.
    pub sides: [Id; 4],
    pub centre: Id,
    /// The construction diagonals of a rectangle drawn from its centre: corner 0 to 2, corner 1 to 3. A rectangle drawn
    /// by its corners has none - its centre is held on the middle without them - as the diagonals would split every
    /// line crossing it for Trim (a construction line is a boundary).
    pub diagonals: Option<[Id; 2]>,
    /// What a width or a height changed grows the rectangle from.
    pub anchor: RectAnchor,
}

/// THE WIDTH AND THE HEIGHT DIMENSIONS OF A RECTANGLE, as indices of its sketch's constraints: the first along its
/// first side, the second along the side after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RectDims {
    pub width: usize,
    pub height: usize,
}

/// What stays put when the width or the height of a rectangle changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RectAnchor {
    /// The corner it was drawn from (a point id).
    Corner(Id),
    /// Its centre: a rectangle drawn from the centre grows about it.
    Centre,
}

/// An editable sketch pattern: source entities plus layout parameters produce derived instances.
///
/// The instance ids are stored so that editing the parameters can recreate them. Instances are real
/// entities and reach profiles and toolpaths; this record only ties them back to their source so the
/// pattern stays editable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SketchPattern {
    pub id: Id,
    pub source: Vec<Id>,
    pub kind: PatternKind,
    #[serde(default)]
    pub instances: Vec<Id>,
}

/// Source of a projection into a sketch: a reference to body geometry by persistent name rather than by
/// number. A projection has to survive edits higher up the timeline, which is the whole point of it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProjSource {
    /// A single edge of a body.
    Edge(u32),
    /// A whole face: its entire outline is projected. The list of edges is deliberately not stored, since
    /// a fillet or a cut changes which edges there are and a stored list would silently fall behind the
    /// part.
    Face(u32),
}

/// Projection of body geometry into a sketch. Driven geometry: it cannot be dragged by hand and is
/// recomputed from the body on every rebuild, but constraints and dimensions may reference it.
///
/// Ids of the derived points and entities stay stable as long as the structure of the source has not
/// changed; otherwise a constraint attached to a corner of the projection would fall off on every rebuild
/// of the body.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SketchProjection {
    pub id: Id,
    /// Source body.
    pub body: Id,
    pub src: ProjSource,
    /// Derived points (driven).
    #[serde(default)]
    pub points: Vec<Id>,
    /// Derived entities (driven).
    #[serde(default)]
    pub entities: Vec<Id>,
    /// THE EDGE OF THE BODY EACH OF `entities` CAME FROM, by its persistent id, in the same order: a rebuild matches the
    /// curves by their edge, so the ids - and the dimensions and the ties on them - stay with the edge whatever order the
    /// kernel gives the edges in.
    #[serde(default)]
    pub edges: Vec<u32>,
}

/// A sketch spline: a smooth Catmull-Rom curve through control points.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Spline {
    pub points: Vec<Id>,
    /// Explicit tangent vector at each node; `None` means the automatic Catmull-Rom tangent, which follows
    /// the nodes. Dragging a handle makes the tangent explicit, pinning the shape at that node.
    #[serde(default)]
    pub tangents: Vec<Option<[f64; 2]>>,
    pub closed: bool,
    /// A construction spline: drawn dashed and never reaching a profile or a toolpath.
    #[serde(default)]
    pub construction: bool,
}

/// A sketch constraint or dimension, expressed through point ids. Solved by the sketch solver.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Constraint {
    /// Pin a point at its current position (an anchor).
    Fixed { p: Id },
    /// Horizontal: points `a` and `b` share the same height (equal y).
    Horizontal { a: Id, b: Id },
    /// Vertical: points `a` and `b` share the same vertical (equal x).
    Vertical { a: Id, b: Id },
    /// THE TURN OF A SKETCH RECTANGLE: the side from `a` to `b` stands at `deg` degrees to the X axis. The rectangle
    /// holds it itself and it is never drawn nor typed: a corner dragged changes the size and leaves the turn, the
    /// Rotate tool changes `deg`, and an angle dimension a person puts on a side takes its place. Without it a
    /// rectangle held by parallel and perpendicular sides turned under a dragged corner.
    Orientation { a: Id, b: Id, deg: f64 },
    /// Points `a` and `b` coincide.
    Coincident { a: Id, b: Id },
    /// Dimension: the distance between `a` and `b` equals `d`. `off` is the offset of the dimension line
    /// (a perpendicular shift of the label in pixels, for manual placement). `expr` is an optional
    /// expression (`w/2`, `len+5`); when it is not empty, `d` is recomputed from the project parameters,
    /// making the dimension parametric.
    Distance {
        a: Id,
        b: Id,
        d: f64,
        #[serde(default)]
        off: f64,
        #[serde(default)]
        expr: String,
        /// A reference (driven) dimension: it does not constrain the geometry and only displays the
        /// measured value. Applied automatically where an ordinary dimension would be redundant.
        #[serde(default)]
        driven: bool,
        /// Orientation of a linear dimension: 0 aligned (euclidean |AB|), 1 horizontal (|dx|), 2 vertical
        /// (|dy|). Chosen by cursor position while placing it.
        #[serde(default)]
        axis: u8,
        /// Where the text stands along the dimension line, as a share of it from its first end: 0.5 in the middle,
        /// below 0 or above 1 past an arrow, the line then running on to the text as a shelf. Absent: the middle, or a
        /// shelf past the second arrow when the text does not fit between them.
        #[serde(default)]
        at: Option<f64>,
    },
    /// Segments `a-b` and `c-d` are parallel.
    Parallel { a: Id, b: Id, c: Id, d: Id },
    /// Segments `a-b` and `c-d` are perpendicular.
    Perpendicular { a: Id, b: Id, c: Id, d: Id },
    /// Segments `a-b` and `c-d` have equal length.
    Equal { a: Id, b: Id, c: Id, d: Id },
    /// Angle at vertex `b` between the rays `b-a` and `b-c`, in degrees. `expr` is a parametric
    /// expression; when it is not empty, `deg` is recomputed from the project parameters.
    Angle {
        a: Id,
        b: Id,
        c: Id,
        deg: f64,
        #[serde(default)]
        expr: String,
        /// A reference (driven) angle; see `Distance::driven`.
        #[serde(default)]
        driven: bool,
        /// Where the arc of the dimension stands: its radius from the vertex, in sketch units; 0 is the default
        /// distance on screen.
        #[serde(default)]
        off: f64,
        /// Where the label stands along the arc, as a share of the angle from the side `b-a` towards `b-c`: 0.5 on the
        /// bisector (also when absent), below 0 or above 1 past a side, the arc then running on to it. A share rather
        /// than a direction, so the label keeps its place when the sides turn.
        #[serde(default)]
        at: Option<f64>,
    },
    /// Segments `a-b` and `c-d` are collinear (they lie on one straight line).
    Collinear { a: Id, b: Id, c: Id, d: Id },
    /// Point `p` is the midpoint of segment `a-b`.
    Midpoint { p: Id, a: Id, b: Id },
    /// Tangency: the line `a-b` touches the circle with centre `c` and radius `r`.
    Tangent { a: Id, b: Id, c: Id, r: f64 },
    /// Tangency of two circles or arcs (by centre ids): the centre distance equals r1 + r2 when
    /// `external`, or |r1 - r2| when internal. The radii are solver variables keyed by the centres.
    CircleTangent { c1: Id, c2: Id, external: bool },
    /// Points `a` and `b` are symmetric about the axis `la-lb`.
    Symmetric { a: Id, b: Id, la: Id, lb: Id },
    /// Point `p` lies on the infinite line through `a` and `b`: a point coincident with an edge, an axis
    /// or a construction line.
    PointOnLine { p: Id, a: Id, b: Id },
    /// Two circles (by centre id) have equal radii: their radius variables are equal.
    EqualRadius { c1: Id, c2: Id },
    /// Point `p` lies on the circle or arc centred at point `c` (distance equals the radius variable of the
    /// centre). It also implicitly keeps the endpoints of an arc on its circle, which is what makes an arc
    /// a real entity.
    PointOnCircle { p: Id, c: Id },
    /// Concentric: centres `c1` and `c2` coincide. A distinct kind with its own glyph, not to be confused
    /// with coincidence of ordinary points.
    Concentric { c1: Id, c2: Id },
    /// Angle between two segments `a-b` and `c-d` that share no vertex, in degrees.
    AngleLines {
        a: Id,
        b: Id,
        c: Id,
        d: Id,
        deg: f64,
        #[serde(default)]
        expr: String,
        #[serde(default)]
        driven: bool,
        /// Where the arc of the dimension stands: its radius from the intersection, in sketch units; 0 is the
        /// default distance on screen. Set while the dimension follows the cursor, as `off` of `Distance`.
        #[serde(default)]
        off: f64,
        /// Where the label stands along the arc; see `Angle::at`.
        #[serde(default)]
        at: Option<f64>,
    },
    /// Radius or diameter dimension of the circle centred at `c`. `d` is the value — a radius, or a
    /// diameter when `diam` is set. `off` is the label offset; `expr` and `driven` behave as in `Distance`.
    Diameter {
        c: Id,
        d: f64,
        #[serde(default)]
        off: f64,
        #[serde(default)]
        expr: String,
        #[serde(default)]
        driven: bool,
        #[serde(default)]
        diam: bool,
        /// Where the text stands ON the dimension line, as a share of it from its start (the far rim of a diameter,
        /// the centre of a radius), laid along the line; absent, the text stands on the shelf past the knee.
        #[serde(default)]
        at: Option<f64>,
    },
    /// Dimension: the perpendicular distance from point `p` to the line `a-b` equals `d`. Covers
    /// point-to-line, line-to-parallel-line and distance-to-axis dimensions. `off`, `expr` and `driven`
    /// behave as in `Distance`.
    DistancePL {
        p: Id,
        a: Id,
        b: Id,
        d: f64,
        #[serde(default)]
        off: f64,
        #[serde(default)]
        expr: String,
        #[serde(default)]
        driven: bool,
        /// Where the text stands along the dimension line, as a share of it from its first end: 0.5 in the middle,
        /// below 0 or above 1 past an arrow, the line then running on to the text as a shelf. Absent: the middle, or a
        /// shelf past the second arrow when the text does not fit between them.
        #[serde(default)]
        at: Option<f64>,
    },
    /// Tangent (edge-to-edge) dimension: the distance between `c1` and `c2` corrected by the radii of their
    /// circles. `m1` and `m2` are in {-1, 0, +1}: 0 is an ordinary point, -1 the near edge of the circle,
    /// +1 the far edge. The residual is `dist(c1,c2) + m1*r1 + m2*r2 - d`.
    EdgeDistance {
        c1: Id,
        c2: Id,
        d: f64,
        m1: i8,
        m2: i8,
        #[serde(default)]
        off: f64,
        #[serde(default)]
        expr: String,
        #[serde(default)]
        driven: bool,
        /// Where the text stands along the dimension line; see `Distance::at`.
        #[serde(default)]
        at: Option<f64>,
    },
    /// Arc length dimension: the arc (centre `c`, endpoints `a` and `b`, direction `ccw`) has length `len`,
    /// equal to R * theta for radius R and swept angle theta. A driving one holds the length, a reference
    /// one measures it.
    ArcLength {
        c: Id,
        a: Id,
        b: Id,
        ccw: bool,
        len: f64,
        #[serde(default)]
        off: f64,
        #[serde(default)]
        expr: String,
        #[serde(default)]
        driven: bool,
    },
}

impl Constraint {
    /// Points a constraint references.
    ///
    /// A fact about the constraint itself, so it lives in the core rather than in the interface. The
    /// constraint list uses it to name the participants, and so does anything else that asks what a
    /// constraint holds: deletion, highlighting, diagnostics.
    pub fn points(&self) -> Vec<Id> {
        use Constraint::*;
        match *self {
            Fixed { p } => vec![p],
            Horizontal { a, b } | Vertical { a, b } | Orientation { a, b, .. } | Coincident { a, b } | Distance { a, b, .. } => vec![a, b],
            Parallel { a, b, c, d } | Perpendicular { a, b, c, d } | Equal { a, b, c, d } | Collinear { a, b, c, d } | AngleLines { a, b, c, d, .. } => vec![a, b, c, d],
            Angle { a, b, c, .. } | Midpoint { p: a, a: b, b: c } | PointOnLine { p: a, a: b, b: c } | DistancePL { p: a, a: b, b: c, .. } | ArcLength { c: a, a: b, b: c, .. } => vec![a, b, c],
            Tangent { a, b, c, .. } => vec![a, b, c],
            CircleTangent { c1, c2, .. } | EqualRadius { c1, c2 } | Concentric { c1, c2 } | EdgeDistance { c1, c2, .. } => vec![c1, c2],
            Symmetric { a, b, la, lb } => vec![a, b, la, lb],
            PointOnCircle { p, c } => vec![p, c],
            Diameter { c, .. } => vec![c],
        }
    }

    /// WHERE THE LABEL OF A DIMENSION STANDS - its offset, its place along its line or arc (NaN where there is none) and
    /// the axis of a length - for the fingerprint of the document; `None` for a constraint with no label.
    pub fn label_place(&self) -> Option<[f64; 3]> {
        match *self {
            Constraint::Distance { off, axis, at, .. } => Some([off, at.unwrap_or(f64::NAN), axis as f64]),
            Constraint::DistancePL { off, at, .. } | Constraint::Diameter { off, at, .. } | Constraint::EdgeDistance { off, at, .. } => Some([off, at.unwrap_or(f64::NAN), 0.0]),
            Constraint::ArcLength { off, .. } => Some([off, f64::NAN, 0.0]),
            Constraint::Angle { off, at, .. } | Constraint::AngleLines { off, at, .. } => Some([off, at.unwrap_or(f64::NAN), 0.0]),
            _ => None,
        }
    }

    /// Numeric value of a dimensional constraint (for fingerprints and comparisons); `None` for the rest.
    pub fn dim_value(&self) -> Option<f64> {
        match *self {
            Constraint::Distance { d, .. } | Constraint::DistancePL { d, .. } | Constraint::Diameter { d, .. } | Constraint::EdgeDistance { d, .. } => Some(d),
            Constraint::Angle { deg, .. } | Constraint::AngleLines { deg, .. } => Some(deg),
            Constraint::ArcLength { len, .. } => Some(len),
            _ => None,
        }
    }

    /// The formula a dimension is set by, as typed (empty when a number was typed); `None` for a constraint that is
    /// not a dimension.
    pub fn dim_expr(&self) -> Option<&str> {
        match self {
            Constraint::Distance { expr, .. }
            | Constraint::Angle { expr, .. }
            | Constraint::AngleLines { expr, .. }
            | Constraint::Diameter { expr, .. }
            | Constraint::DistancePL { expr, .. }
            | Constraint::EdgeDistance { expr, .. }
            | Constraint::ArcLength { expr, .. } => Some(expr),
            _ => None,
        }
    }

    /// A reference (driven) dimension or angle: it does not constrain the geometry, only displays a value.
    pub fn is_driven(&self) -> bool {
        matches!(
            self,
            Constraint::Distance { driven: true, .. }
                | Constraint::Angle { driven: true, .. }
                | Constraint::DistancePL { driven: true, .. }
                | Constraint::Diameter { driven: true, .. }
                | Constraint::AngleLines { driven: true, .. }
                | Constraint::ArcLength { driven: true, .. }
                | Constraint::EdgeDistance { driven: true, .. }
        )
    }
}

/// Whether point `p` lies inside polygon `poly` (ray casting).
fn point_in_poly(poly: &[Point2], p: Point2) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Whether the contours touch: the inner one reached the boundary of the outer one.
///
/// Such a contour is not a hole. An inner loop that touches the outer loop forms no face, and the whole
/// region containing it silently disappears. The tolerance is relative (a fraction of the outer bounding
/// box): an absolute one reports false touches on a part hundreds of millimetres across and misses real
/// ones on a small part.
fn polys_touch(outer: &[Point2], inner: &[Point2]) -> bool {
    if outer.len() < 3 || inner.len() < 3 {
        return false;
    }
    let (mut lo, mut hi) = (Point2::new(f64::MAX, f64::MAX), Point2::new(f64::MIN, f64::MIN));
    for p in outer {
        lo = Point2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let tol = ((hi.x - lo.x).hypot(hi.y - lo.y) * 1e-6).max(1e-9);
    for a in inner {
        for i in 0..outer.len() {
            let (b, c) = (outer[i], outer[(i + 1) % outer.len()]);
            let (vx, vy) = (c.x - b.x, c.y - b.y);
            let l2 = vx * vx + vy * vy;
            let t = if l2 > 1e-18 { (((a.x - b.x) * vx + (a.y - b.y) * vy) / l2).clamp(0.0, 1.0) } else { 0.0 };
            if (a.x - (b.x + t * vx)).hypot(a.y - (b.y + t * vy)) <= tol {
                return true;
            }
        }
    }
    false
}

fn poly_contains(outer: &[Point2], inner: &[Point2]) -> bool {
    !inner.is_empty() && inner.iter().all(|p| point_in_poly(outer, *p))
}

/// Orient `axis` from `center` towards the material: the side of the plane through `center` perpendicular
/// to `axis` holding more of `verts` points into the body.
///
/// Used by threads, which grow from the rim into the material. Counting vertices is more reliable than a
/// centroid on asymmetric or L-shaped bodies.
pub fn orient_axis_into_mesh(center: [f64; 3], axis: [f64; 3], verts: &[crate::geom::Point3]) -> [f64; 3] {
    let (mut pos, mut neg) = (0i64, 0i64);
    for v in verts {
        let dp = (v.x - center[0]) * axis[0] + (v.y - center[1]) * axis[1] + (v.z - center[2]) * axis[2];
        if dp > 1e-6 {
            pos += 1;
        } else if dp < -1e-6 {
            neg += 1;
        }
    }
    if neg > pos {
        [-axis[0], -axis[1], -axis[2]]
    } else {
        axis
    }
}

/// Value of dimension `key`: the expression from `dims` when it is present and evaluates against `vars`,
/// otherwise `fallback`.
fn eval_dim(dims: Option<&std::collections::HashMap<String, String>>, key: &str, fallback: f64, vars: &std::collections::HashMap<String, f64>) -> f64 {
    dims.and_then(|m| m.get(key)).map(|e| e.trim()).filter(|e| !e.is_empty()).and_then(|e| crate::expr::eval(e, vars).ok()).unwrap_or(fallback)
}

/// A sketch point with a stable id, referenced by entities and dimensions.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SketchPoint {
    pub id: Id,
    pub x: f64,
    pub y: f64,
}

/// A typed sketch entity.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SketchEntity {
    pub id: Id,
    pub kind: EntityKind,
    /// Construction geometry: never part of a profile, only a support for snapping and constraints. Drawn
    /// dashed.
    #[serde(default)]
    pub construction: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum EntityKind {
    /// Segment between points `a` and `b`.
    Line { a: Id, b: Id },
    /// Arc around `center` from `a` to `b` (counter-clockwise when `ccw`).
    Arc { center: Id, a: Id, b: Id, ccw: bool },
    /// Circle around `center` with radius `r`.
    Circle { center: Id, r: f64 },
    /// Ellipse: centre `c`, endpoint of the major semi-axis `ma`, endpoint of the minor semi-axis `mi`.
    /// The semi-axes are kept perpendicular by an implicit constraint; the major axis is |c-ma|, the minor
    /// |c-mi|, and the rotation is the direction from `c` to `ma`. A real analytic entity with five degrees
    /// of freedom, not a polygon.
    Ellipse { c: Id, ma: Id, mi: Id },
}

/// Snapshot of sketch geometry for the clipboard (copy, cut and paste between sketches or within one).
///
/// Points keep their old local ids so entity references can be remapped; `ref_x` and `ref_y` are the
/// reference point given at copy time, from which the paste is measured.
#[derive(Clone, Debug, Default)]
pub struct GeomClip {
    /// Points: (old local id, x, y).
    pub points: Vec<(Id, f64, f64)>,
    /// Entities holding the old ids; they are remapped on paste.
    pub entities: Vec<SketchEntity>,
    /// Constraints and dimensions internal to the set (every reference inside the copy, except `Fixed`),
    /// holding the old ids and remapped on paste. This way a copy carries its own shape (horizontals,
    /// verticals, edge dimensions) but not the anchors or the axes.
    pub constraints: Vec<Constraint>,
    /// Reference point, given at copy time.
    pub ref_x: f64,
    pub ref_y: f64,
}

impl GeomClip {
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}

/// Intersection of segments `a-b` and `c-d`. Returns the parameter t in (0,1) along `a-b` when the
/// intersection lies strictly inside `a-b` and within `c-d`.
fn seg_seg_t(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<f64> {
    let ([ax, ay], [bx, by], [cx, cy], [dx, dy]) = (a, b, c, d);
    let (rx, ry) = (bx - ax, by - ay);
    let (sx, sy) = (dx - cx, dy - cy);
    let rxs = rx * sy - ry * sx;
    if rxs.abs() < 1e-12 {
        return None; // parallel
    }
    let (qpx, qpy) = (cx - ax, cy - ay);
    let t = (qpx * sy - qpy * sx) / rxs;
    let u = (qpx * ry - qpy * rx) / rxs;
    if t > 1e-9 && t < 1.0 - 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u) {
        Some(t)
    } else {
        None
    }
}

/// Intersection of the infinite lines through `a-b` and `c-d`. `None` when they are parallel.
fn line_intersect_inf(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<(f64, f64)> {
    let ([ax, ay], [bx, by], [cx, cy], [dx, dy]) = (a, b, c, d);
    let (r1, r2) = (bx - ax, by - ay);
    let (s1, s2) = (dx - cx, dy - cy);
    let den = r1 * s2 - r2 * s1;
    if den.abs() < 1e-9 {
        return None;
    }
    let t = ((cx - ax) * s2 - (cy - ay) * s1) / den;
    Some((ax + r1 * t, ay + r2 * t))
}

/// Tangency tolerance, in millimetres.
///
/// The solver drives a tangency to its own precision rather than to an absolute zero, so "does it touch"
/// has to be decided in units of length. The value matches the one used to stitch coincident points
/// (1e-3 mm): if two points that close count as one, a tangency that close counts as a tangency. It is
/// orders of magnitude below any design intent, so it produces no false cuts.
const TOUCH_TOL: f64 = 1e-3;

/// Roots of the intersection of the line `a-b` with a circle, treating tangency honestly.
///
/// Tangency must not be decided by "discriminant is near zero": the scale of the discriminant depends on
/// the segment length and the radius, so a single threshold there either catches too much or misses the
/// real thing. Measured case: a line 1.6e-10 mm from the rim is geometrically tangent, yet the
/// discriminant came out at -2.9e-6 against a 1e-12 threshold, so no cut was made and three lines tangent
/// to a circle formed no closed region to extrude. Distance is measured instead: when the point of the
/// line nearest the centre differs from the radius by less than the tolerance, there is one tangency
/// point.
fn line_circle_roots(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64, r: f64) -> Vec<f64> {
    let (dx, dy) = (bx - ax, by - ay);
    let (fx, fy) = (ax - cx, ay - cy);
    let aa = dx * dx + dy * dy;
    if aa < 1e-12 {
        return Vec::new();
    }
    let bb = 2.0 * (fx * dx + fy * dy);
    let cc = fx * fx + fy * fy - r * r;
    // The point of the line nearest the centre, and its deviation from the rim.
    let t_near = -bb / (2.0 * aa);
    let (px, py) = (ax + dx * t_near, ay + dy * t_near);
    let gap = ((px - cx).hypot(py - cy) - r).abs();
    let tol = TOUCH_TOL * r.abs().max(1.0); // tolerance in units of length (mm), not in units of the discriminant
    if gap <= tol {
        return vec![t_near];
    }
    let disc = bb * bb - 4.0 * aa * cc;
    if disc <= 0.0 {
        return Vec::new();
    }
    let sq = disc.sqrt();
    vec![(-bb - sq) / (2.0 * aa), (-bb + sq) / (2.0 * aa)]
}

/// Parameters t in (0,1) along segment `a-b` where it crosses the circle (cx, cy, r).
fn seg_circle_t(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64, r: f64) -> Vec<f64> {
    // Endpoints are included: an intersection exactly at a vertex (where the first trim already left a
    // point) is a valid cut, otherwise trimming again at the same point finds nothing.
    //
    // The endpoint tolerance is geometric rather than a fixed 1e-9 in the parameter. Measured: a tangent
    // line 2e-7 mm from the rim put the tangency point 5e-7 of the parameter before the start of the
    // segment, the cut was discarded, and three tangent lines formed no closed region to extrude. A
    // parameter threshold unrelated to length is the same mistake as measuring tangency by a zero
    // discriminant: it is stricter than a millimetre on a long segment and looser on a short one.
    let len = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt().max(1e-12);
    let te = (TOUCH_TOL / len).max(1e-9); // mm converted into a fraction of the segment length
    line_circle_roots(ax, ay, bx, by, cx, cy, r).into_iter().filter(|&t| t > -te && t < 1.0 + te).collect()
}

/// Parameter t along the infinite line `a-b` where it crosses the segment `c-d`.
fn line_seg_t(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<f64> {
    let ([ax, ay], [bx, by], [cx, cy], [dx, dy]) = (a, b, c, d);
    let (rx, ry) = (bx - ax, by - ay);
    let (sx, sy) = (dx - cx, dy - cy);
    let den = rx * sy - ry * sx;
    if den.abs() < 1e-9 {
        return None;
    }
    let t = ((cx - ax) * sy - (cy - ay) * sx) / den;
    let u = ((cx - ax) * ry - (cy - ay) * rx) / den;
    if (-1e-9..=1.0 + 1e-9).contains(&u) {
        Some(t)
    } else {
        None
    }
}

/// Parameters t along the infinite line `a-b` where it crosses a circle.
fn line_circle_t(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64, r: f64) -> Vec<f64> {
    line_circle_roots(ax, ay, bx, by, cx, cy, r)
}

/// Semi-axes of an ellipse from its centre and axis endpoints. The major axis is `c` to `ma` (length
/// `major`), the minor axis is perpendicular to it (length |c-mi|), matching `ellipse_contour`. Returns
/// the unit vector of the major axis (ux, uy) together with `major` and `minor`.
fn ellipse_axes(cx: f64, cy: f64, max: f64, may: f64, mix: f64, miy: f64) -> (f64, f64, f64, f64) {
    let major = ((max - cx).powi(2) + (may - cy).powi(2)).sqrt().max(1e-9);
    let minor = ((mix - cx).powi(2) + (miy - cy).powi(2)).sqrt().max(1e-9);
    let (ux, uy) = ((max - cx) / major, (may - cy) / major);
    (ux, uy, major, minor)
}

/// Parameters t along the infinite line `a-b` where it crosses an ellipse with centre (cx, cy), major-axis
/// unit vector (ux, uy) and semi-axes `major` and `minor` (the minor axis is perpendicular to the major
/// one). In local normalised coordinates the ellipse is a unit circle, which makes this a quadratic. Zero
/// to two roots; filtering by t is left to the caller.
fn line_ellipse_roots(a: [f64; 2], b: [f64; 2], c: [f64; 2], u: [f64; 2], major: f64, minor: f64) -> Vec<f64> {
    let ([ax, ay], [bx, by], [cx, cy], [ux, uy]) = (a, b, c, u);
    let (vx, vy) = (-uy, ux); // unit vector of the minor axis
    let (dx, dy) = (bx - ax, by - ay);
    let (acx, acy) = (ax - cx, ay - cy);
    let xa = (acx * ux + acy * uy) / major;
    let xd = (dx * ux + dy * uy) / major;
    let ya = (acx * vx + acy * vy) / minor;
    let yd = (dx * vx + dy * vy) / minor;
    let aa = xd * xd + yd * yd;
    let bb = 2.0 * (xa * xd + ya * yd);
    let cc = xa * xa + ya * ya - 1.0;
    let disc = bb * bb - 4.0 * aa * cc;
    if disc < 0.0 || aa < 1e-18 {
        return Vec::new();
    }
    let sq = disc.sqrt();
    vec![(-bb - sq) / (2.0 * aa), (-bb + sq) / (2.0 * aa)]
}

/// Parameters t along the segment `a-b`, within [0,1] plus an endpoint tolerance, where it crosses an
/// ellipse.
fn seg_ellipse_t(a: [f64; 2], b: [f64; 2], c: [f64; 2], u: [f64; 2], major: f64, minor: f64) -> Vec<f64> {
    line_ellipse_roots(a, b, c, u, major, minor).into_iter().filter(|&t| t > -1e-9 && t < 1.0 + 1e-9).collect()
}

/// Intersection points of a circle (cx, cy, r) with an ellipse (centre ce, major-axis unit vector u,
/// semi-axes `major` and `minor`).
///
/// There is no closed form (the equation is a quartic), so the roots of f(t) = |E(t) - c|^2 - r^2 are
/// found numerically over the ellipse parameter, refining each sign-change interval by bisection. Up to
/// four points.
fn circle_ellipse_pts(c: [f64; 2], r: f64, ce: [f64; 2], u: [f64; 2], major: f64, minor: f64) -> Vec<(f64, f64)> {
    let ([cx, cy], [cex, cey], [ux, uy]) = (c, ce, u);
    use std::f64::consts::TAU;
    let (vx, vy) = (-uy, ux);
    let e = |th: f64| {
        let (ct, st) = (th.cos(), th.sin());
        (cex + major * ct * ux + minor * st * vx, cey + major * ct * uy + minor * st * vy)
    };
    let f = |th: f64| {
        let (ex, ey) = e(th);
        (ex - cx).powi(2) + (ey - cy).powi(2) - r * r
    };
    let n = 256;
    let mut out: Vec<(f64, f64)> = Vec::new();
    let mut prev_th = 0.0_f64;
    let mut prev = f(0.0);
    for k in 1..=n {
        let th = TAU * k as f64 / n as f64;
        let cur = f(th);
        if prev == 0.0 {
            out.push(e(prev_th));
        } else if prev * cur < 0.0 {
            let (mut lo, mut hi, mut flo) = (prev_th, th, prev);
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                let fm = f(mid);
                if flo * fm <= 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                    flo = fm;
                }
            }
            out.push(e(0.5 * (lo + hi)));
        }
        prev_th = th;
        prev = cur;
    }
    out
}

/// Intersection points of two circles (cx1, cy1, r1) and (cx2, cy2, r2): zero, one or two points.
fn circle_circle_pts(cx1: f64, cy1: f64, r1: f64, cx2: f64, cy2: f64, r2: f64) -> Vec<(f64, f64)> {
    let (dx, dy) = (cx2 - cx1, cy2 - cy1);
    let d = (dx * dx + dy * dy).sqrt();
    if d < 1e-9 || d > r1 + r2 + 1e-9 || d < (r1 - r2).abs() - 1e-9 {
        return Vec::new();
    }
    let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h = (r1 * r1 - a * a).max(0.0).sqrt();
    let (mx, my) = (cx1 + a * dx / d, cy1 + a * dy / d);
    let (ox, oy) = (-dy / d * h, dx / d * h);
    if h < 1e-9 {
        vec![(mx, my)]
    } else {
        vec![(mx + ox, my + oy), (mx - ox, my - oy)]
    }
}

/// Whether angle `ang` lies on the arc from `a0` to `a1` (`ccw` means counter-clockwise).
///
/// The test is relative to the start, through the swept angle, so it is stable across the wrap at 0 and
/// 2*pi: an intersection exactly at the end of the arc, where floating-point error can push the angle
/// past zero, still counts. Boundary tolerance is 1e-6.
fn angle_in_arc(ang: f64, a0: f64, a1: f64, ccw: bool) -> bool {
    let tau = std::f64::consts::TAU;
    let (s, e) = if ccw { (a0, a1) } else { (a1, a0) };
    let sweep = (e - s).rem_euclid(tau); // arc length along the traversal direction, [0, 2*pi)
    let rel = (ang - s).rem_euclid(tau); // position of the angle from the start, [0, 2*pi)
    rel <= sweep + 1e-6 || rel >= tau - 1e-6 // within the swept angle, or right at the start (the wrap)
}

impl Constraint {
    /// Expression of a dimensional constraint, when it has one; used to rebuild selectively by parameter
    /// name.
    pub fn expr(&self) -> Option<&str> {
        match self {
            Constraint::Distance { expr, .. }
            | Constraint::DistancePL { expr, .. }
            | Constraint::Diameter { expr, .. }
            | Constraint::EdgeDistance { expr, .. }
            | Constraint::Angle { expr, .. }
            | Constraint::AngleLines { expr, .. }
            | Constraint::ArcLength { expr, .. } => Some(expr).filter(|e| !e.is_empty()).map(|e| e.as_str()),
            _ => None,
        }
    }

    /// Mutable access to a dimension expression. Renaming a driver needs it: sketch formulas reference
    /// names just as project parameters and feature parameters do, and skipping them would leave the
    /// document holding references to a name that no longer exists.
    pub fn expr_mut(&mut self) -> Option<&mut String> {
        match self {
            Constraint::Distance { expr, .. }
            | Constraint::DistancePL { expr, .. }
            | Constraint::Diameter { expr, .. }
            | Constraint::EdgeDistance { expr, .. }
            | Constraint::Angle { expr, .. }
            | Constraint::AngleLines { expr, .. }
            | Constraint::ArcLength { expr, .. } => Some(expr),
            _ => None,
        }
    }
}

/// Whether a constraint references any of the lines being deleted (by the unordered pair of endpoints).
///
/// Line-dependent constraints (horizontal, vertical, distance, parallel, perpendicular, equal, collinear,
/// tangent, midpoint) lose their meaning without their line and have to go with it.
fn constraint_uses_line(c: &Constraint, lines: &[(Id, Id)]) -> bool {
    let same = |x: Id, y: Id| lines.iter().any(|&(a, b)| (a == x && b == y) || (a == y && b == x));
    match *c {
        Constraint::Horizontal { a, b } | Constraint::Vertical { a, b } | Constraint::Distance { a, b, .. } => same(a, b),
        Constraint::Parallel { a, b, c, d } | Constraint::Perpendicular { a, b, c, d } | Constraint::Equal { a, b, c, d } | Constraint::Collinear { a, b, c, d } => same(a, b) || same(c, d),
        Constraint::Tangent { a, b, .. } => same(a, b),
        Constraint::Midpoint { a, b, .. } => same(a, b),
        Constraint::PointOnLine { a, b, .. } => same(a, b),
        Constraint::DistancePL { a, b, .. } => same(a, b),
        Constraint::AngleLines { a, b, c, d, .. } => same(a, b) || same(c, d),
        _ => false,
    }
}

/// Every point id a constraint references. The single source of truth for validating and pruning orphaned
/// constraints and for moving references when a fillet or a chamfer is applied.
pub fn constraint_point_ids(c: &Constraint) -> Vec<Id> {
    match *c {
        Constraint::Fixed { p } => vec![p],
        Constraint::Horizontal { a, b } | Constraint::Vertical { a, b } | Constraint::Orientation { a, b, .. } | Constraint::Coincident { a, b } | Constraint::Distance { a, b, .. } => vec![a, b],
        Constraint::Parallel { a, b, c, d } | Constraint::Perpendicular { a, b, c, d } | Constraint::Equal { a, b, c, d } | Constraint::Collinear { a, b, c, d } => vec![a, b, c, d],
        Constraint::Angle { a, b, c, .. } => vec![a, b, c],
        Constraint::Midpoint { p, a, b } => vec![p, a, b],
        Constraint::Tangent { a, b, c, .. } => vec![a, b, c],
        Constraint::Symmetric { a, b, la, lb } => vec![a, b, la, lb],
        Constraint::PointOnLine { p, a, b } => vec![p, a, b],
        Constraint::DistancePL { p, a, b, .. } => vec![p, a, b],
        Constraint::EdgeDistance { c1, c2, .. } => vec![c1, c2],
        Constraint::AngleLines { a, b, c, d, .. } => vec![a, b, c, d],
        Constraint::Diameter { c, .. } => vec![c],
        Constraint::EqualRadius { c1, c2 } => vec![c1, c2],
        Constraint::CircleTangent { c1, c2, .. } => vec![c1, c2],
        Constraint::PointOnCircle { p, c } => vec![p, c],
        Constraint::Concentric { c1, c2 } => vec![c1, c2],
        Constraint::ArcLength { c, a, b, .. } => vec![c, a, b],
    }
}

/// Replace a reference to point `from` with `to` in every point field of a constraint: the mutating
/// counterpart of [`constraint_point_ids`].
///
/// Needed by fillets and chamfers, where the corner point disappears and constraints that referenced it
/// move to the tangency point. Non-point fields (radii, expressions) are left alone.
/// The points an entity stands on.
pub fn entity_points(e: &SketchEntity) -> Vec<Id> {
    match e.kind {
        EntityKind::Line { a, b } => vec![a, b],
        EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
        EntityKind::Circle { center, .. } => vec![center],
        EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
    }
}

/// Re-point one entity from one point to another.
pub fn remap_entity_point(e: &mut SketchEntity, from: Id, to: Id) {
    let fix = |id: &mut Id| {
        if *id == from {
            *id = to;
        }
    };
    match &mut e.kind {
        EntityKind::Line { a, b } => {
            fix(a);
            fix(b);
        }
        EntityKind::Arc { center, a, b, .. } => {
            fix(center);
            fix(a);
            fix(b);
        }
        EntityKind::Circle { center, .. } => fix(center),
        EntityKind::Ellipse { c, ma, mi } => {
            fix(c);
            fix(ma);
            fix(mi);
        }
    }
}

/// The same as `remap_constraint_point`, reachable from the sketch module.
pub fn remap_constraint_point_pub(c: &mut Constraint, from: Id, to: Id) {
    remap_constraint_point(c, from, to);
}

fn remap_constraint_point(c: &mut Constraint, from: Id, to: Id) {
    let fix = |id: &mut Id| {
        if *id == from {
            *id = to;
        }
    };
    match c {
        Constraint::Fixed { p } => fix(p),
        Constraint::Horizontal { a, b } | Constraint::Vertical { a, b } | Constraint::Orientation { a, b, .. } | Constraint::Coincident { a, b } | Constraint::Distance { a, b, .. } => {
            fix(a);
            fix(b);
        }
        Constraint::Parallel { a, b, c, d }
        | Constraint::Perpendicular { a, b, c, d }
        | Constraint::Equal { a, b, c, d }
        | Constraint::Collinear { a, b, c, d }
        | Constraint::AngleLines { a, b, c, d, .. } => {
            fix(a);
            fix(b);
            fix(c);
            fix(d);
        }
        Constraint::Angle { a, b, c, .. } | Constraint::Tangent { a, b, c, .. } => {
            fix(a);
            fix(b);
            fix(c);
        }
        Constraint::Midpoint { p, a, b } | Constraint::PointOnLine { p, a, b } | Constraint::DistancePL { p, a, b, .. } => {
            fix(p);
            fix(a);
            fix(b);
        }
        Constraint::Symmetric { a, b, la, lb } => {
            fix(a);
            fix(b);
            fix(la);
            fix(lb);
        }
        Constraint::EdgeDistance { c1, c2, .. } | Constraint::EqualRadius { c1, c2 } | Constraint::CircleTangent { c1, c2, .. } | Constraint::Concentric { c1, c2 } => {
            fix(c1);
            fix(c2);
        }
        Constraint::Diameter { c, .. } => fix(c),
        Constraint::PointOnCircle { p, c } => {
            fix(p);
            fix(c);
        }
        Constraint::ArcLength { c, a, b, .. } => {
            fix(c);
            fix(a);
            fix(b);
        }
    }
}

/// Clone of a constraint with every point reference translated through `map` (old id to new id).
///
/// Points missing from `map` are left as they are; for constraints internal to a copy there are none.
/// Fresh ids from `map` cannot collide with the old ones, so remapping in sequence is safe.
fn remap_constraint_via(c: &Constraint, map: &std::collections::HashMap<Id, Id>) -> Constraint {
    let mut nc = c.clone();
    for old in constraint_point_ids(c) {
        if let Some(&new) = map.get(&old) {
            remap_constraint_point(&mut nc, old, new);
        }
    }
    nc
}

/// THE PALETTE OF A BODY'S COLOURS, given one per triangle, and the place of each triangle's colour in it. The places are
/// bytes, so a palette holds 255. Up to that the distinct colours stand in the order they come. Past it - a scan, whose
/// colours run to thousands - 255 are cut out of them by median, the colours weighed by how many triangles have them,
/// and every colour takes the nearest; the first 255 met would leave a colour met later with nothing like it.
pub fn palette_of(colours: &[[u8; 3]]) -> (Vec<[u8; 3]>, Vec<u8>) {
    let mut place: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
    let mut distinct: Vec<([u8; 3], u64)> = Vec::new();
    for c in colours {
        let k = *place.entry(*c).or_insert_with(|| {
            distinct.push((*c, 0));
            distinct.len() - 1
        });
        distinct[k].1 += 1;
    }
    if distinct.len() <= 255 {
        return (distinct.iter().map(|(c, _)| *c).collect(), colours.iter().map(|c| place[c] as u8).collect());
    }
    // split the box whose colours spread widest along a channel, at the median of the triangles, until there are 255
    let spread = |b: &[([u8; 3], u64)], ch: usize| b.iter().map(|(c, _)| c[ch]).max().unwrap_or(0) - b.iter().map(|(c, _)| c[ch]).min().unwrap_or(0);
    let mut boxes = vec![distinct];
    while boxes.len() < 255 {
        let Some((bi, ch, _)) = boxes.iter().enumerate().filter(|(_, b)| b.len() > 1).flat_map(|(i, b)| (0..3).map(move |ch| (i, ch, spread(b, ch)))).max_by_key(|x| x.2) else { break };
        let mut b = boxes.swap_remove(bi);
        b.sort_unstable_by_key(|(c, _)| c[ch]);
        let total: u64 = b.iter().map(|(_, n)| n).sum();
        let mut acc = 0;
        let cut = b
            .iter()
            .position(|(_, n)| {
                acc += n;
                acc * 2 >= total
            })
            .map_or(1, |k| k + 1)
            .clamp(1, b.len() - 1);
        let rest = b.split_off(cut);
        boxes.push(b);
        boxes.push(rest);
    }
    let palette: Vec<[u8; 3]> = boxes
        .iter()
        .map(|b| {
            let total: u64 = b.iter().map(|(_, n)| n).sum();
            std::array::from_fn(|ch| ((b.iter().map(|(c, n)| u64::from(c[ch]) * n).sum::<u64>() + total / 2) / total) as u8)
        })
        .collect();
    let near = |c: &[u8; 3]| (0..palette.len()).min_by_key(|&k| (0..3).map(|i| u32::from(palette[k][i].abs_diff(c[i])).pow(2)).sum::<u32>()).unwrap_or(0) as u8;
    let nearest: std::collections::HashMap<[u8; 3], u8> = place.keys().map(|c| (*c, near(c))).collect();
    let places = colours.iter().map(|c| nearest[c]).collect();
    (palette, places)
}

/// Default part colour from the palette, by index, so an assembly of many bodies is not monochrome. Soft,
/// distinguishable tones; the palette wraps around.
pub fn default_part_color(index: usize) -> [u8; 3] {
    const PALETTE: [[u8; 3]; 8] = [
        [150, 170, 190], // steel
        [200, 160, 110], // brass
        [140, 180, 150], // green
        [180, 150, 175], // lilac
        [170, 175, 120], // olive
        [130, 165, 195], // light blue
        [195, 145, 135], // terracotta
        [160, 160, 170], // grey
    ];
    PALETTE[index % PALETTE.len()]
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
    /// Parametric definition: when it is not `Manual`, `origin` and `normal` are resolved during
    /// regenerate.
    #[serde(default)]
    pub def: PlaneDef,
}

impl Default for WorkPlane {
    fn default() -> Self {
        Self { id: 0, name: "name-plane".into(), origin: [0.0; 3], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, def: PlaneDef::Manual }
    }
}

impl WorkPlane {
    /// Build a plane from a recognised mesh face.
    pub fn from_face(name: impl Into<String>, face: &crate::geom::MeshFace) -> Self {
        Self { id: 0, name: name.into(), origin: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, rot_deg: 0.0, def: PlaneDef::Manual }
    }
}

/// Edge-name translation map for a single rebuild pass: body to "old number to new name".
///
/// Edge names are inherited down the timeline: a resulting body carries the edges of its inputs under the
/// same names. The map of a body therefore travels into its descendants, so any reference saved under the
/// previous scheme (a chamfer, a fillet) is translated exactly, through a known mapping, instead of being
/// looked up as a similar edge.
type EdgeRenames = std::collections::HashMap<Id, std::collections::HashMap<u32, u32>>;

mod assembly;
pub use assembly::{ExportMesh, ExportNode, FileGroup, ImportNode};
pub mod contours;
mod regen;
pub use regen::{ArrayAxis, BodyOp, ChamferShape, CombineSpan, ExtrudeSpan, HoleTool, RevolveAxis, RevolveTurn};
mod tess;
mod timeline;
mod sketch;
mod drag;
mod round;
pub use sketch::{ChamferLegs, CornerAt, CornerBlend, CornerCut, CornerTool, ExtendAsk, ExtendSides, ExtendStop, ExtendStops, FilletBy, FilletSize, LineExtension, TextSpec};
pub(crate) mod comp_pattern;
pub use comp_pattern::{CompPattern, CompPatternKind};
mod projection;

impl Project {
    /// Hand out a new stable id.
    pub fn alloc_id(&mut self) -> Id {
        self.next_id += 1;
        self.next_id
    }

    // --- Contours. Mutations happen only here, so the parallel id array cannot drift out of step. ---

    /// Add a set of contours (a DXF or SVG import, for example).
    pub fn add_contours(&mut self, cs: impl IntoIterator<Item = Contour>) {
        for c in cs {
            self.add_contour(c);
        }
    }

    // --- Typed sketch: points plus entities. ---

    // --- Editing and duplicating sketch entities, by selected entity ids. ---

    /// Mirror the entities (as a copy) about the line (ax,ay)-(bx,by).
    pub fn mirror_entities(&mut self, si: usize, eids: &[Id], ax: f64, ay: f64, bx: f64, by: f64) {
        let (dx, dy) = (bx - ax, by - ay);
        let len2 = dx * dx + dy * dy;
        if len2 < 1e-12 {
            return;
        }
        self.dup_entities(
            si,
            eids,
            |x, y| {
                let (vx, vy) = (x - ax, y - ay);
                let t = (vx * dx + vy * dy) / len2;
                let (projx, projy) = (dx * t, dy * t);
                let (perpx, perpy) = (vx - projx, vy - projy);
                (x - 2.0 * perpx, y - 2.0 * perpy)
            },
            false,
        );
    }

    /// Duplicate the selected entities with an offset of (dx, dy) and return the ids of the copies. Used by
    /// the interactive copy command, which reads as base point then target.
    pub fn copy_entities(&mut self, si: usize, eids: &[Id], dx: f64, dy: f64) -> Vec<Id> {
        self.dup_entities(si, eids, |x, y| (x + dx, y + dy), true) // a copy carries its internal constraints and dimensions
    }

    /// Linear array: `count` copies spaced by (dx, dy).
    pub fn array_linear(&mut self, si: usize, eids: &[Id], dx: f64, dy: f64, count: u32) {
        for k in 1..count.max(1) {
            let (ox, oy) = (dx * k as f64, dy * k as f64);
            self.dup_entities_unbuilt(si, eids, |x, y| (x + ox, y + oy), false);
        }
        self.regen_sketch(si); // the loops once, for every copy
    }

    /// Circular array: `count` copies around (cx, cy) spanning `total_deg` in total.
    pub fn array_circular(&mut self, si: usize, eids: &[Id], cx: f64, cy: f64, count: u32, total_deg: f64) {
        let count = count.max(1);
        // A full circle (360 degrees) spreads `count` copies around it; a sector holds `count` copies within it.
        let step = if (total_deg - 360.0).abs() < 1e-6 { total_deg / count as f64 } else { total_deg / count.max(2) as f64 };
        for k in 1..count {
            let ang = (step * k as f64).to_radians();
            let (s_, c_) = (ang.sin(), ang.cos());
            self.dup_entities_unbuilt(
                si,
                eids,
                move |x, y| {
                    let (vx, vy) = (x - cx, y - cy);
                    (cx + vx * c_ - vy * s_, cy + vx * s_ + vy * c_)
                },
                false,
            );
        }
        self.regen_sketch(si); // the loops once, for every copy
    }

    /// Create the pattern instances without recording the pattern. Returns the ids of the new entities.
    fn pattern_instances(&mut self, si: usize, source: &[Id], kind: PatternKind) -> Vec<Id> {
        let mut out = Vec::new();
        match kind {
            PatternKind::Linear { dx, dy, count, dx2, dy2, count2 } => {
                // Grid: i along (dx,dy), j along (dx2,dy2). (0,0) is the original and is not duplicated.
                for i in 0..count.max(1) {
                    for j in 0..count2.max(1) {
                        if i == 0 && j == 0 {
                            continue;
                        }
                        let (ox, oy) = (dx * i as f64 + dx2 * j as f64, dy * i as f64 + dy2 * j as f64);
                        out.extend(self.dup_entities_unbuilt(si, source, |x, y| (x + ox, y + oy), false));
                    }
                }
            }
            PatternKind::Circular { cx, cy, count, total_deg } => {
                let count = count.max(1);
                let step = if (total_deg - 360.0).abs() < 1e-6 { total_deg / count as f64 } else { total_deg / count.max(2) as f64 };
                for k in 1..count {
                    let ang = (step * k as f64).to_radians();
                    let (s_, c_) = (ang.sin(), ang.cos());
                    out.extend(self.dup_entities_unbuilt(
                        si,
                        source,
                        move |x, y| {
                            let (vx, vy) = (x - cx, y - cy);
                            (cx + vx * c_ - vy * s_, cy + vx * s_ + vy * c_)
                        },
                        false,
                    ));
                }
            }
        }
        out
    }

    /// Fillet the corner at vertex `pid`, when exactly two edges meet there. The inner side is chosen by
    /// the bisector of the chords. Used both by click-on-corner and by the chain command. Returns whether
    /// it succeeded.
    /// THE LARGEST VALUE A CORNER OF TWO LINES TAKES for `tool`: a fillet's point of touching lies r / tan(theta / 2) from
    /// the corner (theta the angle between the lines), a chamfer's leg along the line, and the cut of a symmetric chamfer
    /// is 2 sin(theta / 2) times its leg - none may reach the far end of the shorter line. `None` for a corner not made of two lines. The field of the tool refuses a value past it
    /// in words, where the corner used to take it and do nothing, or cut it down without a word.
    pub fn corner_limit(&self, si: usize, pid: Id, tool: CornerTool) -> Option<f64> {
        // Where more than two edges meet, there is a corner for every pair, and the field must refuse a value that
        // is too big for AT LEAST ONE of them: the tightest corner is the one that bounds it.
        self.vertex_pairs(si, pid).iter().filter_map(|pair| self.corner_limit_of_pair(si, pid, *pair, tool)).fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))))
    }
    /// The bound of the one corner the point `(x, y)` stands in, where more than two edges meet at `pid`.
    pub fn corner_limit_near(&self, si: usize, pid: Id, tool: CornerTool, x: f64, y: f64) -> Option<f64> {
        self.vertex_pair(si, pid, Some((x, y))).and_then(|pair| self.corner_limit_of_pair(si, pid, pair, tool))
    }
    /// The bound of one named pair of edges at `pid`.
    pub fn corner_limit_of_pair(&self, si: usize, pid: Id, (e1, e2): (Id, Id), tool: CornerTool) -> Option<f64> {
        let (pcx, pcy) = self.point_xy(si, pid)?;
        let mut dirs = Vec::new();
        for e in [e1, e2] {
            let (a, b) = self.line_ends(si, e)?;
            let other = if a == pid { b } else { a };
            let (ox, oy) = self.point_xy(si, other)?;
            let l = (ox - pcx).hypot(oy - pcy);
            if l < 1e-9 {
                return None;
            }
            dirs.push(((ox - pcx) / l, (oy - pcy) / l, l));
        }
        if dirs.len() != 2 {
            return None;
        }
        let shorter = dirs[0].2.min(dirs[1].2);
        let theta = (dirs[0].0 * dirs[1].0 + dirs[0].1 * dirs[1].1).clamp(-1.0, 1.0).acos();
        Some(match tool {
            CornerTool::Fillet => shorter * (theta / 2.0).tan(),
            CornerTool::Chamfer(crate::feature::ChamferMode::Symmetric) => shorter * 2.0 * (theta / 2.0).sin(),
            CornerTool::Chamfer(_) => shorter,
        })
    }

    /// THE LARGEST VALUE `tool` TAKES on the corners of a set of lines at once (every line of the sketch with `None`): a
    /// line between two corners being cut spends both on itself - for a fillet r / tan(a / 2) + r / tan(b / 2) of its
    /// length - and a line with one corner cut spends one. `None` when there is no corner of two lines to cut. A
    /// rectangle 40 x 30 takes a radius up to 15, half its short side, where one corner alone takes 30.
    pub fn all_corners_limit(&self, si: usize, only: Option<&std::collections::HashSet<Id>>, tool: CornerTool) -> Option<f64> {
        let s = self.sketches.get(si)?;
        let lines: Vec<(Id, Id, Id)> = s
            .entities
            .iter()
            .filter(|e| only.is_none_or(|o| o.contains(&e.id)))
            .filter_map(|e| match e.kind {
                EntityKind::Line { a, b } => Some((e.id, a, b)),
                _ => None,
            })
            .collect();
        // what one unit of the value spends along each line at every corner of two lines - the cotangent of half the angle for
        // a fillet - the corners being the points two lines of the set meet at
        let mut cot: std::collections::HashMap<Id, f64> = std::collections::HashMap::new();
        for &(_, a, b) in &lines {
            for p in [a, b] {
                if cot.contains_key(&p) || lines.iter().filter(|(_, x, y)| *x == p || *y == p).count() != 2 || self.vertex_edges(si, p).len() != 2 {
                    continue;
                }
                if let Some(l) = self.corner_limit(si, p, tool) {
                    let (pcx, pcy) = self.point_xy(si, p)?;
                    let ends: Vec<f64> = self
                        .vertex_edges(si, p)
                        .iter()
                        .filter_map(|e| self.line_ends(si, *e))
                        .filter_map(|(x, y)| self.point_xy(si, if x == p { y } else { x }))
                        .map(|(ox, oy)| (ox - pcx).hypot(oy - pcy))
                        .collect();
                    let shorter = ends.iter().copied().fold(f64::INFINITY, f64::min);
                    cot.insert(p, shorter / l); // corner_limit = shorter / (what one unit spends)
                }
            }
        }
        lines
            .iter()
            .filter_map(|&(_, a, b)| {
                let spent = cot.get(&a).copied().unwrap_or(0.0) + cot.get(&b).copied().unwrap_or(0.0);
                let (ax, ay) = self.point_xy(si, a)?;
                let (bx, by) = self.point_xy(si, b)?;
                (spent > 1e-12).then(|| (bx - ax).hypot(by - ay) / spent)
            })
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))))
    }

    pub fn fillet_at_vertex(&mut self, si: usize, pid: Id, r: f64) -> bool {
        self.fillet_at_vertex_by(si, pid, FilletSize::radius(r))
    }

    /// HOW FAR THE ARC OF A FILLET TURNS on the corner at `pid`: pi minus the angle between its two edges, each taken
    /// from the corner to its far end (exact for lines, the chord of an arc for an arc). `None` without a corner of two
    /// edges.
    pub fn corner_sweep(&self, si: usize, pid: Id) -> Option<f64> {
        let edges = self.vertex_edges(si, pid);
        if edges.len() != 2 {
            return None;
        }
        self.corner_sweep_of_pair(si, pid, (edges[0], edges[1]))
    }

    /// The same for the corner the two named edges make at `pid`, where more than two edges may meet.
    pub fn corner_sweep_of_pair(&self, si: usize, pid: Id, (e1, e2): (Id, Id)) -> Option<f64> {
        let (pcx, pcy) = self.point_xy(si, pid)?;
        let mut dirs = Vec::new();
        for e in [e1, e2] {
            let (a, b) = self.edge_end_ids(si, e)?;
            let (ox, oy) = self.point_xy(si, if a == pid { b } else { a })?;
            let l = (ox - pcx).hypot(oy - pcy);
            if l < 1e-9 {
                return None;
            }
            dirs.push(((ox - pcx) / l, (oy - pcy) / l));
        }
        let corner = (dirs[0].0 * dirs[1].0 + dirs[0].1 * dirs[1].1).clamp(-1.0, 1.0).acos();
        Some(std::f64::consts::PI - corner)
    }

    /// FILLET THE CORNER AT `pid` WITH A SIZE GIVEN BY ITS RADIUS, ITS CHORD OR ITS ARC LENGTH. The radius the size
    /// makes on this corner is laid, and the fillet keeps the size as it was given: a chord as the distance between
    /// the two points of touching, an arc length as the length of the arc - so a dimension changed afterwards moves the
    /// fillet. A chord or an arc the corner cannot take is refused, where a radius is pressed to the corner as before.
    pub fn fillet_at_vertex_by(&mut self, si: usize, pid: Id, size: FilletSize) -> bool {
        let r = match size.by {
            FilletBy::Radius => size.value,
            _ => {
                let Some(r) = self.corner_sweep(si, pid).and_then(|sweep| size.radius_on(sweep)) else { return false };
                if self.corner_limit(si, pid, CornerTool::Fillet).is_some_and(|l| r >= l * (1.0 - 1e-9)) {
                    return false;
                }
                r
            }
        };
        if !self.fillet_round(si, pid, r) {
            return false;
        }
        if size.by != FilletBy::Radius {
            self.give_fillet_its_size(si, size);
        }
        true
    }

    /// CUT EVERY CORNER OF A SET WITH ONE ANSWER, OR NONE OF THEM: a fillet of the size on every corner - a chord or an
    /// arc length makes its own radius on each and is kept as it was given - or a chamfer of the legs, its first line
    /// the one nearer `toward`. A corner the cut does not fit leaves the sketch as it was and answers 0: cut on the
    /// corners that took it and left whole on the one that did not, the set was a half-made drawing nobody asked for.
    /// Answers how many corners were cut.
    pub fn cut_corner_set(&mut self, si: usize, corners: &[CornerAt], cut: CornerCut, toward: Option<crate::geom::Point2>) -> usize {
        // every corner laid, the sketch rebuilt once (`batch`)
        self.batch(|p| p.cut_corner_set_unbuilt(si, corners, cut, toward))
    }

    fn cut_corner_set_unbuilt(&mut self, si: usize, corners: &[CornerAt], cut: CornerCut, toward: Option<crate::geom::Point2>) -> usize {
        let Some(before) = self.sketches.get(si).cloned() else { return 0 };
        for corner in corners {
            let done = match cut {
                CornerCut::Chamfer(legs) => self.chamfer_lines_of_pair(si, corner.pair, legs, toward),
                CornerCut::Fillet(size) if size.by == FilletBy::Radius => self.fillet_at_pair(si, corner.pair, size.value),
                CornerCut::Fillet(size) => self.fillet_pair_by(si, *corner, size),
            };
            if !done {
                self.sketches[si] = before;
                self.regen_sketch(si);
                return 0;
            }
        }
        corners.len()
    }

    /// A FILLET OF THE CORNER OF A SET BY ITS CHORD OR ITS ARC LENGTH: the radius the size makes on this corner, as the
    /// preview of the corner draws it, and the size kept as it was given.
    fn fillet_pair_by(&mut self, si: usize, corner: CornerAt, size: FilletSize) -> bool {
        let Some((_, r)) = self.corner_blend(si, corner.point, corner.pair, CornerCut::Fillet(size)).and_then(|b| b.arc) else { return false };
        if !self.fillet_at_pair(si, corner.pair, r) {
            return false;
        }
        self.give_fillet_its_size(si, size);
        true
    }

    /// THE LAST FILLET LAID KEEPS ITS SIZE AS IT WAS GIVEN: its radius dimension is put in place of a distance between
    /// the two points of touching (a chord) or of an arc length dimension, and the sketch is solved.
    pub(super) fn give_fillet_its_size(&mut self, si: usize, size: FilletSize) {
        let Some(s) = self.sketches.get(si) else { return };
        let Some(last) = s.entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).max_by_key(|e| e.id).copied() else { return };
        let EntityKind::Arc { center: centre, a, b, ccw } = last.kind else { return };
        let Some(ci) = self.fillet_radius_constraint(si, last.id) else { return };
        let end = |id: Id| self.point_xy(si, id).map(|(x, y)| sketch::DimEnd { id, at: crate::geom::Point2::new(x, y) });
        let (Some(end_a), Some(end_b), Some((cx, cy))) = (end(a), end(b), self.point_xy(si, centre)) else { return };
        // THE CHORD STANDS OUTSIDE THE SHAPE, on the side of the arc away from its centre: written as the arc ran, it
        // stood inside the shape on one corner out of two
        let [ca, cb] = sketch::ends_facing([end_a, end_b], sketch::Side::AwayFrom(crate::geom::Point2::new(cx, cy)));
        self.sketches[si].constraints[ci] = match size.by {
            FilletBy::Chord => Constraint::Distance { a: ca, b: cb, d: size.value, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None },
            _ => Constraint::ArcLength { c: centre, a, b, ccw, len: size.value, off: 0.0, expr: String::new(), driven: false },
        };
        self.solve_sketch(si);
    }

    fn fillet_round(&mut self, si: usize, pid: Id, r: f64) -> bool {
        let Some(pair) = self.vertex_pair(si, pid, None) else { return false };
        self.fillet_at_pair(si, pair, r)
    }

    /// Fillet the corner at `pid` that the point `(x, y)` stands in, where more than two edges meet there.
    pub fn fillet_at_vertex_near(&mut self, si: usize, pid: Id, r: f64, x: f64, y: f64) -> bool {
        let Some(pair) = self.vertex_pair(si, pid, Some((x, y))) else { return false };
        self.fillet_at_pair(si, pair, r)
    }

    /// Fillet the corner the two named edges make at a shared vertex. The arc goes on the side of their bisector,
    /// which is what the fillet at a vertex has always done.
    pub fn fillet_at_pair(&mut self, si: usize, (e1, e2): (Id, Id), r: f64) -> bool {
        let Some(shared) = self.corner_of_pair(si, e1, e2) else { return false };
        let Some((pcx, pcy)) = self.point_xy(si, shared) else { return false };
        let dir = |me: &Self, eid: Id| -> Option<(f64, f64)> {
            let (a, b) = me.edge_end_ids(si, eid)?;
            let other = if a == shared { b } else { a };
            let (ox, oy) = me.point_xy(si, other)?;
            let (dx, dy) = (ox - pcx, oy - pcy);
            let l = (dx * dx + dy * dy).sqrt();
            (l > 1e-9).then(|| (dx / l, dy / l))
        };
        let (Some(d1), Some(d2)) = (dir(self, e1), dir(self, e2)) else { return false };
        let (bx, by) = (d1.0 + d2.0, d1.1 + d2.1);
        let bl = (bx * bx + by * by).sqrt();
        let near = if bl < 1e-6 { (pcx, pcy) } else { (pcx + bx / bl * r * 0.3, pcy + by / bl * r * 0.3) };
        self.fillet_curves(si, e1, e2, r, near.0, near.1)
    }

    /// Fillet every corner (each vertex with exactly two edges) with radius `r`, returning how many
    /// corners were filleted. The vertex list is snapshotted first, because filleting removes the old
    /// vertex.
    pub fn fillet_all_corners(&mut self, si: usize, r: f64) -> usize {
        self.fillet_all_corners_of(si, r, None)
    }

    /// Solve the sketch constraints (moving the points) and re-tessellate the contour.
    /// Map of global parameter names to values, used to evaluate feature and command dimension expressions
    /// through [`crate::expr::eval`], the same way sketch dimensions are evaluated. A name is a key as written: `H`
    /// and `h` are two parameters.
    pub fn param_map(&self) -> std::collections::HashMap<String, f64> {
        let mut m: std::collections::HashMap<String, f64> = self.parameters.iter().filter(|p| !p.name.is_empty()).map(|p| (p.name.clone(), p.value)).collect();
        // Named driving dimensions (the skeleton sketch of an assembly): a name resolves to a dimension
        // value, which parts then consume through expressions.
        for nd in &self.named_dims {
            if !nd.name.is_empty() {
                if let Some(v) = self.named_dim_value(nd) {
                    m.insert(nd.name.clone(), v);
                }
            }
        }
        m
    }

    /// Name a driver: either a sketch dimension or a feature parameter.
    ///
    /// One entry point for both cases, because the rules — the name is taken, an empty name clears the
    /// previous one, renaming in place — have to be identical or they drift apart.
    ///
    /// A taken name is refused rather than accepted as a second driver under the same name. A formula
    /// knows one name and the scope stores one value, so two dimensions called `len` are always a broken
    /// model in which one of them is unreachable and nothing says so. Measured: naming `len` in two
    /// sketches left only the last one (70.0) visible to formulas while the first (20.0) disappeared,
    /// showing up as two identical rows in the parameter list. Both halves of the fix apply — the name
    /// cannot be taken twice, and the path to the owning sketch is shown next to it.
    ///
    /// Global parameters are checked too: they share the same scope.
    pub fn name_dim(&mut self, name: String, target: DimTarget) -> bool {
        let nm = name.trim().to_string();
        if !nm.is_empty() && self.driver_name_taken_by(&nm, &target) {
            return false;
        }
        self.named_dims.retain(|n| n.target != target);
        if nm.is_empty() {
            return false; // An empty name clears the previous one: the dimension stops being a driver.
        }
        self.named_dims.push(NamedDim { name: nm, target });
        true
    }

    /// Name a sketch dimension. A wrapper over [`Project::name_dim`] that reads better at the call site.
    pub fn add_named_dim(&mut self, name: String, sketch: Id, refs: Vec<Id>) -> bool {
        self.name_dim(name, DimTarget::Sketch { sketch, refs: Self::dim_key(&refs) })
    }

    /// Name a feature parameter (`height`, `radius`, `thickness` and so on).
    pub fn add_named_feat_dim(&mut self, name: String, node: Id, key: &str) -> bool {
        self.name_dim(name, DimTarget::Feature { node, key: key.to_string() })
    }

    /// Whether a driver name is already taken by something else (a global parameter or another
    /// dimension). The interface uses it to explain the refusal before Enter is pressed and nothing
    /// happens.
    pub fn driver_name_taken_by(&self, name: &str, target: &DimTarget) -> bool {
        let nm = name.trim();
        if nm.is_empty() {
            return false;
        }
        self.parameters.iter().any(|p| p.name == nm) || self.named_dims.iter().any(|n| n.name == nm && n.target != *target)
    }

    /// The same for a sketch dimension, addressed by entities the way the popup addresses it.
    pub fn driver_name_taken(&self, name: &str, sketch: Id, refs: &[Id]) -> bool {
        self.driver_name_taken_by(name, &DimTarget::Sketch { sketch, refs: Self::dim_key(refs) })
    }

    /// The name given to this dimension or parameter. Empty means it is not a driver.
    pub fn name_of_target(&self, target: &DimTarget) -> String {
        self.named_dims.iter().find(|n| n.target == *target).map(|n| n.name.clone()).unwrap_or_default()
    }

    /// The same key, exposed: the interface needs it to build a dimension target and ask the model for its
    /// name without repeating the comparison rule.
    pub fn dim_key_pub(refs: &[Id]) -> Vec<Id> {
        Self::dim_key(refs)
    }

    /// Dimension key: the same entities in a stable order. A dimension is identified by the set, not by
    /// the order, or "distance A to B" and "distance B to A" would count as different dimensions.
    fn dim_key(refs: &[Id]) -> Vec<Id> {
        let mut v = refs.to_vec();
        v.sort_unstable();
        v
    }

    /// Entities a dimensional constraint is re-identified by. `None` means the constraint is not a
    /// dimension: it cannot become a driver and must not offer a name field.
    pub fn dim_refs(c: &Constraint) -> Option<Vec<Id>> {
        Some(match c {
            Constraint::Distance { a, b, .. } => vec![*a, *b],
            Constraint::Angle { a, b, c: c3, .. } => vec![*a, *b, *c3],
            Constraint::AngleLines { a, b, c: c3, d, .. } => vec![*a, *b, *c3, *d],
            Constraint::Diameter { c: c1, .. } => vec![*c1],
            Constraint::DistancePL { p, a, b, .. } => vec![*p, *a, *b],
            Constraint::EdgeDistance { c1, c2, .. } => vec![*c1, *c2],
            Constraint::ArcLength { c: c1, a, b, .. } => vec![*c1, *a, *b],
            _ => return None,
        })
    }

    /// Value of a dimensional constraint (length, angle, diameter). `None` when it is not a dimension.
    pub fn dim_value_of(c: &Constraint) -> Option<f64> {
        Some(match c {
            Constraint::Distance { d, .. } | Constraint::Diameter { d, .. } | Constraint::DistancePL { d, .. } | Constraint::EdgeDistance { d, .. } => *d,
            Constraint::Angle { deg, .. } | Constraint::AngleLines { deg, .. } => *deg,
            Constraint::ArcLength { len, .. } => *len,
            _ => return None,
        })
    }

    /// Set the value of whatever was named as a driver, from the parameter table.
    ///
    /// The parameter table is the one place where the whole set of numbers in a project is visible, so it
    /// has to be editable there and not read-only.
    ///
    /// Returns `true` when the value was written. The sketch is deliberately not re-solved here: the
    /// caller decides, and the caller also marks the document for rebuild, keeping one undo step per
    /// edit.
    pub fn set_dim_target_value(&mut self, target: &DimTarget, v: f64) -> bool {
        match target {
            DimTarget::Sketch { sketch, refs } => {
                let Some(si) = self.sketch_index(*sketch) else { return false };
                let want: std::collections::BTreeSet<Id> = refs.iter().copied().collect();
                let Some(ci) = self.sketches[si].constraints.iter().position(|c| Self::dim_refs(c).is_some_and(|r| r.into_iter().collect::<std::collections::BTreeSet<Id>>() == want)) else {
                    return false;
                };
                // An entered value overrides the expression: a typed number has to stay, instead of losing
                // to the previous formula on the next evaluation.
                match self.sketches[si].constraints.get_mut(ci) {
                    Some(Constraint::Distance { d, expr, .. }) | Some(Constraint::Diameter { d, expr, .. }) | Some(Constraint::EdgeDistance { d, expr, .. }) => {
                        *d = v;
                        expr.clear();
                    }
                    // In `DistancePL` the sign is the side, while the entered value is the magnitude.
                    Some(Constraint::DistancePL { d, expr, .. }) => {
                        *d = if *d < 0.0 { -v.abs() } else { v.abs() };
                        expr.clear();
                    }
                    Some(Constraint::Angle { deg, expr, .. }) | Some(Constraint::AngleLines { deg, expr, .. }) => {
                        *deg = v;
                        expr.clear();
                    }
                    Some(Constraint::ArcLength { len, expr, .. }) => {
                        *len = v;
                        expr.clear();
                    }
                    _ => return false,
                }
                true
            }
            DimTarget::Feature { node, key } => {
                if self.timeline.iter().find(|n| n.id == *node).and_then(|n| n.kind.dim(key)).is_none() {
                    return false; // This feature has no such parameter.
                }
                // For a feature the number lives in the expression: that is how the rebuild applies it
                // (`feat_dims` overrides the stored value) and how the edit stays visible in the feature
                // properties.
                self.set_feat_dim(*node, key, crate::expr::fmt_num(v));
                true
            }
        }
    }

    /// Remove a driver name.
    pub fn remove_named_dim(&mut self, name: &str) {
        self.named_dims.retain(|n| n.name != name);
    }

    /// Set the parametric expression for dimension `key` of feature `id` (`height`, `angle`, `radius` and
    /// so on). An empty expression clears it and returns to the stored number. Marks the feature dirty, so
    /// regenerate rebuilds it.
    pub fn set_feat_dim(&mut self, id: Id, key: &str, expr: String) {
        let e = expr.trim().to_string();
        if e.is_empty() {
            if let Some(m) = self.feat_dims.get_mut(&id) {
                m.remove(key);
                if m.is_empty() {
                    self.feat_dims.remove(&id);
                }
            }
        } else {
            self.feat_dims.entry(id).or_default().insert(key.to_string(), e);
        }
        self.mark_node_dirty(id);
    }

    /// Expression for dimension `key` of feature `id`, for display and editing in the properties panel.
    pub fn feat_dim(&self, id: Id, key: &str) -> Option<&str> {
        self.feat_dims.get(&id).and_then(|m| m.get(key)).map(|s| s.as_str())
    }

    /// Value of dimension `key` of feature `id` as the rebuild takes it: the expression evaluated against `vars`
    /// when there is one, otherwise `stored`, the number kept in the node. The number in the node is what was typed
    /// last and does not follow a parameter; whatever shows a size of a built feature reads it through here.
    pub fn feat_dim_value(&self, id: Id, key: &str, stored: f64, vars: &std::collections::HashMap<String, f64>) -> f64 {
        eval_dim(self.feat_dims.get(&id), key, stored, vars)
    }

    /// Mark as dirty the features whose expressions reference parameter `name`, and their consumers.
    ///
    /// Marking every feature that has any expression means rebuilding a project with a hundred dimensions
    /// after a single keystroke, so only the expressions that actually mention the name are considered.
    pub fn mark_param_dependents_dirty_for(&mut self, name: &str) {
        let ids: Vec<Id> = self.feat_dims.iter().filter(|(_, m)| m.values().any(|e| crate::expr::mentions(e, name))).map(|(id, _)| *id).collect();
        for id in ids {
            self.mark_node_dirty(id);
        }
        // Sketch dimensions using this parameter: the sketch is re-solved and its consumers follow it down
        // the timeline.
        let sids: Vec<Id> = self.sketches.iter().filter(|s| s.constraints.iter().any(|c| c.expr().is_some_and(|e| crate::expr::mentions(e, name)))).map(|s| s.id).collect();
        for sid in sids {
            self.mark_sketch_dirty(sid);
        }
    }

    /// Mark every feature that has a parametric dimension expression as dirty. Called after `parameters`
    /// changed, so the dependent bodies rebuild on the next regenerate.
    pub fn mark_param_dependents_dirty(&mut self) {
        let ids: Vec<Id> = self.feat_dims.keys().copied().collect();
        for id in ids {
            self.mark_node_dirty(id);
        }
    }

    /// Re-evaluate the expressions. Returns pairs of "what was evaluated" and an error code rather than
    /// phrases: the core knows no language, and the first half is the user's own input, which must not be
    /// translated.
    pub fn eval_parameters(&mut self) -> Vec<(String, crate::errors::ExprError)> {
        use crate::expr::eval;
        let mut errs = Vec::new();
        // THE SCOPE HOLDS BOTH KINDS OF NAME. A global parameter `w = 50` and a named dimension are the same
        // thing seen from a formula: a name that resolves to a number. Seeding from `self.parameters` alone
        // left every driver out of scope, so a dimension written as `w*2` could not be evaluated at all and
        // silently kept its previous value - which is exactly the reported behaviour, that drivers do not
        // work inside other dimensions.
        //
        // `param_map` is the one place that answers "what names does this document have", and it is used here
        // so that the scope of a rebuild and the scope of the completion list cannot drift apart.
        let mut vars = self.param_map();
        for p in &self.parameters {
            if !p.name.is_empty() {
                vars.insert(p.name.clone(), p.value); // seed with the previous value
            }
        }
        // Fixed point over the dependencies (up to eight passes).
        for _ in 0..8 {
            let mut changed = false;
            for p in &self.parameters {
                if p.name.is_empty() || p.expr.trim().is_empty() {
                    continue;
                }
                if let Ok(v) = eval(&p.expr, &vars) {
                    let key = p.name.clone();
                    if vars.get(&key).is_none_or(|o| (o - v).abs() > 1e-12) {
                        changed = true;
                    }
                    vars.insert(key, v);
                }
            }
            if !changed {
                break;
            }
        }
        for p in &mut self.parameters {
            if p.name.is_empty() {
                errs.push((String::new(), crate::errors::ExprError::UnknownFn(String::new())));
                continue;
            }
            if p.expr.trim().is_empty() {
                continue;
            }
            match eval(&p.expr, &vars) {
                Ok(v) => p.value = v,
                Err(e) => errs.push((p.name.clone(), e)),
            }
        }
        // Apply the expressions to dimensional constraints.
        for s in &mut self.sketches {
            for c in &mut s.constraints {
                match c {
                    Constraint::Distance { d, expr, .. } if !expr.trim().is_empty() => match eval(expr, &vars) {
                        Ok(v) => *d = v,
                        Err(e) => errs.push((expr.clone(), e)),
                    },
                    Constraint::Angle { deg, expr, .. } if !expr.trim().is_empty() => match eval(expr, &vars) {
                        Ok(v) => *deg = v,
                        Err(e) => errs.push((expr.clone(), e)),
                    },
                    Constraint::DistancePL { d, expr, .. } if !expr.trim().is_empty() => match eval(expr, &vars) {
                        Ok(v) => *d = if *d < 0.0 { -v.abs() } else { v.abs() }, // keep the side
                        Err(e) => errs.push((expr.clone(), e)),
                    },
                    Constraint::Diameter { d, expr, .. } if !expr.trim().is_empty() => match eval(expr, &vars) {
                        Ok(v) => *d = v,
                        Err(e) => errs.push((expr.clone(), e)),
                    },
                    Constraint::AngleLines { deg, expr, .. } if !expr.trim().is_empty() => match eval(expr, &vars) {
                        Ok(v) => *deg = v,
                        Err(e) => errs.push((expr.clone(), e)),
                    },
                    Constraint::ArcLength { len, expr, .. } if !expr.trim().is_empty() => match eval(expr, &vars) {
                        Ok(v) => *len = v,
                        Err(e) => errs.push((expr.clone(), e)),
                    },
                    _ => {}
                }
            }
        }
        errs
    }

    /// Evaluate an arbitrary expression in the context of the project parameters, for validating a
    /// dimension field in the interface. A parameter name is matched as written.
    pub fn eval_expr(&self, src: &str) -> Result<f64, crate::errors::ExprError> {
        // Going through `param_map` exposes both the global parameters and the named driving dimensions of
        // a skeleton sketch.
        crate::expr::eval(src, &self.param_map())
    }

    pub fn solve_sketch(&mut self, si: usize) -> f64 {
        self.solve_sketch_drag(si, None)
    }

    /// Solve a sketch while pinning the dragged point `drag = Some((id, x, y))` to the cursor: the point
    /// follows the pointer and constrained geometry resists.
    pub fn solve_sketch_drag(&mut self, si: usize, drag: Option<(Id, f64, f64)>) -> f64 {
        self.eval_parameters(); // Parametric dimensions become values before the solve.
        self.solve_sketch_inner(si, drag, crate::solver::Budget::FULL, sketch::Rebuild::Whole)
    }

    /// Solve a sketch within `budget`: what is left when its time is out stands as it stood and is counted in
    /// `Sketch::left_unsolved`.
    pub fn solve_sketch_within(&mut self, si: usize, budget: crate::solver::Budget) -> f64 {
        self.eval_parameters();
        self.solve_sketch_inner(si, None, budget, sketch::Rebuild::Whole)
    }

    /// Fast path for a drag frame: no `eval_parameters` (parameters are static during a drag, and the
    /// dimension values were already applied by the last full solve) and a reduced iteration budget for
    /// responsiveness. On release the interface calls the full `solve_sketch`.
    ///
    /// Without this every drag frame ran `eval_parameters`, a full Levenberg-Marquardt solve (120
    /// iterations with a numeric Jacobian) and a regenerate over every sketch, which lagged visibly on any
    /// sizeable sketch.
    pub fn solve_sketch_drag_fast(&mut self, si: usize, drag: Option<(Id, f64, f64)>) -> f64 {
        // a point dragged: through what the drag keeps from frame to frame, where the sketch takes it
        if let Some(r) = drag.and_then(|d| self.drag_frame(si, d)) {
            return r;
        }
        self.solve_sketch_drag_frame_alone(si, drag)
    }

    /// A FRAME OF A DRAG made without what the drag keeps from frame to frame: the part solved and the loops rebuilt as
    /// the frame finds them. The reference of the frames made through it (`solve_sketch_drag_fast`).
    pub fn solve_sketch_drag_frame_alone(&mut self, si: usize, drag: Option<(Id, f64, f64)>) -> f64 {
        self.solve_sketch_inner(si, drag, crate::solver::Budget::FRAME, sketch::Rebuild::Moved)
    }

    /// Add a regular polygon as a parametric group of entities: a construction circumscribed circle plus n
    /// side lines held by constraints (each vertex on the circle, equal sides, a radius dimension). The
    /// centre and the first vertex give the radius and the orientation.
    ///
    /// The polygon stays regular under the solver, scales with the circle dimension and rotates by
    /// dragging.
    pub fn add_polygon_entity(&mut self, si: usize, c: crate::geom::Point2, v: crate::geom::Point2, n: u32, purpose: crate::feature::Purpose) -> Vec<Id> {
        let ((cx, cy), (vx, vy)) = ((c.x, c.y), (v.x, v.y));
        let construction = purpose == crate::feature::Purpose::Construction;
        self.add_polygon_param(si, Point2::new(cx, cy), Point2::new(vx, vy), n, crate::feature::Purpose::of(construction)).1
    }

    // --- Sketches: the grouping layer above contours. ---

    /// Delete a sketch together with its contours and its embedded source.
    pub fn remove_sketch(&mut self, sketch_index: usize) {
        if sketch_index >= self.sketches.len() {
            return;
        }
        let sk = self.sketches.remove(sketch_index);
        for cid in &sk.contour_ids {
            if let Some(idx) = self.contour_index(*cid) {
                self.contours.remove_at(idx); // The id, the provenance and the nesting go with it.
            }
        }
        if let Some(src) = sk.source {
            self.sources.retain(|s| s.id != src);
        }
    }

    // --- Meshes. ---

    pub fn add_mesh(&mut self, m: crate::geom::Mesh) -> Id {
        let id = self.alloc_id();
        let name = format!("name-body#{}", self.bodies.len() + 1);
        self.bodies.push(Body { id, name, mesh: m, faces: Vec::new(), visible: true, sheet: false });
        id
    }

    pub fn set_meshes(&mut self, ms: Vec<crate::geom::Mesh>) {
        self.bodies.clear();
        self.bodies.clear();
        self.bodies.clear();
        for m in ms {
            self.add_mesh(m);
        }
    }

    /// Name map from body `from` to body `to`, matched by place.
    ///
    /// Needed when a node is deleted: its consumers are moved onto the source body, but their geometry
    /// references still name the faces and edges of the deleted body. An edge name is derived from the
    /// pair of faces meeting at it, and the source body has different faces, hence a different name, even
    /// though the edge itself is the same and sits in the same place.
    ///
    /// Without this translation, deleting a fillet turns a chamfer red on the opposite side of the part —
    /// geometry that never touched it. What broke the chamfer was the missing name translation, which is
    /// done here by matching place, because the place of that edge did not change.
    ///
    /// Anything not found is not translated: the reference goes red honestly instead of latching onto
    /// something similar.
    fn geom_name_map(&self, from: Id, to: Id) -> std::collections::HashMap<u32, u32> {
        const TOL: f64 = 1e-6;
        let mut map = std::collections::HashMap::new();
        if let (Some(a), Some(b)) = (self.regen_edges.get(&from), self.regen_edges.get(&to)) {
            for e in a.iter().filter(|e| e.id != 0) {
                if let Some(t) = b.iter().find(|t| t.id != 0 && (t.mid[0] - e.mid[0]).abs() < TOL && (t.mid[1] - e.mid[1]).abs() < TOL && (t.mid[2] - e.mid[2]).abs() < TOL) {
                    if t.id != e.id {
                        map.insert(e.id, t.id);
                    }
                }
            }
        }
        if let (Some(a), Some(b)) = (self.regen_faces.get(&from), self.regen_faces.get(&to)) {
            for f in a.iter().filter(|f| f.id != 0) {
                let same = |x: &crate::geom::MeshFace| {
                    (x.centroid.x - f.centroid.x).abs() < TOL
                        && (x.centroid.y - f.centroid.y).abs() < TOL
                        && (x.centroid.z - f.centroid.z).abs() < TOL
                        && x.normal[0] * f.normal[0] + x.normal[1] * f.normal[1] + x.normal[2] * f.normal[2] > 0.9
                };
                if let Some(t) = b.iter().find(|x| x.id != 0 && same(x)) {
                    if t.id != f.id {
                        map.insert(f.id, t.id);
                    }
                }
            }
        }
        map
    }

    /// Re-point non-body references from body `from` to body `to`: sketches on a face, datum axes on an
    /// edge or a face, datum planes offset from a face. Face and edge keys are persistent, so they resolve
    /// on the new body by themselves; whatever does not resolve goes red on rebuild instead of
    /// disappearing silently.
    ///
    /// `nmap` translates names from body `from` to body `to` (see `geom_name_map`).
    ///
    /// Changing the body number is not enough: a sketch on a face, a datum axis on an edge and a plane
    /// offset from a face also hold the name of that geometry, and on the source body the name differs,
    /// being derived from the surroundings. Without the translation such an anchor points at nothing after
    /// the node is deleted, and silently builds nothing.
    fn rebind_body_refs(&mut self, from: Id, to: Id, nmap: &std::collections::HashMap<u32, u32>) {
        use crate::feature::SketchPlane;
        let m = |x: u32| nmap.get(&x).copied().unwrap_or(x);
        let mkey = |k: &crate::feature::FaceKey| {
            let mut k = *k;
            k.id = m(k.id);
            k
        };
        for s in self.sketches.iter_mut() {
            if let SketchPlane::Face(b, ref key) = s.plane {
                if b == from {
                    s.plane = SketchPlane::Face(to, mkey(key));
                }
            }
        }
        for a in self.datum_axes.iter_mut() {
            match a.def {
                AxisDef::FromEdge { body, edge } if body == from => a.def = AxisDef::FromEdge { body: to, edge: m(edge) },
                AxisDef::FromFace { body, face } if body == from => a.def = AxisDef::FromFace { body: to, face: m(face) },
                _ => {}
            }
        }
        for pl in self.planes.iter_mut() {
            if let PlaneDef::OffsetFace { body, ref face, dist } = pl.def {
                if body == from {
                    pl.def = PlaneDef::OffsetFace { body: to, face: mkey(face), dist };
                }
            }
        }
    }

    /// Remove ghosts: orphaned meshes, a body with no timeline node that is not an import - for example a
    /// primitive whose node was deleted while the mesh stayed. A body is valid when it has a timeline node or is
    /// listed in `imported_bodies`. A feature whose source is gone is NOT removed: it stays in the timeline, red
    /// with the reason, for the person to repair or delete - taking it away here would be the cascade the delete
    /// no longer makes unless asked. Returns the bodies that were removed.
    pub fn prune_dangling(&mut self) -> std::collections::HashSet<Id> {
        let mut valid: std::collections::HashSet<Id> = self.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
        valid.extend(self.imported_bodies.iter().copied());
        let removed: std::collections::HashSet<Id> = self.bodies.iter().map(|b| b.id).filter(|b| !valid.contains(b)).collect();
        if !removed.is_empty() {
            self.remove_bodies(&removed);
        }
        // Orphaned connectors and joints: a connector whose owning component is gone, or whose anchor
        // references a body that no longer exists, is broken. Its joint resolves to a garbage frame and the
        // solve then places bodies at wild coordinates, which reads as the assembly blowing apart. Cleaned
        // on every regenerate, which also repairs older files that still carry such orphans.
        use crate::feature::AnchorRef;
        let live_comp: std::collections::HashSet<Id> = self.components.iter().map(|c| c.id).collect();
        let live_body: std::collections::HashSet<Id> = self.timeline.iter().flat_map(|n| n.kind.bodies()).chain(self.imported_bodies.iter().copied()).collect();
        let anchor_ok = |a: &AnchorRef| match a {
            AnchorRef::FaceCenter(b, _) | AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => live_body.contains(b),
            _ => true, // Origin and BasePlane reference no body.
        };
        let dead_conn: std::collections::HashSet<Id> = self.connectors.iter().filter(|c| !live_comp.contains(&c.owner) || !anchor_ok(&c.anchor)).map(|c| c.id).collect();
        if !dead_conn.is_empty() {
            self.joints.retain(|j| !dead_conn.contains(&j.a) && !dead_conn.contains(&j.b));
            self.connectors.retain(|c| !dead_conn.contains(&c.id));
        }
        removed
    }

    /// Delete a body by index (mesh, id and name together). A manual colour, keyed by lineage root, stays in
    /// `part_colors`: a harmless leftover entry once the root is gone too.
    pub fn remove_mesh(&mut self, index: usize) {
        if index < self.bodies.len() {
            self.bodies.remove(index);
        }
    }

    pub fn mesh_index(&self, id: Id) -> Option<usize> {
        self.bodies.iter().position(|b| b.id == id)
    }

    /// THE FACES OF A BODY, by its id.
    ///
    /// Two places wrote `bodies.last_mut().faces = fs` after adding a body - a shape import and a rebuild -
    /// and "the last one" is only true while nothing else has been pushed in between. By id it is true
    /// always.
    pub fn set_body_faces(&mut self, body: Id, faces: Vec<crate::geom::MeshFace>) -> bool {
        match self.bodies.iter_mut().find(|b| b.id == body) {
            Some(b) => {
                b.faces = faces;
                true
            }
            None => false,
        }
    }

    pub fn mesh_id(&self, index: usize) -> Option<Id> {
        self.bodies.get(index).map(|b| b.id)
    }

    /// Body name by index, falling back to a generated one.
    pub fn mesh_name(&self, index: usize) -> String {
        self.bodies.get(index).map(|b| b.name.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| format!("name-body#{}", index + 1))
    }

    pub fn set_mesh_name(&mut self, index: usize, name: String) {
        if let Some(b) = self.bodies.get_mut(index) {
            b.name = name;
        }
    }

    /// Id of the first mesh, used as the default target when creating a 3D operation.
    pub fn first_mesh_id(&self) -> Id {
        self.bodies.first().map(|b| b.id).unwrap_or(0)
    }

    /// Embed the original of an imported file and return its id.
    pub fn add_source(&mut self, name: impl Into<String>, data: Vec<u8>) -> Id {
        let name = name.into();
        let ext = std::path::Path::new(&name).extension().and_then(|e| e.to_str()).unwrap_or("bin").to_lowercase();
        let id = self.alloc_id();
        self.sources.push(SourceFile { id, name, ext, data });
        id
    }

    /// Bring the id arrays in line with the geometry after loading, when ids were missing or of a different
    /// length.
    pub fn ensure_ids(&mut self) {
        // Contours no longer need this: the list and its ids are one structure (`model::contours`) and
        // cannot differ in length. Bodies no longer need it either, since id and name are part of the record
        // itself; only what may have arrived empty from the file is repaired here.
        for i in 0..self.bodies.len() {
            if self.bodies[i].id == 0 {
                self.bodies[i].id = self.alloc_id();
            }
            if self.bodies[i].name.is_empty() {
                self.bodies[i].name = format!("name-body#{}", i + 1);
            }
        }
        // Colours are no longer a parallel array to the meshes: manual ones live in `part_colors` keyed by
        // lineage root, and the rest come from the palette.
    }

    /// Post-load normalisation of a document: make sure the root assembly exists and attach floating nodes
    /// and components (`parent == None`) to it. The timeline is canonical, so no projection from the pools
    /// is needed. Idempotent.
    pub fn ensure_document(&mut self) {
        self.migrate_root();
    }

    /// Make sure the root assembly component exists and attach every floating node and component
    /// (`parent == None`) to it. Afterwards only the root has `parent == None`. Idempotent.
    pub fn migrate_root(&mut self) {
        let root = self.ensure_root();
        // The root has no name of its own, only a catalogue key.
        //
        // The root assembly is not a component someone created and can rename: there is always exactly
        // one. Stored as a plain string, its name freezes in the language of the build that created the
        // document, so an English interface ends up showing a Russian label. A name is only translatable
        // when it is a key, which is why the key is written here on every load: a document whose string
        // already froze would otherwise have no way back.
        if let Some(c) = self.components.iter_mut().find(|c| c.id == root) {
            c.name = "name-assembly".into();
        }
        for n in self.timeline.iter_mut() {
            if n.parent.is_none() {
                n.parent = Some(root);
            }
        }
        for c in self.components.iter_mut() {
            if c.parent.is_none() && c.id != root {
                c.parent = Some(root);
            }
        }
    }

    /// Repair lost face references in one pass before the build, instead of guessing inside every resolve.
    ///
    /// Falling back to a geometric fingerprint (co-directed normal plus nearest centre) inside each resolve
    /// repeats the guess on every rebuild: the reference is "found" every time, possibly a different one
    /// every time, and nothing reports it. Instead:
    ///
    /// * resolution goes by id, and the fingerprint only proposes a candidate;
    /// * the id found is written back into the key, so the guess happens once rather than every time;
    /// * the repair is recorded in the report and visible in the tree instead of dissolving.
    pub fn rebind_lost_face_refs_for_test(&mut self, report: &mut crate::feature::RegenReport) {
        self.rebind_lost_face_refs(report);
    }

    fn rebind_lost_face_refs(&mut self, report: &mut crate::feature::RegenReport) {
        use crate::feature::{Rebind, SketchPlane};
        // Candidate by fingerprint among the faces of the body: co-directed normal, nearest centre.
        let candidate = |faces: &[crate::geom::MeshFace], key: &crate::feature::FaceKey| -> Option<u32> {
            let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let d2 = |c: &crate::geom::Point3| (c.x - key.centroid[0]).powi(2) + (c.y - key.centroid[1]).powi(2) + (c.z - key.centroid[2]).powi(2);
            faces.iter().filter(|f| f.id != 0 && dot(f.normal, key.normal) > 0.9).min_by(|a, b| d2(&a.centroid).partial_cmp(&d2(&b.centroid)).unwrap_or(std::cmp::Ordering::Equal)).map(|f| f.id)
        };
        let known = |me: &Self, body: Id, id: u32| -> bool { id != 0 && me.regen_faces.get(&body).is_some_and(|fs| fs.iter().any(|f| f.id == id)) };
        // Holes are deliberately not handled here: their reference became a query, which needs no matching
        // — it either finds the face by recipe or refuses with a named reason.
        //
        // Sketches placed on a face of a body:
        let mut sfixes: Vec<(Id, Id, u32, String)> = Vec::new();
        for s in &self.sketches {
            if let SketchPlane::Face(body, ref key) = s.plane {
                if !known(self, body, key.id) {
                    if let Some(fs) = self.regen_faces.get(&body) {
                        if let Some(newid) = candidate(fs, key) {
                            sfixes.push((s.id, body, newid, format!("rebind-sketch-face#{}: {} -> {newid}", s.name, key.id)));
                        }
                    }
                }
            }
        }
        for (sid, body, newid, what) in sfixes {
            if let Some(s) = self.sketches.iter_mut().find(|s| s.id == sid) {
                if let SketchPlane::Face(b, key) = &mut s.plane {
                    let _ = b;
                    key.id = newid;
                }
            }
            report.rebinds.push(Rebind { node: sid, body, what });
        }
    }

    /// Every face of a body as reference resolution sees it: the pool a query is evaluated against.
    ///
    /// Built from the live rebuild rather than from what a reference recorded earlier. That is the whole
    /// point: a query asks today's geometry instead of comparing against last year's snapshot.
    pub fn face_pool(&self, body: Id) -> Vec<crate::refs::Candidate> {
        self.regen_faces
            .get(&body)
            .map(|fs| {
                fs.iter()
                    .map(|f| crate::refs::Candidate {
                        edge: None, // A face has no endpoints; this field belongs to edges.
                        desc: f.id,
                        centroid: [f.centroid.x, f.centroid.y, f.centroid.z],
                        normal: f.normal,
                        area: f.area,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resolve a reference to a set of faces into descriptors, or into a named refusal.
    ///
    /// The order is stable (it matches the live rebuild), so the result of an operation does not shift
    /// between runs: the kernel builds from this list, and reordering it changes the names of the faces it
    /// produces.
    pub fn resolve_face_refs(&self, body: Id, r: &crate::refs::Ref, what: &str) -> Result<Vec<u32>, crate::refs::RefError> {
        let pool = self.face_pool(body);
        r.resolve(what, &pool, &self.names, &pool)
    }

    /// Every edge of a body as resolution sees it. The edge direction plays the role of a normal and the
    /// length the role of an area, so descriptive queries ("along the axis", "the longest") work on edges
    /// without any separate code.
    pub fn edge_pool(&self, body: Id) -> Vec<crate::refs::Candidate> {
        self.regen_edges.get(&body).map(|es| Self::edge_candidates(es)).unwrap_or_default()
    }

    /// A list of edges as resolution sees it, wherever the list came from.
    fn edge_candidates(es: &[crate::geom::MeshEdge]) -> Vec<crate::refs::Candidate> {
        es.iter()
            .map(|e| crate::refs::Candidate {
                desc: e.id,
                centroid: e.mid,
                normal: e.dir,
                area: ((e.b[0] - e.a[0]).powi(2) + (e.b[1] - e.a[1]).powi(2) + (e.b[2] - e.a[2]).powi(2)).sqrt(),
                edge: Some(crate::refs::EdgeGeom { a: e.a, b: e.b, center: e.center, axis: e.axis, radius: e.radius }),
            })
            .collect()
    }

    /// Every vertex of a body as resolution sees it: a point plus a name derived from its edges.
    ///
    /// Built from the live rebuild rather than from a separate kernel channel, because a vertex has nothing
    /// of its own except its place, while the edges and their names already exist. The edge endpoints that
    /// meet at one point are the vertex, and the name (`VertexName`) is derived from them the same way an
    /// edge name is derived from its pair of faces.
    ///
    /// The merge tolerance is 1e-6: the endpoints come from one rebuild of one body, that is, from the same
    /// geometry, so anything looser would merge two neighbouring vertices into one.
    pub fn vertex_pool(&mut self, body: Id) -> Vec<crate::refs::Candidate> {
        self.vertex_spots(body)
            .into_iter()
            .map(|(p, ids)| {
                let desc = self.names.intern_vertex(crate::names::VertexName::new(ids));
                crate::refs::Candidate { desc, centroid: p, normal: [0.0, 0.0, 1.0], area: 0.0, edge: None }
            })
            .collect()
    }

    /// Vertices of a body without names: a point and the edges meeting at it.
    ///
    /// Kept separate from [`Project::vertex_pool`] because this is asked far more often than names are
    /// created: drawing and hit-testing hold `&self` and have no right to allocate names, and a name is
    /// allocated exactly when a vertex is actually referenced.
    pub fn vertex_spots(&self, body: Id) -> Vec<([f64; 3], Vec<u32>)> {
        const TOL: f64 = 1e-6;
        let Some(edges) = self.regen_edges.get(&body) else { return Vec::new() };
        // Group the endpoints: point to the edges meeting there.
        let mut spots: Vec<([f64; 3], Vec<u32>)> = Vec::new();
        for e in edges {
            if e.id == 0 {
                continue;
            }
            for p in [e.a, e.b] {
                let near = |q: &[f64; 3]| (q[0] - p[0]).abs() < TOL && (q[1] - p[1]).abs() < TOL && (q[2] - p[2]).abs() < TOL;
                match spots.iter_mut().find(|(q, _)| near(q)) {
                    Some((_, ids)) => {
                        if !ids.contains(&e.id) {
                            ids.push(e.id);
                        }
                    }
                    None => spots.push((p, vec![e.id])),
                }
            }
        }
        // The order follows the endpoints as encountered, which is determined by the edge order of the live
        // rebuild; the name does not depend on order at all.
        spots
    }

    /// Resolve a reference to vertices. The edge pool is passed alongside the vertex pool, because a vertex
    /// is described through its edges and has no other vocabulary.
    pub fn resolve_vertex_refs(&mut self, body: Id, r: &crate::refs::Ref, what: &str) -> Result<Vec<u32>, crate::refs::RefError> {
        let pool = self.vertex_pool(body);
        let edges = self.edge_pool(body);
        r.resolve(what, &pool, &self.names, &edges)
    }

    /// Where the vertex with this name is: a point, or `None` when the body no longer has it.
    pub fn vertex_point(&mut self, body: Id, desc: u32) -> Option<[f64; 3]> {
        self.vertex_pool(body).into_iter().find(|c| c.desc == desc).map(|c| c.centroid)
    }

    /// Resolve a reference to edges. The face pool is passed alongside the edge pool, because an edge query
    /// is always phrased through faces ("every edge of this face", "the seam between these two sets").
    pub fn resolve_edge_refs(&self, body: Id, r: &crate::refs::Ref, what: &str) -> Result<Vec<u32>, crate::refs::RefError> {
        r.resolve(what, &self.edge_pool(body), &self.names, &self.face_pool(body))
    }

    /// The same against an explicit list of the body's edges, as the kernel holds them now.
    ///
    /// `regen_edges` is filled by the post pass of a rebuild and is not written into a bundle: in the middle of
    /// a pass it holds the previous state of the body, and right after a file is opened it holds nothing. A
    /// query resolved against it there found no edges at all, while the faces it is phrased through were known.
    pub fn resolve_edge_refs_in(&self, body: Id, edges: &[crate::geom::MeshEdge], r: &crate::refs::Ref, what: &str) -> Result<Vec<u32>, crate::refs::RefError> {
        r.resolve(what, &Self::edge_candidates(edges), &self.names, &self.face_pool(body))
    }

    /// Resolve a reference to a single face into a descriptor, a centre and a normal, or into a named
    /// refusal.
    ///
    /// A refusal here is the job, not a failure: it states what was looked for and where it used to be,
    /// instead of silently matching a similar face. Silent matching is what moved holes and fillets onto
    /// neighbouring geometry.
    pub fn resolve_face_ref(&self, body: Id, r: &crate::refs::Ref, what: &str) -> Result<crate::refs::Candidate, crate::refs::RefError> {
        let pool = self.face_pool(body);
        let found = r.resolve(what, &pool, &self.names, &pool)?;
        let d = found.first().copied().unwrap_or(0);
        pool.iter().find(|c| c.desc == d).copied().ok_or_else(|| crate::refs::RefError::Lost { what: what.into(), was: r.hint })
    }

    pub fn resolve_face(&self, body: Id, key: &crate::feature::FaceKey) -> ([f64; 3], [f64; 3]) {
        // Resolution goes by persistent id only. Matching a similar face by fingerprint is deliberately not
        // done here: it silently substituted the wrong face and repeated on every rebuild. Lost references
        // are repaired by the single `rebind_lost_face_refs` pass before the build, which writes the id it
        // found into the key and records the repair in the report. A reference arrives here either with a
        // current id or with its own fingerprint when there was nothing to repair against (the body is not
        // built), and in that case the fingerprint is returned honestly.
        if let Some(faces) = self.regen_faces.get(&body) {
            if key.id != 0 {
                if let Some(f) = faces.iter().find(|f| f.id == key.id) {
                    return ([f.centroid.x, f.centroid.y, f.centroid.z], f.normal);
                }
            }
        }
        (key.centroid, key.normal)
    }

    /// Frame of a sketch plane, by sketch id.
    pub fn sketch_frame_by_id(&self, sid: Id) -> Option<crate::feature::PlaneFrame> {
        self.sketch_index(sid).and_then(|si| self.sketch_frame(si))
    }

    /// Profile of a sketch by id: its first closed contour, flattened to XY.
    pub fn sketch_profile_by_id(&self, sid: Id) -> Option<Vec<f64>> {
        let si = self.sketch_index(sid)?;
        self.sketch_profile_xy(si).map(|(_, xy)| xy)
    }

    /// Profile for a feature: the specific contour `profile` when it is set, otherwise the first closed
    /// contour of the sketch.
    pub fn feature_profile_xy(&self, sketch: Id, profile: Id) -> Option<Vec<f64>> {
        if profile != 0 {
            self.contour_profile_xy(profile)
        } else {
            self.sketch_profile_by_id(sketch)
        }
    }

    /// Exact profile of a feature for the kernel: the outer contour (`profile`, or the first closed one)
    /// plus its holes, in the `geom::encode_profile` encoding — real edges producing exact faces rather
    /// than a faceted approximation.
    pub fn feature_profile_encoded(&self, sketch: Id, profile: Id) -> Option<Vec<f64>> {
        self.feature_profile_encoded_fill(sketch, profile, &[])
    }

    /// Face-name descriptor from its recipe, interned in the document name table.
    pub fn intern_name(&mut self, feature: Id, role: crate::names::Role, src: Id) -> u32 {
        self.names.intern_face(crate::names::GeoName::new(feature, role, src))
    }

    /// Names of the operation caps: the start (the profile itself) and the end (its translated copy). They
    /// are passed to the kernel as separate parameters, because a cap is not produced by a profile edge and
    /// has no name to take from the encoding.
    pub fn cap_names(&mut self, feature: Id) -> [u32; 2] {
        [self.intern_name(feature, crate::names::Role::CapStart, 0), self.intern_name(feature, crate::names::Role::CapEnd, 0)]
    }

    /// Loft encoding. For each section sketch the contour is taken (`contours[i]`, or the first closed one)
    /// and the result is the concatenation of `loop_block`s, `offsets[nsec+1]` marking where each section
    /// starts, and `places[nsec*12]` holding the 3x4 placements of the section planes. `None` when fewer
    /// than two valid sections were found, since a loft needs at least two.
    pub fn loft_encoded(&self, sketches: &[Id], contours: &[Id]) -> Option<(Vec<f64>, Vec<usize>, Vec<f64>)> {
        self.loft_encoded_with(sketches, contours, &std::collections::HashMap::new())
    }

    /// Write the mesh of a body by id: replace the existing one or create a new body with that id.
    pub fn set_body_mesh(&mut self, id: Id, m: crate::geom::Mesh) {
        if let Some(idx) = self.mesh_index(id) {
            self.bodies[idx].mesh = m;
        } else {
            let name = format!("name-body#{}", self.bodies.len() + 1);
            self.bodies.push(Body { id, name, mesh: m, faces: Vec::new(), visible: true, sheet: false });
        }
    }

    /// Extrude the first closed contour of a sketch. Returns the id of the resulting body.
    pub fn add_extrude(&mut self, sketch: Id, height: f64) -> Id {
        self.add_extrude_on(sketch, 0, height, crate::feature::Reach::Forward, 0.0)
    }

    /// Revolve every closed contour of a sketch as one node.
    pub fn add_revolve(&mut self, sketch: Id, axis: u8, angle: f64) -> Id {
        self.add_revolve_axis(sketch, Vec::new(), axis, angle, 0, 0)
    }

    /// Revolve a specific contour `profile` (0 means every closed one). `axis_datum` is a datum axis; 0 uses
    /// the X or Y axis of the sketch.
    pub fn add_revolve_on(&mut self, sketch: Id, profile: Id, axis: u8, angle: f64) -> Id {
        let profiles = if profile == 0 { Vec::new() } else { vec![profile] };
        self.add_revolve_axis(sketch, profiles, axis, angle, 0, 0)
    }

    /// Add a datum point (a `DatumPoint` timeline node) and return its id.
    pub fn add_datum_point(&mut self, mut dp: DatumPoint) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        if dp.id == 0 {
            dp.id = self.alloc_id();
        }
        let (id, name) = (dp.id, dp.name.clone());
        let parent = Some(self.active_ctx());
        self.datum_points.push(dp);
        self.push_timeline(FeatureNode { id, name, kind: FeatureKind::DatumPoint { point: id }, parent, dirty: false, suppressed: false });
        id
    }

    /// Add a datum axis (a `DatumAxis` timeline node) and return its id.
    pub fn add_datum_axis(&mut self, mut da: DatumAxis) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        if da.id == 0 {
            da.id = self.alloc_id();
        }
        let (id, name) = (da.id, da.name.clone());
        let parent = Some(self.active_ctx());
        // AN AXIS TAKEN FROM SOMETHING IS WHERE THAT THING IS ONLY ONCE A REBUILD HAS LOOKED: an edge, a round face or
        // two points are resolved in the timeline pass. Left clean, a window that rebuilds only what is marked never
        // ran that pass, and the axis stood at its default - the world Z through the origin.
        let derived = !matches!(da.def, AxisDef::Manual { .. });
        self.datum_axes.push(da);
        self.push_timeline(FeatureNode { id, name, kind: FeatureKind::DatumAxis { axis: id }, parent, dirty: derived, suppressed: false });
        id
    }

    /// Delete a datum plane by id, from `planes` and from the timeline together. One method rather than
    /// logic inlined in the interface. Returns whether the plane existed.
    /// Delete a datum plane ALONE, as a base is deleted (decided 24.09): what stands on it stays, red with the reason.
    /// A sketch on it is put on a snapshot of where the plane stood (`PlaneDef::PlaneGone`), so it does not move and
    /// goes red saying its plane was deleted; an offset plane built from it stays where it stands, a plane of its own.
    pub fn delete_plane(&mut self, id: Id) -> bool {
        use crate::feature::SketchPlane;
        let Some(gone) = self.planes.iter().find(|p| p.id == id).cloned() else { return false };
        let on_plane: Vec<(usize, Id)> = self.sketches.iter().enumerate().filter(|(_, s)| matches!(s.plane, SketchPlane::Datum(pid) if pid == id)).map(|(si, s)| (si, s.id)).collect();
        if !on_plane.is_empty() {
            let snap =
                self.add_plane(WorkPlane { name: "name-plane-deleted-frozen".into(), origin: gone.origin, normal: gone.normal, rot_deg: gone.rot_deg, def: PlaneDef::PlaneGone, ..Default::default() });
            for (si, sid) in on_plane {
                self.sketches[si].plane = SketchPlane::Datum(snap);
                self.mark_node_dirty(sid); // the rebuild visits it and marks it red
            }
        }
        for p in self.planes.iter_mut().filter(|p| matches!(p.def, PlaneDef::OffsetPlane { plane, .. } if plane == id)) {
            p.def = PlaneDef::Manual; // it keeps the origin and the normal it was last resolved to
        }
        self.drop_plane_node(id)
    }

    /// Delete a datum plane WITH what is built on it (the "and what is built on it" box): the sketches on it and what
    /// they make, and the offset planes built from it, recursively.
    pub fn delete_plane_with_dependents(&mut self, id: Id) -> bool {
        use crate::feature::SketchPlane;
        if !self.planes.iter().any(|p| p.id == id) {
            return false;
        }
        let on_plane: Vec<Id> = self.sketches.iter().filter(|s| matches!(s.plane, SketchPlane::Datum(pid) if pid == id)).map(|s| s.id).collect();
        for sid in on_plane {
            self.delete_sketch_with_dependents(sid);
        }
        let children: Vec<Id> = self.planes.iter().filter(|p| matches!(p.def, PlaneDef::OffsetPlane { plane, .. } if plane == id)).map(|p| p.id).collect();
        for cid in children {
            self.delete_plane_with_dependents(cid);
        }
        self.drop_plane_node(id)
    }

    /// The plane itself goes, and the mirrors and splits that named it go red on the rebuild.
    fn drop_plane_node(&mut self, id: Id) -> bool {
        use crate::feature::FeatureKind;
        let had = self.planes.iter().any(|p| p.id == id);
        // Neither mirror nor section degrades. The reference is left dangling so that regenerate sees the
        // missing plane and goes red honestly.
        //
        // Degrading a mirror to the world plane (datum = 0) sounds defensible — a mirror about another
        // plane is still a mirror — but the measurement refutes it: a mirror about a datum at x = 50 gave a
        // part with face centres from x = 10 to x = 90, and deleting the datum moved them to -30..30. The
        // part moved and not a single node went red. Section has refused in the same situation for a long
        // time, for the same reason: a silent substitution is invisible until someone notices the part is
        // in the wrong place.
        for n in &mut self.timeline {
            let touches = match &n.kind {
                FeatureKind::Mirror { datum, .. } | FeatureKind::SplitBody { datum, .. } => *datum == id,
                _ => false,
            };
            if touches {
                n.dirty = true;
            }
        }
        self.planes.retain(|p| p.id != id);
        self.timeline.retain(|n| !matches!(n.kind, FeatureKind::Plane { plane } if plane == id));
        had
    }

    /// Delete a datum point by id, from `datum_points` and from the timeline together. Two-point axes and
    /// patterns that used it degrade through regenerate, resolving to their defaults. Returns whether the
    /// point existed.
    pub fn delete_datum_point(&mut self, id: Id) -> bool {
        let had = self.datum_points.iter().any(|p| p.id == id);
        if !had {
            return false;
        }
        // Two-point axes that use this point: mark their nodes dirty. Resolution will freeze or degrade the
        // axis to its previous value, but the rebuild has to run cleanly, without stale consumer geometry.
        let affected: Vec<Id> = self.datum_axes.iter().filter(|a| matches!(a.def, AxisDef::TwoPoints { a: pa, b: pb } if pa == id || pb == id)).map(|a| a.id).collect();
        self.datum_points.retain(|p| p.id != id);
        self.timeline.retain(|n| !matches!(n.kind, crate::feature::FeatureKind::DatumPoint { point } if point == id));
        for aid in affected {
            for n in &mut self.timeline {
                if matches!(n.kind, crate::feature::FeatureKind::DatumAxis { axis } if axis == aid) {
                    n.dirty = true;
                }
            }
        }
        had
    }

    /// Delete a datum axis by id, from `datum_axes` and from the timeline together. Circular patterns around
    /// it degrade through regenerate, resolving to world Z. Returns whether the axis existed.
    pub fn delete_datum_axis(&mut self, id: Id) -> bool {
        use crate::feature::FeatureKind;
        let had = self.datum_axes.iter().any(|a| a.id == id);
        if !had {
            return false;
        }
        // Degrading references: a revolve around this axis falls back to the sketch X or Y axis
        // (`axis_datum = 0`), and a circular pattern falls back to world Z (`axis = 0`). Without marking the
        // nodes dirty here the degradation happened only inside regenerate and left stale geometry behind.
        //
        // The measured price of this behaviour is high: the part moves silently. A revolve around an axis
        // at x = 40 produced a body at x = 40 and, without the axis, at x = 0; a pattern around an axis at
        // x = 60 spread its centres over 0..120 and, without the axis, over -10..10 — with no red node in
        // either case. Neighbouring behaviour (section, mirror) refuses to build in the same situation.
        //
        // Here, however, degradation is a recorded contract rather than code that merely happened: the
        // tests `delete_datum_axis_degrades_circular_array` and `revolve_around_datum_axis_uses_axis_kernel`
        // assert it. Changing that contract is a separate decision.
        for n in &mut self.timeline {
            match &mut n.kind {
                FeatureKind::Revolve { axis_datum, .. } => {
                    if *axis_datum == id {
                        *axis_datum = 0;
                        n.dirty = true;
                    }
                }
                FeatureKind::CircularArray { axis, .. } if *axis == id => {
                    *axis = 0;
                    n.dirty = true;
                }
                _ => {}
            }
        }
        self.datum_axes.retain(|a| a.id != id);
        self.timeline.retain(|n| !matches!(n.kind, FeatureKind::DatumAxis { axis } if axis == id));
        had
    }

    /// Initialise a new document: the root assembly plus one active, empty part.
    ///
    /// Under strict isolation (bodies may exist only inside a part) nothing can be drawn in an empty
    /// assembly, so a document always starts with an active part. Returns the id of that part.
    pub fn new_document(&mut self) -> Id {
        self.ensure_root();
        let part = self.add_part(self.free_part_name());
        self.set_active_component(Some(part));
        part
    }

    /// WHERE A NEW SKETCH GOES: the part being worked in, or - when the context is an assembly - a new part made in it
    /// and stood in, so a sketch always belongs to a part (a part is one body, and a sketch in an assembly has no body
    /// to build). `Some(part)` when a part was made; the window steps into it.
    pub fn part_to_draw_in(&mut self) -> Option<Id> {
        let ctx = self.active_ctx();
        if self.component_is_part(ctx) {
            return None;
        }
        let part = self.add_part(self.free_part_name());
        self.set_active_component(Some(part));
        Some(part)
    }

    /// THE DOCUMENT OF A FIRST START: a part called Cube holding a sketch of a 20 x 20 square on XY and its extrusion
    /// 20 up - a cube of 8000 mm^3. Opened into, it shows how the work goes here: a sketch, and a feature built on it.
    /// Decided 25.09 on a report that a first start met a person with an empty part.
    pub fn cube_sample(&mut self) -> Id {
        let root = self.ensure_root();
        self.set_active_component(Some(root));
        let part = self.add_part("name-sample-cube");
        self.set_active_component(Some(part));
        let si = self.new_sketch("name-sketch-n#1");
        let sid = self.sketches[si].id;
        self.add_sketch_node(sid, "name-sketch-n#1");
        self.add_rect_entity(si, -10.0, -10.0, 10.0, 10.0, crate::feature::Purpose::Real);
        self.regen_sketch(si);
        let body = self.add_extrude(sid, 20.0);
        self.finish_base_body(body, 1);
        // the person stands in the assembly and sees the part in it; stepping into it shows how it is made
        self.set_active_component(Some(root));
        part
    }

    /// AN EMPTY DOCUMENT: the root assembly and nothing else.
    ///
    /// Reported behaviour: "in an empty project remove the empty part - a person opens the CAD and sees an
    /// empty part; the first time round that spoils everything." A part nobody asked for is not a starting
    /// point, it is a thing to delete before starting.
    ///
    /// SEPARATE FROM `new_document` rather than a flag on it. `new_document` is what "New part" gives, and
    /// there the part is exactly what was asked for; the two are different answers to different questions,
    /// and a boolean argument would make every call site say which question it is asking.
    ///
    /// The root is still made: a document without one has nowhere to put anything, and the tree would show
    /// an empty pane rather than a document.
    pub fn new_empty_document(&mut self) -> Id {
        let root = self.ensure_root();
        // the context is the root itself, not a part inside it - named as the window names it, or the window's first
        // frame names it so and the untouched document reads as changed
        self.set_active_component(Some(root));
        root
    }

    /// A part in the active context.
    pub fn add_part(&mut self, name: impl Into<String>) -> Id {
        self.add_component_kind(name, crate::feature::ComponentKind::Part)
    }

    /// A subassembly in the active context.
    pub fn add_assembly(&mut self, name: impl Into<String>) -> Id {
        self.add_component_kind(name, crate::feature::ComponentKind::Assembly)
    }

    /// Whether a context may hold bodies. The root and any assembly may not; only a part may.
    pub fn ctx_holds_bodies(&self, ctx: Id) -> bool {
        self.component_kind(ctx) == Some(crate::feature::ComponentKind::Part)
    }

    /// Add a part component.
    pub fn add_component(&mut self, name: impl Into<String>) -> Id {
        self.add_part(name)
    }

    // --- Mates (joints). ---

    /// Whether the cross-component reference from `consumer` to `body` is authorised by an explicit external reference.
    pub fn external_authorized(&self, consumer: Id, body: Id) -> bool {
        self.external_ref_for(consumer, body).is_some()
    }

    /// Break an external reference: the geometry stays exactly where it is and the associativity ends.
    ///
    /// Every consumer sketch sitting on a face of the source body is moved onto a snapshot — a fixed datum
    /// plane in the local space of the consumer. Without that, simply deleting the reference would drop the
    /// part into an isolation error ("a sketch on a face of another component's body without an external
    /// reference"). Returns how many sketches were frozen.
    pub fn break_external_ref(&mut self, ref_id: Id) -> usize {
        use crate::feature::SketchPlane;
        let Some(r) = self.external_refs.iter().find(|r| r.id == ref_id) else { return 0 };
        let consumer = r.from_component;
        let Some(body) = r.source_body() else {
            self.remove_external_ref(ref_id);
            return 0;
        };
        let targets: Vec<(usize, Id, crate::feature::FaceKey)> = self
            .sketches
            .iter()
            .enumerate()
            .filter_map(|(si, s)| match s.plane {
                SketchPlane::Face(b, key) if b == body && self.sketch_owner(s.id) == Some(consumer) => Some((si, s.id, key)),
                _ => None,
            })
            .collect();
        let mut frozen = 0;
        for (si, sid, key) in targets {
            // The frame before freezing, in consumer local space; the snapshot has to land exactly on it.
            let live = self.sketch_frame(si);
            let pid = self.snapshot_face_plane_for(consumer, body, &key);
            self.sketches[si].plane = SketchPlane::Datum(pid);
            if let (Some(live), Some(pi)) = (live, self.planes.iter().position(|p| p.id == pid)) {
                // A datum defines only the plane: its 2D zero is the projection of the consumer origin,
                // whereas on the live face it was the projection of the source origin, and a tilted neighbour
                // also picks its axes differently (the dominant-axis rule in `world_aligned`). Without this
                // correction, breaking the link would shift and rotate all the 2D geometry, while freezing
                // has to be invisible. The rotation goes into `rot_deg` and the remainder into the sketch
                // `origin_uv` — the same 2D-zero shift a manual snap to an edge produces.
                let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
                let (po, pn) = (self.planes[pi].origin, self.planes[pi].normal);
                let base0 = crate::feature::PlaneFrame::world_aligned(po, pn, 0.0);
                let rot = dot(live.x, base0.y).atan2(dot(live.x, base0.x)).to_degrees();
                self.planes[pi].rot_deg = rot;
                let base = crate::feature::PlaneFrame::world_aligned(po, pn, rot);
                let d = [live.origin[0] - base.origin[0], live.origin[1] - base.origin[1], live.origin[2] - base.origin[2]];
                self.sketches[si].origin_uv = Some(crate::geom::Point2::new(dot(d, base.x), dot(d, base.y)));
            }
            self.mark_sketch_dirty(sid); // Rebuild bodies on this sketch: its plane became a snapshot.
            frozen += 1;
        }
        self.remove_external_ref(ref_id);
        frozen
    }

    /// Resolve an edge of body `body` by persistent id into its midpoint and tangent, in body local space.
    /// `None` when the edges are not built yet (`regen_edges` is empty) or the id is unknown.
    pub fn resolve_edge(&self, body: Id, edge_id: u32) -> Option<([f64; 3], [f64; 3])> {
        self.regen_edges.get(&body)?.iter().find(|e| e.id == edge_id).map(|e| (e.mid, e.dir))
    }

    /// Home assembly of a mate: the lowest common ancestor of its connector owners.
    ///
    /// A mate belongs to exactly one assembly — between parts of the root it belongs to the root, between
    /// parts of a subassembly it belongs to that subassembly. The parent assembly must neither show nor
    /// account for the mates of its nested subassemblies.
    pub fn joint_home(&self, j: &crate::feature::Joint) -> Option<Id> {
        let oa = self.connector(j.a)?.owner;
        let ob = self.connector(j.b)?.owner;
        self.common_ancestor(oa, ob)
    }

    /// Delete component `id` with everything it holds: subcomponents, bodies, sketches, datums, and the
    /// connectors and joints that reference them.
    ///
    /// Returns the ids of the deleted bodies, so the interface can drop its mesh and shape caches. The root
    /// cannot be deleted and yields an empty list. One method, because this is a topology operation.
    pub fn delete_component(&mut self, id: Id) -> Vec<Id> {
        use crate::feature::FeatureKind;
        if id == self.root || !self.components.iter().any(|c| c.id == id) {
            return Vec::new();
        }
        // The whole subtree: the component itself plus its descendants.
        let mut subtree: std::collections::HashSet<Id> = self.descendants(id).into_iter().collect();
        subtree.insert(id);
        // Classify the timeline nodes of the subtree into bodies, sketches and datums.
        let mut bodies = Vec::new();
        let mut sketches = Vec::new();
        let mut planes = Vec::new();
        let mut points = Vec::new();
        let mut axes = Vec::new();
        for nd in &self.timeline {
            if !nd.parent.is_some_and(|p| subtree.contains(&p)) {
                continue;
            }
            bodies.extend(nd.kind.bodies());
            match nd.kind {
                FeatureKind::Sketch { sketch } => sketches.push(sketch),
                FeatureKind::Plane { plane } => planes.push(plane),
                FeatureKind::DatumPoint { point } => points.push(point),
                FeatureKind::DatumAxis { axis } => axes.push(axis),
                _ => {}
            }
        }
        // Drop the timeline nodes of the subtree (bodies, sketches, datums and features at once).
        self.timeline.retain(|nd| !nd.parent.is_some_and(|p| subtree.contains(&p)));
        // Meshes and the parametric dimensions of the bodies.
        for b in &bodies {
            if let Some(mi) = self.mesh_index(*b) {
                self.remove_mesh(mi);
            }
            self.feat_dims.remove(b);
        }
        // Sketches from the pool.
        for s in &sketches {
            if let Some(si) = self.sketch_index(*s) {
                self.remove_sketch(si);
            }
        }
        // Datums from the pools.
        let (ps, pts, axs): (std::collections::HashSet<Id>, std::collections::HashSet<Id>, std::collections::HashSet<Id>) =
            (planes.into_iter().collect(), points.into_iter().collect(), axes.into_iter().collect());
        self.planes.retain(|p| !ps.contains(&p.id));
        self.datum_points.retain(|p| !pts.contains(&p.id));
        self.datum_axes.retain(|a| !axs.contains(&a.id));
        // Broken connectors and their joints, on two counts: the owner is inside the subtree, or the anchor
        // references a body being deleted. The second case is real because the owner of a connector may be
        // the parent context (a subassembly) while the geometry lives in the part being deleted (see
        // `ancestor_child_of` when a joint is created). Without it a joint survives with a broken anchor,
        // `connector_frame` resolves to the old centroid fingerprint and the solve scatters bodies to
        // garbage coordinates.
        use crate::feature::AnchorRef;
        let dead_bodies: std::collections::HashSet<Id> = bodies.iter().copied().collect();
        let anchor_dead = |a: &AnchorRef| matches!(a, AnchorRef::FaceCenter(b, _) | AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) if dead_bodies.contains(b));
        let dead_conn: std::collections::HashSet<Id> = self.connectors.iter().filter(|c| subtree.contains(&c.owner) || anchor_dead(&c.anchor)).map(|c| c.id).collect();
        self.joints.retain(|j| !dead_conn.contains(&j.a) && !dead_conn.contains(&j.b));
        self.connectors.retain(|c| !dead_conn.contains(&c.id));
        // The regenerate caches go too, as they do when a sketch is deleted. Dropping the meshes while
        // leaving `regen_faces` and `regen_edges` behind produced a body that no timeline node builds yet
        // that still counts as alive for everything that enumerates live bodies. Every deletion path was
        // checked: plane deletion, body cascade and feature deletion were clean, and the ghost remained for
        // sketch deletion (since fixed) and here.
        self.drop_orphan_bodies();
        // External references whose source body or consumer component is inside the subtree are cut.
        let dead_refs: Vec<Id> =
            self.external_refs.iter().filter(|e| subtree.contains(&e.from_component) || e.source_body().and_then(|b| self.body_owner(b)).is_some_and(|o| subtree.contains(&o))).map(|e| e.id).collect();
        self.external_refs.retain(|e| !dead_refs.contains(&e.id));
        // The components themselves.
        self.components.retain(|c| !subtree.contains(&c.id));
        if self.active_component.is_some_and(|a| subtree.contains(&a)) {
            self.active_component = None;
        }
        self.drop_orphan_sources();
        bodies
    }

    /// AN IMPORT SOURCE GOES WITH WHAT CAME OF IT: a file no node of the timeline (an imported solid, a mesh piece)
    /// and no sketch comes of any more is dropped. Reported behaviour: the part a file came in as was deleted and its
    /// source still hung in the tree.
    fn drop_orphan_sources(&mut self) {
        use crate::feature::FeatureKind;
        let mut used: std::collections::HashSet<Id> = self.sketches.iter().filter_map(|s| s.source).collect();
        used.extend(self.timeline.iter().filter_map(|n| match n.kind {
            FeatureKind::Import { source, .. } | FeatureKind::MeshPiece { source, .. } => Some(source),
            _ => None,
        }));
        self.sources.retain(|s| used.contains(&s.id));
    }

    // --- Tree clipboard: copying and cutting sketches, parts and subassemblies. ---

    /// Clone a single sketch (from the tree clipboard) into a target component. Returns the id of the clone.
    pub fn clone_sketch_node(&mut self, sid: Id, target_parent: Id) -> Option<Id> {
        self.clone_sketch_impl(sid, target_parent, &std::collections::HashMap::new())
    }

    /// Re-parent sketch node `sid` under another component (a cut followed by a paste). Returns whether it
    /// succeeded.
    pub fn move_sketch_node(&mut self, sid: Id, target_parent: Id) -> bool {
        if !self.components.iter().any(|c| c.id == target_parent) {
            return false;
        }
        if let Some(ti) = self.timeline_index(sid) {
            self.timeline[ti].parent = Some(target_parent);
            true
        } else {
            false
        }
    }

    /// Deep clone of component `id`, with its whole subtree (subcomponents, sketches, datums, features,
    /// bodies), under `target_parent`.
    ///
    /// Every id is remapped, and every reference (input bodies, sketches, datums, contour `profile`) is
    /// re-pointed at the clones. Bodies are marked dirty, so the application builds them with
    /// `regenerate(kernel)`. Returns the id of the cloned subtree root.
    ///
    /// A within-project clone is a special case of the cross-project `clone_subtree_into`, with the source
    /// being a snapshot of self, so both share one id-remapping engine.
    pub fn clone_component(&mut self, id: Id, target_parent: Id) -> Option<Id> {
        let from = self.clone();
        let copy = Self::clone_subtree_into(self, &from, id, target_parent)?;
        self.name_apart(&[copy], &|n| n.to_string()); // a copy is told from its original: "Part 1 (2)"
        Some(copy)
    }

    /// Extract a product: build a new minimal `Project` whose root assembly holds a deep clone of component
    /// `component` and its subtree. The `.qpart` format is the serialisation of that project.
    ///
    /// Joints, references and connectors leading outside the subtree are not carried over (as in the
    /// `clone_*` methods). Bodies are dirty and are rebuilt by `regenerate(kernel)` on paste. A `component`
    /// equal to the root yields `None`: only a part or a subassembly can be saved as a product.
    pub fn subproject_of(&self, component: Id) -> Option<Project> {
        let mut out = Project { units: self.units, ..Project::default() };
        let out_root = out.ensure_root();
        let new_root = Self::clone_subtree_into(&mut out, self, component, out_root)?;
        out.next_id = out.next_id.max(self.next_id);
        out.active_component = Some(new_root);
        Some(out)
    }

    /// Insert a product: bring the contents of `other` (a `.qpart` mini-project) into `self` under
    /// `target_parent`, the active assembly.
    ///
    /// Every top-level component of `other` (the children of its root) is cloned with a full id remap, so
    /// the result is first-class build-tree nodes, indistinguishable from geometry modelled in place.
    /// Bodies are marked dirty and the application builds them with `regenerate(kernel)`. Returns the id of
    /// the inserted root (the first one when the product has several components). The mirror image of
    /// `clone_component`, with an external project as the source.
    pub fn graft(&mut self, other: &Project, target_parent: Id, shown: &dyn Fn(&str) -> String) -> Option<Id> {
        if !self.components.iter().any(|c| c.id == target_parent) || self.component_is_part(target_parent) {
            return None;
        }
        let tops: Vec<Id> = other.components.iter().filter(|c| c.parent == Some(other.root)).map(|c| c.id).collect();
        let mut first = None;
        for t in tops {
            if let Some(nid) = Self::clone_subtree_into(self, other, t, target_parent) {
                self.name_apart(&[nid], shown); // it keeps its own name, numbered when one reading the same is here
                first.get_or_insert(nid);
            }
        }
        self.next_id = self.next_id.max(other.next_id);
        first
    }

    /// World transform of a body, through its owning component. A body with no owner (a raw import) gets
    /// the identity.
    pub fn body_world_transform(&self, body: Id) -> [f64; 12] {
        match self.body_owner(body) {
            Some(owner) => self.world_transform(owner),
            None => crate::feature::PLACE_IDENTITY,
        }
    }

    /// Transform of component `comp` relative to an ancestor or context `base`, that is
    /// `inv(world(base)) * world(comp)`.
    ///
    /// `base == comp` gives the identity, so a part is drawn at its own zero while being edited from
    /// inside; `base == root` gives absolute world space, which is the assembly overview.
    pub fn relative_transform(&self, comp: Id, base: Id) -> [f64; 12] {
        use crate::feature::{mat_inv12, mat_mul12};
        mat_mul12(&mat_inv12(&self.world_transform(base)), &self.world_transform(comp))
    }

    /// Transform of a body for drawing and picking, relative to the active context `active`.
    pub fn body_display_transform(&self, body: Id, active: Id) -> [f64; 12] {
        match self.body_owner(body) {
            Some(owner) => self.relative_transform(owner, active),
            None => crate::feature::PLACE_IDENTITY,
        }
    }

    /// Local frame of a component base plane, in the local space of that component. Used as an anchor by
    /// sketches, datums and connectors.
    pub fn component_base_frame(&self, plane: crate::feature::BasePlane) -> crate::feature::PlaneFrame {
        plane.frame()
    }

    /// World frame of a component base plane: the local frame carried through `world_transform`.
    pub fn component_base_frame_world(&self, id: Id, plane: crate::feature::BasePlane) -> crate::feature::PlaneFrame {
        plane.frame().transformed(&self.world_transform(id))
    }

    /// Whether the component is a part (and so may hold bodies).
    pub fn component_is_part(&self, id: Id) -> bool {
        self.component_kind(id) == Some(crate::feature::ComponentKind::Part)
    }

    /// Whether bodies may be created in the active context; used to gate the body tools in the interface.
    pub fn can_add_body(&self) -> bool {
        self.ctx_holds_bodies(self.current_ctx())
    }

    /// Owning component of a node reference (a body or a sketch). Planes, datums and anything unknown give
    /// `None`: isolation does not restrict those, since they are not geometry of another part.
    pub fn ref_owner(&self, id: Id) -> Option<Id> {
        self.body_owner(id).or_else(|| self.sketch_owner(id))
    }

    /// Operation using the first closed contour of a sketch against body `src`.
    pub fn add_combine(&mut self, src: Id, sketch: Id, height: f64, op: u8) -> Id {
        self.add_combine_on(src, sketch, 0, crate::model::CombineSpan { height, down: 0.0, extent: crate::feature::Extent::default(), fill: &[] }, op)
    }

    /// Capture geometric snapshots of faces `faces` of body `src` and attach them to feature `fid`, the way
    /// `capture_edge_refs` does for edges.
    pub(crate) fn capture_face_refs(&mut self, fid: Id, src: Id, faces: &[u32]) {
        let Some(fs) = self.regen_faces.get(&src) else { return };
        let refs: Vec<ElemSnapshot> =
            faces.iter().filter_map(|&id| fs.iter().find(|f| f.id == id).map(|f| ElemSnapshot { id, at: [f.centroid.x, f.centroid.y, f.centroid.z], dir: f.normal })).collect();
        if !refs.is_empty() {
            self.face_refs.insert(fid, refs);
        }
    }

    /// Resolve a face reference: by name first, then by a recorded merge, and only then by snapshot.
    ///
    /// The rules match those for edges, for the same reason: a name is an address and a positional number
    /// is not. A number is trusted only while the snapshot agrees with it — as soon as some faces gain
    /// names, the numbering of the rest shifts and a number stored in the file starts pointing at a
    /// different face, silently. The snapshot fallback is used only on an unambiguous match.
    pub fn resolve_face_id(&self, fid: Id, src: Id, stored: u32) -> Option<u32> {
        let fs = self.regen_faces.get(&src)?;
        if fs.is_empty() {
            return None;
        }
        let snap = self.face_refs.get(&fid).and_then(|v| v.iter().find(|s| s.id == stored)).map(|s| (s.at, s.dir));
        let live = |id: u32| fs.iter().find(|f| f.id == id);
        if let Some(f) = live(stored) {
            let ok = crate::names::NameTable::is_named(stored)
                || match snap {
                    None => true,
                    Some((c, _)) => {
                        let d = (f.centroid.x - c[0]).powi(2) + (f.centroid.y - c[1]).powi(2) + (f.centroid.z - c[2]).powi(2);
                        d.sqrt() <= self.face_snap_tol(fs)
                    }
                };
            if ok {
                return Some(stored);
            }
        }
        if let Some(t) = self.names.absorbed_target(stored) {
            if live(t).is_some() {
                return Some(t);
            }
        }
        // The only face of the body: there is nothing to confuse it with, so this is proof rather than
        // similarity.
        if fs.len() == 1 {
            return Some(fs[0].id);
        }
        let (c, n) = snap?;
        let tol = self.face_snap_tol(fs);
        let mut scored: Vec<(f64, u32)> = fs
            .iter()
            .map(|f| {
                let d = ((f.centroid.x - c[0]).powi(2) + (f.centroid.y - c[1]).powi(2) + (f.centroid.z - c[2]).powi(2)).sqrt();
                let dot = (f.normal[0] * n[0] + f.normal[1] * n[1] + f.normal[2] * n[2]).clamp(-1.0, 1.0);
                (d + (1.0 - dot) * 10.0, f.id)
            })
            .collect();
        scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let best = *scored.first()?;
        let second = scored.get(1).map(|x| x.0).unwrap_or(f64::MAX);
        let hit = best.0 < tol && (second - best.0) > tol * 0.6;
        if hit {
            self.snap_rebinds.fetch_add(1, std::sync::atomic::Ordering::Relaxed); // The fallback fired.
            if std::env::var("QYM_SNAP_DEBUG").is_ok() {
                eprintln!("face fallback: feature {fid}, reference {stored} = {} -> {} = {}", self.names.describe(stored), best.1, self.names.describe(best.1));
            }
        }
        hit.then_some(best.1)
    }

    /// Face snapshot tolerance, taken from the bounding box of the body, as for edges: an edit moves
    /// geometry by an amount proportional to the part rather than to a hard-coded number.
    fn face_snap_tol(&self, fs: &[crate::geom::MeshFace]) -> f64 {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for f in fs {
            let c = [f.centroid.x, f.centroid.y, f.centroid.z];
            for k in 0..3 {
                lo[k] = lo[k].min(c[k]);
                hi[k] = hi[k].max(c[k]);
            }
        }
        let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
        (diag * 0.02).max(0.5)
    }

    /// Capture geometric snapshots of edges `edges` of body `src` (from `regen_edges`) and attach them to
    /// feature `fid`, so an edge selection can be repaired when the topology changes. An empty selection
    /// (every edge, or no geometry) captures nothing.
    fn capture_edge_refs(&mut self, fid: Id, src: Id, edges: &[u32]) {
        let Some(es) = self.regen_edges.get(&src) else { return };
        let refs: Vec<ElemSnapshot> = edges.iter().filter_map(|&eid| es.iter().find(|e| e.id == eid).map(|e| ElemSnapshot { id: eid, at: e.mid, dir: e.dir })).collect();
        if !refs.is_empty() {
            self.edge_refs.insert(fid, refs);
        }
    }

    /// Resolve the stored edge ids of feature `fid` against the current edges of body `src`.
    ///
    /// A valid id passes through as is. A stale one (absent from `regen_edges[src]` because the topology
    /// changed) is repaired from the snapshot: the nearest edge by midpoint and direction, but only on an
    /// unambiguous match — the best candidate has to be clearly closer than the second and within
    /// tolerance. Otherwise the edge drops out. Returns the current ids for the kernel.
    pub fn resolve_edge_ids(&self, fid: Id, src: Id, stored: &[u32]) -> Vec<u32> {
        self.resolve_edge_ids_in(self.regen_edges.get(&src).map(|v| v.as_slice()).unwrap_or(&[]), fid, stored)
    }

    /// Like `resolve_edge_ids`, but against an explicit list of current edges: regenerate has fresh ones
    /// from the kernel, while `regen_edges` may be empty or stale in the middle of a pass.
    ///
    /// The repair metric is the distance from the old midpoint to the candidate segment, not midpoint to
    /// midpoint. A merged collinear edge (produced by seam stitching) contains the old midpoint, giving an
    /// unambiguous match, whereas midpoint-to-midpoint drifts by half the merge length and the edge drops
    /// out.
    pub fn resolve_edge_ids_in(&self, es: &[crate::geom::MeshEdge], fid: Id, stored: &[u32]) -> Vec<u32> {
        if es.is_empty() {
            return stored.to_vec();
        }
        let snaps = self.edge_refs.get(&fid);
        // The tolerance comes from the scale of the body, not from a hard-coded number.
        //
        // A fixed "accept an edge within 0.5 mm of its former place" has a measured price: moving one
        // sketch point by 1.5 mm changed 22 edge names, and a fillet over 36 selected edges found 20 —
        // silently, without an error. The repair held snapshots of every lost edge and refused to use them:
        // right by its own yardstick, wrong in substance, since it is the same edge that simply moved with
        // the edit.
        //
        // An edit moves geometry by an amount proportional to the part, not to half a millimetre, so the
        // part is the yardstick: the tolerance is a fraction of its bounding box, with a floor for tiny
        // bodies.
        //
        // The fraction was chosen by measurement. At 0.05 the repair returned 34 of 36 edges but also
        // pulled in edges the fillet cannot build on, so the whole feature failed. At 0.02 all 36 are found
        // and nothing breaks.
        //
        // The unambiguity requirement stays and grows with the tolerance: a fillet on the wrong edge is
        // worse than a lost one, so the second candidate has to be clearly worse than the first.
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for e in es {
            for k in 0..3 {
                lo[k] = lo[k].min(e.mid[k]);
                hi[k] = hi[k].max(e.mid[k]);
            }
        }
        let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
        let tol = (diag * 0.02).max(0.5);
        let gap = (tol * 0.6).max(0.3);
        let mut out = Vec::with_capacity(stored.len());
        for &eid in stored {
            // A positional number is trusted only while the snapshot agrees with it.
            //
            // A name is an address: while it lives, it denotes that exact element. A positional number is
            // not an address; it means "this many along in today's numbering". As soon as naming improves
            // and some elements gain names, the numbering of the rest shifts and a number stored in a file
            // starts pointing at a different edge, silently.
            //
            // Measured: enabling names for a thread relief moved all 36 fillet references, some by 118 mm,
            // while neither the references nor the geometry had changed.
            //
            // So the number needs a witness: the place snapshot taken at the same time as the reference. If
            // they agree, the number is used (within a session the numbering does not move, so the behaviour
            // is unchanged). If they disagree, the number proves nothing and resolution falls through to
            // name translation and snapshot repair. With no snapshot the number is trusted as before, there
            // being no other witness.
            let numeric_ok = crate::names::NameTable::is_named(eid)
                || match (es.iter().find(|e| e.id == eid), snaps.and_then(|v| v.iter().find(|s| s.id == eid))) {
                    (Some(cur), Some(snap)) => {
                        let mid = snap.at;
                        let d = ((cur.mid[0] - mid[0]).powi(2) + (cur.mid[1] - mid[1]).powi(2) + (cur.mid[2] - mid[2]).powi(2)).sqrt();
                        d <= tol
                    }
                    _ => true,
                };
            if numeric_ok && es.iter().any(|e| e.id == eid) {
                if !out.contains(&eid) {
                    out.push(eid);
                }
                continue;
            }
            // Name translation through a recorded merge, before and instead of geometry.
            //
            // An edge name is a pair of face names. When a union merges two coplanar faces into one, one
            // face name yields to the other, so the edge is named differently even though it is the same
            // edge of the same body. The kernel reported which name yielded to which, so this is not a
            // search for something similar but a translation of a recorded fact: old pair to new pair. The
            // geometric fallback stays after it, and only for what has no translation.
            if let Some(t) = self.names.absorbed_target(eid) {
                if es.iter().any(|e| e.id == t) {
                    if !out.contains(&t) {
                        out.push(t);
                    }
                    continue;
                }
            }
            // An edge between a piece and its neighbour is the same edge. Once an operation starts cutting
            // a face, the edge is named by the pair "piece plus neighbour" while the reference remembers
            // "whole plus neighbour" (or the other way round). The canonical form of the pair is compared,
            // and the match is accepted only when there is a single candidate: ambiguity is worse than loss,
            // because a fillet on the wrong edge cannot be undone by the user.
            //
            // The ordinal within a pair does not take part in the decision. The edge number inside a pair of
            // faces is assigned by sorting on kernel ids, that is, by order rather than by recipe. When a
            // pair of faces carries several edges, names cannot tell them apart, and taking "the one whose
            // ordinal matches" is a coin flip: measured, that substituted a twin edge and an R2.0 fillet
            // stopped building entirely. So the match is taken only when the pair has exactly one edge; the
            // ambiguous case goes to the geometric fallback, which at least looks at the place.
            if let Some((pair, _)) = self.names.canonical_edge(eid) {
                let mut same = es.iter().filter(|e| self.names.canonical_edge(e.id).map(|(p, _)| p) == Some(pair));
                if let (Some(one), None) = (same.next(), same.next()) {
                    let id = one.id;
                    if !out.contains(&id) {
                        out.push(id);
                    }
                    continue;
                }
            }
            if let Some(&ElemSnapshot { at: mid, dir, .. }) = snaps.and_then(|v| v.iter().find(|s| s.id == eid)) {
                let seg_dist = |e: &crate::geom::MeshEdge| -> f64 {
                    let (a, b) = (e.a, e.b);
                    let v = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                    let l2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
                    if l2 < 1e-18 {
                        return ((e.mid[0] - mid[0]).powi(2) + (e.mid[1] - mid[1]).powi(2) + (e.mid[2] - mid[2]).powi(2)).sqrt();
                    }
                    let t = (((mid[0] - a[0]) * v[0] + (mid[1] - a[1]) * v[1] + (mid[2] - a[2]) * v[2]) / l2).clamp(0.0, 1.0);
                    let p = [a[0] + v[0] * t, a[1] + v[1] * t, a[2] + v[2] * t];
                    ((p[0] - mid[0]).powi(2) + (p[1] - mid[1]).powi(2) + (p[2] - mid[2]).powi(2)).sqrt()
                };
                let mut scored: Vec<(f64, u32)> = es
                    .iter()
                    .map(|e| {
                        let dot = (e.dir[0] * dir[0] + e.dir[1] * dir[1] + e.dir[2] * dir[2]).abs().min(1.0);
                        (seg_dist(e) + (1.0 - dot) * 10.0, e.id)
                    })
                    .collect();
                scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                if let Some(&(best, bid)) = scored.first() {
                    let second = scored.get(1).map(|x| x.0).unwrap_or(f64::MAX);
                    if best < tol && (second - best) > gap && !out.contains(&bid) {
                        self.snap_rebinds.fetch_add(1, std::sync::atomic::Ordering::Relaxed); // The fallback fired.
                        if std::env::var("QYM_SNAP_DEBUG").is_ok() {
                            eprintln!("edge fallback: feature {fid}, reference {} = {} -> {bid} = {}", eid, self.names.describe(eid), self.names.describe(bid));
                        }
                        out.push(bid); // Unambiguously repaired (for example a fragment mapping onto a merged edge).
                    }
                }
            }
        }
        out
    }

    /// Add a chamfer on edges of body `src` (an empty `edges` means every edge). Returns the id of the result.
    pub fn add_chamfer(&mut self, src: Id, dist: f64, edges: Vec<u32>) -> Id {
        use crate::feature::ChamferMode;
        self.add_chamfer_ex(src, dist, ChamferShape { mode: ChamferMode::Symmetric, d2: 0.0, flip: false, ref_face: 0 }, edges)
    }

    /// Add a shell of body `src`: remove faces `faces` and leave a wall of `thickness`. Returns the id of the
    /// result.
    pub fn add_shell(&mut self, src: Id, thickness: f64, faces: Vec<u32>, outward: bool) -> Id {
        let side = if outward { crate::feature::ShellSide::Outward } else { crate::feature::ShellSide::Inward };
        self.add_shell_mode(src, thickness, faces, side)
    }

    /// Add a linear pattern of body `src`: `count` copies spaced by (dx, dy, dz). Returns the result id.
    pub fn add_linear_array(&mut self, src: Id, dx: f64, dy: f64, dz: f64, count: u32) -> Id {
        self.add_linear_array_grid(src, ArrayAxis { d: [dx, dy, dz], count }, ArrayAxis::none())
    }

    /// Grid pattern: direction one (`d1` by `count`) and direction two (`d2` by `count2`).
    #[allow(clippy::too_many_arguments)]
    pub fn add_linear_array_grid(&mut self, src: Id, a: ArrayAxis, b: ArrayAxis) -> Id {
        let ([dx, dy, dz], count) = (a.d, a.count);
        let ([dx2, dy2, dz2], count2) = (b.d, b.count);
        self.add_linear_array_grid3(src, [ArrayAxis { d: [dx, dy, dz], count }, ArrayAxis { d: [dx2, dy2, dz2], count: count2 }, ArrayAxis::none()])
    }

    /// Add a circular pattern of body `src`: `count` copies over `angle` degrees. `axis` is a datum axis id,
    /// or 0 for world Z.
    pub fn add_circular_array(&mut self, src: Id, count: u32, angle: f64) -> Id {
        self.add_circular_array_axis(src, count, angle, 0)
    }

    /// Mirrored component in an assembly: a new sibling of the source whose body is the reflection of the
    /// active body of `src_comp` through the world plane `wo`/`wn` (a click on XY, XZ, YZ, a datum or a
    /// face).
    ///
    /// The shape is associative — editing the source rebuilds it — while the placement is free, so the
    /// mirrored component can be moved with the gizmo. A part yields one mirrored part.
    ///
    /// A subassembly yields a mirrored sibling subassembly reflected as one rigid whole. The world mirror
    /// plane is used once, on the assembly itself: its placement is the reflected world zero of the source,
    /// with the orientation preserved. Inside it, the rotation of each part relative to the subassembly is
    /// left exactly as in the original, while its local offset is reflected by the same formula in the
    /// local space of the subassembly (`add_mirror_part_rigid`). Without that, the mutual arrangement of
    /// the parts — which one is left of which — would not be mirrored with the whole and would end up on
    /// the original side. Returns the ids of the mirror nodes.
    pub fn add_mirror_component(&mut self, src_comp: Id, wo: [f64; 3], wn: [f64; 3]) -> Vec<Id> {
        if self.component_is_part(src_comp) {
            return vec![self.add_mirror_part(src_comp, wo, wn)];
        }
        let (src_name, src_parent) = self.components.iter().find(|c| c.id == src_comp).map(|c| (c.name.clone(), c.parent)).unwrap_or(("name-assembly".into(), None));
        let saved = self.active_component;
        self.active_component = src_parent.or(saved);
        let asm = self.add_assembly(format!("name-mirror-of#{src_name}"));
        self.active_component = saved;
        // One reflected frame for the whole subassembly: the orientation of the source, with the world zero
        // reflected through (wo, wn) — the same formula as for a single part, applied to the subassembly as
        // one rigid body.
        let ssrc = self.world_transform(src_comp);
        let n_sa = crate::feature::apply12_dir(&crate::feature::mat_inv12(&ssrc), wn); // Normal in subassembly local space, reused for the child offsets.
        let t = [ssrc[3], ssrc[7], ssrc[11]];
        let d = (t[0] - wo[0]) * wn[0] + (t[1] - wo[1]) * wn[1] + (t[2] - wo[2]) * wn[2];
        let t_new = [t[0] - 2.0 * d * wn[0], t[1] - 2.0 * d * wn[1], t[2] - 2.0 * d * wn[2]];
        let mut rfull = ssrc;
        rfull[3] = t_new[0];
        rfull[7] = t_new[1];
        rfull[11] = t_new[2];
        let ap = self.components.iter().find(|c| c.id == asm).and_then(|c| c.parent).map(|pp| self.world_transform(pp)).unwrap_or(crate::feature::PLACE_IDENTITY);
        self.set_component_transform(asm, crate::feature::mat_mul12(&crate::feature::mat_inv12(&ap), &rfull));
        // Every part in the subtree that holds bodies (`src_comp` itself is an assembly, so walk its descendants).
        let parts: Vec<Id> = self.descendants(src_comp).into_iter().filter(|&c| self.component_is_part(c) && self.timeline.iter().any(|n| n.parent == Some(c) && n.kind.body().is_some())).collect();
        let mut out = Vec::new();
        for part in parts {
            out.push(self.add_mirror_part_rigid(part, src_comp, n_sa, wn, asm));
        }
        out
    }

    /// Datum plane offset from a face by `dist`, used by mirrors and by sketches on a face. Associative: it
    /// is resolved during regenerate.
    pub fn add_plane_from_face(&mut self, body: Id, face: crate::feature::FaceKey, dist: f64) -> Id {
        let wp = WorkPlane { name: "name-plane-from-face".into(), def: PlaneDef::OffsetFace { body, face, dist }, ..Default::default() };
        self.add_plane(wp)
    }

    /// Snapshot a face of another component's body into a fixed (`Manual`) datum plane owned by the active
    /// component.
    ///
    /// Unlike a live external reference — associative top-down design that pins a part to a neighbour's
    /// face — this is a one-off snapshot: the world frame of the source face is converted into the local
    /// space of the consumer, through its current `world_transform`, and stored as coordinates. After that
    /// the part moves independently and is mated by hand. Returns the id of the new datum plane, whose
    /// parent is the active context, that is, the consumer.
    pub fn snapshot_face_plane(&mut self, body: Id, face: &crate::feature::FaceKey) -> Id {
        let consumer = self.active_ctx();
        self.snapshot_face_plane_for(consumer, body, face)
    }

    /// Like [`Project::snapshot_face_plane`], but with the consumer given explicitly. Breaking an external
    /// reference does not happen from the active context: the part to repair is the one that held the
    /// reference, wherever the current context happens to be.
    pub fn snapshot_face_plane_for(&mut self, consumer: Id, body: Id, face: &crate::feature::FaceKey) -> Id {
        use crate::feature::{apply12, apply12_dir, mat_inv12, mat_mul12};
        // The face resolves in source-body local space: source local, then world, then consumer local.
        let (c, n) = self.resolve_face(body, face);
        let src_to_world = self.body_world_transform(body);
        let world_to_consumer = mat_inv12(&self.world_transform(consumer));
        let m = mat_mul12(&world_to_consumer, &src_to_world);
        let origin = apply12(&m, c);
        let normal = apply12_dir(&m, n);
        let wp = WorkPlane { name: "name-plane-from-face-frozen".into(), origin, normal, rot_deg: 0.0, def: PlaneDef::Manual, ..Default::default() };
        self.add_plane(wp)
    }

    /// Datum plane offset from a base plane (XY, XZ, YZ) by `dist` (parametric through the `dist` feature
    /// dimension).
    pub fn add_offset_plane(&mut self, base: crate::feature::BasePlane, dist: f64) -> Id {
        let wp = WorkPlane { name: "name-plane".into(), def: PlaneDef::OffsetBase { base, dist }, ..Default::default() };
        self.add_plane(wp)
    }

    /// Datum plane offset from another datum plane `src` by `dist` (parametric).
    pub fn add_offset_from_plane(&mut self, src: Id, dist: f64) -> Id {
        let wp = WorkPlane { name: "name-plane".into(), def: PlaneDef::OffsetPlane { plane: src, dist }, ..Default::default() };
        self.add_plane(wp)
    }

    /// Datum point at coordinates `at` (parametric through the `x`, `y` and `z` feature dimensions).
    pub fn add_point_at(&mut self, at: [f64; 3]) -> Id {
        self.add_datum_point(DatumPoint { name: "name-datum-point".into(), at, ..Default::default() })
    }

    /// Datum point bound associatively to a vertex of a body (an endpoint of a persistent edge): it travels
    /// with the vertex when the source is rebuilt. `at` is the initial snapshot in body local space and is
    /// refined by resolution.
    pub fn add_point_at_vertex(&mut self, at: [f64; 3], body: Id, edge: u32, end: bool) -> Id {
        self.add_datum_point(DatumPoint { name: "name-datum-point".into(), at, def: PointDef::AtVertex { body, edge, end }, ..Default::default() })
    }

    /// Manual datum axis: an origin and a direction (normalised at the point of use).
    pub fn add_axis_manual(&mut self, origin: [f64; 3], dir: [f64; 3]) -> Id {
        self.add_datum_axis(DatumAxis { name: "name-datum-axis".into(), def: AxisDef::Manual { origin, dir }, ..Default::default() })
    }

    /// Datum axis through two datum points `a` and `b` (parametric: origin = a and dir = norm(b - a), both
    /// resolved during regenerate).
    pub fn add_axis_two_points(&mut self, a: Id, b: Id) -> Id {
        self.add_datum_axis(DatumAxis { name: "name-datum-axis".into(), def: AxisDef::TwoPoints { a, b }, ..Default::default() })
    }

    /// Datum axis bound associatively to a straight edge of a body (persistent id): it travels with the edge
    /// when the body is rebuilt.
    pub fn add_axis_from_edge(&mut self, body: Id, edge: u32) -> Id {
        self.add_datum_axis(DatumAxis { name: "name-datum-axis".into(), def: AxisDef::FromEdge { body, edge }, ..Default::default() })
    }

    /// Datum axis bound associatively to the axis of a cylindrical or conical face of a body (persistent
    /// id).
    pub fn add_axis_from_face(&mut self, body: Id, face: u32) -> Id {
        self.add_datum_axis(DatumAxis { name: "name-datum-axis".into(), def: AxisDef::FromFace { body, face }, ..Default::default() })
    }

    /// Add a hole to body `src` (a cylinder of `diameter` and `depth`, at `point` along `normal`).
    ///
    /// The hole is placed by a face reference (`face`, with a persistent id): the centre and normal are
    /// resolved from that face on every rebuild, so the hole travels with the face. The stored
    /// `face.centroid` and `face.normal` are kept as a fallback fingerprint.
    pub fn add_hole(&mut self, src: Id, face: crate::feature::FaceKey, diameter: f64, depth: f64) -> Id {
        self.add_hole_typed(src, face, HoleTool { kind: 0, diameter, depth, dia2: 0.0, depth2: 0.0 })
    }

    /// Copy of the project without the bytes of the embedded sources. The `sources` records themselves stay,
    /// with their name, extension and an empty `data`.
    ///
    /// Used for undo snapshots: import originals never change yet weigh tens of megabytes, so 40 undo steps
    /// on a real assembly would hold gigabytes and copy about 90 MB on every edit. The bytes are put back
    /// by [`Project::take_source_data_from`].
    pub fn clone_without_source_data(&self) -> Project {
        let mut p = self.clone();
        for s in &mut p.sources {
            s.data = Vec::new();
        }
        p
    }

    /// A STATE BROUGHT BACK BY UNDO OR REDO HANDS OUT NO ID THE LIVE DOCUMENT HAS HANDED OUT ALREADY: the counter goes on
    /// from the higher of the two. Brought back as it was, the counter gave the next node the id of the body the undo
    /// took away, and a pick still held on that body named the new node instead - a shell laid on its own body, red.
    pub fn keep_ids_past(&mut self, live: &Project) {
        self.next_id = self.next_id.max(live.next_id);
    }

    /// Take the source bytes from `live` (matched by id) into this project: the inverse of
    /// [`Project::clone_without_source_data`], used when restoring a snapshot. Sources no longer present in
    /// the live project stay empty, since nothing will read them.
    pub fn take_source_data_from(&mut self, live: &mut Project) {
        for s in &mut self.sources {
            if s.data.is_empty() {
                if let Some(src) = live.sources.iter_mut().find(|l| l.id == s.id) {
                    s.data = std::mem::take(&mut src.data);
                }
            }
        }
    }

    /// Take the placement from the live document: where the components stand and what the mates were told
    /// to hold.
    ///
    /// Needed wherever the document is replaced by a result computed on a copy: a background rebuild takes a
    /// snapshot into a thread and returns a whole document. Its geometry is fresh, which is what the rebuild
    /// was for, but its placement is stale, taken at the moment the snapshot was sent. Meanwhile the pointer
    /// was dragging a component, and accepting the stale placement discards that work.
    ///
    /// The alternative — discarding the result on such a collision — produces two visible symptoms: the
    /// discarded result immediately asks to be recomputed, so the modal rebuild window flashes on every
    /// drag step (measured: eight drag steps, eight rebuild requests), and component placements only catch
    /// up once the pointer stops and the loop opens, which reads as rubber-banding.
    ///
    /// What counts as placement: the transform of a component and the specified value of a mate (`drive`) —
    /// exactly what dragging changes. The mating side (`flip`) counts too: the solver chooses it from the
    /// current placement, so taking it from an older copy means restoring an answer chosen for a different
    /// placement.
    pub fn take_placement_from(&mut self, live: &Project) {
        for c in &mut self.components {
            if let Some(l) = live.components.iter().find(|l| l.id == c.id) {
                c.transform = l.transform;
            }
        }
        for j in &mut self.joints {
            if let Some(l) = live.joints.iter().find(|l| l.id == j.id) {
                // A slot given by an expression keeps the value the rebuild evaluated: the live one is the old
                // value of the parameter. Reported behaviour: a hinge angle typed as pa stayed at 90 deg after
                // pa became 45.
                let exprs = self.feat_dims.get(&j.id);
                for (slot, key) in ["angle", "offset", "offset2"].into_iter().enumerate() {
                    if !exprs.and_then(|d| d.get(key)).is_some_and(|e| !e.trim().is_empty()) {
                        j.drive[slot] = l.drive[slot];
                    }
                }
                j.flip = l.flip;
                j.roll_flip = l.roll_flip;
                j.flip_decided = l.flip_decided;
            }
        }
    }

    /// Cheap state key of the model: has anything worth saving changed.
    ///
    /// Computing such a key by RON-serialising half the project on every frame is both expensive and
    /// incomplete: it misses structure, so creating an empty part, renaming it, moving a component, adding
    /// joints or external references would not make the project dirty, and closing the window would not ask
    /// about unsaved work.
    ///
    /// This is manual hashing without a single allocation: structure (components, timeline, datums, joints,
    /// parameters) plus a cheap fingerprint of the sketches (points, counts, dimension values). Derived
    /// data is excluded (`regen_faces`, `regen_edges`, `regen_errors`, mesh geometry), and so are the source
    /// bytes — they never change, so id, name and size suffice.
    pub fn state_key(&self) -> u64 {
        self.key_of(true)
    }

    /// What the timeline is built from: a state key without the placement.
    ///
    /// "Is there unsaved work" and "does anything need rebuilding" are different questions, and confusing
    /// them is expensive. Dragging a component along a degree of freedom changes the placement (component
    /// transforms, mate values) and changes no body at all: the timeline is built from recipes, sketches and
    /// parameters, not from where a component stands. Comparing the full document key makes the scheduler
    /// read every drag frame as "the document moved, rebuild it".
    ///
    /// Measured on that path: eight drag steps produced eight rebuild requests, that is, eight flashes of
    /// the modal rebuild window.
    ///
    /// Placement that does affect geometry is not lost here: a sketch on a face of another part is an
    /// external reference, and moving the source marks its consumers dirty
    /// (`mark_external_consumers_dirty`). The scheduler queries those marks separately, so that rebuild
    /// still happens — once, when the component is released, instead of twenty times a second.
    pub fn rebuild_key(&self) -> u64 {
        self.key_of(false)
    }

    /// Shared computation of both keys; they must not diverge, so there is exactly one implementation.
    /// `placement` selects whether the placement is included (where components stand and what the mates were
    /// told to hold).
    fn key_of(&self, placement: bool) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let bits = |v: f64, h: &mut std::collections::hash_map::DefaultHasher| v.to_bits().hash(h);
        self.next_id.hash(&mut h);
        self.root.hash(&mut h);
        self.active_component.hash(&mut h);
        self.rollback.hash(&mut h);
        (self.units as u8).hash(&mut h);
        // Components: name, kind, parent, visibility, grounding and placement.
        for c in &self.components {
            (c.id, &c.name, c.kind as u8, c.parent, c.visible, c.grounded).hash(&mut h);
            if placement {
                for v in c.transform {
                    bits(v, &mut h);
                }
            }
        }
        // The whole timeline, feature parameters included.
        //
        // Leaving the parameters out and relying on the application's `geom_rev` — a revision of the drawing
        // cache — makes "has the document changed" answer yes to any rebuild: opening a project, touching
        // nothing and closing it asks whether to save. Three patches in a row treated the symptom; the cause
        // was that the document key did not describe the document.
        //
        // The parameters are taken by serialising the recipe, which is complete by construction. A hand-
        // written field list across forty feature kinds would be cheaper per tick and more dangerous in
        // substance — a forgotten field means a lost edit, which is worse than one extra question. The cost
        // is measured: 1.25 ms over a thousand nodes, that is, hundredths of a millisecond on a real part.
        for n in &self.timeline {
            (n.id, &n.name, n.parent, n.suppressed, n.kind.body()).hash(&mut h);
            ron::ser::to_string(&n.kind).unwrap_or_default().hash(&mut h);
        }
        let mut dims: Vec<(&Id, &std::collections::HashMap<String, String>)> = self.feat_dims.iter().collect();
        dims.sort_by_key(|(k, _)| **k);
        for (id, m) in dims {
            id.hash(&mut h);
            let mut kv: Vec<(&String, &String)> = m.iter().collect();
            kv.sort();
            kv.hash(&mut h);
        }
        // Sketches: points, counts and dimension values, so an edited number is visible before the solve
        // runs.
        for s in &self.sketches {
            (s.id, &s.name, s.entities.len(), s.constraints.len(), s.splines.len()).hash(&mut h);
            std::mem::discriminant(&s.plane).hash(&mut h);
            if let crate::feature::SketchPlane::Datum(p) = s.plane {
                p.hash(&mut h);
            }
            if let crate::feature::SketchPlane::Face(b, _) = s.plane {
                b.hash(&mut h);
            }
            for p in &s.points {
                p.id.hash(&mut h);
                bits(p.x, &mut h);
                bits(p.y, &mut h);
            }
            // a line turned into construction leaves the profile; unseen here, the turn was no step and undo lost it
            for e in &s.entities {
                (e.id, e.construction).hash(&mut h);
            }
            for c in &s.constraints {
                if let Some(d) = c.dim_value() {
                    bits(d, &mut h);
                }
                // A LABEL MOVED IS AN EDIT OF THE DOCUMENT - it goes into the file - but not of what is built: counted
                // with the placement, so dragging a label asks to save and rebuilds nothing
                if placement {
                    for v in c.label_place().unwrap_or([0.0; 3]) {
                        bits(v, &mut h);
                    }
                }
            }
            // labels: a placed or retyped label is a change of the document like any line
            for t in &s.texts {
                (t.id, &t.text, t.construction).hash(&mut h);
                for v in [t.x, t.y, t.height, t.angle] {
                    bits(v, &mut h);
                }
            }
        }
        // Datums and planes.
        for p in &self.planes {
            (p.id, &p.name).hash(&mut h);
            for v in p.origin.iter().chain(p.normal.iter()) {
                bits(*v, &mut h);
            }
            bits(p.rot_deg, &mut h);
        }
        for d in &self.datum_points {
            (d.id, &d.name).hash(&mut h);
            for v in d.at {
                bits(v, &mut h);
            }
        }
        for a in &self.datum_axes {
            (a.id, &a.name).hash(&mut h);
            for v in a.origin().iter().chain(a.dir().iter()) {
                bits(*v, &mut h);
            }
        }
        // Assembly: connectors, joints, external references.
        for c in &self.connectors {
            (c.id, c.owner).hash(&mut h);
        }
        for j in &self.joints {
            (j.id, &j.name, j.a, j.b, j.kind as u8).hash(&mut h);
            // The offset and angle of a mate are placement too: they move a component and touch no body.
            // They belong in the "is there unsaved work" key and not in the "does it need rebuilding" one.
            if placement {
                bits(j.angle, &mut h);
                bits(j.offset, &mut h);
                bits(j.offset2, &mut h);
                // Specified values, limits and "hold as built" are part of the document, not derived data.
                //
                // Leaving them out lets a mate whose specified value moved nothing (the value is unchanged
                // and the component already stands there) pass silently: the document counts as saved and
                // closing asks nothing, although an edit was made that the file does not know about.
                //
                // The mating side (`flip`, `roll_flip`, `flip_decided`) is deliberately excluded: the solver
                // writes it, and including it would declare every solve an edit, outside any operation
                // boundary.
                for slot in 0..3 {
                    if let Some(v) = j.drive[slot] {
                        bits(v, &mut h)
                    }
                    if let Some(v) = j.limit_min[slot] {
                        bits(v, &mut h)
                    }
                    if let Some(v) = j.limit_max[slot] {
                        bits(v, &mut h)
                    }
                    (j.drive[slot].is_some(), j.limit_min[slot].is_some(), j.limit_max[slot].is_some()).hash(&mut h);
                }
                j.global.hash(&mut h);
                if let Some(m) = &j.as_built {
                    for v in m {
                        bits(*v, &mut h);
                    }
                }
                j.as_built.is_some().hash(&mut h);
            }
        }
        // relations between mates move components and touch no body: their numbers are placement, like a mate's angle
        for r in &self.relations {
            (r.id, &r.name, r.kind as u8, r.a, r.slot_a, r.b, r.slot_b).hash(&mut h);
            if placement {
                bits(r.value, &mut h);
                bits(r.phase, &mut h);
                r.reversed.hash(&mut h);
            }
        }
        for r in &self.external_refs {
            (r.id, r.from_component, r.source_body()).hash(&mut h);
        }
        // Parameters and named dimensions (text, and cheap to hash).
        for p in &self.parameters {
            (&p.name, &p.expr).hash(&mut h);
        }
        for d in &self.named_dims {
            d.name.hash(&mut h);
            match &d.target {
                DimTarget::Sketch { sketch, refs } => (0u8, sketch, refs).hash(&mut h),
                DimTarget::Feature { node, key } => (1u8, node, key).hash(&mut h),
            }
        }
        // Bodies and meshes: membership and names only. The geometry is derived from the timeline and was
        // already accounted for by the nodes.
        for b in &self.bodies {
            (b.id, &b.name, b.visible).hash(&mut h);
        }
        let mut imported: Vec<Id> = self.imported_bodies.iter().copied().collect();
        imported.sort_unstable();
        imported.hash(&mut h);
        // Import sources: id, name and size, but not the bytes — tens of megabytes of data that never
        // change.
        for s in &self.sources {
            (s.id, &s.name, &s.ext, s.data.len()).hash(&mut h);
        }
        h.finish()
    }

    /// Fingerprint of a node recipe, used to judge whether its geometry could have changed between two
    /// project states (undo and redo).
    ///
    /// The fingerprint holds only what the body is built from: the `kind` itself (the feature parameters),
    /// the contents of the input sketches and of the planes they sit on, and the dimension expressions of
    /// the feature. Meshes, faces and bodies are derived and are not part of it.
    pub fn node_recipe_key(&self, node_id: Id) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let Some(n) = self.timeline.iter().find(|n| n.id == node_id) else { return 0 };
        ron::ser::to_string(&n.kind).unwrap_or_default().hash(&mut h);
        n.suppressed.hash(&mut h);
        n.parent.hash(&mut h);
        for inp in n.kind.inputs() {
            if let Some(s) = self.sketches.iter().find(|s| s.id == inp) {
                ron::ser::to_string(s).unwrap_or_default().hash(&mut h);
                if let crate::feature::SketchPlane::Datum(pid) = s.plane {
                    if let Some(pl) = self.planes.iter().find(|p| p.id == pid) {
                        ron::ser::to_string(pl).unwrap_or_default().hash(&mut h);
                    }
                }
            }
        }
        if let Some(d) = self.feat_dims.get(&node_id) {
            let mut kv: Vec<(&String, &String)> = d.iter().collect();
            kv.sort();
            kv.hash(&mut h);
        }
        h.finish()
    }

    /// Every id a node depends on, that is, everything that has to be produced before it: bodies and
    /// sketches (`inputs`) plus datum references — a sketch on a datum or a face, an offset plane, an axis
    /// through two points or along an edge or face, and a mirror, revolve or pattern around a datum.
    fn node_required_refs(&self, kind: &crate::feature::FeatureKind) -> Vec<Id> {
        use crate::feature::{FeatureKind as FK, SketchPlane};
        let mut r = kind.inputs();
        match *kind {
            FK::Sketch { sketch } => {
                if let Some(s) = self.sketches.iter().find(|s| s.id == sketch) {
                    match s.plane {
                        SketchPlane::Datum(pid) => r.push(pid),
                        SketchPlane::Face(body, _) => r.push(body),
                        SketchPlane::World(_) => {}
                    }
                }
            }
            FK::Plane { plane } => {
                if let Some(p) = self.planes.iter().find(|p| p.id == plane) {
                    match p.def {
                        PlaneDef::OffsetPlane { plane: src, .. } => r.push(src),
                        PlaneDef::OffsetFace { body, .. } => r.push(body),
                        _ => {}
                    }
                }
            }
            FK::DatumAxis { axis } => {
                if let Some(a) = self.datum_axes.iter().find(|a| a.id == axis) {
                    match a.def {
                        AxisDef::TwoPoints { a: pa, b: pb } => {
                            r.push(pa);
                            r.push(pb);
                        }
                        AxisDef::FromEdge { body, .. } | AxisDef::FromFace { body, .. } => r.push(body),
                        AxisDef::Manual { .. } => {}
                    }
                }
            }
            FK::Mirror { datum, .. } if datum != 0 => r.push(datum),
            FK::Revolve { axis_datum, .. } if axis_datum != 0 => r.push(axis_datum),
            FK::CircularArray { axis, .. } if axis != 0 => r.push(axis),
            _ => {}
        }
        r
    }

    /// Span of a body along a normal: the projection of the bounding box of `body` onto axis `n`, measured
    /// from `origin`.
    ///
    /// Extracted because the extent computation needed it twice with identical code: once for "through the
    /// whole body" and once to check whether the tool end landed exactly on the far face.
    fn body_span_along(&self, body: Id, origin: [f64; 3], n: [f64; 3]) -> Option<(f64, f64)> {
        let bb = self.mesh_index(body).and_then(|i| self.bodies[i].mesh.bounds())?;
        let corners = [
            [bb.min.x, bb.min.y, bb.min.z],
            [bb.max.x, bb.min.y, bb.min.z],
            [bb.min.x, bb.max.y, bb.min.z],
            [bb.max.x, bb.max.y, bb.min.z],
            [bb.min.x, bb.min.y, bb.max.z],
            [bb.max.x, bb.min.y, bb.max.z],
            [bb.min.x, bb.max.y, bb.max.z],
            [bb.max.x, bb.max.y, bb.max.z],
        ];
        let (mut tmin, mut tmax) = (f64::MAX, f64::MIN);
        for c in corners {
            let t = (c[0] - origin[0]) * n[0] + (c[1] - origin[1]) * n[1] + (c[2] - origin[2]) * n[2];
            tmin = tmin.min(t);
            tmax = tmax.max(t);
        }
        Some((tmin, tmax))
    }

    /// Remove a body from view: the mesh and the face and edge caches go together.
    ///
    /// Three lines repeated in four places of the rebuild, where any copy could forget one of them and leave
    /// the face cache and the edge cache disagreeing about which bodies exist at all. The feature itself
    /// stays in the timeline: this hides a result (rollback, suppression) rather than deleting a node.
    fn drop_body_from_view(&mut self, b: Id) {
        if let Some(mi) = self.mesh_index(b) {
            self.remove_mesh(mi);
        }
        self.regen_faces.remove(&b);
        self.regen_edges.remove(&b);
    }

    /// Seams: a face with no provenance is identified by its neighbours, exactly as an edge is.
    ///
    /// Measured on a circular pattern: nine faces are born in pairs at the joints between copies, and no
    /// face of the source corresponds to them — history does not consider them produced by anything, so the
    /// "produced by face N" mechanism finds nothing for them (verified: zero matches).
    ///
    /// Namelessness is not inevitable here. An edge has no recipe of its own either — it is where two
    /// surfaces meet — which is exactly why its name is derived from its pair of faces. A seam is in the
    /// same position: it is where named faces meet. The lowest name among the named neighbours is taken
    /// (the set is deterministic and does not depend on traversal order), and the ordinal within one
    /// neighbour is separated by the same marker used for split pieces: the set of the other named
    /// neighbours.
    ///
    /// A positional number remains only where there are no named neighbours at all: there is nothing to
    /// derive a name from, and inventing one is worse than admitting there is no recipe.
    fn name_seam_faces_of(&mut self, feature: Id, body: Id, kernel: &dyn crate::feature::Kernel) {
        // In waves rather than in one pass. An orphan takes its name from a named neighbour, but a face in a
        // corner has neighbours that are orphans themselves: on a shell of a shell exactly four corner faces
        // stayed nameless (three neighbours, none of them named). This is not cosmetic — measured, those are
        // precisely the faces that differ between two builds, because a positional number depends on
        // traversal order and a name does not. So after each wave the neighbours are asked again: the faces
        // named by the first wave become the anchor for the second.
        for _ in 0..8 {
            if !self.name_seam_wave(feature, body, kernel) {
                return;
            }
        }
    }

    /// One wave of seam naming: orphans that have a named neighbour get a name. Returns `true` when
    /// something was renamed and another wave is worth running.
    fn name_seam_wave(&mut self, feature: Id, body: Id, kernel: &dyn crate::feature::Kernel) -> bool {
        let live: Vec<u32> = self.regen_faces.get(&body).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
        let orphans: Vec<u32> = live.iter().copied().filter(|d| !crate::names::NameTable::is_named(*d)).collect();
        if orphans.is_empty() {
            return false;
        }
        let mut nb: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
        for (_, a, b) in kernel.edge_face_pairs(body) {
            nb.entry(a).or_default().push(b);
            nb.entry(b).or_default().push(a);
        }
        let named_nb = |f: u32| -> Vec<u32> {
            let mut v: Vec<u32> = nb.get(&f).map(|ns| ns.iter().copied().filter(|n| crate::names::NameTable::is_named(*n)).collect()).unwrap_or_default();
            v.sort_unstable();
            v.dedup();
            v
        };
        let mut groups: std::collections::BTreeMap<u32, Vec<u32>> = std::collections::BTreeMap::new();
        for f in orphans {
            let ns = named_nb(f);
            let Some(&host) = ns.first() else { continue }; // No named neighbours yet; wait for the next wave.
            groups.entry(host).or_default().push(f);
        }
        let mut renames: Vec<(u32, u32)> = Vec::new();
        let taken: std::collections::HashSet<u32> = live.iter().copied().collect();
        for (host, mut members) in groups {
            members.sort_by_key(|f| (named_nb(*f), *f));
            for (k, f) in members.iter().enumerate() {
                // The feature is part of the name. Without it a seam name is global: two different
                // operations both produce "seam at face N", and once their bodies meet in one body a
                // reference leads to two places at once (caught by the duplicate guard: body 431, three
                // pairs).
                let name = crate::names::GeoName { feature, role: crate::names::Role::Seam, src: host as Id, split: k.min(u16::MAX as usize) as u16 };
                let d = self.names.intern_face(name);
                if *f != d && !taken.contains(&d) {
                    renames.push((*f, d));
                }
            }
        }
        if renames.is_empty() {
            return false;
        }
        kernel.rename_faces(body, &renames);
        if let Some(faces) = self.regen_faces.get_mut(&body) {
            let map: std::collections::HashMap<u32, u32> = renames.into_iter().collect();
            for f in faces.iter_mut() {
                if let Some(&d) = map.get(&f.id) {
                    f.id = d;
                }
            }
        }
        true
    }

    fn name_face_splits_of(&mut self, body: Id, kernel: &dyn crate::feature::Kernel) {
        let splits = kernel.face_splits(body);
        if splits.is_empty() {
            return;
        }
        // The record is single-use: read it and clear it. Otherwise the next operation folds it into its own
        // group and re-elects the name holder, the name moves to a neighbour, and a reference written
        // earlier leads to a different face (measured: a body came out with 26 faces instead of 56 and a
        // face pull stopped building). It is cleared immediately, before any names are handed out, so that
        // no early return below leaves the record alive.
        kernel.clear_face_splits(body);
        // Which piece keeps the name of the whole is decided by recipe, not by list order.
        //
        // The kernel returns the pieces in `Modified` order and the first one used to inherit the name of
        // the original face. That order depends on subshape numbering, which shifts with any edit higher up
        // the timeline. Measured: as soon as a thread named the faces of its relief, the wall names moved to
        // different pieces and 16 of 36 fillet references landed on the wrong edges, while the references
        // themselves had not changed.
        //
        // The stable marker of a piece is the same as for an edge: which named faces are adjacent to it
        // (adjacency comes from the face pairs of the edges). Neighbours inside the group do not count, as
        // they are nameless right now and are being renamed together. The face number remains the last
        // separator, used only where the recipe is silent.
        let mut nb: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
        for (_, a, b) in kernel.edge_face_pairs(body) {
            nb.entry(a).or_default().push(b);
            nb.entry(b).or_default().push(a);
        }
        let live: std::collections::HashSet<u32> = self.regen_faces.get(&body).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
        // IN THE ORDER OF THE ORIGINALS, not of a hash: the names are interned as the groups are walked, and a step back
        // takes the interned names away, so the rebuild of a step forward hands them out again. Walked in hash order
        // the same name came to the half of another side - the top of the right (40, 15, 7.5) went to the front.
        let mut groups: std::collections::BTreeMap<u32, Vec<u32>> = std::collections::BTreeMap::new();
        for (piece, origin, _idx) in splits {
            groups.entry(origin).or_default().push(piece);
        }
        let mut renames: Vec<(u32, u32)> = Vec::new();
        for (origin, mut members) in groups {
            let Some(base) = self.names.get(origin) else { continue }; // The original is nameless, so there is nothing to inherit.
            if live.contains(&origin) {
                members.push(origin); // The piece currently holding the name of the whole takes part in the split too.
            }
            let own: std::collections::HashSet<u32> = members.iter().copied().collect();
            let key = |f: u32| -> Vec<u32> {
                let mut v: Vec<u32> = nb.get(&f).map(|ns| ns.iter().copied().filter(|n| crate::names::NameTable::is_named(*n) && !own.contains(n)).collect()).unwrap_or_default();
                v.sort_unstable();
                v.dedup();
                v
            };
            members.sort_by_key(|f| (key(*f), *f));
            for (k, f) in members.iter().enumerate() {
                // When the split ordinal is already occupied, what is being cut is itself a piece, and that
                // piece is the whole for the new pieces.
                //
                // For a pattern instance the `split` field holds the copy number, not a piece number.
                // Writing a piece number there erases the copy number: five pieces of a copied face come out
                // under one name, "instance, piece 2", and a reference then leads to five places at once
                // (measured on a scenario document). Their whole is the copied face itself, so its
                // descriptor becomes the source: "piece k of face origin". The holder (k = 0) stays itself.
                //
                // For a seam the ordinal is occupied too — it separates seams sharing one neighbour. Writing
                // a piece number there erases the seam ordinal, and "piece 1" lands on the name of the
                // second seam (the guard caught three pairs in body 431). So a seam follows the same path as
                // a pattern instance.
                let name = if base.split != 0 || base.role == crate::names::Role::Seam {
                    // The holder takes the name of the whole rather than staying as it is: the elected piece
                    // may itself be a positional one, and "as is" would then leave the name of the whole
                    // carried by nobody while the original face moved into "piece 1" (measured: 378
                    // positional names instead of 290, entirely from this).
                    if k == 0 {
                        base
                    } else {
                        crate::names::GeoName { feature: base.feature, role: base.role, src: origin as Id, split: k.min(u16::MAX as usize) as u16 }
                    }
                } else {
                    crate::names::GeoName { split: k.min(u16::MAX as usize) as u16, ..base }
                };
                let d = self.names.intern_face(name);
                if *f != d {
                    renames.push((*f, d));
                }
            }
        }
        if renames.is_empty() {
            return;
        }
        kernel.rename_faces(body, &renames);
        // Rename the faces in the model too: references resolve from them and edge names are derived from
        // them.
        if let Some(faces) = self.regen_faces.get_mut(&body) {
            let map: std::collections::HashMap<u32, u32> = renames.into_iter().collect();
            for f in faces.iter_mut() {
                if let Some(&d) = map.get(&f.id) {
                    f.id = d;
                }
            }
        }
    }

    /// Edge names from the pair of faces, plus exact translation of references.
    ///
    /// An edge has no recipe of its own: it is where two surfaces meet. While face names were positional
    /// there was nothing to derive from; now there is.
    ///
    /// The switch happens for a whole body at a time. A mixed space, where some edges are named and some are
    /// not, is more dangerous than either pure state: an old "edge 8" reference lands on a positional
    /// survivor and silently attaches to a different edge — measured, an R10 fillet ended up on an edge that
    /// only allows 1 mm.
    ///
    /// There is no global scheme freeze any more, and removing it was removing a dead safeguard rather than
    /// relaxing a rule. The freeze protected documents saved under the old edge-naming scheme, and such
    /// documents no longer exist: the format changed together with query-style references, and the current
    /// build does not read the older `.qcad` at all, backwards compatibility being out of scope by
    /// decision.
    ///
    /// Its harm, meanwhile, was real. Its cause was local (one positional number in one reference) while its
    /// effect was global (no names issued anywhere in the document). Measured, that cost four threads: a
    /// fillet on a body with one nameless face wrote positional numbers, the scheme froze, and threads on
    /// other bodies stopped finding their rims.
    ///
    /// The per-body switch is the condition that stayed, and it is the right one.
    fn name_edges_of(&mut self, body: Id, kernel: &dyn crate::feature::Kernel, emap: &mut EdgeRenames) {
        let pairs = kernel.edge_face_pairs(body);
        if pairs.is_empty() {
            return;
        }
        let named = crate::names::NameTable::is_named;
        // All-or-nothing was the earlier rule and it was a measured defect: one nameless face cancelled the
        // pass for the whole body and left every edge positional. On a body with 34 of 64 faces positional,
        // none of its 190 edges got a recipe name and the fillet references fell off on every edit (20 of 36
        // found). Removing the rule gives 36 of 36 with no fallback, measured twice.
        //
        // An edge between two named faces has a legitimate name regardless of what happens at the other end
        // of the body. Only edges with nowhere to take a name from are skipped.
        // Names already taken on this body. An edge name is a pair of faces plus an ordinal within that
        // pair, and the ordinal has to be free rather than merely "the first among the new ones".
        //
        // What that cost: shelling a through pocket. The outer edge of the end face arrives from the extrude
        // already named, while the inner one is born with the shell. Both share one pair of faces (wall and
        // end), and numbering from zero handed the newcomer ordinal 0, the one the neighbour already had.
        // Two different edges received one descriptor, and everything built on it collapsed: clicking one
        // highlighted both, a fillet cut the outer and the inner at once, and there was no way to select one
        // of them, the id being shared. Measured: 8 of 24 edges collided into pairs that way.
        //
        // The taken names come from the whole body rather than from the list of pairs. The "edge with its
        // two faces" list is incomplete: a seam and a degenerate edge have no pair, and body 275 contributes
        // 11 of its 17 edges while body 276 contributes 265 of its 274. The name of such an edge stayed
        // "free", the election handed it out a second time, and two edges ended up under one name — exactly
        // the twins the guard was catching.
        let mut used: std::collections::HashSet<u32> = kernel
            .edges(body)
            .into_iter()
            .map(|e| e.id)
            .chain(pairs.iter().map(|(e, _, _)| *e)) // Both sources: a mock kernel leaves the body edges empty.
            .filter(|e| named(*e))
            .collect();
        let mut by_pair: std::collections::BTreeMap<(u32, u32), Vec<u32>> = std::collections::BTreeMap::new();
        for (e, a, b) in pairs {
            if named(e) {
                continue; // Already named: it came through the operation history.
            }
            if !named(a) || !named(b) {
                continue; // Nowhere to take a name from: at least one of the faces is nameless itself.
            }
            by_pair.entry((a.min(b), a.max(b))).or_default().push(e);
        }
        // The ordinal within a pair comes from the recipe, not from the kernel number.
        //
        // When several edges lie between one pair of faces (a through slot gives an upper and a lower edge
        // between the same walls), they have to be ordered somehow. Ordering by kernel number means the
        // number shifts with any edit higher up the timeline and the twins silently swap places. Measured:
        // as soon as a thread named the faces of its relief, 12 of 36 edges moved to the wrong place and an
        // R2.0 fillet landed on the wrong edge.
        //
        // The stable marker is asked of the kernel: which named faces meet at the endpoints of the edge,
        // besides its own two. For the upper edge of a slot that is the top face, for the lower one the
        // bottom. The kernel number remains only as the last separator, where the recipe is silent (both
        // sides nameless), which is an honest tie rather than a choice.
        let ends: std::collections::HashMap<u32, (u32, u32)> = kernel.edge_end_faces(body).into_iter().map(|(e, a, b)| (e, (a, b))).collect();
        let mut renames: Vec<(u32, u32)> = Vec::new();
        for ((a, b), mut es) in by_pair {
            es.sort_unstable_by_key(|e| (ends.get(e).copied().unwrap_or((0, 0)), *e));
            let mut next = 0u16;
            for e in es {
                // The first free ordinal within this pair. A taken ordinal already exists in the table, so
                // probing through `intern_edge` creates no garbage: the same name comes back.
                let name = loop {
                    let n = self.names.intern_edge(crate::names::EdgeName::new(a, b, next));
                    next = next.saturating_add(1);
                    if !used.contains(&n) {
                        break n;
                    }
                };
                used.insert(name);
                renames.push((e, name));
            }
        }
        if renames.is_empty() {
            return;
        }
        kernel.rename_edges(body, &renames);
        emap.entry(body).or_default().extend(renames);
    }
}

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
