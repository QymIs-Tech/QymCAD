//! WHAT A CLICK LANDS ON IS TIED: the relations a new point or shape takes from what is under it.

use super::*;

/// A POINT OF A NEW LINE where it was put: the place, the point, and the ends of the new line itself.
#[derive(Clone, Copy)]
pub(crate) struct LandedOn {
    pub(crate) at: Point2,
    pub(crate) point: Id,
    pub(crate) own: (Id, Id),
}

/// THE POINT TIED TO THE LINE IT LANDED ON (not its own): at its middle - within the 2 px a point lands on a line by - a
/// midpoint, anywhere else along it a point on the line. Reported behaviour: "nothing snaps to projected geometry" - the
/// pointer stuck to the middle of a line and along it, and the point put there was held by nothing, on a projected
/// line as on a line drawn by hand.
pub(crate) fn tie_to_the_line_under(project: &mut Project, view: qymcad_ui_state::View2d, si: usize, landed: LandedOn) {
    use qymcad_core::model::Constraint;
    let Some((la, lb)) = qymcad_ui_state::line_under_point(project, &view, si, landed.at, landed.own.0, landed.own.1) else { return };
    let at = |id: Id| project.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    let middle = at(la).zip(at(lb)).is_some_and(|(u, v)| ((u.0 + v.0) / 2.0 - landed.at.x).hypot((u.1 + v.1) / 2.0 - landed.at.y) * f64::from(view.scale) <= 2.0);
    let tie = if middle { Constraint::Midpoint { p: landed.point, a: la, b: lb } } else { Constraint::PointOnLine { p: landed.point, a: la, b: lb } };
    project.add_constraint_if_independent(si, tie);
}

/// THE POINTS OF A SHAPE JUST PUT WHERE IT WAS CLICKED, TIED TO WHAT THEY LANDED ON, as the ends of a line are: of the
/// points `own` the shape stands on, the one at each of `clicks` - put on a point of the sketch (a corner of a
/// projection, a grey corner of the face `auto_project_under` took in) is made coincident with it, put on a line is held
/// on it (`tie_to_the_line_under`). Reported behaviour: "even without projecting, tie to the edges of the face (the thin
/// grey ones) - at a rebuild everything falls apart" - a rectangle drawn from a grey corner and a circle around one
/// stood where they were drawn when the body changed.
pub(crate) fn tie_the_clicked_points(project: &mut Project, view: qymcad_ui_state::View2d, si: usize, clicks: &[Point2], own: &[Id]) {
    use qymcad_core::model::Constraint;
    for at in clicks {
        let Some(s) = project.sketches.get(si) else { return };
        let near = |q: &qymcad_core::model::SketchPoint| (q.x - at.x).hypot(q.y - at.y) < 1e-6;
        let Some(point) = s.points.iter().find(|q| own.contains(&q.id) && near(q)).map(|q| q.id) else { continue };
        let used: std::collections::HashSet<Id> = s.entities.iter().flat_map(qymcad_core::model::entity_points).collect();
        match s.points.iter().find(|q| !own.contains(&q.id) && used.contains(&q.id) && near(q)).map(|q| q.id) {
            Some(other) => {
                project.add_constraint_if_independent(si, Constraint::Coincident { a: other, b: point });
            }
            None => tie_to_the_line_under(project, view, si, LandedOn { at: *at, point, own: (point, point) }),
        }
    }
}

