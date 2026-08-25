//! Parametric backbone: the data model of the feature timeline.
//!
//! Sketches, datum planes and part features are nodes of one ordered timeline (`Project::timeline`).
//! References use stable ids only, never indices. The 2D geometry of a sketch stays 2D; the plane is only a
//! placement, and the plane frame lifts a 2D point into the world (`PlaneFrame::lift`).

use serde::{Deserialize, Serialize};

use crate::geom::{Mesh, MeshFace, Point2, Point3};
use crate::model::Id;

/// Base plane of the global coordinate system, used to place a sketch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BasePlane {
    XY,
    XZ,
    YZ,
}

impl Default for BasePlane {
    fn default() -> Self {
        BasePlane::XY
    }
}

/// Stable key of a planar face of a body (topological naming).
///
/// It carries the positional index of the face at the time the body was created, as a fast path, plus a
/// geometric fingerprint (centre and normal) as a fallback for when the index shifts after a rebuild.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FaceKey {
    pub index: u32,
    pub centroid: [f64; 3],
    pub normal: [f64; 3],
    /// Persistent face id from the B-rep. It survives a rebuild, so the reference does not drift. Zero means
    /// unknown (an older key, or a mesh). Resolution goes by id, with the centroid and normal fingerprint as
    /// the fallback match.
    #[serde(default)]
    pub id: u32,
}

/// Placement of a sketch in 3D. The 2D geometry is lifted into the world through the plane frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SketchPlane {
    /// A base plane of the global coordinate system (XY, XZ, YZ).
    World(BasePlane),
    /// A datum plane feature, by id (in `Project::planes`).
    Datum(Id),
    /// A planar face of a body: the body id plus the face key.
    Face(Id, FaceKey),
}

impl Default for SketchPlane {
    fn default() -> Self {
        SketchPlane::World(BasePlane::XY)
    }
}

/// Orthonormal frame of a plane: an origin plus the X and Y axes in world space (the normal is X cross Y).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneFrame {
    pub origin: [f64; 3],
    pub x: [f64; 3],
    pub y: [f64; 3],
}

impl PlaneFrame {
    /// Lift a 2D sketch point into world coordinates.
    pub fn lift(&self, p: Point2) -> Point3 {
        Point3::new(
            self.origin[0] + self.x[0] * p.x + self.y[0] * p.y,
            self.origin[1] + self.x[1] * p.x + self.y[1] * p.y,
            self.origin[2] + self.x[2] * p.x + self.y[2] * p.y,
        )
    }

    /// Project a world point into the local 2D coordinates of the frame, along its X and Y axes: the inverse
    /// of `lift`.
    ///
    /// Used to draw a body inside the 2D sketch editor on any plane (a face or a datum), so the body lines up
    /// with the sketch coordinates instead of being shown from above.
    pub fn project(&self, p: Point3) -> Point2 {
        let d = [p.x - self.origin[0], p.y - self.origin[1], p.z - self.origin[2]];
        Point2::new(d[0] * self.x[0] + d[1] * self.x[1] + d[2] * self.x[2], d[0] * self.y[0] + d[1] * self.y[1] + d[2] * self.y[2])
    }

    /// Depth of a world point along the frame normal, used for ordering and shading in the 2D projection.
    pub fn depth(&self, p: Point3) -> f64 {
        let n = self.normal();
        (p.x - self.origin[0]) * n[0] + (p.y - self.origin[1]) * n[1] + (p.z - self.origin[2]) * n[2]
    }

    /// Normal of the plane (X cross Y).
    pub fn normal(&self) -> [f64; 3] {
        [
            self.x[1] * self.y[2] - self.x[2] * self.y[1],
            self.x[2] * self.y[0] - self.x[0] * self.y[2],
            self.x[0] * self.y[1] - self.x[1] * self.y[0],
        ]
    }

    /// Build an orthonormal frame from an origin, a normal and a rotation of X about it, in degrees.
    ///
    /// The reference up direction is world +Z (or +X when the normal is nearly parallel to Z), so that X lies
    /// in the plane.
    pub fn from_origin_normal(origin: [f64; 3], normal: [f64; 3], rot_deg: f64) -> PlaneFrame {
        let n = norm3(normal).unwrap_or([0.0, 0.0, 1.0]);
        // A normal parallel to Z gives axes equal to world X and Y, the natural frame for an XY datum;
        // otherwise X = Z cross n (which lies in the plane) and Y = n cross X.
        let x0 = if n[2].abs() > 0.999 {
            [1.0, 0.0, 0.0]
        } else {
            norm3(cross3([0.0, 0.0, 1.0], n)).unwrap_or([1.0, 0.0, 0.0])
        };
        let y0 = cross3(n, x0);
        // Rotate the X and Y axes about the normal by `rot_deg`.
        let (s, c) = rot_deg.to_radians().sin_cos();
        let x = [x0[0] * c + y0[0] * s, x0[1] * c + y0[1] * s, x0[2] * c + y0[2] * s];
        let y = [y0[0] * c - x0[0] * s, y0[1] * c - x0[1] * s, y0[2] * c - x0[2] * s];
        PlaneFrame { origin, x, y }
    }


    /// Frame from an origin, a main axis (the normal) and an explicit secondary axis.
    ///
    /// The difference from `from_origin_normal` is fundamental: there the secondary axis is derived from
    /// world Z, so it depends on how the body happens to be rotated in the world and comes out differently
    /// for two bodies being mated. Here the caller supplies it, from the geometry of the body itself.
    ///
    /// `x_hint` is orthogonalised against the normal; if it is parallel to it or degenerate, the behaviour
    /// falls back to `from_origin_normal` — a predictable fallback beats a division by zero.
    pub fn from_origin_axes(origin: [f64; 3], normal: [f64; 3], x_hint: [f64; 3]) -> Self {
        let n = norm3(normal).unwrap_or([0.0, 0.0, 1.0]);
        let d = x_hint[0] * n[0] + x_hint[1] * n[1] + x_hint[2] * n[2];
        let proj = [x_hint[0] - n[0] * d, x_hint[1] - n[1] * d, x_hint[2] - n[2] * d];
        match norm3(proj) {
            Some(x) => PlaneFrame { origin, x, y: cross3(n, x) },
            None => Self::from_origin_normal(origin, normal, 0.0),
        }
    }

    /// Frame of a plane whose origin is the projection of the world origin onto that plane, rather than an
    /// arbitrary point of a face or a datum. For axis-aligned planes the local XY axes then coincide with the
    /// world ones, so the 2D sketch and the 3D view agree.
    ///
    /// `point` is any point of the plane (a face centre, a datum origin) and `normal` is its normal.
    pub fn world_aligned(point: [f64; 3], normal: [f64; 3], rot_deg: f64) -> PlaneFrame {
        let n = norm3(normal).unwrap_or([0.0, 0.0, 1.0]);
        // Foot of the perpendicular from the origin onto the plane {x : (x - point) . n = 0}.
        let d = point[0] * n[0] + point[1] * n[1] + point[2] * n[2];
        let origin = [d * n[0], d * n[1], d * n[2]];
        // In-plane axes. For an axis-aligned normal the two positive world axes serve as X and Y, ordered so
        // that X cross Y = n and the triple stays right-handed, which keeps the extrude direction correct.
        // This removes the mirrored snapping between opposing faces (+X against -X and so on): every face has
        // its origin in the same corner and the body grows towards +X and +Y. A tilted face keeps X = Z cross
        // n.
        let dom = if n[0].abs() > 0.999 {
            Some(0)
        } else if n[1].abs() > 0.999 {
            Some(1)
        } else if n[2].abs() > 0.999 {
            Some(2)
        } else {
            None
        };
        let (x0, y0) = match dom {
            Some(k) => {
                let e = |i: usize| {
                    let mut v = [0.0; 3];
                    v[i] = 1.0;
                    v
                };
                let (i, j) = match k {
                    0 => (1, 2),
                    1 => (0, 2),
                    _ => (0, 1),
                };
                let (u, v) = (e(i), e(j));
                // u cross v = +e_k: co-directed with n gives (u,v), otherwise they are swapped to keep
                // X cross Y = n.
                if cross3(u, v)[k] * n[k] > 0.0 {
                    (u, v)
                } else {
                    (v, u)
                }
            }
            None => {
                let x = norm3(cross3([0.0, 0.0, 1.0], n)).unwrap_or([1.0, 0.0, 0.0]);
                (x, cross3(n, x))
            }
        };
        // Rotate the X and Y axes about the normal by `rot_deg`.
        let (s, c) = rot_deg.to_radians().sin_cos();
        let x = [x0[0] * c + y0[0] * s, x0[1] * c + y0[1] * s, x0[2] * c + y0[2] * s];
        let y = [y0[0] * c - x0[0] * s, y0[1] * c - x0[1] * s, y0[2] * c - x0[2] * s];
        PlaneFrame { origin, x, y }
    }

    /// Lift a local 3D point (in frame axes) into the world: origin + x*lx + y*ly + n*lz.
    pub fn lift3(&self, p: Point3) -> Point3 {
        let n = self.normal();
        Point3::new(
            self.origin[0] + self.x[0] * p.x + self.y[0] * p.y + n[0] * p.z,
            self.origin[1] + self.x[1] * p.x + self.y[1] * p.y + n[1] * p.z,
            self.origin[2] + self.x[2] * p.x + self.y[2] * p.y + n[2] * p.z,
        )
    }

    /// Rotate a local direction (a face normal, for example) into the world, without translation.
    pub fn rotate_dir(&self, d: [f64; 3]) -> [f64; 3] {
        let n = self.normal();
        [
            self.x[0] * d[0] + self.y[0] * d[1] + n[0] * d[2],
            self.x[1] * d[0] + self.y[1] * d[1] + n[1] * d[2],
            self.x[2] * d[0] + self.y[2] * d[1] + n[2] * d[2],
        ]
    }

