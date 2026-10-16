//! WHAT IS INFERRED AND SHOWN: hints, snaps, glyphs of constraints, anchors of mates, curves through.

use super::*;

/// The candidate automatic constraints for the segment p1 -> p2 being drawn — for a live preview
/// while drawing. Returns (glyph, the world point for the badge). Geometry only, nothing applied.
pub fn infer_hints(dc: &DrawCtx, si: usize, prev: Option<Point2>, p1: Point2, p2: Point2) -> Vec<(Gly, Point2)> {
    let mut out: Vec<(Gly, Point2)> = Vec::new();
    let (dx, dy) = ((p2.x - p1.x).abs(), (p2.y - p1.y).abs());
    let mid = Point2::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
    let tol = 0.06;
    let mut axis = false;
    if dy <= dx * tol && dx > 1e-6 {
        out.push((Gly::Horiz, mid));
        axis = true;
    } else if dx <= dy * tol && dy > 1e-6 {
        out.push((Gly::Vert, mid));
        axis = true;
    }
    // perpendicular to the previous one
    if !axis {
        if let Some(pv) = prev {
            let (ux, uy) = (p1.x - pv.x, p1.y - pv.y);
            let (vx, vy) = (p2.x - p1.x, p2.y - p1.y);
            let (lu, lv) = ((ux * ux + uy * uy).sqrt(), (vx * vx + vy * vy).sqrt());
            if lu > 1e-6 && lv > 1e-6 && ((ux * vx + uy * vy) / (lu * lv)).abs() < tol {
                out.push((Gly::Perp, p1));
            }
        }
        // parallel to the nearest non-axis line
        if nearest_parallel_line(dc.project, si, p1, p2, 0, 0).is_some() {
            out.push((Gly::Parallel, mid));
        }
    }
    // tangent to a circle or an arc
    if nearest_tangent_circle(dc.project, si, p1, p2).is_some() {
        out.push((Gly::Tangent, mid));
    }
    // equal in length to the nearest line
    if nearest_equal_line(dc.project, si, p1, p2, 0, 0).is_some() {
        out.push((Gly::Equal, p2));
    }
    // an end coinciding with a vertex, or a point on an edge, is shown by the shared snap hint (`snap_infer_glyph`)
    out
}

/// The joint whose degree-of-freedom gizmo is active in the viewport right now. Two ways in:
/// 1) a COMPONENT is selected — a direct child of the context, driven by a joint that has freedoms;
/// 2) the joint's glyph is selected DIRECTLY (`Sel::Joint`) — needed for a joint held at the ROOT whose
///    driven part lies inside a subassembly and is not a direct child (`gizmo_component()` gives None),
///    while the subassembly's slider still wants to be draggable by its handle straight from the root.
pub fn active_dof_joint(pn: &Painting) -> Option<Id> {
    if let Some(comp) = gizmo_component(pn.active_path, pn.project, pn.sel, pn.workbench) {
        if let CompGizmoMode::Joint(jid) = comp_gizmo_mode(pn, comp) {
            return Some(jid);
        }
    }
    if let Sel::Joint(jid) = pn.sel {
        let ctx = current_ctx_id(pn.active_path, pn.project);
        if let Some(j) = pn.project.joints.iter().find(|j| j.id == jid) {
            if pn.project.joint_in_context(j, ctx)
                && joint_giz_handles(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, jid).is_some_and(|g| !g.handles.is_empty())
            {
                return Some(jid);
            }
        }
    }
    None
}

/// The gizmo mode of the selected component: grounded gives None; driven by a joint that has a freedom
/// gives Joint(jid); otherwise (free, or a seed with no freedoms of its own) gives Free, the plain 6-DOF one.
pub fn comp_gizmo_mode(pn: &Painting, comp: Id) -> CompGizmoMode {
    if pn.project.is_grounded(comp) {
        return CompGizmoMode::None;
    }
    if let Some(jid) = pn.project.drive_joint_for(comp) {
        // at least one handle is free and not driven by an expression, so the degree-of-freedom gizmo applies
        if joint_giz_handles(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, jid).is_some_and(|g| !g.handles.is_empty()) {
            return CompGizmoMode::Joint(jid);
        }
        // there is a joint, but it has no freedoms (a rigid one) or every parameter is driven by an
        // expression, so the component is pinned
        return CompGizmoMode::None;
    }
    CompGizmoMode::Free
}

