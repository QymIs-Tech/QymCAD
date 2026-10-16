//! THE KINDS OF FEATURES of the timeline, their nodes, and what each reads, owns and consumes.

use super::*;

/// A PIECE A CUT LEFT: a cut or an intersection that parts a body in several solids makes each solid a body of the same
/// part. The first piece is the node's own body (the biggest solid when the pieces first appear), the others get bodies
/// of their own. `at` is where the piece was when last built - the centre of its mass: the next build gives each stored
/// piece the solid nearest its point, so an edit of the cut that moves the pieces keeps every body on its piece, and
/// the faces named on it stay named.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CutPiece {
    pub body: Id,
    pub at: [f64; 3],
}

/// A node of the feature timeline. Sketches, datum planes and part features are all nodes of one ordered
/// timeline; the cached geometry lives in the project pools, keyed by id.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeatureNode {
    pub id: Id,
    pub name: String,
    pub kind: FeatureKind,
    /// Owning component (`None` is the document root).
    #[serde(default)]
    pub parent: Option<Id>,
    /// The node needs rebuilding (dirty), for the ordered `regenerate()` pass.
    #[serde(default)]
    pub dirty: bool,
    /// Suppressed: not built and removed from view, with dependent features cascading into the same state.
    /// Unlike `rollback`, which suppresses the tail of the timeline, this switches off a single feature.
    #[serde(default)]
    pub suppressed: bool,
}

impl FeatureNode {
    /// THE COMPONENT BODY `body` OF THIS NODE BELONGS TO: the node's own component, except for a pattern of
    /// components, whose every body belongs to its copy.
    pub fn owner_of(&self, body: Id) -> Option<Id> {
        match &self.kind {
            FeatureKind::ComponentPattern { copies, bodies, .. } => bodies.iter().position(|b| *b == body).and_then(|k| copies.get(k).copied()),
            _ => self.parent,
        }
    }
}

/// Chamfer mode. `Symmetric` uses an equal setback on both faces (`dist`). `TwoDist` uses two setbacks
/// (`dist` on the reference face, `d2` on the adjacent one). `DistAngle` uses a setback `dist` on the
/// reference face plus an angle `d2` in degrees.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum ChamferMode {
    #[default]
    Symmetric,
    TwoDist,
    DistAngle,
}

/// THE MOST INSTANCES ONE PATTERN MAY HOLD, the source counted, fixed when the program is built: a linear or circular
/// pattern of a body, or a pattern of components. A pattern past it is refused with a message.
///
/// Why a limit at all. The input box takes up to 512 per direction, but a formula or a file is not held by the box, and
/// nothing limited the product: it was multiplied in 32 bits, so a 65,536 x 65,536 component grid wrapped to a total of
/// 0 and the next rebuild divided by it, and a feature pattern reserved room for every copy before anything checked it
/// (a circular count of 4,000,000,000 asked for 384,000,000,000 bytes and the program died).
///
/// Why this number. The rebuild of a pattern grows with its size. Measured on a grid of separate 5 mm boxes (`cargo
/// test` build, 4 cores, two runs each): 1,000 instances rebuilt in 3.1-3.5 s with 0.22 GB at the peak, 2,000 in
/// 6.4-7.5 s (0.41 GB), 5,000 in 16 s (0.95 GB), 10,000 in 33-34 s (1.8 GB). Real patterns hold tens to a few hundred
/// copies; at 10,000 a count typed or computed past real use costs half a minute and memory an ordinary machine has,
/// not the program. The time grows with the size and no faster, which is what lets the limit sit this high.
pub const MAX_PATTERN_INSTANCES: u32 = 10_000;

/// THE MOST STARTS (separate helices) A THREAD OR AN AUGER MAY HAVE, fixed when the program is built.
///
/// Why this number. A multi-start thread or an auger has a handful of starts; 64 leaves room for any special case while
/// closing the hole an unlimited count opened: every start is a groove whose faces are named up front,
/// `edges x starts` names reserved before any check, so a thread or an auger with a billion starts was still running
/// after five minutes (measured) before anything was built. The turns of a thread already have their own limit (`ThreadTooManyTurns`, 400).
pub const MAX_HELIX_STARTS: u32 = 64;

/// How many instances a pattern of these per-direction counts asks for, each count taken as at least one: multiplied
/// in 64 bits and saturating, so no layout can wrap round to a small total.
pub fn pattern_asked(counts: &[u32]) -> u64 {
    counts.iter().fold(1u64, |total, &c| total.saturating_mul(c.max(1) as u64))
}

/// How many instances a pattern of these per-direction counts holds: `None` when that is past `MAX_PATTERN_INSTANCES`.
pub fn pattern_instances(counts: &[u32]) -> Option<u32> {
    let total = pattern_asked(counts);
    (total <= MAX_PATTERN_INSTANCES as u64).then_some(total as u32)
}

/// The refusal of a pattern of these counts, past `MAX_PATTERN_INSTANCES`.
pub fn pattern_too_large(counts: &[u32]) -> crate::errors::CoreError {
    crate::errors::CoreError::PatternTooLarge { asked: pattern_asked(counts), limit: MAX_PATTERN_INSTANCES }
}

