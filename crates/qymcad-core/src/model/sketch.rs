//! Sketches: planar geometry, contours and their solving.
//!
//! The split is mechanical: a method belongs here because it touches the fields of this subsystem alone, not
//! because of what it is called.

use super::*;
use super::tess::*; // 2D sketch geometry: profiles, tessellation, region analysis.

/// A TEXT OBJECT IN A SKETCH: where it sits, how tall it stands, how it is turned, what it says and the
/// outlines it was traced into.
///
/// The glyphs travel with the string because the fonts are not part of the document: the outlines are
/// traced once, when the text is typed, and are what the rebuild actually extrudes. Adding a text and
/// changing one took the same five things as five separate arguments each.
pub struct TextSpec {
    pub at: Point2,
    pub height: f64,
    pub angle: f64,
    pub text: String,
    pub glyphs: Vec<Vec<Point2>>,
    /// The font the glyphs were baked from; it travels with them, see `crate::model::FontRef`.
    pub font: crate::model::FontRef,
}

/// THE SIZE OF A SKETCH CHAMFER: how (`ChamferMode`), the first value and the second. For a symmetric chamfer the first
/// is the length of the cut itself and there is no second; for two legs, the first leg - along the first line - and the
/// second; for a leg and an angle, the first leg and the angle from the first line in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChamferLegs {
    pub mode: crate::feature::ChamferMode,
    pub first: f64,
    pub second: f64,
}

impl ChamferLegs {
    /// A symmetric chamfer whose cut is `d` long.
    pub fn equal(d: f64) -> Self {
        ChamferLegs { mode: crate::feature::ChamferMode::Symmetric, first: d, second: 0.0 }
    }

    /// The two legs along the lines of a corner whose angle is `corner` (radians); `None` where the size cannot be
    /// laid: a leg of no length, or an angle that with the corner leaves no triangle (law of sines).
    pub fn along(&self, corner: f64) -> Option<(f64, f64)> {
        let (d1, d2) = match self.mode {
            // the cut is the base of an isosceles triangle whose apex is the corner: each leg is cut / (2 sin(corner / 2))
            crate::feature::ChamferMode::Symmetric => {
                let base = 2.0 * (corner / 2.0).sin();
                if base <= 1e-9 {
                    return None;
                }
                (self.first / base, self.first / base)
            }
            crate::feature::ChamferMode::TwoDist => (self.first, self.second),
            crate::feature::ChamferMode::DistAngle => {
                let a = self.second.to_radians();
                if a <= 0.0 || a + corner >= std::f64::consts::PI - 1e-9 {
                    return None;
                }
                (self.first, self.first * a.sin() / (a + corner).sin())
            }
        };
        (d1 > 1e-9 && d2 > 1e-9 && d1.is_finite() && d2.is_finite()).then_some((d1, d2))
    }
}

/// WHICH OF THE TWO CORNER TOOLS A VALUE IS FOR, and for a chamfer how its first value is read: the length of the cut for a
/// symmetric one, a leg for the others. What the most a corner takes is depends on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CornerTool {
    Fillet,
    Chamfer(crate::feature::ChamferMode),
}

/// THE CUT OF A CORNER: a fillet of its size - a radius, a chord or an arc length - or a chamfer of its legs. The same
/// cut is what the corner is shown with before it is made and what is made: a fillet by its chord was shown as a
/// fillet of that radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CornerCut {
    Fillet(FilletSize),
    Chamfer(ChamferLegs),
}

/// ONE CORNER OF A SET: the point it stands at and the two edges that make it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerAt {
    pub point: Id,
    pub pair: (Id, Id),
}

/// HOW A SKETCH FILLET IS GIVEN: by its radius, by its chord - the straight distance between the two points where it
/// meets the lines - or by the length of its arc.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FilletBy {
    #[default]
    Radius,
    Chord,
    ArcLength,
}

/// THE SIZE OF A SKETCH FILLET: how it is given and the value in that way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilletSize {
    pub by: FilletBy,
    pub value: f64,
}

impl FilletSize {
    /// A fillet of radius `r`.
    pub fn radius(r: f64) -> Self {
        FilletSize { by: FilletBy::Radius, value: r }
    }

    /// The radius of the fillet on a corner whose arc turns through `sweep` (radians, pi minus the angle between the
    /// lines): the chord is 2 r sin(sweep / 2), the arc r sweep. On a square corner a chord of 5 is a radius of 3.54,
    /// an arc of 5 a radius of 3.18. `None` for a value of no length or a corner with no turn.
    pub fn radius_on(&self, sweep: f64) -> Option<f64> {
        let r = match self.by {
            FilletBy::Radius => self.value,
            FilletBy::Chord => self.value / (2.0 * (sweep / 2.0).sin()),
            FilletBy::ArcLength => self.value / sweep,
        };
        (self.value > 1e-9 && sweep > 1e-6 && r.is_finite() && r > 1e-9).then_some(r)
    }

    /// The value in this way of a fillet of radius `r` on a corner whose arc turns through `sweep`: the inverse of
    /// `radius_on`, for the largest value a corner takes.
    pub fn of_radius(by: FilletBy, r: f64, sweep: f64) -> f64 {
        match by {
            FilletBy::Radius => r,
            FilletBy::Chord => 2.0 * r * (sweep / 2.0).sin(),
            FilletBy::ArcLength => r * sweep,
        }
    }
}

/// THE CONSTRAINTS A RECTANGLE IS HELD BY WHEN AN ANGLE DIMENSION ON ITS SIDE TAKES ITS TURN: opposite sides parallel
/// and the first two square - its shape without a turn of its own.
pub(crate) fn rect_free_constraints(c0: Id, c1: Id, c2: Id, c3: Id) -> [Constraint; 3] {
    [
        Constraint::Parallel { a: c0, b: c1, c: c3, d: c2 },
        Constraint::Parallel { a: c0, b: c3, c: c1, d: c2 },
        Constraint::Perpendicular { a: c0, b: c1, c: c0, d: c3 },
    ]
}

/// Is `c` one of the constraints rectangle `r` holds itself by, held by its turns or by its shape alone?
fn is_rect_own(r: &crate::model::SketchRect, c: &Constraint) -> bool {
    let [c0, c1, c2, c3] = r.corners;
    rect_own_constraints(c0, c1, c2, c3, r.centre, 0.0).iter().chain(rect_free_constraints(c0, c1, c2, c3).iter()).any(|o| same_rect_constraint(o, c))
}

