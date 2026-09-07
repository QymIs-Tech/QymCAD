//! WHAT THE CURSOR CATCHES.
//!
//! Given the document, the view and a point on screen, these functions say what is under it: a vertex, an
//! edge, a face, a body, a datum plane, a contour. They change nothing and draw nothing - which is why
//! every workbench can call them and none of them owns the picking.

use egui::{Color32, Pos2, Rect};
use qymcad_core::geom::Point2;
use qymcad_core::feature::AnchorRef;
use qymcad_core::model::{Id, Project};
use qymcad_ui_state::grab::{grab, Grab};
use qymcad_ui_state::*;

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
pub fn nearest_sketch_point(pick: &PickCtx, rect: Rect, screen: Pos2, si: usize) -> Option<Id> {
    let s = pick.project.sketches.get(si)?;
    let mut best: Option<(f32, Id)> = None;
    for p in &s.points {
        let d = (qymcad_ui_state::Sheet { view: *pick.view, rect: rect }).at(Point2::new(p.x, p.y)).distance(screen);
        if d <= grab(pick.set, Grab::Point) && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, p.id));
        }
    }
    best.map(|(_, id)| id)
}

/// The line entity nearest to a screen point -> ITS OWN id (not its ends).
///
/// A method of its own precisely because the neighbouring `nearest_line_entity` returns THE ENDS of a
/// line: while choosing the axis of a revolution the id of a point was compared against a list of
/// lines, there was never a match, and the half-sketcher opened while not one line could be
/// selected.
pub fn nearest_line_id(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize, only: &[Id]) -> Option<Id> {
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect: rect };
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
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect: rect };
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
pub fn nearest_vertex(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
    let s = pick.project.sketches.get(si)?;
    let mut best: Option<(f32, Id)> = None;
    for p in &s.points {
        let d = (qymcad_ui_state::Sheet { view: *pick.view, rect: rect }).at(Point2::new(p.x, p.y)).distance(pos);
        if d <= grab(pick.set, Grab::Point) && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, p.id));
        }
    }
    best.map(|(_, id)| id)
}

/// The nearest line entity -> the Id of the entity (for trimming).
pub fn nearest_line_eid(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect: rect };
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

/// The circle or arc entity nearest to a screen point -> its Id. It catches both the outline and the
/// diameter or radius label (the position of the label is where `draw_sketch_dims` draws it).
pub fn nearest_circle_entity(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect: rect };
    use qymcad_core::model::EntityKind;
    let s = pick.project.sketches.get(si)?;
    let mut best: Option<(f32, Id)> = None;
    for e in &s.entities {
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
                (c, r, (sh.at(edge).to_vec2() + egui::vec2(10.0, -8.0)).to_pos2())
            }
            _ => continue,
        };
        let sc = sh.at(center);
        let rp = (sh.at(Point2::new(center.x + r, center.y)).x - sc.x).abs();
        let d_out = (sc.distance(pos) - rp).abs(); // by the outline of the circle
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
    let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
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
    let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
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

