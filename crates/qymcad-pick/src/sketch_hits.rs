//! WHAT IS UNDER THE POINTER IN A SKETCH: points, lines, circles, arcs, tangents, the sketch in space.

use super::*;

/// Find the INDEX of a face `(mi, fi)` of `body` by the key `key`: first by the PERSISTENT id,
/// otherwise by a fallback match — an aligned normal plus the nearest centre (as `Project::resolve_face`
/// does, but returning an index for `Sel::Face`). Needed to restore the selection of a face when the
/// shell or the hole is reopened.
pub fn resolve_face_sel(project: &Project, body: Id, key: &qymcad_core::feature::FaceKey) -> Option<(usize, usize)> {
    let mi = project.mesh_index(body)?;
    let faces = &project.bodies.get(mi)?.faces;
    if key.id != 0 {
        if let Some(fi) = faces.iter().position(|f| f.id == key.id) {
            return Some((mi, fi));
        }
    }
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let d2 = |c: &qymcad_core::geom::Point3| (c.x - key.centroid[0]).powi(2) + (c.y - key.centroid[1]).powi(2) + (c.z - key.centroid[2]).powi(2);
    let fi = faces
        .iter()
        .enumerate()
        .filter(|(_, f)| dot(f.normal, key.normal) > 0.9)
        .min_by(|(_, a), (_, b)| d2(&a.centroid).partial_cmp(&d2(&b.centroid)).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)?;
    Some((mi, fi))
}

/// The sketch point of `si` nearest to a screen position (within the threshold) -> its Id.
///
/// THE FRAME OF REFERENCE IS NOT GEOMETRY, and this is where that has to be honoured, because four points
/// stand at or beside zero in any sketch a person has worked in: the origin, the anchor of the frame at the
/// same (0,0), and the two axis guides at (1,0) and (0,1). At the ordinary zoom of 6 px per mm a guide is 6
/// px away while the radius for a point is 10 - so a click at zero has four candidates and three of them
/// belong to the frame.
///
/// The rule is the one `sketch_hit` already keeps for selection: the anchor and the guides are never handed
/// out, and the origin yields to the person's own point standing in the same place. Without it a dimension
/// clicked at zero attached to the anchor - a point that is drawn nowhere and can never be clicked again.
pub fn nearest_sketch_point(pick: &PickCtx, rect: Rect, screen: Pos2, si: usize) -> Option<Id> {
    let s = pick.project.sketches.get(si)?;
    // a system id of 0 means "not materialised yet" and must not match a real point
    let is = |sys: Id, id: Id| sys != 0 && sys == id;
    let mut best: Option<(f32, Id)> = None;
    let mut best_origin: Option<(f32, Id)> = None;
    for p in &s.points {
        if is(s.frame, p.id) || s.axis_pts.iter().any(|g| is(*g, p.id)) {
            continue; // the anchor and the guides are the frame, not the drawing
        }
        let d = (qymcad_ui_state::Sheet { view: *pick.view, rect }).at(Point2::new(p.x, p.y)).distance(screen);
        if d > grab(pick.set, Grab::Point) {
            continue;
        }
        let slot = if is(s.origin, p.id) { &mut best_origin } else { &mut best };
        if slot.is_none_or(|(bd, _)| d < bd) {
            *slot = Some((d, p.id));
        }
    }
    best.or(best_origin).map(|(_, id)| id)
}