/// THE POINTS A DRAWING TOOL PUT DOWN, TIED TO WHAT THEY LANDED ON, whatever the tool. Of the points a click made
/// (`made`), the one standing at each place clicked for the shape is tied as the snap there showed it: on a point of the
/// sketch or on the origin - coincident; on a line - on it, or at its middle; on a circle or an arc - on it; on the X or
/// Y axis - on the axis; where two of those cross - on both. A snap puts the point exactly on its target, so a point
/// within half a pixel of a curve stands on it. Reported behaviour: the snap was shown and the point was put down where
/// the pointer was, held by nothing - an end on a circle, a centre, the axes and the origin, for every tool but the line.
pub(crate) fn tie_what_landed(project: &mut Project, view: qymcad_ui_state::View2d, si: usize, clicks: &[Point2], made: &std::collections::HashSet<Id>) {
    use qymcad_core::model::{Constraint, EntityKind};
    let reach = 0.5 / f64::from(view.scale);
    for at in clicks {
        let Some(s) = project.sketches.get(si) else { return };
        let near = |q: Point2| (q.x - at.x).hypot(q.y - at.y) <= reach;
        let Some(point) = s.points.iter().find(|q| made.contains(&q.id) && (q.x - at.x).hypot(q.y - at.y) < 1e-9).map(|q| q.id) else {
            // A SHAPE THAT PUT NO POINT THERE - a circle through three points runs through its clicks: a point of the
            // sketch clicked is held on the new circle
            let standing = s.points.iter().find(|q| !made.contains(&q.id) && near(Point2::new(q.x, q.y))).map(|q| q.id);
            let rim = s.entities.iter().find_map(|e| match e.kind {
                EntityKind::Circle { center, r } if made.contains(&center) => s.points.iter().find(|q| q.id == center).filter(|c| ((at.x - c.x).hypot(at.y - c.y) - r).abs() <= reach).map(|_| center),
                _ => None,
            });
            if let Some(c) = rim {
                let p = match standing {
                    Some(p) => Some(p),
                    None if near(Point2::new(0.0, 0.0)) => Some(project.ensure_origin(si)),
                    None => None,
                };
                if let Some(p) = p {
                    project.add_constraint_if_independent(si, Constraint::PointOnCircle { p, c });
                }
            }
            continue;
        };
        // a point standing there: the origin before any other, and never a guide of the frame
        let guides: Vec<Id> = s.axis_pts.iter().copied().chain([s.frame]).filter(|g| *g != 0).collect();
        let standing = s.points.iter().filter(|q| !made.contains(&q.id) && !guides.contains(&q.id) && near(Point2::new(q.x, q.y))).min_by_key(|q| q.id != s.origin).map(|q| q.id);
        let other = match standing {
            Some(other) => Some(other),
            None if near(Point2::new(0.0, 0.0)) => Some(project.ensure_origin(si)),
            None => None,
        };
        if let Some(other) = other {
            project.add_constraint_if_independent(si, Constraint::Coincident { a: other, b: point });
            continue;
        }
        // the curves passing there, not those the shape itself is made of
        let s = &project.sketches[si];
        let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
        let mut ties: Vec<Constraint> = Vec::new();
        for e in s.entities.iter().filter(|e| !qymcad_core::model::entity_points(e).iter().any(|id| made.contains(id))) {
            match e.kind {
                EntityKind::Line { a, b } => {
                    let (Some(pa), Some(pb)) = (pt(a), pt(b)) else { continue };
                    if qymcad_ui_state::project_on_seg(*at, pa, pb).is_some_and(near) {
                        let middle = near(Point2::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0));
                        ties.push(if middle { Constraint::Midpoint { p: point, a, b } } else { Constraint::PointOnLine { p: point, a, b } });
                    }
                }
                EntityKind::Circle { center, r } => {
                    if pt(center).is_some_and(|c| ((at.x - c.x).hypot(at.y - c.y) - r).abs() <= reach) {
                        ties.push(Constraint::PointOnCircle { p: point, c: center });
                    }
                }
                EntityKind::Arc { center, a, b, ccw } => {
                    let (Some(c), Some(pa), Some(pb)) = (pt(center), pt(a), pt(b)) else { continue };
                    let rim = qymcad_ui_state::Rim { centre: c, radius: (pa.x - c.x).hypot(pa.y - c.y), arc: Some(qymcad_ui_state::RimArc { from: pa, to: pb, ccw }) };
                    if ((at.x - c.x).hypot(at.y - c.y) - rim.radius).abs() <= reach && rim.holds(*at) {
                        ties.push(Constraint::PointOnCircle { p: point, c: center });
                    }
                }
                EntityKind::Ellipse { .. } => {}
            }
        }
        for (axis, off) in [(0, at.y), (1, at.x)] {
            if off.abs() <= reach {
                let (o, guide) = project.ensure_axis(si, axis);
                ties.push(Constraint::PointOnLine { p: point, a: o, b: guide });
            }
        }
        for tie in ties.into_iter().take(2) {
            project.add_constraint_if_independent(si, tie);
        }
    }
}