    /// Placement as a 3x4 row-major matrix (the X, Y and N axes as columns plus the origin), for a kernel
    /// transform.
    pub fn matrix12(&self) -> [f64; 12] {
        let n = self.normal();
        [
            self.x[0], self.y[0], n[0], self.origin[0],
            self.x[1], self.y[1], n[1], self.origin[1],
            self.x[2], self.y[2], n[2], self.origin[2],
        ]
    }

    /// Whether the frame is the identity (world XY), in which case a body needs no transform.
    pub fn is_identity(&self) -> bool {
        self.origin == [0.0; 3] && self.x == [1.0, 0.0, 0.0] && self.y == [0.0, 1.0, 0.0]
    }

    /// Carry the frame through a 3x4 transform: the origin as a point and the axes as directions. This is how
    /// a local component frame is taken into world space (`world_transform`).
    pub fn transformed(&self, m: &[f64; 12]) -> PlaneFrame {
        PlaneFrame { origin: apply12(m, self.origin), x: apply12_dir(m, self.x), y: apply12_dir(m, self.y) }
    }

    /// Move a body (mesh plus faces) built in the local axes of the frame into world space.
    pub fn place_body(&self, mut mesh: Mesh, mut faces: Vec<MeshFace>) -> (Mesh, Vec<MeshFace>) {
        if self.is_identity() {
            return (mesh, faces);
        }
        for v in &mut mesh.verts {
            *v = self.lift3(*v);
        }
        for f in &mut faces {
            f.centroid = self.lift3(f.centroid);
            f.normal = self.rotate_dir(f.normal);
        }
        (mesh, faces)
    }
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn norm3(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-12 {
        None
    } else {
        Some([v[0] / l, v[1] / l, v[2] / l])
    }
}

impl BasePlane {
    /// Frame of a base plane at the origin.
    ///
    /// XY: X = +x, Y = +y (normal +z). XZ: X = +x, Y = +z (normal -y). YZ: X = +y, Y = +z (normal +x).
    pub fn frame(self) -> PlaneFrame {
        match self {
            BasePlane::XY => PlaneFrame { origin: [0.0; 3], x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0] },
            BasePlane::XZ => PlaneFrame { origin: [0.0; 3], x: [1.0, 0.0, 0.0], y: [0.0, 0.0, 1.0] },
            BasePlane::YZ => PlaneFrame { origin: [0.0; 3], x: [0.0, 1.0, 0.0], y: [0.0, 0.0, 1.0] },
        }
    }
}

impl SketchPlane {
    /// Frame without project context, available for `World` only. `Datum` and `Face` need a `Project` to
    /// resolve the plane or the face.
    pub fn world_frame(&self) -> Option<PlaneFrame> {
        match self {
            SketchPlane::World(b) => Some(b.frame()),
            _ => None,
        }
    }
}

/// Component kind. A part holds bodies, features, sketches and datums; an assembly holds subcomponents
/// (parts and subassemblies), datums and skeleton sketches, and holds no bodies directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ComponentKind {
    #[default]
    Part,
    Assembly,
}

/// A component: a container in the build tree, either a part or an assembly.
///
/// Timeline nodes (sketches, datums, features) with `parent == Some(component.id)` belong to it, and
/// components nest inside each other through `parent`. The document root is the only component with
/// `parent == None`, and it is an assembly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Component {
    pub id: Id,
    pub name: String,
    /// Part or assembly.
    #[serde(default)]
    pub kind: ComponentKind,
    /// Parent component. `None` only for the document root.
    #[serde(default)]
    pub parent: Option<Id>,
    /// Placement inside the parent (3x4 row-major). The identity means sitting at the parent zero. Moved by
    /// hand (`set_component_transform`) or driven by mates.
    #[serde(default = "place_identity")]
    pub transform: [f64; 12],
    /// Visibility in 3D, toggled from the tree.
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Grounded (fixed in the assembly): not driven by mates. Serves as the anchor from which placements are
    /// propagated.
    #[serde(default)]
    pub grounded: bool,
}

fn place_identity() -> [f64; 12] {
    PLACE_IDENTITY
}
fn default_true() -> bool {
    true
}

// --- Mates: joints built on connectors. ---

/// Anchor of a mate connector: geometry inside a component, in its local space, that the connector frame is
/// attached to. The references are persistent (a face by its persistent id).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnchorRef {
    /// Origin of the component (local zero, world axes).
    Origin,
    /// Base plane of the component (XY, XZ, YZ).
    BasePlane(BasePlane),
    /// Centre of a face of body `body` (the frame is the face centre with its normal as Z).
    FaceCenter(Id, FaceKey),
    /// Midpoint of an edge of body `body`, by persistent id (the frame is the midpoint with the edge tangent
    /// as Z).
    EdgeMid(Id, u32),
    /// A vertex, that is, an endpoint of an edge of body `body`: the edge id plus `true` for the end and
    /// `false` for the start. A point anchor.
    Vertex(Id, u32, bool),
}

impl AnchorRef {
    /// The anchor defines a plane (a normal plus a point): a face, a base plane or the origin.
    pub fn is_plane(&self) -> bool {
        matches!(self, AnchorRef::FaceCenter(..) | AnchorRef::BasePlane(_) | AnchorRef::Origin)
    }
    /// The anchor defines an axis (a direction plus a point): an edge or a cylinder.
    pub fn is_axis(&self) -> bool {
        matches!(self, AnchorRef::EdgeMid(..))
    }
    /// Whether the anchor has a direction (a normal or an axis), as parallelism and angle require.
    pub fn has_dir(&self) -> bool {
        self.is_plane() || self.is_axis()
    }
}

/// A mate connector: a named frame on component `owner`, in its local space. A joint builds a relative
/// placement from a pair of connectors. `flip` reverses the normal (180 degrees about X), `offset_xyz` shifts
/// the origin along the frame's own axes, and `rot_deg` turns the secondary axis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MateConnector {
    pub id: Id,
    /// A name, because a connector is an element in its own right rather than a part of some mate.
    ///
    /// While it lived only inside a mate there was nothing to call it but "anchor A of that mate". A
    /// connector belongs in the tree next to a sketch: it is created in advance, reused by several mates and
    /// edited on its own, and all of that needs a name to refer to it by.
    #[serde(default)]
    pub name: String,
    /// Whether the connector is standalone, that is, created on its own rather than for one mate.
    ///
    /// The distinction serves exactly one purpose: a connector created for a mate disappears with it, or the
    /// document fills with debris nobody created, while a standalone one stays, because a second mate may be
    /// attached to it later.
    #[serde(default)]
    pub standalone: bool,
    pub owner: Id,
    pub anchor: AnchorRef,
    #[serde(default)]
    pub flip: bool,
    /// Offset of the connector along its own X, Y and Z axes, in millimetres.
    ///
    /// A single offset along the main axis leaves no way to move the anchor sideways except by moving the
    /// body itself by guesswork, and an anchor almost never coincides exactly with the centre of a face.
    ///
    /// The axes are those of the connector, not the world ones: otherwise the same number would mean
    /// different things on different bodies.
    #[serde(default)]
    pub offset_xyz: [f64; 3],
    /// Attachment point on the selected geometry: the midpoint or one of the ends.
    ///
    /// The midpoint is not a universal answer: for a hinge built on two holes of different lengths the
    /// midpoints are tens of millimetres apart and the body honestly moves by the difference. A cylinder
    /// therefore offers three attachment points — the midpoint and both ends — chosen by the user.
    #[serde(default)]
    pub point: crate::asm::connector::AttachPoint,
    /// Rotation of the secondary axis about the main one, in degrees: the reorientation control.
    ///
    /// Needed when the automatic derivation got the axis right but pointed it the wrong way. Storing quarter
    /// turns (N times 90 degrees) covers the common case and no more — a slot at 30 degrees to the body axis
    /// cannot be expressed in quarters. The "+90 degrees" button still exists and simply adds to this
    /// angle.
    #[serde(default)]
    pub rot_deg: f64,
    /// Geometry that defines the secondary axis: an optional second pick.
    ///
    /// The secondary axis is derived automatically — the long side of a face, the adjacent face of an edge.
    /// That is right on a rail, but a square face has no long side at all and the automatic answer is
    /// arbitrary. In that case the user points at an edge (or a face) and the axis runs along it.
    ///
    /// The direction is placed perpendicular to the main axis: the pick chooses a side, it does not replace
    /// the anchor. `None` keeps the automatic derivation.
    #[serde(default)]
    pub axis_ref: Option<AnchorRef>,
}

/// What exactly a constraint restricts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstraintKind {
    /// Group: a set of bodies rigidly fixed relative to each other in their current placements.
    Group,
    /// Width: a body stands midway between two walls, at equal distance from both.
    Width,
    /// Tangency: two surfaces touch. The only constraint that needs no connectors.
    Tangent,
}

impl ConstraintKind {
    /// A catalogue key rather than a word: the core is a library and has no language of its own.
    pub fn label(self) -> &'static str {
        match self {
            ConstraintKind::Group => "constraint-kind-group",
            ConstraintKind::Width => "constraint-kind-width",
            ConstraintKind::Tangent => "constraint-kind-tangent",
        }
    }
}