/// The line entity nearest to a screen point -> ITS OWN id (not its ends).
///
/// A method of its own precisely because the neighbouring `nearest_line_entity` returns THE ENDS of a
/// line: while choosing the axis of a revolution the id of a point was compared against a list of
/// lines, there was never a match, and the half-sketcher opened while not one line could be
/// selected.
pub fn nearest_line_id(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize, only: &[Id]) -> Option<Id> {
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect };
    use qymcad_core::model::EntityKind;
    let s = pick.project.sketches.get(si)?;
    let mut best: Option<(f32, Id)> = None;
    for e in &s.entities {
        if !only.is_empty() && !only.contains(&e.id) {
            continue;
        }
        if let EntityKind::Line { a, b } = e.kind {
            if let (Some(pa), Some(pb)) = (sketch_pt(pick.project, si, a), sketch_pt(pick.project, si, b)) {
                let d = screen_dist_seg(pos, sh.at(pa), sh.at(pb));
                if d <= grab(pick.set, Grab::Curve) && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, e.id));
                }
            }
        }
    }
    best.map(|(_, id)| id)
}

/// The line entity nearest to a screen point -> its ends (a, b).
pub fn nearest_line_entity(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize) -> Option<(Id, Id)> {
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect };
    use qymcad_core::model::EntityKind;
    let s = pick.project.sketches.get(si)?;
    // the hierarchy: an ordinary line outranks a construction one (for dimensions and constraints)
    let mut best: Option<(u8, f32, (Id, Id))> = None;
    for e in &s.entities {
        if let EntityKind::Line { a, b } = e.kind {
            if let (Some(pa), Some(pb)) = (sketch_pt(pick.project, si, a), sketch_pt(pick.project, si, b)) {
                let d = screen_dist_seg(pos, sh.at(pa), sh.at(pb));
                if d <= grab(pick.set, Grab::Curve) {
                    let tier = if e.construction { 1u8 } else { 0u8 };
                    if best.is_none_or(|(bt, bd, _)| (tier, d) < (bt, bd)) {
                        best = Some((tier, d, (a, b)));
                    }
                }
            }
        }
    }
    best.map(|(_, _, ab)| ab)
}

/// The vertex (sketch point) nearest to a screen point within the tolerance — for clicking a corner.
///
/// ONE FUNCTION, NOT TWO. This was a byte-for-byte copy of `nearest_sketch_point`, and the copy is exactly
/// how a rule gets kept in one door and forgotten in the other: the fix for the frame would have landed in
/// one of them.
pub fn nearest_vertex(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
    nearest_sketch_point(pick, rect, pos, si)
}

/// THE SAME POINT, WITH A WIDER REACH: the zone in which a click names a corner at that point rather than a line.
///
/// The reach of a pick of a point is right for PICKING a point and wrong for CONFIRMING a corner - a person aims
/// at a point rather than at a spot inside a disc a shade wider than the dot, so the corner tools ask for the
/// zone and the picks keep their own tolerance. One door: the rule about the frame of reference (the anchor and
/// the guides are never handed out, and the origin yields to the person's own point in the same place) belongs
/// to the point, not to the tolerance, so it is read from `nearest_sketch_point` rather than written out again.
pub fn nearest_point_within(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize, reach: f32) -> Option<Id> {
    let s = pick.project.sketches.get(si)?;
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect };
    let is = |sys: Id, id: Id| sys != 0 && sys == id;
    let mut best: Option<(f32, Id)> = None;
    let mut best_origin: Option<(f32, Id)> = None;
    for p in &s.points {
        if is(s.frame, p.id) || s.axis_pts.iter().any(|g| is(*g, p.id)) {
            continue; // the anchor and the guides are the frame, not the drawing
        }
        let d = sh.at(Point2::new(p.x, p.y)).distance(pos);
        if d > reach {
            continue;
        }
        let slot = if is(s.origin, p.id) { &mut best_origin } else { &mut best };
        if slot.is_none_or(|(bd, _)| d < bd) {
            *slot = Some((d, p.id));
        }
    }
    best.or(best_origin).map(|(_, id)| id)
}