/// Automatic constraints while drawing the segment prev-p1-p2: horizontal or vertical, perpendicular to
/// the previous segment, and a point-on-edge for the new end. Every constraint is added ONLY if it is
/// independent (does not over-define the sketch), so no redundant ones appear.
pub fn infer_on_segment(project: &mut Project, view: qymcad_ui_state::View2d, si: usize, prev: Option<Point2>, p1: Point2, p2: Point2) {
    use qymcad_core::model::Constraint;
    let a = project.sketch_point_at(si, p1.x, p1.y, 1e-6);
    let b = project.sketch_point_at(si, p2.x, p2.y, 1e-6);
    let (dx, dy) = ((p2.x - p1.x).abs(), (p2.y - p1.y).abs());
    let tol = 0.06; // ~3.5°
                    // 1) horizontal or vertical
    let mut axis_aligned = false;
    if dy <= dx * tol && dx > 1e-6 {
        axis_aligned = project.add_constraint_if_independent(si, Constraint::Horizontal { a, b });
    } else if dx <= dy * tol && dy > 1e-6 {
        axis_aligned = project.add_constraint_if_independent(si, Constraint::Vertical { a, b });
    }
    // 2) perpendicular to the previous segment (only when the new one did not land on an axis, or it
    // would be a duplicate)
    if !axis_aligned {
        if let Some(pv) = prev {
            let pa = project.sketch_point_at(si, pv.x, pv.y, 1e-6);
            let (ux, uy) = (p1.x - pv.x, p1.y - pv.y);
            let (vx, vy) = (p2.x - p1.x, p2.y - p1.y);
            let (lu, lv) = ((ux * ux + uy * uy).sqrt(), (vx * vx + vy * vy).sqrt());
            if lu > 1e-6 && lv > 1e-6 {
                let cosang = (ux * vx + uy * vy) / (lu * lv);
                if cosang.abs() < 0.06 {
                    project.add_constraint_if_independent(si, Constraint::Perpendicular { a: pa, b: a, c: a, d: b });
                }
            }
        }
    }
    // 2b) PARALLEL to the nearest non-axis line (when the new one did not land on an axis itself)
    if !axis_aligned {
        if let Some((la, lb)) = qymcad_pick::nearest_parallel_line(project, si, p1, p2, a, b) {
            project.add_constraint_if_independent(si, Constraint::Parallel { a: la, b: lb, c: a, d: b });
        }
    }
    // 2c) TANGENT to the nearest circle or arc that is almost tangent already
    if let Some((cen, r)) = qymcad_pick::nearest_tangent_circle(project, si, p1, p2) {
        project.add_constraint_if_independent(si, Constraint::Tangent { a, b, c: cen, r });
    }
    // 2d) EQUAL LENGTH to the nearest line of the same length
    if let Some((la, lb)) = qymcad_pick::nearest_equal_line(project, si, p1, p2, a, b) {
        project.add_constraint_if_independent(si, Constraint::Equal { a: la, b: lb, c: a, d: b });
    }
    // 3) a point on an edge: the new end landed on an existing line (not its own), so it is tied to it - and so is the start
    // of the first segment of a chain; the start of a next segment is the end of the one before, tied already
    tie_to_the_line_under(project, view, si, LandedOn { at: p2, point: b, own: (a, b) });
    if prev.is_none() {
        tie_to_the_line_under(project, view, si, LandedOn { at: p1, point: a, own: (a, b) });
    }
    // (a coincidence with a vertex happens by itself: `sketch_point_at(1e-6)` already shares the point)
    project.solve_sketch(si);
}

/// WHICH SIDE OF PUTTING A RECTANGLE `tie_the_rect_to_what_is_under` STANDS ON.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum RectPut {
    /// before it is made: the edges of the body under the clicks taken in
    Before,
    /// after: its points at the clicks tied to what they landed on
    After,
}

/// A RECTANGLE CLICKED AT `clicks` TIED TO WHAT IS UNDER THEM, as the ends of a line are, when the automatic constraints
/// are on and it is not construction.
pub(crate) fn tie_the_rect_to_what_is_under(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, si: usize, clicks: &[Point2], put: RectPut) {
    if !tied(sk) {
        return;
    }
    match put {
        RectPut::Before => {
            for at in clicks {
                auto_project_under(sk, rect, si, *at);
            }
        }
        RectPut::After => {
            let own: Vec<Id> = sk.project.sketches[si].rects.last().map(|r| r.corners.iter().copied().chain([r.centre]).collect()).unwrap_or_default();
            tie_the_clicked_points(&mut *sk.project, *sk.view, si, clicks, &own);
        }
    }
}

/// WHETHER WHAT IS DRAWN NOW IS TIED TO WHAT IT LANDS ON (`qymcad_ui_state::ties_laid`).
pub(crate) fn tied(sk: &qymcad_ui_state::SketchCtx) -> bool {
    qymcad_ui_state::ties_laid(sk.tool, sk.set)
}

/// Where the two ends of line `seg` of sketch `si` stand, first end first.
pub(crate) fn segment_ends(project: &Project, si: usize, seg: Id) -> Option<[Point2; 2]> {
    let s = project.sketches.get(si)?;
    let qymcad_core::model::EntityKind::Line { a, b } = s.entities.iter().find(|e| e.id == seg)?.kind else { return None };
    let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
    Some([at(a)?, at(b)?])
}