/// An assembly constraint: a condition that is not a joint kind between a pair of anchors.
///
/// Joint kinds (`JointKind`) describe which of the six degrees of freedom a pair of anchors keeps. Not every
/// condition reduces to a pair, though: a group fixes a set of bodies, and a width constraint takes three
/// anchors — two walls and a tab. Those live in this separate object.
///
/// One object for all such conditions rather than a structure per kind: two nearly identical structures drift
/// apart silently.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MateConstraint {
    pub id: Id,
    #[serde(default)]
    pub name: String,
    pub kind: ConstraintKind,
    /// Components, for a group. Fewer than two leaves nothing to fix.
    #[serde(default)]
    pub members: Vec<Id>,
    /// Connectors, for a width constraint: the first two are the walls and the third is the tab between them.
    #[serde(default)]
    pub anchors: Vec<Id>,
    /// Geometry directly, for tangency: two surfaces with their owners.
    ///
    /// Tangency has no connectors at all, so it stores the anchors themselves rather than references to
    /// connectors.
    #[serde(default)]
    pub faces: Vec<(Id, AnchorRef)>,
}

/// Kind of relation between mates; there are exactly four.
///
/// A relation does not place a body and does not remove a degree from a pair of anchors: it ties two already
/// existing degrees of freedom together by a constant factor. Hence a separate object — this is expressible
/// neither as a joint nor as a `MateConstraint`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationKind {
    /// Gear: a constant ratio between the angles of two rotations.
    Gear,
    /// Rack and pinion: the angle of one mate against the travel of another.
    RackPinion,
    /// Screw: angle against travel within one and the same mate (a thread pitch). It needs no second mate.
    Screw,
    /// Linear: a constant ratio between the travels of two sliders.
    Linear,
}

impl RelationKind {
    /// A catalogue key rather than a word: the core is a library and has no language of its own.
    pub fn label(self) -> &'static str {
        match self {
            RelationKind::Gear => "relation-kind-gear",
            RelationKind::RackPinion => "relation-kind-rack-pinion",
            RelationKind::Screw => "relation-kind-screw",
            RelationKind::Linear => "relation-kind-linear",
        }
    }

    /// Which degrees it ties together: `(first is a rotation, second is a rotation)`.
    ///
    /// This is the definition of the kind rather than a hint for the interface: the user's selection is
    /// validated against it and the bridge computes the coefficient from it. For a rack the order is fixed —
    /// the rotation is always first and the travel second, or "travel per revolution" would have to be
    /// divided in one case and multiplied in the other.
    pub fn slots_are_rotations(self) -> (bool, bool) {
        match self {
            RelationKind::Gear => (true, true),
            RelationKind::RackPinion | RelationKind::Screw => (true, false),
            RelationKind::Linear => (false, false),
        }
    }

    /// Whether two different mates are required. A screw does not need them: it lives inside one cylindrical
    /// mate.
    pub fn needs_two_mates(self) -> bool {
        !matches!(self, RelationKind::Screw)
    }

    /// What the number means: a dimensionless ratio for gear and linear relations, and travel per revolution
    /// in millimetres for rack and screw.
    pub fn value_is_per_turn(self) -> bool {
        matches!(self, RelationKind::RackPinion | RelationKind::Screw)
    }
}

/// A relation between two degrees of freedom.
///
/// What is stored is not "a mate of mates" but a (mate, slot) pair on each side: a degree of freedom is
/// addressed the same way the specified value, the reading and the limit address it — by slot number.
/// Otherwise the relation would hold one degree while the mate field displayed another.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MateRelation {
    pub id: Id,
    #[serde(default)]
    pub name: String,
    pub kind: RelationKind,
    /// First mate and its slot. For kinds with mixed degrees this is the rotation.
    pub a: Id,
    #[serde(default)]
    pub slot_a: usize,
    /// Second mate and its slot. For a screw this is the same mate with a different slot.
    pub b: Id,
    #[serde(default)]
    pub slot_b: usize,
    /// The relation number: how much faster the second degree runs than the first (gear, linear), or travel
    /// per revolution in millimetres (rack, screw). The kind decides which — see `value_is_per_turn`.
    pub value: f64,
    /// Reverse direction. It changes the sign of the number, not its meaning.
    #[serde(default)]
    pub reversed: bool,
    /// Phase offset captured at creation time, in radians or millimetres, matching the degrees themselves.
    ///
    /// A relation ties motion together, not absolute readings: gears standing in arbitrary positions must not
    /// jump when the relation is created; it only forbids them to diverge from then on. Holding
    /// `second = k * first` without a phase would turn a body by an arbitrary angle the moment the relation
    /// appears, which nobody asked for. So the phase is captured once and stored.
    #[serde(default)]
    pub phase: f64,
}

/// What kind of entry a mate list row is (see `Project::mate_timeline`).
///
/// The three kinds of object — a mate between a pair of anchors, a condition over a set of bodies, and a
/// relation between degrees — are one and the same thing to the user: the rules that hold the assembly
/// together. They are distinguished to know what a click should do, not to be shown as three separate
/// lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MateItem {
    /// A mate between a pair of anchors (`Joint`).
    Joint,
    /// A condition that does not reduce to a pair: group, width, tangency (`MateConstraint`).
    Constraint,
    /// A relation between two degrees of freedom (`MateRelation`).
    Relation,
}

/// State of a mate list entry: what the user sees as a colour and a word.
///
/// "Faulty" and "violated" are different failures and must not be conflated. A faulty entry has nothing to
/// hold with (a lost anchor, a slot of the wrong kind); a violated one is sound but the solver could not
/// satisfy it, usually because another entry contradicts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MateState {
    Ok,
    /// Catalogue key naming the reason; the application supplies the words.
    Faulty(&'static str),
    Violated,
}

/// A row of the mate list: everything needed to draw it and to see what it holds.
#[derive(Clone, Debug)]
pub struct MateEntry {
    pub item: MateItem,
    pub id: Id,
    pub name: String,
    /// Catalogue key of the kind (`JointKind::label`, `ConstraintKind::label`, `RelationKind::label`).
    pub kind_label: &'static str,
    pub state: MateState,
    /// Components the entry touches; highlighting in the tree and in the viewport uses the same list.
    pub touches: Vec<Id>,
}

/// Joint kind and the degrees of freedom it leaves free, in connector space: R is a rotation, T is a
/// translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JointKind {
    /// Rigid (fastened): zero degrees of freedom, the frames coincide.
    Rigid,
    /// Revolute: 1R about the connector Z axis.
    Revolute,
    /// Slider: 1T along the connector Z axis.
    Slider,
    /// Cylindrical: 1R plus 1T, about and along Z.
    ///
    /// `Concentric` is an earlier name for the same thing: coaxial alignment is a cylindrical mate. The alias
    /// exists only so that already saved projects still open; there is no separate kind and no separate
    /// solving path for it.
    #[serde(alias = "Concentric")]
    Cylindrical,
    /// Planar: 2T in XY plus 1R about Z.
    ///
    /// `Coincident` and `Angle` are earlier names: mating two faces is a planar mate.
    #[serde(alias = "Coincident", alias = "Angle")]
    Planar,
    /// Ball: 3R about a point.
    Ball,
    /// Pin-slot: 1R about Z plus 1T along X.
    PinSlot,
    /// Parallel: holds direction only, leaving four degrees of freedom (3T plus 1R about Z).
    ///
    /// A condition rather than a fit: the anchor axes point the same way, while where the body stands and how
    /// it is rotated about that axis is its own business. Shelves parallel to a base, rails parallel to each
    /// other.
    ///
    /// This used to be an alias of the planar mate, justified by "parallelism and angle constrain directions
    /// only and remove the same number of degrees". That is wrong: a planar mate leaves three degrees of
    /// freedom and a parallel one leaves four, because planar also holds the distance along the normal, which
    /// parallelism never promises. The alias is gone: it turned one mate into another silently.
    Parallel,
}

impl JointKind {
    /// Number of degrees of freedom the pair keeps.
    pub fn dof(self) -> u8 {
        match self {
            JointKind::Rigid => 0,
            JointKind::Revolute | JointKind::Slider => 1,
            JointKind::Cylindrical | JointKind::PinSlot => 2,
            JointKind::Planar | JointKind::Ball => 3,
            JointKind::Parallel => 4,
        }
    }


    /// Which slots — angle, offset, offset2 — are free degrees of freedom that the solver may vary to close a
    /// loop. Matches `motion`. The offset of a rigid mate is a user-specified gap rather than a degree of
    /// freedom, so it is not varied.
    pub fn free_slots(self) -> [bool; 3] {
        match self {
            JointKind::Rigid => [false, false, false],
            JointKind::Revolute => [true, false, false],
            JointKind::Slider => [false, true, false],
            JointKind::Cylindrical | JointKind::PinSlot => [true, true, false],
            JointKind::Planar | JointKind::Ball => [true, true, true],
            // A parallel mate has no value: it holds a direction, not a distance or an angle.
            JointKind::Parallel => [false, false, false],
        }
    }

    /// A catalogue key rather than a word: the core is a library and has no language of its own. The
    /// application supplies the words.
    pub fn label(self) -> &'static str {
        match self {
            JointKind::Rigid => "joint-kind-rigid",
            JointKind::Revolute => "joint-kind-revolute",
            JointKind::Slider => "joint-kind-slider",
            JointKind::Cylindrical => "joint-kind-cylindrical",
            JointKind::Planar => "joint-kind-planar",
            JointKind::Ball => "joint-kind-ball",
            JointKind::PinSlot => "joint-kind-pin-slot",
            JointKind::Parallel => "joint-kind-parallel",
        }
    }

    /// Motion matrix of a joint in connector space, parameterised by the free values and applied between
    /// frames A and B. `angle`, in degrees, turns about Z (revolute, cylindrical, planar, pin-slot, ball);
    /// `off` and `off2` are translations or extra angles depending on the kind.
    pub fn motion(self, angle: f64, off: f64, off2: f64) -> [f64; 12] {
        let rz = |deg: f64| {
            let (s, c) = deg.to_radians().sin_cos();
            [c, -s, 0.0, 0.0, s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]
        };
        let rx = |deg: f64| {
            let (s, c) = deg.to_radians().sin_cos();
            [1.0, 0.0, 0.0, 0.0, 0.0, c, -s, 0.0, 0.0, s, c, 0.0]
        };
        let ry = |deg: f64| {
            let (s, c) = deg.to_radians().sin_cos();
            [c, 0.0, s, 0.0, 0.0, 1.0, 0.0, 0.0, -s, 0.0, c, 0.0]
        };
        let tr = |x: f64, y: f64, z: f64| [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z];
        match self {
            // Rigid: zero degrees of freedom, but `off` is a user-specified gap along the connector normal
            // (faces N apart; flush is 0). Returning the identity here ignores that offset.
            JointKind::Rigid => tr(0.0, 0.0, off),
            JointKind::Revolute => rz(angle),
            JointKind::Slider => tr(0.0, 0.0, off),
            JointKind::Cylindrical => mat_mul12(&rz(angle), &tr(0.0, 0.0, off)),
            JointKind::Planar => mat_mul12(&tr(off, off2, 0.0), &rz(angle)),
            JointKind::Ball => mat_mul12(&rz(angle), &mat_mul12(&rx(off), &ry(off2))),
            JointKind::PinSlot => mat_mul12(&rz(angle), &tr(off, 0.0, 0.0)),
            // A parallel mate has no values: there is nothing to hold and nothing to move.
            JointKind::Parallel => [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        }
    }
}

