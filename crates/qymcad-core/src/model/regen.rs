//! Timeline rebuild: one pass that builds the model from the feature tree.
//!
//! `regenerate` lives here together with the helpers nothing else calls. The split is mechanical, by call
//! graph rather than by name, which makes this a closed unit: the module has exactly one entry point,
//! `regenerate`, and all the machinery of a single pass (re-pointing references at new bodies, face names from
//! recipes, the extent of a tool body, resolving a revolve axis) stays hidden.
//!
//! The rebuild is the most interconnected part of the core, and keeping it in a general file next to two
//! hundred unrelated methods means starting every edit by working out what here is even connected.

use super::*;
use crate::feature::FeatureKind;
use super::tess::*; // 2D sketch geometry: profiles, tessellation, region analysis.

/// EVERYTHING ONE NODE'S REBUILD NEEDS besides the document itself.
///
/// Forty kinds of feature are rebuilt in one pass, and every one of them wants the same handful of things:
/// the kernel, the parameter values, the dimension expressions, the rename map, the set of bodies that
/// changed and the report. Passing those as six arguments to forty functions is how a signature grows past
/// reading — a complaint this very refactor exists to answer — so they travel as one.
///
/// The fields are locals OF THE PASS, not parts of the document. That is what makes the split possible at
/// all: a `&mut Project` and a `&mut Pass` are disjoint borrows, so a branch can take the document mutably
/// and still write its findings here.
/// WHERE THE RESULT OF ONE REBUILD LANDS: the node it belongs to, the kernel that built it, and the
/// three books kept for the whole pass - what changed, what went wrong, and how edges were renamed.
///
/// Every branch of the rebuild ends by handing its result over with exactly these five, and they were
/// spelled out at thirty-seven call sites.
pub(crate) struct Landing<'a, 'b> {
    /// The node being rebuilt: the owner of every name minted while it is built.
    node: Id,
    /// The bodies that changed, whose consumers must therefore rebuild.
    dirty: &'a mut std::collections::HashSet<Id>,
    report: &'a mut crate::feature::RegenReport,
    kernel: &'b dyn crate::feature::Kernel,
    /// "Old edge number -> new name" for the bodies built so far in this pass.
    emap: &'a mut EdgeRenames,
}

pub(crate) struct Pass<'a> {
    kernel: &'a dyn crate::feature::Kernel,
    /// The values of the global parameters, for the dimension expressions.
    vars: &'a std::collections::HashMap<String, f64>,
    /// The dimension expressions of the node being rebuilt, if it has any.
    dims: Option<&'a std::collections::HashMap<String, String>>,
    /// "Old edge number -> new name" for the bodies built so far in this pass.
    emap: &'a mut EdgeRenames,
    /// The bodies that changed, whose consumers must therefore rebuild.
    dirty: &'a mut std::collections::HashSet<Id>,
    report: &'a mut crate::feature::RegenReport,
    /// The node being rebuilt: the owner of every name minted while it is built.
    node: Id,
}

impl Pass<'_> {
    /// A dimension of this node: the expression where there is one, the stored number otherwise.
    fn dim(&self, key: &str, stored: f64) -> f64 {
        eval_dim(self.dims, key, stored, self.vars)
    }
}

/// A NODE'S PARCEL in a wave: the node, the body it writes, and the work that builds it.
pub(crate) struct Parcel {
    node: Id,
    body: Id,
    job: crate::feature::KernelJob,
}

/// A parcel sent to another thread, with the worker that holds the bodies it reads.
pub(crate) struct Travelling {
    parcel: Parcel,
    worker: Box<dyn crate::feature::KernelWorker>,
}

/// A parcel back from its thread: what it built, and the worker to give back to the shared kernel.
pub(crate) struct Returned {
    node: Id,
    computed: Computed,
    worker: Box<dyn crate::feature::KernelWorker>,
}

/// What a node's work came to, kept until the walk writes it down: the body it writes and the result.
pub(crate) struct Computed {
    body: Id,
    res: Result<crate::geom::Built, crate::errors::CoreError>,
}

/// Estimate of how much work a rebuild will be; see [`Project::regen_plan`].
#[derive(Default, Debug, Clone)]
pub struct RegenPlan {
    /// Nodes the rebuild will touch, with a margin.
    pub nodes: Vec<Id>,
    /// Total number of nodes in the timeline, to tell a local edit from a full rebuild.
    pub total: usize,
    /// Whether the affected set contains a known slow operation (a thread).
    pub heavy: bool,
}