/// The glyph of the automatic constraint implied by the cursor's current SNAP (coinciding with a
/// vertex, centre, midpoint or intersection gives Coincident; on an edge it gives point-on-circle or
/// point-on-line). For the preview in ALL the tools: any click snapped to geometry will attach a
/// coincidence through the shared point.
///
/// The glyph is the tie a click there lays (`tie_what_landed` in the sketcher): none while a drawing tool lays no ties.
pub fn snap_infer_glyph(pn: &Painting) -> Option<(Gly, Point2)> {
    let (p, kind) = pn.snap_hint?;
    if pn.armed.draw_kind() != 0 && !qymcad_ui_state::ties_laid(pn.tool, pn.set) {
        return None;
    }
    let g = match kind {
        0 | 3 | 4 => Gly::Coincident, // a vertex, a midpoint or a centre
        // where two curves cross the point is tied to both; on one, at a line of the grid, it is a point on that curve
        5 if curves_through(pn, p).total() >= 2 => Gly::Coincident,
        5 | 6 if curves_through(pn, p).rims > 0 => Gly::PointOnCircle,
        5 | 6 => Gly::PointOnLine,                        // on an edge
        2 if p.x == 0.0 && p.y == 0.0 => Gly::Coincident, // the origin, where the two axes meet
        2 => Gly::PointOnLine,                            // on an axis
        _ => return None,                                 // the grid: not a constraint
    };
    Some((g, p))
}

/// HOW MANY CURVES OF THE SKETCH PASS A POINT a snap put on them: lines and the axes, and circles and arcs.
pub(crate) struct Through {
    pub(crate) lines: usize,
    pub(crate) rims: usize,
}

impl Through {
    pub(crate) fn total(&self) -> usize {
        self.lines + self.rims
    }
}

/// The curves of the sketch being edited that `p` lies on, as a snap puts it on them - exactly, within 1e-6 mm.
pub(crate) fn curves_through(pn: &Painting, p: Point2) -> Through {
    let Some(si) = qymcad_ui_state::edit_si(pn.project, &pn.sketch_ses) else { return Through { lines: 0, rims: 0 } };
    let dc = DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path };
    let qymcad_ui_state::ActiveEdges { lines, circles } = qymcad_ui_state::active_edges(&dc, si);
    let on_line = |(a, b): &(Point2, Point2)| qymcad_ui_state::project_on_seg(p, *a, *b).is_some_and(|q| (q.x - p.x).hypot(q.y - p.y) < 1e-6);
    let axes = usize::from(p.x.abs() < 1e-6) + usize::from(p.y.abs() < 1e-6);
    Through {
        lines: lines.iter().filter(|l| on_line(l)).count() + axes,
        rims: circles.iter().filter(|rim| ((p.x - rim.centre.x).hypot(p.y - rim.centre.y) - rim.radius).abs() < 1e-6 && rim.holds(p)).count(),
    }
}

/// The contours of sketches that do NOT belong to the active context (its subtree) — they are hidden
/// both in the 2D sketcher and in the 3D overlay (one source for both drawing loops).
pub fn foreign_contour_ids(pn: &Painting) -> std::collections::HashSet<Id> {
    // ASKED FIRST, FILTERED AFTER. `sketch_in_ctx` takes the context now, and a closure that calls it
    // while the document is being walked borrows the same record twice.
    // ONE COPY OF THE RULE. Inlining "does this sketch belong to the context" here left the same rule
    // written twice; the free function takes exactly what it reads instead, so drawing needs no `&mut`.
    pn.project.sketches.iter().filter(|s| !sketch_in_ctx(pn.active_path, pn.project, pn.sketch_ses, s.id)).flat_map(|s| s.contour_ids.iter().copied()).collect()
}