/// The ends of the nearest NON-axis line almost parallel to the segment p1-p2 (for the automatic
/// parallel constraint).
pub fn nearest_parallel_line(project: &Project, si: usize, p1: Point2, p2: Point2, ea: Id, eb: Id) -> Option<(Id, Id)> {
    use qymcad_core::model::EntityKind;
    let s = project.sketches.get(si)?;
    let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
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

/// DOES THE SCREEN BOUNDING BOX OF A BODY CONTAIN THE CURSOR? A cheap cull before the expensive walk
/// over the edges.
///
/// At any moment there are one or two bodies under the cursor, and all of them were being walked. What
/// matters is not the cull itself but its PRICE: approaching a body (finding the mesh, the display
/// transform) cost 0.56 ms, and because of that the cull did not pay for itself. The corners of the
/// box in world coordinates are cached per rebuild, and eight projections are all that is left.
pub fn body_bbox_hit(pn: &Painting, body: qymcad_core::model::Id, rect: Rect, pos: Pos2, basis: &([f64; 3], [f64; 3], [f64; 3]), margin: f32) -> bool {
    let corners = {
        let mut c = pn.cache.bbox_world.borrow_mut();
        if c.rev != view_rev(pn.regen) {
            c.rev = view_rev(pn.regen);
            c.value.clear();
        }
        match c.value.get(&body) {
            Some(v) => *v,
            None => {
                let Some(mi) = pn.project.mesh_index(body) else { return true };
                let Some(bb) = pn.project.bodies[mi].mesh.bounds() else { return true };
                let wt = pn.project.body_display_transform(body, current_ctx_id(pn.active_path, pn.project));
                let mut out = [[0.0f64; 3]; 8];
                for (k, p) in [
                    [bb.min.x, bb.min.y, bb.min.z], [bb.max.x, bb.min.y, bb.min.z],
                    [bb.min.x, bb.max.y, bb.min.z], [bb.max.x, bb.max.y, bb.min.z],
                    [bb.min.x, bb.min.y, bb.max.z], [bb.max.x, bb.min.y, bb.max.z],
                    [bb.min.x, bb.max.y, bb.max.z], [bb.max.x, bb.max.y, bb.max.z],
                ]
                .into_iter()
                .enumerate()
                {
                    out[k] = if qymcad_core::feature::is_identity12(&wt) { p } else { qymcad_core::feature::apply12(&wt, p) };
                }
                c.value.insert(body, out);
                out
            }
        }
    };
    let (mut lo, mut hi) = (Pos2::new(f32::MAX, f32::MAX), Pos2::new(f32::MIN, f32::MIN));
    for w in corners {
        let p = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: basis }.at(w).0;
        lo = Pos2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Pos2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    pos.x >= lo.x - margin && pos.x <= hi.x + margin && pos.y >= lo.y - margin && pos.y <= hi.y + margin
}

/// THE EDGES OF A BODY FOR PICKING — from a cache rather than from the kernel every frame.
///
/// Extracting the edges of a live B-rep is expensive, and under the cursor it happens on every frame
/// and for every body. On a real assembly (1182 components) that literally hung the application while
/// an edge or vertex anchor was being chosen. The cache lives until the next rebuild of the
/// geometry.
pub fn body_edges_cached(cache: &Caches, live: &LiveGeom, regen: &Rebuilding, body: qymcad_core::model::Id) -> Option<std::rc::Rc<qymcad_ui_state::EdgePolys>> {
    {
        let c = cache.pick_edges.borrow();
        if c.rev == view_rev(regen) {
            if let Some(v) = c.value.get(&body) {
                return Some(v.clone()); // an `Rc`: a shared reference, not a copy of tens of thousands
                                        // of points
            }
        }
    }
    let shape = live.shapes.get(&body)?;
    // The kernel hands back a bare pair; it is named here, once, at the only place it is built.
    let (polys, ids) = shape.edges_with_ids();
    let v = std::rc::Rc::new(qymcad_ui_state::EdgePolys { polys, ids });
    let mut c = cache.pick_edges.borrow_mut();
    if c.rev != view_rev(regen) {
        c.rev = view_rev(regen);
        c.value.clear();
    }
    c.value.insert(body, v.clone());
    Some(v)
}

pub fn pick_contour(project: &Project, sel: &mut Sel, set: &Settings, view: View2d, rect: Rect, screen: Pos2) {
    let world = to_world(&view, rect, screen);
    let mut best: Option<(usize, f64)> = None;
    for (i, c) in project.contours.iter().enumerate() {
        let d = dist_to_contour(c, world);
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    let empty = match best {
        Some((_, d)) if d * view.scale as f64 <= grab(set, Grab::Curve) as f64 => false,
        _ => true,
    };
    if empty {
        // a click into emptiness clears the selection of an object (if a contour is selected)
        if matches!(sel, Sel::Contour(..)) {
            *sel = Sel::None;
        }
        return;
    }
    let (i, _) = best.unwrap();
    *sel = Sel::Contour(i);
}

/// THE VERTEX UNDER THE CURSOR among the ends of the SELECTED edges: its descriptor and its point.
///
/// Among the selected ones only — a variable radius makes sense on the set being rounded, and offering
/// the vertices of the whole part would clutter the aim with what will affect nothing anyway.
pub fn fillet_vertex_at(scr: &Screen, armed: &qymcad_ui_state::Armed, edges: &EdgeCache, gsel: &GeomSelection, project: &mut Project, screen: Pos2) -> Option<(u32, [f64; 3])> {
    if armed.cmd_kind() != 4 || gsel.edges.is_empty() {
        return None;
    }
    let body = edges.body?;
    let picked: Vec<[[f64; 3]; 2]> = project.regen_edges.get(&body)?.iter().filter(|e| gsel.edges.contains(&e.id)).map(|e| [e.a, e.b]).collect();
    let grab = grab(scr.set, Grab::Point);
    let mut best: Option<(f32, u32, [f64; 3])> = None;
    for c in project.vertex_pool(body) {
        let on_picked = picked.iter().flatten().any(|p| {
            (p[0] - c.centroid[0]).abs() < 1e-6 && (p[1] - c.centroid[1]).abs() < 1e-6 && (p[2] - c.centroid[2]).abs() < 1e-6
        });
        if !on_picked {
            continue;
        }
        let d = scr.at(c.centroid).0.distance(screen);
        if d <= grab && best.as_ref().is_none_or(|(bd, _, _)| d < *bd) {
            best = Some((d, c.desc, c.centroid));
        }
    }
    best.map(|(_, desc, p)| (desc, p))
}

/// Toggle the selection of the edge under the cursor (in 3D) by its PERSISTENT id. `true` means an
/// edge was hit.
/// WHICH EDGE IS UNDER THE CURSOR is a question in its own right, apart from any selection.
///
/// Two things ask it: a click (select the edge) and the right button (offer menu items for that edge).
/// One answer for both — otherwise the menu would offer a chain for one edge while the command took
/// another.
pub fn edge_at(active_path: &[Id], cam: Cam3, edges: &EdgeCache, project: &Project, set: &Settings, rect: Rect, screen: Pos2) -> Option<u32> {
    if edges.polys.is_empty() {
        return None;
    }
    let basis = cam.basis();
    let wt = edges.body.map(|b| project.body_display_transform(b, current_ctx_id(active_path, project))).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
    let tp = |p: &[f32; 3]| -> [f64; 3] {
        let v = [p[0] as f64, p[1] as f64, p[2] as f64];
        if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) }
    };
    // ON A COLLINEAR OVERLAP (a long seam edge containing a short one) both project into the same
    // screen line. The first one met used to win (usually the long one), so the short one could not be
    // selected by a click. Now, at an ALMOST equal distance (within 2 px), the SHORT edge is preferred
    // as the more specific one: over the overlapping stretch the short one is chosen, and the long one
    // is available where it is ALONE. Both are reachable.
    let mut best: Option<(f32, f32, usize)> = None; // (the least distance, the screen length, the index)
    for (i, poly) in edges.polys.iter().enumerate() {
        let pts: Vec<Pos2> = poly.iter().map(|p| qymcad_ui_state::Screen { cam: &cam, set: set, rect: rect, basis: &basis }.at(tp(p)).0).collect();
        let (mut d, mut slen) = (f32::MAX, 0.0f32);
        for k in 0..pts.len().saturating_sub(1) {
            d = d.min(screen_dist_seg(screen, pts[k], pts[k + 1]));
            slen += pts[k].distance(pts[k + 1]);
        }
        let better = match best {
            None => true,
            Some((bd, bl, _)) => d < bd - 2.0 || (d < bd + 2.0 && slen < bl),
        };
        if better {
            best = Some((d, slen, i));
        }
    }
    let (d, _, i) = best?;
    if d > grab(set, Grab::Curve) {
        return None;
    }
    edges.ids.get(i).copied().filter(|id| *id != 0)
}