/// A mate between connector `a` and connector `b`, joining their owning components. The free parameters
/// depend on the kind: `angle` about Z, `offset` and `offset2` as translations or extra angles. The
/// parameters are plain numbers; expressions sit on top of them, as `feat_dims` does.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Joint {
    pub id: Id,
    pub name: String,
    pub a: Id,
    pub b: Id,
    pub kind: JointKind,
    /// Readings of the degrees of freedom — angle in degrees, offset in millimetres, second offset — that is,
    /// where the body ended up. Written by the solver after every solve and never affecting the solve.
    ///
    /// Using these same fields as the specified values, with "non-zero means specified", makes the measured
    /// value read as a requirement on the next solve, so a free degree stops being free and a body slid along
    /// the axis of a cylindrical mate springs back. The specified value now lives separately, in `drive`, and
    /// zero is finally distinguishable from unset.
    #[serde(default)]
    pub angle: f64,
    #[serde(default)]
    pub offset: f64,
    #[serde(default)]
    pub offset2: f64,
    /// Specified values by slot — angle, offset, second offset. `None` leaves the degree free, so the body
    /// stays where it was put; `Some(v)` means that value was requested and the solver holds it. A mate does
    /// not invent a value until one is given.
    #[serde(default)]
    pub drive: [Option<f64>; 3],
    /// Contact side of a rigid face-to-face mate. `false` puts the faces coplanar with the smallest turn, so
    /// the body is not flipped; `true` picks the opposite side, turning by 180 degrees to bring the faces face
    /// to face. Not used for axes or points.
    #[serde(default)]
    pub flip: bool,
    /// Second half of the mating side: the secondary axis of the first anchor reversed (180 degrees about the
    /// main one).
    ///
    /// Aligning two coordinate systems is ambiguous in two ways, not one, and together they give four equally
    /// valid placements: the main axis may point either way, and independently of that so may the secondary
    /// one. With the side stored as a single bit, the second half was left to geometry: for a slider on a flat
    /// face the main axis is the travel direction (the same on neighbouring faces) while the secondary axis is
    /// the face normal, which points the other way on the opposite face. That is why replacing a face turned
    /// the body by 180 degrees about the travel axis while the main-axis bit knew nothing about it.
    ///
    /// The sign of a secondary axis derived from geometry is arbitrary in any case: the principal direction of
    /// a face is a line, not an arrow, and its sign is fixed by a convention inside the computation. Choosing
    /// it as the nearest to the current placement is therefore the only honest approach, not a concession.
    #[serde(default)]
    pub roll_flip: bool,
    /// Whether the mating side has already been decided.
    ///
    /// The side is chosen as the nearest to the current placement, but only once. Without this flag the choice
    /// is recomputed on every solve while the mate itself changes that placement, so the answer depends on its
    /// own consequence. Measured on a machine document: a slider with 150 mm of travel gave -310, -310, -10
    /// over three consecutive solves — the body swung by 300 mm because the side flipped false, true, false.
    /// Once decided, it lives in `flip` and changes only by explicit request.
    #[serde(default)]
    pub flip_decided: bool,
    /// A mate of a nested subassembly promoted to the root for global control — a slider, for example, that
    /// is driven from the root while the parts of the subassembly move. When set, the joint is visible and
    /// editable in the root context, not only in its home (the lowest common ancestor of the connector
    /// owners).
    #[serde(default)]
    pub global: bool,
    /// Hold as built: the placement of the second anchor in the frame of the first, captured at the moment
    /// the current arrangement was declared correct.
    ///
    /// `None` is an ordinary mate: it aligns the anchors, which is the right behaviour when a body is being
    /// placed by the mate. An assembly arranged by hand or arriving through an import needs no alignment,
    /// though — the bodies already stand correctly and the mate is only there to keep them from drifting
    /// apart. Otherwise the very first mate collapses an imported assembly into a point and the offsets have
    /// to be dialled in by hand.
    ///
    /// What is stored is the relative placement of the anchors, not a world one: the assembly is later moved
    /// as a whole, and a remembered world placement would silently disagree with it.
    #[serde(default)]
    pub as_built: Option<[f64; 12]>,
    /// Limits per slot — angle, offset, offset2 — where `None` means unbounded. Free slots are clamped to
    /// [min, max] both while dragging the gizmo and inside `solve_joints`.
    #[serde(default)]
    pub limit_min: [Option<f64>; 3],
    #[serde(default)]
    pub limit_max: [Option<f64>; 3],
}

impl Joint {
    /// The mating side recorded on the joint, as one value.
    ///
    /// The two halves are read together in every place that uses them, and carrying them apart invites the
    /// swap: `flip` and `roll_flip` are both bare booleans, and nothing but the order of arguments said which
    /// was which.
    pub fn side(&self) -> Side {
        Side { flip: self.flip, roll_flip: self.roll_flip }
    }

    /// Clamp the value of a slot (0 angle, 1 offset, 2 offset2) to its limits, where they are set.
    pub fn clamp_slot(&self, slot: usize, v: f64) -> f64 {
        let mut v = v;
        if let Some(lo) = self.limit_min[slot] {
            if v < lo {
                v = lo;
            }
        }
        if let Some(hi) = self.limit_max[slot] {
            if v > hi {
                v = hi;
            }
        }
        v
    }

    /// Apply the limits to every free slot of the joint, clamping angle, offset and offset2 in place.
    pub fn clamp_free(&mut self) {
        let free = self.kind.free_slots();
        for slot in 0..3 {
            if !free[slot] {
                continue;
            }
            // What is clamped is the specified value: the reading is a fact, and correcting it by a limit
            // would misreport where the body stands.
            if let Some(v) = self.drive[slot] {
                self.drive[slot] = Some(self.clamp_slot(slot, v));
            }
        }
    }