/// Is `c` the constraint `own` a rectangle holds itself by - the same kind on the same points, whatever the turn held?
fn same_rect_constraint(own: &Constraint, c: &Constraint) -> bool {
    match (own, c) {
        (Constraint::Orientation { a, b, .. }, Constraint::Orientation { a: x, b: y, .. }) => a == x && b == y,
        (Constraint::Parallel { a, b, c: cc, d }, Constraint::Parallel { a: w, b: x, c: y, d: z })
        | (Constraint::Perpendicular { a, b, c: cc, d }, Constraint::Perpendicular { a: w, b: x, c: y, d: z }) => [a, b, cc, d] == [w, x, y, z],
        (Constraint::Midpoint { p, a, b }, Constraint::Midpoint { p: q, a: x, b: y }) => [p, a, b] == [q, x, y],
        _ => false,
    }
}

/// A PLACE A LINE CAN BE EXTENDED TO along its axis: `t` along it from its start (0) to its end (1), and the curve met
/// there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtendStop {
    pub t: f64,
    pub by: Id,
}

/// THE PLACES BEYOND EACH END OF A LINE, nearest first: before its start, past its end.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtendStops {
    pub before_a: Vec<ExtendStop>,
    pub past_b: Vec<ExtendStop>,
}

/// WHICH ENDS THE EXTEND TOOL TAKES: the nearer one, or both ("Both sides" in its bar).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExtendSides {
    Nearer,
    Both,
}

/// WHAT THE EXTEND TOOL ASKS: where the pointer is, the curve it is over, and which ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtendAsk {
    pub pointer: crate::geom::Point2,
    pub over: Option<Id>,
    pub sides: ExtendSides,
}

/// WHERE EACH END OF A LINE GOES along its axis (`ExtendStop::t`), or `None` where it stays, and whether an end that
/// had somewhere to go was kept because it is joined to other geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineExtension {
    pub a: Option<f64>,
    pub b: Option<f64>,
    pub kept: EndKept,
}

/// A STRETCH OF A STRAIGHT OR OF A CIRCLE, from one parameter to the other: a length along a line, or angles taken
/// counter-clockwise round a circle.
#[derive(Clone, Copy)]
pub(crate) struct Stretch {
    from: f64,
    to: f64,
}

/// A ROUND CURVE BY ITS CIRCLE: the centre, the radius, and the stretch of it an arc takes (none for a whole circle).
#[derive(Clone, Copy)]
pub(crate) struct OnCircle {
    centre: (f64, f64),
    r: f64,
    span: Option<Stretch>,
}

/// WHERE TWO LINES CROSS: the two lines and the point of their crossing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinesCrossing {
    pub one: Id,
    pub other: Id,
    pub at: Point2,
}

/// WHY AN END OF A LINE STAYS WHERE IT IS under Extend.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum EndKept {
    /// nothing kept it: an end with nowhere to go stays, and so does the end the tool did not ask for
    #[default]
    Free,
    /// it is joined to other geometry - the end of another curve, a point tied onto it, a dimension - and is not moved
    Joined,
}

/// WHETHER THE CENTRE OF A RECTANGLE IS HELD by the sketch - fixed, or set by dimensions - so that no motion the
/// constraints allow moves it (`solver::free_points`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Centre {
    Held,
    Free,
}

/// WHAT A SOLVE HOLDS OF RECTANGLE `r` so that its size changes from where it should, `dragged` being the point under
/// the hand. A dragged corner stretches the rectangle from the corner across from it - a corner shares a side with each
/// neighbour, so with a neighbour held it could only slide along that side, one size at a time. A rectangle drawn from
/// its centre holds the centre under a dragged corner, and so does one drawn from a corner whose centre is held: the
/// corner across is the reflection of the dragged one through the held centre, and held it leaves the dragged one
/// nowhere to go. Reported behaviour: "a rectangle drawn from a corner, its centre fixed - a corner cannot be dragged;
/// with the centre let go it drags as it should". Not dragged, its anchor is held: the centre, or the corner it was
/// drawn from, so that a width or a height typed grows it from there. Nothing when the centre is dragged: the rectangle
/// goes with it.
pub(super) fn held_for_size(r: &crate::model::SketchRect, dragged: Option<Id>, centre: Centre) -> Option<Id> {
    if dragged == Some(r.centre) {
        return None;
    }
    let corner_dragged = dragged.and_then(|d| r.corners.iter().position(|k| *k == d));
    match (r.anchor, corner_dragged) {
        (crate::model::RectAnchor::Centre, _) => Some(r.centre),
        (crate::model::RectAnchor::Corner(_), Some(_)) if centre == Centre::Held => Some(r.centre),
        (crate::model::RectAnchor::Corner(_), Some(k)) => Some(r.corners[(k + 2) % 4]),
        (crate::model::RectAnchor::Corner(c), None) => Some(c),
    }
}

/// Where a rectangle was drawn from: a corner (its index round the rectangle) or the centre.
#[derive(Clone, Copy)]
pub(crate) enum DrawnFrom {
    Corner(usize),
    Centre,
}

/// THE CONSTRAINTS A RECTANGLE HOLDS ITSELF BY, corners `c0`..`c3` in order round it: every side at its turn - the
/// first and the third at `deg`, the second and the fourth square to them - and the centre on the middle of the
/// diagonal c0 - c2. Four for the shape and its turn, two for the centre: a rectangle of four corners and a centre (ten
/// unknowns) is left with its place, its width and its height. At 0 deg the four are exactly Horizontal and Vertical,
/// and as stable: the same shape held by parallel and perpendicular sides let a side squeezed short by contradicting
/// constraints drift a point by 2.3 to 243 mm from one solve to the next (7 of 400 random sketches).
pub(crate) fn rect_own_constraints(c0: Id, c1: Id, c2: Id, c3: Id, centre: Id, deg: f64) -> [Constraint; 5] {
    [
        Constraint::Orientation { a: c0, b: c1, deg },
        Constraint::Orientation { a: c3, b: c2, deg },
        Constraint::Orientation { a: c1, b: c2, deg: deg + 90.0 },
        Constraint::Orientation { a: c0, b: c3, deg: deg + 90.0 },
        Constraint::Midpoint { p: centre, a: c0, b: c2 },
    ]
}