pub fn pick_sketch_plane_at(pn: &Painting, rect: Rect, screen: Pos2) -> Option<qymcad_core::feature::SketchPlane> {
    use qymcad_core::feature::SketchPlane;
    let basis = pn.cam.basis();
    let face_best: Option<(f64, SketchPlane)> = face_under_cursor(pn, rect, screen).map(|(d, b, k)| (d, SketchPlane::Face(b, k)));
    // A base plane (XY/XZ/YZ or a datum) is compared with a face BY DEPTH rather than "a face always
    // wins". A plane (a fixed 60 mm square) used to be blocked SOLIDLY by any body under the cursor
    // (`body_hit`, even when the hit did not resolve into a valid face) — and at the enlarged,
    // scale-dependent size that would have blocked the plane almost everywhere a body appears on
    // screen. An honest comparison of depth also removes the flicker at the silhouette of a body — it
    // was reported as planes flashing yellow for a second and vanishing, with a shift of one pixel
    // curing it: `body_hit` used to jump between true and false at the boundary, while depth changes
    // continuously.
    let h = plane_pick_half_size(pn);
    let mut plane_best: Option<(f64, SketchPlane)> = None;
    for (sp, fr) in sketch_plane_candidates(pn) {
        let corners = [fr.lift(Point2::new(-h, -h)), fr.lift(Point2::new(h, -h)), fr.lift(Point2::new(h, h)), fr.lift(Point2::new(-h, h))];
        let pr: Vec<(Pos2, f64)> = corners.iter().map(|p| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis }.at([p.x, p.y, p.z])).collect();
        if point_in_tri(screen, pr[0].0, pr[1].0, pr[2].0) || point_in_tri(screen, pr[0].0, pr[2].0, pr[3].0) {
            let depth = pr.iter().map(|(_, d)| *d).sum::<f64>() / 4.0;
            if plane_best.is_none_or(|(bd, _)| depth < bd) {
                plane_best = Some((depth, sp));
            }
        }
    }
    match (face_best, plane_best) {
        (Some((fd, fsp)), Some((pd, psp))) => Some(if fd <= pd { fsp } else { psp }),
        (Some((_, fsp)), None) => Some(fsp),
        (None, Some((_, psp))) => Some(psp),
        (None, None) => None,
    }
}

/// Picking the axis of a circular array or of a datum axis by a click: the nearest DATUM AXIS (a line)
/// OR a STRAIGHT edge of ANY visible body (`axis_edges`) within the threshold, otherwise the axis of the
/// cylindrical face under the cursor.
pub fn pick_axis_at(pn: &Painting, rect: Rect, screen: Pos2) -> Option<AxisHit> {
    use qymcad_core::feature::apply12;
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis };
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let mut best: Option<(f32, AxisHit)> = None;
    // 1) the datum axes that exist
    for d in &pn.project.datum_axes {
        if let Some(wt) = datum_render_transform(pn, d.id) {
            let (s, e) = axis_segment(d.origin(), d.dir(), 45.0);
            let a = scr.at(apply12(&wt, s)).0;
            let b = scr.at(apply12(&wt, e)).0;
            let dist = screen_dist_seg(screen, a, b);
            if dist <= grab(pn.set, Grab::Curve) && best.is_none_or(|(bd, _)| dist < bd) {
                best = Some((dist, AxisHit::Datum(d.id)));
            }
        }
    }
    // 2) the straight edges of every visible body (`axis_edges`, each with its own display transform)
    for (i, (body, _id, poly)) in pn.edges.axes.iter().enumerate() {
        let wt = pn.project.body_display_transform(*body, ctx);
        let pts: Vec<Pos2> = poly.iter().map(|p| scr.at(apply12(&wt, [p[0] as f64, p[1] as f64, p[2] as f64])).0).collect();
        for k in 0..pts.len().saturating_sub(1) {
            let dist = screen_dist_seg(screen, pts[k], pts[k + 1]);
            if dist <= grab(pn.set, Grab::Curve) && best.is_none_or(|(bd, _)| dist < bd) {
                best = Some((dist, AxisHit::Edge(i)));
            }
        }
    }
    // LINES (a datum or an edge) are more precise — if one was hit it is taken; otherwise the
    // CYLINDRICAL face under the cursor is tried
    if let Some((_, h)) = best {
        return Some(h);
    }
    pick_cyl_face_axis_at(pn, rect, screen)
}

/// The DATUM POINT under the cursor -> (its Id, its world position). For a two-point axis (kept
/// parametric through `TwoPoints`).
pub fn pick_datum_point_at(pn: &Painting, rect: Rect, pos: Pos2) -> Option<(Id, [f64; 3])> {
    use qymcad_core::feature::apply12;
    let basis = pn.cam.basis();
    let mut best: Option<(f32, Id, [f64; 3])> = None;
    for d in &pn.project.datum_points {
        let Some(wt) = datum_render_transform(pn, d.id) else { continue };
        let w = apply12(&wt, d.at);
        let dist = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis }.at(w).0.distance(pos);
        if best.is_none_or(|(bd, _, _)| dist < bd) {
            best = Some((dist, d.id, w));
        }
    }
    best.filter(|(dist, _, _)| *dist <= grab(pn.set, Grab::Point)).map(|(_, id, w)| (id, w))
}