    /// The specified value of a slot, when it applies to this joint kind at all.
    ///
    /// Applying is not the same as the degree being free. A rigid mate has no free degrees whatsoever, yet the
    /// gap between the faces can be specified and must be honoured: it is a parameter of the mate rather than
    /// a freedom it leaves. Testing "the slot is free" trips over exactly that and silently stops applying the
    /// gap.
    pub fn driven(&self, slot: usize) -> Option<f64> {
        // A rigid mate applies both of its parameters: the rotation about the joint axis (slot 0) and the gap (slot 1).
        let applies = self.kind.free_slots().get(slot).copied().unwrap_or(false) || (matches!(self.kind, JointKind::Rigid) && slot <= 1);
        applies.then(|| self.drive.get(slot).copied().flatten()).flatten()
    }
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
    Fillet { src: Id, radius: f64, edges: crate::refs::Ref, #[serde(default)] at_vertices: Vec<(crate::refs::Ref, f64)>, body: Id },
    /// Patch: a surface spanned over a chain of edges of body `src`. The edges come as a query and the source
    /// is not consumed.
    ///
    /// `tangent` makes the patch meet the edges smoothly, tangent to the adjacent faces, rather than merely
    /// coinciding in position: otherwise the seam is visible and can be felt.
    Patch { src: Id, edges: crate::refs::Ref, #[serde(default)] tangent: bool, body: Id },
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
    /// Copy faces into a separate surface: faces of body `src`, selected by a query, become an independent
    /// sheet `body`. The source is not consumed — a copy is a copy, so the body stays where it is while the
    /// surface lives its own life and returns to it through a face replacement.
    FaceCopy { src: Id, faces: crate::refs::Ref, body: Id },
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
    Shell { src: Id, thickness: f64, faces: crate::refs::Ref, #[serde(default)] side: ShellSide, body: Id },
    /// Draft: tilt the faces named by `faces` on body `src` by `angle` degrees relative to the neutral face
    /// `neutral`, whose line of intersection with them stays fixed. The pull direction is the normal of the
    /// neutral face, reversed by `flip`. Used for cast and stamped draft angles.
    ///
    /// Both references are queries: "every wall of this feature" is as expressible as a hand-picked set.
    Draft { src: Id, faces: crate::refs::Ref, neutral: crate::refs::Ref, angle: f64, #[serde(default)] flip: bool, body: Id },
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
    Thicken { src: Id, face: u32, thickness: f64, #[serde(default)] join: Id, body: Id },
    /// Split faces by a plane without cutting the body: one body, more faces. The plane is given by a
    /// reference (a datum, or the world plane in `plane`) plus a parametric `offset`, as for a body split.
    SplitFace { src: Id, plane: u8, datum: Id, offset: f64, body: Id },
    /// Part instance (a copy inside a component pattern): the body is a one-to-one copy of the active body of
    /// `src_comp`, and the placement comes from the transform of the copied component itself.
    ///
    /// The same approach as the mirrored part: the shape is associative, so editing the source moves every
    /// copy, while the position is a matter for the assembly. The cross-component reference is legal by
    /// construction (an isolation exception, as for `MirrorPart`): `src_comp` is resolved dynamically through
    /// `active_body`.
    PartInstance { src_comp: Id, body: Id },
    /// Mirror body `src` about a plane: `plane` selects a world plane (0 XY, 1 XZ, 2 YZ), or a non-zero
    /// `datum` selects an arbitrary datum plane. `keep` unites the result with the original.
    Mirror { src: Id, plane: u8, keep: bool, #[serde(default)] datum: Id, body: Id },
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
    SplitBody { src: Id, plane: u8, datum: Id, offset: f64, bodies: Vec<Id> },
    /// Offset a face: planar face `face` of body `src` is moved by `dist` along its own normal, producing
    /// `body`. Parametric like everything else in the timeline — `dist` is edited and recomputed, and the face
    /// reference is a query resolved by recipe rather than matched by similarity.
    PushFace { src: Id, face: crate::refs::Ref, dist: f64, body: Id },
    /// Rigid translation and rotation of body `src` (3x4 row-major), producing `body`. It moves the B-rep
    /// rather than the mesh, so the body stays parametric, and it replaces `src` in the chain as a modifier.
    Move { src: Id, mat: [f64; 12], body: Id },
    /// Parametric body-to-body boolean: `op` applied to the B-reps of body `a` (the base) and body `b` (the
    /// tool), producing `body`. `op` is 0 for a cut (a minus b), 1 for a union and 2 for an intersection. Both
    /// input bodies are consumed and hidden.
    BodyBoolean { a: Id, b: Id, op: u8, body: Id },
    /// An imported external B-rep solid (STEP) as the base body of a part: `body` is its id, `source` is the
    /// id of the embedded original file (`Project::sources`) and `solid` is the index of the solid within that
    /// file.
    ///
    /// There is no build recipe — the shape is restored by re-importing the source, and regenerate only
    /// re-tessellates the already loaded shape. The result is a full part: sketches, chamfers, fillets and
    /// booleans can be built on top of it as on any base body.
    Import { body: Id, source: Id, solid: u32 },
}

/// Identity rigid transform (3x4 row-major): the body is built in place.
pub const PLACE_IDENTITY: [f64; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// A 90 degree rotation about X, taking the local Y axis (the axis of revolution) onto world Z, so that
/// bodies of revolution such as a cone or a torus stand along Z, as a cylinder does.
pub const ROT_Y_TO_Z: [f64; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Composition of two affine 3x4 row-major matrices: the result applies `b` first and then `a`, that is
/// `a * b`. The linear part (columns 0 to 2) is the product of the rotations and column 3 is
/// `a * b_translation + a_translation`, which is how `world(child) = mat_mul12(world(parent),
/// child.transform)` works.
pub fn mat_mul12(a: &[f64; 12], b: &[f64; 12]) -> [f64; 12] {
    let mut c = [0.0; 12];
    for r in 0..3 {
        for col in 0..3 {
            c[r * 4 + col] = a[r * 4] * b[col] + a[r * 4 + 1] * b[4 + col] + a[r * 4 + 2] * b[8 + col];
        }
        c[r * 4 + 3] = a[r * 4] * b[3] + a[r * 4 + 1] * b[7] + a[r * 4 + 2] * b[11] + a[r * 4 + 3];
    }
    c
}

/// Apply a 3x4 transform to a point, including the translation.
pub fn apply12(m: &[f64; 12], p: [f64; 3]) -> [f64; 3] {
    [
        m[0] * p[0] + m[1] * p[1] + m[2] * p[2] + m[3],
        m[4] * p[0] + m[5] * p[1] + m[6] * p[2] + m[7],
        m[8] * p[0] + m[9] * p[1] + m[10] * p[2] + m[11],
    ]
}

/// Apply the rotational part of a 3x4 transform to a direction, without the translation.
pub fn apply12_dir(m: &[f64; 12], d: [f64; 3]) -> [f64; 3] {
    [
        m[0] * d[0] + m[1] * d[1] + m[2] * d[2],
        m[4] * d[0] + m[5] * d[1] + m[6] * d[2],
        m[8] * d[0] + m[9] * d[1] + m[10] * d[2],
    ]
}

/// Whether a 3x4 transform is the identity, in which case a body needs no transform into world space.
pub fn is_identity12(m: &[f64; 12]) -> bool {
    *m == PLACE_IDENTITY
}

/// Extent of an extrude or a cut along the sketch plane normal, as `(start, total)`: the profile is swept by
/// `total` (a non-negative length) from `start` along the normal. One rule shared by `Extrude` and `Combine`:
/// - [`Reach::BothWays`] gives [-h/2, +h/2], and the second side is not consulted: half the height each way
///   is already both sides, which is why the old pair of booleans had a state that meant nothing;
/// - two-sided (`down > 0`) gives [-down, +h], and [`Reach::Backward`] swaps up and down;
/// - one-sided gives [0, +h] forwards and [-h, 0] backwards.
pub fn extrude_extent(height: f64, down: f64, reach: Reach) -> (f64, f64) {
    let (h, d) = (height.abs(), down.abs());
    match reach {
        Reach::BothWays => (-h / 2.0, h),
        Reach::Backward if d > 1e-9 => (-h, h + d),
        Reach::Forward if d > 1e-9 => (-d, h + d),
        Reach::Backward => (-h, h),
        Reach::Forward => (0.0, h),
    }
}

/// Inverse of an affine 3x4 transform: the inverse of the 3x3 linear part plus a translation of -inv * t.
///
/// Needed to convert a world placement into the local space of the active context, which is what draws a part
/// at its own zero. A degenerate matrix (determinant near zero) yields the identity as a guard.
/// Composition of rigid 3x4 transforms: (a after b)(x) = a(b(x)).
pub fn compose12(a: &[f64; 12], b: &[f64; 12]) -> [f64; 12] {
    let mut r = [0.0; 12];
    for i in 0..3 {
        for j in 0..3 {
            r[i * 4 + j] = a[i * 4] * b[j] + a[i * 4 + 1] * b[4 + j] + a[i * 4 + 2] * b[8 + j];
        }
        r[i * 4 + 3] = a[i * 4] * b[3] + a[i * 4 + 1] * b[7] + a[i * 4 + 2] * b[11] + a[i * 4 + 3];
    }
    r
}

/// A 3x4 rotation about an axis (point `o`, direction `d`, angle `deg`, right-handed). Used to pre-rotate a
/// revolve for the symmetric and flipped cases without touching the kernel.
pub fn rot12_axis(o: [f64; 3], d: [f64; 3], deg: f64) -> [f64; 12] {
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-12);
    let (x, y, z) = (d[0] / l, d[1] / l, d[2] / l);
    let (s, c) = deg.to_radians().sin_cos();
    let ic = 1.0 - c;
    let m = [
        c + x * x * ic, x * y * ic - z * s, x * z * ic + y * s,
        y * x * ic + z * s, c + y * y * ic, y * z * ic - x * s,
        z * x * ic - y * s, z * y * ic + x * s, c + z * z * ic,
    ];
    // Translation: o - R * o.
    let ro = [
        m[0] * o[0] + m[1] * o[1] + m[2] * o[2],
        m[3] * o[0] + m[4] * o[1] + m[5] * o[2],
        m[6] * o[0] + m[7] * o[1] + m[8] * o[2],
    ];
    [m[0], m[1], m[2], o[0] - ro[0], m[3], m[4], m[5], o[1] - ro[1], m[6], m[7], m[8], o[2] - ro[2]]
}

pub fn mat_inv12(m: &[f64; 12]) -> [f64; 12] {
    let (a0, a1, a2) = (m[0], m[1], m[2]);
    let (a3, a4, a5) = (m[4], m[5], m[6]);
    let (a6, a7, a8) = (m[8], m[9], m[10]);
    let det = a0 * (a4 * a8 - a5 * a7) - a1 * (a3 * a8 - a5 * a6) + a2 * (a3 * a7 - a4 * a6);
    if det.abs() < 1e-12 {
        return PLACE_IDENTITY;
    }
    let id = 1.0 / det;
    // inv(3×3) = adj/det
    let i = [
        (a4 * a8 - a5 * a7) * id, (a2 * a7 - a1 * a8) * id, (a1 * a5 - a2 * a4) * id,
        (a5 * a6 - a3 * a8) * id, (a0 * a8 - a2 * a6) * id, (a2 * a3 - a0 * a5) * id,
        (a3 * a7 - a4 * a6) * id, (a1 * a6 - a0 * a7) * id, (a0 * a4 - a1 * a3) * id,
    ];
    let (tx, ty, tz) = (m[3], m[7], m[11]);
    [
        i[0], i[1], i[2], -(i[0] * tx + i[1] * ty + i[2] * tz),
        i[3], i[4], i[5], -(i[3] * tx + i[4] * ty + i[5] * tz),
        i[6], i[7], i[8], -(i[6] * tx + i[7] * ty + i[8] * tz),
    ]
}

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
    /// the names for the faces the groove produces
    pub gnames: &'a [u32],
    /// the names for the faces the run-out produces
    pub rnames: &'a [u32],
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
pub trait Kernel {
    fn extrude(&self, body: Id, profile: &[f64], height: f64, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    fn revolve(&self, body: Id, profile: &[f64], axis: u8, angle_deg: f64, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    fn boolean(&self, body: Id, base: &[f64], base_h: f64, tool: &[f64], tool_h: f64, op: u8, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Extrude a region from the exact profile `profile` (the `geom::encode_profile` encoding: an outer
    /// contour plus holes made of real edges — lines, arcs, circles) to a height, giving a body with exact
    /// faces.
    fn extrude_region(&self, body: Id, profile: &[f64], height: f64, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Extrude a profile and combine it with the already built body `src` (`op`: 0 cut, 1 union, 2
    /// intersection).
    fn combine(&self, body: Id, src: Id, profile: &[f64], height: f64, op: u8, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Like `combine`, but the tool is a region from the exact profile `profile` (outer contour plus holes).
    fn combine_region(&self, body: Id, src: Id, profile: &[f64], height: f64, op: u8, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Extrude several profiles (all from one sketch and plane `place`, to `height`), merge them into one
    /// tool and apply a single boolean against body `src`, giving one body. A zero `src` produces a new body
    /// from the tool itself and ignores `op`. This replaces chains of extrude plus boolean, keeping one
    /// operation in one node.
    ///
    /// `caps` holds the name descriptors of the start and end caps from the document name table: a cap is not
    /// produced by a profile edge, so its name arrives as a separate parameter rather than inside the
    /// encoding.
    fn combine_region_multi(&self, body: Id, src: Id, profiles: &[Vec<f64>], height: f64, op: u8, place: [f64; 12], caps: &[u32]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Revolve the exact profile `profile` about an axis (0 for X, 1 for Y) by an angle, giving a body with
    /// exact faces.
    fn revolve_region(&self, body: Id, profile: &[f64], axis: u8, angle_deg: f64, place: [f64; 12], caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Revolve a region about an arbitrary axis (`origin` and `dir` in profile local space), that is, a datum
    /// axis. The default implementation is a fallback for the mock: an ordinary revolve about X.
    fn revolve_region_axis(&self, body: Id, profile: &[f64], _origin: [f64; 3], _dir: [f64; 3], angle_deg: f64, place: [f64; 12], caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        self.revolve_region(body, profile, 0, angle_deg, place, caps)
    }

    /// Revolve several profiles, merge them into one tool and apply a single boolean against body `src`,
    /// giving one body and one timeline node. A zero `src` produces a new body and ignores `op`. `origin_dir`
    /// is an arbitrary axis in profile local space (a datum or a centreline); `None` uses the X or Y axis
    /// from `axis`.
    ///
    /// The default implementation is a fallback for the mock and for builds without OCCT: the profiles are
    /// revolved one at a time and added together with booleans. The shape comes out the same; what is missing
    /// is merging touching contours into one face.
    fn revolve_region_multi(
        &self,
        body: Id,
        src: Id,
        profiles: &[Vec<f64>],
        axis: u8,
        origin_dir: Option<([f64; 3], [f64; 3])>,
        angle_deg: f64,
        place: [f64; 12],
        op: u8,
        caps: &[u32],
    ) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        // The fallback is honest: a single profile without a boolean is reproduced exactly, arbitrary axis
        // included. Merging several profiles or combining with a body needs a real kernel, and silently
        // returning a boss instead of a cut would give a test a green light on the wrong shape.
        if src != 0 || profiles.len() > 1 {
            return Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::RevolveProfile));
        }
        let _ = op;
        let first = profiles.first().ok_or(crate::errors::CoreError::NoContours)?;
        let cap2 = [caps.get(1).copied().unwrap_or(0), caps.get(2).copied().unwrap_or(0)];
        match origin_dir {
            Some((o, d)) => self.revolve_region_axis(body, first, o, d, angle_deg, place, cap2),
            None => self.revolve_region(body, first, axis, angle_deg, place, cap2),
        }
    }

    /// Sweep several profiles along one path, merge them and apply a single boolean against body `src`. See
    /// [`Kernel::revolve_region_multi`].
    fn sweep_multi(
        &self,
        body: Id,
        src: Id,
        profiles: &[Vec<f64>],
        profile_place: [f64; 12],
        path: &[f64],
        path_place: [f64; 12],
        op: u8,
        caps: &[u32],
    ) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        if src != 0 || profiles.len() > 1 {
            return Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::Sweep));
        }
        let _ = op;
        let first = profiles.first().ok_or(crate::errors::CoreError::NoContours)?;
        let cap2 = [caps.get(1).copied().unwrap_or(0), caps.get(2).copied().unwrap_or(0)];
        self.sweep(body, first, profile_place, path, path_place, cap2)
    }
    /// Sweep: the exact profile `profile` (`encode_profile`), placed by `profile_place`, is swept along the
    /// path `path` (`[1.0, loop_block]`, one open or closed contour) placed by `path_place`, giving a body
    /// with exact faces. The default implementation is a fallback, since there is no sweep without OCCT.
    #[allow(clippy::too_many_arguments)]
    fn sweep(&self, _body: Id, _profile: &[f64], _profile_place: [f64; 12], _path: &[f64], _path_place: [f64; 12], _caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::Sweep))
    }
    /// Loft through sections. `sections` is the concatenation of loop blocks (each `[nedges, nedges*8]`),
    /// `offsets` marks where section i starts within `sections` (length nsec + 1), and `places` holds the 3x4
    /// placement per section (length nsec * 12). `ruled` gives straight faces and `solid` closes the result
    /// into a body. The default implementation is a fallback.
    #[allow(clippy::too_many_arguments)]
    fn loft(&self, _body: Id, _sections: &[f64], _offsets: &[usize], _places: &[f64], _walls: LoftWalls, _kind: LoftBody, _caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::Loft))
    }
    /// Lofted boolean: the lofted solid acts as a tool and is combined with body `src` (`op`: 0 cut, 1 union,
    /// 2 intersection). The section parameters are as in `loft`. The default implementation is a fallback.
    #[allow(clippy::too_many_arguments)]
    fn loft_combine(&self, _body: Id, _src: Id, _sections: &[f64], _offsets: &[usize], _places: &[f64], _walls: LoftWalls, _op: u8, _caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::LoftBoolean))
    }
    /// Stepped hole: a cutting tool (a cylinder plus a counterbore or countersink) in frame `pl`. `kind` is 0
    /// for a plain hole, 1 for a counterbore and 2 for a countersink. The mock default cuts a plain cylinder
    /// through `combine_region`.
    #[allow(clippy::too_many_arguments)]
    fn hole(&self, body: Id, src: Id, _kind: u8, pl: [f64; 12], dia: f64, depth: f64, _dia2: f64, _depth2: f64, _bore: u32, _extra: &[u32]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        let prof = crate::geom::encode_profile(&crate::geom::circle_contour(0.0, 0.0, dia / 2.0, 0.05), &[]);
        self.combine_region(body, src, &prof, -depth.abs(), 0, pl)
    }
    /// Many holes at once (at the points of a sketch): one cutting tool per frame in `pls`, all merged and
    /// subtracted by a single boolean. `kind`, `dia`, `depth`, `dia2` and `depth2` are as in `hole`. The
    /// default implementation applies `hole` for each point in turn.
    #[allow(clippy::too_many_arguments)]
    fn holes(&self, body: Id, src: Id, kind: u8, pls: &[[f64; 12]], dia: f64, depth: f64, dia2: f64, depth2: f64, bores: &[u32], extra: &[u32]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        let mut cur = src;
        let mut out: Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> = Err(crate::errors::CoreError::NoPointsForHoles);
        for (i, pl) in pls.iter().enumerate() {
            out = self.hole(body, cur, kind, *pl, dia, depth, dia2, depth2, bores.get(i).copied().unwrap_or(0), extra);
            if out.is_err() {
                return out;
            }
            cur = body; // After the first cut, work on the accumulated body.
        }
        out
    }
    /// Helical rib or groove from an exact profile.
    ///
    /// `profile` is the axial section computed by the model core from the thread standard
    /// (`qymcad_core::thread`) and encoded as an ordinary exact profile (segments plus arcs). The kernel only
    /// sweeps it along a helix of radius `radius` about the axis (`origin`, `dir`) and either subtracts it
    /// (`fuse = false`, a thread) or unites it (`fuse = true`, an auger flight). Thread and auger are thus one
    /// operation, and the profile stays exact, so a chamfer can be applied to it later.
    fn helical(&self, _h: Helical<'_>) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::Helix))
    }
    /// Exact cylinder primitive (axis Z, base at z = 0): a native kernel solid with three faces.
    fn cylinder(&self, body: Id, r: f64, h: f64, names: [u32; 3]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Exact sphere (centred at the origin): one face.
    fn sphere(&self, body: Id, r: f64, names: [u32; 3]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Exact cone (r1 at z = 0 to r2 at z = h, axis Z).
    fn cone(&self, body: Id, r1: f64, r2: f64, h: f64, names: [u32; 3]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Exact torus (in the XY plane, axis Z).
    fn torus(&self, body: Id, major: f64, minor: f64, names: [u32; 3]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Fillet the edges of body `src` with radius `radius` (an empty `edges` means every edge).
    fn fillet(&self, body: Id, src: Id, radius: f64, edges: &[u32], names: &[u32], corners: &[u32], all_names: &[u32]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Variable fillet specified at vertices: `verts` holds a vertex point and the radius there, and the
    /// kernel interpolates along the edge itself. A shared vertex has one radius for both neighbours, so a
    /// chain meets without a step — a property of the way it is specified rather than of any check. An
    /// endpoint without an entry uses `radius`. The mock default produces a constant fillet.
    fn fillet_at_vertices(&self, body: Id, src: Id, radius: f64, edges: &[u32], _verts: &[([f64; 3], f64)]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        self.fillet(body, src, radius, edges, &[], &[], &[])
    }
    /// Copy faces into a separate sheet: the bridge from the parametric model into the surface layer. `names`
    /// holds the names of the copies, whose provenance the model knows. The default is an honest refusal:
    /// there is nothing to fake a surface with, and silently reporting success would take the timeline into a
    /// state that does not exist.
    fn copy_faces(&self, _body: Id, _src: Id, _faces: &[u32], _names: &[u32]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::OpFailed(crate::errors::Op::CopyFaces))
    }

    /// Patch: span a surface over a chain of edges. The default is a refusal.
    fn patch(&self, _body: Id, _src: Id, _edges: &[u32], _tangent: bool, _name: u32) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::OpFailed(crate::errors::Op::Patch))
    }

    /// Replace faces of a body with a surface: the node that stitches the surface layer back into the
    /// timeline. The default is a refusal.
    fn replace_faces(&self, _body: Id, _src: Id, _faces: &[u32], _surface: Id) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::OpFailed(crate::errors::Op::ReplaceFaces))
    }

    /// Trim a surface: the piece nearest to `keep` is retained. The default is a refusal.
    fn trim(&self, _body: Id, _src: Id, _tool: Id, _keep: [f64; 3]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::OpFailed(crate::errors::Op::Trim))
    }

    /// Stitch sheets into one surface. If the result closes, the output is a solid. The default is a
    /// refusal.
    fn stitch(&self, _body: Id, _parts: &[Id], _tol: f64) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::OpFailed(crate::errors::Op::Stitch))
    }

    /// Whether a body is a sheet (a surface) rather than a solid. The document has to know: a sheet has no
    /// volume, is not exported to CAM and does not count as the one body of a part. The default is `false`,
    /// since the mock works with solids.
    fn body_is_sheet(&self, _body: Id) -> bool {
        false
    }

    /// Chamfer the edges of body `src` by `dist` (an empty `edges` means every edge).
    fn chamfer(&self, body: Id, src: Id, dist: f64, edges: &[u32], names: &[u32], corners: &[u32], all_names: &[u32]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Asymmetric chamfer. `TwoDist` uses setbacks `d1` on the reference face and `d2` on the adjacent one;
    /// `DistAngle` uses setback `d1` plus angle `d2` in degrees; `flip` selects which face is the reference.
    /// `ref_face` is the persistent id of a manually chosen reference face (0 selects it automatically from
    /// `flip`). `Symmetric` falls through to a plain `chamfer(d1)`. It requires an explicit edge selection,
    /// since asymmetry is not defined for "every edge". The mock default is a symmetric `chamfer(d1)`.
    #[allow(clippy::too_many_arguments)]
    fn chamfer_ex(&self, body: Id, src: Id, d1: f64, _d2: f64, _mode: ChamferMode, _flip: bool, _ref_face: u32, edges: &[u32]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        self.chamfer(body, src, d1, edges, &[], &[], &[])
    }
    /// Shell body `src`: open the faces named by `face_ids` (persistent ids) and leave walls of `thickness`.
    /// `outward` puts the wall outside instead of inside.
    fn shell(&self, body: Id, src: Id, thickness: f64, outward: bool, face_ids: &[u32]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;

    /// Shell with named walls: `walls` holds pairs of "source face to the name of its inner wall".
    ///
    /// A wall is produced by offsetting a face rather than by copying it, so the names have to be seeded
    /// during construction: in the finished body an outer face and its wall are indistinguishable.
    fn shell_named(&self, body: Id, src: Id, thickness: f64, outward: bool, face_ids: &[u32], _walls: &[(u32, u32)]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        self.shell(body, src, thickness, outward, face_ids)
    }
    /// Centred shell: a wall of `thickness` centred on the surface. The mock default shells inwards as
    /// usual.
    fn shell_center(&self, body: Id, src: Id, thickness: f64, face_ids: &[u32]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        self.shell(body, src, thickness, false, face_ids)
    }
    /// Push-pull a face: planar face `face` of body `src` moves along its own normal by `dist` (positive adds
    /// material, negative removes it), producing body `body`. Direct modelling: a body stops being hostage to
    /// its sketch and can be edited by grabbing a face and pulling it.
    fn push_face(&self, _body: Id, _src: Id, _face: u32, _dist: f64) -> Result<(crate::geom::Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::PushFace))
    }
    /// Delete faces and heal: the faces named by `face_ids` are removed from body `src` and the neighbouring
    /// ones extended, producing body `body`. This is how a hole, a boss or a fillet is removed without taking
    /// the timeline apart.
    fn remove_faces(&self, _body: Id, _src: Id, _face_ids: &[u32]) -> Result<(crate::geom::Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::RemoveFaces))
    }
    /// Thicken a face: face `face` of body `src` becomes a plate of `thickness` as a new body `body`. The
    /// source stays alive — the plate is a separate part rather than a reworking of the original.
    fn thicken_face(&self, _body: Id, _src: Id, _face: u32, _thickness: f64, _join: Id, _fmap: &[u32], _emap: &[u32]) -> Result<(crate::geom::Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::Thicken))
    }
    /// Split faces by a plane without cutting the body: the body stays one, and the faces the plane crosses
    /// fall into pieces, producing `body`. This marks out a region rather than breaking the part apart.
    fn split_faces(&self, _body: Id, _src: Id, _origin: [f64; 3], _normal: [f64; 3]) -> Result<(crate::geom::Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::SplitFaces))
    }
    /// Split a body by a plane: body `src` is cut by the plane (`origin`, `normal`) into pieces, and each
    /// piece becomes an independent body. The pieces are ordered by position along the normal, from bottom to
    /// top; without that ordering they would swap bodies every time the plane moves.
    ///
    /// `bodies` holds pre-allocated ids, one per piece. If the number of pieces differs, an error is
    /// returned: silently losing a piece is worse than refusing.
    fn split_body(&self, _bodies: &[Id], _src: Id, _origin: [f64; 3], _normal: [f64; 3], _section: u32) -> Result<Vec<(crate::geom::Mesh, Vec<MeshFace>)>, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::SplitBody))
    }
    /// Draft: tilt the faces named by `face_ids` on body `src` by `angle` degrees relative to the neutral
    /// plane (`np_origin`, `np_normal`) along the pull direction `pull`. Requires a real kernel; the mock is a
    /// stub.
    #[allow(clippy::too_many_arguments)]
    fn draft(&self, _body: Id, _src: Id, _face_ids: &[u32], _angle: f64, _pull: [f64; 3], _np_origin: [f64; 3], _np_normal: [f64; 3], _sides: &[u32]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::Draft))
    }
    /// Pattern of body `src`: unite its copies placed by the transforms in `transforms`.
    fn pattern(&self, body: Id, src: Id, transforms: &[[f64; 12]]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;

    /// Pattern with named instances: `seeds[k]` holds pairs of "source face id to its name in copy k".
    ///
    /// Within one copy the ids are unique, so renaming by pairs is expressible here, unlike in the assembled
    /// result where every copy carries the same source number.
    fn pattern_named(&self, body: Id, src: Id, transforms: &[[f64; 12]], _seeds: &[Vec<(u32, u32)>]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        self.pattern(body, src, transforms)
    }
    /// Mirror body `src` about plane `plane`; `keep` unites the result with the original.
    fn mirror(&self, body: Id, src: Id, plane: u8, keep: bool) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;

    /// Mirror with named images: `seed` holds pairs of "source face to its name in the reflection". The copy
    /// has to receive its names before being united with the original, since afterwards both halves carry the
    /// same number.
    fn mirror_named(&self, body: Id, src: Id, plane: u8, keep: bool, _seed: &[(u32, u32)]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        self.mirror(body, src, plane, keep)
    }

    fn mirror_plane_named(&self, body: Id, src: Id, origin: [f64; 3], normal: [f64; 3], keep: bool, _seed: &[(u32, u32)]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError> {
        self.mirror_plane(body, src, origin, normal, keep)
    }
    /// Mirror about an arbitrary plane (origin and normal), that is, a datum or a face. `keep` unites the
    /// result with the original.
    fn mirror_plane(&self, body: Id, src: Id, origin: [f64; 3], normal: [f64; 3], keep: bool) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Rigid translation and rotation of the B-rep of body `src` by matrix `mat` (3x4), producing `body`.
    fn transform_body(&self, body: Id, src: Id, mat: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Re-tessellate the already built or imported shape of body `body` from the kernel cache (an external
    /// STEP solid has no recipe). `None` means the kernel does not hold that body.
    fn tessellate(&self, _body: Id) -> Option<(Mesh, Vec<MeshFace>)> {
        None
    }
    /// Body-to-body boolean: `op` applied to the B-reps of bodies `a` and `b`, producing `body`. `op` is 0 for
    /// a cut (a minus b), 1 for a union and 2 for an intersection. Both operands have to be built.
    fn body_boolean(&self, body: Id, a: Id, b: Id, op: u8) -> Result<(Mesh, Vec<MeshFace>), crate::errors::CoreError>;
    /// Edges of the built body `body`: a persistent id plus the midpoint and tangent, in body local space.
    /// Used to resolve axis connectors by id. The default is empty, since a mock supplies no edges.
    fn face_splits(&self, _body: Id) -> Vec<(u32, u32, u32)> {
        Vec::new()
    }

    /// Clear the split records once names have been handed out from them: the record is single-use.
    fn clear_face_splits(&self, _body: Id) {}

    /// Absorbed names: pairs of "former face name to the name of the merged face". Merging coplanar faces
    /// collapses two named faces into one and one name yields; the pair tells the model which name it yielded
    /// to, so a reference to the former name finds the merged face instead of getting lost. The default is
    /// empty.
    fn absorbed_names(&self, _body: Id) -> Vec<(u32, u32)> {
        Vec::new()
    }

    /// What distinguishes two edges sharing one pair of faces: `(edge, name1, name2)`, the lowest names of the
    /// faces meeting at its endpoints. The ordinal within a pair has to follow the recipe rather than the
    /// traversal order.
    fn edge_end_faces(&self, _body: Id) -> Vec<(u32, u32, u32)> {
        Vec::new()
    }

    /// Rewrite the face names of a body (pairs of old to new).
    fn rename_faces(&self, _body: Id, _pairs: &[(u32, u32)]) {}

    /// Edge names: pairs of "edge to its two faces", by face name. Empty when the kernel does not supply
    /// them.
    fn edge_face_pairs(&self, _body: Id) -> Vec<(u32, u32, u32)> {
        Vec::new()
    }

    /// Rewrite the edge names of a body (pairs of old to new).
    fn rename_edges(&self, _body: Id, _pairs: &[(u32, u32)]) {}

    /// Edge geometry of a body: for each edge, its persistent name, a polyline in body local space and, for a
    /// circular edge, its exact centre, axis and radius. Needed by projection into a sketch: a circle has to
    /// project as a circle rather than as a tessellated polyline.
    #[allow(clippy::type_complexity)]
    fn body_edge_geometry(&self, _body: Id) -> Vec<(u32, Vec<[f64; 3]>, Option<([f64; 3], [f64; 3], f64)>)> {
        Vec::new()
    }

    fn edges(&self, _body: Id) -> Vec<crate::geom::MeshEdge> {
        Vec::new()
    }
    /// Axis of a cylindrical or conical face `face_id` of a body (origin and direction in body local space),
    /// used by a datum axis bound to a face. The default is `None`, since a mock supplies no face axis.
    fn face_axis(&self, _body: Id, _face_id: u32) -> Option<([f64; 3], [f64; 3])> {
        None
    }
}

/// Result of a `regenerate` pass: which bodies were rebuilt, together with their faces (the application puts
/// those into its own face cache), and the per-node errors, whose nodes stay dirty.
#[derive(Default, Debug)]
pub struct RegenReport {
    /// (body id, faces) for the bodies rebuilt in this pass.
    pub built: Vec<(Id, Vec<MeshFace>)>,
    /// (node id, error code) for the nodes that could not be rebuilt.
    ///
    /// A code rather than a phrase: the core is a library with no language of its own, while the application
    /// has to show text in the user's language (see `crate::errors`). It also makes errors distinguishable
    /// programmatically instead of by substring matching.
    pub errors: Vec<(Id, crate::errors::CoreError)>,
    /// Repairs: geometry references resolved not by persistent id but by geometric fingerprint.
    ///
    /// Doing that silently and afresh on every rebuild means the reference is "found" every time, possibly the
    /// wrong one, with nothing reporting it. The event now appears in the report and the key itself is updated
    /// with the id found, so the guess happens once rather than every time.
    pub rebinds: Vec<Rebind>,
    /// The rebuild was stopped part-way (see [`RegenWatch`]). The report is then incomplete, and concluding
    /// from it that a feature failed to build is wrong — the pass simply never reached it. The caller has to
    /// discard such a result entirely rather than apply it partially.
    pub cancelled: bool,
}

/// Rebuild observer: learns how much has been done and can stop the rebuild.
///
/// Rebuilding a large assembly takes seconds, during which a window can do nothing but spin: there is no way
/// to report how much is left and nowhere to change one's mind. Both questions concern one place — the loop
/// over the timeline — so one interface answers both.
///
/// The core knows nothing about threads or buttons: it asks before each node and obeys the answer.
pub trait RegenWatch {
    /// Node `done` of `total` is starting. Returning `false` stops the rebuild.
    fn step(&self, done: usize, total: usize) -> bool {
        let _ = (done, total);
        true
    }
}

/// An observer that does nothing and never interferes, for callers that do not need cancellation.
pub struct NoWatch;
impl RegenWatch for NoWatch {}

/// A record of a geometry reference having been repaired.
#[derive(Clone, Debug, PartialEq)]
pub struct Rebind {
    /// Timeline node whose reference was lost.
    pub node: Id,
    /// Body that was searched.
    pub body: Id,
    /// What was repaired (a face or an edge) and onto what, for display in the tree and in the status
    /// line.
    pub what: String,
}

/// Result of the placement pass (solving the mates): diagnostics of the joint graph.
#[derive(Default, Debug)]
pub struct JointReport {
    /// Joints that close a loop, with both components already placed; these need the iterative solver.
    pub loops: Vec<Id>,
    /// Isolated joints with no path to a grounded anchor.
    pub unsolved: Vec<Id>,
    /// (joint id, error text): an unresolved connector and similar failures.
    pub errors: Vec<(Id, String)>,
}

// --- External references: controlled top-down design. ---

/// Source geometry of an external reference, in the local space of its owning component. It is resolved into
/// the local space of the consumer through `world_transform`, world space being derived.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ExternalGeom {
    /// A face of a body in another component: the body id plus the persistent face key.
    Face(Id, FaceKey),
}

/// An external reference: a controlled, directed, cross-component dependency for top-down and in-context
/// design. Without it, component isolation blocks references between parts outright. The dependency is
/// explicit, enumerable and breakable rather than a silent `body_id`. Source geometry is resolved into
/// consumer space as `consumer_local = relative_transform(src_owner, from) * src_local`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalRef {
    pub id: Id,
    /// Consumer component: the one that references outwards.
    pub from_component: Id,
    /// Source geometry in another component.
    pub to_geometry: ExternalGeom,
}

impl ExternalRef {
    /// Source body of the reference, when the geometry is bound to a body.
    pub fn source_body(&self) -> Option<Id> {
        match &self.to_geometry {
            ExternalGeom::Face(body, _) => Some(*body),
        }
    }
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
            FeatureKind::PushFace { dist, .. } => vec![("dist", dist)],
            FeatureKind::Stitch { tol, .. } => vec![("tol", tol)],
            FeatureKind::SplitFace { offset, .. } => vec![("offset", offset)],
            FeatureKind::SplitBody { offset, .. } => vec![("offset", offset)],
            FeatureKind::CircularArray { angle, .. } => vec![("angle", angle)],
            FeatureKind::LinearArray { dx, dy, dz, dx2, dy2, dz2, dx3, dy3, dz3, .. } => vec![
                ("dx", dx),
                ("dy", dy),
                ("dz", dz),
                ("dx2", dx2),
                ("dy2", dy2),
                ("dz2", dz2),
                ("dx3", dx3),
                ("dy3", dy3),
                ("dz3", dz3),
            ],
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
            | FeatureKind::Stitch { body, .. }
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
            | FeatureKind::BodyBoolean { body, .. }
            | FeatureKind::MirrorPart { body, .. }
            | FeatureKind::PartInstance { body, .. }
            | FeatureKind::Thicken { body, .. }
            | FeatureKind::SplitFace { body, .. }
            | FeatureKind::Import { body, .. } => Some(body),
            _ => None,
        }
    }

    /// Every body a node produces. For most features that is `[body]`; for a split it is one body per piece.
    /// Body listings (visibility, export, the tree) have to come from here, or every split piece but the first
    /// disappears.
    pub fn bodies(&self) -> Vec<Id> {
        match self {
            FeatureKind::SplitBody { bodies, .. } => bodies.clone(),
            _ => self.body().into_iter().collect(),
        }
    }

    /// What a node introduces into the document: the resulting bodies plus a sketch or a datum when the node
    /// creates one.
    ///
    /// It differs from [`bodies`](Self::bodies) in that a body is not the only thing a feature can bring: a
    /// sketch, a datum plane, a point and an axis also appear as timeline nodes and also serve as inputs to
    /// others. Without this, "what created it" would only answer for bodies while half the tree — sketches and
    /// datums — would have no provenance.
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
            FeatureKind::SplitBody { src, datum, .. } if datum != 0 => vec![src, datum],
            FeatureKind::SplitBody { src, .. } => vec![src],
            FeatureKind::SplitFace { src, datum, .. } if datum != 0 => vec![src, datum],
            FeatureKind::SplitFace { src, .. } => vec![src],
            FeatureKind::Thicken { src, .. } => vec![src],
            FeatureKind::Mirror { src, datum, .. } if datum != 0 => vec![src, datum],
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
            // A face copy depends on its source body but does not consume it: the body stays where it is while
            // the surface lives alongside. Otherwise taking a face into the surface layer would lose the
            // part.
            | FeatureKind::FaceCopy { src, .. }
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
            | FeatureKind::SplitBody { src, .. } => fix(src),
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
            FeatureKind::PushFace { src, body, .. } | FeatureKind::FaceCopy { src, body, .. } | FeatureKind::Patch { src, body, .. } => {
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
            FeatureKind::SplitFace { src, datum, body, .. } => {
                m(src);
                m(datum);
                m(body);
            }
            FeatureKind::SplitBody { src, datum, bodies, .. } => {
                m(src);
                m(datum);
                bodies.iter_mut().for_each(&m);
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
            | FeatureKind::Move { src, body, .. } => {
                m(src);
                m(body);
            }
            FeatureKind::CircularArray { src, axis, body, .. } => {
                m(src);
                m(axis);
                m(body);
            }
            FeatureKind::Mirror { src, datum, body, .. } => {
                m(src);
                m(datum);
                m(body);
            }
            FeatureKind::BodyBoolean { a, b, body, .. } => {
                m(a);
                m(b);
                m(body);
            }
            // `source` is the shared embedded file and is not part of the clone map, so it is left alone; the
            // body is remapped.
            FeatureKind::Import { body, .. } => m(body),
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
            FeatureKind::FaceCopy { faces, .. } | FeatureKind::SurfaceReplace { faces, .. } => faces.remap_descs(&mut |d| m(d)),
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
            | FeatureKind::SplitBody { src, .. }
            // A thicken carries its body onward. Leaving the plate as a separate body turns one part into
            // two, visible on screen as a differently coloured piece, which the "one part is one body" rule
            // forbids: the plate is welded on and the source is consumed.
            | FeatureKind::Thicken { src, .. }
            // Splitting faces carries the same body onward (there is only one body), so the source is consumed
            // as it is for a chamfer.
            | FeatureKind::SplitFace { src, .. } => Some(src),
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