/// The nearest line entity -> the Id of the entity (for trimming).
pub fn nearest_line_eid(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect };
    use qymcad_core::model::EntityKind;
    let s = pick.project.sketches.get(si)?;
    let mut best: Option<(f32, Id)> = None;
    for e in &s.entities {
        if let EntityKind::Line { a, b } = e.kind {
            if let (Some(pa), Some(pb)) = (sketch_pt(pick.project, si, a), sketch_pt(pick.project, si, b)) {
                let d = screen_dist_seg(pos, sh.at(pa), sh.at(pb));
                if d <= grab(pick.set, Grab::Curve) && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, e.id));
                }
            }
        }
    }
    best.map(|(_, id)| id)
}

/// HOW FAR A SCREEN POINT IS FROM AN ARC, on screen: to its outline where the point stands within the arc's sweep, to the
/// nearer end past it. Measured round the whole circle the arc lies on, a click where the cut half of a trimmed circle
/// of radius 10 had been - (0, -10), nothing drawn there - took the upper half left.
pub fn arc_screen_dist(sh: &qymcad_ui_state::Sheet, pos: Pos2, c: Point2, pa: Point2, pb: Point2, ccw: bool) -> f32 {
    use std::f64::consts::TAU;
    let sc = sh.at(c);
    let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
    let rp = (sh.at(Point2::new(c.x + r, c.y)).x - sc.x).abs();
    let w = qymcad_ui_state::to_world(&sh.view, sh.rect, pos);
    let ang = |x: f64, y: f64| (y - c.y).atan2(x - c.x);
    let (a0, a1, at) = (ang(pa.x, pa.y), ang(pb.x, pb.y), ang(w.x, w.y));
    let along = |from: f64, to: f64| if ccw { (to - from).rem_euclid(TAU) } else { (from - to).rem_euclid(TAU) };
    if along(a0, at) <= along(a0, a1) {
        (sc.distance(pos) - rp).abs()
    } else {
        sh.at(pa).distance(pos).min(sh.at(pb).distance(pos))
    }
}

/// The circle or arc entity nearest to a screen point -> its Id. It catches both the outline and the
/// diameter or radius label (the position of the label is where `draw_sketch_dims` draws it).
pub fn nearest_circle_entity(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect };
    use qymcad_core::model::EntityKind;
    let s = pick.project.sketches.get(si)?;
    let mut best: Option<(f32, Id)> = None;
    for e in &s.entities {
        // an arc is caught along its own sweep (see `arc_screen_dist`)
        let mut past_ends: Option<f32> = None;
        let (center, r, label) = match e.kind {
            EntityKind::Circle { center, r } => {
                let Some(c) = sketch_pt(pick.project, si, center) else { continue };
                let sc = sh.at(c);
                let ed = sh.at(Point2::new(c.x + r, c.y));
                // the diameter label sits above the middle of the leader from the centre to the rim
                (c, r, ((sc.to_vec2() + ed.to_vec2()) / 2.0 + egui::vec2(0.0, -8.0)).to_pos2())
            }
            EntityKind::Arc { center, a, b, .. } => {
                let (Some(c), Some(pa), Some(pb)) = (sketch_pt(pick.project, si, center), sketch_pt(pick.project, si, a), sketch_pt(pick.project, si, b)) else { continue };
                let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                // the radius label sits at the rim of the arc towards its middle (as in
                // `draw_sketch_dims`)
                let mid = Point2::new((pa.x + pb.x) / 2.0 - c.x, (pa.y + pb.y) / 2.0 - c.y);
                let ml = (mid.x * mid.x + mid.y * mid.y).sqrt().max(1e-9);
                let edge = Point2::new(c.x + mid.x / ml * r, c.y + mid.y / ml * r);
                let EntityKind::Arc { ccw, .. } = e.kind else { continue };
                past_ends = Some(arc_screen_dist(&sh, pos, c, pa, pb, ccw));
                (c, r, (sh.at(edge).to_vec2() + egui::vec2(10.0, -8.0)).to_pos2())
            }
            _ => continue,
        };
        let sc = sh.at(center);
        let rp = (sh.at(Point2::new(center.x + r, center.y)).x - sc.x).abs();
        let d_out = past_ends.unwrap_or_else(|| (sc.distance(pos) - rp).abs()); // by the outline of the circle, or the arc
        let d_label = label.distance(pos); // by the diameter or radius label
        let d = d_out.min(d_label);
        if d <= grab(pick.set, Grab::Label) && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, e.id));
        }
    }
    best.map(|(_, id)| id)
}