/// The stored `ruled` flag as the word it means.
fn walls(ruled: bool) -> crate::feature::LoftWalls {
    if ruled {
        crate::feature::LoftWalls::Ruled
    } else {
        crate::feature::LoftWalls::Smooth
    }
}

/// WHICH BODY IS BEING REBUILT, AND HOW THE RESULT JOINS IT.
///
/// The same three arguments trail the end of nearly every rebuild branch. Together they are one thing:
/// take `src`, make something, and add or cut it into `body` according to `op`.
#[derive(Clone, Copy)]
pub struct BodyOp {
    /// The body the operation reads.
    pub src: Id,
    /// The kernel's own code for join or cut.
    pub op: u8,
    /// The body the result lands in.
    pub body: Id,
}

/// A HELICAL CUT ALONG AN EDGE: where it runs, how long, and how it eases in and out.
///
/// A thread and an auger differ only in the profile of the groove; everything about WHERE the helix goes
/// is the same six things, and both branches took them one by one.
#[derive(Clone, Copy)]
pub(crate) struct HelixRun {
    /// The body the helix is cut into.
    pub src: Id,
    /// The circular edge it follows.
    pub edge: u32,
    /// How far along the axis it runs.
    pub length: f64,
    /// A run-up before the full depth is reached, so the tool does not bite at once.
    pub lead_in: f64,
    /// The same at the far end.
    pub lead_out: f64,
    /// The body the result lands in.
    pub body: Id,
}

/// THE DRAFT ITSELF, apart from the faces it is applied to: the face that stays put, the angle, and
/// which side of it leans.
pub(crate) struct DraftShape {
    /// The neutral face - the one that keeps its size while the others lean.
    pub neutral: crate::refs::Ref,
    /// The lean, in degrees.
    pub angle: f64,
    /// Lean the other way.
    pub flip: bool,
}

/// HOW FAR AN EXTRUSION RUNS AND WHICH CONTOURS STAY SOLID.
///
/// The body spans [-down, +height] along the sketch normal, `reach` says which of the two sides is
/// actually used, and `fill` names the nested contours that are NOT to become holes.
#[derive(Clone, Copy)]
pub struct ExtrudeSpan<'a> {
    pub height: f64,
    pub down: f64,
    pub reach: crate::feature::Reach,
    pub fill: &'a [Id],
}

/// THE SAME FOR A JOIN OR A CUT, where the stop can also be "the whole way through".
#[derive(Clone, Copy)]
pub struct CombineSpan<'a> {
    pub height: f64,
    pub down: f64,
    pub extent: crate::feature::Extent,
    pub fill: &'a [Id],
}

/// WHICH PROFILE OF WHICH SKETCH. Never one without the other.
#[derive(Clone, Copy)]
pub(crate) struct Profile<'a> {
    pub sketch: Id,
    pub profiles: &'a [Id],
}

/// HOW FAR A REVOLVE TURNS, and which way it goes from the sketch plane.
#[derive(Clone, Copy)]
pub struct RevolveTurn {
    /// Degrees. 360 makes a full body of revolution.
    pub angle: f64,
    pub reach: crate::feature::Reach,
}

/// THE AXIS A REVOLVE TURNS ABOUT, in the three ways it can be given.
#[derive(Clone, Copy)]
pub struct RevolveAxis {
    /// A world axis by number, when neither of the two below is set.
    pub axis: u8,
    /// A datum axis.
    pub datum: Id,
    /// A line of the sketch itself.
    pub line: Id,
}

/// THE SHAPE OF A CHAMFER: symmetric, two-distance or angled, and which face the sizes are measured from.
#[derive(Clone, Copy)]
pub struct ChamferShape {
    pub mode: crate::feature::ChamferMode,
    /// The second distance (or the angle), for the asymmetric modes.
    pub d2: f64,
    /// Measure from the other side of the edge.
    pub flip: bool,
    /// The face the distances are measured from; zero lets the kernel choose.
    pub ref_face: u32,
}

/// WHERE A HOLE IS DRILLED: either a face found by recipe, or the isolated points of a sketch.
///
/// The two ways are exclusive - `sketch` non-zero means the points, and then `face` is not consulted -
/// and they used to travel as five loose arguments in the middle of a signature of fourteen.
pub(crate) struct HoleSite {
    /// The face, found by recipe. Not consulted when a sketch places the holes.
    pub face: crate::refs::Ref,
    /// The point and the normal recorded when the reference was made - the fallback if the recipe misses.
    pub point: [f64; 3],
    pub normal: [f64; 3],
    /// The sketch whose isolated points place the holes; zero means the face above.
    pub sketch: Id,
    /// Drill against the sketch's normal.
    pub flip: bool,
}