/// The WORLD COORDINATE of the nearest vertex (an end of an edge) of a visible body under the cursor —
/// for snapping a datum point.
pub fn pick_vertex_pos(pn: &Painting, rect: Rect, pos: Pos2) -> Option<[f64; 3]> {
    use qymcad_core::feature::{apply12, is_identity12};
    let basis = pn.cam.basis();
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let mut best: Option<(f32, [f64; 3])> = None;
    for (_mi, body) in shown_bodies(pn) {
        // a cull by the bounding box: a body whose screen rectangle does not cover the cursor is not
        // worth walking
        if !body_bbox_hit(pn, body, rect, pos, &basis, 12.0) {
            continue;
        }
        let Some(edges) = body_edges_cached(pn.cache, pn.live, pn.regen, body) else { continue };
        let (polys, ids) = (&edges.polys, &edges.ids);
        let wt = pn.project.body_display_transform(body, ctx);
        let tp = |p: &[f32; 3]| -> [f64; 3] {
            let v = [p[0] as f64, p[1] as f64, p[2] as f64];
            if is_identity12(&wt) { v } else { apply12(&wt, v) }
        };
        for (poly, id) in polys.iter().zip(ids.iter().copied()) {
            if id == 0 || poly.len() < 2 {
                continue;
            }
            for vert in [&poly[0], &poly[poly.len() - 1]] {
                let w = tp(vert);
                let d = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis }.at(w).0.distance(pos);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, w));
                }
            }
        }
    }
    best.filter(|(d, _)| *d <= grab(pn.set, Grab::Point)).map(|(_, w)| w)
}

/// THE VISIBLE BODIES — as a list from a cache rather than recomputed for every body every frame.
///
/// Deciding whether a body is visible requires finding its owner (a linear walk over the timeline) and
/// following the chain of tick boxes in the tree. That is not expensive in itself, but inside the
/// picking loop it repeats for EVERY body on EVERY frame — and it is exactly that which hung the
/// application while an edge or vertex anchor was being chosen.
pub fn shown_bodies(pn: &Painting) -> Vec<(usize, qymcad_core::model::Id)> {
    let ctx = current_ctx_id(pn.active_path, pn.project);
    {
        let c = pn.cache.shown_bodies.borrow();
        if c.rev == view_rev(pn.regen) && c.value.ctx == ctx {
            return c.value.list.clone();
        }
    }
    let list: Vec<(usize, qymcad_core::model::Id)> = (0..pn.project.bodies.len())
        .filter(|&mi| body_shown(pn.body_view(), mi))
        .filter_map(|mi| pn.project.mesh_id(mi).map(|b| (mi, b)))
        .collect();
    pn.cache.shown_bodies.borrow_mut().put(view_rev(pn.regen), qymcad_ui_state::ShownBodies { ctx, list: list.clone() });
    list
}

