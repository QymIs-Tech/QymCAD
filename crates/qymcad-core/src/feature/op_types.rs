//! THE SHAPES OF THE VALUES OF OPERATIONS: sides, purposes, extents, walls, helices, the records a kernel answers with.

use super::*;

/// WHICH WAY ROUND TWO CONNECTORS MEET: whether one is turned to face the other, and whether it is rolled
/// half a turn about its own axis.
///
/// It used to travel as a bare `(bool, bool)`, and `side_turn(side.0, side.1)` says nothing at all about
/// which is which - the worst reading of the lot, because the two are easy to swap and the result is a part
/// mated back to front.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Side {
    /// the connector faces the other way
    pub flip: bool,
    /// the connector is rolled half a turn about its own axis
    pub roll_flip: bool,
}

/// WHAT A SKETCH ENTITY IS FOR: it becomes part of the shape, or it only helps to build one.
///
/// Every builder of the sketcher ended with a bare `construction`, and `add_line_entity(si, 0.0, 0.0, 10.0,
/// 0.0, crate::feature::Purpose::Real)` says nothing at all about what that last `false` decides - while the difference is whether
/// the line is extruded or ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// part of the shape: it forms contours and is extruded, revolved, swept
    Real,
    /// construction geometry: it holds constraints and dimensions, and no contour is taken from it
    Construction,
}

impl Purpose {
    /// The window keeps the toggle as a flag; the model keeps a word. One conversion, at the boundary.
    pub fn of(construction: bool) -> Self {
        if construction {
            Self::Construction
        } else {
            Self::Real
        }
    }
}

/// WHICH WAY AN ARC GOES from its first end to its second.
///
/// It used to be a bare `ccw` next to a bare `construction`, and `add_arc_entity(si, 0.0, 0.0, 5.0, 0.0,
/// 0.0, 5.0, crate::feature::Winding::Ccw, crate::feature::Purpose::Real)` says which is which only to whoever wrote it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Winding {
    /// counter-clockwise, the positive direction of the sketch plane
    Ccw,
    /// clockwise
    Cw,
}

/// WHETHER A SPLINE CLOSES on itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ends {
    /// the last point does not join the first
    Open,
    /// the curve is a loop
    Closed,
}

/// WHICH WAY A TOOL GROWS from the sketch plane: along the normal, against it, or half each way.
///
/// Two booleans used to say this - `symmetric` and `flip` - and the first swallowed the second whole:
/// `(symmetric: true, flip: true)` and `(symmetric: true, flip: false)` produce the very same extent, so a
/// document could carry a reversal that meant nothing and a reader could not tell which of the two had been
/// intended. Three states are three words, and the fourth combination stops existing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reach {
    /// along the normal of the sketch plane
    #[default]
    Forward,
    /// against the normal
    Backward,
    /// half of the height to each side of the plane
    BothWays,
}

/// HOW FAR A TOOL GOES from the sketch plane, and which way.
///
/// Three booleans in a row used to say this, and `add_combine_on(base, sid, c, 20.0, 0, true, true, false,
/// 0.0)` tells a reader nothing about which of them is which. Named fields cost one word each at the call
/// site, and the plain case - one-sided, forwards, by the given height - is the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extent {
    /// go the whole way through whatever stands in the way, ignoring the height
    pub through: bool,
    /// which way the tool grows from the plane
    pub reach: Reach,
}

/// WHICH WAY THE WALL OF A SHELL GOES from the face it is measured on.
///
/// Two booleans used to say this - `outward` and `center`, with the second quietly overriding the first - so
/// `(false, true)` and `(true, true)` meant the same thing and nothing at the call site said which. Three
/// states are three words, and the document stores the word: two flags in the record left the same trap one
/// layer down, where a "centred" shell also carried a direction nobody read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ShellSide {
    /// the wall is taken out of the body, the outer surface staying where it was
    #[default]
    Inward,
    /// the wall is added outside, the inner surface staying where it was
    Outward,
    /// the wall straddles the face, half of it on each side
    Centred,
}