/// WHAT A FILLET OR A CHAMFER WOULD LEAVE AT A CORNER: the point of the corner, the two ends the edges will be cut
/// at, and the arc that will stand between them (`None` for a chamfer, which is a straight cut).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerBlend {
    pub vertex: [f64; 2],
    pub ends: [[f64; 2]; 2],
    pub arc: Option<([f64; 2], f64)>,
}

impl Project {
    /// Add a work plane with a stable id (a timeline node). Returns its id.
    /// THE TEXT OF A NOTE ON A SKETCH.
    ///
    /// Reaching two levels down (`sketches[si].notes[ni].text`) from a panel means the panel knows how
    /// notes are stored. Here it knows only that a note has text.
    pub fn set_note_text(&mut self, si: usize, ni: usize, text: impl Into<String>) -> bool {
        match self.sketches.get_mut(si).and_then(|s| s.notes.get_mut(ni)) {
            Some(n) => {
                n.text = text.into();
                true
            }
            None => false,
        }
    }

    /// RENAME A WORK PLANE.
    pub fn rename_plane(&mut self, id: Id, name: impl Into<String>) -> bool {
        match self.planes.iter_mut().find(|p| p.id == id) {
            Some(p) => {
                p.name = name.into();
                true
            }
            None => false,
        }
    }

    pub fn add_plane(&mut self, mut pl: WorkPlane) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        if pl.id == 0 {
            pl.id = self.alloc_id();
        }
        let id = pl.id;
        let name = pl.name.clone();
        let parent = Some(self.active_ctx());
        self.planes.push(pl);
        self.push_timeline(FeatureNode { id, name, kind: FeatureKind::Plane { plane: id }, parent, dirty: false, suppressed: false });
        id
    }
    /// Resolve a parametric datum plane: recompute its origin and normal from `def` and write them into the
    /// `WorkPlane`, so the rest of the code (the sketch frame, the renderer) reads finished values. `dist` is
    /// parametric through the `dist` feature dimension of the plane node. `Manual` is left alone.
    pub(super) fn resolve_plane_into(&mut self, plane_id: Id, vars: &std::collections::HashMap<String, f64>, dim: Option<&std::collections::HashMap<String, String>>) {
        let Some(pi) = self.planes.iter().position(|p| p.id == plane_id) else { return };
        let placed = match self.planes[pi].def {
            PlaneDef::OffsetBase { base, dist } => {
                let dist = eval_dim(dim, "dist", dist, vars);
                let f = base.frame();
                // the side the preview shows (`offset_direction`); the normal stays the frame's, as sketches on it lie
                let d = base.offset_direction();
                self.planes[pi].normal = f.normal();
                self.planes[pi].origin = [f.origin[0] + d[0] * dist, f.origin[1] + d[1] * dist, f.origin[2] + d[2] * dist];
                true
            }
            PlaneDef::OffsetFace { body, face, dist } => {
                let dist = eval_dim(dim, "dist", dist, vars);
                let (c, n) = self.resolve_face(body, &face); // Persistent face resolution: centre and
                                                             // normal.
                self.planes[pi].normal = n;
                self.planes[pi].origin = [c[0] + n[0] * dist, c[1] + n[1] * dist, c[2] + n[2] * dist];
                true
            }
            PlaneDef::OffsetPlane { plane: src, dist } => {
                // The source plane is resolved earlier in the timeline, having been created earlier, so its
                // origin and normal are already available.
                let dist = eval_dim(dim, "dist", dist, vars);
                match self.planes.iter().find(|p| p.id == src).map(|p| (p.origin, p.normal)) {
                    Some((o, n)) => {
                        let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
                        let nn = [n[0] / nl, n[1] / nl, n[2] / nl];
                        self.planes[pi].normal = n;
                        self.planes[pi].origin = [o[0] + nn[0] * dist, o[1] + nn[1] * dist, o[2] + nn[2] * dist];
                        true
                    }
                    None => false,
                }
            }
            PlaneDef::Manual | PlaneDef::FaceGone | PlaneDef::PlaneGone => false,
        };
        // THE TILT AND THE TURN, parametric as the offset is: applied to the normal just resolved from the definition,
        // never to the one left from the rebuild before - that would tilt it again on every rebuild
        if placed {
            let pl = &mut self.planes[pi];
            let tilt = crate::feature::PlaneTilt { about_x: eval_dim(dim, "tilt_x", pl.tilt.about_x, vars), about_y: eval_dim(dim, "tilt_y", pl.tilt.about_y, vars) };
            // an untilted plane keeps the normal its definition gave bit for bit: normalised, a face's normal moved in its
            // last bits, and a project just opened called itself unsaved
            if tilt != crate::feature::PlaneTilt::default() {
                pl.normal = tilt.apply(pl.normal);
            }
            pl.rot_deg = eval_dim(dim, "turn", pl.rot_deg, vars);
        }
    }
    /// Deep clone of sketch `sid` into a new node under component `target_parent`.
    ///
    /// The internal ids of points, entities and constraints are local and are reused as they are, which is safe
    /// since none of them reference anything outside. Only `Sketch::id` is new, and the timeline node references
    /// it. With a `remap` map given, `source` and `plane` are re-pointed at the cloned objects (for a clone made
    /// as part of a component); otherwise they are kept. Returns the id of the new sketch, which equals the id of
    /// its timeline node.
    pub(super) fn clone_sketch_impl(&mut self, sid: Id, target_parent: Id, remap: &std::collections::HashMap<Id, Id>) -> Option<Id> {
        let si = self.sketch_index(sid)?;
        let mut sk = self.sketches[si].clone();
        let new_id = self.alloc_id();
        sk.id = new_id;
        sk.contour_ids.clear(); // Regenerated afresh, with their own contour ids.
        if let Some(src) = sk.source {
            if let Some(&n) = remap.get(&src) {
                sk.source = Some(n);
            }
        }
        // Placement: a datum or a face of another body is re-pointed at the clone, when it is in the map.
        sk.plane = match sk.plane {
            crate::feature::SketchPlane::Datum(p) => crate::feature::SketchPlane::Datum(remap.get(&p).copied().unwrap_or(p)),
            crate::feature::SketchPlane::Face(b, f) => crate::feature::SketchPlane::Face(remap.get(&b).copied().unwrap_or(b), f),
            other => other,
        };
        // The anchor may still point at a face or a datum of another component (with the tree clipboard the
        // remap is empty and the source body is not carried over). Such a cross-component reference does not
        // resolve (`sketch_frame` is `None`) and the sketch would hang: no gizmo, no contour selection, nothing
        // to extrude. It is detached onto the XY plane of the target part, which preserves the flat profile in
        // local 2D and makes the sketch self-contained.
        use crate::feature::{BasePlane, SketchPlane};
        let carried = |slf: &Self, owner: Option<Id>| owner.is_none_or(|o| slf.component_is_within(o, target_parent));
        let attached = match sk.plane {
            SketchPlane::Face(body, _) => carried(self, self.body_owner(body)),
            SketchPlane::Datum(p) => carried(self, self.plane_owner(p)),
            SketchPlane::World(_) => true,
        };
        if !attached {
            sk.plane = SketchPlane::World(BasePlane::XY);
        }
        let name = sk.name.clone();
        self.sketches.push(sk);
        let ni = self.sketches.len() - 1;
        self.regen_sketch(ni);
        // The timeline node under the target component.
        use crate::feature::{FeatureKind, FeatureNode};
        self.push_timeline(FeatureNode { id: new_id, name, kind: FeatureKind::Sketch { sketch: new_id }, parent: Some(target_parent), dirty: false, suppressed: false });
        Some(new_id)
    }
    /// The body whose face a sketch is placed on, when its plane is a `Face`; otherwise `None`. Used by the
    /// isolation check: a sketch must not rest on a face of another component's body.
    pub fn sketch_plane_body(&self, sketch: Id) -> Option<Id> {
        let si = self.sketch_index(sketch)?;
        match self.sketches[si].plane {
            crate::feature::SketchPlane::Face(body, _) => Some(body),
            _ => None,
        }
    }
    /// Settle the sketches before building: a body must not be built from geometry that does not yet satisfy
    /// its own constraints.
    ///
    /// Measured case: a slot 5 wide sat collinear with a construction line dimensioned at exactly 130 from the
    /// axis, while the file held x = -129.99900 and -129.99930. The constraints were not grossly violated, yet
    /// they were not satisfied either — the endpoints of a vertical line differed by 0.3 micrometres. The cut
    /// was built against exactly that geometry and left a film a fraction of a micrometre thick: a wall where
    /// an opening was expected, and after an edit, a wall on the other side. One full solve moves the points to
    /// -129.999999908 (0.1 nanometres from the dimension) and the film disappears.
    ///
    /// An interactive drag solves the sketch on a reduced budget for responsiveness, and the interface runs a
    /// full solve on release, but catching every interface path is pointless: the invariant is cheaper to hold
    /// here. For an already solved sketch this is a single residual evaluation, the solver exiting on the first
    /// iteration, so there is no measurable cost.
    /// A DOCUMENT READ FROM A FILE, brought to the form a rebuild brings it to: the frame of each sketch, its contours
    /// in order. Done as it is read, so the document opened is the one that then stands untouched.
    pub fn settle_loaded(&mut self) {
        self.settle_sketches();
    }

    pub(super) fn settle_sketches(&mut self) {
        // Contours loaded from a file may have been written by an older build with a clockwise traversal, or
        // with a polyline that drifted apart from the exact edges. Opening a document recomputes nothing, so
        // such a contour reaches the kernel and produces a face with an inside-out hole. It is canonicalised
        // here; the operation is idempotent and changes nothing for freshly built contours.
        for c in self.contours.iter_mut() {
            c.canonicalize();
        }
        // The nesting graph is not in the file either, and waiting for a sketch edit to create it would mean
        // two behaviours for one model. It is computed for every sketch that lacks one; after that it lives with
        // the sketch regeneration.
        let need: Vec<Vec<Id>> = self.sketches.iter().map(|s| s.contour_ids.clone()).filter(|ids| !ids.is_empty() && !ids.iter().any(|c| self.contours.parent_of(*c).is_some())).collect();
        for ids in need {
            self.rebuild_contour_nesting(&ids);
        }
        // No `eval_parameters` here: the dimension values are taken as they are. Re-evaluating the expressions
        // is a separate step (editing a parameter or a sketch), and doing it from here would overwrite values
        // set directly, changing a feature height during an ordinary build.
        for si in 0..self.sketches.len() {
            let before: Vec<(Id, f64, f64)> = self.sketches[si].points.iter().map(|p| (p.id, p.x, p.y)).collect();
            self.solve_sketch_inner(si, None, crate::solver::Budget::WHOLE, Rebuild::Whole);
            let moved = self.sketches[si].points.iter().zip(before.iter()).any(|(p, (id, x, y))| p.id != *id || (p.x - x).abs() > 1e-9 || (p.y - y).abs() > 1e-9);
            if moved {
                self.regen_sketch(si); // The points moved, so the contours are rebuilt; otherwise the profile
                                       // stays as it was.
            }
        }
    }
    /// Bounding box of all the geometry.
    pub fn bounds(&self) -> Option<Bbox> {
        let mut acc: Option<Bbox> = None;
        for c in self.contours.iter() {
            if let Some(b) = c.bbox() {
                acc = Some(match acc {
                    None => b,
                    Some(a) => Bbox { min: crate::geom::Point2::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y)), max: crate::geom::Point2::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y)) },
                });
            }
        }
        acc
    }
}