/// Kind of a timeline node. The parameters of part features live here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FeatureKind {
    /// A sketch node (the geometry lives in `Project::sketches` under this id).
    Sketch { sketch: Id },
    /// A datum plane (in `Project::planes` under this id).
    Plane { plane: Id },
    /// A datum point (in `Project::datum_points` under this id).
    DatumPoint { point: Id },
    /// A datum axis (in `Project::datum_axes` under this id).
    DatumAxis { axis: Id },
    /// Extrude a sketch to a height, producing body `body`. `profile` is the id of a specific closed contour
    /// of the sketch (0 means the first closed one), which is how one of several shapes is chosen.
    Extrude {
        sketch: Id,
        /// Sketch contours to extrude. Any number of them form one operation and merge into one body.
        #[serde(default)]
        profiles: Vec<Id>,
        height: f64,
        /// Which way the body grows from the sketch plane: along the normal, against it, or half each way.
        /// Dragging the gizmo backwards sets [`Reach::Backward`].
        #[serde(default)]
        reach: Reach,
        /// Second side (against the normal): the body spans [-down, +height]. Zero means one-sided.
        #[serde(default)]
        down: f64,
        /// Contours explicitly selected as filled: they are not subtracted as holes of the profile even when
        /// they lie inside it. Empty means every nested contour becomes a hole automatically, which is the
        /// usual behaviour. Selecting both an outer and an inner circle fills the inner one and gives a solid
        /// cylinder.
        #[serde(default)]
        fill: Vec<Id>,
        body: Id,
    },
    /// Revolve a sketch about an axis (0 for X, 1 for Y), producing body `body`.
    ///
    /// Every selected contour forms one operation and one node, as for an extrude. With a single `profile`
    /// field a command over two contours produced one `Revolve` node per contour plus a `BodyBoolean` node
    /// per cut, which reads as the revolve falling apart into two features doing an add instead of a cut:
    /// `Revolve` itself did create a new body while the cut lived as a separate sibling, so editing the node
    /// showed one contour and knew nothing of the cut, and deleting it took the whole chain below with it.
    Revolve {
        sketch: Id,
        /// Tool contours. Empty means every closed contour of the sketch.
        #[serde(default)]
        profiles: Vec<Id>,
        axis: u8,
        angle: f64,
        /// Datum axis to revolve about (0 uses the sketch X or Y axis from `axis`). The axis has to lie in the
        /// sketch plane. It may be associative, built from a straight edge or a cylindrical face
        /// (`add_axis_from_edge`, `add_axis_from_face`).
        #[serde(default)]
        axis_datum: Id,
        /// A centreline of the sketch used as the revolve axis (0 means none). It takes priority over
        /// `axis_datum` and `axis`. Its 2D endpoints are the local axis directly in sketch space, so a diameter
        /// through the centre of a contour gives a sphere or a body of revolution.
        #[serde(default)]
        axis_line: Id,
        /// Which way the angle is swept from the sketch plane: forwards, back, or half each way.
        #[serde(default)]
        reach: Reach,
        /// Target body of the boolean (0 makes the operation create a new body). The boolean lives inside the
        /// node: otherwise a revolved cut is two nodes and is edited in two places.
        #[serde(default)]
        src: Id,
        /// 0 cut, 1 union, 2 intersection. Applies only when `src` is non-zero.
        #[serde(default)]
        op: u8,
        body: Id,
        /// The solids a cut or an intersection left, each a body of its own (see `CutPiece`); empty for one solid.
        #[serde(default)]
        pieces: Vec<CutPiece>,
    },
    /// Sweep: profiles (sketch `sketch`, contours `profiles`) along a path (sketch `path_sketch`, contour
    /// `path`), producing body `body`.
    ///
    /// The profile and the path are different sketches; the profile usually sits on a plane at the start of
    /// the path, roughly perpendicular to it. An empty `profiles` or a zero `path` picks the first suitable
    /// contour automatically. Exact lines and arcs of the path (`build_exact_wire`) produce true curved faces.
    ///
    /// One body per node and the boolean inside the node, for the same reasons as in
    /// [`FeatureKind::Revolve`].
    Sweep {
        sketch: Id,
        /// Profile contours. Empty means the first suitable contour of the sketch.
        #[serde(default)]
        profiles: Vec<Id>,
        path_sketch: Id,
        #[serde(default)]
        path: Id,
        /// Target body of the boolean (0 makes a new body).
        #[serde(default)]
        src: Id,
        /// 0 cut, 1 union, 2 intersection. Applies only when `src` is non-zero.
        #[serde(default)]
        op: u8,
        body: Id,
        /// The solids a cut or an intersection left, each a body of its own (see `CutPiece`); empty for one solid.
        #[serde(default)]
        pieces: Vec<CutPiece>,
    },
    /// Loft: a body through an ordered set of section profiles (sketches `sketches`, each taking the contour
    /// from `contours[i]`, where 0 means the first closed one). Each section sits on its own plane. `ruled`
    /// gives straight faces between sections, otherwise the surface is a smooth B-spline. Two or more sections
    /// produce body `body`.
    Loft {
        sketches: Vec<Id>,
        #[serde(default)]
        contours: Vec<Id>,
        #[serde(default)]
        ruled: bool,
        /// Target body of the boolean (0 makes a separate new body). When set, the lofted solid is combined
        /// with it through `op` and body `src` is consumed, giving a lofted cut, boss or intersection.
        #[serde(default)]
        src: Id,
        /// Boolean operation against `src` (0 cut, 1 union, 2 intersection). Ignored when `src` is zero.
        #[serde(default)]
        op: u8,
        /// The result is a surface (a sheet) rather than a solid. The sections then need not be closed: open
        /// curves cannot bound a solid, while lofting a surface through them is an ordinary design task.
        #[serde(default)]
        surface: bool,
        body: Id,
        /// The solids a cut or an intersection left, each a body of its own (see `CutPiece`); empty for one solid.
        #[serde(default)]
        pieces: Vec<CutPiece>,
    },
    /// Box primitive (centred in XY at the origin, base at z = 0), producing a body.
    Box3 { dx: f64, dy: f64, dz: f64, body: Id },
    /// Cylinder primitive (axis Z, base at z = 0), producing a body.
    Cylinder { r: f64, h: f64, body: Id },
    /// Sphere primitive (centred at the origin), producing a body.
    Sphere { r: f64, body: Id },
    /// Operation performed on body `src` with an extruded sketch: `op` is 0 for a cut, 1 for a boss (union)
    /// and 2 for an intersection. The profile is extruded by `height` and combined with `src` into `body`.
    Combine {
        src: Id,
        sketch: Id,
        /// Contours of the tool sketch. Any number of them form one operation, merging into a single boolean
        /// against `src`.
        #[serde(default)]
        profiles: Vec<Id>,
        height: f64,
        op: u8,
        /// How far the tool goes from the sketch plane and which way: through all, reversed, symmetric.
        ///
        /// One named value rather than three loose booleans, here and everywhere past the record. Documents
        /// saved before the rename lose these three: the field is missing, `serde` fills a default, and a cut
        /// saved as reversed comes back cutting the other way. That is the accepted price of changing a format
        /// straight instead of carrying a reader for the old shape - a broken old document is repaired by hand
        /// in the window, not by compatibility code in the core.
        #[serde(default)]
        extent: Extent,
        /// Second side (against the normal) for a two-sided tool: it spans [-down, +height]. Zero means
        /// one-sided.
        #[serde(default)]
        down: f64,
        /// Contours explicitly selected as filled: they are not subtracted as holes of the tool, as in
        /// `Extrude`.
        #[serde(default)]
        fill: Vec<Id>,
        body: Id,
        /// The solids a cut or an intersection left, each a body of its own (see `CutPiece`); empty for one solid.
        #[serde(default)]
        pieces: Vec<CutPiece>,
    },
    /// Fillet the edges of body `src` with radius `radius`, producing `body`.
    ///
    /// `edges` is a query reference: a hand-picked set of edges is expressible alongside "every edge of this
    /// face" and "the seam between these two sets", and the latter keeps up when the model gains edges.
    ///
    /// `at_vertices` gives a variable radius specified at vertices: a vertex reference plus the radius there.
    /// An empty table means a constant radius. Specifying the radius as "at the start of the edge, at the
    /// end" describes one directed edge and is fundamentally incompatible with a set, which has no
    /// direction.
    Fillet {
        src: Id,
        radius: f64,
        edges: crate::refs::Ref,
        #[serde(default)]
        at_vertices: Vec<(crate::refs::Ref, f64)>,
        body: Id,
    },
    /// Patch: a surface spanned over a chain of edges of body `src`. The edges come as a query and the source
    /// is not consumed.
    ///
    /// `tangent` makes the patch meet the edges smoothly, tangent to the adjacent faces, rather than merely
    /// coinciding in position: otherwise the seam is visible and can be felt.
    Patch {
        src: Id,
        edges: crate::refs::Ref,
        #[serde(default)]
        tangent: bool,
        body: Id,
    },
    /// Replace faces with a surface. A face taken off the body, edited on its own, is returned to the body
    /// instead of standing next to it. `faces` is a query and `surface` is a sheet body. Both are consumed:
    /// what continues down the timeline is the result.
    SurfaceReplace { src: Id, faces: crate::refs::Ref, surface: Id, body: Id },
    /// Trim a surface: sheet `src` is cut by body `tool` and the piece nearest to point `keep` — the place
    /// that was clicked — is retained. A point rather than a piece number, because a number is a property of
    /// today's traversal order and points somewhere else after the base is edited.
    Trim { src: Id, tool: Id, keep: [f64; 3], body: Id },
    /// Stitch sheets into one surface. A surface is rarely born whole — a patch here, a copied face there —
    /// and while those are separate bodies they can be neither treated as one surface nor thickened, since
    /// thickening would take each piece on its own. If the result closes, the output is a solid. Every input
    /// is consumed: what continues down the timeline is the result.
    Stitch { parts: Vec<Id>, tol: f64, body: Id },
    /// A mesh turned into a solid a person can cut and drill: every triangle a flat face, flat neighbours merged into
    /// one, sewn - a polyhedron, not the exact geometry a recognition would give. `src` is the mesh body and is
    /// consumed; the solid is built again from its mesh, which the document keeps.
    /// `simplify` is the deviation in mm the mesh is made lighter within first (edges collapsed by their quadric
    /// error); zero leaves every triangle.
    MeshSolid {
        src: Id,
        body: Id,
        #[serde(default)]
        simplify: f64,
    },
    /// A mesh recognised into a body of exact surfaces: planes, cylinders, cones, spheres and tori found on it, their
    /// edges the curves those surfaces meet in. `src` is the mesh body and is consumed; `tol` multiplies the distance a
    /// corner may lie from its surface - the person chooses it, 1 for a mesh from a CAD, more for a coarser one.
    /// `simplify` makes the mesh lighter first, as for `MeshSolid`.
    MeshRecognised {
        src: Id,
        body: Id,
        tol: f64,
        sharp: f64,
        #[serde(default)]
        simplify: f64,
    },
    /// Copy faces into a separate surface: faces of body `src`, selected by a query, become an independent
    /// sheet `body`. The source is not consumed — a copy is a copy, so the body stays where it is while the
    /// surface lives its own life and returns to it through a face replacement.
    FaceCopy { src: Id, faces: crate::refs::Ref, body: Id },
    /// Offset a surface: faces of body `src` (or a whole sheet), selected by a query, taken out as a sheet `body` moved
    /// `dist` along their normals - outward for the faces of a solid, a negative distance going in, zero a copy in
    /// place. Like a face copy the source is read, not consumed.
    OffsetSurface { src: Id, faces: crate::refs::Ref, dist: f64, body: Id },
    /// Chamfer the edges of body `src` by `dist`, producing `body`. `edges` is a query reference.
    ///
    /// `mode`, `d2` and `flip` cover the asymmetric variants: `d2` is the second setback or the angle in
    /// degrees, and `flip` selects which of the two faces adjacent to an edge is the reference one.
    Chamfer {
        src: Id,
        dist: f64,
        edges: crate::refs::Ref,
        #[serde(default)]
        mode: ChamferMode,
        #[serde(default)]
        d2: f64,
        #[serde(default)]
        flip: bool,
        /// Persistent id of the reference face chosen by hand for the asymmetric modes. Zero selects it
        /// automatically, from the two faces adjacent to the edge according to `flip`. When set, every edge
        /// adjacent to that face uses it as the reference (the `dist` setback), while the remaining edges fall
        /// back to `flip`.
        #[serde(default)]
        ref_face: u32,
        body: Id,
    },
    /// Cone primitive (axis Z, base radius r1 at z = 0, top radius r2 at z = h), producing a body.
    Cone { r1: f64, r2: f64, h: f64, body: Id },
    /// Torus primitive (major radius `major`, section radius `minor`, axis Z), producing a body.
    Torus { major: f64, minor: f64, body: Id },
    /// Regular prism primitive (circumscribed radius `r`, `n` sides, height `h`), producing a body.
    Prism { r: f64, n: u32, h: f64, body: Id },
    /// Shell body `src`: open the faces named by `faces` and leave walls of `thickness`, producing `body`.
    ///
    /// `faces` is a query reference saying which faces to open: "these three" is as expressible as "every top
    /// face of this feature", and the latter keeps up with the model when it gains faces.
    ///
    /// `side` says where the wall goes: into the body, out of it, or half on each side of the face.
    Shell {
        src: Id,
        thickness: f64,
        faces: crate::refs::Ref,
        #[serde(default)]
        side: ShellSide,
        body: Id,
    },
    /// Draft: tilt the faces named by `faces` on body `src` by `angle` degrees relative to the neutral face
    /// `neutral`, whose line of intersection with them stays fixed. The pull direction is the normal of the
    /// neutral face, reversed by `flip`. Used for cast and stamped draft angles.
    ///
    /// Both references are queries: "every wall of this feature" is as expressible as a hand-picked set.
    Draft {
        src: Id,
        faces: crate::refs::Ref,
        neutral: crate::refs::Ref,
        angle: f64,
        #[serde(default)]
        flip: bool,
        body: Id,
    },
    /// Linear pattern of body `src`: direction one (`dx,dy,dz` by `count`) plus an optional direction two
    /// (`dx2,dy2,dz2` by `count2`), forming a grid; the copies are united into `body`. A `count2` of one or
    /// less uses the first direction only. Every step is parametric.
    LinearArray {
        src: Id,
        dx: f64,
        dy: f64,
        dz: f64,
        count: u32,
        #[serde(default)]
        dx2: f64,
        #[serde(default)]
        dy2: f64,
        #[serde(default)]
        dz2: f64,
        #[serde(default)]
        count2: u32,
        // Third direction: a full 3D grid of count by count2 by count3.
        #[serde(default)]
        dx3: f64,
        #[serde(default)]
        dy3: f64,
        #[serde(default)]
        dz3: f64,
        #[serde(default)]
        count3: u32,
        body: Id,
    },
    /// Circular pattern of body `src` about an axis: `count` copies filling `angle` degrees, united into
    /// `body`. `axis` is a datum axis id (its origin and direction are resolved during regenerate), or 0 for
    /// world Z.
    CircularArray {
        src: Id,
        count: u32,
        angle: f64,
        #[serde(default)]
        axis: Id,
        body: Id,
    },
    /// Mirrored part in an assembly: the body of the new part is the reflection of the active body of part
    /// `src_comp` through a plane passing through the local zero of the source with normal `ln` (the normal of
    /// the clicked plane, rotated into source local space at creation time — a click on XY, XZ, YZ, a datum or
    /// a face).
    ///
    /// The shape is associative, so editing the source rebuilds the mirror, while the placement is free. It is
    /// set once at creation: the local zero of the copy is the world reflection of the local zero of the
    /// source through the same plane, and the orientation of the coordinate system is not mirrored, so the
    /// gizmo travels with the reflected body instead of staying at the source. After that the placement is
    /// moved by hand and regenerate does not touch it.
    ///
    /// The cross-component reference is legal by construction (an isolation exception): `src` is resolved
    /// dynamically through `active_body`.
    MirrorPart {
        src_comp: Id,
        #[serde(default)]
        ln: [f64; 3],
        body: Id,
    },
    /// Thicken: face `face` of body `src` becomes a plate of `thickness` as a new body `body`. The source is
    /// not consumed — a skin is made from a face of the housing while the housing stays where it is.
    ///
    /// `join` names the body to weld the plate onto (0 means no welding). Without it, thickening a sheet
    /// leaves a second body inside the part, visible on screen as a differently coloured piece and breaking
    /// the "one part is one body" rule. A sheet that grew out of a part returns into it.
    Thicken {
        src: Id,
        face: u32,
        thickness: f64,
        #[serde(default)]
        join: Id,
        body: Id,
    },
    /// Split faces by a plane without cutting the body: one body, more faces. The plane is given by a
    /// reference (a datum, or the world plane in `plane`) plus a parametric `offset`, as for a body split.
    SplitFace {
        src: Id,
        plane: u8,
        datum: Id,
        offset: f64,
        body: Id,
        #[serde(default)]
        face: Option<(Id, FaceKey)>,
    },
    /// Part instance (a copy inside a component pattern): the body is a one-to-one copy of the active body of
    /// `src_comp`, and the placement comes from the transform of the copied component itself.
    ///
    /// The same approach as the mirrored part: the shape is associative, so editing the source moves every
    /// copy, while the position is a matter for the assembly. The cross-component reference is legal by
    /// construction (an isolation exception, as for `MirrorPart`): `src_comp` is resolved dynamically through
    /// `active_body`.
    PartInstance { src_comp: Id, body: Id },
    /// A pattern of components: ONE node of the assembly's timeline that builds the body of every copy. The copies
    /// are parts of their own - mated, moved by the pattern - listed in `copies`, each with its body at the same
    /// place in `bodies`; the body repeats the active body of `src` and belongs to its copy (`FeatureNode::owner_of`).
    /// The layout is `kind`. Deleting the node takes the copies with it, the source stays.
    ComponentPattern { src: Id, kind: crate::model::CompPatternKind, copies: Vec<Id>, bodies: Vec<Id> },
    /// Mirror body `src` about a plane: `plane` selects a world plane (0 XY, 1 XZ, 2 YZ), a non-zero `datum` an
    /// arbitrary datum plane, and `face` the plane of a face of a body, read off it at every rebuild - the one node of
    /// the timeline, rather than a datum plane of its own beside it. `keep` unites the result with the original.
    Mirror {
        src: Id,
        plane: u8,
        keep: bool,
        #[serde(default)]
        datum: Id,
        body: Id,
        #[serde(default)]
        face: Option<(Id, FaceKey)>,
    },
    /// Hole in body `src`: a cylinder of `diameter` and `depth` cut at the centre of face `face`, so the hole
    /// travels with the face; `point` and `normal` are the fallback fingerprint.
    ///
    /// `kind` is 0 for a plain hole, 1 for a counterbore and 2 for a countersink; `dia2` and `depth2` give the
    /// diameter and depth of that recess.
    Hole {
        src: Id,
        /// Which face is drilled, expressed as a query rather than as a number.
        ///
        /// A `FaceKey` is a specific name plus a fingerprint, and a lost reference was matched to the nearest
        /// co-directed face — silently, and sometimes to the wrong one. A query either finds the face by
        /// recipe or refuses with a named reason.
        face: crate::refs::Ref,
        point: [f64; 3],
        normal: [f64; 3],
        diameter: f64,
        depth: f64,
        #[serde(default)]
        kind: u8,
        #[serde(default)]
        dia2: f64,
        #[serde(default)]
        depth2: f64,
        /// When non-zero, holes are placed at the isolated points of sketch `sketch`, many at once, and
        /// `point` and `normal` are ignored.
        #[serde(default)]
        sketch: Id,
        /// Drill against the sketch normal instead of along it.
        #[serde(default)]
        flip: bool,
        body: Id,
        /// The solids a cut or an intersection left, each a body of its own (see `CutPiece`); empty for one solid.
        #[serde(default)]
        pieces: Vec<CutPiece>,
    },
    /// Thread: a modifier of a cylinder or a hole, associative to the circular edge `edge` of body `src` (the
    /// rim of the cylinder or hole, which supplies the centre point on the axis together with the axis and the
    /// radius).
    ///
    /// The specification is given the way a professional CAD does it: `spec` carries the standard and the size
    /// (`ThreadSpec`) while all the geometry — diameters, depth, the profile with its arcs — is computed from
    /// the formulas of that standard. Storing a bare angle and depth instead leaves the numbers to be guessed,
    /// and the tool then appears not to work.
    Thread {
        src: Id,
        /// Persistent id of the circular edge (the rim) on `src`, which supplies the axis (centre and normal) and
        /// the radius.
        edge: u32,
        /// Standard, size, fit and rounding (see `qymcad_core::thread`).
        spec: crate::thread::ThreadSpec,
        /// Thread length along the axis, in millimetres.
        length: f64,
        /// Lead-in, in millimetres: the turn sinks smoothly into the surface at the end face.
        #[serde(default)]
        lead_in: f64,
        /// Lead-out, in millimetres.
        #[serde(default)]
        lead_out: f64,
        body: Id,
    },
    /// Auger: a helical flight added onto shaft `src`, associative to the circular edge `edge`. Same family as
    /// a thread but the inverse operation — welding a flight on rather than cutting a groove.
    Auger {
        src: Id,
        edge: u32,
        spec: crate::thread::AugerSpec,
        /// Auger length along the axis, in millimetres.
        length: f64,
        /// Run-out at the start and at the end, in millimetres: over this length the flight height fades to
        /// zero. Without it the flight ends in an abrupt transverse cut.
        #[serde(default)]
        lead_in: f64,
        #[serde(default)]
        lead_out: f64,
        body: Id,
    },
    /// Delete faces: the faces named by `faces` are removed from body `src` and the neighbouring ones are
    /// extended, producing `body`. The reference is a query.
    RemoveFace { src: Id, faces: crate::refs::Ref, body: Id },
    /// Split a body by a plane: `src` falls apart into pieces, each an independent body. `bodies` holds one id
    /// per piece, ordered from bottom to top along the normal, and the first of them also serves as the
    /// `body()` of the node.
    ///
    /// The plane is given by a reference, as for a mirror: `datum` selects a datum plane (including a face
    /// snapshot created at pick time), otherwise the world plane in `plane` (0 XY, 1 XZ, 2 YZ) is used. That
    /// makes the split associative — when the face moves, the split moves with it. `offset` shifts along the
    /// normal (the `offset` feature dimension), so a cut can be made next to the plane rather than on it,
    /// without creating a separate datum for one number.
    SplitBody {
        src: Id,
        plane: u8,
        datum: Id,
        offset: f64,
        bodies: Vec<Id>,
        #[serde(default)]
        face: Option<(Id, FaceKey)>,
    },
    /// Offset a face: planar face `face` of body `src` is moved by `dist` along its own normal, producing
    /// `body`. Parametric like everything else in the timeline — `dist` is edited and recomputed, and the face
    /// reference is a query resolved by recipe rather than matched by similarity.
    PushFace { src: Id, face: crate::refs::Ref, dist: f64, body: Id },
    /// Rigid translation and rotation of body `src` (3x4 row-major), producing `body`. It moves the B-rep
    /// rather than the mesh, so the body stays parametric, and it replaces `src` in the chain as a modifier.
    Move { src: Id, mat: [f64; 12], body: Id },
    /// A PIECE MADE A PART by hand: body `src`, one of the bodies a cut or a split of another part left, as body `body`
    /// of a part of its own, in the same place. It reads the body of the part it came from - the named exception to
    /// the isolation of parts - so an edit of the cut rebuilds both; the piece leaves the part it came from.
    Piece { src: Id, body: Id },
    /// Parametric body-to-body boolean: `op` applied to the B-reps of body `a` (the base) and body `b` (the
    /// tool), producing `body`. `op` is 0 for a cut (a minus b), 1 for a union and 2 for an intersection. Both
    /// input bodies are consumed and hidden.
    BodyBoolean {
        a: Id,
        b: Id,
        op: u8,
        body: Id,
        /// The solids a cut or an intersection left, each a body of its own (see `CutPiece`); empty for one solid.
        #[serde(default)]
        pieces: Vec<CutPiece>,
    },
    /// An imported external B-rep solid (STEP) as the base body of a part: `body` is its id, `source` is the
    /// id of the embedded original file (`Project::sources`) and `solid` is the index of the solid within that
    /// file.
    ///
    /// There is no build recipe — the shape is restored by re-importing the source, and regenerate only
    /// re-tessellates the already loaded shape. The result is a full part: sketches, chamfers, fillets and
    /// booleans can be built on top of it as on any base body.
    Import {
        body: Id,
        source: Id,
        solid: u32,
        /// The factor the solid is taken at from the file's own numbers, as set when the file came in: a file can
        /// name the wrong unit. 1 is the file as it is.
        #[serde(default = "unit_scale")]
        scale: f64,
    },
    /// A piece of an imported mesh (STL, OBJ, PLY, glTF, 3MF, AMF) as the body of a part: `body` is its id, `source`
    /// the embedded original file and `piece` its place among the pieces the file was read into.
    ///
    /// Apart from `Import` on purpose: an imported solid has a B-rep, is raised again from its source and is exported
    /// as one; a mesh piece never has a B-rep. Its mesh is the geometry, kept by the document, and it is exported as
    /// a mesh.
    MeshPiece {
        body: Id,
        source: Id,
        piece: u32,
        /// The factor the piece is taken at from the file's own numbers, as set when the file came in or since: its
        /// mesh is kept at it, and a new factor is taken from the mesh divided by this one. 1 is the file as it is.
        #[serde(default = "unit_scale")]
        scale: f64,
    },
}