/// HOW THE SECTIONS OF A LOFT ARE JOINED: straight from one to the next, or through a smooth surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoftWalls {
    /// a smooth surface through the sections
    Smooth,
    /// straight strips between neighbouring sections
    Ruled,
}

/// WHAT A LOFT PRODUCES: a solid with its ends closed, or an open sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoftBody {
    /// a solid, its ends capped
    Solid,
    /// a sheet, left open
    Sheet,
}

/// WHAT A HELICAL CUT OR RIB IS MADE OF, in one piece.
///
/// Sixteen arguments in a row is a signature nobody reads, and that was proved the hard way: adding ONE
/// parameter to it meant edits in four layers, two of them were forgotten, and only a full run found them.
/// Named fields cost nothing at the call site and cannot be swapped for one another by accident.
pub struct Helical<'a> {
    /// the body being written
    pub body: Id,
    /// the body the helix is cut from or welded onto
    pub src: Id,
    /// a point on the axis
    pub origin: [f64; 3],
    /// the direction of the axis
    pub dir: [f64; 3],
    /// the radius of the surface the profile is swept on
    pub radius: f64,
    /// the axial section from `qymcad_core::thread`, encoded as an exact profile
    pub profile: &'a [f64],
    /// how far the helix runs along the axis
    pub length: f64,
    /// the rise of one turn
    pub lead: f64,
    /// how many starts
    pub starts: u32,
    /// a left-hand helix
    pub left: bool,
    /// unite the profile (an auger flight) rather than subtract it (a thread)
    pub fuse: bool,
    /// the run-out at the start
    pub lead_in: f64,
    /// the run-out at the end
    pub lead_out: f64,
    /// the node: the faces of the groove and of the run-out are named after it
    pub feature: Id,
    /// how far the whole profile is moved in radially, to take up the clearance the groove could not
    pub crest_relief: f64,
}

/// The geometry kernel (OCCT behind FFI in `qymcad-kernel`).
///
/// The model core is headless, so the application injects the implementation through this trait and
/// `Project::regenerate` can be tested against a mock.
///
/// The kernel caches the B-rep shape by body id: modifier features (`combine`, `fillet`, `chamfer`) read the
/// shape of their source body `src`. `place` is the 3x4 transform onto the sketch plane (`PLACE_IDENTITY` for
/// primitives). Every method returns the mesh and the faces of the result.
/// A LINE IN SPACE: a point on it and the direction along it.
///
/// It travelled as an unnamed pair - and as `Option<([f64; 3], [f64; 3])>` at that, where nothing said
/// which of the two triples was the point and which the direction.
#[derive(Clone, Copy)]
pub struct AxisLine {
    pub origin: [f64; 3],
    pub dir: [f64; 3],
}

/// WHAT A REVOLVE TURNS ABOUT: a world axis by number, or a line given outright.
///
/// The two are exclusive - a line, when there is one, wins - and the kernel needs both to tell the
/// refusal apart: turning about a world axis and about a given line fail for different reasons.
#[derive(Clone, Copy)]
pub struct RevolveAbout {
    /// The world axis by number, used when `line` is `None`.
    pub axis: u8,
    pub line: Option<AxisLine>,
}

/// WHETHER THE FACES A BLEND MAKES ARE NAMED, and after which node: the rule of the names - a surface after its edge, a
/// corner patch after the edge too - is the bridge's; the model says only which node the blend is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendRoles {
    Named { feature: Id },
    Unnamed,
}

/// A BODY BROUGHT IN, AS THE DOOR LANDED IT: its mesh and faces, and why it was kept as it came when the door could not
/// clean it (`None`: it passed).
pub struct ImportLanded {
    pub built: Built,
    pub kept: Option<crate::errors::CoreError>,
}