/// The nearest WORLD point on an edge of a visible body to the cursor (within the grab). It
/// complements `pick_vertex_pos` (the vertices) — the origin of a sketch snaps not only to corners but
/// to any point on an edge.
pub fn pick_edge_point(pn: &Painting, rect: Rect, pos: Pos2) -> Option<[f64; 3]> {
    use qymcad_core::feature::{apply12, is_identity12};
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis };
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let mut best: Option<(f32, [f64; 3])> = None;
    for (_mi, body) in shown_bodies(pn) {
        // a cull by the bounding box: a body whose screen rectangle does not cover the cursor is not
        // worth walking
        if !body_bbox_hit(pn, body, rect, pos, &basis, 12.0) {
            continue;
        }
        let Some(edges) = body_edges_cached(pn.cache, pn.live, pn.regen, body) else { continue };
        let (polys, ids) = (&edges.polys, &edges.ids);
        let wt = pn.project.body_display_transform(body, ctx);
        let tp = |p: &[f32; 3]| -> [f64; 3] {
            let v = [p[0] as f64, p[1] as f64, p[2] as f64];
            if is_identity12(&wt) { v } else { apply12(&wt, v) }
        };
        for (poly, id) in polys.iter().zip(ids.iter().copied()) {
            if id == 0 || poly.len() < 2 {
                continue;
            }
            for w in poly.windows(2) {
                let (a3, b3) = (tp(&w[0]), tp(&w[1]));
                let pa = scr.at(a3).0;
                let pb = scr.at(b3).0;
                let ab = pb - pa;
                let len2 = ab.length_sq();
                let t = if len2 > 1e-6 { ((pos - pa).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
                let d = (pa + ab * t).distance(pos);
                if best.is_none_or(|(bd, _)| d < bd) {
                    let td = t as f64;
                    best = Some((d, [a3[0] + (b3[0] - a3[0]) * td, a3[1] + (b3[1] - a3[1]) * td, a3[2] + (b3[2] - a3[2]) * td]));
                }
            }
        }
    }
    best.filter(|(d, _)| *d <= grab(pn.set, Grab::Point)).map(|(_, w)| w)
}

/// THE FACE OF A PART UNDER THE CURSOR — a base plane does NOT intercept it.
///
/// The assembly tools go by this resolution: collecting an anchor, editing an anchor, the secondary
/// axis, the tangency, the width. All of them want a face of a part, and a miss must mean emptiness
/// under the cursor rather than an invisible square a third of the scene across turning out to be
/// nearer.
pub fn pick_part_face_at(pn: &Painting, rect: Rect, screen: Pos2) -> Option<(qymcad_core::model::Id, qymcad_core::feature::FaceKey)> {
    face_under_cursor(pn, rect, screen).map(|(_, b, k)| (b, k))
}

/// What is under the cursor while a sketch plane is being chosen (the nearest by depth): a world or
/// datum plane, or a FACE of a part. Returns a `SketchPlane`.
/// THE FACE OF A VISIBLE PART UNDER THE CURSOR and its depth — the part shared by two resolutions.
///
/// It became a door of its own for this reason. When a sketch plane is being chosen, a face COMPETES
/// with a base plane, and there a comparison by depth is right. The assembly tools have nothing to
/// compete with: they want a part and only a part. While there was one door, collecting an anchor got
/// a base plane and declared a miss — see `a_mate_takes_the_face_you_click.rs`.
pub fn face_under_cursor(pn: &Painting, rect: Rect, screen: Pos2) -> Option<(f64, qymcad_core::model::Id, qymcad_core::feature::FaceKey)> {
    use qymcad_core::feature::FaceKey;
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis };
    let ctx = current_ctx_id(pn.active_path, pn.project);
    // ONLY WHAT IS DRAWN gets picked: the consumed bodies are skipped (except the source of an edit)
    // along with the result of the feature being edited — otherwise stale geometry is caught (the faces
    // of a hidden body hanging in empty space where it stood before the move).
    let consumed = consumed_bodies(pn.project);
    let edit_hide = edit_hidden_bodies(pn.cmd, pn.project);
    let edit_src = if !edit_hide.is_empty() { edit_src_body(pn.cmd, pn.project) } else { None };
    let mut face_best: Option<(f64, qymcad_core::model::Id, FaceKey)> = None;
    for (mi, mesh) in pn.project.bodies.iter().map(|b| &b.mesh).enumerate() {
        if !body_shown(pn.body_view(), mi) {
            continue;
        }
        // GHOSTS (the neighbouring components shown translucently in context, for reference) are NOT
        // picked ONLY in the mirror and section modes: the plane of a mirror or a section must not be
        // taken by accident from a part those tools logically cannot reach.
        // For an ORDINARY pick of a sketch plane a ghost stays pickable — working in context exists
        // precisely for that ("a sketch on the face of a neighbour gives an external reference"). The
        // filter was once applied unconditionally and broke the top-down associative sketch on a
        // neighbouring part.
        if (pn.mirror.part.is_some() || pn.section.pick) && body_is_ghost(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, mi) {
            continue;
        }
        let bid = pn.project.mesh_id(mi);
        if bid.is_some_and(|b| edit_hide.contains(&b)) || bid.is_some_and(|b| consumed.contains(&b) && Some(b) != edit_src) {
            continue; // a hidden body (consumed, or the result of an edit) — its faces are not picked
        }
        // a face is hit-tested where it is drawn; the centroid and normal in a `FaceKey` are LOCAL (for
        // `resolve_face`)
        let wt = pn.project.mesh_id(mi).map(|b| pn.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
        let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
        for ti in 0..mesh.tris.len() {
            let t = mesh.triangle(ti);
            let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
            if section_tri_hidden(pn.section, wa, wb, wc) {
                continue; // THE SECTION: what is hidden is not picked, so clicks reach the innards
            }
            // A BACK-FACING triangle, as in `rasterize_3d`: a wrapping face (a cylinder) gives half
            // its triangles facing away from the camera, and those must not be picked (they are
            // invisible, and picking them punches a hole in the picture of the body)
            if v_dot(v_norm(v_cross(v_sub(wb, wa), v_sub(wc, wa))), basis.2) >= 0.0 {
                continue;
            }
            let (pa, da) = scr.at(wa);
            let (pb, db) = scr.at(wb);
            let (pc, dc) = scr.at(wc);
            if point_in_tri(screen, pa, pb, pc) {
                let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                if face_best.as_ref().is_none_or(|(bd, _, _)| depth < *bd) {
                    if let Some(fi) = pn.project.bodies.get(mi).and_then(|b| b.faces.iter().position(|f| f.triangles.contains(&(ti as u32)))) {
                        let face = &pn.project.bodies[mi].faces[fi];
                        let body = pn.project.mesh_id(mi).unwrap_or(0);
                        let key = FaceKey { index: fi as u32, centroid: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, id: face.id };
                        face_best = Some((depth, body, key));
                    }
                }
            }
        }
    }
    face_best
}

/// The axis of the CYLINDRICAL or conical face under the cursor (for picking the axis of a circular
/// array by a click): the nearest visible face by depth for which OCCT gives an axis
/// (`Shape::face_axis`). Planar and spline faces are passed over.
pub fn pick_cyl_face_axis_at(pn: &Painting, rect: Rect, screen: Pos2) -> Option<AxisHit> {
    use qymcad_core::feature::{apply12, is_identity12};
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis };
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let consumed = consumed_bodies(pn.project);
    let edit_hide = edit_hidden_bodies(pn.cmd, pn.project);
    let edit_src = if !edit_hide.is_empty() { edit_src_body(pn.cmd, pn.project) } else { None };
    let mut best: Option<(f64, Id, u32)> = None; // (the depth, the body, the id of the face)
    for (mi, mesh) in pn.project.bodies.iter().map(|b| &b.mesh).enumerate() {
        if !body_shown(pn.body_view(), mi) {
            continue;
        }
        let bid = pn.project.mesh_id(mi);
        if bid.is_some_and(|b| edit_hide.contains(&b)) || bid.is_some_and(|b| consumed.contains(&b) && Some(b) != edit_src) {
            continue;
        }
        let wt = bid.map(|b| pn.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
        let tp = |v: [f64; 3]| if is_identity12(&wt) { v } else { apply12(&wt, v) };
        for ti in 0..mesh.tris.len() {
            let t = mesh.triangle(ti);
            let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
            if section_tri_hidden(pn.section, wa, wb, wc) {
                continue; // THE SECTION: what is hidden is not picked, so clicks reach the innards
            }
            let (pa, da) = scr.at(wa);
            let (pb, db) = scr.at(wb);
            let (pc, dc) = scr.at(wc);
            if point_in_tri(screen, pa, pb, pc) {
                let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                if best.is_none_or(|(bd, _, _)| depth < bd) {
                    if let (Some(b), Some(fi)) = (bid, pn.project.bodies.get(mi).and_then(|b| b.faces.iter().position(|f| f.triangles.contains(&(ti as u32))))) {
                        let fid = pn.project.bodies[mi].faces[fi].id;
                        // only a cylinder or a cone has an axis in OCCT; otherwise the face is passed
                        // over as a candidate
                        if fid != 0 && pn.live.shapes.get(&b).and_then(|s| s.face_axis(fid)).is_some() {
                            best = Some((depth, b, fid));
                        }
                    }
                }
            }
        }
    }
    best.map(|(_, b, fid)| AxisHit::Face(b, fid))
}

/// The edge under the cursor among ALL the visible bodies (for picking the axis of a connector) -> (the
/// body, the persistent id of the edge).
pub fn pick_edge_any(pn: &Painting, rect: Rect, pos: Pos2) -> Option<(Id, u32)> {
    let basis = pn.cam.basis();
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let mut best: Option<(f32, Id, u32)> = None;
    for (_mi, body) in shown_bodies(pn) {
        // a cull by the bounding box: a body whose screen rectangle does not cover the cursor is not
        // worth walking
        if !body_bbox_hit(pn, body, rect, pos, &basis, 12.0) {
            continue;
        }
        let Some(edges) = body_edges_cached(pn.cache, pn.live, pn.regen, body) else { continue };
        let (polys, ids) = (&edges.polys, &edges.ids);
        let wt = pn.project.body_display_transform(body, ctx);
        let tp = |p: &[f32; 3]| -> [f64; 3] {
            let v = [p[0] as f64, p[1] as f64, p[2] as f64];
            if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) }
        };
        for (poly, id) in polys.iter().zip(ids.iter().copied()) {
            if id == 0 {
                continue;
            }
            let sp: Vec<Pos2> = poly.iter().map(|p| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis }.at(tp(p)).0).collect();
            for k in 0..sp.len().saturating_sub(1) {
                let d = screen_dist_seg(pos, sp[k], sp[k + 1]);
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, body, id));
                }
            }
        }
    }
    best.filter(|(d, _, _)| *d <= grab(pn.set, Grab::Curve)).map(|(_, b, id)| (b, id))
}

