//! A CORNER OF CURVES OF ANY KIND ROUNDED OR CHAMFERED: the corner a spline or an arc of an ellipse makes with any curve,
//! and the preview of every corner with a curve in it. A fillet of two lines, two arcs or a line and an arc keeps its own
//! path in `corners.rs`, with the shape of the corner held by tangencies and its sharp held on both lines; here the geometry comes from `curve::rounding` and
//! `curve::chamfer`, each side is cut back to where the cut meets it and laid again as a piece of its own kind, and the
//! side that is a line or an arc is held to the fillet as there.
use super::*;
use crate::model::curve::{chamfer, radius_in, rounding, Arm, Curve, End};

/// A SIDE OF A CORNER IN A SKETCH: the entity, its curve, the end of it on the corner, and the point at its other end.
struct Leg {
    eid: Id,
    curve: Curve,
    end: End,
    far: Id,
}

impl Leg {
    fn arm(&self) -> Arm<'_> {
        Arm { curve: &self.curve, end: self.end }
    }
}

/// AN EDGE AT A CORNER: the curve and the point of the corner it ends at.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EdgeAt {
    pub(crate) edge: Id,
    pub(crate) corner: Id,
}

/// A CORNER OF TWO SIDES: the sides and the point they meet at.
struct Corner {
    legs: [Leg; 2],
    point: Id,
}

/// WHAT THE CUT WOULD LEAVE AT THE CORNER OF TWO ARMS, changing nothing: the corner, where the cut meets each arm, and
/// the arc of a fillet.
pub(crate) fn blend_of_arms(arms: [Arm; 2], cut: CornerCut) -> Option<CornerBlend> {
    let corner = arms[0].corner();
    let (places, arc) = match cut {
        CornerCut::Fillet(size) => {
            let r = radius_in(arms, size)?;
            let found = rounding(arms, r)?;
            (found.at, Some(([found.centre.x, found.centre.y], r)))
        }
        CornerCut::Chamfer(legs_of_cut) => (chamfer(arms, legs_of_cut)?, None),
    };
    let end = |k: usize| {
        let p = arms[k].curve.at(places[k]);
        [p.x, p.y]
    };
    Some(CornerBlend { vertex: [corner.x, corner.y], ends: [end(0), end(1)], arc })
}

impl Project {
    /// The side of the corner that the edge makes; `None` where the edge does not end there.
    fn leg(&self, si: usize, at: EdgeAt) -> Option<Leg> {
        let EdgeAt { edge: eid, corner: pid } = at;
        let s = self.sketches.get(si)?;
        let e = s.entity(eid)?;
        let [a, b] = entity_ends(e)?;
        let (end, far) = if a == pid {
            (End::Start, b)
        } else if b == pid {
            (End::Finish, a)
        } else {
            return None;
        };
        Some(Leg { eid, curve: Curve::of(s, e)?, end, far })
    }

    /// THE WAY THE EDGE LEAVES ITS CORNER, a unit vector along it: a line towards its far end, any other curve along its
    /// tangent there.
    pub(crate) fn leaving(&self, si: usize, at: EdgeAt) -> Option<Point2> {
        self.leg(si, at).map(|l| l.arm().leaving())
    }

    /// The two sides of `corner`, at its point.
    fn legs_of(&self, si: usize, corner: CornerAt) -> Option<Corner> {
        let CornerAt { point, pair } = corner;
        let (e1, e2) = pair;
        Some(Corner { legs: [self.leg(si, EdgeAt { edge: e1, corner: point })?, self.leg(si, EdgeAt { edge: e2, corner: point })?], point })
    }

    /// WHAT THE CUT WOULD LEAVE AT THE CORNER OF ANY TWO CURVES, changing nothing: the preview `corner_blend` shows.
    pub(crate) fn blend_any(&self, si: usize, corner: CornerAt, cut: CornerCut) -> Option<CornerBlend> {
        let legs = self.legs_of(si, corner)?.legs;
        blend_of_arms([legs[0].arm(), legs[1].arm()], cut)
    }

    /// CUT A SIDE BACK TO PLACE `t`: the piece of it from there to its far end laid in its stead, under its own id, of its
    /// own kind. Answers the point at `t`.
    fn cut_leg_back(&mut self, si: usize, leg: &Leg, t: f64) -> Option<Id> {
        let run = match leg.end {
            End::Start => [t, 1.0],
            End::Finish => [0.0, t],
        };
        let piece = leg.curve.piece(run);
        self.sketches[si].let_go_the_copy(leg.eid);
        let how = Self::lay_as_of(self.sketches[si].entity(leg.eid)?);
        let kind = self.curve_kind(si, &piece, how);
        let at = self.point_exactly_at(si, leg.curve.at(t));
        self.sketches[si].entities.iter_mut().find(|e| e.id == leg.eid)?.kind = kind;
        Some(at)
    }