impl FeatureKind {
    /// Numeric parameters of a feature, as `(key, stored value)`.
    ///
    /// For a feature parameter to become a named driver, its value has to be readable by key name, the way a
    /// sketch dimension value is readable by its set of entities.
    ///
    /// The keys are the same ones `feat_dims` uses to override numbers with expressions (see `regen.rs`,
    /// `eval_dim`): the list has to match what is applied there, or a driver would name a parameter the
    /// rebuild does not know. Hence one table, living next to the type instead of being rebuilt in the
    /// interface.
    pub fn dims(&self) -> Vec<(&'static str, f64)> {
        match *self {
            FeatureKind::Extrude { height, down, .. } => vec![("height", height), ("down", down)],
            FeatureKind::Combine { height, down, .. } => vec![("height", height), ("down", down)],
            FeatureKind::Revolve { angle, .. } => vec![("angle", angle)],
            FeatureKind::Box3 { dx, dy, dz, .. } => vec![("dx", dx), ("dy", dy), ("dz", dz)],
            FeatureKind::Cylinder { r, h, .. } => vec![("r", r), ("h", h)],
            FeatureKind::Sphere { r, .. } => vec![("r", r)],
            FeatureKind::Cone { r1, r2, h, .. } => vec![("r1", r1), ("r2", r2), ("h", h)],
            FeatureKind::Torus { major, minor, .. } => vec![("major", major), ("minor", minor)],
            FeatureKind::Prism { r, h, .. } => vec![("r", r), ("h", h)],
            FeatureKind::Fillet { radius, .. } => vec![("radius", radius)],
            FeatureKind::Chamfer { dist, d2, .. } => vec![("dist", dist), ("d2", d2)],
            FeatureKind::Shell { thickness, .. } => vec![("thickness", thickness)],
            FeatureKind::Thicken { thickness, .. } => vec![("thickness", thickness)],
            FeatureKind::Draft { angle, .. } => vec![("angle", angle)],
            FeatureKind::PushFace { dist, .. } | FeatureKind::OffsetSurface { dist, .. } => vec![("dist", dist)],
            FeatureKind::Stitch { tol, .. } => vec![("tol", tol)],
            FeatureKind::SplitFace { offset, .. } => vec![("offset", offset)],
            FeatureKind::SplitBody { offset, .. } => vec![("offset", offset)],
            FeatureKind::CircularArray { angle, .. } => vec![("angle", angle)],
            FeatureKind::LinearArray { dx, dy, dz, dx2, dy2, dz2, dx3, dy3, dz3, .. } => {
                vec![("dx", dx), ("dy", dy), ("dz", dz), ("dx2", dx2), ("dy2", dy2), ("dz2", dz2), ("dx3", dx3), ("dy3", dy3), ("dz3", dz3)]
            }
            FeatureKind::Hole { diameter, depth, dia2, depth2, .. } => {
                vec![("diameter", diameter), ("depth", depth), ("dia2", dia2), ("depth2", depth2)]
            }
            FeatureKind::Thread { length, lead_in, lead_out, .. } => vec![("length", length), ("lead_in", lead_in), ("lead_out", lead_out)],
            FeatureKind::Auger { length, lead_in, lead_out, .. } => vec![("length", length), ("lead_in", lead_in), ("lead_out", lead_out)],
            _ => Vec::new(),
        }
    }

