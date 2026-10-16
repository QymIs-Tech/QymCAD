//! THE RECORDS OF AN ASSEMBLY: components, connectors, constraints, relations, joints.

use super::*;

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

pub(crate) fn place_identity() -> [f64; 12] {
    PLACE_IDENTITY
}
pub(crate) fn default_true() -> bool {
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
    // THE INDEX IS AN ARGUMENT, not a way of reaching an element: `clamp_slot(slot, v)` needs the slot's
    // number to pick the right limit. An iterator would have to hand it back through `enumerate` anyway.
    #[allow(clippy::needless_range_loop)]
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