pub fn sketch_ref_edges_2d(cache: &Caches, cmd: &FeatCommand, live: &LiveGeom, project: &qymcad_core::model::Project, regen: &Rebuilding, si: usize) -> Vec<Vec<Point2>> {
    use qymcad_core::feature::SketchPlane;
    let s = match project.sketches.get(si) {
        Some(s) => s,
        None => return Vec::new(),
    };
    // THE BODY IS TAKEN LIVE, not the one recorded when the sketch was created.
    //
    // `SketchPlane::Face(b, _)` holds the id of the body AT THE MOMENT of creation, while every subsequent
    // operation creates a new body and consumes the previous one. It looked like this: extrude a square,
    // make a hole, cut the chamfers - and the sketcher shows an overlay of both the original square and
    // the chamfered body. There was no overlay: exactly one contour was drawn, but of the CONSUMED body,
    // that is, geometry out of the past on top of the current model.
    let body = match s.plane {
        SketchPlane::Face(b, _) => Some(project.live_body(b)),
        _ => cmd.ref_body,
    };
    let (Some(body), Some(frame)) = (body, project.sketch_frame(si)) else { return Vec::new() };
    let Some(shape) = live.shapes.get(&body) else { return Vec::new() };
    let rel = match (project.sketch_owner(s.id), project.body_owner(body)) {
        (Some(so), Some(bo)) if so != bo => project.relative_transform(bo, so),
        _ => qymcad_core::feature::PLACE_IDENTITY,
    };
    let ident = qymcad_core::feature::is_identity12(&rel);
    // a sketch on A FACE of a part projects the edges of THAT FACE ONLY (its contour: the outer one plus
    // the holes) rather than EVERY edge of the body. Otherwise the edges of the bottom and the sides were
    // flattened onto the plane of the sketch over the contour of the face, and both the square (the
    // unfilleted edges below and to the side) and the fillets showed at once. A non-zero `face_id` means
    // filtering by that face.
    // From the cache: this projection is drawn on every frame of a sketch edit, while pulling the edges
    // out of the B-rep costs as much as a rebuild - on a large part, drawing on a face itself became
    // ragged.
    let Some(edges) = body_edges_cached(cache, live, regen, body) else { return Vec::new() };
    let (polys, ids) = (&edges.polys, &edges.ids);
    let face_edges: Option<std::collections::HashSet<u32>> = match &s.plane {
        SketchPlane::Face(_, fid) if fid.id != 0 => Some(shape.face_edge_ids(fid.id).into_iter().collect()),
        _ => None,
    };
    polys
        .iter()
        .zip(ids.iter().copied())
        .filter(|(_, id)| face_edges.as_ref().is_none_or(|set| set.contains(id)))
        .map(|(poly, _)| poly)
        .map(|poly| {
            poly.iter()
                .map(|p| {
                    let mut l = [p[0] as f64, p[1] as f64, p[2] as f64];
                    if !ident {
                        l = qymcad_core::feature::apply12(&rel, l);
                    }
                    frame.project(qymcad_core::geom::Point3::new(l[0], l[1], l[2]))
                })
                .collect()
        })
        .collect()
}

/// The glyphs actually shown in the viewport. The "show constraints" toggle hides ORDINARY constraints
/// but NOT the conflicting ones: an error cannot be switched off by a tick box - otherwise the conflict
/// exists while the viewport is empty.
pub fn visible_constraint_glyphs(pn: &Painting, rect: Rect, si: usize) -> Vec<(usize, Pos2, Gly)> {
    if pn.win.constraints {
        return constraint_glyphs(&PickCtx { project: pn.project, set: pn.set, view: &pn.view }, rect, si);
    }
    let conflicts = sketch_diag(pn.cache, pn.project, si).conflicts;
    if conflicts.is_empty() {
        return Vec::new();
    }
    constraint_glyphs(&PickCtx { project: pn.project, set: pn.set, view: &pn.view }, rect, si).into_iter().filter(|(ci, _, _)| conflicts.contains(ci)).collect()
}

/// The anchor point for THE ORIGIN of a new sketch, as (u, v) in the axes of plane `sp`.
/// The priority: a vertex of a body (a firmer anchor) over a point on an edge. `None` means there is no
/// geometry nearby.
pub fn sketch_origin_snap(pn: &Painting, rect: Rect, pos: Pos2, sp: &qymcad_core::feature::SketchPlane) -> Option<Point2> {
    let w = pick_vertex_pos(pn, rect, pos).or_else(|| pick_edge_point(pn, rect, pos))?;
    let fr = world_frame_of_plane(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, sp)?;
    Some(fr.project(qymcad_core::geom::Point3::new(w[0], w[1], w[2])))
}