    /// Stored value of a parameter by key. `None` means this feature has no such parameter.
    pub fn dim(&self, key: &str) -> Option<f64> {
        self.dims().into_iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// Id of the resulting body (`None` for a sketch or a plane).
    pub fn body(&self) -> Option<Id> {
        // A split produces several bodies (a `Vec`, which cannot be matched by value here): the first piece
        // counts as primary and the full list is `bodies()`.
        if let FeatureKind::SplitBody { bodies, .. } = self {
            return bodies.first().copied();
        }
        match *self {
            FeatureKind::Extrude { body, .. }
            | FeatureKind::Revolve { body, .. }
            | FeatureKind::Sweep { body, .. }
            | FeatureKind::Loft { body, .. }
            | FeatureKind::Box3 { body, .. }
            | FeatureKind::Cylinder { body, .. }
            | FeatureKind::Sphere { body, .. }
            | FeatureKind::Combine { body, .. }
            | FeatureKind::Fillet { body, .. }
            | FeatureKind::PushFace { body, .. }
            | FeatureKind::RemoveFace { body, .. }
            | FeatureKind::Chamfer { body, .. }
            | FeatureKind::FaceCopy { body, .. }
            | FeatureKind::OffsetSurface { body, .. }
            | FeatureKind::Stitch { body, .. }
            | FeatureKind::MeshSolid { body, .. }
            | FeatureKind::MeshRecognised { body, .. }
            | FeatureKind::Trim { body, .. }
            | FeatureKind::Patch { body, .. }
            | FeatureKind::SurfaceReplace { body, .. }
            | FeatureKind::Cone { body, .. }
            | FeatureKind::Torus { body, .. }
            | FeatureKind::Prism { body, .. }
            | FeatureKind::Shell { body, .. }
            | FeatureKind::Draft { body, .. }
            | FeatureKind::LinearArray { body, .. }
            | FeatureKind::CircularArray { body, .. }
            | FeatureKind::Mirror { body, .. }
            | FeatureKind::Hole { body, .. }
            | FeatureKind::Thread { body, .. }
            | FeatureKind::Auger { body, .. }
            | FeatureKind::Move { body, .. }
            | FeatureKind::Piece { body, .. }
            | FeatureKind::BodyBoolean { body, .. }
            | FeatureKind::MirrorPart { body, .. }
            | FeatureKind::PartInstance { body, .. }
            | FeatureKind::Thicken { body, .. }
            | FeatureKind::SplitFace { body, .. }
            | FeatureKind::Import { body, .. }
            | FeatureKind::MeshPiece { body, .. } => Some(body),
            _ => None,
        }
    }

    /// Every body a node produces. For most features that is `[body]`; for a split it is one body per piece.
    /// Body listings (visibility, export, the tree) have to come from here, or every split piece but the first
    /// disappears.
    pub fn bodies(&self) -> Vec<Id> {
        match self {
            FeatureKind::SplitBody { bodies, .. } | FeatureKind::ComponentPattern { bodies, .. } => bodies.clone(),
            _ => self.body().into_iter().chain(self.cut_pieces().iter().skip(1).map(|p| p.body)).collect(),
        }
    }

    /// THE PIECES A CUT OR AN INTERSECTION LEFT, the node's own body first; empty while the result is one solid, and for
    /// a node that never cuts.
    pub fn cut_pieces(&self) -> &[CutPiece] {
        match self {
            FeatureKind::Revolve { pieces, .. }
            | FeatureKind::Sweep { pieces, .. }
            | FeatureKind::Loft { pieces, .. }
            | FeatureKind::Combine { pieces, .. }
            | FeatureKind::Hole { pieces, .. }
            | FeatureKind::BodyBoolean { pieces, .. } => pieces,
            _ => &[],
        }
    }

    /// The pieces of a node that may leave some - a cut or an intersection - to be written by the rebuild; `None` for a
    /// node that adds material or makes a body of its own, which keeps its result one body whatever it is.
    pub fn cut_pieces_mut(&mut self) -> Option<&mut Vec<CutPiece>> {
        match self {
            FeatureKind::Revolve { op, src, pieces, .. } | FeatureKind::Sweep { op, src, pieces, .. } | FeatureKind::Loft { op, src, pieces, .. } if *op != 1 && *src != 0 => Some(pieces),
            FeatureKind::Combine { op, pieces, .. } if *op != 1 => Some(pieces),
            FeatureKind::BodyBoolean { op, pieces, .. } if *op != 1 => Some(pieces),
            FeatureKind::Hole { pieces, .. } => Some(pieces),
            _ => None,
        }
    }

    /// WHETHER THE BODY OF THIS NODE GETS A LIVE B-REP - the one rule for every list of "bodies still waiting for
    /// one". A mesh piece never has one; counted as waiting, it asked for a rebuild of nothing every time a live
    /// B-rep was wanted and kept the cache from ever being ready.
    pub fn waits_for_brep(&self) -> bool {
        !matches!(self, FeatureKind::MeshPiece { .. })
    }

    /// WHETHER THIS NODE BROUGHT A FILE IN - a solid or a piece of a mesh. It has no command of its own to reopen:
    /// editing it is asking again about the file's units and scale, from its row in the tree and from its properties.
    pub fn is_import(&self) -> bool {
        matches!(self, FeatureKind::Import { .. } | FeatureKind::MeshPiece { .. })
    }

    /// DOES THIS NODE OWN THIS BODY - the same question as `bodies().contains(&id)`, without building the list.
    ///
    /// Measured on a document of 1296 bodies: asking it through `bodies()` allocated a vector per node per
    /// body - 1.7 million allocations for one pass over the scene, about 175 ms, and that pass happens on
    /// EVERY frame. It showed up as the whole program going sluggish, settings windows included.
    pub fn owns_body(&self, id: Id) -> bool {
        match self {
            FeatureKind::SplitBody { bodies, .. } | FeatureKind::ComponentPattern { bodies, .. } => bodies.contains(&id),
            _ => self.body() == Some(id) || self.cut_pieces().iter().any(|p| p.body == id),
        }
    }

    /// What a node introduces into the document: the resulting bodies plus a sketch or a datum when the node
    /// creates one.
    ///
    /// It differs from [`bodies`](Self::bodies) in that a body is not the only thing a feature can bring: a
    /// sketch, a datum plane, a point and an axis also appear as timeline nodes and also serve as inputs to
    /// others. Without this, "what created it" would only answer for bodies while half the tree — sketches and
    /// datums — would have no provenance.
    /// THE PART WHOSE ACTIVE BODY THIS NODE REPEATS: a mirrored part, a clone, a pattern of components. That body is
    /// not among `inputs` - it is whatever the source ends in - so the rebuild asks for it here.
    pub fn copy_source(&self) -> Option<Id> {
        match *self {
            FeatureKind::MirrorPart { src_comp, .. } | FeatureKind::PartInstance { src_comp, .. } => Some(src_comp),
            FeatureKind::ComponentPattern { src, .. } => Some(src),
            _ => None,
        }
    }

    pub fn declares(&self) -> Vec<Id> {
        match *self {
            FeatureKind::Sketch { sketch } => vec![sketch],
            FeatureKind::Plane { plane } => vec![plane],
            FeatureKind::DatumPoint { point } => vec![point],
            FeatureKind::DatumAxis { axis } => vec![axis],
            _ => self.bodies(),
        }
    }

    /// Ids of the inputs of a node — the sketches and bodies it depends on — for the rebuild graph. Primitives
    /// have no inputs, being built from parameters alone.
    pub fn inputs(&self) -> Vec<Id> {
        // A loft depends on every section sketch (a `Vec`, handled before the match); for a lofted boolean the
        // target body `src` joins the dependencies.
        if let FeatureKind::Loft { sketches, src, .. } = self {
            let mut v = sketches.clone();
            if *src != 0 {
                v.push(*src);
            }
            return v;
        }
        match *self {
            // A referenced plane is an input too: without it a split or a mirror would not follow a datum that
            // moved, and deleting the datum would leave the feature cutting against geometry that is gone.
            // the body whose face the plane is read off, when it is not the source itself
            FeatureKind::SplitBody { src, face: Some((b, _)), .. } | FeatureKind::SplitFace { src, face: Some((b, _)), .. } | FeatureKind::Mirror { src, face: Some((b, _)), .. } if b != src => vec![src, b],
            FeatureKind::SplitBody { src, datum, .. } if datum != 0 => vec![src, datum],
            FeatureKind::SplitBody { src, .. } => vec![src],
            FeatureKind::SplitFace { src, datum, .. } if datum != 0 => vec![src, datum],
            FeatureKind::SplitFace { src, .. } => vec![src],
            FeatureKind::Thicken { src, .. } => vec![src],
            FeatureKind::Mirror { src, datum, .. } if datum != 0 => vec![src, datum],
            // the axis a circular pattern turns about, the same way: resolved first, and followed when it moves
            FeatureKind::CircularArray { src, axis, .. } if axis != 0 => vec![src, axis],
            FeatureKind::Extrude { sketch, .. } => vec![sketch],
            // The target body is an input too: without it a revolved or swept cut would not follow the body it
            // cuts from, and deleting that body would leave the feature cutting from nothing.
            FeatureKind::Revolve { sketch, src, .. } if src != 0 => vec![src, sketch],
            FeatureKind::Revolve { sketch, .. } => vec![sketch],
            FeatureKind::Sweep { sketch, path_sketch, src, .. } if src != 0 => vec![src, sketch, path_sketch],
            FeatureKind::Sweep { sketch, path_sketch, .. } => vec![sketch, path_sketch],
            FeatureKind::BodyBoolean { a, b, .. } => vec![a, b],
            FeatureKind::Combine { src, sketch, .. } => vec![src, sketch],
            FeatureKind::Fillet { src, .. }
            | FeatureKind::Chamfer { src, .. }
            | FeatureKind::Shell { src, .. }
            | FeatureKind::Draft { src, .. }
            | FeatureKind::PushFace { src, .. }
            | FeatureKind::RemoveFace { src, .. }
            | FeatureKind::LinearArray { src, .. }
            | FeatureKind::CircularArray { src, .. }
            | FeatureKind::Mirror { src, .. }
            | FeatureKind::Hole { src, .. }
            | FeatureKind::Thread { src, .. }
            | FeatureKind::Auger { src, .. }
            | FeatureKind::Move { src, .. }
            | FeatureKind::Piece { src, .. }
            // A face copy depends on its source body but does not consume it: the body stays where it is while
            // the surface lives alongside. Otherwise taking a face into the surface layer would lose the
            // part.
            | FeatureKind::FaceCopy { src, .. }
            | FeatureKind::OffsetSurface { src, .. }
            | FeatureKind::MeshSolid { src, .. }
            | FeatureKind::MeshRecognised { src, .. }
            | FeatureKind::Patch { src, .. } => vec![src],
            FeatureKind::SurfaceReplace { src, surface, .. } => vec![src, surface],
            FeatureKind::Stitch { ref parts, .. } => parts.clone(),
            FeatureKind::Trim { src, tool, .. } => vec![src, tool],
            _ => Vec::new(),
        }
    }

    /// Re-point the body input of a node from `from` to `to`, leaving sketches alone. Used when restructuring
    /// chains — for example while editing a grouped cut — so that consumers of the old last body point at the
    /// new one.
    ///
    /// A piece made a part is not handed over: it IS that piece, and the whole body its split or cut came from would
    /// make the whole of the part it left a second part, silently. Its base deleted, it stands red with the reason.
    pub fn remap_body_input(&mut self, from: Id, to: Id) {
        let fix = |x: &mut Id| {
            if *x == from {
                *x = to;
            }
        };
        match self {
            FeatureKind::BodyBoolean { a, b, .. } => {
                fix(a);
                fix(b);
            }
            FeatureKind::Combine { src, .. }
            | FeatureKind::Fillet { src, .. }
            | FeatureKind::Chamfer { src, .. }
            | FeatureKind::Shell { src, .. }
            | FeatureKind::Draft { src, .. }
            | FeatureKind::Loft { src, .. }
            | FeatureKind::LinearArray { src, .. }
            | FeatureKind::CircularArray { src, .. }
            | FeatureKind::Mirror { src, .. }
            | FeatureKind::Hole { src, .. }
            | FeatureKind::Thread { src, .. }
            | FeatureKind::Auger { src, .. }
            | FeatureKind::Move { src, .. }
            | FeatureKind::SplitBody { src, .. }
            // a body read without being consumed is handed over the same way: a patch over the rim of a deleted
            // hole left naming the hole's body built green step by step, over the live body the deletion left
            // behind, and red rebuilt from the start
            | FeatureKind::Revolve { src, .. }
            | FeatureKind::Sweep { src, .. }
            | FeatureKind::PushFace { src, .. }
            | FeatureKind::RemoveFace { src, .. }
            | FeatureKind::FaceCopy { src, .. }
            | FeatureKind::OffsetSurface { src, .. }
            | FeatureKind::Patch { src, .. }
            | FeatureKind::MeshSolid { src, .. }
            | FeatureKind::MeshRecognised { src, .. }
            | FeatureKind::SplitFace { src, .. } => fix(src),
            FeatureKind::SurfaceReplace { src, surface, .. } => {
                fix(src);
                fix(surface);
            }
            FeatureKind::Trim { src, tool, .. } => {
                fix(src);
                fix(tool);
            }
            FeatureKind::Thicken { src, join, .. } => {
                fix(src);
                fix(join);
            }
            FeatureKind::Stitch { parts, .. } => parts.iter_mut().for_each(fix),
            _ => {}
        }
    }

    /// Remap every id reference of a node through a map, as used by a deep clone of a component subtree: the
    /// output body, the input bodies, the sketch, the datum plane, point and axis, and the datum axes and
    /// planes of features. `profile` (a sketch contour) and `face` (a persistent face) are handled separately
    /// by `remap_profile`.
    pub fn remap_ids(&mut self, map: &std::collections::HashMap<Id, Id>) {
        let m = |x: &mut Id| {
            if let Some(&n) = map.get(x) {
                *x = n;
            }
        };
        match self {
            FeatureKind::MirrorPart { body, .. } => {
                m(body); // `src_comp` is a component and is remapped by the component map separately.
            }
            FeatureKind::PartInstance { body, .. } => {
                m(body); // `src_comp` is a component and is remapped by the component map separately.
            }
            FeatureKind::ComponentPattern { bodies, .. } => {
                bodies.iter_mut().for_each(m); // the source and the copies are components, remapped separately
            }
            FeatureKind::PushFace { src, body, .. }
            | FeatureKind::FaceCopy { src, body, .. }
            | FeatureKind::OffsetSurface { src, body, .. }
            | FeatureKind::Patch { src, body, .. }
            | FeatureKind::MeshSolid { src, body, .. }
            | FeatureKind::MeshRecognised { src, body, .. } => {
                m(src);
                m(body);
            }
            FeatureKind::SurfaceReplace { src, surface, body, .. } => {
                m(src);
                m(surface);
                m(body);
            }
            FeatureKind::Trim { src, tool, body, .. } => {
                m(src);
                m(tool);
                m(body);
            }
            FeatureKind::Stitch { parts, body, .. } => {
                for p in parts.iter_mut() {
                    m(p);
                }
                m(body);
            }
            FeatureKind::RemoveFace { src, body, .. } => {
                m(src);
                m(body);
            }
            FeatureKind::Thicken { src, join, body, .. } => {
                m(src);
                m(join);
                m(body);
            }
            FeatureKind::SplitFace { src, datum, body, face, .. } => {
                m(src);
                m(datum);
                m(body);
                if let Some((b, _)) = face {
                    m(b);
                }
            }
            FeatureKind::SplitBody { src, datum, bodies, face, .. } => {
                m(src);
                m(datum);
                bodies.iter_mut().for_each(&m);
                if let Some((b, _)) = face {
                    m(b);
                }
            }
            FeatureKind::Sketch { sketch } => m(sketch),
            FeatureKind::Plane { plane } => m(plane),
            FeatureKind::DatumPoint { point } => m(point),
            FeatureKind::DatumAxis { axis } => m(axis),
            FeatureKind::Extrude { sketch, body, .. } => {
                m(sketch);
                m(body);
            }
            FeatureKind::Revolve { sketch, axis_datum, src, body, .. } => {
                m(sketch);
                m(axis_datum);
                m(src);
                m(body);
            }
            FeatureKind::Sweep { sketch, path_sketch, src, body, .. } => {
                m(sketch);
                m(path_sketch);
                m(src);
                m(body);
            }
            FeatureKind::Loft { sketches, src, body, .. } => {
                for s in sketches.iter_mut() {
                    m(s);
                }
                m(src);
                m(body);
            }
            FeatureKind::Box3 { body, .. }
            | FeatureKind::Cylinder { body, .. }
            | FeatureKind::Sphere { body, .. }
            | FeatureKind::Cone { body, .. }
            | FeatureKind::Torus { body, .. }
            | FeatureKind::Prism { body, .. } => m(body),
            FeatureKind::Combine { src, sketch, body, .. } => {
                m(src);
                m(sketch);
                m(body);
            }
            FeatureKind::Fillet { src, body, .. }
            | FeatureKind::Chamfer { src, body, .. }
            | FeatureKind::Shell { src, body, .. }
            | FeatureKind::Draft { src, body, .. }
            | FeatureKind::LinearArray { src, body, .. }
            | FeatureKind::Hole { src, body, .. }
            | FeatureKind::Thread { src, body, .. }
            | FeatureKind::Auger { src, body, .. }
            | FeatureKind::Move { src, body, .. }
            | FeatureKind::Piece { src, body, .. } => {
                m(src);
                m(body);
            }
            FeatureKind::CircularArray { src, axis, body, .. } => {
                m(src);
                m(axis);
                m(body);
            }
            FeatureKind::Mirror { src, datum, body, face, .. } => {
                m(src);
                m(datum);
                m(body);
                if let Some((b, _)) = face {
                    m(b);
                }
            }
            FeatureKind::BodyBoolean { a, b, body, .. } => {
                m(a);
                m(b);
                m(body);
            }
            // `source` is the shared embedded file and is not part of the clone map, so it is left alone; the
            // body is remapped.
            FeatureKind::Import { body, .. } | FeatureKind::MeshPiece { body, .. } => m(body),
        }
    }

    /// Remap topological names (face and edge descriptors) through a map of old name to new.
    ///
    /// The counterpart of [`FeatureKind::remap_ids`], without which a cloned part is incomplete. An id and a
    /// name are different things: `remap_ids` re-points references to bodies and sketches, while a face or an
    /// edge is addressed by a name from the recipe ("wall of feature 43 from entity 7"). A copy has different
    /// feature ids, hence different face names.
    ///
    /// Measured on a thread: a copied part went red with "thread crest not found" and drifted, because
    /// `Thread.edge` in the copy still pointed at an edge of the original. Shell, draft, fillet and a hole on
    /// a face would break just as silently, their references being of the same kind.
    pub fn remap_names(&mut self, nmap: &std::collections::HashMap<u32, u32>) {
        let m = |x: &mut u32| {
            if let Some(&n) = nmap.get(x) {
                *x = n;
            }
        };
        match self {
            FeatureKind::Fillet { edges, .. } => edges.remap_descs(&mut |d| m(d)),
            FeatureKind::Chamfer { edges, ref_face, .. } => {
                edges.remap_descs(&mut |d| m(d));
                m(ref_face);
            }
            FeatureKind::Shell { faces, .. } => faces.remap_descs(&mut |d| m(d)),
            FeatureKind::Draft { faces, neutral, .. } => {
                faces.remap_descs(&mut |d| m(d));
                neutral.remap_descs(&mut |d| m(d));
            }
            FeatureKind::Thicken { face, .. } => m(face),
            // A query reference is carried over through its own descriptors: a query has no single id, but it
            // does contain the descriptors written into it, and those are what get remapped.
            FeatureKind::Hole { face, .. } | FeatureKind::PushFace { face, .. } => face.remap_descs(&mut |d| m(d)),
            FeatureKind::RemoveFace { faces, .. } => faces.remap_descs(&mut |d| m(d)),
            // Surface features hold references of the same kind: without translation a cloned part and a
            // deleted node would break them exactly as they broke a thread.
            FeatureKind::FaceCopy { faces, .. } | FeatureKind::OffsetSurface { faces, .. } | FeatureKind::SurfaceReplace { faces, .. } => faces.remap_descs(&mut |d| m(d)),
            FeatureKind::Patch { edges, .. } => edges.remap_descs(&mut |d| m(d)),
            FeatureKind::Thread { edge, .. } | FeatureKind::Auger { edge, .. } => m(edge),
            // The rest address geometry through ids (a sketch, a datum, a body), which `remap_ids` handles.
            _ => {}
        }
    }

    /// Remap `profile` (the id of a specific sketch contour) through the contour map, after sketches have been
    /// cloned and regenerated.
    pub fn remap_profile(&mut self, cmap: &std::collections::HashMap<Id, Id>) {
        let remap = |p: &mut Id| {
            if let Some(&n) = cmap.get(p) {
                *p = n;
            }
        };
        match self {
            FeatureKind::Extrude { profiles, fill, .. } | FeatureKind::Combine { profiles, fill, .. } => {
                for p in profiles.iter_mut() {
                    remap(p);
                }
                for f in fill.iter_mut() {
                    remap(f);
                }
            }
            FeatureKind::Revolve { profiles, axis_line, .. } => {
                for p in profiles.iter_mut() {
                    remap(p);
                }
                remap(axis_line);
            }
            // Profile contours and the path contour, all of which live in sketches.
            FeatureKind::Sweep { profiles, path, .. } => {
                for p in profiles.iter_mut() {
                    remap(p);
                }
                remap(path);
            }
            // A loft has one contour per section.
            FeatureKind::Loft { contours, .. } => {
                for c in contours.iter_mut() {
                    remap(c);
                }
            }
            _ => {}
        }
    }

    /// The body a node consumes or replaces (for modifier features). That body is hidden so only the result of
    /// the chain stays visible. This is the primary one, used for lineage and colour; the full list is
    /// `consumed()`.
    pub fn consumed_body(&self) -> Option<Id> {
        match *self {
            // A loft producing a separate body consumes nothing, while a lofted boolean (non-zero `src`)
            // consumes its target body. The same holds for revolve and sweep: with `src == 0` the result is a
            // new body, and with a non-zero `src` it is a boolean whose source body has to be hidden.
            // Otherwise the original part stays next to the result of the cut, on screen and in exports,
            // giving two parts instead of one.
            FeatureKind::Loft { src, .. } | FeatureKind::Revolve { src, .. } | FeatureKind::Sweep { src, .. } if src != 0 => Some(src),
            FeatureKind::Combine { src, .. }
            | FeatureKind::Fillet { src, .. }
            | FeatureKind::Chamfer { src, .. }
            | FeatureKind::Shell { src, .. }
            | FeatureKind::Draft { src, .. }
            | FeatureKind::PushFace { src, .. }
            | FeatureKind::RemoveFace { src, .. }
            | FeatureKind::LinearArray { src, .. }
            | FeatureKind::CircularArray { src, .. }
            | FeatureKind::Mirror { src, .. }
            | FeatureKind::Hole { src, .. }
            | FeatureKind::Thread { src, .. }
            | FeatureKind::Auger { src, .. }
            | FeatureKind::Move { src, .. }
            // the piece that went into a part of its own leaves the part it came from
            | FeatureKind::Piece { src, .. }
            | FeatureKind::SplitBody { src, .. }
            // A thicken carries its body onward. Leaving the plate as a separate body turns one part into
            // two, visible on screen as a differently coloured piece, which the "one part is one body" rule
            // forbids: the plate is welded on and the source is consumed.
            | FeatureKind::Thicken { src, .. }
            // Splitting faces carries the same body onward (there is only one body), so the source is consumed
            // as it is for a chamfer.
            | FeatureKind::SplitFace { src, .. }
            // a mesh turned into a solid is carried on by the solid: the mesh would otherwise stand beside it as a
            // second body of the part
            | FeatureKind::MeshSolid { src, .. }
            | FeatureKind::MeshRecognised { src, .. } => Some(src),
            FeatureKind::BodyBoolean { a, .. } => Some(a), // The base is primary, for lineage; `b` is in `consumed()` too.
            // For a stitch the lineage follows the first sheet; the rest are in `consumed()` as well.
            FeatureKind::Stitch { ref parts, .. } => parts.first().copied(),
            // A trim carries the trimmed sheet onward; the tool stays where it is and is not consumed.
            FeatureKind::Trim { src, .. } => Some(src),
            _ => None,
        }
    }

    /// Every body a node consumes and hides. For most features that is `[consumed_body]`; for a body boolean it
    /// is both operands `[a, b]`, so only the result stays visible.
    pub fn consumed(&self) -> Vec<Id> {
        match *self {
            FeatureKind::BodyBoolean { a, b, .. } => vec![a, b],
            // A face replacement consumes both the base and the surface; what lives on is the result. Leaving
            // the sheet visible would show the part plus another surface on top of it.
            FeatureKind::SurfaceReplace { src, surface, .. } => vec![src, surface],
            // A thickened sheet welded back into a part consumes the part as well: what lives on is the
            // result, not the part with a lid next to it.
            FeatureKind::Thicken { src, join, .. } if join != 0 => vec![src, join],
            // A stitch consumes every piece: what lives on is one surface, not that surface plus its
            // parts.
            FeatureKind::Stitch { ref parts, .. } => parts.clone(),
            _ => self.consumed_body().into_iter().collect(),
        }
    }
}