/// A RAY INTO A FACE WITH NO SIDE EFFECTS: (the body, the persistent id of the face, the point of the
/// hit in the world).
///
/// Besides searching, `pick_face_3d` also CHANGES the selection, the set of faces of a command and the
/// highlight — the measuring tool needs none of that and is harmed by it: measure a gap and lose the
/// selection of the part. The search is the same, only clean.
pub fn pick_face_ray(pn: &Painting, rect: Rect, screen: Pos2) -> Option<(qymcad_core::model::Id, u32, [f64; 3])> {
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis };
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let consumed = consumed_bodies(pn.project);
    let mut best: Option<(f64, usize, usize, [f64; 3])> = None;
    for (mi, mesh) in pn.project.bodies.iter().map(|b| &b.mesh).enumerate() {
        if !body_shown(pn.body_view(), mi) {
            continue;
        }
        let b = pn.project.mesh_id(mi);
        if b.is_some_and(|b| consumed.contains(&b)) {
            continue;
        }
        let wt = b.map(|b| pn.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
        let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
        for ti in 0..mesh.tris.len() {
            let t = mesh.triangle(ti);
            let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
            if section_tri_hidden(pn.section, wa, wb, wc) {
                continue;
            }
            if v_dot(v_norm(v_cross(v_sub(wb, wa), v_sub(wc, wa))), basis.2) >= 0.0 {
                continue; // a back-facing face is not the one under the cursor
            }
            let (pa, da) = scr.at(wa);
            let (pb, db) = scr.at(wb);
            let (pc, dc) = scr.at(wc);
            if point_in_tri(screen, pa, pb, pc) {
                let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                if best.is_none_or(|(bd, _, _, _)| depth < bd) {
                    let hit = [(wa[0] + wb[0] + wc[0]) / 3.0, (wa[1] + wb[1] + wc[1]) / 3.0, (wa[2] + wb[2] + wc[2]) / 3.0];
                    best = Some((depth, mi, ti, hit));
                }
            }
        }
    }
    let (_, mi, ti, hit) = best?;
    let body = pn.project.mesh_id(mi)?;
    let fid = pn.project.bodies.get(mi)?.faces.iter().find(|f| f.triangles.contains(&(ti as u32)))?.id;
    Some((body, fid, hit))
}

/// The PERSISTENT id of the face under the cursor among the bodies THAT ARE DRAWN (the same logic of
/// visibility as `pick_face_3d`). For choosing the reference face of a chamfer by hand. `None` means a
/// miss, or that the triangle has no face.
pub fn pick_face_persist_id(pn: &Painting, rect: Rect, screen: Pos2) -> Option<u32> {
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis };
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let consumed = consumed_bodies(pn.project);
    let edit_hide = edit_hidden_bodies(pn.cmd, pn.project);
    let edit_src = if !edit_hide.is_empty() { edit_src_body(pn.cmd, pn.project) } else { None };
    let mut best: Option<(f64, usize, usize)> = None;
    for (mi, mesh) in pn.project.bodies.iter().map(|b| &b.mesh).enumerate() {
        if !body_shown(pn.body_view(), mi) {
            continue;
        }
        let b = pn.project.mesh_id(mi);
        if b.is_some_and(|b| edit_hide.contains(&b)) || b.is_some_and(|b| consumed.contains(&b) && Some(b) != edit_src) {
            continue;
        }
        let wt = pn.project.mesh_id(mi).map(|b| pn.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
        let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
        for ti in 0..mesh.tris.len() {
            let t = mesh.triangle(ti);
            let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
            if section_tri_hidden(pn.section, wa, wb, wc) {
                continue; // THE SECTION: what is hidden is not picked, so clicks reach the innards
            }
            // A BACK-FACING triangle, as in `rasterize_3d`
            if v_dot(v_norm(v_cross(v_sub(wb, wa), v_sub(wc, wa))), basis.2) >= 0.0 {
                continue;
            }
            let (pa, da) = scr.at(wa);
            let (pb, db) = scr.at(wb);
            let (pc, dc) = scr.at(wc);
            if point_in_tri(screen, pa, pb, pc) {
                let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                if best.is_none_or(|(bd, _, _)| depth < bd) {
                    best = Some((depth, mi, ti));
                }
            }
        }
    }
    let (_, mi, ti) = best?;
    pn.project.bodies.get(mi).and_then(|b| b.faces.iter().find(|f| f.triangles.contains(&(ti as u32)))).map(|f| f.id)
}