/// Angle of the witness line of a fillet radius, pointing into the corner, in screen radians.
///
/// Zero means to the right, and the "R6.0" label then lands exactly on a tangency point: on a rounded
/// rectangle the tangency points sit to the left and right of the arc centre. Found on a screenshot of a sketch
/// run — the numbers show nothing, the constraints are intact and the sketch solves.
///
/// Outwards is wrong too: along the bisector on the outside stands the virtual corner, the sharp vertex a
/// fillet leaves in place for the dimensions. The label lands on it and the guard reddens all four corners
/// instead of two.
///
/// Into the corner is the only direction that is empty by construction: that is the open part of the shape. The
/// tangency points lie ninety degrees to the sides and the virtual corner lies behind. The screen Y axis points
/// down, which is why the sign of the angle is inverted.
/// A closed loop without repeated points: coincident neighbours are dropped, and so is a closing point equal
/// to the first one.
///
/// Reported behaviour: "only the built-in font extrudes, four fonts of my own and none of them works".
/// Measured on the reporter's file: every glyph loop carried exactly one such pair, and the rebuild said
/// "Extrude failed (check the contour)". A loop is closed by being a loop; saying so twice is a segment of
/// zero length, and OCCT refuses to build a wire out of one.
fn seam_cleaned(loop_: &[Point2]) -> Vec<Point2> {
    let mut out: Vec<Point2> = Vec::with_capacity(loop_.len());
    for p in loop_ {
        if out.last().is_none_or(|q: &Point2| q.x != p.x || q.y != p.y) {
            out.push(*p);
        }
    }
    while out.len() >= 2 && out[0].x == out[out.len() - 1].x && out[0].y == out[out.len() - 1].y {
        out.pop();
    }
    out
}