/// A RADIUS SET AT A VERTEX of a fillet: where the vertex stands and the radius there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadiusAt {
    pub at: [f64; 3],
    pub radius: f64,
}

/// HOW MANY OF THE EDGES ASKED A BLEND LEFT OUT, said in yellow on its node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DroppedEdges {
    pub asked: usize,
    pub dropped: usize,
}

/// HOW FAR A DRAFT LEANS AND WHICH WAY THE MOULD COMES OUT.
///
/// The angle alone says nothing: the same lean tilts the faces one way or the other depending on the
/// pull. The two are one instruction and were two arguments.
#[derive(Clone, Copy)]
pub struct DraftPull {
    /// Degrees.
    pub angle: f64,
    /// The direction the mould is drawn in.
    pub dir: [f64; 3],
}

/// WHAT A MIRROR MAKES: what it keeps, and what its image is called.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mirrored {
    pub keeps: MirrorKeeps,
    pub names: ImageNames,
}

/// WHAT A MIRROR KEEPS: the image alone, or the image united with the body it reflects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MirrorKeeps {
    Image,
    Both,
}

/// WHAT THE IMAGE OF A MIRROR IS CALLED: the names of the faces it reflects - a mirrored part is a body of its own,
/// nothing to tell it from - or the names of an instance of the node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImageNames {
    Source,
    Instance { feature: Id },
}

/// A PLANE GIVEN BY A POINT ON IT AND ITS NORMAL.
#[derive(Clone, Copy)]
pub struct PlaneAt {
    pub origin: [f64; 3],
    pub normal: [f64; 3],
}

/// THE SECTIONS OF A LOFT: the encoded contours, where each begins, and where each is placed.
///
/// Three parallel arrays that are one thing; separately, nothing said that the second indexes into the
/// first.
#[derive(Clone, Copy)]
pub struct LoftSections<'a> {
    pub data: &'a [f64],
    pub offsets: &'a [usize],
    pub places: &'a [f64],
}

/// A PROFILE AND HOW FAR IT IS EXTRUDED - the pair that makes a solid out of a flat contour.
#[derive(Clone, Copy)]
pub struct Extruded<'a> {
    pub profile: &'a [f64],
    pub height: f64,
}

/// A PARCEL OF WORK FOR THE KERNEL: one node's geometry, with nothing borrowed from the document.
///
/// The rebuild of a node has three parts: reading the document (which references resolve to what, what the
/// numbers are), asking the kernel to build the geometry, and writing the result back. Only the middle part is
/// slow, and only it can be done for several nodes at once - but it could not be lifted out while it sat in
/// the middle of a method holding `&mut Project`.
///
/// A parcel is that middle part, already prepared: the ids of the bodies whose shapes it needs, and the work
/// itself. It owns everything it uses, so it can be handed to another thread: ownership is passed, a cache is
/// not shared.
pub struct KernelJob {
    pub(crate) inputs: Vec<Id>,
    pub(crate) work: Box<KernelWork>,
}

/// The work of a parcel: run once, on whichever kernel holds the bodies it reads.
pub(crate) type KernelWork = dyn FnOnce(&dyn Kernel) -> Result<Built, crate::errors::CoreError> + Send;

impl KernelJob {
    /// `inputs` are the bodies whose live shapes the work reads; they are what a worker has to be given.
    pub fn new(inputs: Vec<Id>, work: impl FnOnce(&dyn Kernel) -> Result<Built, crate::errors::CoreError> + Send + 'static) -> Self {
        Self { inputs, work: Box::new(work) }
    }

    /// A parcel that asks for nothing: the node refused before the kernel was needed at all.
    pub fn refused(e: crate::errors::CoreError) -> Self {
        Self::new(Vec::new(), move |_| Err(e))
    }

    pub fn inputs(&self) -> &[Id] {
        &self.inputs
    }

    pub fn run(self, kernel: &dyn Kernel) -> Result<Built, crate::errors::CoreError> {
        (self.work)(kernel)
    }
}