/// The mesh index of the VISIBLE body under the cursor (the topmost by depth) — for picking an operand
/// of the boolean of bodies. Only what is drawn is picked (not the consumed or hidden ones), as in
/// `pick_face_3d`.
pub fn pick_body_at(pn: &Painting, rect: Rect, screen: Pos2) -> Option<usize> {
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis };
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let consumed = consumed_bodies(pn.project);
    let mut best: Option<(f64, usize)> = None;
    for (mi, mesh) in pn.project.bodies.iter().map(|b| &b.mesh).enumerate() {
        if !body_shown(pn.body_view(), mi) {
            continue;
        }
        let b = pn.project.mesh_id(mi);
        if b.is_some_and(|b| consumed.contains(&b)) {
            continue;
        }
        let wt = b.map(|b| pn.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
        let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
        for ti in 0..mesh.tris.len() {
            let t = mesh.triangle(ti);
            let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
            if section_tri_hidden(pn.section, wa, wb, wc) {
                continue; // THE SECTION: what is hidden is not picked, so clicks reach the innards
            }
            // A BACK-FACING triangle, as in `rasterize_3d`
            if v_dot(v_norm(v_cross(v_sub(wb, wa), v_sub(wc, wa))), basis.2) >= 0.0 {
                continue;
            }
            let (pa, da) = scr.at(wa);
            let (pb, db) = scr.at(wb);
            let (pc, dc) = scr.at(wc);
            if point_in_tri(screen, pa, pb, pc) {
                let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                if best.is_none_or(|(bd, _)| depth < bd) {
                    best = Some((depth, mi));
                }
            }
        }
    }
    best.map(|(_, mi)| mi)
}

/// The full LOCAL-TO-WORLD 3x4 frame for placing a primitive under the cursor: first an exact SNAP (the
/// vertex of an edge or a datum point) — a translation only, with no rotation (the axes of the world);
/// otherwise the intersection of the cursor ray with the CHOSEN plane (a face of a body, a base
/// XY/XZ/YZ plane or a datum): the origin is the point ON the plane under the cursor, and the local +Z
/// is oriented along the normal (the base of the primitive sits ON the surface). `None` means a miss.
pub fn pick_place_frame_at(pn: &Painting, rect: Rect, screen: Pos2) -> Option<[f64; 12]> {
    use qymcad_core::feature::{apply12, apply12_dir, SketchPlane};
    let translate = |w: [f64; 3]| [1.0, 0.0, 0.0, w[0], 0.0, 1.0, 0.0, w[1], 0.0, 0.0, 1.0, w[2]];
    if let Some(w) = pick_vertex_pos(pn, rect, screen) {
        return Some(translate(w)); // a snap to the vertex of an edge has the highest priority
    }
    if let Some((_, w)) = pick_datum_point_at(pn, rect, screen) {
        return Some(translate(w)); // a snap to a datum point
    }
    // the plane under the cursor -> its world point and normal -> the intersection with the cursor ray
    let (p0, n) = match pick_sketch_plane_at(pn, rect, screen)? {
        SketchPlane::World(bp) => {
            let f = bp.frame();
            (f.origin, f.normal())
        }
        SketchPlane::Datum(id) => {
            let p = pn.project.planes.iter().find(|p| p.id == id)?;
            (p.origin, p.normal)
        }
        SketchPlane::Face(body, key) => {
            let wt = pn.project.body_display_transform(body, current_ctx_id(pn.active_path, pn.project));
            (apply12(&wt, key.centroid), apply12_dir(&wt, key.normal))
        }
    };
    let (o, d) = screen_ray(&pn.cam, rect, screen);
    let origin = ray_plane(o, d, p0, n)?;
    // the orthonormal basis of the plane: the columns of the transform are the images of the local
    // X (u), Y (v) and Z (n, the normal)
    let nn = v_norm(n);
    let a = if nn[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let u = v_norm(v_cross(a, nn));
    let vv = v_cross(nn, u);
    Some([u[0], vv[0], nn[0], origin[0], u[1], vv[1], nn[1], origin[1], u[2], vv[2], nn[2], origin[2]])
}

/// The vertex (an end of an edge) under the cursor among ALL the visible bodies -> (the body, the id
/// of the edge, which end).
pub fn pick_vertex_any(pn: &Painting, rect: Rect, pos: Pos2) -> Option<(Id, u32, bool)> {
    let basis = pn.cam.basis();
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let mut best: Option<(f32, Id, u32, bool)> = None;
    for (_mi, body) in shown_bodies(pn) {
        // a cull by the bounding box: a body whose screen rectangle does not cover the cursor is not
        // worth walking
        if !body_bbox_hit(pn, body, rect, pos, &basis, 12.0) {
            continue;
        }
        let Some(edges) = body_edges_cached(pn.cache, pn.live, pn.regen, body) else { continue };
        let (polys, ids) = (&edges.polys, &edges.ids);
        let wt = pn.project.body_display_transform(body, ctx);
        let tp = |p: &[f32; 3]| -> [f64; 3] {
            let v = [p[0] as f64, p[1] as f64, p[2] as f64];
            if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) }
        };
        for (poly, id) in polys.iter().zip(ids.iter().copied()) {
            if id == 0 || poly.len() < 2 {
                continue;
            }
            for (end, vert) in [(false, &poly[0]), (true, &poly[poly.len() - 1])] {
                let d = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis }.at(tp(vert)).0.distance(pos);
                if best.is_none_or(|(bd, _, _, _)| d < bd) {
                    best = Some((d, body, id, end));
                }
            }
        }
    }
    best.filter(|(d, _, _, _)| *d <= grab(pn.set, Grab::Point)).map(|(_, b, id, e)| (b, id, e))
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