/// THE HORIZONTAL AND VERTICAL DIMENSIONS THAT MEASURE A WHOLE SIDE: a `Distance` along an axis (1 or 2) between two
/// points that both turn (`turns`), the two standing on that axis's line, so the measure along the axis is the whole
/// length between them. Answers their indices. A dimension of the projection of a slanted line is not among them: along
/// the line it would say another number.
pub(crate) fn axis_dims_of_whole_sides(s: &crate::model::Sketch, turns: &dyn Fn(Id) -> bool) -> Vec<usize> {
    let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    s.constraints
        .iter()
        .enumerate()
        .filter_map(|(i, c)| match *c {
            Constraint::Distance { a, b, axis: axis @ (1 | 2), .. } if turns(a) && turns(b) => {
                let ((ax, ay), (bx, by)) = (at(a)?, at(b)?);
                let (len, across) = ((bx - ax).hypot(by - ay), if axis == 1 { (by - ay).abs() } else { (bx - ax).abs() });
                (len > 1e-9 && across <= 1e-6 * len).then_some(i)
            }
            _ => None,
        })
        .collect()
}

/// THE DIMENSIONS `dims` TURNED BY `deg` WITH THEIR SIDES, LAID ALONG THEM: a horizontal or vertical dimension of a
/// side becomes one along the side (axis 0), on the side of it it stood, as far from it - its offset turned with the
/// side and read across the side as it stands now. Left horizontal and vertical, the dimensions of a rectangle turned by
/// 30 deg stayed on the axes where they stood, measuring along them rather than along the sides.
pub(crate) fn lay_along_their_sides(s: &mut crate::model::Sketch, dims: &[usize], deg: f64) {
    let (sn, cs) = (deg.to_radians().sin(), deg.to_radians().cos());
    let at = |s: &crate::model::Sketch, id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    for &i in dims {
        let Some(Constraint::Distance { a, b, axis, off, .. }) = s.constraints.get(i).cloned() else { continue };
        let (Some((ax, ay)), Some((bx, by))) = (at(s, a), at(s, b)) else { continue };
        // where the dimension stood off its side, in the world: down the screen is -y for a horizontal one, right +x for a
        // vertical one; turned with the side
        let (vx, vy) = if axis == 1 { (0.0, -off) } else { (off, 0.0) };
        let (vx, vy) = (vx * cs - vy * sn, vx * sn + vy * cs);
        // a dimension along its line stands off it along the world normal (dy, -dx) of its first end looking at its second
        let (dx, dy) = (bx - ax, by - ay);
        let len = dx.hypot(dy);
        if len < 1e-9 {
            continue;
        }
        if let Some(Constraint::Distance { axis, off, .. }) = s.constraints.get_mut(i) {
            *axis = 0;
            *off = (vx * dy - vy * dx) / len;
        }
    }
}

/// ONE END OF A LINEAR DIMENSION: its point and where the point stands.
#[derive(Clone, Copy)]
pub(crate) struct DimEnd {
    pub(crate) id: Id,
    pub(crate) at: Point2,
}

/// WHICH SIDE OF ITS TWO ENDS A LINEAR DIMENSION IS TO STAND ON: the side of a point, or the side away from one.
#[derive(Clone, Copy)]
pub(crate) enum Side {
    Toward(Point2),
    AwayFrom(Point2),
}

/// THE ORDER OF THE TWO ENDS OF A LINEAR DIMENSION THAT PUTS IT ON `side`: a linear dimension is drawn off to the left of
/// its first end looking at the second, on screen - the world normal (dy, -dx) - so the order is the side.
pub(crate) fn ends_facing([a, b]: [DimEnd; 2], side: Side) -> [Id; 2] {
    let (dx, dy) = (b.at.x - a.at.x, b.at.y - a.at.y);
    let (mx, my) = ((a.at.x + b.at.x) / 2.0, (a.at.y + b.at.y) / 2.0);
    let (q, toward) = match side {
        Side::Toward(q) => (q, true),
        Side::AwayFrom(q) => (q, false),
    };
    let on_normal = dy * (q.x - mx) - dx * (q.y - my) > 0.0;
    if on_normal == toward {
        [a.id, b.id]
    } else {
        [b.id, a.id]
    }
}

/// WHERE THE RADIUS OF A FILLET IS LED: from the centre out through its arc, a quarter of the arc off its middle towards
/// the more level of its two ends. Its text stands on a level shelf past the arc, and led through the middle it stood
/// over the sharp of the corner, which lies on the same bisector 0.41 r past the arc of a square corner. The answer is
/// an angle on the screen, where y runs down: the angle of the world direction (x, y) is atan2(-y, x). Taken as
/// atan2(y, -x) it pointed the other way, from the centre into the shape.
fn fillet_label_angle(points: &[SketchPoint], cen: Id, t1: Id, t2: Id) -> f64 {
    let get = |id: Id| points.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
    let (Some(c), Some(a), Some(b)) = (get(cen), get(t1), get(t2)) else { return 0.0 };
    let unit = |(x, y): (f64, f64)| {
        let l = x.hypot(y);
        (l > 1e-12).then(|| (x / l, y / l))
    };
    let (Some(ua), Some(ub)) = (unit((a.0 - c.0, a.1 - c.1)), unit((b.0 - c.0, b.1 - c.1))) else { return 0.0 };
    let Some(mid) = unit((ua.0 + ub.0, ua.1 + ub.1)) else {
        return 0.0; // The tangency points are diametrically opposite, so there is no bisector.
    };
    let level = if ua.0.abs() >= ub.0.abs() { ua } else { ub };
    let (x, y) = unit((mid.0 + level.0, mid.1 + level.1)).unwrap_or(mid);
    (-y).atan2(x)
}

/// THE POINTS OF AN ARC and the way it runs from `a` to `b`.
pub(crate) struct ArcEnds {
    center: Id,
    a: Id,
    b: Id,
    ccw: bool,
}