/// The circle or arc that the segment p1-p2 is ALMOST TANGENT to (the distance from the centre to the
/// line is about the radius, and the point of tangency lies inside the segment). Returns (the Id of the
/// centre, the radius). For the automatic tangency.
pub fn nearest_tangent_circle(project: &Project, si: usize, p1: Point2, p2: Point2) -> Option<(Id, f64)> {
    use qymcad_core::model::EntityKind;
    let s = project.sketches.get(si)?;
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt = |id: Id| points_by_id.get(&id).copied().map(|q| (q.x, q.y));
    let (dx, dy) = (p2.x - p1.x, p2.y - p1.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        return None;
    }
    let mut best: Option<(f64, (Id, f64))> = None;
    for e in &s.entities {
        let (center, r) = match e.kind {
            EntityKind::Circle { center, r } => (center, r),
            EntityKind::Arc { center, a, .. } => match (pt(center), pt(a)) {
                (Some((cx, cy)), Some((ax, ay))) => (center, ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt()),
                _ => continue,
            },
            _ => continue,
        };
        let Some((cx, cy)) = pt(center) else { continue };
        // the signed perpendicular distance from the centre to the line, plus where the projection
        // falls on the segment
        let dist = ((dx * (cy - p1.y) - dy * (cx - p1.x)) / len).abs();
        let t = ((cx - p1.x) * dx + (cy - p1.y) * dy) / (len * len);
        let err = (dist - r).abs();
        if err < 0.06 * r && t > -0.1 && t < 1.1 && best.is_none_or(|(be, _)| err < be) {
            best = Some((err, (center, r)));
        }
    }
    best.map(|(_, v)| v)
}

/// The ends of an ADJACENT line (sharing an end with p1 or p2) whose length is about the length of the
/// segment p1-p2 — for the automatic equal-length constraint. The requirement of adjacency plus a hard
/// tolerance of 2% guard against false matches of length with distant unrelated geometry. The line's
/// own ends (ea, eb) are excluded.
pub fn nearest_equal_line(project: &Project, si: usize, p1: Point2, p2: Point2, ea: Id, eb: Id) -> Option<(Id, Id)> {
    use qymcad_core::model::EntityKind;
    let s = project.sketches.get(si)?;
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt = |id: Id| points_by_id.get(&id).copied().map(|q| (q.x, q.y));
    let ln = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt();
    if ln < 1e-3 {
        return None;
    }
    let near = |x: f64, y: f64, p: Point2| (x - p.x).abs() < 1e-4 && (y - p.y).abs() < 1e-4;
    let mut best: Option<(f64, (Id, Id))> = None;
    for e in &s.entities {
        let EntityKind::Line { a, b } = e.kind else { continue };
        if (a == ea && b == eb) || (a == eb && b == ea) {
            continue;
        }
        let (Some((ax, ay)), Some((bx, by))) = (pt(a), pt(b)) else { continue };
        // adjacency: an end shared with the new segment (by position, which works in the preview too)
        let adj = near(ax, ay, p1) || near(ax, ay, p2) || near(bx, by, p1) || near(bx, by, p2);
        if !adj {
            continue;
        }
        let le = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
        if le < 1e-3 {
            continue;
        }
        let rel = (ln - le).abs() / ln.max(le);
        if rel < 0.02 && best.is_none_or(|(br, _)| rel < br) {
            best = Some((rel, (a, b)));
        }
    }
    best.map(|(_, v)| v)
}