/// The positions of the constraint glyphs: (the index of the constraint, the screen point, the symbol).
/// One source for both drawing and hit testing (deleting by click).
pub fn constraint_glyphs(pick: &PickCtx, rect: Rect, si: usize) -> Vec<(usize, Pos2, Gly)> {
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect };
    use qymcad_core::model::Constraint;
    let Some(s) = pick.project.sketches.get(si) else { return Vec::new() };
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt = |id: Id| points_by_id.get(&id).copied().map(|p| Point2::new(p.x, p.y));
    let mid = |a: Id, b: Id| -> Option<Pos2> {
        let (pa, pb) = (pt(a)?, pt(b)?);
        Some(((sh.at(pa).to_vec2() + sh.at(pb).to_vec2()) / 2.0).to_pos2())
    };
    let mut out: Vec<(usize, Pos2, Gly)> = Vec::new();
    // THE SAME GLYPH AT THE SAME PLACE IS DRAWN ONCE.
    //
    // A regular hexagon is held by FIVE equality constraints, and all five share the same first side.
    // That gave five glyphs at one point, and spreading them out carried them 17 pixels to the right
    // each - on a shot of a sketch run they lined up in a row IN EMPTY SPACE, attached to nothing. There
    // is no sense in four copies: "this side equals the others" is said once. The remaining constraints
    // do not go anywhere - they are in the list of constraints, and they are picked from there.
    let mut seen: std::collections::HashSet<(u8, i32, i32)> = std::collections::HashSet::new();
    // THE LAYOUT SIZES OF THE GLYPHS ARE NAMED, not written as numbers on the spot. This is THE DISTANCE
    // BETWEEN badges, not a grab radius (that lives in `grab.rs` and obeys the pointing-precision
    // setting). The guard over the radii told these two meanings apart by an accident of formatting - by
    // the word `guard` on the same line; splitting the lines was enough to make it go red on code that
    // had not changed in substance.
    const BADGE_GAP: f32 = 16.0;
    const BADGE_STEP: f32 = 17.0;
    // The badge is placed above the point or the midpoint and spread out NEAR IT rather than in a row to
    // the right: a glyph a hundred pixels away from its geometry lies worse than an overlapped one.
    let place = |out: &mut Vec<(usize, Pos2, Gly)>, seen: &mut std::collections::HashSet<(u8, i32, i32)>, ci: usize, p: Option<Pos2>, g: Gly| {
        if let Some(p) = p {
            if !seen.insert((g as u8, (p.x / 4.0).round() as i32, (p.y / 4.0).round() as i32)) {
                return;
            }
            let base = p + egui::vec2(0.0, -15.0);
            let mut at = base;
            for k in 1..9 {
                if !out.iter().any(|(_, q, _)| q.distance(at) < BADGE_GAP) {
                    break;
                }
                // in a snake: two steps to the right, then a row lower - a compact block by the geometry
                at = base + egui::vec2(BADGE_STEP * (k % 3) as f32, -BADGE_STEP * (k / 3) as f32);
            }
            out.push((ci, at, g));
        }
    };
    let mut push = |out: &mut Vec<(usize, Pos2, Gly)>, ci: usize, p: Option<Pos2>, g: Gly| place(out, &mut seen, ci, p, g);
    let holders = s.corner_holders(); // a cut corner's own constraints show no badge: the corner holds itself by them
    for (ci, c) in s.constraints.iter().enumerate().filter(|(ci, _)| !holders.contains(ci)) {
        match *c {
            Constraint::Horizontal { a, b } => push(&mut out, ci, mid(a, b), Gly::Horiz),
            Constraint::Vertical { a, b } => push(&mut out, ci, mid(a, b), Gly::Vert),
            Constraint::Coincident { a, .. } => push(&mut out, ci, pt(a).map(|p| sh.at(p) + egui::vec2(8.0, 8.0)), Gly::Coincident),
            Constraint::Parallel { a, b, c: cc, d } => {
                push(&mut out, ci, mid(a, b), Gly::Parallel);
                push(&mut out, ci, mid(cc, d), Gly::Parallel);
            }
            Constraint::Perpendicular { a, b, c: cc, d } => {
                push(&mut out, ci, mid(a, b), Gly::Perp);
                push(&mut out, ci, mid(cc, d), Gly::Perp);
            }
            Constraint::Equal { a, b, c: cc, d } => {
                push(&mut out, ci, mid(a, b), Gly::Equal);
                push(&mut out, ci, mid(cc, d), Gly::Equal);
            }
            Constraint::Collinear { a, b, c: cc, d } => {
                push(&mut out, ci, mid(a, b), Gly::Collinear);
                push(&mut out, ci, mid(cc, d), Gly::Collinear);
            }
            Constraint::Tangent { a, b, .. } => push(&mut out, ci, mid(a, b), Gly::Tangent),
            Constraint::CircleTangent { c1, c2, .. } => push(&mut out, ci, mid(c1, c2), Gly::Tangent),
            Constraint::Symmetric { a, b, .. } => push(&mut out, ci, mid(a, b), Gly::Symmetric),
            // a `Fixed` on SYSTEM points (the origin, the ends of the axes) is not a constraint placed by
            // hand but a service anchor. No glyph is drawn for it (otherwise a dimension to an axis looks
            // as if it were breeding extra constraints).
            Constraint::Fixed { p } if !s.system_ids().contains(&p) => push(&mut out, ci, pt(p).map(|q| sh.at(q) + egui::vec2(8.0, 8.0)), Gly::Fix),
            Constraint::PointOnLine { p, .. } => push(&mut out, ci, pt(p).map(|q| sh.at(q) + egui::vec2(8.0, 8.0)), Gly::PointOnLine),
            // THE CENTRE OF A RECTANGLE SHOWS NO MIDPOINT GLYPH: the rectangle holds it there itself, as it holds its
            // turn, and the glyph beside the point took the click meant for the point - a constraint on the centre was
            // refused, the centre never picked
            Constraint::Midpoint { p, .. } if s.rects.iter().any(|r| r.centre == p) => {}
            Constraint::Midpoint { p, .. } => push(&mut out, ci, pt(p).map(|q| sh.at(q) + egui::vec2(8.0, 8.0)), Gly::Midpoint),
            Constraint::EqualRadius { c1, c2 } => {
                push(&mut out, ci, pt(c1).map(|q| sh.at(q) + egui::vec2(8.0, 8.0)), Gly::Equal);
                push(&mut out, ci, pt(c2).map(|q| sh.at(q) + egui::vec2(8.0, 8.0)), Gly::Equal);
            }
            Constraint::PointOnCircle { p, .. } => push(&mut out, ci, pt(p).map(|q| sh.at(q) + egui::vec2(8.0, 8.0)), Gly::PointOnCircle),
            Constraint::Concentric { c1, .. } => push(&mut out, ci, pt(c1).map(|q| sh.at(q) + egui::vec2(8.0, 8.0)), Gly::Concentric),
            _ => {}
        }
    }
    out
}