/// WHERE THE RADIUS OF AN ARC IS LED: from the centre out through the middle of the arc as it runs from its start to its
/// end, so the leader meets the arc that is drawn. A leader to the right (zero) misses an arc lying below its centre.
/// Reported behaviour: "the size on an arc is not put on the segment of the arc that is seen". The middle is taken
/// along the way the arc runs, not as the bisector of its ends: an arc over half a turn has its middle on the far side
/// of that bisector. An angle on the screen, where y runs down: the world angle t is -t there.
fn arc_label_angle(points: &[SketchPoint], arc: ArcEnds) -> f64 {
    let get = |id: Id| points.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
    let (Some(c), Some(a), Some(b)) = (get(arc.center), get(arc.a), get(arc.b)) else { return 0.0 };
    let (ta, tb) = ((a.1 - c.1).atan2(a.0 - c.0), (b.1 - c.1).atan2(b.0 - c.0));
    let tau = std::f64::consts::TAU;
    let mid = if arc.ccw { ta + (tb - ta).rem_euclid(tau) / 2.0 } else { ta - (ta - tb).rem_euclid(tau) / 2.0 };
    -mid
}

/// A CIRCLE OR AN ARC READ AS ONE THING: the centre, the radius, and for an arc the angular range.
///
/// It used to be built twice, in two commands, as the same five-place tuple with the same comment beside it -
/// and `Option<(f64, f64, bool)>` in a signature says nothing about what the three are.
pub(crate) struct Round {
    /// The point that is the centre.
    center_id: Id,
    /// The centre, in sketch coordinates.
    cx: f64,
    cy: f64,
    /// The radius.
    r: f64,
    /// `None` for a circle. For an arc: the start angle, the end angle, and whether it runs anticlockwise.
    span: Option<(f64, f64, bool)>,
}

/// Read a circle or an arc; anything else, or a missing point, gives `None`.
fn round_entity(s: &crate::model::Sketch, e: &crate::model::SketchEntity) -> Option<Round> {
    let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    match e.kind {
        EntityKind::Circle { center, r } => {
            let (cx, cy) = p(center)?;
            Some(Round { center_id: center, cx, cy, r, span: None })
        }
        EntityKind::Arc { center, a, b, ccw } => {
            let ((cx, cy), (ax, ay), (bx, by)) = (p(center)?, p(a)?, p(b)?);
            let r = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
            Some(Round { center_id: center, cx, cy, r, span: Some(((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx), ccw)) })
        }
        _ => None,
    }
}

fn at_of(pts: &[SketchPoint], id: Id) -> Option<(f64, f64)> {
    pts.iter().find(|p| p.id == id).map(|p| (p.x, p.y))
}

/// WHAT A SEGMENT OF AN OFFSET LOOP WAS OFFSET FROM: a source line (its two points), a source arc (its centre and
/// ends), a sharp corner of the source the offset rounds (its point), or nothing found.
#[derive(Clone, Copy)]
pub(crate) enum OffsetSource {
    Line(Id, Id),
    Arc { center: Id, a: Id, b: Id },
    Corner(Id),
    None,
}

/// How far `p` stands from the segment `a` - `b`.
fn seg_dist(p: Point2, a: Point2, b: Point2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-18 { 0.0 } else { (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0) };
    (p.x - a.x - dx * t).hypot(p.y - a.y - dy * t)
}

/// THE DISTANCES OF THE COPY FROM ITS SOURCE LINES, SIGNED AS THEY STAND: a distance from a point to a line is signed by
/// the side of the line it is on, its way from the line's first point to its second (`DistancePL`), and the offset was
/// laid unsigned. Where a source line runs against the way of its chain the copy stood on the side the sign did not
/// say: measured on a single line offset 2, one redundant constraint and a freedom too many, and the copy of a polyline
/// pulled across its source when a corner of it was moved.
fn side_of_place(mut held: Vec<Constraint>, s: &super::Sketch) -> Vec<Constraint> {
    let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    for c in &mut held {
        if let Constraint::DistancePL { p, a, b, d, .. } = c {
            if let (Some(pp), Some(pa), Some(pb)) = (at(*p), at(*a), at(*b)) {
                let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
                let len = dx.hypot(dy);
                if len > 1e-12 {
                    let side = (dx * (pp.1 - pa.1) - dy * (pp.0 - pa.0)) / len;
                    *d = d.abs().copysign(side);
                }
            }
        }
    }
    held
}

/// WHAT AN OFFSET COPY IS: a loop, every corner of it between two of its curves, or a chain with two free ends, each
/// standing square to the end of the source it was offset from where that end is known.
#[derive(Clone, Copy)]
pub(crate) enum OffsetShape {
    Closed,
    Open { first: Option<EndOf>, last: Option<EndOf> },
}

/// THE END OF THE SOURCE an end of an open copy stands square to.
#[derive(Clone, Copy)]
pub(crate) struct EndOf {
    src: Id,
}

/// A line of the source of an offset: its end points, and where they stand.
pub(crate) struct SourceLine {
    a: Id,
    b: Id,
    pa: (f64, f64),
    pb: (f64, f64),
}

/// An arc of the source of an offset: its centre and end points, where the centre stands, and its radius.
pub(crate) struct SourceArc {
    center: Id,
    a: Id,
    b: Id,
    at: (f64, f64),
    radius: f64,
}

/// The source line a copy line from `v` to `w` runs along: parallel to it, and as far from it as the offset.
fn offset_line_source(lines: &[SourceLine], v: (f64, f64), w: (f64, f64), dist: f64) -> OffsetSource {
    let tol = 1e-6 * (1.0 + dist.abs());
    lines
        .iter()
        .find(|SourceLine { pa: p, pb: q, .. }| {
            let (dx, dy) = (q.0 - p.0, q.1 - p.1);
            let len = dx.hypot(dy).max(1e-12);
            let cross = (dx * (w.1 - v.1) - dy * (w.0 - v.0)).abs() / (len * (w.0 - v.0).hypot(w.1 - v.1)).max(1e-12);
            let off = ((v.0 - p.0) * dy - (v.1 - p.1) * dx).abs() / len;
            cross < 1e-6 && (off - dist.abs()).abs() < tol
        })
        .map_or(OffsetSource::None, |l| OffsetSource::Line(l.a, l.b))
}