/// The ends of the NON-axis line of the segment's own shape most nearly parallel to the segment p1-p2 (for the
/// automatic parallel constraint).
///
/// ONLY A LINE OF THE SAME SHAPE: one joined to the segment through a chain of shared points, the segment's ends found
/// by their ids `ea`, `eb` or, in the preview where it has none yet, by where they stand. Any non-axis line of the
/// sketch within 3.4 deg used to be taken wherever it stood, and a second triangle drawn beside the first was tied to it
/// by three Parallels - an edit of one moved the other.
pub fn nearest_parallel_line(project: &Project, si: usize, p1: Point2, p2: Point2, ea: Id, eb: Id) -> Option<(Id, Id)> {
    use qymcad_core::model::EntityKind;
    let s = project.sketches.get(si)?;
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt = |id: Id| points_by_id.get(&id).copied().map(|q| Point2::new(q.x, q.y));
    let at_end = |q: &qymcad_core::model::SketchPoint| [p1, p2].iter().any(|p| (q.x - p.x).abs() < 1e-4 && (q.y - p.y).abs() < 1e-4);
    let mut shape: std::collections::HashSet<Id> = s.points.iter().filter(|q| q.id == ea || q.id == eb || at_end(q)).map(|q| q.id).collect();
    loop {
        let before = shape.len();
        for e in &s.entities {
            let ends = qymcad_core::model::entity_points(e);
            if ends.iter().any(|p| shape.contains(p)) {
                shape.extend(ends);
            }
        }
        if shape.len() == before {
            break;
        }
    }
    let (vx, vy) = (p2.x - p1.x, p2.y - p1.y);
    let lv = (vx * vx + vy * vy).sqrt();
    if lv < 1e-6 {
        return None;
    }
    let mut best: Option<(f64, (Id, Id))> = None;
    for e in &s.entities {
        let EntityKind::Line { a, b } = e.kind else { continue };
        if (a == ea && b == eb) || (a == eb && b == ea) {
            continue;
        }
        if !shape.contains(&a) && !shape.contains(&b) {
            continue; // a line of another shape
        }
        let (Some(pa), Some(pb)) = (pt(a), pt(b)) else { continue };
        let (ux, uy) = (pb.x - pa.x, pb.y - pa.y);
        let lu = (ux * ux + uy * uy).sqrt();
        if lu < 1e-6 {
            continue;
        }
        // non-axis (otherwise the horizontal or vertical constraint fires) and almost parallel
        let axis = (ux.abs() <= uy.abs() * 0.06) || (uy.abs() <= ux.abs() * 0.06);
        let cross = (ux * vy - uy * vx).abs() / (lu * lv);
        if !axis && cross < 0.06 && best.is_none_or(|(bc, _)| cross < bc) {
            best = Some((cross, (a, b)));
        }
    }
    best.map(|(_, e)| e)
}

/// THE SKETCH TO LIGHT UNDER THE CURSOR, or nothing.
///
/// Answers only while a tool is WAITING for a sketch: outside that, an outline lighting under the cursor
/// would say a click does something it does not.
pub fn sketch_hover_3d(pn: &Painting, rect: Rect, at: Option<Pos2>, waiting: Option<u8>) -> Option<usize> {
    let at = at.filter(|p| rect.contains(*p))?;
    waiting?;
    sketch_at_3d(pn, rect, at)
}