    /// THE SHARP OF THE CORNER STAYS where the side went on (`point_on`): on the line of a line - the side as it is cut
    /// back, from its far end to where the cut meets it - the circle of an arc, the ellipse of an arc of one, a spline.
    fn hold_sharp(&mut self, si: usize, pid: Id, leg: &Leg) {
        let held = self.sketches[si].entity(leg.eid).map(|side| crate::model::point_on(side, pid));
        self.sketches[si].constraints.extend(held);
    }

    /// ROUND THE CORNER OF ANY TWO CURVES with an arc of radius `r`, the one way every fillet of a sketch is made: both
    /// sides cut back to where it touches them, the arc between, held to each side by the touching its kinds take
    /// (`tangency`), its sharp held on the way each side went on, what held a line of the corner carried onto the line as
    /// it is cut back (`carry_edge_constraints`), and the arc sized by its radius. A side the arc takes whole - its far end
    /// where the arc touches it - goes, its two points one: a square 4 x 4 rounded at 2 is a circle, not four arcs held by
    /// lines of no length. Answers whether it was made - not where the radius does not fit.
    pub(crate) fn round_any(&mut self, si: usize, corner: CornerAt, r: f64) -> bool {
        let Some(Corner { legs, point: pid }) = self.legs_of(si, corner) else { return false };
        let Some(found) = rounding([legs[0].arm(), legs[1].arm()], r) else { return false };
        let lines = legs.iter().map(|l| self.sketches[si].entity(l.eid).is_some_and(|e| e.kind.as_line().is_some())).collect::<Vec<bool>>();
        let (Some(t1), Some(t2)) = (self.cut_leg_back(si, &legs[0], found.at[0]), self.cut_leg_back(si, &legs[1], found.at[1])) else { return false };
        let cen = self.point_exactly_at(si, found.centre);
        // the short way round from the first point of touching to the second
        let angle = |p: Point2| (p.y - found.centre.y).atan2(p.x - found.centre.x);
        let (a1, a2) = (angle(legs[0].curve.at(found.at[0])), angle(legs[1].curve.at(found.at[1])));
        let ccw = (a2 - a1).rem_euclid(std::f64::consts::TAU) <= std::f64::consts::PI;
        let arc = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id: arc, kind: crate::model::ArcParts { center: cen, a: t1, b: t2, ccw }.into(), construction: false });
        let mut carried = [None, None];
        for (k, (leg, t)) in legs.iter().zip([t1, t2]).enumerate() {
            if self.side_eaten(si, leg, t) {
                continue;
            }
            let touch = self.tangency(si, [arc, leg.eid]);
            self.sketches[si].constraints.extend(touch);
            if self.sketches[si].entity(leg.eid).is_some_and(|e| e.kind.as_spline().is_some()) {
                // THE SHARP ON THE WAY OF A SPLINE PAST ITS END: the line of touching at the end of the side, square to the
                // radius of the arc there. A point on a spline itself stops at its end: the sharp held there was drawn to
                // the point of touching, and took the arc down to nothing with it
                self.sketches[si].constraints.push(Constraint::Perpendicular { a: cen, b: t, c: t, d: pid });
            } else {
                self.hold_sharp(si, pid, leg);
            }
            if lines[k] {
                carried[k] = Some((leg.far, t));
            }
        }
        self.carry_edge_constraints(si, pid, carried[0], carried[1]);
        let off = fillet_label_angle(&self.sketches[si].points, cen, t1, t2);
        self.sketches[si].constraints.push(Constraint::Diameter { c: cen, d: r, off, expr: String::new(), driven: false, diam: false, at: None });
        self.regen_sketch(si);
        true
    }

    /// WHETHER THE SIDE OF A CORNER WAS TAKEN WHOLE by the cut at `t`, its far end standing there: then it goes, and its
    /// two points are made one.
    fn side_eaten(&mut self, si: usize, leg: &Leg, t: Id) -> bool {
        let (Some(f), Some(q)) = (self.point_xy(si, leg.far), self.point_xy(si, t)) else { return false };
        if (f.0 - q.0).hypot(f.1 - q.1) >= 1e-6 {
            return false;
        }
        let s = &mut self.sketches[si];
        s.entities.retain(|e| e.id != leg.eid);
        if leg.far != t {
            s.constraints.push(Constraint::Coincident { a: leg.far, b: t });
        }
        true
    }

    /// CHAMFER THE CORNER OF ANY TWO CURVES by `legs`, the first leg on the first of the pair: both sides cut back to
    /// where the cut meets them, the cut laid as a line, its size kept as dimensions - the cut of a symmetric one, a leg
    /// straight from the sharp otherwise. Answers whether it was made - not where a side does not reach as far.
    pub(crate) fn chamfer_any(&mut self, si: usize, corner: CornerAt, legs_of_cut: ChamferLegs) -> bool {
        let Some(Corner { legs, point: pid }) = self.legs_of(si, corner) else { return false };
        let Some(places) = chamfer([legs[0].arm(), legs[1].arm()], legs_of_cut) else { return false };
        let lines = legs.iter().map(|l| self.sketches[si].entity(l.eid).is_some_and(|e| e.kind.as_line().is_some())).collect::<Vec<bool>>();
        let (sharp, at1, at2) = (legs[0].arm().corner(), legs[0].curve.at(places[0]), legs[1].curve.at(places[1]));
        let (Some(t1), Some(t2)) = (self.cut_leg_back(si, &legs[0], places[0]), self.cut_leg_back(si, &legs[1], places[1])) else { return false };
        let seg = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id: seg, kind: crate::model::LineParts { a: t1, b: t2 }.into(), construction: false });
        let leg_dim = |ends: [DimEnd; 2], side: Side, d: f64| {
            let [a, b] = ends_facing(ends, side);
            Constraint::Distance { a, b, d, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None }
        };
        let (end_c, end_1, end_2) = (DimEnd { id: pid, at: sharp }, DimEnd { id: t1, at: at1 }, DimEnd { id: t2, at: at2 });
        // THE CHAMFER KEEPS ITS SIZE AS DIMENSIONS, so it can be changed afterwards and the cut follows: a symmetric one by
        // its cut, its two legs from the sharp held equal; two legs each from the sharp; a leg and the angle between the way
        // of the first side to the sharp and the cut, the arc of the angle past the end of the cut, 1.25 of its length out.
        // Every dimension stands outside the shape: the cut on the side of the sharp, a leg on the side of its side away
        // from the cut.
        let dims = match legs_of_cut.mode {
            crate::feature::ChamferMode::Symmetric => vec![leg_dim([end_1, end_2], Side::Toward(sharp), legs_of_cut.first), Constraint::Equal { a: pid, b: t1, c: pid, d: t2 }],
            crate::feature::ChamferMode::TwoDist => vec![leg_dim([end_c, end_1], Side::AwayFrom(at2), sharp.dist(at1)), leg_dim([end_c, end_2], Side::AwayFrom(at1), sharp.dist(at2))],
            crate::feature::ChamferMode::DistAngle => vec![
                leg_dim([end_c, end_1], Side::AwayFrom(at2), sharp.dist(at1)),
                // the way of the first side to the sharp: along a line from its far end, as its arc is drawn; on a curve the
                // chord from where the cut meets it to the sharp
                if lines[0] {
                    Constraint::AngleLines { a: legs[0].far, b: t1, c: t1, d: t2, deg: legs_of_cut.second, expr: String::new(), driven: false, off: 1.25 * at1.dist(at2), at: None }
                } else {
                    Constraint::AngleLines { a: t1, b: pid, c: t1, d: t2, deg: legs_of_cut.second, expr: String::new(), driven: false, off: 1.25 * at1.dist(at2), at: None }
                },
            ],
        };
        self.sketches[si].constraints.extend(dims);
        // the sharp on a side that is a spline is not held on it: a point on a spline stops at its end, and the sharp drawn
        // there took the cut down to nothing; the other side and the legs hold it
        let held: Vec<&Leg> = legs.iter().filter(|l| self.sketches[si].entity(l.eid).is_some_and(|e| e.kind.as_spline().is_none())).collect();
        for leg in held {
            self.hold_sharp(si, pid, leg);
        }
        // what held a line of the corner goes with the line as it is cut back
        let side = |k: usize, t: Id| lines[k].then_some((legs[k].far, t));
        self.carry_edge_constraints(si, pid, side(0, t1), side(1, t2));
        // SOLVED, not only drawn: a chamfer of two legs or of a leg and an angle stands by its dimensions only once the
        // sketch is solved - at the end of a batch of corners, once
        self.solve_sketch_held(si);
        true
    }
}