/// The source arc a copy arc centred at `c` of radius `r` was offset from - the same centre, the radius apart by the
/// offset - or the sharp source corner it rounds: centred on it, of the offset for a radius.
fn offset_arc_source(arcs: &[SourceArc], lines: &[SourceLine], c: (f64, f64), r: f64, dist: f64) -> OffsetSource {
    let tol = 1e-6 * (1.0 + dist.abs());
    if let Some(a) = arcs.iter().find(|a| (a.at.0 - c.0).hypot(a.at.1 - c.1) < tol && ((r - a.radius).abs() - dist.abs()).abs() < tol) {
        return OffsetSource::Arc { center: a.center, a: a.a, b: a.b };
    }
    lines
        .iter()
        .flat_map(|l| [(l.a, l.pa), (l.b, l.pb)])
        .find(|(_, p)| (p.0 - c.0).hypot(p.1 - c.1) < tol && (r - dist.abs()).abs() < tol)
        .map_or(OffsetSource::None, |(id, _)| OffsetSource::Corner(id))
}

/// THE CONSTRAINTS HOLDING AN OFFSET LOOP TO ITS SOURCE, one for each freedom of the copy and no more - a constraint
/// more is redundant, and one that also holds the source takes its freedoms away.
///
/// An arc: concentric with its source arc and its radius apart by the offset, or, rounding a sharp corner, centred on
/// that corner with the offset for a radius. A corner of the copy where two lines meet: as far from each source line
/// as the offset. A corner where a line or an arc meets an arc: on the ray from the source arc's centre through the
/// source point they met at (the arc keeps the copy on its circle by itself); where a line meets the arc rounding a
/// corner: square to the source line from the corner. A line between such corners runs parallel to its source by
/// itself. Tangency is why the corners are held by rays and squares rather than by "on the line": a point on a line
/// and on a circle the line touches is one condition, not two, and the second was taken from the source - measured,
/// a rounded rectangle's copy took 2 of its source's 4 freedoms.
///
/// A loop with a segment whose source is not found - an offset that dropped or merged segments - is left free: nothing.
fn hold_offset(ids: &[Id], kinds: &[(EntityKind, OffsetSource)], dist: f64, shape: OffsetShape) -> Vec<Constraint> {
    let n = kinds.len();
    if kinds.iter().any(|(_, s)| matches!(s, OffsetSource::None)) {
        return Vec::new();
    }
    let mut held = Vec::new();
    for (kind, src) in kinds {
        match (kind, src) {
            (EntityKind::Arc { center, .. }, OffsetSource::Arc { center: sc, .. }) => held.push(Constraint::Concentric { c1: *sc, c2: *center }),
            (EntityKind::Arc { center, .. }, OffsetSource::Corner(c)) => held.push(Constraint::Coincident { a: *c, b: *center }),
            _ => {}
        }
    }
    // THE RADIUS OF A COPY ARC by one of its ends: as far from the source point it was offset from as the offset. Not by
    // the difference of the two radii about one centre - a distance between two points that coincide has no slope, and
    // the rank read from it took two freedoms from the source.
    let mut sized = vec![false; n];
    let dim = |a: Id, b: Id| Constraint::Distance { a, b, d: dist.abs(), off: 0.0, expr: String::new(), driven: false, axis: 0, at: None };
    // the corners between two curves of the copy: all of a loop, the inner ones of a chain
    let joints: Vec<(usize, usize, usize)> = match shape {
        OffsetShape::Closed => (0..n).map(|k| (k, (k + n - 1) % n, k)).collect(),
        OffsetShape::Open { .. } => (1..n).map(|k| (k, k - 1, k)).collect(),
    };
    for (k, pk, nk) in joints {
        let v = ids[k];
        let (prev, next) = (kinds[pk].1, kinds[nk].1);
        match (prev, next) {
            (OffsetSource::Line(a1, b1), OffsetSource::Line(a2, b2)) => {
                held.push(Constraint::DistancePL { p: v, a: a1, b: b1, d: dist.abs(), off: 0.0, expr: String::new(), driven: false, at: None });
                held.push(Constraint::DistancePL { p: v, a: a2, b: b2, d: dist.abs(), off: 0.0, expr: String::new(), driven: false, at: None });
            }
            (OffsetSource::Arc { center, a, b, .. }, other) | (other, OffsetSource::Arc { center, a, b, .. }) => {
                let arc_at = if matches!(prev, OffsetSource::Arc { center: c, .. } if c == center) { pk } else { nk };
                // the source point the two met at: the arc's end the other source shares
                let shared = match other {
                    OffsetSource::Line(la, lb) => [a, b].into_iter().find(|e| *e == la || *e == lb),
                    OffsetSource::Arc { a: oa, b: ob, .. } => [a, b].into_iter().find(|e| *e == oa || *e == ob),
                    OffsetSource::Corner(c) => Some(c),
                    OffsetSource::None => None,
                };
                if let Some(s) = shared {
                    held.push(Constraint::PointOnLine { p: v, a: center, b: s });
                    if !sized[arc_at] {
                        sized[arc_at] = true;
                        held.push(dim(s, v));
                    }
                }
            }
            (OffsetSource::Corner(c), s) | (s, OffsetSource::Corner(c)) => {
                let arc_at = if matches!(prev, OffsetSource::Corner(_)) { pk } else { nk };
                if let OffsetSource::Line(la, lb) = s {
                    held.push(Constraint::Perpendicular { a: c, b: v, c: la, d: lb });
                    if !sized[arc_at] {
                        sized[arc_at] = true;
                        held.push(dim(c, v));
                    }
                }
            }
            _ => {}
        }
    }
    // THE FREE ENDS OF A CHAIN, each square to the end of its source it was offset from: on the line as far as the offset
    // and square to it from that end, or on the ray from the centre of the source arc through that end, the offset away
    if let OffsetShape::Open { first, last } = shape {
        // (the curve the end belongs to, the vertex it is, the source end): one curve is both the first and the last
        for (k, at, end) in [(0, 0, first), (n - 1, n, last)] {
            let (Some(EndOf { src }), Some(&v)) = (end, ids.get(at)) else { continue };
            match kinds[k].1 {
                OffsetSource::Line(la, lb) => {
                    held.push(Constraint::DistancePL { p: v, a: la, b: lb, d: dist.abs(), off: 0.0, expr: String::new(), driven: false, at: None });
                    held.push(Constraint::Perpendicular { a: src, b: v, c: la, d: lb });
                }
                OffsetSource::Arc { center, .. } => {
                    held.push(Constraint::PointOnLine { p: v, a: center, b: src });
                    if !sized[k] {
                        sized[k] = true;
                        held.push(dim(src, v));
                    }
                }
                _ => {}
            }
        }
    }
    held
}

/// Ends of imported curves closer than this along x and along y are one point, mm.
const IMPORT_WELD: f64 = 1e-4;