/// THE SKETCH UNDER THE CURSOR IN THE 3D VIEW, by its index.
///
/// Reported behaviour: "the sketch can be picked in the tree or by clicking it in the 3D viewport, and it
/// highlights the way faces, edges and surfaces do in the other tools." It could not: the outlines are
/// DRAWN in 3D and were highlighted once selected, but nothing turned a click there into a selection - the
/// tree was the only way in.
///
/// PROJECTED THE WAY IT IS DRAWN. The outlines of a sketch on a plane of its own are lifted onto that
/// plane before projecting (`sketch_frame`), so a hit test that skipped the lift would answer about a
/// sketch lying somewhere the person cannot see it.
pub fn sketch_at_3d(pn: &Painting, rect: Rect, screen: Pos2) -> Option<usize> {
    let basis = pn.cam.basis();
    let scr = |w: [f64; 3]| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(w).0;
    let foreign = foreign_contour_ids(pn);
    let reach = qymcad_ui_state::grab::grab(pn.set, Grab::Curve);
    let mut best: Option<(usize, f32)> = None;
    for si in 0..pn.project.sketches.len() {
        // ASKED OF THE SKETCH ITSELF, not through its contours: the walk is per sketch here, and the set of
        // hidden ones is kept by sketch. Going the other way would need the drawing crate, which picking
        // must not depend on.
        if pn.sketch_hidden.contains(&pn.project.sketches[si].id) {
            continue; // hidden by its own checkbox - what is not drawn cannot be pointed at
        }
        if !qymcad_ui_state::sketch_shown_by_components(pn.project, pn.project.sketches[si].id, qymcad_ui_state::current_ctx_id(pn.active_path, pn.project)) {
            continue; // hidden with its part - not drawn either
        }
        let frame = pn.project.sketch_frame(si).filter(|f| !f.is_identity());
        let lift = |q: Point2| -> [f64; 3] {
            match frame {
                Some(f) => {
                    let w = f.lift(q);
                    [w.x, w.y, w.z]
                }
                None => [q.x, q.y, 0.0],
            }
        };
        for &cid in &pn.project.sketches[si].contour_ids {
            if foreign.contains(&cid) {
                continue; // a sketch of another component - outside the active context
            }
            let Some(ci) = pn.project.contour_index(cid) else { continue };
            let c = &pn.project.contours[ci];
            if c.points.len() < 2 {
                continue;
            }
            let n = c.points.len();
            let last = if c.closed { n } else { n - 1 };
            for k in 0..last {
                let (a, b) = (scr(lift(c.points[k])), scr(lift(c.points[(k + 1) % n])));
                let d = qymcad_ui_state::screen_dist_seg(screen, a, b);
                if d <= reach && best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((si, d));
                }
            }
        }
    }
    best.map(|(si, _)| si)
}

/// The tangent direction at the point `p` (the end of an existing line or arc), plus the base edge.
/// For a tangent arc: the new arc starts off smoothly from the end of the previous curve.
pub fn arc_tangent_ref(dc: &DrawCtx, si: usize, p: Point2) -> Option<((f64, f64), EdgeRef)> {
    use qymcad_core::model::EntityKind;
    let s = dc.project.sketches.get(si)?;
    let near = |q: Point2| (q.x - p.x).abs() < 1e-6 && (q.y - p.y).abs() < 1e-6;
    let norm = |x: f64, y: f64| {
        let l = (x * x + y * y).sqrt().max(1e-9);
        (x / l, y / l)
    };
    for e in &s.entities {
        match e.kind {
            EntityKind::Line { a, b } => {
                if let (Some(pa), Some(pb)) = (sketch_pt(dc.project, si, a), sketch_pt(dc.project, si, b)) {
                    if near(pa) {
                        return Some((norm(pa.x - pb.x, pa.y - pb.y), EdgeRef::Line { a, b }));
                    }
                    if near(pb) {
                        return Some((norm(pb.x - pa.x, pb.y - pa.y), EdgeRef::Line { a, b }));
                    }
                }
            }
            EntityKind::Arc { center, a, b, .. } => {
                if let (Some(pc), Some(pa), Some(pb)) = (sketch_pt(dc.project, si, center), sketch_pt(dc.project, si, a), sketch_pt(dc.project, si, b)) {
                    let r = ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2)).sqrt();
                    if near(pa) {
                        return Some((norm(-(pa.y - pc.y), pa.x - pc.x), EdgeRef::Circle { center, r }));
                    }
                    if near(pb) {
                        return Some((norm(-(pb.y - pc.y), pb.x - pc.x), EdgeRef::Circle { center, r }));
                    }
                }
            }
            _ => {}
        }
    }
    None
}