/// Shading a triangle, plus the branches for a highlighted body and for a ghosted one.
/// One source of colour for the CPU raster and for the GPU pass (the normal and the light are in world
/// space, so they do not depend on the camera).
///
/// THE DEPTH OF THE SHADOW COMES FROM THE SCHEME. `0.4 + 0.6 * |n . light|` used to stand here, and the
/// function had no `&self` — it simply could not look at the scheme. Against a dark background a shadow
/// down to 40% brightness reads very well; against a light one that same shadow turns the part into a
/// dark blot.
/// `ghost_alpha` is an ARGUMENT for the same reason as the scheme: a function without `&self` can ask
/// about neither. Hiding the setting in a constant here would mean a second source of truth about a
/// ghost's opacity — exactly the case that once kept the scheme from reaching the shading at all.
pub fn shade_tri(pal: &qymcad_scheme::Palette, ghost_alpha: u8, hot: bool, ghost: bool, base: [u8; 3], n: [f64; 3], light: [f64; 3]) -> Color32 {
    let diff = v_dot(n, light).abs();
    let lit = qymcad_scheme::lit(pal.shade_floor_body, diff as f32);
    // shading can only darken: without this the brightest a part gets is its own colour, which on a
    // light canvas is darker than the background. So the BRIGHTNESS is raised (lightness plus
    // saturation) rather than white being mixed in: white washes the colour out and gives whitewash
    // instead of a light-coloured part.
    let base = qymcad_scheme::brighten(base, pal.body_lighten, pal.body_saturate);
    if hot {
        // a selected BODY or COMPONENT is LIGHTER than its own colour: lifted towards white plus a
        // slight cool tint, so that it reads as selected and highlighted. That shows the part or
        // subassembly was selected entire; NOT bright orange, which was confused with a face selection.
        let b = |c: f32, cool: f32| {
            let v = c * lit;
            (v + (255.0 - v) * 0.4 + cool).min(255.0) as u8
        };
        Color32::from_rgb(b(base[0] as f32, 0.0), b(base[1] as f32, 8.0), b(base[2] as f32, 22.0))
    } else if ghost {
        // a context ghost (a neighbouring part): a dim blue-grey and TRANSLUCENT, so that one's own body
        // or sketch shows through it. Real transparency (a two-pass render), not dimming.
        // A quarter of its own colour and three quarters of the colour the scheme leads a ghost towards:
        // on a dark background that is darkness, as it was; on a light one it is the canvas itself.
        let t = pal.ghost_target;
        let m = |c: u8, k: usize| (c as f32 * lit * 0.25 + t[k] as f32 * 0.75) as u8;
        Color32::from_rgba_unmultiplied(m(base[0], 0), m(base[1], 1), m(base[2], 2), ghost_alpha)
    } else {
        Color32::from_rgb((base[0] as f32 * lit) as u8, (base[1] as f32 * lit) as u8, (base[2] as f32 * lit) as u8)
    }
}

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
pub fn snap_infer_glyph(pn: &Painting) -> Option<(Gly, Point2)> {
    let (p, kind) = pn.snap_hint?;
    let g = match kind {
        0 | 3 | 4 | 5 => Gly::Coincident, // a vertex, a midpoint, a centre or an intersection
        6 => Gly::PointOnLine,            // on an edge
        _ => return None,                 // the grid or an axis: not a constraint
    };
    Some((g, p))
}

/// The contours of sketches that do NOT belong to the active context (its subtree) — they are hidden
/// both in the 2D sketcher and in the 3D overlay (one source for both drawing loops).
pub fn foreign_contour_ids(pn: &Painting) -> std::collections::HashSet<Id> {
    // ASKED FIRST, FILTERED AFTER. `sketch_in_ctx` takes the context now, and a closure that calls it
    // while the document is being walked borrows the same record twice.
    // ONE COPY OF THE RULE. Inlining "does this sketch belong to the context" here left the same rule
    // written twice; the free function takes exactly what it reads instead, so drawing needs no `&mut`.
    pn.project
        .sketches
        .iter()
        .filter(|s| !sketch_in_ctx(pn.active_path, pn.project, pn.sketch_ses, s.id))
        .flat_map(|s| s.contour_ids.iter().copied())
        .collect()
}

pub fn sketch_ref_edges_2d(cache: &Caches, cmd: &FeatCommand, live: &LiveGeom, project: &qymcad_core::model::Project, regen: &Rebuilding, si: usize) -> Vec<Vec<Point2>> {
    use qymcad_core::feature::SketchPlane;
    let s = match project.sketches.get(si) { Some(s) => s, None => return Vec::new() };
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
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect: rect };
    use qymcad_core::model::Constraint;
    let Some(s) = pick.project.sketches.get(si) else { return Vec::new() };
    let pt = |id: Id| s.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y));
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
    for (ci, c) in s.constraints.iter().enumerate() {
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
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis };
    let ctx = qymcad_ui_state::current_ctx_id(pn.active_path, pn.project);
    // THE CENTRE OF THE FACE UNDER THE CURSOR is as much a snap point as a vertex and takes part
    // on equal terms.
    if let Some((body, key)) = face {
        let wt = pn.project.body_display_transform(body, ctx);
        let w = qymcad_core::feature::apply12(&wt, key.centroid);
        offer(scr.at(w).0.distance(pos), body, AnchorRef::FaceCenter(body, key));
    }
    for (_mi, body) in shown_bodies(pn) {
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
            if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) }
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
                    if qymcad_core::feature::is_identity12(&wt) { p } else { qymcad_core::feature::apply12(&wt, p) }
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