/// A NAME GIVEN IN PLACE OF ANOTHER: the element that was `was` is `now`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rename {
    pub was: u32,
    pub now: u32,
}

/// AN EDGE AND THE TWO FACES IT RUNS BETWEEN, by name - for a free edge of a sheet, its face and the face that stood
/// beside it in the body it was cut from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeFaces {
    pub edge: u32,
    pub faces: [u32; 2],
}

/// AN EDGE AND THE TWO SMALLEST NAMED FACES MEETING AT ITS ENDS, apart from its own two (0 where there is none): what
/// tells two edges between the same pair of faces apart by the recipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeEnds {
    pub edge: u32,
    pub around: [u32; 2],
}

/// A PIECE OF A FACE AN OPERATION CUT IN SEVERAL: the piece, the face it was cut from, and its number among the
/// pieces as the history gave them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceSplit {
    pub piece: u32,
    pub origin: u32,
    pub index: u32,
}

/// WHAT A RESULT IS NAMED BY WHERE ITS HISTORY WAS SILENT: the body, the node that made it, the faces the document
/// lists for it (none for a body raised again from its file, whose faces are asked of the kernel), the factor it was
/// brought in at, and the table the names are given by.
pub struct SilentNaming<'a> {
    pub body: Id,
    pub feature: Id,
    pub listed: Option<&'a [u32]>,
    pub scale: f64,
    pub names: &'a mut dyn crate::names::NameMint,
}

/// THE NAMES GIVEN: the faces and the edges renamed, and the names the faces merged into one yielded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamedAfter {
    pub faces: Vec<Rename>,
    pub edges: Vec<Rename>,
    pub absorbed: Vec<Absorbed>,
}

/// A NAME YIELDED: two faces merged into one (coplanar faces of a union), `loser` gave way to `winner`, and a reference
/// to the former finds the merged face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Absorbed {
    pub loser: u32,
    pub winner: u32,
}

/// HOW FAR THE TOOL OF A PROFILE OPERATION REACHES along the normal of its sketch: from `start` for `length`, and
/// whether its ends meet the body it cuts as a person drew them - a one-sided cut, whose end on a face of the body the
/// bridge breaks (OCCT leaves a cap over a coincident end), or ends kept as they are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolSpan {
    pub start: f64,
    pub length: f64,
    pub ends: ToolEnds,
}

/// WHAT BECOMES OF THE ENDS OF A TOOL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolEnds {
    /// As drawn.
    Kept,
    /// A one-sided cut: an end on a face of the body it crosses is carried past it, an end on the sketch plane is set
    /// back by a micrometre.
    OneSidedCut,
}

/// WHERE A SIDE FACE OF A PROFILE OPERATION COMES FROM: the sketch entity its edge was cut from, and the number of
/// that piece along the entity (an entity cut by its neighbours gives several pieces, each a face of its own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SideSource {
    pub src: Id,
    pub split: u16,
}

/// THE RECIPE OF THE SIDE FACES OF A PROFILE OPERATION: the node, and the sources of its sides. A profile carries in
/// the slot of the name of each edge the place of its source in `sources`, counted from 1 (0 - an edge with no
/// source). What a side face is called from that - and what the caps are - is the bridge's rule.
#[derive(Clone, Copy, Debug)]
pub struct Sides<'a> {
    pub feature: Id,
    pub sources: &'a [SideSource],
}

/// PROFILES WITH THE SOURCES OF THEIR SIDES, as the model encodes them for a profile operation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourcedProfiles {
    pub profiles: Vec<Vec<f64>>,
    pub sources: Vec<SideSource>,
}

/// WHERE A FACE LIES: the point of its surface at the middle of its parametric bounds and the normal there. What a face
/// is named by when nothing else about it is known - a face of a body brought in from a file or made of a mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FacePlace {
    pub id: u32,
    pub at: [f64; 3],
    pub normal: [f64; 3],
}