/// THE POINTS AN IMPORT HAS MADE: the places welded into nodes, and the point each node is.
pub(crate) struct ImportPoints {
    welded: super::tess::Welded,
    ids: Vec<Id>,
}

/// WHAT A SOLVE OF A SKETCH REBUILDS of its loops after it.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Rebuild {
    /// every loop: an edit, the release of a drag
    Whole,
    /// the loops of what the solve moved: a frame of a drag (`Project::regen_sketch_moved`)
    Moved,
}

/// THE VALUE OF A REFERENCE (DRIVEN) DIMENSION read off the geometry: `pos` - where the points stand, `crad` - the
/// radius of each circle by its centre. A dimension that drives is left as it is.
pub(super) fn measure_driven(c: &mut Constraint, pos: &std::collections::HashMap<Id, (f64, f64)>, crad: &std::collections::HashMap<Id, f64>) {
    match c {
        Constraint::Distance { a, b, d, driven: true, axis, .. } => {
            if let (Some(&(ax, ay)), Some(&(bx, by))) = (pos.get(a), pos.get(b)) {
                *d = match axis {
                    1 => (ax - bx).abs(),
                    2 => (ay - by).abs(),
                    _ => ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt(),
                };
            }
        }
        Constraint::Angle { a, b, c: cc, deg, driven: true, .. } => {
            if let (Some(&(ax, ay)), Some(&(bx, by)), Some(&(cx, cy))) = (pos.get(a), pos.get(b), pos.get(cc)) {
                let (ux, uy) = (ax - bx, ay - by);
                let (vx, vy) = (cx - bx, cy - by);
                *deg = (ux * vy - uy * vx).atan2(ux * vx + uy * vy).abs().to_degrees();
            }
        }
        Constraint::DistancePL { p, a, b, d, driven: true, .. } => {
            if let (Some(&(px, py)), Some(&(ax, ay)), Some(&(bx, by))) = (pos.get(p), pos.get(a), pos.get(b)) {
                let (dx, dy) = (bx - ax, by - ay);
                let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                *d = (dx * (py - ay) - dy * (px - ax)) / len; // Signed, so the side is preserved.
            }
        }
        Constraint::AngleLines { a, b, c, d, deg, driven: true, .. } => {
            if let (Some(&(ax, ay)), Some(&(bx, by)), Some(&(cx, cy)), Some(&(dx2, dy2))) = (pos.get(a), pos.get(b), pos.get(c), pos.get(d)) {
                let (ux, uy) = (bx - ax, by - ay);
                let (vx, vy) = (dx2 - cx, dy2 - cy);
                *deg = (ux * vy - uy * vx).atan2(ux * vx + uy * vy).abs().to_degrees();
            }
        }
        Constraint::Diameter { c, d, driven: true, diam, .. } => {
            if let Some(&r) = crad.get(c) {
                *d = if *diam { 2.0 * r } else { r };
            }
        }
        Constraint::ArcLength { c, a, b, ccw, len, driven: true, .. } => {
            if let (Some(&(cx, cy)), Some(&(ax, ay)), Some(&(bx, by))) = (pos.get(c), pos.get(a), pos.get(b)) {
                let rad = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
                let (a0, a1) = ((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx));
                let theta = if *ccw { (a1 - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - a1).rem_euclid(std::f64::consts::TAU) };
                *len = rad * theta;
            }
        }
        _ => {}
    }
}

/// THE BOX OF A CURVE `[x0, y0, x1, y1]` where its points stand (`at`), widened past the welds of the loops (1e-3 mm)
/// and by a millionth of its size: curves whose boxes so widened do not meet share no loop.
pub(super) fn entity_box(e: &SketchEntity, at: &dyn Fn(Id) -> Option<(f64, f64)>) -> [f64; 4] {
    let p = |id: Id| at(id).unwrap_or((f64::NAN, f64::NAN));
    let [x0, y0, x1, y1] = match e.kind {
        EntityKind::Line { a, b } => {
            let (pa, pb) = (p(a), p(b));
            [pa.0.min(pb.0), pa.1.min(pb.1), pa.0.max(pb.0), pa.1.max(pb.1)]
        }
        EntityKind::Circle { center, r } => {
            let c = p(center);
            [c.0 - r, c.1 - r, c.0 + r, c.1 + r]
        }
        EntityKind::Arc { center, a, .. } => {
            let (c, pa) = (p(center), p(a));
            let r = (pa.0 - c.0).hypot(pa.1 - c.1);
            [c.0 - r, c.1 - r, c.0 + r, c.1 + r]
        }
        EntityKind::Ellipse { c, ma, mi } => {
            let (pc, pa, pi) = (p(c), p(ma), p(mi));
            let r = (pa.0 - pc.0).hypot(pa.1 - pc.1).max((pi.0 - pc.0).hypot(pi.1 - pc.1));
            [pc.0 - r, pc.1 - r, pc.0 + r, pc.1 + r]
        }
    };
    let m = 1e-2 + 1e-6 * (x1 - x0).max(y1 - y0);
    [x0 - m, y0 - m, x1 + m, y1 + m]
}

/// A PIECE OF A SKETCH ENTITY: the entity and the place of the piece on it (`geom::piece_of`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PieceKey {
    src: Id,
    key: [i64; 2],
}

/// A piece met on an entity, and how far along the entity it lies.
pub(crate) struct PieceAt {
    key: [i64; 2],
    along: f64,
}

/// THE SOURCES OF THE PIECES OF A PROFILE, in their order, and the place (from 1) of every piece among them.
#[derive(Default)]
pub(crate) struct PieceSources {
    sources: Vec<crate::feature::SideSource>,
    slot: std::collections::HashMap<PieceKey, u32>,
}

/// A LOFT ENCODED with the sources of its sides: the sections as `loft_encoded` gives them, and the sources of the
/// edges of the first section.
pub struct LoftEncoded {
    pub sections: LoftSectionsEncoded,
    pub sources: Vec<crate::feature::SideSource>,
}

/// THE SECTIONS OF A LOFT, encoded: the loop blocks one after another, where each section starts in them, and the
/// placement of each section plane (3 x 4).
pub struct LoftSectionsEncoded {
    pub data: Vec<f64>,
    pub offsets: Vec<usize>,
    pub places: Vec<f64>,
}

mod entities;
mod edit;
mod trim;
mod corners;
mod offset;
mod solve;
mod contours;
mod profiles;