/// WHAT DRILLS IT: the cylinder, and the counterbore or countersink above it.
#[derive(Clone, Copy)]
pub struct HoleTool {
    /// Plain, counterbored or countersunk - the kernel's own code.
    pub kind: u8,
    /// The through cylinder.
    pub diameter: f64,
    pub depth: f64,
    /// The wider part at the top; zero when there is none.
    pub dia2: f64,
    pub depth2: f64,
}

/// ONE DIRECTION OF A LINEAR PATTERN: the step vector and how many copies go along it.
///
/// The rebuild used to take these twelve numbers and three counts as fifteen separate arguments, and the
/// signature ran to sixteen places. Reading it, one had to count commas to tell `dy2` from `dz2`.
#[derive(Clone, Copy)]
pub struct ArrayAxis {
    /// The step, as a vector in the body's space.
    pub d: [f64; 3],
    /// How many copies along it, the original included. One or less means this direction is unused.
    pub count: u32,
}

impl ArrayAxis {
    /// A DIRECTION THAT IS NOT USED. A grid of one row still has three directions; two of them are this.
    pub fn none() -> Self {
        ArrayAxis { d: [0.0; 3], count: 1 }
    }
}

impl<'a> Pass<'a> {
    /// The five things a finished branch hands its result over with.
    fn landing(&mut self) -> Landing<'_, 'a> {
        Landing { node: self.node, dirty: self.dirty, report: self.report, kernel: self.kernel, emap: self.emap }
    }
}

impl Project {}

impl Project {
    /// The error of the first value of node `node` written as an expression that does not count with `vars`.
    fn dim_expr_error(&self, node: Id, vars: &std::collections::HashMap<String, f64>) -> Option<crate::errors::CoreError> {
        self.feat_dims.get(&node)?.values().map(|e| e.trim()).filter(|e| !e.is_empty()).find_map(|e| crate::expr::eval(e, vars).err()).map(crate::errors::CoreError::Expr)
    }
}

/// WHETHER EVERY SIZE OF A PRIMITIVE IS ABOVE ZERO. A box, a cylinder or a sphere of nothing reached the kernel and came
/// back as a broken body or as "the operation failed" (measured on all six primitives), where the cause is the size the
/// person typed.
fn above_zero(sizes: &[f64]) -> bool {
    sizes.iter().all(|s| *s > 1e-9)
}

/// `mesh` made lighter within `simplify` mm, or as it is where that is zero.
fn lighter(mesh: crate::geom::Mesh, simplify: f64) -> crate::geom::Mesh {
    if simplify > 0.0 {
        crate::mesh_simplify::simplify(&mesh, simplify)
    } else {
        mesh
    }
}

/// The recognition's tolerance factor, raised so that a surface is held within `simplify` mm at least: a mesh made
/// lighter stands off the surface it was made from by up to that, and held to the recognition's own hundred-thousandth
/// of its size no plane or cylinder lay on it - measured on a smooth handle simplified by 0.01 mm, 8 544 regions came
/// out of 21 234 triangles and 1 164 of them stayed mesh; within the simplification, 159 regions and none.
fn within_simplification(mesh: &crate::geom::Mesh, tol: f64, simplify: f64) -> f64 {
    let Some(b) = mesh.bounds().filter(|_| simplify > 0.0) else { return tol };
    let size = ((b.max.x - b.min.x).powi(2) + (b.max.y - b.min.y).powi(2) + (b.max.z - b.min.z).powi(2)).sqrt();
    // the recognition holds a corner within a hundred-thousandth of the mesh's size times this factor
    tol.max(simplify / (size * 1e-5).max(1e-12))
}

/// How many edges a reference names: the length of a plain pick list, and none for a description, whose numbers
/// name the faces or the seed it is phrased through rather than edges.
fn asked_edge_count(edges: &crate::refs::Ref) -> usize {
    if edges.query.is_pick_list() {
        edges.query.picked_descs().len()
    } else {
        0
    }
}

/// The refusal for an edge reference that found nothing: the `asked` named edges lost for a pick list, a description
/// that finds nothing otherwise. Reported behaviour: "every edge of the top face" was refused as "Not one of the 1
/// named edges is left", the face number counted as an edge.
fn edges_lost(edges: &crate::refs::Ref, asked: usize) -> crate::errors::CoreError {
    if edges.query.is_pick_list() {
        crate::errors::CoreError::EdgesNotFound { asked }
    } else {
        crate::errors::CoreError::DescribedEdgesNotFound
    }
}

mod refs;
mod run;
mod prep_ops;
mod prep_bodies;
mod prep_profiles;
mod apply;
