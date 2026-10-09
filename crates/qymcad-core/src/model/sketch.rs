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
enum DrawnFrom {
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
    /// Add a contour and return its stable id.
    pub fn add_contour(&mut self, c: Contour) -> Id {
        let id = self.alloc_id();
        self.contours.push(id, c);
        id
    }
    /// Replace the contour geometry entirely (a fresh import), handing out new ids.
    pub fn set_contours(&mut self, cs: Vec<Contour>) {
        self.contours.clear();
        self.add_contours(cs);
    }
    /// Delete a contour by index. Its id goes with it, and references to it in operations simply stop
    /// resolving, so no re-indexing is needed.
    pub fn remove_contour(&mut self, index: usize) {
        if let Some(cid) = self.contours.remove_at(index) {
            // Detach it from the sketches.
            for s in &mut self.sketches {
                s.contour_ids.retain(|x| *x != cid);
            }
        }
    }
    /// The sketch to draw into: the first non-imported one, or a new one.
    pub fn drawing_sketch(&mut self) -> usize {
        if let Some(i) = self.sketches.iter().position(|s| s.source.is_none()) {
            return i;
        }
        let id = self.alloc_id();
        self.sketches.push(Sketch {
            id,
            name: "name-sketch".into(),
            contour_ids: Vec::new(),
            source: None,
            points: Vec::new(),
            entities: Vec::new(),
            closed: false,
            constraints: Vec::new(),
            splines: Vec::new(),
            notes: Vec::new(),
            texts: Vec::new(),
            patterns: Vec::new(),
            rects: Vec::new(),
            projections: Vec::new(),
            plane: crate::feature::SketchPlane::default(),
            origin: 0,
            axis_pts: [0, 0],
            frame: 0,
            origin_uv: None,
            left_unsolved: 0,
            drag_session: None,
            laid: None,
            checked: Default::default(),
            point_at: Default::default(),
            entity_at: Default::default(),
        });
        self.sketches.len() - 1
    }
    /// Add an ellipse as a real entity: a centre plus the endpoints of the major and minor semi-axes.
    ///
    /// `rot` is the rotation of the major axis in radians. Returns the id of the centre, which serves as the
    /// handle. The semi-axes are kept perpendicular by an implicit constraint, so the ellipse is parametric
    /// with five degrees of freedom rather than a polygon.
    pub fn add_ellipse_entity(&mut self, si: usize, c: crate::geom::Point2, rx: f64, ry: f64, rot: f64, purpose: crate::feature::Purpose) -> Id {
        let (cx, cy) = (c.x, c.y);
        let construction = purpose == crate::feature::Purpose::Construction;
        let (rx, ry) = (rx.max(0.01), ry.max(0.01));
        let (ux, uy) = (rot.cos(), rot.sin());
        let c = self.sketch_point_at(si, cx, cy, 1e-6);
        let ma = self.sketch_point_at(si, cx + rx * ux, cy + rx * uy, 1e-6); // Endpoint of the major axis.
        let mi = self.sketch_point_at(si, cx - ry * uy, cy + ry * ux, 1e-6); // Endpoint of the minor axis, perpendicular to it.
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Ellipse { c, ma, mi }, construction });
        self.regen_sketch(si);
        c
    }
    /// The major and minor semi-axes of an ellipse entity, by the id of its centre.
    pub fn ellipse_axes(&self, si: usize, center: Id) -> Option<(f64, f64)> {
        let s = self.sketches.get(si)?;
        let (ma, mi) = s.entities.iter().find_map(|e| match e.kind {
            EntityKind::Ellipse { c, ma, mi } if c == center => Some((ma, mi)),
            _ => None,
        })?;
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let ((cx, cy), (max, may), (mix, miy)) = (p(center)?, p(ma)?, p(mi)?);
        Some((((max - cx).powi(2) + (may - cy).powi(2)).sqrt(), ((mix - cx).powi(2) + (miy - cy).powi(2)).sqrt()))
    }
    /// Set the semi-axes of an ellipse entity (`rx` major, `ry` minor) by the id of its centre. The axis
    /// endpoints move along their current directions, so the rotation is preserved, and the solver keeps them
    /// perpendicular.
    pub fn set_ellipse_axes(&mut self, si: usize, center: Id, rx: f64, ry: f64) -> bool {
        let (rx, ry) = (rx.max(0.01), ry.max(0.01));
        let Some(s) = self.sketches.get(si) else { return false };
        let Some((ma, mi)) = s.entities.iter().find_map(|e| match e.kind {
            EntityKind::Ellipse { c, ma, mi } if c == center => Some((ma, mi)),
            _ => None,
        }) else {
            return false;
        };
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let (Some((cx, cy)), Some((max, may)), Some((mix, miy))) = (p(center), p(ma), p(mi)) else { return false };
        // Unit vectors of the current axes; the rotation is preserved.
        let dma = ((max - cx).powi(2) + (may - cy).powi(2)).sqrt().max(1e-9);
        let dmi = ((mix - cx).powi(2) + (miy - cy).powi(2)).sqrt().max(1e-9);
        let (uax, uay) = ((max - cx) / dma, (may - cy) / dma);
        let (ubx, uby) = ((mix - cx) / dmi, (miy - cy) / dmi);
        for (pid, (x, y)) in [(ma, (cx + rx * uax, cy + rx * uay)), (mi, (cx + ry * ubx, cy + ry * uby))] {
            if let Some(q) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
                q.x = x;
                q.y = y;
            }
        }
        self.solve_sketch(si);
        true
    }
    /// Vertices of a parametric polygon: the points bound by `PointOnCircle` to the centre of its circumscribed
    /// circle, in creation order. The handle of the polygon is the id of that centre.
    pub(super) fn polygon_vertices(&self, si: usize, center: Id) -> Vec<Id> {
        self.sketches
            .get(si)
            .map(|s| {
                s.constraints
                    .iter()
                    .filter_map(|c| match *c {
                        Constraint::PointOnCircle { p, c: cc } if cc == center => Some(p),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Set the circumscribed radius of a polygon through its driving dimension, letting the solver rescale it.
    /// `center` is the id of the circle centre. Returns `false` when the entity is not a polygon.
    pub fn set_polygon_radius(&mut self, si: usize, center: Id, r: f64) -> bool {
        let r = r.max(0.01);
        let Some(s) = self.sketches.get_mut(si) else { return false };
        let mut found = false;
        for c in s.constraints.iter_mut() {
            if let Constraint::Diameter { c: cc, d, diam, .. } = c {
                if *cc == center {
                    *d = if *diam { 2.0 * r } else { r };
                    found = true;
                }
            }
        }
        if !found {
            // Without a dimension, the circle radius is edited directly and the solver keeps the vertices on
            // it.
            for e in s.entities.iter_mut() {
                if let EntityKind::Circle { center: cc, r: er } = &mut e.kind {
                    if *cc == center {
                        *er = r;
                        found = true;
                    }
                }
            }
        }
        if found {
            self.solve_sketch(si);
        }
        found
    }
    /// Centre x, centre y and radius of the circumscribed circle of a polygon, by the id of its centre.
    pub fn polygon_circle(&self, si: usize, center: Id) -> Option<(f64, f64, f64)> {
        let s = self.sketches.get(si)?;
        let (cx, cy) = s.points.iter().find(|q| q.id == center).map(|q| (q.x, q.y))?;
        let r = s.entities.iter().find_map(|e| match e.kind {
            EntityKind::Circle { center: cc, r } if cc == center => Some(r),
            _ => None,
        })?;
        Some((cx, cy, r))
    }
    /// Rotation angle of a polygon in radians: the direction from the centre to the first vertex.
    pub fn polygon_angle(&self, si: usize, center: Id) -> Option<f64> {
        let s = self.sketches.get(si)?;
        let (cx, cy) = s.points.iter().find(|q| q.id == center).map(|q| (q.x, q.y))?;
        let v0 = *self.polygon_vertices(si, center).first()?;
        let (vx, vy) = s.points.iter().find(|q| q.id == v0).map(|q| (q.x, q.y))?;
        Some((vy - cy).atan2(vx - cx))
    }
    /// Rotate a polygon to an absolute angle `a` in radians: the vertices move around the centre at a constant
    /// radius and the solver keeps the shape regular. The rotation is a free degree of freedom, not a
    /// dimension.
    pub fn set_polygon_angle(&mut self, si: usize, center: Id, a: f64) -> bool {
        let verts = self.polygon_vertices(si, center);
        let Some((cx, cy, r)) = self.polygon_circle(si, center) else { return false };
        if verts.is_empty() {
            return false;
        }
        let cur = {
            let Some(q) = self.sketches[si].points.iter().find(|q| q.id == verts[0]) else { return false };
            (q.y - cy).atan2(q.x - cx)
        };
        let delta = a - cur;
        for &vid in &verts {
            if let Some(q) = self.sketches[si].points.iter_mut().find(|q| q.id == vid) {
                let ang = (q.y - cy).atan2(q.x - cx) + delta;
                q.x = cx + r * ang.cos();
                q.y = cy + r * ang.sin();
            }
        }
        self.solve_sketch(si);
        true
    }
    /// Create a typed sketch from a polyline (a chain of lines through points). The tessellation goes into a
    /// contour with a stable id, which the toolpath side references.
    pub fn add_line_sketch(&mut self, name: impl Into<String>, pts: Vec<Point2>, closed: bool) -> Id {
        let points: Vec<SketchPoint> = pts.iter().map(|p| SketchPoint { id: self.alloc_id(), x: p.x, y: p.y }).collect();
        let mut entities = Vec::new();
        let n = points.len();
        let last = if closed { n } else { n.saturating_sub(1) };
        for k in 0..last {
            let a = points[k].id;
            let b = points[(k + 1) % n].id;
            let id = self.alloc_id();
            entities.push(SketchEntity { id, kind: EntityKind::Line { a, b }, construction: false });
        }
        let sid = self.alloc_id();
        let si = self.sketches.len();
        self.sketches.push(Sketch {
            id: sid,
            name: name.into(),
            contour_ids: Vec::new(),
            source: None,
            points,
            entities,
            closed,
            constraints: Vec::new(),
            splines: Vec::new(),
            notes: Vec::new(),
            texts: Vec::new(),
            patterns: Vec::new(),
            rects: Vec::new(),
            projections: Vec::new(),
            plane: crate::feature::SketchPlane::default(),
            origin: 0,
            axis_pts: [0, 0],
            frame: 0,
            origin_uv: None,
            left_unsolved: 0,
            drag_session: None,
            laid: None,
            checked: Default::default(),
            point_at: Default::default(),
            entity_at: Default::default(),
        });
        self.regen_sketch(si);
        sid
    }
    /// Create an empty sketch (to enter edit mode) and return its index.
    pub fn new_sketch(&mut self, name: impl Into<String>) -> usize {
        let id = self.alloc_id();
        self.sketches.push(Sketch {
            id,
            name: name.into(),
            contour_ids: Vec::new(),
            source: None,
            points: Vec::new(),
            entities: Vec::new(),
            closed: false,
            constraints: Vec::new(),
            splines: Vec::new(),
            notes: Vec::new(),
            texts: Vec::new(),
            patterns: Vec::new(),
            rects: Vec::new(),
            projections: Vec::new(),
            plane: crate::feature::SketchPlane::default(),
            origin: 0,
            axis_pts: [0, 0],
            frame: 0,
            origin_uv: None,
            left_unsolved: 0,
            drag_session: None,
            laid: None,
            checked: Default::default(),
            point_at: Default::default(),
            entity_at: Default::default(),
        });
        self.sketches.len() - 1
    }
    /// Fill the profile of an existing sketch from a polyline (points, lines and a contour). Returns `false`
    /// when the sketch already has profile entities.
    pub fn fill_sketch_polyline(&mut self, si: usize, pts: Vec<Point2>, closed: bool) -> bool {
        if pts.len() < 2 || si >= self.sketches.len() {
            return false;
        }
        if self.sketches[si].entities.iter().any(|e| !e.construction) {
            return false; // A profile already exists and is not overwritten.
        }
        let points: Vec<SketchPoint> = pts.iter().map(|p| SketchPoint { id: self.alloc_id(), x: p.x, y: p.y }).collect();
        let mut entities = Vec::new();
        let n = points.len();
        let last = if closed { n } else { n - 1 };
        for k in 0..last {
            let id = self.alloc_id();
            entities.push(SketchEntity { id, kind: EntityKind::Line { a: points[k].id, b: points[(k + 1) % n].id }, construction: false });
        }
        let s = &mut self.sketches[si];
        s.points.extend(points);
        s.entities.extend(entities);
        s.closed = closed;
        self.regen_sketch(si);
        true
    }
    /// Degrees of freedom of a sketch from the rank of the constraint Jacobian, which accounts for redundancy.
    ///
    /// Returns the number of degrees of freedom and the number of redundant constraints. Zero and zero means
    /// fully constrained; a non-zero second value means there are redundant constraints.
    pub fn sketch_dof(&self, si: usize) -> (i32, i32) {
        let Some(s) = self.sketches.get(si) else { return (0, 0) };
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|c| !c.is_driven()).cloned().collect();
        active.extend(self.entity_intrinsics(si)); // Arcs keep their endpoints on the circle: five degrees of
                                                   // freedom for a free arc.
        crate::solver::dof(&s.points, &self.entity_radii(si), &active)
    }
    /// Point ids referenced by constraint `ci` of a sketch, used to highlight geometry on hover.
    pub fn sketch_constraint_points(&self, si: usize, ci: usize) -> Vec<Id> {
        self.sketches.get(si).and_then(|s| s.constraints.get(ci)).map(constraint_point_ids).unwrap_or_default()
    }
    /// Whether dimension `ci` is redundant: adding it does not raise the rank of the Jacobian, so the geometry
    /// is already determined by other constraints. Distance and angle dimensions only.
    pub fn dim_redundant(&self, si: usize, ci: usize) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        if !matches!(
            s.constraints.get(ci),
            Some(Constraint::Distance { .. })
                | Some(Constraint::Angle { .. })
                | Some(Constraint::DistancePL { .. })
                | Some(Constraint::AngleLines { .. })
                | Some(Constraint::ArcLength { .. })
                | Some(Constraint::Diameter { .. })
                | Some(Constraint::EdgeDistance { .. })
        ) {
            return false;
        }
        let intr = self.entity_intrinsics(si);
        let mut without: Vec<Constraint> = s.constraints.iter().enumerate().filter(|(i, c)| *i != ci && !c.is_driven()).map(|(_, c)| c.clone()).collect();
        without.extend(intr.iter().cloned());
        let mut with = without;
        with.push(s.constraints[ci].clone());
        // The rank did not grow, so the dimension constrains nothing.
        crate::solver::freedom_taken(&s.points, &self.entity_radii(si), &with, with.len() - 1) == 0
    }
    /// Redundant constraints: non-reference constraints whose removal frees no degree of freedom (the rank of
    /// the Jacobian does not drop), so they can be removed without losing determinacy.
    ///
    /// It names the specific redundant constraints rather than only counting them. Computed only when there is
    /// an excess to explain.
    pub fn sketch_redundant_constraints(&self, si: usize) -> Vec<usize> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        // the sketch's own constraints that constrain (not the reference ones), by their place in the sketch, then the
        // entities' own
        let at: Vec<usize> = (0..s.constraints.len()).filter(|&ci| !s.constraints[ci].is_driven()).collect();
        let mut active: Vec<Constraint> = at.iter().map(|&ci| s.constraints[ci].clone()).collect();
        active.extend(self.entity_intrinsics(si));
        crate::solver::redundant(&s.points, &self.entity_radii(si), &active, at.len()).into_iter().map(|k| at[k]).collect()
    }
    /// Add a constraint only when it is independent, that is, when it reduces the degrees of freedom by raising
    /// the rank of the Jacobian. Redundant automatic constraints (inferred while drawing) are dropped, so the
    /// sketch is not over-constrained. Returns whether the constraint was added.
    pub fn add_constraint_if_independent(&mut self, si: usize, c: Constraint) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        let intr = self.entity_intrinsics(si);
        let radii = self.entity_radii(si);
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|x| !x.is_driven()).cloned().collect();
        active.extend(intr.iter().cloned());
        let mut with = active;
        with.push(c.clone());
        // JUDGED WHERE THE CONSTRAINTS HOLD, on a copy solved with the new one: at the geometry as clicked, nearly but not
        // quite satisfying what is laid, constraints that follow from each other read as independent. A U drawn with
        // Line, its corners squared, got a Parallel between its legs on top of the two Perpendiculars that imply it, and
        // once solved all of them stood redundant. Only the part the new constraint lies in is solved and counted.
        if crate::solver::freedom_taken_where_solved(&s.points, &radii, &with, with.len() - 1) > 0 {
            self.sketches[si].constraints.push(c);
            true
        } else {
            false
        }
    }
    /// THE NEW CONSTRAINTS THAT CONSTRAIN SOMETHING, in their order: each is kept only when it raises the rank of the
    /// Jacobian over what the sketch holds and the ones kept before it. A second Vertical on a line that has one, or
    /// Vertical on a line Vertical by other constraints, constrains nothing, and laid beside the first it turned both
    /// redundant (yellow). Equal radii pass untouched: the caller makes a radius a reference for them first.
    pub fn independent_of(&self, si: usize, new: Vec<Constraint>) -> Vec<Constraint> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let radii = self.entity_radii(si);
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|x| !x.is_driven()).cloned().collect();
        active.extend(self.entity_intrinsics(si));
        let mut kept = Vec::new();
        for c in new {
            if matches!(c, Constraint::EqualRadius { .. }) {
                kept.push(c);
                continue;
            }
            active.push(c.clone());
            if crate::solver::freedom_taken(&s.points, &radii, &active, active.len() - 1) > 0 {
                kept.push(c);
            } else {
                active.pop();
            }
        }
        kept
    }
    /// LIFT THE RELATIONS A NEW ONE MADE REDUNDANT: among the constraints standing before index `had`, a relation
    /// between lines (horizontal, vertical, parallel, perpendicular, equal, collinear) whose points all lie in `among`
    /// and whose removal frees no degree of freedom is removed. Collinear on three vertical lines makes the Vertical of
    /// the second and third follow from the first; kept, they stood yellow. Returns how many went.
    pub fn drop_implied_relations(&mut self, si: usize, had: usize, among: &std::collections::HashSet<Id>) -> usize {
        let Some(s) = self.sketches.get(si) else { return 0 };
        if self.sketch_dof(si).1 <= 0 {
            return 0;
        }
        let radii = self.entity_radii(si);
        let intr = self.entity_intrinsics(si);
        let relation = |c: &Constraint| {
            matches!(
                c,
                Constraint::Horizontal { .. } | Constraint::Vertical { .. } | Constraint::Parallel { .. } | Constraint::Perpendicular { .. } | Constraint::Equal { .. } | Constraint::Collinear { .. }
            )
        };
        let mut cs = s.constraints.clone();
        let active = |cs: &[Constraint]| -> Vec<Constraint> { cs.iter().filter(|c| !c.is_driven()).cloned().chain(intr.iter().cloned()).collect() };
        let mut gone = 0;
        for ci in (0..had.min(cs.len())).rev() {
            if !relation(&cs[ci]) || !constraint_point_ids(&cs[ci]).iter().all(|p| among.contains(p)) {
                continue;
            }
            // the place of the relation among the active constraints: the reference ones before it are not there
            let at = cs[..ci].iter().filter(|c| !c.is_driven()).count();
            if crate::solver::freedom_taken(&s.points, &radii, &active(&cs), at) == 0 {
                cs.remove(ci);
                gone += 1;
            }
        }
        self.sketches[si].constraints = cs;
        gone
    }
    /// If dimension `ci` is redundant, turn it into a reference (driven) dimension. Returns whether it did.
    pub fn auto_driven(&mut self, si: usize, ci: usize) -> bool {
        if !self.dim_redundant(si, ci) {
            return false;
        }
        if let Some(
            Constraint::AngleLines { driven, .. }
            | Constraint::Distance { driven, .. }
            | Constraint::Angle { driven, .. }
            | Constraint::DistancePL { driven, .. }
            | Constraint::Diameter { driven, .. }
            | Constraint::ArcLength { driven, .. }
            | Constraint::EdgeDistance { driven, .. },
        ) = self.sketches.get_mut(si).and_then(|s| s.constraints.get_mut(ci))
        {
            *driven = true;
            return true;
        }
        false
    }
    /// Toggle dimension `ci` between driving and reference. Returns the new state.
    pub fn toggle_driven(&mut self, si: usize, ci: usize) -> bool {
        if let Some(
            Constraint::AngleLines { driven, .. }
            | Constraint::Distance { driven, .. }
            | Constraint::Angle { driven, .. }
            | Constraint::DistancePL { driven, .. }
            | Constraint::Diameter { driven, .. }
            | Constraint::ArcLength { driven, .. },
        ) = self.sketches.get_mut(si).and_then(|s| s.constraints.get_mut(ci))
        {
            *driven = !*driven;
            return *driven;
        }
        false
    }
    /// Mobility mask of the sketch points (`true` means the point can still move), one entry per point.
    /// THE CHECKS OF A SKETCH COUNTED TOGETHER (`solver::checks`): its degrees of freedom, its free points and its
    /// redundant constraints by their place, from one elimination a part - the answers of `sketch_dof`,
    /// `sketch_free_points` and `sketch_redundant_constraints`.
    pub fn sketch_checks(&self, si: usize) -> crate::solver::Checks {
        let Some(s) = self.sketches.get(si) else { return crate::solver::Checks { dof: (0, 0), free: Vec::new(), redundant: Vec::new() } };
        let at: Vec<usize> = (0..s.constraints.len()).filter(|&ci| !s.constraints[ci].is_driven()).collect();
        let mut active: Vec<Constraint> = at.iter().map(|&ci| s.constraints[ci].clone()).collect();
        active.extend(self.entity_intrinsics(si));
        let radii = self.entity_radii(si);
        let mut checks = s.checked.with(|memo| crate::solver::checks_remembered(&s.points, &radii, &active, at.len(), memo));
        checks.redundant = checks.redundant.into_iter().map(|k| at[k]).collect();
        checks
    }
    pub fn sketch_free_points(&self, si: usize) -> Vec<bool> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|c| !c.is_driven()).cloned().collect();
        active.extend(self.entity_intrinsics(si));
        crate::solver::free_points(&s.points, &self.entity_radii(si), &active)
    }
    /// Add a text note to a sketch.
    pub fn add_note(&mut self, si: usize, x: f64, y: f64, text: String) {
        if let Some(s) = self.sketches.get_mut(si) {
            s.notes.push(Note { x, y, text });
        }
    }
    /// Add parametric text geometry. `glyphs` are the glyph polylines baked by the application (in world
    /// coordinates, since the font lives there). Returns the id of the text; the contours are updated.
    pub fn add_sketch_text(&mut self, si: usize, t: TextSpec, purpose: crate::feature::Purpose) -> Id {
        let TextSpec { at, height, angle, text, glyphs, font } = t;
        let (x, y) = (at.x, at.y);
        let construction = purpose == crate::feature::Purpose::Construction;
        let id = self.alloc_id();
        if let Some(s) = self.sketches.get_mut(si) {
            s.texts.push(SketchText { id, x, y, height, angle, text, construction, glyphs, font });
        }
        self.regen_sketch(si);
        id
    }
    /// Update the parameters of a text and its baked glyphs, after the application re-baked them for a new
    /// font or string.
    pub fn set_sketch_text(&mut self, si: usize, ti: usize, t: TextSpec) {
        let TextSpec { at, height, angle, text, glyphs, font } = t;
        let (x, y) = (at.x, at.y);
        if let Some(t) = self.sketches.get_mut(si).and_then(|s| s.texts.get_mut(ti)) {
            t.x = x;
            t.y = y;
            t.height = height;
            t.angle = angle;
            t.text = text;
            t.glyphs = glyphs;
            t.font = font;
        }
        self.regen_sketch(si);
    }
    /// Move a text by (dx, dy), shifting both its parameters and its baked glyphs, so no font is needed.
    pub fn move_sketch_text(&mut self, si: usize, ti: usize, dx: f64, dy: f64) {
        if let Some(t) = self.sketches.get_mut(si).and_then(|s| s.texts.get_mut(ti)) {
            t.x += dx;
            t.y += dy;
            for loop_ in t.glyphs.iter_mut() {
                for p in loop_.iter_mut() {
                    p.x += dx;
                    p.y += dy;
                }
            }
        }
        self.regen_sketch(si);
    }
    /// TURN TEXT `ti` by `deg` about (cx, cy): its place goes round the centre, its angle grows by `deg`, and its baked
    /// glyphs turn with it - no font is needed, as for a move. Reported behaviour (issue #32): the Rotate tool did not
    /// take a text, and a text stood level always.
    pub fn rotate_sketch_text(&mut self, si: usize, ti: usize, cx: f64, cy: f64, deg: f64) {
        let (sn, cs) = deg.to_radians().sin_cos();
        let turn = |x: f64, y: f64| (cx + (x - cx) * cs - (y - cy) * sn, cy + (x - cx) * sn + (y - cy) * cs);
        if let Some(t) = self.sketches.get_mut(si).and_then(|s| s.texts.get_mut(ti)) {
            (t.x, t.y) = turn(t.x, t.y);
            t.angle += deg;
            for p in t.glyphs.iter_mut().flatten() {
                (p.x, p.y) = turn(p.x, p.y);
            }
        }
        self.regen_sketch(si);
    }
    /// COPY TEXT `ti` shifted by (dx, dy): the same string, height, angle and font, the glyphs shifted with it. Returns
    /// the id of the copy.
    pub fn copy_sketch_text(&mut self, si: usize, ti: usize, dx: f64, dy: f64) -> Option<Id> {
        let mut copy = self.sketches.get(si)?.texts.get(ti)?.clone();
        copy.id = self.alloc_id();
        copy.x += dx;
        copy.y += dy;
        for p in copy.glyphs.iter_mut().flatten() {
            p.x += dx;
            p.y += dy;
        }
        let id = copy.id;
        self.sketches[si].texts.push(copy);
        self.regen_sketch(si);
        Some(id)
    }
    /// Delete text `ti` from a sketch.
    pub fn delete_sketch_text(&mut self, si: usize, ti: usize) {
        if let Some(s) = self.sketches.get_mut(si) {
            if ti < s.texts.len() {
                s.texts.remove(ti);
            }
        }
        self.regen_sketch(si);
    }
    /// Bounding box of a text (min_x, min_y, max_x, max_y) from its baked glyphs.
    pub fn sketch_text_bbox(&self, si: usize, ti: usize) -> Option<(f64, f64, f64, f64)> {
        let t = self.sketches.get(si)?.texts.get(ti)?;
        let mut bb: Option<(f64, f64, f64, f64)> = None;
        for loop_ in &t.glyphs {
            for p in loop_ {
                bb = Some(match bb {
                    None => (p.x, p.y, p.x, p.y),
                    Some((a, b, c, d)) => (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
                });
            }
        }
        // Fallback bounding box when the glyphs are empty (a space, or a non-printing character).
        bb.or(Some((t.x, t.y, t.x + t.height * 0.5, t.y + t.height)))
    }
    /// Add a spline (a smooth Catmull-Rom curve) through control points.
    pub fn add_spline(&mut self, si: usize, pts: Vec<Point2>, ends: crate::feature::Ends, purpose: crate::feature::Purpose) {
        let construction = purpose == crate::feature::Purpose::Construction;
        let closed = ends == crate::feature::Ends::Closed;
        if pts.len() < 2 || si >= self.sketches.len() {
            return;
        }
        let ids: Vec<Id> = pts.iter().map(|_| self.alloc_id()).collect();
        if let Some(s) = self.sketches.get_mut(si) {
            for (id, p) in ids.iter().zip(&pts) {
                s.points.push(SketchPoint { id: *id, x: p.x, y: p.y });
            }
            let nt = ids.len();
            s.splines.push(Spline { points: ids, tangents: vec![None; nt], closed, construction });
        }
        self.regen_sketch(si);
    }
    /// Polyline of a spline (a cubic Hermite tessellation) in world coordinates, for drawing — including the
    /// dashed rendering of a construction spline, which never reaches the contours.
    pub fn spline_polyline(&self, si: usize, spi: usize) -> Vec<Point2> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let Some(sp) = s.splines.get(spi) else { return Vec::new() };
        let cps: Vec<Point2> = sp.points.iter().filter_map(|id| s.points.iter().find(|p| p.id == *id).map(|p| crate::geom::Point2::new(p.x, p.y))).collect();
        if cps.len() < 2 {
            return Vec::new();
        }
        tessellate_spline_hermite(&cps, &sp.tangents, sp.closed).points
    }
    /// Tangent handles of spline `spi`: for each node, its position and the end of its handle, in world
    /// coordinates. A handle is the node plus its tangent, explicit or automatic. Used for drawing and
    /// dragging.
    pub fn spline_handles(&self, si: usize, spi: usize) -> Vec<(Point2, Point2)> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let Some(sp) = s.splines.get(spi) else { return Vec::new() };
        let pos = |id: Id| s.points.iter().find(|p| p.id == id).map(|p| crate::geom::Point2::new(p.x, p.y));
        let cps: Vec<Point2> = sp.points.iter().filter_map(|id| pos(*id)).collect();
        let n = cps.len();
        if n < 2 {
            return Vec::new();
        }
        (0..n)
            .map(|i| {
                let m = spline_tangent_at(&cps, &sp.tangents, i, sp.closed);
                (cps[i], crate::geom::Point2::new(cps[i].x + m.x, cps[i].y + m.y))
            })
            .collect()
    }
    /// Set the tangent of node `ki` of spline `spi` from a dragged handle in world coordinates. This makes the
    /// tangent explicit, pinning the shape at that node, and rebuilds the contour. Returns `false` when there is
    /// no such node.
    pub fn set_spline_handle(&mut self, si: usize, spi: usize, ki: usize, hx: f64, hy: f64) -> bool {
        let Some(s) = self.sketches.get_mut(si) else { return false };
        let Some(sp) = s.splines.get(spi) else { return false };
        let Some(&kid) = sp.points.get(ki) else { return false };
        let Some((kx, ky)) = s.points.iter().find(|p| p.id == kid).map(|p| (p.x, p.y)) else { return false };
        let sp = &mut s.splines[spi];
        if sp.tangents.len() != sp.points.len() {
            sp.tangents = vec![None; sp.points.len()];
        }
        sp.tangents[ki] = Some([hx - kx, hy - ky]);
        self.regen_sketch(si);
        true
    }
    /// Reset the tangent of a node to automatic (Catmull-Rom), so the handle follows the nodes again.
    pub fn reset_spline_handle(&mut self, si: usize, spi: usize, ki: usize) -> bool {
        let Some(s) = self.sketches.get_mut(si) else { return false };
        let Some(sp) = s.splines.get_mut(spi) else { return false };
        if ki >= sp.points.len() {
            return false;
        }
        if sp.tangents.len() != sp.points.len() {
            sp.tangents = vec![None; sp.points.len()];
        }
        sp.tangents[ki] = None;
        self.regen_sketch(si);
        true
    }
    /// Add a construction polyline (a projection of part geometry into a sketch). Every segment is construction
    /// geometry: a support for snapping and construction, never a profile.
    pub fn add_construction_polyline(&mut self, si: usize, pts: &[Point2], closed: bool) {
        if pts.len() < 2 {
            return;
        }
        let pids: Vec<Id> = pts.iter().map(|_| self.alloc_id()).collect();
        let n = pids.len();
        let last = if closed { n } else { n - 1 };
        let eids: Vec<Id> = (0..last).map(|_| self.alloc_id()).collect();
        if let Some(s) = self.sketches.get_mut(si) {
            for (id, p) in pids.iter().zip(pts) {
                s.points.push(SketchPoint { id: *id, x: p.x, y: p.y });
            }
            for (k, eid) in eids.into_iter().enumerate() {
                s.entities.push(SketchEntity { id: eid, kind: EntityKind::Line { a: pids[k], b: pids[(k + 1) % n] }, construction: true });
            }
        }
    }
    /// Unique points referenced by the given entities, centres included.
    pub fn entity_point_ids(&self, si: usize, eids: &[Id]) -> Vec<Id> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let mut out: Vec<Id> = Vec::new();
        for e in &s.entities {
            if !eids.contains(&e.id) {
                continue;
            }
            let ids: Vec<Id> = match e.kind {
                EntityKind::Line { a, b } => vec![a, b],
                EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
                EntityKind::Circle { center, .. } => vec![center],
                EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
            };
            for id in ids {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }
    /// Centroid of the selected entities (the mean of their points).
    pub fn entities_centroid(&self, si: usize, eids: &[Id]) -> (f64, f64) {
        let pts = self.movable_of(si, eids);
        let Some(s) = self.sketches.get(si) else { return (0.0, 0.0) };
        let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
        for p in &s.points {
            if pts.contains(&p.id) {
                sx += p.x;
                sy += p.y;
                n += 1.0;
            }
        }
        if n > 0.0 {
            (sx / n, sy / n)
        } else {
            (0.0, 0.0)
        }
    }
    /// Prune orphaned sketch points (used by no entity and no spline) together with the constraints on them.
    ///
    /// Without this, trimming leaves dangling points behind that have to be deleted by hand. Protected: the
    /// system points (origin and axes), materialised midpoints, virtual fillet corners (points held by
    /// `PointOnLine` or `PointOnCircle`, which dimensions are measured to) and any point carrying a
    /// dimension.
    pub fn prune_orphan_sketch_points(&mut self, si: usize) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        let mut used: std::collections::HashSet<Id> = s
            .entities
            .iter()
            .flat_map(|e| match e.kind {
                EntityKind::Line { a, b } => vec![a, b],
                EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
                EntityKind::Circle { center, .. } => vec![center],
                EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
            })
            .collect();
        for sp in &s.splines {
            used.extend(sp.points.iter().copied());
        }
        let mut protected: std::collections::HashSet<Id> = s.system_ids().into_iter().collect();
        for c in &s.constraints {
            let keep = match c {
                Constraint::Midpoint { p, .. } => Some(vec![*p]),
                // A virtual fillet corner is held by its supports and dimensions are measured to it.
                Constraint::PointOnLine { p, .. } | Constraint::PointOnCircle { p, .. } => Some(vec![*p]),
                _ if c.dim_value().is_some() => Some(constraint_point_ids(c)),
                _ => None,
            };
            if let Some(ids) = keep {
                protected.extend(ids);
            }
        }
        s.points.retain(|p| used.contains(&p.id) || protected.contains(&p.id));
        let alive: std::collections::HashSet<Id> = s.points.iter().map(|p| p.id).collect();
        s.constraints.retain(|c| constraint_point_ids(c).iter().all(|id| alive.contains(id)));
    }
    /// Delete the selected entities, together with the points they orphan, the constraints on those points,
    /// and the constraints that hung on the deleted lines by their pair of endpoints — even when those
    /// endpoints are shared and still alive.
    pub fn delete_entities(&mut self, si: usize, eids: &[Id]) {
        // A RECTANGLE LOSING A SIDE IS BROKEN: its diagonals go with the side, its centre with them, and the constraints
        // it held itself by - four plain lines are left, with nothing of the rectangle on them.
        let mut eids = eids.to_vec();
        let before: Vec<crate::model::SketchRect> = self.sketches.get(si).map(|s| s.rects.clone()).unwrap_or_default();
        if let Some(s) = self.sketches.get_mut(si) {
            let broken: Vec<crate::model::SketchRect> = s.rects.iter().filter(|r| r.sides.iter().chain(r.diagonals.iter().flatten()).any(|e| eids.contains(e))).cloned().collect();
            s.rects.retain(|r| !broken.contains(r));
            for r in &broken {
                eids.extend(r.diagonals.into_iter().flatten());
                let [c0, c1, c2, c3] = r.corners;
                let own = rect_own_constraints(c0, c1, c2, c3, r.centre, 0.0);
                s.constraints.retain(|c| !own.iter().any(|o| same_rect_constraint(o, c)));
            }
        }
        let eids = &eids[..];
        {
            let Some(s) = self.sketches.get_mut(si) else { return };
            // Endpoint pairs of the lines being deleted: constraints referencing those lines go as well.
            let dead_lines: Vec<(Id, Id)> = s
                .entities
                .iter()
                .filter(|e| eids.contains(&e.id))
                .filter_map(|e| match e.kind {
                    EntityKind::Line { a, b } => Some((a, b)),
                    _ => None,
                })
                .collect();
            s.entities.retain(|e| !eids.contains(&e.id));
            // Drop the constraints attached to a deleted line (horizontal, vertical, parallel, perpendicular,
            // equal, collinear, tangent, midpoint — those referencing its pair of endpoints), or dangling
            // glyphs remain.
            if !dead_lines.is_empty() {
                s.constraints.retain(|c| !constraint_uses_line(c, &dead_lines));
            }
            let used: std::collections::HashSet<Id> = s
                .entities
                .iter()
                .flat_map(|e| match e.kind {
                    EntityKind::Line { a, b } => vec![a, b],
                    EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
                    EntityKind::Circle { center, .. } => vec![center],
                    EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
                })
                .collect();
            // Protected free points: the system ones (origin and axes) and materialised midpoints. They must
            // not be dropped as orphaned endpoints, or the axes and the dimensions measured from them are
            // lost.
            let protected: std::collections::HashSet<Id> =
                s.system_ids().into_iter().chain(s.constraints.iter().filter_map(|c| if let Constraint::Midpoint { p, .. } = c { Some(*p) } else { None })).collect();
            s.points.retain(|p| used.contains(&p.id) || protected.contains(&p.id));
            let alive: std::collections::HashSet<Id> = s.points.iter().map(|p| p.id).collect();
            s.constraints.retain(|c| constraint_point_ids(c).iter().all(|id| alive.contains(id)));
        }
        self.regen_sketch(si);
        // the sides a broken rectangle leaves keep what they plainly have, as they do in a sketch of plain lines
        self.relate_lines_left(si, &before);
    }
    /// Toggle the selected entities between ordinary and construction geometry. Construction geometry never
    /// reaches a profile. Returns the new state.
    pub fn toggle_construction(&mut self, si: usize, eids: &[Id]) -> bool {
        let mut now_construction = false;
        if let Some(s) = self.sketches.get_mut(si) {
            // The target state is the inverse of the first selected entity.
            let target = !s.entities.iter().find(|e| eids.contains(&e.id)).is_some_and(|e| e.construction);
            for e in s.entities.iter_mut() {
                if eids.contains(&e.id) {
                    e.construction = target;
                }
            }
            now_construction = target;
        }
        self.regen_sketch(si);
        now_construction
    }
    /// Delete the selected sketch points together with the entities incident to them and the constraints they
    /// orphan. Every other point, the origin included, is kept.
    pub fn delete_points(&mut self, si: usize, ids: &[Id]) {
        {
            let Some(s) = self.sketches.get_mut(si) else { return };
            let set: std::collections::HashSet<Id> = ids.iter().copied().collect();
            // Entities incident to the deleted points go as well.
            s.entities.retain(|e| {
                let inc = match e.kind {
                    EntityKind::Line { a, b } => set.contains(&a) || set.contains(&b),
                    EntityKind::Arc { center, a, b, .. } => set.contains(&center) || set.contains(&a) || set.contains(&b),
                    EntityKind::Circle { center, .. } => set.contains(&center),
                    EntityKind::Ellipse { c, ma, mi } => set.contains(&c) || set.contains(&ma) || set.contains(&mi),
                };
                !inc
            });
            s.points.retain(|p| !set.contains(&p.id));
            let alive: std::collections::HashSet<Id> = s.points.iter().map(|p| p.id).collect();
            s.constraints.retain(|c| constraint_point_ids(c).iter().all(|id| alive.contains(id)));
            s.splines.retain(|sp| sp.points.iter().all(|id| alive.contains(id)));
            if set.contains(&s.origin) {
                s.origin = 0;
            }
        }
        self.regen_sketch(si);
    }
    /// THE WIDTH AND THE HEIGHT DIMENSIONS OF THE RECTANGLE WITH SIDE `side`, laid when it has none: a dimension of a
    /// side, or of the side across from it, is its width or its height. `None` when the side is no rectangle's.
    pub fn rect_dims(&mut self, si: usize, side: Id) -> Option<crate::model::RectDims> {
        let s = self.sketches.get(si)?;
        let r = s.rects.iter().find(|r| r.sides.contains(&side))?.clone();
        let [c0, c1, c2, c3] = r.corners;
        let find =
            |pairs: [(Id, Id); 2]| s.constraints.iter().position(|c| matches!(*c, Constraint::Distance { a, b, axis: 0, .. } if pairs.iter().any(|&(x, y)| (a == x && b == y) || (a == y && b == x))));
        let (width, height) = (find([(c0, c1), (c3, c2)]), find([(c1, c2), (c0, c3)]));
        match (width, height) {
            (Some(width), Some(height)) => Some(crate::model::RectDims { width, height }),
            _ => {
                // the one there is goes, and both are laid afresh beside each other
                let sk = self.sketches.get_mut(si)?;
                if let Some(i) = width.or(height) {
                    sk.constraints.remove(i);
                }
                self.dimension_rect(si, side);
                let n = self.sketches[si].constraints.len();
                Some(crate::model::RectDims { width: n - 2, height: n - 1 })
            }
        }
    }
    /// THE WIDTH AND THE HEIGHT OF THE RECTANGLE WITH SIDE `side` LAID AS DIMENSIONS, at the lengths its first two sides
    /// stand at, each led out a fifth of the shorter side beyond its side (a positive offset is to the right of a side
    /// run from its first corner, and the corners go round anticlockwise - the dimensions stand outside). Returns
    /// whether the rectangle was found.
    pub fn dimension_rect(&mut self, si: usize, side: Id) -> bool {
        let Some(s) = self.sketches.get_mut(si) else { return false };
        let Some(r) = s.rects.iter().find(|r| r.sides.contains(&side)).cloned() else { return false };
        let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let (Some(p0), Some(p1), Some(p2)) = (at(r.corners[0]), at(r.corners[1]), at(r.corners[2])) else { return false };
        let (w, h) = ((p1.0 - p0.0).hypot(p1.1 - p0.1), (p2.0 - p1.0).hypot(p2.1 - p1.1));
        let off = 0.2 * w.min(h);
        let dim = |a: Id, b: Id, d: f64| Constraint::Distance { a, b, d, off, expr: String::new(), driven: false, axis: 0, at: None };
        s.constraints.push(dim(r.corners[0], r.corners[1], w));
        s.constraints.push(dim(r.corners[1], r.corners[2], h));
        true
    }
    /// AN ANGLE DIMENSION ON A SIDE OF A RECTANGLE TAKES ITS TURN: called with the dimension before it is laid. The turns
    /// the rectangle holds itself by go, and its shape is held by parallel and square sides, so the dimension turns it -
    /// both held, the rectangle would be held twice over and the dimension refused.
    pub fn give_rect_turn_to(&mut self, si: usize, dim: &Constraint) {
        let Constraint::AngleLines { a, b, c, d, .. } = *dim else { return };
        let Some(s) = self.sketches.get_mut(si) else { return };
        let side_of = |s: &crate::model::Sketch, r: &crate::model::SketchRect, x: Id, y: Id| {
            r.sides.iter().any(|e| matches!(s.entities.iter().find(|q| q.id == *e).map(|q| q.kind), Some(EntityKind::Line { a, b }) if (a == x && b == y) || (a == y && b == x)))
        };
        let turned: Vec<crate::model::SketchRect> = s.rects.iter().filter(|r| side_of(s, r, a, b) || side_of(s, r, c, d)).cloned().collect();
        for r in turned {
            let held_by_turns = s.constraints.iter().any(|k| matches!(k, Constraint::Orientation { a, .. } if r.corners.contains(a)));
            if !held_by_turns {
                continue;
            }
            s.constraints.retain(|k| !(matches!(k, Constraint::Orientation { .. }) && is_rect_own(&r, k)));
            let [c0, c1, c2, c3] = r.corners;
            s.constraints.extend(rect_free_constraints(c0, c1, c2, c3));
            // the dimensions of its sides go round with the sides the angle is about to turn
            let along = axis_dims_of_whole_sides(s, &|id| r.corners.contains(&id));
            lay_along_their_sides(s, &along, 0.0);
        }
    }
    /// AFTER A CONSTRAINT IS DELETED, the rectangles it was part of: one of a rectangle's own constraints deleted breaks
    /// it - the record, its centre, its diagonals and the rest of its own constraints go, four plain lines are left; an
    /// angle dimension that held a rectangle's turn deleted gives the turn back to the rectangle, at the turn it stands
    /// at, so a dragged corner does not turn it.
    fn settle_rects_after(&mut self, si: usize, removed: &Constraint) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        let broken: Vec<crate::model::SketchRect> = s.rects.iter().filter(|r| is_rect_own(r, removed)).cloned().collect();
        for r in &broken {
            s.rects.retain(|k| k.id != r.id);
            s.constraints.retain(|k| !is_rect_own(r, k));
        }
        let diagonals: Vec<Id> = broken.iter().flat_map(|r| r.diagonals.into_iter().flatten()).collect();
        let free: Vec<crate::model::SketchRect> = s
            .rects
            .iter()
            .filter(|r| !s.constraints.iter().any(|k| matches!(k, Constraint::Orientation { a, .. } if r.corners.contains(a))))
            .filter(|r| !s.constraints.iter().any(|k| matches!(k, Constraint::AngleLines { a, b, c, d, .. } if [a, b, c, d].iter().filter(|p| r.corners.contains(p)).count() >= 2)))
            .cloned()
            .collect();
        for r in free {
            let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
            let (Some(p0), Some(p1)) = (at(r.corners[0]), at(r.corners[1])) else { continue };
            let deg = (p1.1 - p0.1).atan2(p1.0 - p0.0).to_degrees();
            let [c0, c1, c2, c3] = r.corners;
            s.constraints.retain(|k| !(matches!(k, Constraint::Parallel { .. } | Constraint::Perpendicular { .. }) && is_rect_own(&r, k)));
            s.constraints.extend(rect_own_constraints(c0, c1, c2, c3, r.centre, deg).into_iter().filter(|k| matches!(k, Constraint::Orientation { .. })));
        }
        let centres: Vec<Id> = broken.iter().map(|r| r.centre).collect();
        if !diagonals.is_empty() {
            self.delete_entities(si, &diagonals);
        }
        if let Some(s) = self.sketches.get_mut(si) {
            let used: std::collections::HashSet<Id> = s.constraints.iter().flat_map(constraint_point_ids).collect();
            s.points.retain(|q| !centres.contains(&q.id) || used.contains(&q.id));
        }
    }
    /// Delete constraint `ci` of a sketch. For a midpoint constraint the orphaned midpoint is pruned as well
    /// (nothing else uses it and it is not a system point), so no debris is left behind. The sketch is then
    /// re-solved.
    /// AN EDIT `op` OF CURVE `eid` THAT CUTS A PROJECTION: the projection parted one per edge (`part_projections_holding`),
    /// the edit done as on any curve, and when it is done the projection of the edge cut is let go - what is left of that
    /// edge is ordinary geometry, and its lines, with the pieces the edit made, get what they plainly have
    /// (`relate_lines`). The other edges stay projected and follow the body. An edit that did nothing leaves the edges
    /// projected. Reported behaviour: a piece of a projection trimmed, broken or deleted - what is left of it must stop
    /// being a projection at once, or the next rebuild of the body brings the piece back over it.
    fn cut_projection(&mut self, si: usize, eid: Id, op: impl FnOnce(&mut Self) -> bool) -> bool {
        if self.sketches.get(si).is_none_or(|s| !s.projections.iter().any(|p| p.entities.contains(&eid))) {
            return op(self);
        }
        let pids = self.part_projections_holding(si, &[eid]);
        let Some(s) = self.sketches.get(si) else { return false };
        let curves: Vec<Id> = s.projections.iter().filter(|p| pids.contains(&p.id)).flat_map(|p| p.entities.clone()).collect();
        let before: std::collections::HashSet<Id> = s.entities.iter().map(|e| e.id).collect();
        if !op(self) {
            return false;
        }
        let Some(s) = self.sketches.get_mut(si) else { return true };
        s.projections.retain(|p| !pids.contains(&p.id));
        let mut left: Vec<Id> = s.entities.iter().map(|e| e.id).filter(|id| curves.contains(id) || !before.contains(id)).collect();
        left.dedup();
        self.relate_lines(si, &left);
        self.solve_sketch(si);
        true
    }

    /// DELETE CURVES `eids` OF THE SKETCH, cutting what projections they belong to: parted one per edge, the projections
    /// of the edges deleted go with their curves, the other edges stay projected, and what is left of an edge cut is
    /// ordinary geometry held by what it plainly is.
    pub fn delete_sketch_curves(&mut self, si: usize, eids: &[Id]) {
        let pids = self.part_projections_holding(si, eids);
        let Some(s) = self.sketches.get(si) else { return };
        let left: Vec<Id> = s.projections.iter().filter(|p| pids.contains(&p.id)).flat_map(|p| p.entities.clone()).filter(|e| !eids.contains(e)).collect();
        if let Some(s) = self.sketches.get_mut(si) {
            s.projections.retain(|p| !pids.contains(&p.id));
        }
        self.delete_entities(si, eids);
        self.relate_lines(si, &left);
    }

    /// THE LINES A BROKEN RECTANGLE LEAVES GET WHAT THEY PLAINLY HAVE, as lines drawn by hand get it with auto-constraints
    /// on: of every rectangle of `before` the sketch no longer holds, the sides still standing are laid Horizontal or
    /// Vertical where they stand so, then Perpendicular and Parallel between them, then Equal between those of one
    /// length - each only where it constrains something (`add_constraint_if_independent`), so nothing is over-defined.
    /// Left as plain lines, a rectangle broken by deleting a side still stood level and square, held by nothing: a corner
    /// dragged pulled it out of shape. Answers how many constraints were laid.
    pub fn relate_lines_left(&mut self, si: usize, before: &[crate::model::SketchRect]) -> usize {
        let Some(s) = self.sketches.get(si) else { return 0 };
        let sides: Vec<Id> = before.iter().filter(|r| !s.rects.iter().any(|k| k.id == r.id)).flat_map(|r| r.sides).collect();
        self.relate_lines(si, &sides)
    }

    /// THE LINES `eids` GET WHAT THEY PLAINLY HAVE, as lines drawn by hand get it with auto-constraints on: Horizontal or
    /// Vertical where they stand so, then Perpendicular and Parallel between them, then Equal between those of one length -
    /// each only where it constrains something (`add_constraint_if_independent`). For the sides a broken rectangle leaves,
    /// and for what a cut projection leaves. Answers how many constraints were laid.
    pub fn relate_lines(&mut self, si: usize, eids: &[Id]) -> usize {
        let Some(s) = self.sketches.get(si) else { return 0 };
        let lines: Vec<(Id, Id)> = eids
            .iter()
            .filter_map(|side| match s.entities.iter().find(|e| e.id == *side)?.kind {
                EntityKind::Line { a, b } => Some((a, b)),
                _ => None,
            })
            .collect();
        let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let dirs: Vec<Option<(f64, f64, f64)>> = lines
            .iter()
            .map(|&(a, b)| {
                let ((ax, ay), (bx, by)) = (at(a)?, at(b)?);
                let len = (bx - ax).hypot(by - ay);
                (len > 1e-9).then(|| ((bx - ax) / len, (by - ay) / len, len))
            })
            .collect();
        // a relation is read off geometry the rectangle held exactly, so the tolerance is the solver's, not the hand's
        const TOL: f64 = 1e-6;
        let mut wanted: Vec<Constraint> = Vec::new();
        for (&(a, b), d) in lines.iter().zip(&dirs) {
            let Some((ux, uy, _)) = *d else { continue };
            if uy.abs() < TOL {
                wanted.push(Constraint::Horizontal { a, b });
            } else if ux.abs() < TOL {
                wanted.push(Constraint::Vertical { a, b });
            }
        }
        for i in 0..lines.len() {
            for j in i + 1..lines.len() {
                let (Some((ux, uy, lu)), Some((vx, vy, lv))) = (dirs[i], dirs[j]) else { continue };
                let ((a, b), (c, d)) = (lines[i], lines[j]);
                if (ux * vx + uy * vy).abs() < TOL {
                    wanted.push(Constraint::Perpendicular { a, b, c, d });
                } else if (ux * vy - uy * vx).abs() < TOL {
                    wanted.push(Constraint::Parallel { a, b, c, d });
                }
                if (lu - lv).abs() < TOL * lu.max(lv) {
                    wanted.push(Constraint::Equal { a, b, c, d });
                }
            }
        }
        let laid = wanted.into_iter().filter(|c| self.add_constraint_if_independent(si, c.clone())).count();
        if laid > 0 {
            self.solve_sketch(si);
        }
        laid
    }
    pub fn delete_sketch_constraint(&mut self, si: usize, ci: usize) -> bool {
        let before: Vec<crate::model::SketchRect> = self.sketches.get(si).map(|s| s.rects.clone()).unwrap_or_default();
        {
            let Some(s) = self.sketches.get_mut(si) else { return false };
            if ci >= s.constraints.len() {
                return false;
            }
            let removed = s.constraints.remove(ci);
            if let Constraint::Midpoint { p, .. } = &removed {
                let p = *p;
                let used_ent = s.entities.iter().any(|e| match e.kind {
                    EntityKind::Line { a, b } => a == p || b == p,
                    EntityKind::Arc { center, a, b, .. } => center == p || a == p || b == p,
                    EntityKind::Circle { center, .. } => center == p,
                    EntityKind::Ellipse { c, ma, mi } => c == p || ma == p || mi == p,
                });
                let used_con = s.constraints.iter().any(|c| constraint_point_ids(c).contains(&p));
                let used_sp = s.splines.iter().any(|sp| sp.points.contains(&p));
                if !used_ent && !used_con && !used_sp && !s.system_ids().contains(&p) {
                    s.points.retain(|q| q.id != p); // The orphaned midpoint is removed.
                }
            }
            self.settle_rects_after(si, &removed);
        }
        self.solve_sketch(si);
        // a rectangle broken by losing one of its own constraints leaves its sides what they plainly have
        self.relate_lines_left(si, &before);
        true
    }
    /// The points of `eids` an editing tool is allowed to shift: everything except what the sketch holds
    /// (the frame, the driven projections, the pinned points). See `Sketch::held_points`.
    fn movable_of(&self, si: usize, eids: &[Id]) -> Vec<Id> {
        let held = self.sketches.get(si).map(|s| s.held_points()).unwrap_or_default();
        let mut pts = self.entity_point_ids(si, eids);
        // A RECTANGLE TAKEN BY ALL ITS SIDES MOVES AS ONE SHAPE, with what its corners became: rounded or cut, a corner
        // is a virtual sharp no side ends at, and the arc or the cut between two sides is a piece of the same contour.
        // Left behind, the sharps and the arcs held the sides by their tangencies and dimensions, and Rotate of a
        // rectangle 45 x 35 rounded R3 all round was refused as held.
        let taken: Vec<crate::model::SketchRect> = self.sketches.get(si).map(|s| s.rects.iter().filter(|r| r.sides.iter().all(|e| eids.contains(e))).cloned().collect()).unwrap_or_default();
        for r in &taken {
            pts.extend(r.corners);
            let contour: Vec<Id> = self.connected_entities(si, r.sides[0]).into_iter().collect();
            pts.extend(self.entity_point_ids(si, &contour));
        }
        // A RECTANGLE MOVES AS ONE SHAPE: with all its corners its centre goes too. Left behind, the centre held the
        // middle of the diagonal back, and a rectangle moved by 5 came out moved by 4.
        pts.extend(self.whole_rects(si, &pts).iter().map(|r| r.centre));
        pts.sort_unstable();
        pts.dedup();
        pts.into_iter().filter(|id| !held.contains(id)).collect()
    }
    /// The rectangles of sketch `si` whose four corners are all among `pts`.
    fn whole_rects(&self, si: usize, pts: &[Id]) -> Vec<crate::model::SketchRect> {
        self.sketches.get(si).map(|s| s.rects.iter().filter(|r| r.corners.iter().all(|c| pts.contains(c))).cloned().collect()).unwrap_or_default()
    }

    /// THE EDIT ENDS AT THE SOLVER, not at a rebuild. `regen_sketch` recomputes the contours from whatever
    /// the coordinates now are; it does not check that the constraints still hold. An edit that stops there
    /// leaves a drawing standing where nothing allows it to stand, and the lie holds until the next edit
    /// puts it back.
    ///
    /// Reported behaviour: "I moved the circle far away with the move tool and the coincidence with the
    /// origin stayed - and stayed GREEN. Reopen the project and the circle is back at the centre." That was
    /// repaired at the call site in the interface, which left the method itself lying to the next caller;
    /// the rule lives here now.
    /// Move the selected entities by a vector.
    pub fn move_entities(&mut self, si: usize, eids: &[Id], dx: f64, dy: f64) {
        let pts = self.movable_of(si, eids);
        if let Some(s) = self.sketches.get_mut(si) {
            for p in s.points.iter_mut() {
                if pts.contains(&p.id) {
                    p.x += dx;
                    p.y += dy;
                }
            }
        }
        self.solve_sketch(si); // regenerates as well, on the positions the constraints allow
    }
    /// THE EDIT ENDS AT THE SOLVER, not at a rebuild. `regen_sketch` recomputes the contours from whatever
    /// the coordinates now are; it does not check that the constraints still hold. An edit that stops there
    /// leaves a drawing standing where nothing allows it to stand, and the lie holds until the next edit
    /// puts it back.
    ///
    /// Reported behaviour: "I moved the circle far away with the move tool and the coincidence with the
    /// origin stayed - and stayed GREEN. Reopen the project and the circle is back at the centre." That was
    /// repaired at the call site in the interface, which left the method itself lying to the next caller;
    /// the rule lives here now.
    /// Rotate the selected entities about (cx, cy) by an angle in degrees.
    pub fn rotate_entities(&mut self, si: usize, eids: &[Id], cx: f64, cy: f64, deg: f64) {
        let (sn, cs) = (deg.to_radians().sin(), deg.to_radians().cos());
        let pts = self.movable_of(si, eids);
        // THE ROTATE TOOL TURNS A RECTANGLE: the turn it holds itself by goes round with it. Kept, it pulled the
        // rectangle back to where it stood. It is the turn of every side whose two points go round - on the corners, or
        // on the points of touching a rounded corner carried it to, where a turn kept at 0 deg pulled a rectangle 45 x 35
        // rounded R3 back to the axes and flipped its arcs outward.
        if let Some(s) = self.sketches.get_mut(si) {
            for c in s.constraints.iter_mut() {
                if let Constraint::Orientation { a, b, deg: held } = c {
                    if pts.contains(a) && pts.contains(b) {
                        *held += deg;
                    }
                }
            }
            // read before the points go round: which dimensions measure a whole side along its axis
            let along = axis_dims_of_whole_sides(s, &|id| pts.contains(&id));
            for p in s.points.iter_mut() {
                if pts.contains(&p.id) {
                    let (x, y) = (p.x - cx, p.y - cy);
                    p.x = cx + x * cs - y * sn;
                    p.y = cy + x * sn + y * cs;
                }
            }
            lay_along_their_sides(s, &along, deg);
        }
        self.solve_sketch(si); // regenerates as well, on the positions the constraints allow
    }
    /// THE EDIT ENDS AT THE SOLVER, not at a rebuild. `regen_sketch` recomputes the contours from whatever
    /// the coordinates now are; it does not check that the constraints still hold. An edit that stops there
    /// leaves a drawing standing where nothing allows it to stand, and the lie holds until the next edit
    /// puts it back.
    ///
    /// Reported behaviour: "I moved the circle far away with the move tool and the coincidence with the
    /// origin stayed - and stayed GREEN. Reopen the project and the circle is back at the centre." That was
    /// repaired at the call site in the interface, which left the method itself lying to the next caller;
    /// the rule lives here now.
    /// Scale the selected entities about (cx, cy).
    pub fn scale_entities(&mut self, si: usize, eids: &[Id], cx: f64, cy: f64, f: f64) {
        let f = if f.abs() < 1e-6 { 1.0 } else { f };
        let pts = self.movable_of(si, eids);
        if let Some(s) = self.sketches.get_mut(si) {
            for p in s.points.iter_mut() {
                if pts.contains(&p.id) {
                    p.x = cx + (p.x - cx) * f;
                    p.y = cy + (p.y - cy) * f;
                }
            }
            for e in s.entities.iter_mut() {
                if eids.contains(&e.id) {
                    if let EntityKind::Circle { r, .. } = &mut e.kind {
                        *r *= f.abs();
                    }
                }
            }
        }
        self.solve_sketch(si); // regenerates as well, on the positions the constraints allow
    }
    /// Constraints internal to the point set `inside`: every reference that stays within the set, except
    /// `Fixed` — an absolute-position anchor is not copied, since the copy is placed elsewhere. References to
    /// the axes, the origin or outside points are filtered out automatically, their points not being in the
    /// set. The clones carry the old ids.
    pub(super) fn internal_constraints(&self, si: usize, inside: &std::collections::HashSet<Id>) -> Vec<Constraint> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        s.constraints
            .iter()
            .filter(|c| {
                if matches!(c, Constraint::Fixed { .. }) {
                    return false;
                }
                let ps = constraint_point_ids(c);
                !ps.is_empty() && ps.iter().all(|p| inside.contains(p))
            })
            .cloned()
            .collect()
    }
    /// Duplicate entities with point transform `f`. `with_constraints` selects whether the constraints and
    /// dimensions internal to the set are carried over (horizontals, verticals, edge dimensions and so on, but
    /// no `Fixed` and no references to the axes). Returns the new ids.
    pub(super) fn dup_entities<F: Fn(f64, f64) -> (f64, f64)>(&mut self, si: usize, eids: &[Id], f: F, with_constraints: bool) -> Vec<Id> {
        let new_ids = self.dup_entities_unbuilt(si, eids, f, with_constraints);
        self.regen_sketch(si);
        new_ids
    }
    /// `dup_entities` without the loops made again after it: for a pattern, which lays every copy and makes the loops
    /// once. Made again after each copy, a pattern of 200 lines across 200 others made a growing grid 199 times - 29 s,
    /// the last grid of 40 000 cells.
    pub(super) fn dup_entities_unbuilt<F: Fn(f64, f64) -> (f64, f64)>(&mut self, si: usize, eids: &[Id], f: F, with_constraints: bool) -> Vec<Id> {
        let pids = self.entity_point_ids(si, eids);
        let coords: Vec<(Id, f64, f64)> = {
            let Some(s) = self.sketches.get(si) else { return Vec::new() };
            s.points.iter().filter(|p| pids.contains(&p.id)).map(|p| (p.id, p.x, p.y)).collect()
        };
        let ents: Vec<SketchEntity> = {
            let Some(s) = self.sketches.get(si) else { return Vec::new() };
            s.entities.iter().filter(|e| eids.contains(&e.id)).copied().collect()
        };
        let mut map: std::collections::HashMap<Id, Id> = std::collections::HashMap::new();
        for (old, x, y) in coords {
            let (nx, ny) = f(x, y);
            let nid = self.alloc_id();
            self.sketches[si].points.push(SketchPoint { id: nid, x: nx, y: ny });
            map.insert(old, nid);
        }
        let m = |id: Id| map.get(&id).copied().unwrap_or(id);
        let mut new_ids = Vec::new();
        for e in ents {
            let nk = match e.kind {
                EntityKind::Line { a, b } => EntityKind::Line { a: m(a), b: m(b) },
                EntityKind::Arc { center, a, b, ccw } => EntityKind::Arc { center: m(center), a: m(a), b: m(b), ccw },
                EntityKind::Circle { center, r } => EntityKind::Circle { center: m(center), r },
                EntityKind::Ellipse { c, ma, mi } => EntityKind::Ellipse { c: m(c), ma: m(ma), mi: m(mi) },
            };
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: nk, construction: e.construction });
            new_ids.push(id);
        }
        if with_constraints {
            // The internal constraints of the set are re-pointed at the new points, so the copy keeps its
            // shape but not its anchor or its links to the axes.
            let inside: std::collections::HashSet<Id> = map.keys().copied().collect();
            let cons: Vec<Constraint> = self.internal_constraints(si, &inside).iter().map(|c| remap_constraint_via(c, &map)).collect();
            self.sketches[si].constraints.extend(cons);
        }
        new_ids
    }
    /// Capture the selected geometry of sketch `si` into the clipboard, for pasting into this sketch or
    /// another one. `eids` are the selected entities; the snapshot holds them and all their points (centres,
    /// endpoints). `(rx, ry)` is the reference point given at copy time. Does not mutate.
    pub fn copy_sketch_geometry(&self, si: usize, eids: &[Id], rx: f64, ry: f64) -> GeomClip {
        let pids = self.entity_point_ids(si, eids);
        let Some(s) = self.sketches.get(si) else { return GeomClip::default() };
        let points: Vec<(Id, f64, f64)> = s.points.iter().filter(|p| pids.contains(&p.id)).map(|p| (p.id, p.x, p.y)).collect();
        let entities: Vec<SketchEntity> = s.entities.iter().filter(|e| eids.contains(&e.id)).copied().collect();
        let inside: std::collections::HashSet<Id> = pids.iter().copied().collect();
        let constraints = self.internal_constraints(si, &inside); // Internal constraints and dimensions, without
                                                                  // `Fixed` or axis references.
        GeomClip { points, entities, constraints, ref_x: rx, ref_y: ry }
    }
    /// Paste snapshot `clip` into sketch `si` so that its reference point lands at `(tx, ty)`.
    ///
    /// Points and entities receive new ids and the references are remapped. Returns the ids of the pasted
    /// entities, for highlighting and selection. The constraints and dimensions internal to the copy are
    /// carried over, so the shape is preserved, while the anchor and any dimensions to the axes are not, their
    /// points not being in the set.
    pub fn paste_sketch_geometry(&mut self, si: usize, clip: &GeomClip, tx: f64, ty: f64) -> Vec<Id> {
        if si >= self.sketches.len() || clip.is_empty() {
            return Vec::new();
        }
        let (ox, oy) = (tx - clip.ref_x, ty - clip.ref_y);
        let mut map: std::collections::HashMap<Id, Id> = std::collections::HashMap::new();
        for (old, x, y) in &clip.points {
            let nid = self.alloc_id();
            self.sketches[si].points.push(SketchPoint { id: nid, x: x + ox, y: y + oy });
            map.insert(*old, nid);
        }
        let m = |id: Id| map.get(&id).copied().unwrap_or(id);
        let mut new_ids = Vec::new();
        for e in &clip.entities {
            let nk = match e.kind {
                EntityKind::Line { a, b } => EntityKind::Line { a: m(a), b: m(b) },
                EntityKind::Arc { center, a, b, ccw } => EntityKind::Arc { center: m(center), a: m(a), b: m(b), ccw },
                EntityKind::Circle { center, r } => EntityKind::Circle { center: m(center), r },
                EntityKind::Ellipse { c, ma, mi } => EntityKind::Ellipse { c: m(c), ma: m(ma), mi: m(mi) },
            };
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: nk, construction: e.construction });
            new_ids.push(id);
        }
        // The internal constraints and dimensions of the copy are re-pointed at the new points; every
        // reference stays within the set and is remapped through the map.
        for c in &clip.constraints {
            let nc = remap_constraint_via(c, &map);
            self.sketches[si].constraints.push(nc);
        }
        self.regen_sketch(si);
        new_ids
    }
    /// An editable pattern: duplicates the source according to its parameters and records the pattern itself.
    /// Returns the pattern id, so it can be edited again. The instances are real entities and reach profiles and
    /// toolpaths.
    pub fn add_pattern(&mut self, si: usize, source: &[Id], kind: PatternKind) -> Id {
        let instances = self.pattern_instances(si, source, kind);
        let id = self.alloc_id();
        if let Some(s) = self.sketches.get_mut(si) {
            s.patterns.push(SketchPattern { id, source: source.to_vec(), kind, instances });
        }
        self.regen_sketch(si);
        id
    }
    /// Change the parameters of pattern `pi`: remove the old instances and recreate them with the new ones.
    pub fn update_pattern(&mut self, si: usize, pi: usize, kind: PatternKind) {
        let (source, old) = match self.sketches.get(si).and_then(|s| s.patterns.get(pi)) {
            Some(p) => (p.source.clone(), p.instances.clone()),
            None => return,
        };
        self.delete_entities(si, &old); // Remove the old copies and the points they orphan.
        let instances = self.pattern_instances(si, &source, kind);
        if let Some(p) = self.sketches.get_mut(si).and_then(|s| s.patterns.get_mut(pi)) {
            p.kind = kind;
            p.instances = instances;
        }
        self.regen_sketch(si);
    }
    /// Delete pattern `pi` together with its copies; the source stays.
    pub fn delete_pattern(&mut self, si: usize, pi: usize) {
        let old = match self.sketches.get(si).and_then(|s| s.patterns.get(pi)) {
            Some(p) => p.instances.clone(),
            None => return,
        };
        self.delete_entities(si, &old);
        if let Some(s) = self.sketches.get_mut(si) {
            if pi < s.patterns.len() {
                s.patterns.remove(pi);
            }
        }
        self.regen_sketch(si);
    }
    /// Index of the pattern that instance entity `eid` belongs to, so a click can reopen it for editing.
    pub fn pattern_of_entity(&self, si: usize, eid: Id) -> Option<usize> {
        self.sketches.get(si)?.patterns.iter().position(|p| p.instances.contains(&eid))
    }
    /// Attach cut point `pid` to the entity it actually lies on — excluding the ones already holding it as an
    /// endpoint or a centre: a line gives `PointOnLine`, a circle or arc gives `PointOnCircle`.
    ///
    /// Without it the cut point is free (along the angle on an arc, along the crossing entity on a line) and
    /// drifts off the intersection on the next solve, so the trimmed geometry wanders and twists and the
    /// dimensions and radii move with it. Attaching it to the crossing entity, together with the intrinsic
    /// constraint that keeps it on its own curve, pins the cut point to the intersection.
    pub(super) fn anchor_cut_point(&mut self, si: usize, pid: Id) {
        const TOL: f64 = 5e-3;
        let Some((px, py)) = self.point_xy(si, pid) else { return };
        let Some(s) = self.sketches.get(si) else { return };
        let owns = |k: &EntityKind| match *k {
            EntityKind::Line { a, b } => a == pid || b == pid,
            EntityKind::Arc { center, a, b, .. } => center == pid || a == pid || b == pid,
            EntityKind::Circle { center, .. } => center == pid,
            EntityKind::Ellipse { c, ma, mi } => c == pid || ma == pid || mi == pid,
        };
        let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let mut anchor: Option<Constraint> = None;
        for e in &s.entities {
            if owns(&e.kind) {
                continue; // The point already belongs to this entity as an endpoint or a centre, so it is not
                          // anchored to it again.
            }
            match e.kind {
                EntityKind::Line { a, b } => {
                    if let (Some((ax, ay)), Some((bx, by))) = (pt(a), pt(b)) {
                        let (dx, dy) = (bx - ax, by - ay);
                        let len2 = dx * dx + dy * dy;
                        if len2 < 1e-12 {
                            continue;
                        }
                        let t = ((px - ax) * dx + (py - ay) * dy) / len2; // Parameter along the segment.
                        let perp = (dx * (py - ay) - dy * (px - ax)).abs() / len2.sqrt();
                        if perp < TOL && (-1e-6..=1.0 + 1e-6).contains(&t) {
                            anchor = Some(Constraint::PointOnLine { p: pid, a, b });
                            break;
                        }
                    }
                }
                EntityKind::Circle { center, r } => {
                    if let Some((cx, cy)) = pt(center) {
                        if (((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - r).abs() < TOL {
                            anchor = Some(Constraint::PointOnCircle { p: pid, c: center });
                            break;
                        }
                    }
                }
                EntityKind::Arc { center, a, b, ccw } => {
                    if let (Some((cx, cy)), Some((ax, ay)), Some((bx, by))) = (pt(center), pt(a), pt(b)) {
                        let r = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
                        let (a0, a1) = ((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx));
                        let ang = (py - cy).atan2(px - cx);
                        if (((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - r).abs() < TOL && angle_in_arc(ang, a0, a1, ccw) {
                            anchor = Some(Constraint::PointOnCircle { p: pid, c: center });
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(c) = anchor {
            let dup = self.sketches[si].constraints.iter().any(|x| match (x, &c) {
                (Constraint::PointOnLine { p: p1, a: a1, b: b1 }, Constraint::PointOnLine { p: p2, a: a2, b: b2 }) => p1 == p2 && ((a1 == a2 && b1 == b2) || (a1 == b2 && b1 == a2)),
                (Constraint::PointOnCircle { p: p1, c: c1 }, Constraint::PointOnCircle { p: p2, c: c2 }) => p1 == p2 && c1 == c2,
                _ => false,
            });
            if !dup {
                self.sketches[si].constraints.push(c);
            }
        }
    }
    /// Trim segment `eid` at the clicked point: the segment is divided by its intersections with other
    /// entities and the piece containing the click is removed.
    pub fn trim_line(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        self.cut_projection(si, eid, |p| p.trim_line_here(si, eid, clickx, clicky))
    }
    fn trim_line_here(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        let Some((a, b)) = self.line_ends(si, eid) else { return false };
        let (Some((pax, pay)), Some((pbx, pby))) = (self.point_xy(si, a), self.point_xy(si, b)) else { return false };
        let dlen2 = (pbx - pax).powi(2) + (pby - pay).powi(2);
        if dlen2 < 1e-12 {
            return false;
        }
        // Cut points from the intersections, as the parameter t along a to b.
        let ents = self.sketches[si].entities.clone();
        let mut cuts: Vec<f64> = Vec::new();
        for e in &ents {
            if e.id == eid {
                continue; // Construction geometry is a valid trim boundary.
            }
            match e.kind {
                EntityKind::Line { a: c, b: d } => {
                    if let (Some((cx, cy)), Some((dx, dy))) = (self.point_xy(si, c), self.point_xy(si, d)) {
                        if let Some(t) = seg_seg_t([pax, pay], [pbx, pby], [cx, cy], [dx, dy]) {
                            cuts.push(t);
                        }
                    }
                }
                EntityKind::Circle { center, r } => {
                    if let Some((cx, cy)) = self.point_xy(si, center) {
                        cuts.extend(seg_circle_t(pax, pay, pbx, pby, cx, cy, r));
                    }
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    if let (Some((ex, ey)), Some((mx, my)), Some((nx, ny))) = (self.point_xy(si, c), self.point_xy(si, ma), self.point_xy(si, mi)) {
                        let (ux, uy, major, minor) = ellipse_axes(ex, ey, mx, my, nx, ny);
                        cuts.extend(seg_ellipse_t([pax, pay], [pbx, pby], [ex, ey], [ux, uy], major, minor));
                    }
                }
                EntityKind::Arc { center, a: aa, b: bb, ccw } => {
                    if let (Some((cx, cy)), Some((sx, sy)), Some((ex, ey))) = (self.point_xy(si, center), self.point_xy(si, aa), self.point_xy(si, bb)) {
                        let r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                        let a0 = (sy - cy).atan2(sx - cx);
                        let a1 = (ey - cy).atan2(ex - cx);
                        for t in seg_circle_t(pax, pay, pbx, pby, cx, cy, r) {
                            let (ix, iy) = (pax + (pbx - pax) * t, pay + (pby - pay) * t);
                            if angle_in_arc((iy - cy).atan2(ix - cx), a0, a1, ccw) {
                                cuts.push(t);
                            }
                        }
                    }
                }
            }
        }
        if cuts.is_empty() {
            return self.trim_away_whole(si, eid);
        }
        cuts.push(0.0);
        cuts.push(1.0);
        cuts.sort_by(|x, y| x.total_cmp(y));
        cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
        // Parameter of the click.
        let tc = ((clickx - pax) * (pbx - pax) + (clicky - pay) * (pby - pay)) / dlen2;
        let mut keep: Vec<(f64, f64)> = Vec::new();
        let mut removed = false;
        for w in cuts.windows(2) {
            let (t0, t1) = (w[0], w[1]);
            if !removed && tc >= t0 - 1e-9 && tc <= t1 + 1e-9 {
                removed = true;
                continue;
            }
            keep.push((t0, t1));
        }
        if !removed {
            return false;
        }
        // Rebuild: delete the original segment and add the remaining pieces.
        self.sketches[si].entities.retain(|e| e.id != eid);
        let mut seg_pts: Vec<(Id, Id)> = Vec::new();
        for (t0, t1) in keep {
            let p0 = if t0 <= 1e-9 { a } else { self.sketch_point_at(si, pax + (pbx - pax) * t0, pay + (pby - pay) * t0, 1e-9) };
            let p1 = if t1 >= 1.0 - 1e-9 { b } else { self.sketch_point_at(si, pax + (pbx - pax) * t1, pay + (pby - pay) * t1, 1e-9) };
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Line { a: p0, b: p1 }, construction: false });
            seg_pts.push((p0, p1));
        }
        // The pieces of one line stay on one straight line: the interior cut points are held by `PointOnLine`
        // against the line through the outermost points, which the solver cannot collapse (the residual is
        // divided by the length). Otherwise editing a dimension spreads the pieces into a parallelogram and the
        // notch or arc flies away.
        let mut order: Vec<Id> = Vec::new();
        for &(p0, p1) in &seg_pts {
            if order.last() != Some(&p0) {
                order.push(p0);
            }
            order.push(p1);
        }
        order.dedup();
        if order.len() >= 3 {
            let (r0, r1) = (order[0], order[order.len() - 1]);
            for &pi in &order[1..order.len() - 1] {
                self.sketches[si].constraints.push(Constraint::PointOnLine { p: pi, a: r0, b: r1 });
            }
        }
        // The cut points (not the original endpoints) are anchored to the crossing curve — a circle, an arc or
        // another line — so a point stays at the intersection instead of sliding along it and the pieces do not
        // spread when a dimension is edited.
        let cut_pts: Vec<Id> = order.iter().copied().filter(|&pi| pi != a && pi != b).collect();
        for pi in cut_pts {
            self.anchor_cut_point(si, pi);
        }
        self.merge_close_points(si, 1e-3); // Stitch coincident cut points so the pieces stay connected.
        self.prune_orphan_sketch_points(si); // Leave no dangling points behind a trim.
        self.solve_sketch(si); // the edit ends at the solver, not at a rebuild: `regen_sketch` recomputes the contours and checks no constraint
        true
    }
    /// THE PLACES LINE `eid` CAN BE EXTENDED TO: where its axis meets another curve beyond each end, nearest first, each
    /// with the curve met there. `None` when `eid` is no line of the sketch or has no length. Construction geometry is a
    /// boundary as well.
    pub fn extend_stops(&self, si: usize, eid: Id) -> Option<ExtendStops> {
        let (a, b) = self.line_ends(si, eid)?;
        let ((pax, pay), (pbx, pby)) = (self.point_xy(si, a)?, self.point_xy(si, b)?);
        if (pbx - pax).powi(2) + (pby - pay).powi(2) < 1e-12 {
            return None;
        }
        let mut cand: Vec<ExtendStop> = Vec::new();
        for e in self.sketches.get(si)?.entities.iter().filter(|e| e.id != eid) {
            let mut ts: Vec<f64> = Vec::new();
            match e.kind {
                EntityKind::Line { a: c, b: d } => {
                    if let (Some((cx, cy)), Some((dx, dy))) = (self.point_xy(si, c), self.point_xy(si, d)) {
                        ts.extend(line_seg_t([pax, pay], [pbx, pby], [cx, cy], [dx, dy]));
                    }
                }
                EntityKind::Circle { center, r } => {
                    if let Some((cx, cy)) = self.point_xy(si, center) {
                        ts.extend(line_circle_t(pax, pay, pbx, pby, cx, cy, r));
                    }
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    if let (Some((ex, ey)), Some((mx, my)), Some((nx, ny))) = (self.point_xy(si, c), self.point_xy(si, ma), self.point_xy(si, mi)) {
                        let (ux, uy, major, minor) = ellipse_axes(ex, ey, mx, my, nx, ny);
                        ts.extend(line_ellipse_roots([pax, pay], [pbx, pby], [ex, ey], [ux, uy], major, minor));
                    }
                }
                EntityKind::Arc { center, a: aa, b: bb, ccw } => {
                    if let (Some((cx, cy)), Some((sx, sy)), Some((ex, ey))) = (self.point_xy(si, center), self.point_xy(si, aa), self.point_xy(si, bb)) {
                        let r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                        let (a0, a1) = ((sy - cy).atan2(sx - cx), (ey - cy).atan2(ex - cx));
                        for t in line_circle_t(pax, pay, pbx, pby, cx, cy, r) {
                            let (ix, iy) = (pax + (pbx - pax) * t, pay + (pby - pay) * t);
                            if angle_in_arc((iy - cy).atan2(ix - cx), a0, a1, ccw) {
                                ts.push(t);
                            }
                        }
                    }
                }
            }
            cand.extend(ts.into_iter().map(|t| ExtendStop { t, by: e.id }));
        }
        let mut stops = ExtendStops { before_a: cand.iter().copied().filter(|s| s.t < -1e-6).collect(), past_b: cand.into_iter().filter(|s| s.t > 1.0 + 1e-6).collect() };
        stops.before_a.sort_by(|x, y| y.t.total_cmp(&x.t));
        stops.past_b.sort_by(|x, y| x.t.total_cmp(&y.t));
        Some(stops)
    }

    /// WHERE LINE `eid` GOES, ASKED AS THE EXTEND TOOL ASKS: each end taken to the curve the pointer is over when that
    /// curve lies on its side of the axis, else to the nearest curve there; an end with nothing on its side stays. One
    /// end - the one on the side of the curve under the pointer, else the one nearer the pointer - or both.
    ///
    /// AN END JOINED TO OTHER GEOMETRY STAYS (`end_held`): moved, it pulled what it is joined to, and the solve pulled
    /// it back. Reported behaviour: Extend on the side two rectangles share, its end the end of a fillet, moved the
    /// drawing (the ends of the fillet by 5e-6 mm, enough to lose the face of the rectangle), extended nothing and said
    /// it was done. The line answers `EndKept::Joined`, and a new line is drawn from that end instead.
    pub fn line_extension(&self, si: usize, eid: Id, ask: &ExtendAsk) -> LineExtension {
        let free = self.line_extension_free(si, eid, ask);
        let Some((a, b)) = self.line_ends(si, eid) else { return free };
        let (held_a, held_b) = (free.a.is_some() && self.end_held(si, eid, a), free.b.is_some() && self.end_held(si, eid, b));
        LineExtension { a: free.a.filter(|_| !held_a), b: free.b.filter(|_| !held_b), kept: if held_a || held_b { EndKept::Joined } else { EndKept::Free } }
    }

    /// WHETHER END `p` OF LINE `eid` IS HELD BY SOMETHING BUT THE DIRECTION OF THE LINE: another curve ends there, or a
    /// constraint other than one that turns the line (horizontal, vertical, an angle, parallel, perpendicular,
    /// collinear) holds the point - a coincidence, a point on a curve, a midpoint, an anchor, a dimension.
    pub(crate) fn end_held(&self, si: usize, eid: Id, p: Id) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        if s.entities.iter().any(|e| e.id != eid && entity_points(e).contains(&p)) {
            return true;
        }
        s.constraints.iter().any(|c| {
            let turns_the_line = matches!(
                c,
                Constraint::Horizontal { .. }
                    | Constraint::Vertical { .. }
                    | Constraint::Orientation { .. }
                    | Constraint::Parallel { .. }
                    | Constraint::Perpendicular { .. }
                    | Constraint::Collinear { .. }
                    | Constraint::AngleLines { .. }
            );
            !turns_the_line && c.points().contains(&p)
        })
    }

    fn line_extension_free(&self, si: usize, eid: Id, ask: &ExtendAsk) -> LineExtension {
        let Some(stops) = self.extend_stops(si, eid) else { return LineExtension::default() };
        let to = |side: &[ExtendStop]| side.iter().find(|s| Some(s.by) == ask.over).or(side.first()).map(|s| s.t);
        let (at_a, at_b) = (to(&stops.before_a), to(&stops.past_b));
        if ask.sides == ExtendSides::Both {
            return LineExtension { a: at_a, b: at_b, ..Default::default() };
        }
        let over_side = |side: &[ExtendStop]| ask.over.is_some_and(|o| side.iter().any(|s| s.by == o));
        let past_b = if over_side(&stops.past_b) {
            true
        } else if over_side(&stops.before_a) {
            false
        } else {
            let Some((a, b)) = self.line_ends(si, eid) else { return LineExtension::default() };
            let (Some((pax, pay)), Some((pbx, pby))) = (self.point_xy(si, a), self.point_xy(si, b)) else { return LineExtension::default() };
            let tc = ((ask.pointer.x - pax) * (pbx - pax) + (ask.pointer.y - pay) * (pby - pay)) / ((pbx - pax).powi(2) + (pby - pay).powi(2));
            tc >= 0.5
        };
        if past_b {
            LineExtension { a: None, b: at_b, ..Default::default() }
        } else {
            LineExtension { a: at_a, b: None, ..Default::default() }
        }
    }

    /// EXTEND LINE `eid` AS `ext` SAYS: each end given a place along the axis goes there, and the sketch is solved.
    /// Returns whether an end moved.
    pub fn extend_line_by(&mut self, si: usize, eid: Id, ext: LineExtension) -> bool {
        self.cut_projection(si, eid, |p| p.extend_line_by_here(si, eid, ext))
    }
    fn extend_line_by_here(&mut self, si: usize, eid: Id, ext: LineExtension) -> bool {
        let Some((a, b)) = self.line_ends(si, eid) else { return false };
        let (Some((pax, pay)), Some((pbx, pby))) = (self.point_xy(si, a), self.point_xy(si, b)) else { return false };
        let mut moved = false;
        for (pid, t) in [(a, ext.a), (b, ext.b)] {
            let Some(t) = t else { continue };
            if let Some(p) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
                (p.x, p.y) = (pax + (pbx - pax) * t, pay + (pby - pay) * t);
                moved = true;
            }
        }
        if moved {
            self.solve_sketch(si); // the edit ends at the solver, not at a rebuild: `regen_sketch` recomputes the contours and checks no constraint
        }
        moved
    }

    /// Extend: the segment endpoint nearer to the click is stretched to the nearest intersection with another
    /// entity. Returns whether it was extended.
    pub fn extend_line(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        let ask = ExtendAsk { pointer: crate::geom::Point2::new(clickx, clicky), over: None, sides: ExtendSides::Nearer };
        let ext = self.line_extension(si, eid, &ask);
        self.extend_line_by(si, eid, ext)
    }
    /// Extend an arc: the endpoint nearer to the click is stretched along its own circle to the nearest
    /// intersection with another entity, within the gap outside the current span of the arc. Returns whether it
    /// was extended.
    pub fn extend_curve(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        use std::f64::consts::TAU;
        // Arc geometry; the coordinates are copied to release the borrow before calling
        // `curve_cut_angles`.
        let (center, a, b, ccw, cx, cy, ax, ay, bx, by) = {
            let Some(s) = self.sketches.get(si) else { return false };
            let Some(e) = s.entities.iter().find(|e| e.id == eid) else { return false };
            let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
            let EntityKind::Arc { center, a, b, ccw } = e.kind else { return false };
            let (Some((cx, cy)), Some((ax, ay)), Some((bx, by))) = (p(center), p(a), p(b)) else { return false };
            (center, a, b, ccw, cx, cy, ax, ay, bx, by)
        };
        let r = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
        let a0 = (ay - cy).atan2(ax - cx);
        let a1 = (by - cy).atan2(bx - cx);
        let sweep = if ccw { (a1 - a0).rem_euclid(TAU) } else { (a0 - a1).rem_euclid(TAU) };
        // Which endpoint is stretched: the one nearer to the click.
        let extend_b = ((clickx - bx).powi(2) + (clicky - by).powi(2)) < ((clickx - ax).powi(2) + (clicky - ay).powi(2));
        // Angles where the full circle meets the other entities, as a parameter along the direction of the
        // arc.
        let cuts = self.curve_cut_angles(si, eid, cx, cy, r);
        let to_param = |ang: f64| if ccw { (ang - a0).rem_euclid(TAU) } else { (a0 - ang).rem_euclid(TAU) };
        let gap: Vec<f64> = cuts.iter().map(|&ang| to_param(ang)).filter(|&pp| pp > sweep + 1e-6 && pp < TAU - 1e-6).collect();
        // Extending b takes the first cut in the gap just past endpoint b; extending a takes the last one,
        // nearer to a from the other side.
        let target = if extend_b { gap.into_iter().min_by(|x, y| x.total_cmp(y)) } else { gap.into_iter().max_by(|x, y| x.total_cmp(y)) };
        let Some(pp) = target else { return false };
        let new_ang = if ccw { a0 + pp } else { a0 - pp };
        let (nx, ny) = (cx + r * new_ang.cos(), cy + r * new_ang.sin());
        let pid = if extend_b { b } else { a };
        let _ = center;
        if let Some(p) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
            p.x = nx;
            p.y = ny;
        }
        self.solve_sketch(si); // the edit ends at the solver, not at a rebuild: `regen_sketch` recomputes the contours and checks no constraint
        true
    }
    /// Break: split a segment into two at the clicked point.
    pub fn break_line(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        self.cut_projection(si, eid, |p| p.break_line_here(si, eid, clickx, clicky))
    }
    fn break_line_here(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        let Some((a, b)) = self.line_ends(si, eid) else { return false };
        let (Some((pax, pay)), Some((pbx, pby))) = (self.point_xy(si, a), self.point_xy(si, b)) else { return false };
        let dlen2 = (pbx - pax).powi(2) + (pby - pay).powi(2);
        if dlen2 < 1e-12 {
            return false;
        }
        let tc = (((clickx - pax) * (pbx - pax) + (clicky - pay) * (pby - pay)) / dlen2).clamp(0.05, 0.95);
        let mid = self.sketch_point_at(si, pax + (pbx - pax) * tc, pay + (pby - pay) * tc, 1e-9);
        let con = self.sketches[si].entities.iter().find(|e| e.id == eid).is_some_and(|e| e.construction);
        self.sketches[si].entities.retain(|e| e.id != eid);
        for (p, q) in [(a, mid), (mid, b)] {
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Line { a: p, b: q }, construction: con });
        }
        // THE HALVES KEEP WHAT THE LINE WAS HELD BY: a horizontal or a vertical of its ends becomes one of each half. Left
        // as it stood, it held the far ends only, and the break point was free to leave the line - two constraints and
        // seven freedoms where there are three and six.
        let halves = |c: &Constraint| -> Option<[Constraint; 2]> {
            match *c {
                Constraint::Horizontal { a: x, b: y } if (x == a && y == b) || (x == b && y == a) => Some([Constraint::Horizontal { a, b: mid }, Constraint::Horizontal { a: mid, b }]),
                Constraint::Vertical { a: x, b: y } if (x == a && y == b) || (x == b && y == a) => Some([Constraint::Vertical { a, b: mid }, Constraint::Vertical { a: mid, b }]),
                _ => None,
            }
        };
        let old = std::mem::take(&mut self.sketches[si].constraints);
        for c in old {
            match halves(&c) {
                Some(two) => self.sketches[si].constraints.extend(two),
                None => self.sketches[si].constraints.push(c),
            }
        }
        self.merge_close_points(si, 1e-3); // Stitch coincident cut points so the pieces stay connected.
        self.prune_orphan_sketch_points(si); // Leave no dangling points behind a trim.
        self.solve_sketch(si); // the edit ends at the solver, not at a rebuild: `regen_sketch` recomputes the contours and checks no constraint
        true
    }
    pub(super) fn line_ends(&self, si: usize, eid: Id) -> Option<(Id, Id)> {
        let s = self.sketches.get(si)?;
        s.entity(eid).and_then(|e| match e.kind {
            EntityKind::Line { a, b } => Some((a, b)),
            _ => None,
        })
    }
    /// Angles, about centre (cx, cy) with radius r, of the intersections with the other non-construction
    /// entities of the sketch, excluding `eid_self`. These are the cut points of a circle or an arc.
    pub(super) fn curve_cut_angles(&self, si: usize, eid_self: Id, cx: f64, cy: f64, r: f64) -> Vec<f64> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for e in &s.entities {
            if e.id == eid_self {
                continue; // Construction geometry is a valid cut boundary for a curve.
            }
            match e.kind {
                EntityKind::Line { a, b } => {
                    if let (Some((ax, ay)), Some((bx, by))) = (p(a), p(b)) {
                        for t in seg_circle_t(ax, ay, bx, by, cx, cy, r) {
                            pts.push((ax + (bx - ax) * t, ay + (by - ay) * t));
                        }
                    }
                }
                EntityKind::Circle { center, r: r2 } => {
                    if let Some((c2x, c2y)) = p(center) {
                        pts.extend(circle_circle_pts(cx, cy, r, c2x, c2y, r2));
                    }
                }
                EntityKind::Arc { center, a, b, ccw } => {
                    if let (Some((c2x, c2y)), Some((ax, ay)), Some((bx, by))) = (p(center), p(a), p(b)) {
                        let r2 = ((ax - c2x).powi(2) + (ay - c2y).powi(2)).sqrt();
                        let (a0, a1) = ((ay - c2y).atan2(ax - c2x), (by - c2y).atan2(bx - c2x));
                        for (ix, iy) in circle_circle_pts(cx, cy, r, c2x, c2y, r2) {
                            if angle_in_arc((iy - c2y).atan2(ix - c2x), a0, a1, ccw) {
                                pts.push((ix, iy));
                            }
                        }
                    }
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    if let (Some((ex, ey)), Some((mx, my)), Some((nx, ny))) = (p(c), p(ma), p(mi)) {
                        let (ux, uy, major, minor) = ellipse_axes(ex, ey, mx, my, nx, ny);
                        pts.extend(circle_ellipse_pts([cx, cy], r, [ex, ey], [ux, uy], major, minor));
                    }
                }
            }
        }
        let mut angs: Vec<f64> = pts.iter().map(|(x, y)| (y - cy).atan2(x - cx)).collect();
        angs.sort_by(|a, b| a.total_cmp(b));
        angs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        angs
    }
    /// Intersections of entity `eid` with the other entities (construction included) that lie on `eid`, in
    /// world coordinates. Used for the hover preview of trim and break.
    pub fn entity_intersections(&self, si: usize, eid: Id) -> Vec<(f64, f64)> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let Some(kind) = s.entities.iter().find(|e| e.id == eid).map(|e| e.kind) else { return Vec::new() };
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let mut out: Vec<(f64, f64)> = Vec::new();
        match kind {
            EntityKind::Line { a, b } => {
                let (Some((ax, ay)), Some((bx, by))) = (p(a), p(b)) else { return out };
                for o in &s.entities {
                    if o.id == eid {
                        continue;
                    }
                    match o.kind {
                        EntityKind::Line { a: c, b: d } => {
                            if let (Some((cx, cy)), Some((dx, dy))) = (p(c), p(d)) {
                                if let Some(t) = seg_seg_t([ax, ay], [bx, by], [cx, cy], [dx, dy]) {
                                    if t > 1e-6 && t < 1.0 - 1e-6 {
                                        out.push((ax + (bx - ax) * t, ay + (by - ay) * t));
                                    }
                                }
                            }
                        }
                        EntityKind::Circle { center, r } => {
                            if let Some((cx, cy)) = p(center) {
                                for t in seg_circle_t(ax, ay, bx, by, cx, cy, r) {
                                    if t > 1e-6 && t < 1.0 - 1e-6 {
                                        out.push((ax + (bx - ax) * t, ay + (by - ay) * t));
                                    }
                                }
                            }
                        }
                        EntityKind::Arc { center, a: aa, b: bb, ccw } => {
                            if let (Some((cx, cy)), Some((sx, sy)), Some((ex, ey))) = (p(center), p(aa), p(bb)) {
                                let r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                                let (a0, a1) = ((sy - cy).atan2(sx - cx), (ey - cy).atan2(ex - cx));
                                for t in seg_circle_t(ax, ay, bx, by, cx, cy, r) {
                                    let (ix, iy) = (ax + (bx - ax) * t, ay + (by - ay) * t);
                                    if t > 1e-6 && t < 1.0 - 1e-6 && angle_in_arc((iy - cy).atan2(ix - cx), a0, a1, ccw) {
                                        out.push((ix, iy));
                                    }
                                }
                            }
                        }
                        EntityKind::Ellipse { c, ma, mi } => {
                            if let (Some((ex, ey)), Some((mx, my)), Some((nx, ny))) = (p(c), p(ma), p(mi)) {
                                let (ux, uy, major, minor) = ellipse_axes(ex, ey, mx, my, nx, ny);
                                for t in seg_ellipse_t([ax, ay], [bx, by], [ex, ey], [ux, uy], major, minor) {
                                    if t > 1e-6 && t < 1.0 - 1e-6 {
                                        out.push((ax + (bx - ax) * t, ay + (by - ay) * t));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            EntityKind::Circle { center, r } => {
                if let Some((cx, cy)) = p(center) {
                    for ang in self.curve_cut_angles(si, eid, cx, cy, r) {
                        out.push((cx + r * ang.cos(), cy + r * ang.sin()));
                    }
                }
            }
            EntityKind::Arc { center, a, b, ccw } => {
                if let (Some((cx, cy)), Some((ax, ay)), Some((bx, by))) = (p(center), p(a), p(b)) {
                    let r = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
                    let (a0, a1) = ((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx));
                    for ang in self.curve_cut_angles(si, eid, cx, cy, r) {
                        if angle_in_arc(ang, a0, a1, ccw) {
                            out.push((cx + r * ang.cos(), cy + r * ang.sin()));
                        }
                    }
                }
            }
            EntityKind::Ellipse { .. } => {}
        }
        out
    }
    /// TRIM THAT LEAVES NOTHING: the click lay on an entity with no crossing inside it - a piece bounded by its own
    /// ends, or one crossing nothing at all. The whole of it is the piece under the click, so it goes, with what it
    /// orphans. Reported behaviour (issue #31): such a piece answered "The operation did not apply" and had to be
    /// deleted by hand, and every piece an earlier trim leaves is one.
    fn trim_away_whole(&mut self, si: usize, eid: Id) -> bool {
        self.delete_entities(si, &[eid]);
        self.solve_sketch(si);
        true
    }
    /// Trim a circle or an arc: it is cut at the intersections, the angular span under the click is removed and
    /// the remaining spans become arcs. Lines are handled by `trim_line`. Returns whether it succeeded.
    pub fn trim_curve(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        self.cut_projection(si, eid, |p| p.trim_curve_here(si, eid, clickx, clicky))
    }
    fn trim_curve_here(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        use std::f64::consts::TAU;
        let Some(s) = self.sketches.get(si) else { return false };
        let Some(e) = s.entities.iter().find(|e| e.id == eid) else { return false };
        let con = e.construction;
        let Some(Round { center_id, cx, cy, r, span }) = round_entity(s, e) else { return false };
        let cuts = self.curve_cut_angles(si, eid, cx, cy, r);
        if cuts.is_empty() {
            return self.trim_away_whole(si, eid);
        }
        let click_ang = (clicky - cy).atan2(clickx - cx);
        // One click removes only the span under the cursor, between the two nearest cut points, and the rest
        // stays whole: a circle leaves one remaining arc and an arc leaves up to two pieces, before and after
        // the removed window. Cutting at every intersection at once shatters a circle into a pile of arcs, which
        // breaks selection and radius editing and makes a pie slice impossible to cut out. Each remaining arc is
        // a tuple of (angle0, angle1, ccw).
        let mut kept: Vec<(f64, f64, bool)> = Vec::new();
        match span {
            None => {
                // A circle: one span under the click is removed and the remainder is a single arc, running
                // around the circle past the other cuts.
                let mut a: Vec<f64> = cuts.iter().map(|x| x.rem_euclid(TAU)).collect();
                a.sort_by(|x, y| x.total_cmp(y));
                a.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
                if a.len() < 2 {
                    // touched at one point at most: there is no span to cut out, so the circle goes
                    return self.trim_away_whole(si, eid);
                }
                let ca = click_ang.rem_euclid(TAU);
                let n = a.len();
                for i in 0..n {
                    let (lo, hi) = (a[i], if i + 1 < n { a[i + 1] } else { a[0] + TAU });
                    if (ca >= lo - 1e-9 && ca < hi) || (ca + TAU >= lo && ca + TAU < hi) {
                        kept.push((hi, lo + TAU, true)); // The remainder is one arc, from hi around to lo.
                        break;
                    }
                }
            }
            Some((a0, a1, ccw)) => {
                // An arc: the parameter runs along its direction over [0, sweep]; the window under the click is
                // removed and the pieces before and after it stay whole.
                let to_param = |ang: f64| if ccw { (ang - a0).rem_euclid(TAU) } else { (a0 - ang).rem_euclid(TAU) };
                let sweep = if ccw { (a1 - a0).rem_euclid(TAU) } else { (a0 - a1).rem_euclid(TAU) };
                let mut ps: Vec<f64> = cuts.iter().map(|&x| to_param(x)).filter(|&v| v > 1e-6 && v < sweep - 1e-6).collect();
                ps.push(0.0);
                ps.push(sweep);
                ps.sort_by(|x, y| x.total_cmp(y));
                ps.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
                let cp = to_param(click_ang);
                let ang = |param: f64| if ccw { a0 + param } else { a0 - param };
                // an arc crossed at its ends only is one window from end to end: nothing of it is left
                if ps.len() == 2 && cp > 1e-9 && cp < sweep - 1e-9 {
                    return self.trim_away_whole(si, eid);
                }
                for w in ps.windows(2) {
                    if cp > w[0] + 1e-9 && cp < w[1] - 1e-9 {
                        // Remove the window [w0, w1] under the click and keep the pieces before and after it,
                        // each as a single arc.
                        if w[0] > 1e-9 {
                            kept.push((ang(0.0), ang(w[0]), ccw));
                        }
                        if w[1] < sweep - 1e-9 {
                            kept.push((ang(w[1]), ang(sweep), ccw));
                        }
                        break;
                    }
                }
            }
        }
        if kept.is_empty() {
            return false;
        }
        // Replace the original entity with the remaining arcs.
        self.sketches[si].entities.retain(|x| x.id != eid);
        for (g0, g1, accw) in kept {
            let pa = self.sketch_point_at(si, cx + r * g0.cos(), cy + r * g0.sin(), 1e-6);
            let pb = self.sketch_point_at(si, cx + r * g1.cos(), cy + r * g1.sin(), 1e-6);
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Arc { center: center_id, a: pa, b: pb, ccw: accw }, construction: con });
            // The arc endpoints are anchored to the crossing entities, so a cut point does not drift along the
            // angle on the next solve.
            self.anchor_cut_point(si, pa);
            self.anchor_cut_point(si, pb);
        }
        self.merge_close_points(si, 1e-3); // Stitch coincident cut points so the pieces stay connected.
        self.prune_orphan_sketch_points(si); // Leave no dangling points behind a trim.
        self.solve_sketch(si); // the edit ends at the solver, not at a rebuild: `regen_sketch` recomputes the contours and checks no constraint
        true
    }
    /// Break a circle or an arc at the clicked point: an arc becomes two arcs, a circle becomes two half arcs
    /// (split at the click and at the opposite point). Lines are handled by `break_line`. Returns whether it
    /// succeeded.
    pub fn break_curve(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        self.cut_projection(si, eid, |p| p.break_curve_here(si, eid, clickx, clicky))
    }
    fn break_curve_here(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        let Some(e) = s.entities.iter().find(|e| e.id == eid) else { return false };
        let con = e.construction;
        let Some(Round { center_id, cx, cy, r, span }) = round_entity(s, e) else { return false };
        let ca = (clicky - cy).atan2(clickx - cx);
        let mk = |me: &mut Self, g0: f64, g1: f64, ccw: bool| {
            let pa = me.sketch_point_at(si, cx + r * g0.cos(), cy + r * g0.sin(), 1e-6);
            let pb = me.sketch_point_at(si, cx + r * g1.cos(), cy + r * g1.sin(), 1e-6);
            let id = me.alloc_id();
            me.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Arc { center: center_id, a: pa, b: pb, ccw }, construction: con });
        };
        self.sketches[si].entities.retain(|x| x.id != eid);
        match span {
            None => {
                // A circle becomes two arcs: [click, click + pi] and [click + pi, click].
                let opp = ca + std::f64::consts::PI;
                mk(self, ca, opp, true);
                mk(self, opp, ca, true);
            }
            Some((a0, a1, ccw)) => {
                // An arc becomes two arcs at the clicked point.
                mk(self, a0, ca, ccw);
                mk(self, ca, a1, ccw);
            }
        }
        self.merge_close_points(si, 1e-3); // Stitch coincident cut points so the pieces stay connected.
        self.prune_orphan_sketch_points(si); // Leave no dangling points behind a trim.
        self.solve_sketch(si); // the edit ends at the solver, not at a rebuild: `regen_sketch` recomputes the contours and checks no constraint
        true
    }
    /// WHERE A POINT STANDS in the drawing, or `None` where the sketch has no such point.
    pub fn point_xy(&self, si: usize, id: Id) -> Option<(f64, f64)> {
        let s = self.sketches.get(si)?;
        s.point(id).map(|p| (p.x, p.y))
    }
    /// Set the radius of an arc entity, moving its endpoints to radius `rr` while preserving their angles.
    /// PUT A RADIUS DIMENSION ON A PLAIN ARC: the radius set, then held by a driving dimension, as the radius of a
    /// circle is - one constraint more and one freedom less. Reported behaviour: the radius tool wrote 15 into the arc
    /// and left no dimension, the arc free to be dragged to any radius.
    pub fn put_arc_radius_dim(&mut self, si: usize, eid: Id, rr: f64) {
        let Some((center, a, b, ccw)) = self.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).and_then(|e| match e.kind {
            EntityKind::Arc { center, a, b, ccw } => Some((center, a, b, ccw)),
            _ => None,
        }) else {
            return;
        };
        let rr = rr.max(0.01);
        self.set_arc_radius(si, eid, rr);
        if let Some(s) = self.sketches.get_mut(si) {
            if !s.constraints.iter().any(|c| matches!(c, Constraint::Diameter { c: cc, .. } if *cc == center)) {
                let off = arc_label_angle(&s.points, ArcEnds { center, a, b, ccw });
                s.constraints.push(Constraint::Diameter { c: center, d: rr, off, expr: String::new(), driven: false, diam: false, at: None });
            }
        }
        self.solve_sketch(si);
    }
    pub fn set_arc_radius(&mut self, si: usize, eid: Id, rr: f64) {
        let (center, a, b) = {
            let Some(s) = self.sketches.get(si) else { return };
            let Some(e) = s.entities.iter().find(|e| e.id == eid) else { return };
            match e.kind {
                EntityKind::Arc { center, a, b, .. } => (center, a, b),
                _ => return,
            }
        };
        let Some((cx, cy)) = self.point_xy(si, center) else { return };
        let rr = rr.max(0.01);
        for pid in [a, b] {
            if let Some((px, py)) = self.point_xy(si, pid) {
                let ang = (py - cy).atan2(px - cx);
                if let Some(p) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
                    p.x = cx + rr * ang.cos();
                    p.y = cy + rr * ang.sin();
                }
            }
        }
        self.solve_sketch(si); // the edit ends at the solver, not at a rebuild: `regen_sketch` recomputes the contours and checks no constraint
    }
    /// The other endpoint of a segment (other than `exclude`) that uses point `pid`.
    pub(super) fn line_other_end(&self, si: usize, pid: Id, exclude: Id) -> Option<Id> {
        let s = self.sketches.get(si)?;
        s.entities.iter().find_map(|e| {
            if e.id == exclude {
                return None;
            }
            if let EntityKind::Line { a, b } = e.kind {
                if a == pid {
                    return Some(b);
                }
                if b == pid {
                    return Some(a);
                }
            }
            None
        })
    }
    /// Index of the radius dimension of an arc (a `Diameter` constraint in radius mode on the arc centre).
    pub fn fillet_radius_constraint(&self, si: usize, arc_eid: Id) -> Option<usize> {
        let s = self.sketches.get(si)?;
        let center = match s.entities.iter().find(|e| e.id == arc_eid)?.kind {
            EntityKind::Arc { center, .. } => center,
            _ => return None,
        };
        s.constraints.iter().position(|c| matches!(*c, Constraint::Diameter { c: cc, .. } if cc == center))
    }
    /// Change the radius of a fillet through its dimension, letting the solver move the geometry while keeping
    /// the tangency. Returns whether the arc has a radius dimension.
    pub fn set_fillet_radius_dim(&mut self, si: usize, arc_eid: Id, new_r: f64) -> bool {
        let Some(ci) = self.fillet_radius_constraint(si, arc_eid) else { return false };
        if let Some(Constraint::Diameter { d, diam, .. }) = self.sketches[si].constraints.get_mut(ci) {
            *d = if *diam { 2.0 * new_r.max(0.01) } else { new_r.max(0.01) };
        }
        // The tangency points and the centre are repositioned geometrically for the new radius, giving the
        // solver a consistent starting point so the geometry does not spread.
        self.set_fillet_radius(si, arc_eid, new_r);
        self.solve_sketch(si);
        true
    }
    /// Change the radius of a fillet arc: the corner is reconstructed from the two adjacent lines and the
    /// fillet is rebuilt, moving the tangency points and the centre. Returns `false` when it is not a
    /// fillet.
    pub fn set_fillet_radius(&mut self, si: usize, arc_eid: Id, new_r: f64) -> bool {
        let (center, t1, t2) = {
            let Some(s) = self.sketches.get(si) else { return false };
            let Some(e) = s.entities.iter().find(|e| e.id == arc_eid) else { return false };
            match e.kind {
                EntityKind::Arc { center, a, b, .. } => (center, a, b),
                _ => return false,
            }
        };
        let (Some(o1), Some(o2)) = (self.line_other_end(si, t1, arc_eid), self.line_other_end(si, t2, arc_eid)) else { return false };
        let (Some((o1x, o1y)), Some((t1x, t1y)), Some((o2x, o2y)), Some((t2x, t2y))) = (self.point_xy(si, o1), self.point_xy(si, t1), self.point_xy(si, o2), self.point_xy(si, t2)) else {
            return false;
        };
        // The corner is the intersection of the lines o1 to t1 and o2 to t2.
        let Some((px, py)) = line_intersect_inf([o1x, o1y], [t1x, t1y], [o2x, o2y], [t2x, t2y]) else { return false };
        let l1 = ((o1x - px).powi(2) + (o1y - py).powi(2)).sqrt();
        let l2 = ((o2x - px).powi(2) + (o2y - py).powi(2)).sqrt();
        if l1 < 1e-9 || l2 < 1e-9 {
            return false;
        }
        let (ux, uy) = ((o1x - px) / l1, (o1y - py) / l1);
        let (vx, vy) = ((o2x - px) / l2, (o2y - py) / l2);
        let cosang = (ux * vx + uy * vy).clamp(-1.0, 1.0);
        let theta = cosang.acos();
        if theta < 1e-3 || theta > std::f64::consts::PI - 1e-3 {
            return false;
        }
        let half = theta / 2.0;
        let t = (new_r / half.tan()).min(l1 * 0.98).min(l2 * 0.98);
        let r = t * half.tan();
        let (nt1x, nt1y) = (px + ux * t, py + uy * t);
        let (nt2x, nt2y) = (px + vx * t, py + vy * t);
        let bl = ((ux + vx).powi(2) + (uy + vy).powi(2)).sqrt();
        if bl < 1e-9 {
            return false;
        }
        let (bisx, bisy) = ((ux + vx) / bl, (uy + vy) / bl);
        let (ncx, ncy) = (px + bisx * (r / half.sin()), py + bisy * (r / half.sin()));
        let cross = (nt1x - ncx) * (nt2y - ncy) - (nt1y - ncy) * (nt2x - ncx);
        let ccw = cross > 0.0;
        // Move the existing points; the lines follow t1 and t2.
        for (pid, x, y) in [(t1, nt1x, nt1y), (t2, nt2x, nt2y), (center, ncx, ncy)] {
            if let Some(p) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
                p.x = x;
                p.y = y;
            }
        }
        if let Some(e) = self.sketches[si].entities.iter_mut().find(|e| e.id == arc_eid) {
            if let EntityKind::Arc { ccw: c, .. } = &mut e.kind {
                *c = ccw;
            }
        }
        // Keep the radius dimension in step with the geometry, when the fillet has a parametric one.
        if let Some(ci) = self.fillet_radius_constraint(si, arc_eid) {
            if let Some(Constraint::Diameter { d, diam, .. }) = self.sketches[si].constraints.get_mut(ci) {
                *d = if *diam { 2.0 * r.max(0.01) } else { r.max(0.01) };
            }
        }
        self.regen_sketch(si);
        true
    }
    /// The same with a size given by a radius, a chord or an arc length (see `fillet_at_vertex_by`).
    pub fn fillet_lines_by(&mut self, si: usize, e1: Id, e2: Id, size: FilletSize) -> bool {
        let r = match size.by {
            FilletBy::Radius => size.value,
            _ => {
                let (Some((a1, b1)), Some((a2, b2))) = (self.line_ends(si, e1), self.line_ends(si, e2)) else { return false };
                let pc = if a1 == a2 || a1 == b2 {
                    a1
                } else if b1 == a2 || b1 == b2 {
                    b1
                } else {
                    return false;
                };
                let (o1, o2) = (if a1 == pc { b1 } else { a1 }, if a2 == pc { b2 } else { a2 });
                let (Some((px, py)), Some((ax, ay)), Some((bx, by))) = (self.point_xy(si, pc), self.point_xy(si, o1), self.point_xy(si, o2)) else { return false };
                let (la, lb) = ((ax - px).hypot(ay - py), (bx - px).hypot(by - py));
                if la < 1e-9 || lb < 1e-9 {
                    return false;
                }
                let corner = (((ax - px) * (bx - px) + (ay - py) * (by - py)) / (la * lb)).clamp(-1.0, 1.0).acos();
                let Some(r) = size.radius_on(std::f64::consts::PI - corner) else { return false };
                // what the lines take: the points of touching r / tan(corner / 2) from the corner, within the shorter line
                if r / (corner / 2.0).tan() >= la.min(lb) * 0.95 {
                    return false;
                }
                r
            }
        };
        if !self.fillet_lines(si, e1, e2, r) {
            return false;
        }
        if size.by != FilletBy::Radius {
            self.give_fillet_its_size(si, size);
        }
        true
    }
    /// Fillet the corner between two segments sharing a vertex, with radius `r`. The lines are shortened to the
    /// tangency points and an arc is inserted between them. Returns whether it succeeded.
    pub fn fillet_lines(&mut self, si: usize, e1: Id, e2: Id, r: f64) -> bool {
        let (Some((a1, b1)), Some((a2, b2))) = (self.line_ends(si, e1), self.line_ends(si, e2)) else { return false };
        let pc = if a1 == a2 || a1 == b2 {
            a1
        } else if b1 == a2 || b1 == b2 {
            b1
        } else {
            return false; // No shared vertex.
        };
        let o1 = if a1 == pc { b1 } else { a1 };
        let o2 = if a2 == pc { b2 } else { a2 };
        let (Some((px, py)), Some((ax, ay)), Some((bx, by))) = (self.point_xy(si, pc), self.point_xy(si, o1), self.point_xy(si, o2)) else { return false };
        let la = ((ax - px).powi(2) + (ay - py).powi(2)).sqrt();
        let lb = ((bx - px).powi(2) + (by - py).powi(2)).sqrt();
        if la < 1e-9 || lb < 1e-9 {
            return false;
        }
        let (ux, uy) = ((ax - px) / la, (ay - py) / la);
        let (vx, vy) = ((bx - px) / lb, (by - py) / lb);
        let cos = (ux * vx + uy * vy).clamp(-1.0, 1.0);
        let theta = cos.acos();
        if theta < 1e-3 || theta > std::f64::consts::PI - 1e-3 {
            return false; // Collinear: there is no corner to fillet.
        }
        let half = theta / 2.0;
        let mut t = r / half.tan();
        let maxt = la.min(lb) * 0.95;
        let r = if t > maxt {
            t = maxt;
            t * half.tan()
        } else {
            r
        };
        let dd = r / half.sin();
        let (t1x, t1y) = (px + ux * t, py + uy * t);
        let (t2x, t2y) = (px + vx * t, py + vy * t);
        let bl = ((ux + vx).powi(2) + (uy + vy).powi(2)).sqrt();
        if bl < 1e-9 {
            return false;
        }
        let (bisx, bisy) = ((ux + vx) / bl, (uy + vy) / bl);
        let (cxx, cyy) = (px + bisx * dd, py + bisy * dd);
        let cross = (t1x - cxx) * (t2y - cyy) - (t1y - cyy) * (t2x - cxx);
        let ccw = cross > 0.0;
        let t1 = self.sketch_point_at(si, t1x, t1y, 1e-9);
        let t2 = self.sketch_point_at(si, t2x, t2y, 1e-9);
        let cen = self.sketch_point_at(si, cxx, cyy, 1e-9);
        let arc = self.alloc_id();
        if let Some(s) = self.sketches.get_mut(si) {
            for e in s.entities.iter_mut() {
                if e.id == e1 {
                    e.kind = EntityKind::Line { a: o1, b: t1 };
                } else if e.id == e2 {
                    e.kind = EntityKind::Line { a: o2, b: t2 };
                }
            }
            s.entities.push(SketchEntity { id: arc, kind: EntityKind::Arc { center: cen, a: t1, b: t2, ccw }, construction: false });
            // A parametric fillet: the arc is held by tangency to both lines plus a radius dimension (a
            // `Diameter` constraint in radius mode on the arc centre — one R dimension, with no extra linear
            // witness line).
            // The arc is a real entity now (its endpoints sit intrinsically on the circle of a radius
            // variable), so the constraints are stable: moving the walls carries the fillet along and it stays
            // tangent.
            s.constraints.push(Constraint::Tangent { a: o1, b: t1, c: cen, r });
            s.constraints.push(Constraint::Tangent { a: o2, b: t2, c: cen, r });
            let off = fillet_label_angle(&s.points, cen, t1, t2);
            s.constraints.push(Constraint::Diameter { c: cen, d: r, off, expr: String::new(), driven: false, diam: false, at: None });
        }
        // The point of the corner (described in `settle_the_corner_point`): it goes with the corner, and what was
        // stated about it goes with it. What is left on each shortened line is a real point of it.
        self.settle_the_corner_point(si, pc, o1, t1, o2, t2);
        self.regen_sketch(si);
        true
    }
    /// WHAT HOLDS A LINE GOES WITH THE LINE when a fillet or a chamfer shortens it: a horizontal, vertical, parallel,
    /// perpendicular, collinear or equal of an edge's ends (o-pc) is the edge's own, and after the fillet the edge runs
    /// o-t. Left on the vanished vertex, the automatic constraints a person's defaults put on every corner kept that
    /// vertex alive as a point of its own - six points where the filleted corner has five - and a typed radius seemed
    /// to do nothing. A side that is not a line (`None`) keeps what it had.
    pub(super) fn carry_edge_constraints(&mut self, si: usize, pc: Id, side1: Option<(Id, Id)>, side2: Option<(Id, Id)>) {
        let pair = |x: Id, y: Id| -> (Id, Id) {
            for (o, t) in [side1, side2].into_iter().flatten() {
                // a side the corner ate whole (radius equal to it) has no second point to carry onto
                if o != t && ((x == o && y == pc) || (x == pc && y == o)) {
                    return (o, t);
                }
            }
            (x, y)
        };
        // only a constraint naming the corner has a pair to carry: `pair` changes a pair one of whose points is `pc`. The
        // whole match for every constraint of the sketch was 2.3 s of filleting every corner of 2 000 rectangles.
        let names_corner = |c: &Constraint| match *c {
            Constraint::Horizontal { a, b } | Constraint::Vertical { a, b } | Constraint::Orientation { a, b, .. } | Constraint::Tangent { a, b, .. } | Constraint::PointOnLine { a, b, .. } => {
                a == pc || b == pc
            }
            Constraint::Equal { a, b, c, d } | Constraint::Parallel { a, b, c, d } | Constraint::Perpendicular { a, b, c, d } | Constraint::Collinear { a, b, c, d } => [a, b, c, d].contains(&pc),
            _ => false,
        };
        if let Some(s) = self.sketches.get_mut(si) {
            for c in s.constraints.iter_mut().filter(|c| names_corner(c)) {
                match c {
                    // a tangency or a point held on a line names the line by two points too: rounding the next corner of
                    // a rectangle shortened a side whose tangency still ran to the old vertex, and that vertex was kept
                    // as a point nothing drew (14 or 15 points on a rectangle rounded all round, not 12)
                    Constraint::Horizontal { a, b }
                    | Constraint::Vertical { a, b }
                    | Constraint::Orientation { a, b, .. }
                    | Constraint::Tangent { a, b, .. }
                    | Constraint::PointOnLine { a, b, .. } => (*a, *b) = pair(*a, *b),
                    // AN EQUALITY IS A LENGTH: it is carried onto the pieces the cut leaves only where they are still
                    // equal - both sides of the corner shortened alike. Carried regardless, it said 55 = 58 for a chamfer
                    // of 3 on one of two equal sides of 58, and the solver bent everything tied to the drawing to satisfy
                    // it. Otherwise it stays on the corner, which then stays as the virtual sharp.
                    Constraint::Equal { a, b, c: cc, d } => {
                        let (ab, cd) = (pair(*a, *b), pair(*cc, *d));
                        let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
                        let len = |(u, v): (Id, Id)| Some((at(u)?.0 - at(v)?.0).hypot(at(u)?.1 - at(v)?.1));
                        if let (Some(l1), Some(l2)) = (len(ab), len(cd)) {
                            if (l1 - l2).abs() <= 1e-9 * l1.max(l2).max(1.0) {
                                ((*a, *b), (*cc, *d)) = (ab, cd);
                            }
                        }
                    }
                    // a direction is the same along the whole line, so it is carried as it is
                    Constraint::Parallel { a, b, c: cc, d } | Constraint::Perpendicular { a, b, c: cc, d } | Constraint::Collinear { a, b, c: cc, d } => {
                        (*a, *b) = pair(*a, *b);
                        (*cc, *d) = pair(*cc, *d);
                    }
                    _ => {}
                }
            }
        }
    }
    /// THE POINT OF A CORNER THAT HAS BEEN TAKEN OFF: it stays, as the VIRTUAL SHARP on the extensions of both shortened
    /// lines - drawn and picked like any point, so a dimension or a constraint can be measured to the corner at any time.
    ///
    /// Where four lines met at the point and two of them have just been cut, the other two still end at it and the point
    /// stays their vertex - and the two cut lines are tied to it all the same, as to any sharp.
    pub(super) fn settle_the_corner_point(&mut self, si: usize, pc: Id, o1: Id, t1: Id, o2: Id, t2: Id) {
        // THE TWO CUT LINES ARE TIED TO THE CORNER EVEN WHERE OTHER LINES STILL END AT IT. Skipped there, the first cut
        // of a cross left its two lines on nothing: their constraints still named the point, the lines no longer did,
        // and dragging the point swung them loose by 4 mm.
        self.carry_edge_constraints(si, pc, Some((o1, t1)), Some((o2, t2)));
        // THE CORNER STAYS, ALWAYS, as the virtual sharp on the extensions of both shortened lines: drawn and picked, so a
        // dimension or a constraint can be measured to it at any time, as in the professional systems. Kept only while
        // something stood on it, a chamfer left its sharp and a fillet did not, and a rectangle's were hidden: three
        // rules for one point.
        if let Some(s) = self.sketches.get_mut(si) {
            s.constraints.push(Constraint::PointOnLine { p: pc, a: o1, b: t1 });
            s.constraints.push(Constraint::PointOnLine { p: pc, a: o2, b: t2 });
        }
    }
    /// Whether any entity of the sketch stands on the point `pid` - an end of a line or an arc, a centre of one.
    pub fn point_used_by_geometry(&self, si: usize, pid: Id) -> bool {
        self.sketches.get(si).is_some_and(|s| {
            s.entities.iter().any(|e| match e.kind {
                EntityKind::Line { a, b } => a == pid || b == pid,
                EntityKind::Arc { center, a, b, .. } => center == pid || a == pid || b == pid,
                EntityKind::Circle { center, .. } => center == pid,
                EntityKind::Ellipse { c, ma, mi } => c == pid || ma == pid || mi == pid,
            }) || s.splines.iter().any(|sp| sp.points.contains(&pid))
        })
    }
    /// HOW MANY CONSTRAINTS STATE SOMETHING ABOUT THE POINT `pid` - a dimension measured to a corner above all.
    pub fn constraints_on_point(&self, si: usize, pid: Id) -> usize {
        self.sketches.get(si).map(|s| s.constraints.iter().filter(|c| constraint_point_ids(c).contains(&pid)).count()).unwrap_or(0)
    }
    /// Chamfer between two segments sharing a vertex, with setback `d`.
    pub fn chamfer_lines(&mut self, si: usize, e1: Id, e2: Id, legs: ChamferLegs) -> bool {
        let (Some((a1, b1)), Some((a2, b2))) = (self.line_ends(si, e1), self.line_ends(si, e2)) else { return false };
        let pc = if a1 == a2 || a1 == b2 {
            a1
        } else if b1 == a2 || b1 == b2 {
            b1
        } else {
            return false;
        };
        let o1 = if a1 == pc { b1 } else { a1 };
        let o2 = if a2 == pc { b2 } else { a2 };
        let (Some((px, py)), Some((ax, ay)), Some((bx, by))) = (self.point_xy(si, pc), self.point_xy(si, o1), self.point_xy(si, o2)) else { return false };
        let la = ((ax - px).powi(2) + (ay - py).powi(2)).sqrt();
        let lb = ((bx - px).powi(2) + (by - py).powi(2)).sqrt();
        if la < 1e-9 || lb < 1e-9 {
            return false;
        }
        // the angle of the corner between the two lines
        let corner = (((ax - px) * (bx - px) + (ay - py) * (by - py)) / (la * lb)).clamp(-1.0, 1.0).acos();
        let Some((d1, d2)) = legs.along(corner) else { return false };
        // A LEG AS LONG AS ITS LINE, OR LONGER, IS REFUSED: pressed silently to 0.95 of the shorter line it was taken - a leg
        // of 300 on lines of 30 made a chamfer of 28.5 with no word, and so did one of 29.9
        if d1 >= la - 1e-9 || d2 >= lb - 1e-9 {
            return false;
        }
        let (t1x, t1y) = (px + (ax - px) / la * d1, py + (ay - py) / la * d1);
        let (t2x, t2y) = (px + (bx - px) / lb * d2, py + (by - py) / lb * d2);
        let t1 = self.sketch_point_at(si, t1x, t1y, 1e-9);
        let t2 = self.sketch_point_at(si, t2x, t2y, 1e-9);
        let seg = self.alloc_id();
        if let Some(s) = self.sketches.get_mut(si) {
            for e in s.entities.iter_mut() {
                if e.id == e1 {
                    e.kind = EntityKind::Line { a: o1, b: t1 };
                } else if e.id == e2 {
                    e.kind = EntityKind::Line { a: o2, b: t2 };
                }
            }
            s.entities.push(SketchEntity { id: seg, kind: EntityKind::Line { a: t1, b: t2 }, construction: false });
            // THE CHAMFER KEEPS ITS SIZE AS DIMENSIONS, so it can be changed afterwards and the cut follows. A symmetric
            // one is sized by the cut itself, between its two ends, and held symmetric by its legs kept equal; two legs,
            // or a leg and the angle from the first line, are measured from the sharp corner. Whatever stands on the
            // corner keeps it as the virtual sharp below.
            // EVERY DIMENSION OF THE CHAMFER STANDS OUTSIDE THE SHAPE: the cut on the side of the sharp, a leg on the side
            // of its line away from the cut. Written as they came, the dimension of a chamfer on one corner of a triangle
            // dipped into the triangle, and one leg of two on a square stood inside it.
            let (sharp, at1, at2) = (Point2::new(px, py), Point2::new(t1x, t1y), Point2::new(t2x, t2y));
            let leg = |ends: [DimEnd; 2], side: Side, d: f64| {
                let [a, b] = ends_facing(ends, side);
                Constraint::Distance { a, b, d, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None }
            };
            let (end_c, end_1, end_2) = (DimEnd { id: pc, at: sharp }, DimEnd { id: t1, at: at1 }, DimEnd { id: t2, at: at2 });
            match legs.mode {
                crate::feature::ChamferMode::Symmetric => {
                    s.constraints.push(leg([end_1, end_2], Side::Toward(sharp), legs.first));
                    s.constraints.push(Constraint::Equal { a: pc, b: t1, c: pc, d: t2 });
                }
                crate::feature::ChamferMode::TwoDist => {
                    s.constraints.push(leg([end_c, end_1], Side::AwayFrom(at2), d1));
                    s.constraints.push(leg([end_c, end_2], Side::AwayFrom(at1), d2));
                }
                // the angle between the first line, run towards the corner, and the cut: what a drawing of the chamfer gives
                crate::feature::ChamferMode::DistAngle => {
                    s.constraints.push(leg([end_c, end_1], Side::AwayFrom(at2), d1));
                    // THE ARC OF THE ANGLE STANDS PAST THE END OF THE CUT, 1.25 of its length out, both sides carried to it:
                    // at the radius of every angle, 24 px, it was a tick tucked between the cut and the line
                    let off = 1.25 * (t2x - t1x).hypot(t2y - t1y);
                    s.constraints.push(Constraint::AngleLines { a: o1, b: t1, c: t1, d: t2, deg: legs.second, expr: String::new(), driven: false, off, at: None });
                }
            }
        }
        // Virtual corner: the vertex is held on the extensions of both lines, so dimensions to the corner stay
        // valid and the contour stays whole.
        self.settle_the_corner_point(si, pc, o1, t1, o2, t2);
        // SOLVED, not only drawn: the legs and the angle are laid as dimensions above, and a chamfer of two legs or of a
        // leg and an angle stands by them only once the sketch is solved - at the end of a batch of corners, once
        self.solve_sketch_held(si);
        true
    }
    /// Endpoints of an edge entity (a line or an arc), used to find the shared vertex when filleting.
    pub(super) fn edge_end_ids(&self, si: usize, eid: Id) -> Option<(Id, Id)> {
        let s = self.sketches.get(si)?;
        s.entity(eid).and_then(|e| match e.kind {
            EntityKind::Line { a, b } => Some((a, b)),
            EntityKind::Arc { a, b, .. } => Some((a, b)),
            _ => None,
        })
    }
    /// General fillet: an arc of radius `r` tangent to two adjacent edges (lines or arcs) sharing a vertex.
    ///
    /// The offset method: the fillet centre is the intersection of the offset curves of both edges at distance
    /// `r`; of the four sign combinations, the one nearest the click (inside the corner) with tangency points
    /// within the edges is taken. The edges are trimmed to the tangency points and a parametric arc is added
    /// (tangency to each edge plus a radius dimension). Works for line to line, line to arc and arc to arc.
    /// Returns whether it succeeded.
    pub fn fillet_curves(&mut self, si: usize, e1: Id, e2: Id, r: f64, nearx: f64, neary: f64) -> bool {
        if r <= 1e-9 {
            return false;
        }
        let Some((e1a, e1b)) = self.edge_end_ids(si, e1) else { return false };
        let Some((e2a, e2b)) = self.edge_end_ids(si, e2) else { return false };
        // The shared vertex.
        let pc = if e1a == e2a || e1a == e2b {
            e1a
        } else if e1b == e2a || e1b == e2b {
            e1b
        } else {
            return false;
        };
        let o1 = if e1a == pc { e1b } else { e1a };
        let o2 = if e2a == pc { e2b } else { e2a };
        let Some((pcx, pcy)) = self.point_xy(si, pc) else { return false };
        // Support of an edge at the vertex: its kind and parameters, plus the sign of the side facing the
        // interior of the corner.
        #[derive(Clone, Copy)]
        enum Sup {
            Line { px: f64, py: f64, ux: f64, uy: f64, len: f64 }, // Vertex point, unit direction towards o,
            // and length.
            Circle { cx: f64, cy: f64, rad: f64, center: Id },
        }
        let support = |me: &Self, eid: Id, other: Id| -> Option<Sup> {
            let kind = me.sketches.get(si)?.entities.iter().find(|e| e.id == eid).map(|e| e.kind)?;
            match kind {
                EntityKind::Line { .. } => {
                    let (ox, oy) = me.point_xy(si, other)?;
                    let (dx, dy) = (ox - pcx, oy - pcy);
                    let len = (dx * dx + dy * dy).sqrt();
                    if len < 1e-9 {
                        return None;
                    }
                    Some(Sup::Line { px: pcx, py: pcy, ux: dx / len, uy: dy / len, len })
                }
                EntityKind::Arc { center, .. } => {
                    let (cx, cy) = me.point_xy(si, center)?;
                    let rad = ((pcx - cx).powi(2) + (pcy - cy).powi(2)).sqrt();
                    Some(Sup::Circle { cx, cy, rad, center })
                }
                _ => None,
            }
        };
        let (Some(s1), Some(s2)) = (support(self, e1, o1), support(self, e2, o2)) else { return false };
        // Candidate centres at distance r from both supports, over the sign combinations of the offsets.
        let mut centers: Vec<(f64, f64)> = Vec::new();
        let line_off = |px: f64, py: f64, ux: f64, uy: f64, sgn: f64| (px - uy * r * sgn, py + ux * r * sgn, ux, uy); // Offset line: a point plus a direction.
        let line_line = |l1: (f64, f64, f64, f64), l2: (f64, f64, f64, f64)| -> Option<(f64, f64)> {
            let (p1x, p1y, d1x, d1y) = l1;
            let (p2x, p2y, d2x, d2y) = l2;
            let den = d1x * d2y - d1y * d2x;
            if den.abs() < 1e-12 {
                return None;
            }
            let t = ((p2x - p1x) * d2y - (p2y - p1y) * d2x) / den;
            Some((p1x + d1x * t, p1y + d1y * t))
        };
        // Line (point ox, oy plus direction dx, dy) against circle (cx, cy, rr), giving points.
        let line_circle_pts = |ox: f64, oy: f64, dx: f64, dy: f64, cx: f64, cy: f64, rr: f64| -> Vec<(f64, f64)> {
            line_circle_t(ox, oy, ox + dx, oy + dy, cx, cy, rr).into_iter().map(|t| (ox + dx * t, oy + dy * t)).collect()
        };
        for s1g in [-1.0_f64, 1.0] {
            for s2g in [-1.0_f64, 1.0] {
                match (s1, s2) {
                    (Sup::Line { px: ax, py: ay, ux: aux, uy: auy, .. }, Sup::Line { px: bx, py: by, ux: bux, uy: buy, .. }) => {
                        if let Some(c) = line_line(line_off(ax, ay, aux, auy, s1g), line_off(bx, by, bux, buy, s2g)) {
                            centers.push(c);
                        }
                    }
                    (Sup::Line { px, py, ux, uy, .. }, Sup::Circle { cx, cy, rad, .. }) => {
                        let (ox, oy, dx, dy) = line_off(px, py, ux, uy, s1g);
                        centers.extend(line_circle_pts(ox, oy, dx, dy, cx, cy, (rad + r * s2g).abs()));
                    }
                    (Sup::Circle { cx, cy, rad, .. }, Sup::Line { px, py, ux, uy, .. }) => {
                        let (ox, oy, dx, dy) = line_off(px, py, ux, uy, s2g);
                        centers.extend(line_circle_pts(ox, oy, dx, dy, cx, cy, (rad + r * s1g).abs()));
                    }
                    (Sup::Circle { cx: c1x, cy: c1y, rad: r1, .. }, Sup::Circle { cx: c2x, cy: c2y, rad: r2, .. }) => {
                        centers.extend(circle_circle_pts(c1x, c1y, (r1 + r * s1g).abs(), c2x, c2y, (r2 + r * s2g).abs()));
                    }
                }
            }
        }
        // Tangency point of a support, from centre C.
        let tangent_pt = |sup: Sup, cx: f64, cy: f64| -> Option<(f64, f64)> {
            match sup {
                Sup::Line { px, py, ux, uy, len } => {
                    let proj = (cx - px) * ux + (cy - py) * uy;
                    if proj < -1e-6 || proj > len + 1e-6 {
                        return None; // The tangency point lies outside the edge.
                    }
                    Some((px + ux * proj, py + uy * proj))
                }
                Sup::Circle { cx: ccx, cy: ccy, rad, .. } => {
                    let d = ((cx - ccx).powi(2) + (cy - ccy).powi(2)).sqrt();
                    if d < 1e-9 {
                        return None;
                    }
                    Some((ccx + (cx - ccx) / d * rad, ccy + (cy - ccy) / d * rad))
                }
            }
        };
        // A NAMED RECORD, not a four-place tuple of pairs: `(f64, (f64, f64), (f64, f64), (f64, f64))` needed
        // a comment beside it saying which pair was the centre and which the tangency points, and a comment is
        // not checked by anything.
        #[derive(Clone, Copy)]
        struct Candidate {
            /// How far the centre is from the click - smaller wins.
            score: f64,
            /// The fillet's centre.
            center: (f64, f64),
            /// Where it touches the first edge.
            t1: (f64, f64),
            /// Where it touches the second.
            t2: (f64, f64),
        }
        // Choose the centre: tangency valid on both edges, and nearest to the click.
        let mut best: Option<Candidate> = None;
        for (cx, cy) in centers {
            let (Some(t1), Some(t2)) = (tangent_pt(s1, cx, cy), tangent_pt(s2, cx, cy)) else { continue };
            // Both tangency points have to lie on the vertex side, between `pc` and `o`; `tangent_pt` checks
            // that for lines, while for an arc the radial projection is trusted. The score is the distance from
            // the centre to the click.
            let score = (cx - nearx).powi(2) + (cy - neary).powi(2);
            if best.is_none_or(|b| score < b.score) {
                best = Some(Candidate { score, center: (cx, cy), t1, t2 });
            }
        }
        let Some(Candidate { center: (cxx, cyy), t1: (t1x, t1y), t2: (t2x, t2y), .. }) = best else { return false };
        // Build it.
        let t1 = self.sketch_point_at(si, t1x, t1y, 1e-9);
        let t2 = self.sketch_point_at(si, t2x, t2y, 1e-9);
        let cen = self.sketch_point_at(si, cxx, cyy, 1e-9);
        // The short arc from t1 to t2, bulging towards the vertex.
        let a1 = (t1y - cyy).atan2(t1x - cxx);
        let a2 = (t2y - cyy).atan2(t2x - cxx);
        let sweep_ccw = (a2 - a1).rem_euclid(std::f64::consts::TAU);
        let ccw = sweep_ccw <= std::f64::consts::PI;
        let arc = self.alloc_id();
        // Trim the edges: replace the endpoint at the vertex with the tangency point.
        let replace_end = |me: &mut Self, eid: Id, newp: Id| {
            if let Some(e) = me.sketches.get_mut(si).and_then(|s| s.entities.iter_mut().find(|e| e.id == eid)) {
                match &mut e.kind {
                    EntityKind::Line { a, b } => {
                        if *a == pc {
                            *a = newp;
                        } else if *b == pc {
                            *b = newp;
                        }
                    }
                    EntityKind::Arc { a, b, .. } => {
                        if *a == pc {
                            *a = newp;
                        } else if *b == pc {
                            *b = newp;
                        }
                    }
                    _ => {}
                }
            }
        };
        replace_end(self, e1, t1);
        replace_end(self, e2, t2);
        // Degenerate case: a radius equal to half the side collapses the trimmed edge to zero length, the
        // tangency point having met the far end left by the previous fillet. A zero-length line must not be
        // kept: its tangency constraint is degenerate (a 0/0 direction), the solver then conflicts and deflates
        // the radii of the neighbouring arcs — a 4 by 4 square at r = 2 gave about 1.33 instead of a circle. The
        // zero edge is removed, distinct points in the same place are merged with `Coincident`, and no tangency
        // is added for the removed edge.
        let mut dropped: Vec<Id> = Vec::new();
        for eid in [e1, e2] {
            let Some((a, b)) = self.edge_end_ids(si, eid) else { continue };
            let gone = if a == b {
                true
            } else {
                match (self.point_xy(si, a), self.point_xy(si, b)) {
                    (Some((ax, ay)), Some((bx, by))) => ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt() < 1e-6,
                    _ => false,
                }
            };
            if gone {
                if let Some(s) = self.sketches.get_mut(si) {
                    s.entities.retain(|e| e.id != eid);
                    if a != b {
                        s.constraints.push(Constraint::Coincident { a, b });
                    }
                }
                dropped.push(eid);
            }
        }
        // The fillet arc, plus parametric tangency constraints and a radius dimension.
        let (s1c, s2c) = (s1, s2);
        if let Some(s) = self.sketches.get_mut(si) {
            s.entities.push(SketchEntity { id: arc, kind: EntityKind::Arc { center: cen, a: t1, b: t2, ccw }, construction: false });
            // External or internal tangency between circles, chosen by which the actual centre distance is
            // nearer to.
            let ext = |cx: f64, cy: f64, rad: f64| {
                let d = ((cxx - cx).powi(2) + (cyy - cy).powi(2)).sqrt();
                ((rad + r) - d).abs() <= ((rad - r).abs() - d).abs()
            };
            if !dropped.contains(&e1) {
                match s1c {
                    Sup::Line { .. } => s.constraints.push(Constraint::Tangent { a: o1, b: t1, c: cen, r }),
                    Sup::Circle { cx, cy, rad, center } => s.constraints.push(Constraint::CircleTangent { c1: center, c2: cen, external: ext(cx, cy, rad) }),
                }
            }
            if !dropped.contains(&e2) {
                match s2c {
                    Sup::Line { .. } => s.constraints.push(Constraint::Tangent { a: o2, b: t2, c: cen, r }),
                    Sup::Circle { cx, cy, rad, center } => s.constraints.push(Constraint::CircleTangent { c1: center, c2: cen, external: ext(cx, cy, rad) }),
                }
            }
            let off = fillet_label_angle(&s.points, cen, t1, t2);
            s.constraints.push(Constraint::Diameter { c: cen, d: r, off, expr: String::new(), driven: false, diam: false, at: None });
        }
        // Virtual corner: vertex `pc` is kept and the constraints and dimensions on it are left alone, so it
        // stays the sharp corner on the extensions of both shortened edges.
        //
        // Deleting the corner and moving the dimensions onto the tangency points recomputes them against the
        // shortened segment: any edit — the radius, a neighbouring dimension — then makes the geometry wander,
        // the dimensions conflict and go red, and the broken contour cannot be selected. Measuring an edge
        // dimension to the virtual corner holds at any radius, while `pc` belongs to no entity and therefore
        // never reaches a contour or a profile.
        //
        // `pc` is held against every support: a line by `PointOnLine` on its extension, an arc or circle by
        // `PointOnCircle` - also where a third edge still ends at it and it stays that edge's vertex: the two cut
        // edges no longer end there, and untied they swing loose when the point is dragged.
        let side = |sup: &Sup, o: Id, t: Id| matches!(sup, Sup::Line { .. }).then_some((o, t));
        self.carry_edge_constraints(si, pc, side(&s1c, o1, t1), side(&s2c, o2, t2));
        if let Some(s) = self.sketches.get_mut(si) {
            match s1c {
                Sup::Line { .. } => s.constraints.push(Constraint::PointOnLine { p: pc, a: o1, b: t1 }),
                Sup::Circle { center, .. } => s.constraints.push(Constraint::PointOnCircle { p: pc, c: center }),
            }
            match s2c {
                Sup::Line { .. } => s.constraints.push(Constraint::PointOnLine { p: pc, a: o2, b: t2 }),
                Sup::Circle { center, .. } => s.constraints.push(Constraint::PointOnCircle { p: pc, c: center }),
            }
        }
        self.regen_sketch(si);
        true
    }
    /// Current value of a tangent (edge-to-edge) dimension: the centre distance plus m1*r1 + m2*r2. The radii
    /// come from the circle or arc with the matching centre, and are zero when the centre is an ordinary
    /// point.
    pub fn measure_edge_distance(&self, si: usize, c1: Id, m1: i8, c2: Id, m2: i8) -> f64 {
        let Some(s) = self.sketches.get(si) else { return 0.0 };
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let r_of = |cid: Id| -> f64 {
            s.entities
                .iter()
                .find_map(|e| match e.kind {
                    EntityKind::Circle { center, r } if center == cid => Some(r),
                    EntityKind::Arc { center, a, .. } if center == cid => match (p(center), p(a)) {
                        (Some((cx, cy)), Some((ax, ay))) => Some(((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt()),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap_or(0.0)
        };
        let (Some((x1, y1)), Some((x2, y2))) = (p(c1), p(c2)) else { return 0.0 };
        let dist = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
        dist + m1 as f64 * r_of(c1) + m2 as f64 * r_of(c2)
    }
    /// Edges (lines or arcs) for which point `pid` is an endpoint. Used by click-on-corner fillets and
    /// chamfers.
    pub fn vertex_edges(&self, si: usize, pid: Id) -> Vec<Id> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        s.entities
            .iter()
            // construction lines are not edges of a corner: the diagonals of a rectangle end at its corners
            .filter(|e| !e.construction)
            .filter(|e| matches!(e.kind, EntityKind::Line { a, b } if a == pid || b == pid) || matches!(e.kind, EntityKind::Arc { a, b, .. } if a == pid || b == pid))
            .map(|e| e.id)
            .collect()
    }
    /// WHETHER AN EDGE STANDS AT A POINT, which is `vertex_edges` asked the other way round.
    ///
    /// Which of the lines of a point were named by a hand and which were let in by the point itself is decided by
    /// this: a line named by the hand takes its place among the lines of the point it arrives at.
    /// THE TWO POINTS AN EDGE STANDS ON, which is what the pairing of the chosen lines walks along.
    pub fn edge_ends(&self, si: usize, eid: Id) -> Option<(Id, Id)> {
        self.edge_end_ids(si, eid)
    }

    pub fn edge_stands_at(&self, si: usize, eid: Id, pid: Id) -> bool {
        self.edge_end_ids(si, eid).is_some_and(|(x, y)| x == pid || y == pid)
    }
    /// THE CORNER TWO EDGES SHARE: the point that is an end of both of them, when there is one.
    ///
    /// Two lines crossing without a point in common share no corner. That is the whole question the fillet and the
    /// chamfer ask of a chosen pair, and it is asked here once rather than at each place a pair is chosen.
    pub fn shared_vertex(&self, si: usize, e1: Id, e2: Id) -> Option<Id> {
        let (Some((a1, b1)), Some((a2, b2))) = (self.edge_end_ids(si, e1), self.edge_end_ids(si, e2)) else { return None };
        if a1 == a2 || a1 == b2 {
            Some(a1)
        } else if b1 == a2 || b1 == b2 {
            Some(b1)
        } else {
            None
        }
    }
    /// THE CORNER A NAMED PAIR OF EDGES MAKES: the point they meet at, where they make one.
    ///
    /// A shared point is not enough: two lines lying along ONE straight line through it share a vertex and no angle
    /// at all - there is nothing to round between them and nothing to cut. That pair names no corner, and a pair that
    /// names none is not an error: the search for a corner goes on with the second line as the first of the next pair.
    pub fn corner_of_pair(&self, si: usize, e1: Id, e2: Id) -> Option<Id> {
        let pid = self.shared_vertex(si, e1, e2)?;
        let (pcx, pcy) = self.point_xy(si, pid)?;
        let dir = |me: &Self, eid: Id| -> Option<(f64, f64)> {
            let (a, b) = me.edge_end_ids(si, eid)?;
            let other = if a == pid { b } else { a };
            let (ox, oy) = me.point_xy(si, other)?;
            let (dx, dy) = (ox - pcx, oy - pcy);
            let l = (dx * dx + dy * dy).sqrt();
            (l > 1e-9).then_some((dx / l, dy / l))
        };
        let (Some((x1, y1)), Some((x2, y2))) = (dir(self, e1), dir(self, e2)) else { return None };
        // parallel means one straight line: the angle between them is 180 degrees, and there is no corner in it
        ((x1 * y2 - y1 * x2).abs() > 1e-9).then_some(pid)
    }
    /// THE CORNERS A NAMED SET OF LINES MAKES: every pair of them that meets, two at a time, in the order they
    /// were named.
    ///
    /// **WHERE TWO OF THEM MEET THERE IS ONE CORNER**, and one pair is that corner — three lines of a triangle make
    /// three points and so three corners, which is the whole of a closed contour. A line that meets nothing of the
    /// set makes none: a line named on its own is half of the next corner, waiting for whatever answers it.
    ///
    /// **WHERE MORE THAN TWO OF THEM STAND ON ONE POINT** — a cross of four lines, a T-joint — every pair of them is
    /// a corner of that point, and taking all of them would round one place six times. So they are taken **as they
    /// were named, two at a time**: the first with the second, the third with the fourth, and a line left over alone
    /// stands there until another one is named beside it. That is what makes a third line joining a corner two lines
    /// have already made add nothing, and a fourth beside it make a second corner.
    ///
    /// The order is the rule, so it is the order the lines were named in and not the order a map hands them back:
    /// a set read off a hash would pair them by accident of the hashing, and the same drawing would round different
    /// corners on a second run.
    pub fn corners_of_lines(&self, si: usize, lines: &[Id]) -> Vec<(Id, Id)> {
        // THE LINES AS THEY WERE NAMED, one line named twice being one line: a line standing twice at the same
        // point would be paired with itself.
        let mut set: Vec<Id> = Vec::new();
        // a construction line is no side of a corner - the diagonal of a rectangle ends at its corners - so a picked one
        // takes no part in the pairing
        let construction = |me: &Self, l: Id| me.sketches.get(si).is_some_and(|s| s.entities.iter().any(|e| e.id == l && e.construction));
        for &l in lines {
            if !set.contains(&l) && !construction(self, l) {
                set.push(l);
            }
        }
        let here = |me: &Self, p: Id| -> Vec<usize> { (0..set.len()).filter(|&m| me.edge_end_ids(si, set[m]).is_some_and(|(x, y)| x == p || y == p)).collect() };
        let mut out: Vec<(Id, Id)> = Vec::new();
        let mut seen: Vec<Id> = Vec::new(); // the points already asked about, so a line asks about each of its ends once
        for &l in &set {
            let Some((x, y)) = self.edge_end_ids(si, l) else { continue };
            for p in [x, y] {
                if seen.contains(&p) {
                    continue;
                }
                seen.push(p);
                for two in here(self, p).chunks(2) {
                    // A PAIR LYING ALONG ONE STRAIGHT LINE SHARES ITS POINT AND MAKES NO ANGLE: it is not a corner,
                    // and naming it as one would offer a cut where there is nothing to cut.
                    if let [m, n] = two {
                        if self.corner_of_pair(si, set[*m], set[*n]).is_some() {
                            out.push((set[*m], set[*n]));
                        }
                    }
                }
            }
        }
        out
    }
    /// WHAT A CORNER WOULD LOOK LIKE IF THE VALUE WERE APPLIED: the point of the corner, where the two named
    /// edges will be cut, and the arc that will stand between them — a chamfer is a straight cut and has none.
    ///
    /// Nothing is changed here. The edges keep their ends and the point of the corner stays where it is: this is
    /// only what a person is shown before answering for it, drawn exactly as the operation will leave it — or not
    /// drawn at all when the value does not fit the corner.
    ///
    /// `None` when the pair is not a corner, when the value is not one, or when it is too big for the corner.
    pub fn corner_blend(&self, si: usize, pid: Id, (e1, e2): (Id, Id), cut: CornerCut) -> Option<CornerBlend> {
        // THE PAIR MUST BE THE CORNER: two lines that meet nowhere, or that lie along one straight line, are not a
        // corner and there is nothing to show where they are not.
        if self.corner_of_pair(si, e1, e2) != Some(pid) {
            return None;
        }
        let (pcx, pcy) = self.point_xy(si, pid)?;
        // the direction away from the corner along each edge, and how long that edge is
        let dir = |me: &Self, eid: Id| -> Option<(f64, f64, f64)> {
            let (a, b) = me.edge_end_ids(si, eid)?;
            let other = if a == pid { b } else { a };
            let (ox, oy) = me.point_xy(si, other)?;
            let (dx, dy) = (ox - pcx, oy - pcy);
            let l = (dx * dx + dy * dy).sqrt();
            (l > 1e-9).then_some((dx / l, dy / l, l))
        };
        let (Some((d1x, d1y, l1)), Some((d2x, d2y, l2))) = (dir(self, e1), dir(self, e2)) else { return None };
        let shorter = l1.min(l2);
        let theta = (d1x * d2x + d1y * d2y).clamp(-1.0, 1.0).acos();
        let at = |dx: f64, dy: f64, d: f64| [pcx + dx * d, pcy + dy * d];
        // THE MOST THE CORNER TAKES: a chamfer's legs short of the far end of each line, a radius whose touching points
        // stand at r / tan(theta / 2) from the corner for a fillet. Past this there is nothing to show.
        let value = match cut {
            CornerCut::Chamfer(legs) => {
                let (a, b) = legs.along(theta)?;
                if a >= l1 * (1.0 - 1e-9) || b >= l2 * (1.0 - 1e-9) {
                    return None;
                }
                return Some(CornerBlend { vertex: [pcx, pcy], ends: [at(d1x, d1y, a), at(d2x, d2y, b)], arc: None });
            }
            // the radius the size makes on this corner, whose arc turns through pi less the angle of its edges
            CornerCut::Fillet(size) => size.radius_on(std::f64::consts::PI - theta)?,
        };
        let limit = shorter * (theta / 2.0).tan();
        if value <= 1e-6 || !limit.is_finite() || value >= limit * (1.0 - 1e-9) {
            return None;
        }
        let half = (theta / 2.0).max(1e-6);
        let along = value / half.tan(); // where the arc touches each line
        let (bx, by) = (d1x + d2x, d1y + d2y);
        let bl = (bx * bx + by * by).sqrt();
        if bl < 1e-9 {
            return None;
        }
        let out = value / half.sin(); // the centre stands this far from the corner, along the bisector
        Some(CornerBlend { vertex: [pcx, pcy], ends: [at(d1x, d1y, along), at(d2x, d2y, along)], arc: Some(([pcx + bx / bl * out, pcy + by / bl * out], value)) })
    }
    /// THE TWO EDGES OF THE CORNER AT `pid`, out of every edge that meets there.
    ///
    /// Where exactly two edges meet, they are the corner and nothing is asked. Where more of them meet - two squares
    /// sharing a single point, four lines through it - there is a corner for every PAIR, and the pair is the two
    /// edges that bracket the point `near` (the cursor, in the drawing's own coordinates): the angle the cursor is
    /// standing in is the corner being pointed at. Without such a point (`None`) a vertex of more than two edges
    /// names no corner: which of them is meant is not knowable from the geometry alone, and the caller that only
    /// wants a bound asks `corner_limit`, which takes the tightest of them all.
    pub fn vertex_pair(&self, si: usize, pid: Id, near: Option<(f64, f64)>) -> Option<(Id, Id)> {
        let (pcx, pcy) = self.point_xy(si, pid)?;
        // each edge that meets at the vertex, with its direction AWAY from it
        let dirs: Vec<(Id, f64, f64)> = self
            .vertex_edges(si, pid)
            .into_iter()
            .filter_map(|eid| {
                let (a, b) = self.edge_end_ids(si, eid)?;
                let other = if a == pid { b } else { a };
                let (ox, oy) = self.point_xy(si, other)?;
                let (dx, dy) = (ox - pcx, oy - pcy);
                let l = (dx * dx + dy * dy).sqrt();
                (l > 1e-9).then_some((eid, dx / l, dy / l))
            })
            .collect();
        if dirs.len() < 2 {
            return None;
        }
        if dirs.len() == 2 {
            return self.corner_of_pair(si, dirs[0].0, dirs[1].0).map(|_| (dirs[0].0, dirs[1].0));
        }
        let (nx, ny) = near?;
        let (tx, ty) = (nx - pcx, ny - pcy);
        if (tx * tx + ty * ty).sqrt() < 1e-9 {
            return None; // the cursor stands ON the vertex: no side of it to read
        }
        // sorted by the angle of the direction; the pair that brackets the cursor is the one before it and the
        // one after it, round the circle
        let mut v: Vec<(Id, f64)> = dirs.iter().map(|(id, ux, uy)| (*id, uy.atan2(*ux))).collect();
        v.sort_by(|a, b| a.1.total_cmp(&b.1));
        let at = v.partition_point(|(_, a)| *a <= ty.atan2(tx)) % v.len();
        let pair = (v[(at + v.len() - 1) % v.len()].0, v[at].0);
        // the two sides bracketing the cursor are a corner unless they lie along one straight line
        self.corner_of_pair(si, pair.0, pair.1).map(|_| pair)
    }
    /// THE CORNER OF ONE NAMED EDGE AT `pid`, on the side the point `near` stands.
    ///
    /// Where several edges meet at one point every pair of them is a corner, and `vertex_pair` lets the cursor say
    /// which. Where the corner has been named by an edge AND the point together there are fewer corners to say it
    /// about - only the ones this edge takes part in - and the cursor says which of them by the sector it stands in:
    /// the two corners of an edge are the two sectors either side of it.
    ///
    /// Without a point to read (`near` is `None` - the pointer is not over the sheet, which happens while the value
    /// is being typed) the neighbour after it stands. The drawing has to begin somewhere, and a corner is as good a
    /// beginning as any; refusing outright would leave a field open with nothing behind it.
    pub fn vertex_pair_through(&self, si: usize, pid: Id, eid: Id, near: Option<(f64, f64)>) -> Option<(Id, Id)> {
        let (pcx, pcy) = self.point_xy(si, pid)?;
        // every edge at the point with the angle of its direction away from it, in the turn of the circle
        let mut v: Vec<(Id, f64)> = self
            .vertex_edges(si, pid)
            .into_iter()
            .filter_map(|e| {
                let (a, b) = self.edge_end_ids(si, e)?;
                let other = if a == pid { b } else { a };
                let (ox, oy) = self.point_xy(si, other)?;
                let (dx, dy) = (ox - pcx, oy - pcy);
                let l = (dx * dx + dy * dy).sqrt();
                (l > 1e-9).then_some((e, dy.atan2(dx)))
            })
            .collect();
        if v.len() < 2 {
            return None; // one edge at the point is not a corner of anything
        }
        v.sort_by(|a, b| a.1.total_cmp(&b.1));
        let at = v.iter().position(|(id, _)| *id == eid)?;
        let len = v.len();
        // THE TWO SECTORS THIS EDGE BOUNDS, one on either side of it in the turn of the circle, each measured as the
        // turn from the edge named so that neither breaks where the angles run out
        let a0 = v[at].1;
        let up = {
            let x = v[(at + 1) % len].1;
            if x > a0 {
                x - a0
            } else {
                x + std::f64::consts::TAU - a0
            }
        };
        let down = {
            let x = v[(at + len - 1) % len].1;
            if x < a0 {
                a0 - x
            } else {
                a0 + std::f64::consts::TAU - x
            }
        };
        // THE SECTOR THE CURSOR STANDS IN, and where it stands in neither, the one it is nearer the middle of - the
        // pointer may be in a sector that belongs to two other edges altogether. The two are equally far apart when
        // the cursor is opposite the edge named, and the one after it in the turn of the circle stands then.
        let forward = near.is_none_or(|(nx, ny)| {
            let mut d = (ny - pcy).atan2(nx - pcx) - a0;
            if d < 0.0 {
                d += std::f64::consts::TAU;
            }
            (d - up / 2.0).abs() <= (d - (std::f64::consts::TAU - down / 2.0)).abs()
        });
        let other = if forward { v[(at + 1) % len].0 } else { v[(at + len - 1) % len].0 };
        let pair = (eid, other);
        self.corner_of_pair(si, pair.0, pair.1).map(|_| pair)
    }
    /// EVERY PAIR OF EDGES THAT COULD BE THE CORNER at `pid`, for the bounds that do not name a pair.
    pub fn vertex_pairs(&self, si: usize, pid: Id) -> Vec<(Id, Id)> {
        let edges = self.vertex_edges(si, pid);
        let mut out = Vec::new();
        for i in 0..edges.len() {
            for j in (i + 1)..edges.len() {
                out.push((edges[i], edges[j]));
            }
        }
        out
    }
    /// Chamfer the corner at vertex `pid`, where exactly two lines meet. Returns whether it succeeded.
    ///
    /// THE FIRST LINE is the one `toward` stands nearer to - the side of the corner that was clicked: the first leg is
    /// laid along it, and for a leg and an angle the angle is measured from it. With no point given, the line drawn
    /// first.
    pub fn chamfer_at_vertex(&mut self, si: usize, pid: Id, legs: ChamferLegs, toward: Option<Point2>) -> bool {
        let Some(pair) = self.vertex_pair(si, pid, None) else { return false };
        self.chamfer_lines_of_pair(si, pair, legs, toward)
    }
    /// Chamfer the corner at `pid` that the point `(x, y)` stands in, where more than two edges meet there.
    pub fn chamfer_at_vertex_near(&mut self, si: usize, pid: Id, legs: ChamferLegs, x: f64, y: f64, toward: Option<Point2>) -> bool {
        let Some(pair) = self.vertex_pair(si, pid, Some((x, y))) else { return false };
        self.chamfer_lines_of_pair(si, pair, legs, toward)
    }
    /// The cut of `legs` off the corner the two named edges make, both of them straight: the chamfer of a sketch is a
    /// straight cut, and an arc in the pair is refused rather than cut as if it were a line.
    pub fn chamfer_lines_of_pair(&mut self, si: usize, (e1, e2): (Id, Id), legs: ChamferLegs, toward: Option<Point2>) -> bool {
        if self.corner_of_pair(si, e1, e2).is_none() {
            return false; // two lines in one straight line: nothing is cut, and the cut would be along them
        }
        let both_lines = [e1, e2].iter().all(|&eid| matches!(self.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).map(|e| e.kind), Some(EntityKind::Line { .. })));
        if !both_lines {
            return false;
        }
        // the line whose direction from the corner leans nearer to the click is the first
        let lean = |me: &Self, eid: Id| -> f64 {
            let Some(toward) = toward else { return 0.0 };
            let Some(shared) = me.corner_of_pair(si, e1, e2) else { return f64::MAX };
            let (Some((a, b)), Some((px, py))) = (me.line_ends(si, eid), me.point_xy(si, shared)) else { return f64::MAX };
            let far = if a == shared { b } else { a };
            let Some((fx, fy)) = me.point_xy(si, far) else { return f64::MAX };
            let (ux, uy, vx, vy) = (fx - px, fy - py, toward.x - px, toward.y - py);
            -(ux * vx + uy * vy) / (ux.hypot(uy) * vx.hypot(vy)).max(1e-12)
        };
        let (first, second) = if lean(self, e2) < lean(self, e1) { (e2, e1) } else { (e1, e2) };
        self.chamfer_lines(si, first, second, legs)
    }
    /// Connected shape: every entity reachable from `eid` through shared endpoints — a rectangle from one of
    /// its sides, a chain of lines and arcs. A circle or an ellipse stands alone. Used by "fillet all" followed
    /// by a click on a shape.
    pub fn connected_entities(&self, si: usize, eid: Id) -> std::collections::HashSet<Id> {
        let mut out: std::collections::HashSet<Id> = std::collections::HashSet::new();
        let Some(s) = self.sketches.get(si) else { return out };
        let ends = |e: &SketchEntity| -> Vec<Id> {
            match e.kind {
                EntityKind::Line { a, b } => vec![a, b],
                EntityKind::Arc { a, b, .. } => vec![a, b],
                _ => Vec::new(),
            }
        };
        let mut queue = vec![eid];
        while let Some(cur) = queue.pop() {
            if !out.insert(cur) {
                continue;
            }
            let Some(ce) = s.entities.iter().find(|e| e.id == cur) else { continue };
            let cends = ends(ce);
            if cends.is_empty() {
                continue; // A circle or an ellipse stands alone.
            }
            for e in &s.entities {
                if !out.contains(&e.id) && ends(e).iter().any(|id| cends.contains(id)) {
                    queue.push(e.id);
                }
            }
        }
        out
    }
    /// Fillet the corners of the selected geometry: a corner is taken only when both of its edges are in
    /// `only` (`None` means the whole sketch). "Fillet all" with a shape selected fillets that shape alone.
    pub fn fillet_all_corners_of(&mut self, si: usize, r: f64, only: Option<&std::collections::HashSet<Id>>) -> usize {
        self.fillet_all_corners_by(si, FilletSize::radius(r), only)
    }
    /// The same with a size given by a radius, a chord or an arc length: every corner gets the radius the size makes on
    /// it, and keeps the size as it was given (see `fillet_at_vertex_by`).
    pub fn fillet_all_corners_by(&mut self, si: usize, size: FilletSize, only: Option<&std::collections::HashSet<Id>>) -> usize {
        let corners: Vec<Id> = {
            let Some(s) = self.sketches.get(si) else { return 0 };
            let mut count: std::collections::HashMap<Id, usize> = std::collections::HashMap::new();
            for e in &s.entities {
                if e.construction || only.is_some_and(|f| !f.contains(&e.id)) {
                    continue;
                }
                let ends = match e.kind {
                    EntityKind::Line { a, b } => Some((a, b)),
                    EntityKind::Arc { a, b, .. } => Some((a, b)),
                    _ => None,
                };
                if let Some((a, b)) = ends {
                    *count.entry(a).or_default() += 1;
                    *count.entry(b).or_default() += 1;
                }
            }
            count.into_iter().filter(|&(_, c)| c == 2).map(|(id, _)| id).collect()
        };
        // every corner laid, the sketch rebuilt once (`batch`)
        self.batch(|p| {
            let mut done = 0;
            for pid in corners {
                // Is the vertex still intact, with two edges still meeting there?
                if p.sketches.get(si).is_some_and(|s| s.points.iter().any(|q| q.id == pid)) && p.vertex_edges(si, pid).len() == 2 && p.fillet_at_vertex_by(si, pid, size) {
                    done += 1;
                }
            }
            done
        })
    }
    /// Offset the selected entities: their closed loops are moved by `dist`, inwards or outwards, and added as
    /// new entities.
    pub fn offset_entities(&mut self, si: usize, eids: &[Id], dist: f64) -> usize {
        let (pts, ents) = {
            let Some(s) = self.sketches.get(si) else { return 0 };
            let ents: Vec<SketchEntity> = s.entities.iter().filter(|e| eids.contains(&e.id)).copied().collect();
            (s.points.clone(), ents)
        };
        let mut count = 0;
        // A circle entity gives a concentric circle (r plus or minus dist) rather than a polygon, HELD TO ITS SOURCE: the
        // centres concentric and the radii apart by the distance, a dimension of the offset. Laid free, the copy was a
        // circle of its own - five freedoms where the pair has two, and moving or resizing the source left it behind.
        for e in &ents {
            if let EntityKind::Circle { center, r } = e.kind {
                if let Some(c) = pts.iter().find(|p| p.id == center) {
                    let nr = r + dist;
                    if nr > 0.05 {
                        let copy = self.add_circle_entity(si, c.x, c.y, nr, crate::feature::Purpose::of(e.construction));
                        let copy_centre = self.sketches[si].entities.iter().find(|x| x.id == copy).and_then(|x| match x.kind {
                            EntityKind::Circle { center, .. } => Some(center),
                            _ => None,
                        });
                        if let Some(cc) = copy_centre.filter(|cc| *cc != center) {
                            let (m1, m2) = if dist > 0.0 { (-1, 1) } else { (1, -1) }; // r(copy) - r(source) = dist
                            self.sketches[si].constraints.push(Constraint::Concentric { c1: center, c2: cc });
                            self.sketches[si].constraints.push(Constraint::EdgeDistance { c1: center, c2: cc, d: dist.abs(), m1, m2, off: 0.0, expr: String::new(), driven: false, at: None });
                        }
                        count += 1;
                    }
                }
            }
        }
        // Lines and arcs are collected into bulge loops, offset with the arcs preserved, and reassembled into
        // lines and arcs - HELD TO THEIR SOURCE, as the circle is. Laid free, a rectangle's copy had eight freedoms of
        // its own, and resizing the rectangle left the copy where it was. See `hold_offset_loop` for how.
        let src_lines: Vec<SourceLine> = ents
            .iter()
            .filter(|e| !e.construction)
            .filter_map(|e| match e.kind {
                EntityKind::Line { a, b } => Some(SourceLine { a, b, pa: at_of(&pts, a)?, pb: at_of(&pts, b)? }),
                _ => None,
            })
            .collect();
        let src_arcs: Vec<SourceArc> = ents
            .iter()
            .filter(|e| !e.construction)
            .filter_map(|e| match e.kind {
                EntityKind::Arc { center, a, b, .. } => {
                    let (c, p) = (at_of(&pts, center)?, at_of(&pts, a)?);
                    Some(SourceArc { center, a, b, at: c, radius: (p.0 - c.0).hypot(p.1 - c.1) })
                }
                _ => None,
            })
            .collect();
        for loop_v in entity_bulge_loops(&pts, &ents) {
            for oloop in crate::offset::offset_bulge(&loop_v, dist) {
                let n = oloop.len();
                if n < 2 {
                    continue;
                }
                let ids: Vec<Id> = oloop.iter().map(|v| self.sketch_point_at(si, v.x, v.y, 1e-6)).collect();
                let mut kinds: Vec<(EntityKind, OffsetSource)> = Vec::new();
                for k in 0..n {
                    let (v, w) = (oloop[k], oloop[(k + 1) % n]);
                    let (a, b) = (ids[k], ids[(k + 1) % n]);
                    if v.bulge.abs() < 1e-9 {
                        kinds.push((EntityKind::Line { a, b }, offset_line_source(&src_lines, (v.x, v.y), (w.x, w.y), dist)));
                    } else {
                        let (cx, cy, ccw) = arc_center_from_bulge(v.x, v.y, w.x, w.y, v.bulge);
                        // a centre of its own even where the source's stands: the radius of an arc is kept by its centre
                        let center = self.alloc_id();
                        self.sketches[si].points.push(SketchPoint { id: center, x: cx, y: cy });
                        let rc = (v.x - cx).hypot(v.y - cy);
                        kinds.push((EntityKind::Arc { center, a, b, ccw }, offset_arc_source(&src_arcs, &src_lines, (cx, cy), rc, dist)));
                    }
                }
                let held = hold_offset_loop(&ids, &kinds, dist);
                for (kind, _) in kinds {
                    let id = self.alloc_id();
                    self.sketches[si].entities.push(SketchEntity { id, kind, construction: false });
                }
                self.sketches[si].constraints.extend(held);
                count += 1;
            }
        }
        if count > 0 {
            self.regen_sketch(si);
        }
        count
    }
    /// Value of a named driving dimension: the `Distance` constraint of the sketch is looked up by its points
    /// (a and b, in either order) and its `d` is read. `None` when the sketch or the constraint was not
    /// found.
    pub fn named_dim_value(&self, nd: &NamedDim) -> Option<f64> {
        self.dim_target_value(&nd.target)
    }

    /// Value of whatever was named: a sketch dimension or a feature parameter.
    ///
    /// For a feature parameter the value is taken the way the rebuild takes it: an expression in `feat_dims` is
    /// evaluated, otherwise the stored number from the node itself is used. Evaluation goes against the global
    /// parameters rather than the whole scope, or a driver evaluating itself would recurse forever through
    /// `param_map`.
    pub fn dim_target_value(&self, target: &crate::model::DimTarget) -> Option<f64> {
        match target {
            crate::model::DimTarget::Sketch { sketch, refs } => {
                let si = self.sketch_index(*sketch)?;
                let want: std::collections::BTreeSet<crate::model::Id> = refs.iter().copied().collect();
                self.sketches[si].constraints.iter().find_map(|c| {
                    let r: std::collections::BTreeSet<crate::model::Id> = Project::dim_refs(c)?.into_iter().collect();
                    (r == want).then(|| Project::dim_value_of(c)).flatten()
                })
            }
            crate::model::DimTarget::Feature { node, key } => {
                if let Some(e) = self.feat_dim(*node, key) {
                    if !e.trim().is_empty() {
                        let vars: std::collections::HashMap<String, f64> = self.parameters.iter().filter(|p| !p.name.is_empty()).map(|p| (p.name.clone(), p.value)).collect();
                        return crate::expr::eval(e, &vars).ok();
                    }
                }
                self.timeline.iter().find(|n| n.id == *node)?.kind.dim(key)
            }
        }
    }

    /// Returns the remaining residual; zero means the constraints are satisfied.
    /// Sketch diagnostics: the residual of each constraint, in `constraints` order, so the interface can show
    /// which dimension is unsatisfied rather than one number for the whole sketch.
    pub fn sketch_residuals(&self, si: usize) -> Vec<f64> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let radii = self.entity_radii(si);
        let active: Vec<Constraint> = s.constraints.clone();
        crate::solver::residual_per_constraint(&s.points, &radii, &active)
    }
    /// Conflicting constraints of a sketch: the indices of constraints that contradict each other, so the
    /// system cannot be satisfied until one of them is removed or made a reference dimension.
    ///
    /// This is a set of disagreeing constraints rather than "the dimensions that are currently unsatisfied":
    /// geometric constraints can disagree too (a horizontal against an angle), and no single constraint in the
    /// set is at fault — any one of them may be removed.
    ///
    /// A heuristic cannot answer this. Comparing each dimension against the measured geometry and reddening it
    /// when the value misses by more than a scale-derived tolerance fails on two contradictory dimensions: the
    /// solver puts the geometry between them, so either both miss or, if the compromise lands within tolerance,
    /// neither does. The answer comes from rank analysis (`solver::conflicts`) instead: a row whose coefficients
    /// on the variables cancelled out while the residual remained is a linear combination of constraints with an
    /// inconsistent right-hand side, and the constraints entering it are the disagreeing set.
    ///
    /// What is guaranteed: the set is a resolution set. Removing any of its members resolves the conflict
    /// (asserted by the test `geometric_constraints_conflict_too`), so the advice "remove any of the red ones"
    /// is honest.
    ///
    /// What is not: the analysis is linear and local to the current geometry, so a constraint that holds this
    /// position from afar — removing it would help, but only after a global rearrangement such as rotating a
    /// segment by 90 degrees — will not appear in the set. That is a limitation of any local method; the same
    /// situation is otherwise handled by refusing to add a constraint that over-constrains the sketch, which is
    /// `add_constraint_if_independent` while drawing.
    ///
    /// The system analysed is exactly the one the solver solves: the user constraints plus the arc intrinsics
    /// (endpoints on the circle of their own radius). Without the intrinsics the analysis runs against a
    /// different system than the solution: arcs are missing some of their equations, and a conflict involving
    /// them is either not found or blamed on an innocent constraint. Only user indices are returned, an
    /// intrinsic being part of the geometry itself and impossible to delete.
    pub fn sketch_conflicts(&self, si: usize) -> Vec<usize> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let radii = self.entity_radii(si);
        let nuser = s.constraints.len();
        let mut all: Vec<Constraint> = s.constraints.clone();
        all.extend(self.entity_intrinsics(si));
        let mut out = s.checked.with(|memo| crate::solver::conflicts_remembered(&s.points, &radii, &all, memo));
        out.retain(|&ci| ci < nuser);
        // Unevaluable constraints go red too. The solver rejects them (otherwise they silently hold nothing),
        // but a silent rejection is no better than a silent no-op: the point still looks constrained. A
        // constraint with nothing to measure against therefore enters the diagnostics.
        let centers: std::collections::HashSet<Id> = radii.iter().map(|r| r.center).collect();
        for (ci, c) in s.constraints.iter().enumerate() {
            if let Constraint::PointOnCircle { c: cc, .. } = c {
                if !centers.contains(cc) && !out.contains(&ci) {
                    out.push(ci);
                }
            }
        }
        out
    }
    pub(super) fn solve_sketch_inner(&mut self, si: usize, drag: Option<(Id, f64, f64)>, budget: crate::solver::Budget, rebuild: Rebuild) -> f64 {
        // The radius variables and the implicit arc constraints are computed before the mutable borrow.
        let mut radii = self.entity_radii(si);
        let intrinsics = self.entity_intrinsics(si);
        let Some(s) = self.sketches.get_mut(si) else { return 0.0 };
        // where the points stood before anything of this solve moved them, for what it rebuilds after it
        let at_start: Vec<(f64, f64)> = s.points.iter().map(|p| (p.x, p.y)).collect();
        // THE CENTRE OF A RECTANGLE DRAWN BY ITS CORNERS FOLLOWS THE CORNERS: it is put on the middle of the diagonal
        // before the solve. Left where it stood, it held the corners back - a side moved from 20 to 30 came out at 28.9,
        // the solver sharing the move between the corners and the centre. A centre a rectangle was drawn from is its
        // anchor and stays.
        struct Mid {
            centre: Id,
            at: Point2,
        }
        // how many constraints name each centre, and where each point stands, counted once: asked of every constraint for
        // every rectangle and looked along the points, 10 000 rectangles drawn were 6e8 steps a solve
        let corner_drawn: Vec<&crate::model::SketchRect> = s.rects.iter().filter(|r| matches!(r.anchor, crate::model::RectAnchor::Corner(_))).collect();
        let mut naming: std::collections::HashMap<Id, usize> = corner_drawn.iter().map(|r| (r.centre, 0)).collect();
        if !naming.is_empty() {
            for c in &s.constraints {
                for p in c.points() {
                    if let Some(n) = naming.get_mut(&p) {
                        *n += 1;
                    }
                }
            }
        }
        let standing: std::collections::HashMap<Id, Point2> = if corner_drawn.is_empty() { Default::default() } else { s.points.iter().map(|q| (q.id, Point2::new(q.x, q.y))).collect() };
        let mids: Vec<Mid> = corner_drawn
            .into_iter()
            // only a centre nothing else holds: one that carries a constraint of its own is solved with it, and put back on
            // the middle before every solve of a contradicting sketch it was a new compromise each time - a point drifted
            // by 1 mm from one solve to the next
            .filter(|r| naming.get(&r.centre) == Some(&1))
            .filter_map(|r| {
                let at = |id: Id| standing.get(&id).copied();
                let (a, c) = (at(r.corners[0])?, at(r.corners[2])?);
                Some(Mid { centre: r.centre, at: Point2::new((a.x + c.x) / 2.0, (a.y + c.y) / 2.0) })
            })
            .collect();
        if !mids.is_empty() {
            let place: std::collections::HashMap<Id, usize> = s.points.iter().enumerate().map(|(i, q)| (q.id, i)).collect();
            for m in mids {
                if let Some(&i) = place.get(&m.centre) {
                    (s.points[i].x, s.points[i].y) = (m.at.x, m.at.y);
                }
            }
        }
        // A SHAPE DRAGGED BY ITS CENTRE GOES WITH IT AS A WHOLE, as a circle goes with its centre: what the centre
        // carries (`Sketch::carried_with` - the corners of a rectangle, the ends of an arc, a slot, the axis ends of an
        // ellipse) is moved by the move of the centre before the solve. Left where they stood, the drag pulled the
        // centre alone: the middle of a rectangle's diagonal pulled it back by the two corners on it, and an ellipse
        // turned and changed shape about its axis ends.
        if let Some((d, tx, ty)) = drag {
            let carried: Vec<Id> = s.carried_with(d);
            if let Some((cx, cy)) = s.points.iter().find(|q| q.id == d).map(|q| (q.x, q.y)) {
                if !carried.is_empty() {
                    let held = s.held_points();
                    let (dx, dy) = (tx - cx, ty - cy);
                    for q in s.points.iter_mut().filter(|q| (carried.contains(&q.id) || q.id == d) && !held.contains(&q.id)) {
                        q.x += dx;
                        q.y += dy;
                    }
                }
            }
        }
        // Reference (driven) dimensions do not constrain the geometry and are excluded from the solver, while
        // the arc intrinsics (endpoints on the circle of radius R) are always active.
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|c| !c.is_driven()).cloned().collect();
        active.extend(intrinsics);
        let (was, radii_was): (Vec<(f64, f64)>, Vec<f64>) = (s.points.iter().map(|p| (p.x, p.y)).collect(), radii.iter().map(|r| r.value).collect());
        // A RECTANGLE CHANGES ITS SIZE FROM WHERE IT WAS DRAWN: from the centre a rectangle drawn from the centre, from the
        // first corner one drawn by its corners. The solver alone spreads a change over every point it may move, so a
        // dragged corner or a width typed grew a rectangle about wherever the least travel lay. What is held for this
        // solve (`held_for_size`) is let go when the sketch does not solve with it - a dimension that moves the
        // rectangle as a whole.
        // A CORNER DRAGGED OF A RECTANGLE DRAWN FROM A CORNER asks whether its centre is held - counted only then, as a
        // drag frame of a sketch the drag session does not take is solved here
        let dragged = drag.map(|(d, _, _)| d);
        let corner_dragged = dragged.is_some_and(|d| s.rects.iter().any(|r| matches!(r.anchor, crate::model::RectAnchor::Corner(_)) && r.corners.contains(&d)));
        let free: std::collections::HashMap<Id, bool> = if corner_dragged {
            s.points.iter().map(|q| q.id).zip(crate::solver::free_points(&s.points, &radii, &active)).collect()
        } else {
            std::collections::HashMap::new()
        };
        let centre_of = |r: &crate::model::SketchRect| if free.get(&r.centre) == Some(&false) { Centre::Held } else { Centre::Free };
        let anchors: Vec<Constraint> =
            s.rects.iter().filter_map(|r| held_for_size(r, dragged, centre_of(r))).filter(|p| s.points.iter().any(|q| q.id == *p)).map(|p| Constraint::Fixed { p }).collect();
        let outcome = if anchors.is_empty() {
            crate::solver::solve_within(&mut s.points, &mut radii, &active, drag, budget)
        } else {
            let (mut held_points, mut held_radii) = (s.points.clone(), radii.clone());
            let held: Vec<Constraint> = active.iter().cloned().chain(anchors).collect();
            let with_hold = crate::solver::solve_within(&mut held_points, &mut held_radii, &held, drag, budget);
            // SOLVED IS TOLD BY THE SKETCH'S OWN CONSTRAINTS, not by the hold, a soft pull of which a little is always left
            // under a corner dragged away from it. Solved means to the precision the polish reaches (1e-12): a side held
            // 0.0009 short of a collinear line by its anchor left 1e-7 and passed at 1e-6. A DRAG FRAME KEEPS THE HOLD
            // whatever it leaves: a frame is a compromise with the pointer until the release solves it, and the
            // compromise grows with how far the pointer is from where the corner can go - a corner pulled 8 mm up off
            // its horizontal side left 2e-2, and a frame let go of the hold carried the corner drawn from 7 mm with it.
            // The release is solved in full, and lets the hold go if the sketch does not solve with it.
            let r = crate::solver::residual_per_constraint(&held_points, &held_radii, &active).iter().map(|v| v * v).sum::<f64>().sqrt();
            let solved = if drag.is_some() { f64::INFINITY } else { 1e-9 };
            if r <= solved {
                (s.points, radii) = (held_points, held_radii);
                crate::solver::Outcome { residual: r, left: with_hold.left }
            } else {
                crate::solver::solve_within(&mut s.points, &mut radii, &active, drag, budget)
            }
        };
        // NOT SOLVED TO THE END: the parts the time ran out before, said beside the degrees of freedom of the sketch
        s.left_unsolved = outcome.left;
        let resid = outcome.residual;
        // A SOLVED SKETCH SOLVED AGAIN STAYS AS IT WAS: a move below rounding is not written. Each solve of a solved
        // polygon shifted its points by about 2e-18 mm, the document key read every frame as an edit, and a rebuild
        // in the background always came back stale and was started again - the program never came to rest.
        let same = |new: f64, old: f64| (new - old).abs() <= 1e-12 * old.abs().max(1.0);
        for (p, &(x, y)) in s.points.iter_mut().zip(&was) {
            if same(p.x, x) && same(p.y, y) {
                (p.x, p.y) = (x, y);
            }
        }
        for (r, &old) in radii.iter_mut().zip(&radii_was) {
            if same(r.value, old) {
                r.value = old;
            }
        }
        // The solved radii go back into the circles; an arc derives its radius from its points and stores
        // none. By the centre, from a table: every radius against every entity was 4.9e9 steps a solve on 70 000
        // circles.
        let solved: std::collections::HashMap<Id, f64> = radii.iter().map(|rv| (rv.center, rv.value)).collect();
        for e in s.entities.iter_mut() {
            if let EntityKind::Circle { center, r } = &mut e.kind {
                if let Some(&v) = solved.get(center) {
                    *r = v;
                }
            }
        }
        // what the solve moved: the points and the centres of the circles whose radius changed
        let moved: std::collections::HashSet<Id> = s
            .points
            .iter()
            .zip(&at_start)
            .filter(|(p, &(x, y))| p.x.to_bits() != x.to_bits() || p.y.to_bits() != y.to_bits())
            .map(|(p, _)| p.id)
            .chain(radii.iter().zip(&radii_was).filter(|(r, &old)| r.value.to_bits() != old.to_bits()).map(|(r, _)| r.center))
            .collect();
        self.update_driven_dims(si);
        match rebuild {
            Rebuild::Whole => self.regen_sketch(si),
            Rebuild::Moved => self.regen_sketch_moved(si, &moved),
        }
        resid
    }
    /// Radius variables of the solver: circles (which store `r`) and arcs (whose radius is the distance from
    /// the centre to endpoint `a`), keyed by centre id. This makes the radius a real variable for degrees of
    /// freedom, tangency and equal-radius constraints.
    pub(super) fn entity_radii(&self, si: usize) -> Vec<crate::solver::RadiusVar> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let pos: std::collections::HashMap<Id, (f64, f64)> = s.points.iter().map(|p| (p.id, (p.x, p.y))).collect();
        let mut out = Vec::new();
        for e in &s.entities {
            match e.kind {
                EntityKind::Circle { center, r } => out.push(crate::solver::RadiusVar { center, value: r }),
                EntityKind::Arc { center, a, .. } => {
                    if let (Some(&(cx, cy)), Some(&(ax, ay))) = (pos.get(&center), pos.get(&a)) {
                        // `hypot`, as the drag session reads it (`drag_session`): the root of the squares rounds the last
                        // bit apart from it, and a frame kept by the drag and the frame solved alone came out different
                        let rr = (ax - cx).hypot(ay - cy).max(0.001);
                        out.push(crate::solver::RadiusVar { center, value: rr });
                    }
                }
                _ => {}
            }
        }
        out
    }
    /// Implicit constraints of curve entities, neither stored nor deletable: an arc keeps both endpoints on the
    /// circle of its centre (with radius R as a variable), which makes the arc a real entity; an ellipse keeps
    /// its semi-axes perpendicular (c to ma against c to mi), which makes the ellipse a real entity with five
    /// degrees of freedom.
    pub(super) fn entity_intrinsics(&self, si: usize) -> Vec<Constraint> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let mut out = Vec::new();
        let projected = s.projected_entities();
        for e in &s.entities {
            match e.kind {
                // an arc of a projection has its centre and both ends pinned by the body: one end on the circle sets the
                // radius, the other would say it again - a fillet of the body projected showed one redundant
                EntityKind::Arc { center, a, .. } if projected.contains(&e.id) => {
                    out.push(Constraint::PointOnCircle { p: a, c: center });
                }
                EntityKind::Arc { center, a, b, .. } => {
                    out.push(Constraint::PointOnCircle { p: a, c: center });
                    out.push(Constraint::PointOnCircle { p: b, c: center });
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    out.push(Constraint::Perpendicular { a: c, b: ma, c, d: mi });
                }
                _ => {}
            }
        }
        // Projected geometry is driven by the body: its points and circle radii are given, not unknowns.
        // Without this a projected edge counted 4 degrees of freedom and a box face outline 8, and a solve
        // was free to drag the projection off the part.
        // A shared corner occupies several slots of `points`; it is pinned once, or the duplicates would
        // count as redundant constraints (8 on a box face).
        let mut pinned = std::collections::HashSet::new();
        for pr in &s.projections {
            out.extend(pr.points.iter().filter(|&&p| pinned.insert(p)).map(|&p| Constraint::Fixed { p }));
            for e in s.entities.iter().filter(|e| pr.entities.contains(&e.id)) {
                if let EntityKind::Circle { center, r } = e.kind {
                    out.push(Constraint::Diameter { c: center, d: r, off: 0.0, expr: String::new(), driven: false, diam: false, at: None });
                }
            }
        }
        out
    }
    /// Find or create a diameter or radius dimension for the circle entity with centre `c`. Returns the
    /// constraint index; `diam` selects whether it is displayed as a diameter by default.
    pub fn ensure_diameter(&mut self, si: usize, c: Id, diam: bool) -> Option<usize> {
        let s = self.sketches.get(si)?;
        if let Some(i) = s.constraints.iter().position(|x| matches!(x, Constraint::Diameter { c: cc, .. } if *cc == c)) {
            return Some(i);
        }
        // The initial value is the current radius of the circle.
        let r = s.entities.iter().find_map(|e| match e.kind {
            EntityKind::Circle { center, r } if center == c => Some(r),
            _ => None,
        })?;
        let d = if diam { 2.0 * r } else { r };
        let s = self.sketches.get_mut(si)?;
        s.constraints.push(Constraint::Diameter { c, d, off: 0.0, expr: String::new(), driven: false, diam, at: None });
        Some(s.constraints.len() - 1)
    }
    /// Find or create a driving arc-length dimension for arc entity `arc_eid`. Returns its index.
    pub fn ensure_arc_length(&mut self, si: usize, arc_eid: Id) -> Option<usize> {
        let s = self.sketches.get(si)?;
        let (c, a, b, ccw) = s.entities.iter().find(|e| e.id == arc_eid).and_then(|e| match e.kind {
            EntityKind::Arc { center, a, b, ccw } => Some((center, a, b, ccw)),
            _ => None,
        })?;
        if let Some(i) = s.constraints.iter().position(|x| matches!(x, Constraint::ArcLength { c: cc, a: ca, b: cb, .. } if *cc == c && *ca == a && *cb == b)) {
            return Some(i);
        }
        let pos = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let ((cx, cy), (ax, ay), (bx, by)) = (pos(c)?, pos(a)?, pos(b)?);
        let rad = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
        let (a0, a1) = ((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx));
        let theta = if ccw { (a1 - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - a1).rem_euclid(std::f64::consts::TAU) };
        let len = rad * theta;
        let s = self.sketches.get_mut(si)?;
        s.constraints.push(Constraint::ArcLength { c, a, b, ccw, len, off: 0.0, expr: String::new(), driven: false });
        Some(s.constraints.len() - 1)
    }
    /// Update the values of the reference (driven) dimensions from the current geometry: they measure rather
    /// than constrain.
    pub(super) fn update_driven_dims(&mut self, si: usize) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        let pos: std::collections::HashMap<Id, (f64, f64)> = s.points.iter().map(|p| (p.id, (p.x, p.y))).collect();
        // Circle radii by centre, for reference diameter and radius dimensions.
        let crad: std::collections::HashMap<Id, f64> = s
            .entities
            .iter()
            .filter_map(|e| match e.kind {
                EntityKind::Circle { center, r } => Some((center, r)),
                _ => None,
            })
            .collect();
        for c in &mut s.constraints {
            measure_driven(c, &pos, &crad);
        }
    }
    /// Whether a sketch is typed: built from points the sketcher edits - its entities, or its splines, which are
    /// kept apart from the entities. A sketch of splines alone is typed too; read from the entities only, its nodes
    /// could not be dragged.
    pub fn is_typed_sketch(&self, si: usize) -> bool {
        self.sketches.get(si).is_some_and(|s| !s.entities.is_empty() || !s.splines.is_empty())
    }
    /// THE LOOPS OF A SKETCH MADE FROM ALL ITS CURVES, whatever changed since they were made: what a rebuild round a
    /// change (`round`) is held to.
    pub fn regen_sketch_whole(&mut self, si: usize) {
        if let Some(s) = self.sketches.get_mut(si) {
            s.laid = None;
        }
        self.regen_sketch(si);
    }

    /// Rebuild the contours of a sketch from its entities, as a multi-loop tessellation. The contour ids are
    /// preserved where possible (see the matching below).
    pub fn regen_sketch(&mut self, si: usize) {
        // within a batch the rebuild waits for its end, once for every change (`batch`)
        if self.held_rebuilds.depth > 0 {
            self.held_rebuilds.sketches.insert(si);
            return;
        }
        // the loops are made anew: what a drag kept of them goes
        if let Some(s) = self.sketches.get_mut(si) {
            s.drag_session = None;
        }
        // THE FRAME OF REFERENCE FIRST, before anything is derived from it. The origin and the axis
        // guides are not geometry: a contour, a dimension to an axis and every profile taken from this
        // sketch stand on them. See `pin_frame` for what moving them costs.
        if self.sketches.get(si).is_some_and(|s| s.origin != 0 || s.axis_pts.iter().any(|g| *g != 0)) {
            self.ensure_frame(si); // a document from before the anchor gets one, so the repair below has a target
        }
        self.detach_geometry_from_origin(si);
        if let Some(s) = self.sketches.get_mut(si) {
            s.pin_frame();
        }
        let Some(s) = self.sketches.get(si) else { return };
        // only the loops round what changed since they were made (`round::Laid`), where few curves changed; splines and
        // text are made whole
        let plain = s.splines.is_empty() && s.texts.is_empty();
        if let Some(changed) = self.changed_since_laid(si).filter(|_| plain) {
            let drawn = s.entities.iter().filter(|e| !e.construction).count();
            if changed.drawn.is_empty() && changed.gone.is_empty() {
                return; // nothing drawn changed: every loop stands
            }
            if 4 * (changed.drawn.len() + changed.gone.len()) <= drawn {
                return self.loops_round(si, &changed.drawn, &changed.gone);
            }
        }
        let s = &self.sketches[si];
        // Every contour of a sketch comes from entities now; the ids are reused by position.
        let entity_cids: Vec<Id> = s.contour_ids.clone();
        // Construction geometry never reaches a profile or a contour; it is drawn separately, dashed. Only
        // ordinary entities enter the contours.
        let ents: Vec<SketchEntity> = s.entities.iter().filter(|e| !e.construction).cloned().collect();
        let (pts, splines) = (s.points.clone(), s.splines.clone());
        // Closed contours are the region faces of the planar arrangement (intersections give minimal faces), so
        // half-rings and other areas that are not whole loops become selectable. Open chains (sweep and loft
        // paths) come from the ordinary tessellation, the arrangement producing closed faces only.
        let mut pairs: Vec<(Contour, Vec<Id>)> = arrangement_regions_prov(&pts, &ents);
        pairs.extend(tessellate_sketch_multi(&pts, &ents).into_iter().filter(|c| !c.closed).map(|c| (c, Vec::new())));
        // Spline contours (cubic Hermite with tangent handles, automatic Catmull-Rom by default).
        for sp in &splines {
            if sp.construction {
                continue; // A construction spline never reaches a profile; it is drawn dashed.
            }
            let cps: Vec<Point2> = sp.points.iter().filter_map(|id| pts.iter().find(|p| p.id == *id).map(|p| crate::geom::Point2::new(p.x, p.y))).collect();
            if cps.len() >= 2 {
                pairs.push((tessellate_spline_hermite(&cps, &sp.tangents, sp.closed), Vec::new()));
            }
        }
        // Contours of parametric text (its baked glyphs); ordinary, non-construction text reaches the
        // profile.
        for t in &s.texts {
            if t.construction {
                continue;
            }
            for loop_ in &t.glyphs {
                // THE SEAM IS CLEANED HERE TOO, not only where the glyphs are baked. A glyf outline (TTF)
                // closes by returning to its starting point, so the loop carries two coincident points in a
                // row - a segment of zero length, which OCCT will not make a wire from. Documents whose text
                // was baked before this was fixed already exist and keep their glyphs as they were baked:
                // without this they would go on failing to extrude however often they are rebuilt.
                let clean = seam_cleaned(loop_);
                if clean.len() >= 3 {
                    pairs.push((Contour::closed(clean), Vec::new()));
                }
            }
        }
        let new_entity_cids = self.replace_contours(entity_cids, pairs, &[]);
        self.sketches[si].contour_ids = new_entity_cids;
        let laid = if plain { self.laid_now(si).map(Box::new) } else { None };
        self.sketches[si].laid = laid;
    }
    /// A BATCH OF CHANGES TO SKETCHES: within `work` a rebuild of a sketch is held back, and at its end every sketch
    /// asked for is rebuilt once. A tool that lays many elements, each rebuilding the sketch, rebuilt it as many times:
    /// every corner of 2 000 rectangles filleted took 1 673 s in a test build. Batches nest; the outermost rebuilds.
    pub fn batch<R>(&mut self, work: impl FnOnce(&mut Self) -> R) -> R {
        self.held_rebuilds.depth += 1;
        let out = work(self);
        self.held_rebuilds.depth -= 1;
        if self.held_rebuilds.depth == 0 {
            // the solves held back first, each rebuilding its sketch; then the rebuilds of the rest
            let solves = std::mem::take(&mut self.held_rebuilds.solves);
            for &si in &solves {
                self.solve_sketch(si);
            }
            for si in std::mem::take(&mut self.held_rebuilds.sketches) {
                if !solves.contains(&si) {
                    self.regen_sketch(si);
                }
            }
        }
        out
    }

    /// SOLVE A SKETCH, or within a batch at its end, once for every change (`batch`) - for a tool that solves after
    /// laying an element and reads nothing of the solve. A chamfer at each of 20 corners solved the whole sketch 20
    /// times.
    pub fn solve_sketch_held(&mut self, si: usize) {
        if self.held_rebuilds.depth > 0 {
            self.held_rebuilds.solves.insert(si);
            return;
        }
        self.solve_sketch(si);
    }

    /// THE LOOPS OF A SKETCH REBUILT WHERE IT MOVED, for a frame of a drag: `moved` - the points moved by the frame and
    /// the centres of the circles whose radius changed. The loops are the faces of the arrangement of the curves and
    /// the open chains, and neither runs between curves whose boxes, widened past every weld, do not meet: the curves
    /// of a group whose boxes meet one another make their loops alone. So only the groups holding a curve that moved,
    /// or a curve that shared a loop with one, are made again; the loops of the rest stand. Rebuilt whole, a frame
    /// of a drag among 70 000 lines took 0.43 s, among 70 000 rectangles 1.6 s, in a release build. A sketch with
    /// splines or text, or one whose frame of reference had to be put back, is rebuilt whole.
    pub(super) fn regen_sketch_moved(&mut self, si: usize, moved: &std::collections::HashSet<Id>) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        s.drag_session = None; // its loops are made anew: what a drag kept of them goes
        let s = &self.sketches[si];
        // a frame of reference to be made first (`ensure_frame` in `regen_sketch`), splines, text: rebuilt whole
        if !s.splines.is_empty() || !s.texts.is_empty() || (s.origin != 0 || s.axis_pts.iter().any(|g| *g != 0)) && s.frame == 0 {
            return self.regen_sketch(si);
        }
        if self.detach_geometry_from_origin(si) || self.sketches[si].pin_frame() {
            return self.regen_sketch(si);
        }
        let s = &self.sketches[si];
        let changed: std::collections::HashSet<Id> = s.entities.iter().filter(|e| !e.construction && entity_points(e).iter().any(|p| moved.contains(p))).map(|e| e.id).collect();
        if changed.is_empty() {
            return; // nothing drawn moved: every loop stands
        }
        self.loops_round(si, &changed, &std::collections::HashSet::new());
    }

    /// THE LOOPS ROUND A CHANGE MADE AGAIN (`round`): the curves `changed` (drawn now) and `gone` since the loops were
    /// made, the regions and chains round them made again, the rest standing.
    fn loops_round(&mut self, si: usize, changed: &std::collections::HashSet<Id>, gone: &std::collections::HashSet<Id>) {
        let laid = self.laid_held(si);
        let s = &self.sketches[si];
        let at: std::collections::HashMap<Id, SketchPoint> = s.points.iter().map(|p| (p.id, *p)).collect();
        let drawn: Vec<SketchEntity> = s.entities.iter().filter(|e| !e.construction).copied().collect();
        let shifted: Vec<usize> = drawn.iter().enumerate().filter(|(_, e)| changed.contains(&e.id)).map(|(k, _)| k).collect();
        let touched = |e: &Id| changed.contains(e) || gone.contains(e);
        // each loop of the sketch: its place, closed or open, its box, and whether a curve changed is in it - looked at
        // once, with no list made for a loop: made for each of the 40 000 cells of a grid, the lists were 14 ms of an edit
        struct Before {
            id: Id,
            ci: usize,
            closed: bool,
            span: [f64; 4],
            touched: bool,
        }
        let of = |id: Id, ci: usize| self.contours.ents_of(id).into_iter().flatten().chain(&self.contours[ci].edge_src);
        let before: Vec<Before> = s
            .contour_ids
            .iter()
            .filter_map(|&id| {
                let ci = self.contour_index(id)?;
                Some(Before { id, ci, closed: self.contours[ci].closed, span: round::contour_box(&self.contours[ci]), touched: of(id, ci).any(touched) })
            })
            .collect();
        // where the moved curves stood is known by the loops they were in, and where they stand by their boxes now
        let boxes: Vec<[f64; 4]> = drawn.iter().map(|e| entity_box(e, &|id| at.get(&id).map(|p| (p.x, p.y)))).collect();
        let swept: Vec<[f64; 4]> = before.iter().filter(|b| b.touched).map(|b| b.span).chain(shifted.iter().map(|&k| boxes[k])).collect();
        // the regions meeting a swept box, looked up on a grid of their boxes: each region against each of the 400 boxes
        // a line of a grid of 200 by 200 sweeps was 16 million looks, 60 ms of an edit
        let mut old: std::collections::HashSet<Id> = before.iter().filter(|b| b.touched).map(|b| b.id).collect();
        let spans: Vec<[f64; 4]> = before.iter().map(|b| b.span).collect();
        let regions = round::BoxGrid::new(&spans);
        for w in &swept {
            old.extend(regions.near(w, spans.len()).into_iter().filter(|&i| before[i].closed && round::meet(&spans[i], w)).map(|i| before[i].id));
        }
        let held = before.iter().filter(|b| b.closed && old.contains(&b.id)).map(|b| b.span).reduce(round::union);
        // the chains are made from the changed curves, every curve of an open loop one of them was in, and every curve of
        // any loop a curve gone was in: two sides of a rectangle deleted leave the other two an open chain, and with
        // nothing changed left to reach it from it was lost. A curve still there reaches its chain along its ends; seeded
        // from every loop it was in, a line of a grid of 200 by 200 took in the whole grid
        let place: std::collections::HashMap<Id, usize> = drawn.iter().enumerate().map(|(k, e)| (e.id, k)).collect();
        let from: Vec<usize> = shifted
            .iter()
            .copied()
            .chain(before.iter().filter(|b| b.touched && (!b.closed || of(b.id, b.ci).any(|e| gone.contains(e)))).flat_map(|b| of(b.id, b.ci).filter_map(|e| place.get(e).copied())))
            .collect();
        let grid = round::BoxGrid::new(&boxes);
        let curve = |k: usize| drawn[k];
        let point = |id: Id| at.get(&id).copied();
        let near = |b: &[f64; 4]| grid.near(b, boxes.len());
        let change = round::Change { curve: &curve, boxes: &boxes, at: &point, near: &near, moved: &shifted, swept: &swept, held };
        let mut pairs = round::regions_round(&change);
        let chains = round::chains_round(&change, &from);
        pairs.extend(chains.made);
        old.extend(before.iter().filter(|b| !b.closed && of(b.id, b.ci).any(|e| chains.through.contains(e))).map(|b| b.id));
        let (old, kept): (Vec<Id>, Vec<Id>) = s.contour_ids.iter().partition(|cid| old.contains(cid));
        // the loops whose nesting may change: those whose box meets the box of the loops going and coming
        let going: std::collections::HashSet<Id> = old.iter().copied().collect();
        let over = before.iter().filter(|b| going.contains(&b.id)).map(|b| b.span).chain(pairs.iter().map(|(c, _)| round::contour_box(c))).reduce(round::union);
        let near: Vec<Id> = before.iter().filter(|b| !going.contains(&b.id) && over.is_some_and(|o| round::meet(&b.span, &o))).map(|b| b.id).collect();
        let made = self.replace_contours(old, pairs, &near);
        self.sketches[si].contour_ids = kept.into_iter().chain(made).collect();
        self.laid_follows(si, laid, changed, gone);
    }
    /// THE LOOPS OF A SKETCH REPLACED: the loops `old` (contour ids) give way to `pairs` (each a loop and the entities
    /// it is made of), each new loop taking the id of the old one it is - by the entities of its boundary first, by
    /// its place and size after - so a feature keeps its loop through an edit. Answers the ids of the new loops, in
    /// their order. `kept` - the loops of the sketch that stay as they stood, nested with the new ones.
    pub(super) fn replace_contours(&mut self, old: Vec<Id>, pairs: Vec<(Contour, Vec<Id>)>, kept: &[Id]) -> Vec<Id> {
        let entity_cids = old;
        // Stable contour ids: the new loops are matched against the old ones by a geometric signature
        // (closedness, centroid, area) rather than by position in the list. Positional reuse breaks
        // associativity: adding or removing a loop shifts a contour id onto a different physical loop, and an
        // extrude or an operation on the selected contour attaches to the wrong one. The signature survives
        // reordering and a changed number of loops; when the loop itself was edited, the nearest one of the same
        // kind by centroid is taken, as the best available "this is the same loop" heuristic.
        let (new_contours, new_prov): (Vec<Contour>, Vec<Vec<Id>>) = pairs.into_iter().unzip();
        let sig = |pts: &[Point2]| -> (Point2, f64) {
            let n = pts.len().max(1) as f64;
            let (sx, sy) = pts.iter().fold((0.0, 0.0), |(ax, ay), p| (ax + p.x, ay + p.y));
            let mut area = 0.0;
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                area += a.x * b.y - b.x * a.y;
            }
            (crate::geom::Point2::new(sx / n, sy / n), (0.5 * area).abs())
        };
        let old: Vec<(Id, Point2, f64, bool)> = entity_cids
            .iter()
            .filter_map(|&cid| {
                let ci = self.contour_index(cid)?;
                let c = &self.contours[ci];
                let (ctr, ar) = sig(&c.points);
                Some((cid, ctr, ar, c.closed))
            })
            .collect();
        let n_new = new_contours.len();
        let new_sig: Vec<(Point2, f64, bool)> = new_contours
            .iter()
            .map(|c| {
                let (ctr, ar) = sig(&c.points);
                (ctr, ar, c.closed)
            })
            .collect();
        let mut assign: Vec<Option<Id>> = vec![None; n_new];
        let mut used_old = vec![false; old.len()];
        // Phase one: match by provenance, that is, by the set of boundary entities. A loop is its entities
        // wherever it ends up after an edit, so loops that swapped places do not swap ids and a feature follows
        // its own geometry. Several regions sharing one set (a circle cut by a line) are separated within the
        // group by geometric proximity.
        {
            let old_prov: Vec<Vec<Id>> = old.iter().map(|o| self.contours.ents_of(o.0).cloned().unwrap_or_default()).collect();
            let mut groups: std::collections::HashMap<&Vec<Id>, (Vec<usize>, Vec<usize>)> = std::collections::HashMap::new();
            for (ni, pv) in new_prov.iter().enumerate() {
                if !pv.is_empty() {
                    groups.entry(pv).or_default().0.push(ni);
                }
            }
            for (oi, pv) in old_prov.iter().enumerate() {
                if !pv.is_empty() {
                    if let Some(g) = groups.get_mut(pv) {
                        g.1.push(oi);
                    }
                }
            }
            for (nis, ois) in groups.into_values() {
                let mut inner: Vec<(f64, usize, usize)> = Vec::new();
                for &ni in &nis {
                    for &oi in &ois {
                        if old[oi].3 == new_sig[ni].2 {
                            inner.push((new_sig[ni].0.dist(old[oi].1) + (new_sig[ni].1 - old[oi].2).abs().sqrt(), ni, oi));
                        }
                    }
                }
                inner.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                for (_, ni, oi) in inner {
                    if assign[ni].is_none() && !used_old[oi] {
                        assign[ni] = Some(old[oi].0);
                        used_old[oi] = true;
                    }
                }
            }
        }
        // Phase two: the rest are matched greedily by geometry (centroid and area), best matches first, or the
        // first loop would take an id belonging to another.
        //
        // A loop that stands as it stood matches at no distance, and such matches come first, each new loop taking the
        // first old one free: they are made by the exact signature, without a pair of every new loop with every old.
        // A drag frame among 10 000 lines is 10 000 open chains matched to 10 000 - 1e8 pairs on every frame.
        {
            let exact = |ctr: Point2, ar: f64, cl: bool| [(ctr.x + 0.0).to_bits(), (ctr.y + 0.0).to_bits(), (ar + 0.0).to_bits(), cl as u64];
            let mut free_old: std::collections::HashMap<[u64; 4], std::collections::VecDeque<usize>> = std::collections::HashMap::new();
            for (oi, o) in old.iter().enumerate().filter(|(oi, o)| !used_old[*oi] && o.1.x.is_finite() && o.1.y.is_finite() && o.2.is_finite()) {
                free_old.entry(exact(o.1, o.2, o.3)).or_default().push_back(oi);
            }
            for (ni, (ctr, ar, cl)) in new_sig.iter().enumerate() {
                if assign[ni].is_some() {
                    continue;
                }
                if let Some(oi) = free_old.get_mut(&exact(*ctr, *ar, *cl)).and_then(|q| q.pop_front()) {
                    assign[ni] = Some(old[oi].0);
                    used_old[oi] = true;
                }
            }
        }
        let mut cands: Vec<(usize, usize, f64)> = Vec::new();
        for (ni, (ctr, ar, cl)) in new_sig.iter().enumerate().filter(|(ni, _)| assign[*ni].is_none()) {
            for (oi, o) in old.iter().enumerate().filter(|(oi, _)| !used_old[*oi]) {
                if o.3 == *cl {
                    cands.push((ni, oi, ctr.dist(o.1) + (ar - o.2).abs().sqrt()));
                }
            }
        }
        cands.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        for (ni, oi, _) in cands {
            if assign[ni].is_none() && !used_old[oi] {
                assign[ni] = Some(old[oi].0);
                used_old[oi] = true;
            }
        }
        let mut new_entity_cids: Vec<Id> = Vec::with_capacity(n_new);
        for (ni, mut c) in new_contours.into_iter().enumerate() {
            c.canonicalize(); // The exact edges are the single source; the polyline and the traversal come from
                              // them.
            match assign[ni] {
                Some(cid) => {
                    if let Some(ci) = self.contour_index(cid) {
                        if let Some(slot) = self.contours.get_mut(ci) {
                            *slot = c;
                        }
                    }
                    new_entity_cids.push(cid);
                }
                None => new_entity_cids.push(self.add_contour(c)),
            }
        }
        // the nesting among the new loops and `kept`, the loops of the sketch left as they stood
        self.rebuild_contour_nesting(&new_entity_cids.iter().chain(kept).copied().collect::<Vec<Id>>());
        // Old contours not reused by any new loop are deleted, the id, the provenance and the nesting with each.
        let gone: std::collections::HashSet<Id> = old.iter().zip(&used_old).filter(|(_, used)| !**used).map(|(o, _)| o.0).collect();
        self.contours.remove_ids(&gone);
        // Provenance is persisted: contour id to the entities of its boundary, for matching on the next
        // edit.
        for (ni, cid) in new_entity_cids.iter().enumerate() {
            if new_prov[ni].is_empty() {
                self.contours.clear_ents(*cid);
            } else {
                self.contours.set_ents(*cid, new_prov[ni].clone());
            }
        }
        new_entity_cids
    }
    /// The origin point (0,0, fixed) of a sketch, created lazily. Returns its id. It belongs to no entity and
    /// therefore never reaches a profile or a contour.
    pub fn ensure_origin(&mut self, si: usize) -> Id {
        if let Some(s) = self.sketches.get(si) {
            if s.origin != 0 && s.points.iter().any(|p| p.id == s.origin) {
                return s.origin;
            }
        } else {
            return 0;
        }
        let id = self.alloc_id();
        let s = &mut self.sketches[si];
        s.points.push(SketchPoint { id, x: 0.0, y: 0.0 });
        s.constraints.push(Constraint::Fixed { p: id });
        s.origin = id;
        id
    }
    /// The two points defining the infinite line of a coordinate axis (`which` is 0 for X, 1 for Y): the origin
    /// (0,0) and a fixed guide point at (1,0) or (0,1). Created lazily, for constraints against the axes.
    /// GEOMETRY GLUED INTO THE ORIGIN IS GIVEN A POINT OF ITS OWN, and the origin goes back to zero.
    ///
    /// Reported behaviour: "where has the centre of the circle gone?" - and, put more sharply, "the
    /// circle is not at the origin, but its CENTRE is the origin". Both describe one state: the origin
    /// point had been dragged to (-27, 36) and was the circle's centre, while the origin's MARK is drawn
    /// where the axes cross. Two different places on screen claiming to be the same thing.
    ///
    /// How it got there: `merge_close_points` glues whatever lands at zero INTO the origin, keeping the
    /// origin's id, so the origin becomes somebody's geometry and travels with it.
    ///
    /// The repair keeps the drawing exactly where it is - the entity gets a fresh point at the position the
    /// origin stands at - and leaves the origin to the frame. Returns whether anything was separated.
    ///
    /// IT RUNS WHETHER OR NOT THE ORIGIN HAS WANDERED. It used to give up when the origin sat at zero,
    /// on the reading that a point at zero is home and nothing is wrong. But welded geometry at zero is the
    /// worse half of the same defect: the origin is a system point, `immovable_points` holds those, so the
    /// corner cannot be dragged and moving the shape tears it off - and since `pin_frame` now keeps the
    /// origin at zero, that reading meant the repair could never fire at all.
    pub fn detach_geometry_from_origin(&mut self, si: usize) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        let origin = s.origin;
        if origin == 0 {
            return false;
        }
        let Some((ox, oy)) = s.points.iter().find(|p| p.id == origin).map(|p| (p.x, p.y)) else { return false };
        let used = s.entities.iter().any(|e| entity_points(e).contains(&origin)) || s.splines.iter().any(|sp| sp.points.contains(&origin));
        if !used {
            return false; // nobody's geometry - nothing to separate
        }
        let fresh = self.alloc_id();
        let s = &mut self.sketches[si];
        s.points.push(SketchPoint { id: fresh, x: ox, y: oy });
        let (guides, frame) = (s.axis_pts, s.frame);
        for e in &mut s.entities {
            remap_entity_point(e, origin, fresh);
        }
        for sp in &mut s.splines {
            for id in &mut sp.points {
                if *id == origin {
                    *id = fresh;
                }
            }
        }
        for c in &mut s.constraints {
            // `Fixed` on the origin stays on the origin; so does anything naming the frame or an axis
            // guide - those speak about the reference frame, not about the drawing.
            if matches!(c, Constraint::Fixed { p } if *p == origin) {
                continue;
            }
            let pts = c.points();
            if pts.iter().any(|p| guides.contains(p) || *p == frame) {
                continue;
            }
            remap_constraint_point_pub(c, origin, fresh);
        }
        true
    }

    /// The anchor of the frame of reference: a point at (0,0) that is nobody's geometry - see
    /// `Sketch::frame`. Created lazily, like the origin.
    pub fn ensure_frame(&mut self, si: usize) -> Id {
        if let Some(s) = self.sketches.get(si) {
            if s.frame != 0 && s.points.iter().any(|p| p.id == s.frame) {
                return s.frame;
            }
        } else {
            return 0;
        }
        let id = self.alloc_id();
        let s = &mut self.sketches[si];
        s.points.push(SketchPoint { id, x: 0.0, y: 0.0 });
        s.constraints.push(Constraint::Fixed { p: id });
        s.frame = id;
        id
    }

    pub fn ensure_axis(&mut self, si: usize, which: usize) -> (Id, Id) {
        // THE ANCHOR, NOT THE ORIGIN. An axis is the infinite line through these two points, and the
        // origin is a point geometry gets glued to - so an axis hung on it tilts the moment that geometry
        // moves. See `Sketch::frame`.
        let o = self.ensure_frame(si);
        let w = which.min(1);
        let existing = self.sketches.get(si).map(|s| s.axis_pts[w]).unwrap_or(0);
        if existing != 0 && self.sketches.get(si).is_some_and(|s| s.points.iter().any(|p| p.id == existing)) {
            return (o, existing);
        }
        let id = self.alloc_id();
        let (x, y) = if w == 0 { (1.0, 0.0) } else { (0.0, 1.0) };
        let s = &mut self.sketches[si];
        s.points.push(SketchPoint { id, x, y });
        s.constraints.push(Constraint::Fixed { p: id });
        s.axis_pts[w] = id;
        (o, id)
    }
    /// Stitch nearby sketch points (closer than `tol`): duplicates are merged into one and every entity,
    /// constraint and spline is re-pointed at it.
    ///
    /// This cures a corner that falls apart — line endpoints that never merged into a shared point, so a
    /// dimension or a constraint tears the shape open. Returns how many points were merged.
    pub fn merge_close_points(&mut self, si: usize, tol: f64) -> usize {
        let Some(s) = self.sketches.get_mut(si) else { return 0 };
        // Is the point the centre of a circle or an arc, and therefore the carrier of a solver radius variable?
        // Two such centres must not be stitched into one node, or their radii collapse and concentric circles
        // drawn from one coordinate break. The check runs against the current state of the entities, so it
        // accounts for the remappings already done in this loop.
        let is_center = |ents: &[SketchEntity], pid: Id| ents.iter().any(|e| matches!(e.kind, EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } if center == pid));
        // System points (the origin and the axis endpoints) must not be merged: they lie between 1 and sqrt(2)
        // apart while the tolerance of the line tool reaches 2.0, so merging would consume the axes and tear the
        // dimensions measured to them.
        let sys: Vec<Id> = s.system_ids();
        let mut merged = 0usize;
        // NOTHING IS EVER GLUED ONTO THE ORIGIN. Geometry landing at zero used to be given the origin's id,
        // so that a profile corner drawn there would be one node and the contour would close. The cost was
        // paid by the person: the corner then WAS a system point, and `immovable_points` holds those - the
        // shape could not be dragged, and moving it tore the corner off at zero.
        //
        // Reported behaviour: "I cannot move a circle away from the origin with the move tool". A circle
        // survived by accident (a radius centre is never glued); a line drawn from zero did not - measured,
        // its end came back as the origin's own id.
        //
        // The contour still closes: the ordinary stitching below merges two GEOMETRY points standing in one
        // place into one geometry point, which is what closing needs. The frame of reference stays out of it.
        let mut i = 0;
        while i < s.points.len() {
            let (ix, iy, keep) = (s.points[i].x, s.points[i].y, s.points[i].id);
            let mut j = i + 1;
            while j < s.points.len() {
                let dup = s.points[j].id;
                let d2 = (ix - s.points[j].x).powi(2) + (iy - s.points[j].y).powi(2);
                // Not merged: two radius-curve centres (concentricity is a constraint, not a shared node) and
                // any system point (the reference frame of the origin and the axes is left alone).
                if dup != keep && d2 < tol * tol && !(is_center(&s.entities, keep) && is_center(&s.entities, dup)) && !sys.contains(&keep) && !sys.contains(&dup) {
                    remap_point_id(s, dup, keep);
                    s.points.remove(j);
                    merged += 1;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
        if merged > 0 {
            // Drop degenerate zero-length lines and the constraints they orphan.
            s.entities.retain(|e| !matches!(e.kind, EntityKind::Line { a, b } if a == b));
            self.regen_sketch(si);
        }
        merged
    }
    /// Find a sketch point near (x, y) within `eps`, or create one. Returns its id.
    pub fn sketch_point_at(&mut self, si: usize, x: f64, y: f64, eps: f64) -> Id {
        // THE FRAME OF REFERENCE IS NEVER ADOPTED AS SOMEBODY'S GEOMETRY.
        //
        // Reported behaviour: "I cannot move the circle away from the origin", and before that "a
        // dimension to the X or Y axis comes out crooked". Both came from here. A click at zero found
        // the origin already standing there and handed it over as the new circle's centre - so the circle
        // WAS the origin. Drag it and the reference frame travels with it, tilting both axes; pin the
        // frame and the circle can no longer be moved. The drawing and the frame it is measured from
        // cannot be the same point.
        //
        // Geometry landing on the origin gets a point of its own, in the same place. A coincidence with
        // the origin is a CONSTRAINT, made by clicking it with the constraint tool - not by silently
        // sharing the id.
        let sys = self.sketches[si].system_ids();
        // the distance told by its square first, the frame of reference only for a point that near: a root and a look
        // at the frame for every point of the sketch was 2 s of filleting every corner of 2 000 rectangles
        let near = eps * eps;
        if let Some(p) = self.sketches[si].points.iter().find(|p| (p.x - x).powi(2) + (p.y - y).powi(2) <= near && !sys.contains(&p.id)) {
            return p.id;
        }
        let id = self.alloc_id();
        self.sketches[si].points.push(SketchPoint { id, x, y });
        id
    }
    /// Add a segment entity; its endpoints are deduplicated against the existing points.
    pub fn add_line_entity(&mut self, si: usize, ax: f64, ay: f64, bx: f64, by: f64, purpose: crate::feature::Purpose) -> Id {
        let construction = purpose == crate::feature::Purpose::Construction;
        let a = self.sketch_point_at(si, ax, ay, 1e-6);
        let b = self.sketch_point_at(si, bx, by, 1e-6);
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Line { a, b }, construction });
        self.regen_sketch(si);
        id
    }
    /// Whether point `pid` is already used as the centre of a circle or an arc, and so carries a solver radius
    /// variable.
    ///
    /// A radius variable is keyed by centre id (see `solver::RadiusVar`), so two curves cannot share one centre
    /// node without their radii collapsing. Concentricity is a constraint, not a shared node.
    pub(super) fn is_radius_center(&self, si: usize, pid: Id) -> bool {
        self.sketches.get(si).is_some_and(|s| s.entities.iter().any(|e| matches!(e.kind, EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } if center == pid)))
    }
    /// Centre node of a radius curve (circle, arc, polygon, slot) at (x, y).
    ///
    /// Ordinary deduplication, except that landing on another curve's centre (drawing concentrically from one
    /// point) allocates a separate node: the solver radius variable is keyed by centre, and a shared node would
    /// collapse the radii. Concentricity is expressed as a constraint instead.
    pub(super) fn radius_center_at(&mut self, si: usize, x: f64, y: f64) -> Id {
        let c = self.sketch_point_at(si, x, y, 1e-6);
        if self.is_radius_center(si, c) {
            let id = self.alloc_id();
            self.sketches[si].points.push(SketchPoint { id, x, y });
            id
        } else {
            c
        }
    }
    /// Add a circle entity (a centre point plus a radius). Returns the entity id.
    pub fn add_circle_entity(&mut self, si: usize, cx: f64, cy: f64, r: f64, purpose: crate::feature::Purpose) -> Id {
        let construction = purpose == crate::feature::Purpose::Construction;
        let c = self.radius_center_at(si, cx, cy);
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Circle { center: c, r: r.max(0.01) }, construction });
        self.regen_sketch(si);
        id
    }
    /// A rectangle by two opposite corners, kept as one shape (see `SketchRect`); it grows from the corner at
    /// (`x0`, `y0`). Returns the ids of its four sides.
    pub fn add_rect_entity(&mut self, si: usize, x0: f64, y0: f64, x1: f64, y1: f64, purpose: crate::feature::Purpose) -> Vec<Id> {
        let (xa, xb) = (x0.min(x1), x0.max(x1));
        let (ya, yb) = (y0.min(y1), y0.max(y1));
        let corners = [Point2::new(xa, ya), Point2::new(xb, ya), Point2::new(xb, yb), Point2::new(xa, yb)];
        let first = corners.iter().position(|c| c.x == x0 && c.y == y0).unwrap_or(0);
        self.lay_rect(si, corners, DrawnFrom::Corner(first), purpose)
    }
    /// A rectangle drawn from its centre and a corner, kept as one shape; it grows about the centre. Returns the ids
    /// of its four sides.
    pub fn add_rect_from_centre(&mut self, si: usize, centre: Point2, corner: Point2, purpose: crate::feature::Purpose) -> Vec<Id> {
        let (dx, dy) = ((corner.x - centre.x).abs(), (corner.y - centre.y).abs());
        let corners = [
            Point2::new(centre.x - dx, centre.y - dy),
            Point2::new(centre.x + dx, centre.y - dy),
            Point2::new(centre.x + dx, centre.y + dy),
            Point2::new(centre.x - dx, centre.y + dy),
        ];
        self.lay_rect(si, corners, DrawnFrom::Centre, purpose)
    }
    /// LAY A RECTANGLE OF FOUR CORNERS in order round it: the sides, and - for real geometry - the shape kept as one:
    /// opposite sides parallel, the first two square, the turn of the first side held (`Orientation`), a centre point
    /// on the middle of the diagonal, the two construction diagonals for a rectangle drawn from its centre, and the
    /// record. Construction
    /// geometry gets the four sides alone. Returns the ids of the sides.
    fn lay_rect(&mut self, si: usize, corners: [Point2; 4], from: DrawnFrom, purpose: crate::feature::Purpose) -> Vec<Id> {
        let construction = purpose == crate::feature::Purpose::Construction;
        let pids: Vec<Id> = corners.iter().map(|c| self.sketch_point_at(si, c.x, c.y, 1e-6)).collect();
        let mut sides = [0; 4];
        for (k, side) in sides.iter_mut().enumerate() {
            *side = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id: *side, kind: EntityKind::Line { a: pids[k], b: pids[(k + 1) % 4] }, construction });
        }
        if !construction {
            let [c0, c1, c2, c3] = [pids[0], pids[1], pids[2], pids[3]];
            let mid = Point2::new((corners[0].x + corners[2].x) / 2.0, (corners[0].y + corners[2].y) / 2.0);
            let centre = self.sketch_point_at(si, mid.x, mid.y, 1e-6);
            let diagonals = matches!(from, DrawnFrom::Centre).then(|| {
                let mut d = [0; 2];
                for (id, (a, b)) in d.iter_mut().zip([(c0, c2), (c1, c3)]) {
                    *id = self.alloc_id();
                    self.sketches[si].entities.push(SketchEntity { id: *id, kind: EntityKind::Line { a, b }, construction: true });
                }
                d
            });
            let deg = (corners[1].y - corners[0].y).atan2(corners[1].x - corners[0].x).to_degrees();
            for c in rect_own_constraints(c0, c1, c2, c3, centre, deg) {
                self.add_constraint_if_independent(si, c);
            }
            let anchor = match from {
                DrawnFrom::Corner(k) => crate::model::RectAnchor::Corner(pids[k]),
                DrawnFrom::Centre => crate::model::RectAnchor::Centre,
            };
            let id = self.alloc_id();
            self.sketches[si].rects.push(crate::model::SketchRect { id, corners: [c0, c1, c2, c3], sides, centre, diagonals, anchor });
        }
        self.regen_sketch(si);
        sides.to_vec()
    }
    /// Like `add_polygon_entity`, but returns the id of the circumscribed circle centre together with the side
    /// ids.
    pub fn add_polygon_param(&mut self, si: usize, c: crate::geom::Point2, v: crate::geom::Point2, n: u32, purpose: crate::feature::Purpose) -> (Id, Vec<Id>) {
        let ((cx, cy), (vx, vy)) = ((c.x, c.y), (v.x, v.y));
        let construction = purpose == crate::feature::Purpose::Construction;
        let n = n.max(3) as usize;
        let r = ((vx - cx).powi(2) + (vy - cy).powi(2)).sqrt().max(0.01);
        let a0 = (vy - cy).atan2(vx - cx);
        let center = self.radius_center_at(si, cx, cy); // The circumscribed circle of a polygon gets its own
                                                        // centre node.
        let pids: Vec<Id> = (0..n)
            .map(|k| {
                let a = a0 + std::f64::consts::TAU * k as f64 / n as f64;
                self.sketch_point_at(si, cx + r * a.cos(), cy + r * a.sin(), 1e-6)
            })
            .collect();
        // The construction circumscribed circle is the parametric rim of the polygon.
        let circ = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id: circ, kind: EntityKind::Circle { center, r }, construction: true });
        let mut eids = Vec::with_capacity(n);
        for k in 0..n {
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Line { a: pids[k], b: pids[(k + 1) % n] }, construction });
            eids.push(id);
        }
        // Parametric shape: the vertices sit on the circumscribed circle and every side is equal, which makes
        // the polygon regular, while a driving radius dimension holds its size. What stays free is the position
        // of the centre and the rotation.
        let s = &mut self.sketches[si];
        for &p in &pids {
            s.constraints.push(Constraint::PointOnCircle { p, c: center });
        }
        for k in 1..n {
            s.constraints.push(Constraint::Equal { a: pids[0], b: pids[1], c: pids[k], d: pids[(k + 1) % n] });
        }
        // The radius witness line goes into the gap between two vertices rather than onto a vertex.
        //
        // The `off` field of a `Diameter` constraint is the on-screen angle of the witness line, and zero means
        // to the right. The first vertex of a polygon lies where it was dragged, so drawing left to right in the
        // usual way puts the "R20" label exactly on that vertex and the value becomes unreadable. Half a step
        // around the circle moves it into the gap and keeps it there for any number of sides and any rotation.
        // The sign is in screen space, where the canvas Y axis points down.
        s.constraints.push(Constraint::Diameter { c: center, d: r, off: -a0 + std::f64::consts::PI / n as f64, expr: String::new(), driven: false, diam: false, at: None });
        self.regen_sketch(si);
        (center, eids)
    }
    /// Add a slot between two centres with radius `r`: two lines plus two end arcs.
    pub fn add_slot_entity(&mut self, si: usize, c1: crate::geom::Point2, c2: crate::geom::Point2, r: f64, purpose: crate::feature::Purpose) {
        let ((c1x, c1y), (c2x, c2y)) = ((c1.x, c1.y), (c2.x, c2.y));
        let construction = purpose == crate::feature::Purpose::Construction;
        let r = r.max(0.01);
        let (dx, dy) = (c2x - c1x, c2y - c1y);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            return;
        }
        let (px, py) = (-dy / len, dx / len); // Perpendicular.
        let c1 = self.radius_center_at(si, c1x, c1y); // The slot ends get their own centre nodes (see
                                                      // `radius_center_at`).
        let c2 = self.radius_center_at(si, c2x, c2y);
        let i1 = self.sketch_point_at(si, c1x + px * r, c1y + py * r, 1e-6);
        let i2 = self.sketch_point_at(si, c2x + px * r, c2y + py * r, 1e-6);
        let i3 = self.sketch_point_at(si, c2x - px * r, c2y - py * r, 1e-6);
        let i4 = self.sketch_point_at(si, c1x - px * r, c1y - py * r, 1e-6);
        let mut add = |kind| {
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind, construction });
        };
        add(EntityKind::Line { a: i1, b: i2 });
        add(EntityKind::Arc { center: c2, a: i3, b: i2, ccw: true });
        add(EntityKind::Line { a: i3, b: i4 });
        add(EntityKind::Arc { center: c1, a: i1, b: i4, ccw: true });
        // A parametric slot, or dragging and dimension edits spread its corners and arcs apart. The arc
        // endpoints already sit on the radius circles of their centres (the arc intrinsics). Added on top:
        // equal end radii plus tangency of both side lines to both ends (i1 to i4 are the tangency points).
        // That keeps the sides parallel to the centreline and tangentially smooth, so the slot holds its shape
        // under the solver.
        let cs = &mut self.sketches[si].constraints;
        cs.push(Constraint::EqualRadius { c1, c2 });
        cs.push(Constraint::Tangent { a: i1, b: i2, c: c1, r });
        cs.push(Constraint::Tangent { a: i1, b: i2, c: c2, r });
        cs.push(Constraint::Tangent { a: i4, b: i3, c: c1, r });
        cs.push(Constraint::Tangent { a: i4, b: i3, c: c2, r });
        self.regen_sketch(si);
    }
    /// A rotated rectangle from three points: p1 to p2 gives one side and its direction, p3 gives the height as
    /// a projection onto the normal. Returns the ids of the four sides. Opposite sides are held parallel and
    /// adjacent ones perpendicular, so it stays a rectangle under the solver.
    pub fn add_rect3_entity(&mut self, si: usize, p1: crate::geom::Point2, p2: crate::geom::Point2, p3: crate::geom::Point2, purpose: crate::feature::Purpose) -> Vec<Id> {
        let (dx, dy) = (p2.x - p1.x, p2.y - p1.y);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            return Vec::new();
        }
        let (nx, ny) = (-dy / len, dx / len); // Unit normal to the side.
        let h = (p3.x - p2.x) * nx + (p3.y - p2.y) * ny; // Signed height: the projection of p3 onto the normal.
        let corners = [p1, p2, Point2::new(p2.x + nx * h, p2.y + ny * h), Point2::new(p1.x + nx * h, p1.y + ny * h)];
        self.lay_rect(si, corners, DrawnFrom::Corner(0), purpose)
    }
    /// Add an arc entity (centre, start, end, direction).
    pub fn add_arc_entity(&mut self, si: usize, c: crate::geom::Point2, a: crate::geom::Point2, b: crate::geom::Point2, winding: crate::feature::Winding, purpose: crate::feature::Purpose) {
        let ((cx, cy), (ax, ay), (bx, by)) = ((c.x, c.y), (a.x, a.y), (b.x, b.y));
        // THE END LANDS ON THE ARC: the point given for it says which way the arc runs to, and the end stands on the
        // circle of the start's radius in that direction. Laid where it was clicked, the end of an arc drawn by its centre
        // stood inside or outside the arc, away from its end (a third click at (0, 50) of an arc from (100, 0) about the
        // origin left it at (0, 50), where the arc ends at (0, 100)).
        let (r, (dx, dy)) = ((ax - cx).hypot(ay - cy), (bx - cx, by - cy));
        let d = dx.hypot(dy);
        let (bx, by) = if r > 1e-12 && d > 1e-12 { (cx + dx / d * r, cy + dy / d * r) } else { (bx, by) };
        let construction = purpose == crate::feature::Purpose::Construction;
        let ccw = winding == crate::feature::Winding::Ccw;
        let center = self.radius_center_at(si, cx, cy); // Its own centre node (see `radius_center_at`).
        let a = self.sketch_point_at(si, ax, ay, 1e-6);
        let b = self.sketch_point_at(si, bx, by, 1e-6);
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Arc { center, a, b, ccw }, construction });
        self.regen_sketch(si);
    }
    /// Create a sketch from a set of contours (a DXF or SVG import) and return its id.
    pub fn add_sketch(&mut self, name: impl Into<String>, contours: Vec<Contour>, source: Option<Id>) -> Id {
        let contour_ids: Vec<Id> = contours.into_iter().map(|c| self.add_contour(c)).collect();
        let id = self.alloc_id();
        self.sketches.push(Sketch {
            id,
            name: name.into(),
            contour_ids,
            source,
            points: Vec::new(),
            entities: Vec::new(),
            closed: false,
            constraints: Vec::new(),
            splines: Vec::new(),
            notes: Vec::new(),
            texts: Vec::new(),
            patterns: Vec::new(),
            rects: Vec::new(),
            projections: Vec::new(),
            plane: crate::feature::SketchPlane::default(),
            origin: 0,
            axis_pts: [0, 0],
            frame: 0,
            origin_uv: None,
            left_unsolved: 0,
            drag_session: None,
            laid: None,
            checked: Default::default(),
            point_at: Default::default(),
            entity_at: Default::default(),
        });
        id
    }
    /// Import a DXF or SVG as an editable sketch: the exact curves (`ProfEdge`) become typed sketcher entities,
    /// so a circle stays a circle, an arc or a fillet stays an arc and a segment stays a segment, rather than
    /// being tessellated into thousands of segments. Shared curve endpoints are stitched by point deduplication
    /// into a connected, editable chain.
    ///
    /// The sketch is placed on `plane` and gets a timeline node in the active context. Unlike `add_sketch`,
    /// which only fills the contour pool and leaves an orphan sketch invisible under component isolation, this
    /// gives the sketch an owner (`add_sketch_node`) and editable geometry that can later carry dimensions and
    /// constraints. Returns the sketch index.
    pub fn import_sketch(&mut self, name: impl Into<String>, curves: Vec<crate::geom::ProfEdge>, source: Option<Id>, plane: crate::feature::SketchPlane) -> usize {
        use crate::geom::ProfEdge;
        let si = self.new_sketch(name);
        self.sketches[si].source = source;
        self.sketches[si].plane = plane;
        // Endpoint deduplication cache: nearby endpoints of adjacent curves become one sketch point, which is
        // what makes the chain connected and closable.
        let mut cache = ImportPoints { welded: super::tess::Welded::new(IMPORT_WELD), ids: Vec::new() };
        for e in &curves {
            match *e {
                ProfEdge::Line { a, b } => {
                    let (ai, bi) = (self.import_intern_pt(si, &mut cache, a.x, a.y), self.import_intern_pt(si, &mut cache, b.x, b.y));
                    if ai != bi {
                        let id = self.alloc_id();
                        self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Line { a: ai, b: bi }, construction: false });
                    }
                }
                ProfEdge::Arc { a, b, center, ccw } => {
                    let (ai, bi) = (self.import_intern_pt(si, &mut cache, a.x, a.y), self.import_intern_pt(si, &mut cache, b.x, b.y));
                    // The arc centre is its own point: it is not shared with the endpoints and is moved
                    // independently.
                    let ci = self.alloc_id();
                    self.sketches[si].points.push(SketchPoint { id: ci, x: center.x, y: center.y });
                    let id = self.alloc_id();
                    self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Arc { center: ci, a: ai, b: bi, ccw }, construction: false });
                }
                ProfEdge::Circle { center, r } => {
                    let ci = self.alloc_id();
                    self.sketches[si].points.push(SketchPoint { id: ci, x: center.x, y: center.y });
                    let id = self.alloc_id();
                    self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Circle { center: ci, r }, construction: false });
                }
            }
        }
        let (sid, nm) = (self.sketches[si].id, self.sketches[si].name.clone());
        self.add_sketch_node(sid, nm); // The sketch becomes a timeline node owned by the active context.
        self.regen_sketch(si); // Rebuild the contours from the entities: the profile for extrudes and
                               // toolpaths.
        si
    }
    /// Return the id of a sketch point at (x, y), reusing a nearby one (endpoint deduplication for imports) or
    /// creating one. Nearby: within `IMPORT_WELD` along x and along y; the first point made that is, as a look along
    /// all of them in order finds it - through a grid as wide (`Welded`): the 140 000 ends of a drawing of 70 000
    /// segments looked along every point before them took 5.3 s.
    pub(super) fn import_intern_pt(&mut self, si: usize, cache: &mut ImportPoints, x: f64, y: f64) -> Id {
        let node = cache.welded.weld((x, y), |(cx, cy), (x, y)| (cx - x).abs() < IMPORT_WELD && (cy - y).abs() < IMPORT_WELD);
        if let Some(&id) = cache.ids.get(node) {
            return id;
        }
        let id = self.alloc_id();
        self.sketches[si].points.push(SketchPoint { id, x, y });
        cache.ids.push(id);
        id
    }
    pub fn sketch_index(&self, id: Id) -> Option<usize> {
        self.sketches.iter().position(|s| s.id == id)
    }
    /// The sketch a contour belongs to, by contour id.
    pub fn sketch_of_contour(&self, cid: Id) -> Option<Id> {
        self.sketches.iter().find(|s| s.contour_ids.contains(&cid)).map(|s| s.id)
    }
    /// Ids of contours that belong to no sketch (drawn or free-standing).
    pub fn loose_contour_ids(&self) -> Vec<Id> {
        self.contours.ids().iter().copied().filter(|cid| self.sketch_of_contour(*cid).is_none()).collect()
    }
    pub fn contour_id(&self, index: usize) -> Option<Id> {
        self.contours.id_at(index)
    }
    pub fn contour_index(&self, id: Id) -> Option<usize> {
        self.contours.index_of(id)
    }
    /// Frame of the plane sketch `si` is placed on: a world plane, a datum or a face, resolved into an
    /// orthonormal frame.
    pub fn sketch_frame(&self, si: usize) -> Option<crate::feature::PlaneFrame> {
        use crate::feature::{PlaneFrame, SketchPlane};
        // Shift of the origin within the plane (snapped to an edge or a vertex). It is applied to the base
        // frame, in its own axes, before any outer transform: u*X + v*Y is invariant to the placement of the
        // body.
        let uv = self.sketches.get(si)?.origin_uv;
        let shift = |mut f: PlaneFrame| -> PlaneFrame {
            if let Some(uv) = uv {
                let o = f.lift(uv);
                f.origin = [o.x, o.y, o.z];
            }
            f
        };
        match self.sketches.get(si)?.plane {
            SketchPlane::World(b) => Some(shift(b.frame())),
            // The sketch origin is the projection of the world origin onto the plane, so the sketch XY axes
            // coincide with the world ones and what is drawn in 2D lands in the same place in 3D.
            SketchPlane::Datum(id) => self.planes.iter().find(|p| p.id == id).map(|p| shift(PlaneFrame::world_aligned(p.origin, p.normal, p.rot_deg))),
            // A face is resolved by persistent id against the faces of the current rebuild of the body, so the
            // sketch keeps holding the same face and travels with it; the centroid and normal fingerprint is the
            // fallback match.
            SketchPlane::Face(body, key) => {
                let sid = self.sketches.get(si)?.id;
                // The face resolves in the local space of the source body (`regen_faces` is local), and the
                // frame is built in that space: the 2D zero is the projection of the source origin onto the face
                // plane, as for a world plane or a datum, and the axes are stable, with no mirroring.
                let (c, n) = self.resolve_face(body, &key);
                let frame = shift(PlaneFrame::world_aligned(c, n, 0.0));
                // Isolation: a sketch on a face of another component's body is allowed only with an explicit
                // external reference. The frame is then carried whole into the local space of the consumer
                // through `relative_transform`, so the sketch is anchored to the source face and follows it
                // along every axis rather than only along the normal, which is all the projection of the
                // consumer origin would give. Without the reference there is no frame, and regenerate blocks the
                // node.
                if let (Some(so), Some(bo)) = (self.sketch_owner(sid), self.body_owner(body)) {
                    if so != bo {
                        if !self.external_authorized(so, body) {
                            return None;
                        }
                        return Some(frame.transformed(&self.relative_transform(bo, so)));
                    }
                }
                Some(frame)
            }
        }
    }
    /// Isolated points of sketch `sid`: those that are neither endpoints of entities or splines nor system
    /// points (the origin and the axes). These are the marks placed for drilling. Returns their world
    /// coordinates, through the sketch frame, in sketch point order.
    pub fn sketch_isolated_points(&self, sid: Id) -> Vec<[f64; 3]> {
        let Some(si) = self.sketch_index(sid) else { return Vec::new() };
        let Some(frame) = self.sketch_frame(si) else { return Vec::new() };
        let s = &self.sketches[si];
        let mut used: std::collections::HashSet<Id> = s.system_ids().into_iter().collect();
        // Endpoints of the entities.
        for e in &s.entities {
            match e.kind {
                EntityKind::Line { a, b } => {
                    used.insert(a);
                    used.insert(b);
                }
                EntityKind::Arc { center, a, b, .. } => {
                    used.insert(center);
                    used.insert(a);
                    used.insert(b);
                }
                EntityKind::Circle { center, .. } => {
                    used.insert(center);
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    used.insert(c);
                    used.insert(ma);
                    used.insert(mi);
                }
            }
        }
        // Spline nodes.
        for sp in &s.splines {
            for &p in &sp.points {
                used.insert(p);
            }
        }
        s.points
            .iter()
            .filter(|p| !used.contains(&p.id))
            .map(|p| {
                let w = frame.lift(Point2 { x: p.x, y: p.y });
                [w.x, w.y, w.z]
            })
            .collect()
    }
    /// Ids of the same isolated points, in the same order as `sketch_isolated_points`. The hole walls are named
    /// after them, so adding a point to the sketch must not rename the existing holes.
    pub fn sketch_hole_point_ids(&self, sid: Id) -> Vec<Id> {
        let Some(si) = self.sketch_index(sid) else { return Vec::new() };
        let s = &self.sketches[si];
        let mut used: std::collections::HashSet<Id> = s.system_ids().into_iter().collect();
        for e in &s.entities {
            match e.kind {
                EntityKind::Line { a, b } => {
                    used.insert(a);
                    used.insert(b);
                }
                EntityKind::Arc { center, a, b, .. } => {
                    used.insert(center);
                    used.insert(a);
                    used.insert(b);
                }
                EntityKind::Circle { center, .. } => {
                    used.insert(center);
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    used.insert(c);
                    used.insert(ma);
                    used.insert(mi);
                }
            }
        }
        for sp in &s.splines {
            for &p in &sp.points {
                used.insert(p);
            }
        }
        s.points.iter().filter(|p| !used.contains(&p.id)).map(|p| p.id).collect()
    }
    /// Profile of sketch `si` for an extrude or a revolve: its first closed contour, flattened to XY.
    pub fn sketch_profile_xy(&self, si: usize) -> Option<(Id, Vec<f64>)> {
        let s = self.sketches.get(si)?;
        for &cid in &s.contour_ids {
            if let Some(ci) = self.contour_index(cid) {
                let c = &self.contours[ci];
                if c.closed && c.points.len() >= 3 {
                    let xy: Vec<f64> = c.points.iter().flat_map(|p| [p.x, p.y]).collect();
                    return Some((cid, xy));
                }
            }
        }
        None
    }
    /// XY of a specific closed contour by id, used to choose which shape an extrude, cut or revolve takes.
    pub fn contour_profile_xy(&self, cid: Id) -> Option<Vec<f64>> {
        let c = self.contours.get(self.contour_index(cid)?)?;
        if c.closed && c.points.len() >= 3 {
            Some(c.points.iter().flat_map(|p| [p.x, p.y]).collect())
        } else {
            None
        }
    }
    /// Planar regions of a sketch: the minimal closed faces of the arrangement of every entity (intersections,
    /// then splitting, then faces). This makes areas selectable that are not whole loops — a strip between two
    /// circles cut by a line, for instance. Used to pick an extrude profile.
    pub fn sketch_regions(&self, si: usize) -> Vec<crate::geom::Contour> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        arrangement_regions(&s.points, &s.entities)
    }
    /// Like `feature_profile_encoded`, but the contours listed in `fill` are not subtracted as holes, having
    /// been explicitly selected as filled. Two concentric circles with both selected put the inner one in
    /// `fill` and give a solid cylinder rather than a tube.
    ///
    /// Note that this is one face: an outer contour plus its holes. Nesting deeper than two levels also
    /// produces islands, which only [`Project::profile_faces`] and [`Project::encode_profiles_fill`] return.
    pub fn feature_profile_encoded_fill(&self, sketch: Id, profile: Id, fill: &[Id]) -> Option<Vec<f64>> {
        let (outer_cid, hole_ids) = self.profile_faces(sketch, &[profile], fill).into_iter().next()?;
        let outer = self.contours.get(self.contour_index(outer_cid)?)?;
        let holes: Vec<&crate::geom::Contour> = hole_ids.iter().filter_map(|h| self.contour_index(*h).map(|i| &self.contours[i])).collect();
        Some(crate::geom::encode_profile(outer, &holes))
    }
    /// Profile of a feature with names: each side face is named "`role` of feature F from sketch entity E".
    /// `feature` is the node that builds the body, and the role depends on the operation (revolve, sweep,
    /// loft).
    pub fn feature_profile_encoded_named(&mut self, feature: Id, sketch: Id, profile: Id, fill: &[Id], role: crate::names::Role) -> Option<Vec<f64>> {
        let (outer_cid, hole_ids) = self.profile_faces(sketch, &[profile], fill).into_iter().next()?;
        let loops: Vec<Id> = std::iter::once(outer_cid).chain(hole_ids.iter().copied()).collect();
        let map = self.piece_names(feature, role, &loops);
        let outer = self.contours.get(self.contour_index(outer_cid)?)?;
        let holes: Vec<&crate::geom::Contour> = hole_ids.iter().filter_map(|h| self.contour_index(*h).map(|i| &self.contours[i])).collect();
        Some(crate::geom::encode_profile_named(outer, &holes, &|src, e| map.get(&(src, crate::geom::piece_of(e).0)).copied().unwrap_or(0)))
    }

    /// THE NAME OF EVERY PIECE OF EVERY SKETCH ENTITY in the contours `loops`: `role` of `feature` from the entity,
    /// numbered by piece.
    ///
    /// A name per entity is not enough. An entity cut by its neighbours gives several pieces, and regions that meet
    /// only at points keep them all as faces of one body: measured on a rectangle with a slanted bar over its corner,
    /// the band where the two overlap left out, 18 faces under 12 names and 36 edges under 15, so the cursor on one
    /// edge lit another. The pieces of one entity are numbered along it (`geom::piece_of`) into `GeoName::split`; an
    /// entity of one piece, the usual case, keeps split 0 and with it the name it always had.
    fn piece_names(&mut self, feature: Id, role: crate::names::Role, loops: &[Id]) -> std::collections::HashMap<(Id, [i64; 2]), u32> {
        let mut pieces: std::collections::BTreeMap<Id, Vec<([i64; 2], f64)>> = std::collections::BTreeMap::new();
        for cid in loops {
            let Some(i) = self.contour_index(*cid) else { continue };
            for (src, e) in self.contours[i].sourced_edges() {
                if src == 0 {
                    continue;
                }
                let (key, along) = crate::geom::piece_of(&e);
                let of = pieces.entry(src).or_default();
                if !of.iter().any(|(k, _)| *k == key) {
                    of.push((key, along));
                }
            }
        }
        let mut out = std::collections::HashMap::new();
        for (src, mut of) in pieces {
            of.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            for (split, (key, _)) in of.into_iter().enumerate() {
                let name = self.names.intern_face(crate::names::GeoName { feature, role, src, split: split as u16 });
                out.insert((src, key), name);
            }
        }
        out
    }
    /// Resolve a selection into faces: `(outer contour, its holes)`, correct at any depth of nesting.
    ///
    /// The model is simple and predictable: every selected contour is its own region, holding the material
    /// between it and its direct children, which become its holes. The regions are then fused in the kernel, so
    /// selecting adjacent nesting levels composes by itself — selecting the outer and the middle of three
    /// nested rectangles gives `(outer, [middle])` together with `(middle, [inner])`, that is, a plate with a
    /// hole the size of the inner rectangle. This works for any number of nesting levels.
    ///
    /// Marking a nested selected contour as filled on its parent instead, without giving it a region of its
    /// own, ends the descent there and loses everything deeper: three nested rectangles extrude solid. `fill` is
    /// therefore read simply as one more selected contour, which repairs older files without editing the
    /// feature.
    ///
    /// An empty `profiles`, or a zero, means the first closed contour of the sketch.
    pub fn profile_faces(&self, sketch: Id, profiles: &[Id], fill: &[Id]) -> Vec<(Id, Vec<Id>)> {
        let Some(si) = self.sketch_index(sketch) else { return Vec::new() };
        // Closed contours of the sketch usable as a profile.
        let ids: Vec<Id> = self.sketches[si]
            .contour_ids
            .iter()
            .copied()
            .filter(|cid| self.contour_index(*cid).is_some_and(|i| self.contours[i].closed && self.contours[i].points.len() >= 3) && self.contour_profile_xy(*cid).is_some())
            .collect();
        if ids.is_empty() {
            return Vec::new();
        }
        // The selection is `profiles` plus `fill`; the older "filled" mark reads as one more selected
        // contour.
        let mut sel: Vec<Id> = profiles.iter().chain(fill).copied().map(|p| if p == 0 { ids[0] } else { p }).filter(|p| ids.contains(p)).collect();
        if sel.is_empty() {
            sel.push(ids[0]);
        }
        sel.sort_unstable();
        sel.dedup();
        sel.into_iter().map(|c| (c, self.feature_holes(sketch, c))).collect()
    }
    /// Encode every contour in `profiles`, each as its own tool; empty takes the first contour. Returns `None`
    /// when any contour fails to encode. The list feeds `Kernel::combine_region_multi`.
    ///
    /// Nesting of any depth also produces island faces, which enter the same list.
    ///
    /// `feature` is the node building these profiles: its id is part of the name of every side face ("wall of
    /// feature F from sketch entity E"), so two different features over one sketch do not share face names and
    /// the boolean does not confuse them.
    pub fn encode_profiles_fill(&mut self, feature: Id, sketch: Id, profiles: &[Id], fill: &[Id]) -> Option<Vec<Vec<f64>>> {
        self.encode_profiles_role(feature, sketch, profiles, fill, crate::names::Role::Wall)
    }

    /// The same, with the role of the side faces given explicitly. The role is part of a face name, and a
    /// revolve has its own (`Revolved`) as does a sweep (`Swept`): the wall of an extrude and a surface of
    /// revolution are different things, and naming them alike would collide the names of two unlike faces.
    pub fn encode_profiles_role(&mut self, feature: Id, sketch: Id, profiles: &[Id], fill: &[Id], role: crate::names::Role) -> Option<Vec<Vec<f64>>> {
        let faces = self.profile_faces(sketch, profiles, fill);
        if faces.is_empty() {
            return None;
        }
        // The wall names are prepared in advance, in one pass: interning mutates the name table and needs a
        // mutable borrow, while the encoder only needs the finished substitution table. The pieces are counted over
        // every region at once: the pieces of one entity in two regions are two faces of the one body.
        let loops: Vec<Id> = faces.iter().flat_map(|(outer, holes)| std::iter::once(*outer).chain(holes.iter().copied())).collect();
        let wall = self.piece_names(feature, role, &loops);
        let name_of = |src: Id, e: &crate::geom::ProfEdge| -> u32 { wall.get(&(src, crate::geom::piece_of(e).0)).copied().unwrap_or(0) };
        faces
            .into_iter()
            .map(|(outer_cid, hole_ids)| {
                let outer = self.contours.get(self.contour_index(outer_cid)?)?;
                let holes: Vec<&crate::geom::Contour> = hole_ids.iter().filter_map(|h| self.contour_index(*h).map(|i| &self.contours[i])).collect();
                Some(crate::geom::encode_profile_named(outer, &holes, &name_of))
            })
            .collect()
    }
    /// Encode the path of a sweep: one sketch contour, open or closed, as `[1.0, loop_block]` with exact lines
    /// and arcs (`build_exact_wire`). A `path` of zero takes the first contour with at least two points,
    /// preferring an open one, which is the typical path. Sketch contours always carry exact edges.
    pub fn sweep_path_encoded(&self, sketch: Id, path: Id) -> Option<Vec<f64>> {
        let has_pts = |cid: &&Id| self.contour_index(**cid).map(|i| self.contours[i].points.len() >= 2).unwrap_or(false);
        let cid = if path != 0 {
            path
        } else {
            let si = self.sketch_index(sketch)?;
            let ids = &self.sketches[si].contour_ids;
            let open = |cid: &&Id| self.contour_index(**cid).map(|i| !self.contours[i].closed).unwrap_or(false) && has_pts(cid);
            *ids.iter().find(open).or_else(|| ids.iter().find(has_pts))?
        };
        let c = self.contours.get(self.contour_index(cid)?)?;
        if c.points.len() < 2 {
            return None;
        }
        let mut v = vec![1.0];
        v.extend(c.loop_block());
        Some(v)
    }
    /// Candidate profile contours for a sweep (closed, at least three points), in sketch order. The first is
    /// the automatic choice for a `profile` of zero. Used to switch contours in the interface and to preview
    /// them; it matches `feature_profile_encoded`.
    pub fn sweep_profile_contours(&self, sketch: Id) -> Vec<Id> {
        let Some(si) = self.sketch_index(sketch) else { return Vec::new() };
        self.sketches[si]
            .contour_ids
            .iter()
            .copied()
            .filter(|cid| self.contour_index(*cid).map(|i| self.contours[i].closed && self.contours[i].points.len() >= 3).unwrap_or(false) && self.contour_profile_xy(*cid).is_some())
            .collect()
    }
    /// Candidate path contours (at least two points), open ones first as the typical path, then closed ones.
    /// The first is the automatic choice for a `path` of zero, matching the order used by
    /// `sweep_path_encoded`.
    pub fn sweep_path_contours(&self, sketch: Id) -> Vec<Id> {
        let Some(si) = self.sketch_index(sketch) else { return Vec::new() };
        let has_pts = |cid: Id| self.contour_index(cid).map(|i| self.contours[i].points.len() >= 2).unwrap_or(false);
        let is_open = |cid: Id| self.contour_index(cid).map(|i| !self.contours[i].closed).unwrap_or(false);
        let ids: Vec<Id> = self.sketches[si].contour_ids.iter().copied().filter(|c| has_pts(*c)).collect();
        let mut open: Vec<Id> = ids.iter().copied().filter(|c| is_open(*c)).collect();
        let closed: Vec<Id> = ids.iter().copied().filter(|c| !is_open(*c)).collect();
        open.extend(closed);
        open
    }
    /// Loft contour for section sketch `sketch`, honouring the selection `cid` (zero takes the first closed
    /// profile). Returns the id of a contour usable as a section: closed, with at least three points.
    pub fn loft_section_contour(&self, sketch: Id, cid: Id) -> Option<Id> {
        if cid != 0 {
            let ci = self.contour_index(cid)?;
            return (self.contours[ci].closed && self.contours[ci].points.len() >= 3).then_some(cid);
        }
        self.sweep_profile_contours(sketch).first().copied()
    }
    /// Like `loft_encoded`, but the edges of the first section carry the names of the future loft faces: a face
    /// between sections is named after the edge that swept it, there being no other source in the recipe.
    pub fn loft_encoded_named(&mut self, feature: Id, sketches: &[Id], contours: &[Id]) -> Option<(Vec<f64>, Vec<usize>, Vec<f64>)> {
        if sketches.len() < 2 {
            return None;
        }
        // The names are prepared in advance (interning mutates the table) and only for the first section.
        let first_cid = self.loft_section_contour(sketches[0], contours.first().copied().unwrap_or(0))?;
        let mut srcs: Vec<Id> = self.contour_index(first_cid).map(|i| self.contours[i].edge_src.iter().copied().filter(|s| *s != 0).collect()).unwrap_or_default();
        srcs.sort_unstable();
        srcs.dedup();
        let map: std::collections::HashMap<Id, u32> = srcs.into_iter().map(|src| (src, self.intern_name(feature, crate::names::Role::Lofted, src))).collect();
        self.loft_encoded_with(sketches, contours, &map)
    }
    pub(super) fn loft_encoded_with(&self, sketches: &[Id], contours: &[Id], names: &std::collections::HashMap<Id, u32>) -> Option<(Vec<f64>, Vec<usize>, Vec<f64>)> {
        if sketches.len() < 2 {
            return None;
        }
        let mut data = Vec::new();
        let mut offsets = vec![0usize];
        let mut places = Vec::new();
        for (i, &sid) in sketches.iter().enumerate() {
            let cid = self.loft_section_contour(sid, contours.get(i).copied().unwrap_or(0))?;
            let ci = self.contour_index(cid)?;
            let c = &self.contours[ci];
            if !c.closed || c.points.len() < 3 {
                return None;
            }
            if i == 0 {
                data.extend(c.loop_block_named(&|src, _| names.get(&src).copied().unwrap_or(0)));
            } else {
                data.extend(c.loop_block());
            }
            offsets.push(data.len());
            let pl = self.sketch_frame_by_id(sid).map(|f| f.matrix12()).unwrap_or(crate::feature::PLACE_IDENTITY);
            places.extend_from_slice(&pl);
        }
        Some((data, offsets, places))
    }
    /// Points of a contour, by id.
    pub(super) fn contour_pts(&self, cid: Id) -> Vec<Point2> {
        self.contour_index(cid).map(|ci| self.contours[ci].points.clone()).unwrap_or_default()
    }
    /// Recompute the nesting of the sketch contours and record it in `contour_parent`. Computed once when a
    /// sketch is regenerated rather than by every consumer on every build.
    ///
    /// The parent is the smallest strictly enclosing contour. "Strictly" is the key word: a contour that touches
    /// its enclosing one is not a child — it is part of the boundary rather than a hole. Measured failure
    /// without it: a pad flush against a wall was counted as a hole, the hole touched the outer loop, the face
    /// failed to build, and the whole region silently disappeared from the body.
    pub(super) fn rebuild_contour_nesting(&mut self, cids: &[Id]) {
        let data: Vec<(Id, Vec<Point2>, f64)> = cids
            .iter()
            .filter_map(|&cid| {
                let c = self.contours.get(self.contour_index(cid)?)?;
                (c.closed && c.points.len() >= 3).then(|| (cid, c.points.clone(), c.signed_area().abs()))
            })
            .collect();
        for &cid in cids {
            self.contours.clear_parent(cid);
        }
        // A contour holds another only if every point of the other is inside it, so only one whose box meets the box of
        // the other is tried (`meeting_boxes`), in the order of the list: 10 000 rectangles were 1e8 tries of a polygon.
        let boxes: Vec<[f64; 4]> =
            data.iter().map(|(_, pts, _)| pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)])).collect();
        let mut meets: Vec<Vec<usize>> = vec![Vec::new(); data.len()];
        for super::tess::BoxPair { first, second } in super::tess::meeting_boxes(&boxes) {
            meets[first].push(second);
            meets[second].push(first);
        }
        for (k, (cid, pts, _)) in data.iter().enumerate() {
            meets[k].sort_unstable();
            let parent = meets[k]
                .iter()
                .map(|&o| &data[o])
                .filter(|(oid, opts, _)| oid != cid && poly_contains(opts, pts) && !polys_touch(opts, pts))
                .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(oid, _, _)| *oid);
            // A root is marked explicitly (parent 0) rather than by a missing entry: otherwise "the graph is
            // built" and "there is no nesting" are indistinguishable, and a sketch without nesting would be
            // recomputed on every build.
            self.contours.set_parent(*cid, parent.unwrap_or(0));
        }
    }
    pub fn feature_holes(&self, sketch: Id, outer: Id) -> Vec<Id> {
        if outer == 0 {
            return Vec::new();
        }
        let Some(si) = self.sketch_index(sketch) else { return Vec::new() };
        // Nesting is a stored relation (`contour_parent`), computed once when the sketch is regenerated. The
        // direct children are the contours whose parent is this one — no point-in-polygon recomputation on
        // every build and no per-consumer tolerances.
        let by_graph: Vec<Id> = self.sketches[si].contour_ids.iter().copied().filter(|cid| self.contours.parent_of(*cid).as_ref() == Some(&outer)).collect();
        if !by_graph.is_empty() || self.sketches[si].contour_ids.iter().any(|c| self.contours.parent_of(*c).is_some()) {
            return by_graph;
        }
        // The graph is not built yet (the sketch was not regenerated in this session): it is computed here, and
        // nothing beyond it is guessed.
        let outer_pts = self.contour_pts(outer);
        if outer_pts.len() < 3 {
            return Vec::new();
        }
        let cands: Vec<(Id, Vec<Point2>)> = self.sketches[si]
            .contour_ids
            .iter()
            .copied()
            .filter(|cid| *cid != outer && self.contour_profile_xy(*cid).is_some())
            .map(|cid| (cid, self.contour_pts(cid)))
            .filter(|(_, p)| p.len() >= 3 && poly_contains(&outer_pts, p) && !polys_touch(&outer_pts, p))
            .collect();
        // A direct child of `outer` is one that lies inside no other candidate.
        cands.iter().filter(|(id, p)| !cands.iter().any(|(id2, p2)| id2 != id && poly_contains(p2, p))).map(|(id, _)| *id).collect()
    }
    /// Local revolve axis (origin and direction in sketch 2D space, z = 0) from centreline `line`. A `line` of
    /// zero, or one that is not a line or is degenerate, gives `None` and the caller falls back to a datum or the
    /// sketch X or Y axis. `dir` is normalised. The 2D coordinates of the line are already in sketch plane space,
    /// so no inverse placement is needed, unlike a datum axis given in world space.
    pub(super) fn revolve_axis_from_line(&self, sketch: Id, line: Id) -> Option<([f64; 3], [f64; 3])> {
        if line == 0 {
            return None;
        }
        let si = self.sketch_index(sketch)?;
        let (a, b) = self.line_ends(si, line)?;
        let s = &self.sketches[si];
        let pa = s.points.iter().find(|q| q.id == a).map(|q| (q.x, q.y))?;
        let pb = s.points.iter().find(|q| q.id == b).map(|q| (q.x, q.y))?;
        let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
        let len = (dx * dx + dy * dy).sqrt();
        (len > 1e-9).then(|| ([pa.0, pa.1, 0.0], [dx / len, dy / len, 0.0]))
    }
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
        match self.planes[pi].def {
            PlaneDef::OffsetBase { base, dist } => {
                let dist = eval_dim(dim, "dist", dist, vars);
                let f = base.frame();
                let n = f.normal();
                self.planes[pi].normal = n;
                self.planes[pi].origin = [f.origin[0] + n[0] * dist, f.origin[1] + n[1] * dist, f.origin[2] + n[2] * dist];
            }
            PlaneDef::OffsetFace { body, face, dist } => {
                let dist = eval_dim(dim, "dist", dist, vars);
                let (c, n) = self.resolve_face(body, &face); // Persistent face resolution: centre and
                                                             // normal.
                self.planes[pi].normal = n;
                self.planes[pi].origin = [c[0] + n[0] * dist, c[1] + n[1] * dist, c[2] + n[2] * dist];
            }
            PlaneDef::OffsetPlane { plane: src, dist } => {
                // The source plane is resolved earlier in the timeline, having been created earlier, so its
                // origin and normal are already available.
                let dist = eval_dim(dim, "dist", dist, vars);
                if let Some((o, n)) = self.planes.iter().find(|p| p.id == src).map(|p| (p.origin, p.normal)) {
                    let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
                    let nn = [n[0] / nl, n[1] / nl, n[2] / nl];
                    self.planes[pi].normal = n;
                    self.planes[pi].origin = [o[0] + nn[0] * dist, o[1] + nn[1] * dist, o[2] + nn[2] * dist];
                }
            }
            PlaneDef::Manual | PlaneDef::FaceGone | PlaneDef::PlaneGone => {}
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
            self.solve_sketch_inner(si, None, crate::solver::Budget::FULL, Rebuild::Whole);
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
struct ArcEnds {
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
struct Round {
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
enum OffsetSource {
    Line(Id, Id),
    Arc { center: Id, a: Id, b: Id },
    Corner(Id),
    None,
}

/// A line of the source of an offset: its end points, and where they stand.
struct SourceLine {
    a: Id,
    b: Id,
    pa: (f64, f64),
    pb: (f64, f64),
}

/// An arc of the source of an offset: its centre and end points, where the centre stands, and its radius.
struct SourceArc {
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
fn hold_offset_loop(ids: &[Id], kinds: &[(EntityKind, OffsetSource)], dist: f64) -> Vec<Constraint> {
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
    for (k, &v) in ids.iter().enumerate().take(n) {
        let (pk, nk) = ((k + n - 1) % n, k);
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
    held
}

/// Ends of imported curves closer than this along x and along y are one point, mm.
const IMPORT_WELD: f64 = 1e-4;

/// THE POINTS AN IMPORT HAS MADE: the places welded into nodes, and the point each node is.
pub(super) struct ImportPoints {
    welded: super::tess::Welded,
    ids: Vec<Id>,
}

/// WHAT A SOLVE OF A SKETCH REBUILDS of its loops after it.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Rebuild {
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