/// THE DIRECTION UNDER THE CURSOR — for "point at the axis", the second pick.
///
/// The rule is the same as for the anchor: what is pointed at is what is UNDER THE CURSOR. There
/// used to be an order of its own here — `pick_edge_any` over the whole frame first, then the face
/// — and it took the edge of ANY part as long as it came out closer to the cursor on screen.
/// Reported behaviour: pointing at a rail guide that runs along the horizon drew the axis along Z.
/// A measurement: the cursor was over body 18 while the edge belonged to body 6, a neighbour.
///
/// An edge is preferred to a face: when showing an axis, a person aims at something extended.
pub fn infer_axis_anchor(pn: &qymcad_ui_state::Painting, rect: Rect, pos: Pos2) -> Option<(Id, AnchorRef)> {
    let face = pick_part_face_at(pn, rect, pos);
    let edge = pick_edge_any(pn, rect, pos);
    let under = face.as_ref().map(|(b, _)| *b).or_else(|| edge.map(|(b, _)| b))?;
    if let Some((body, e)) = edge.filter(|(b, _)| *b == under) {
        return Some((body, AnchorRef::EdgeMid(body, e)));
    }
    let (body, key) = face?;
    Some((body, AnchorRef::FaceCenter(body, key)))
}

/// THE ANCHOR UNDER THE CURSOR: the body and what was caught on it.
///
/// `None` means there is no part under the cursor; only the caller has the right to call that a
/// miss, and the caller is also the one who says so in words.
pub fn infer_mate_anchor(pn: &qymcad_ui_state::Painting, rect: Rect, pos: Pos2) -> Option<(Id, AnchorRef)> {
    let face = pick_part_face_at(pn, rect, pos);
    let edge = pick_edge_any(pn, rect, pos);
    // THE ANCHOR IS TAKEN ONLY FROM THE BODY UNDER THE CURSOR.
    //
    // Snap points used to be collected FROM EVERY body whose bounding box was near the cursor, and
    // the nearest one on screen won — anybody's. On a single part that goes unnoticed, but in a
    // machine there is always a neighbour next to it: the reported behaviour was that pointing at
    // the start of a face, with both faces horizontal, gave a gizmo handle along the Z axis. A
    // measurement on that document: the cursor stood over body 7 while the program offered
    // `EdgeMid(6, 39)` — an edge of the NEIGHBOURING part, and the axis of travel came out as that
    // part's rather than the one being pointed at. The joint glyph away from the click came from
    // the same place.
    let under = face.as_ref().map(|(b, _)| *b).or_else(|| edge.map(|(b, _)| b))?;
    let mut best: Option<(f32, Id, AnchorRef)> = None;
    let mut offer = |d: f32, body: Id, a: AnchorRef| {
        if best.as_ref().is_none_or(|(bd, _, _)| d < *bd) {
            best = Some((d, body, a));
        }
    };
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let ctx = qymcad_ui_state::current_ctx_id(pn.active_path, pn.project);
    // THE CENTRE OF THE FACE UNDER THE CURSOR is as much a snap point as a vertex and takes part
    // on equal terms.
    if let Some((body, key)) = face {
        let wt = pn.project.body_display_transform(body, ctx);
        let w = qymcad_core::feature::apply12(&wt, key.centroid);
        offer(scr.at(w).0.distance(pos), body, AnchorRef::FaceCenter(body, key));
    }
    for (_mi, body) in qymcad_ui_state::shown_bodies(pn) {
        if body != under {
            continue; // another part gives up no anchor, however close its edge turns out to be
        }
        if !body_bbox_hit(pn, body, rect, pos, &basis, 12.0) {
            continue;
        }
        let Some(edges) = body_edges_cached(pn.cache, pn.live, pn.regen, body) else { continue };
        let wt = pn.project.body_display_transform(body, ctx);
        let to_world = |p: &[f32; 3]| -> [f64; 3] {
            let v = [p[0] as f64, p[1] as f64, p[2] as f64];
            if qymcad_core::feature::is_identity12(&wt) {
                v
            } else {
                qymcad_core::feature::apply12(&wt, v)
            }
        };
        let model = pn.project.regen_edges.get(&body);
        for (poly, id) in edges.polys.iter().zip(edges.ids.iter().copied()) {
            if id == 0 || poly.len() < 2 {
                continue;
            }
            // THE MIDPOINT OF AN EDGE comes from the model if it is raised: on a circular edge
            // that is the centre of the hole, and the polyline will not restore it. With no model
            // the middle of the polyline is taken, which is the same thing for a straight edge.
            let mid = match model.and_then(|es| es.iter().find(|e| e.id == id)) {
                Some(e) => {
                    let (p, _) = e.axis_ref();
                    if qymcad_core::feature::is_identity12(&wt) {
                        p
                    } else {
                        qymcad_core::feature::apply12(&wt, p)
                    }
                }
                None => to_world(&poly[poly.len() / 2]),
            };
            offer(scr.at(mid).0.distance(pos), body, AnchorRef::EdgeMid(body, id));
            for (at_end, v) in [(false, &poly[0]), (true, &poly[poly.len() - 1])] {
                offer(scr.at(to_world(v)).0.distance(pos), body, AnchorRef::Vertex(body, id, at_end));
            }
        }
    }
    if let Some((d, body, a)) = best {
        if d <= qymcad_ui_state::grab::grab(pn.set, Grab::Point) {
            return Some((body, a));
        }
    }
    // NO POINT WAS HIT — so something extended is being pointed at: an edge, and failing that a
    // face. The edge must be our own as well: another part's edge near the cursor gives no anchor.
    if let Some((body, e)) = edge.filter(|(b, _)| *b == under) {
        return Some((body, AnchorRef::EdgeMid(body, e)));
    }
    let (body, key) = face?;
    Some((body, AnchorRef::FaceCenter(body, key)))
}
