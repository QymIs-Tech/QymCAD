//! WHAT IS UNDER THE POINTER ON A BODY: its box, edges, contours, vertices, faces, axes, places.

use super::*;

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
                    [bb.min.x, bb.min.y, bb.min.z],
                    [bb.max.x, bb.min.y, bb.min.z],
                    [bb.min.x, bb.max.y, bb.min.z],
                    [bb.max.x, bb.max.y, bb.min.z],
                    [bb.min.x, bb.min.y, bb.max.z],
                    [bb.max.x, bb.min.y, bb.max.z],
                    [bb.min.x, bb.max.y, bb.max.z],
                    [bb.max.x, bb.max.y, bb.max.z],
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
        let p = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis }.at(w).0;
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
    let (polys, ids, _, smooth) = shape.edges_full_smooth();
    let v = std::rc::Rc::new(qymcad_ui_state::EdgePolys { polys, ids, smooth });
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
    let empty = !matches!(best, Some((_, d)) if d * view.scale as f64 <= grab(set, Grab::Curve) as f64);
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
        let on_picked = picked.iter().flatten().any(|p| (p[0] - c.centroid[0]).abs() < 1e-6 && (p[1] - c.centroid[1]).abs() < 1e-6 && (p[2] - c.centroid[2]).abs() < 1e-6);
        if !on_picked {
            continue;
        }
        let d = scr.at(c.centroid).0.distance(screen);
        if d <= grab && best.as_ref().is_none_or(|(bd, _, _)| d < *bd) {
            best = Some((d, c.desc, c.centroid));
        }
    }
    // THE CORNER IS TAKEN ONLY CLOSE UP when an edge is under the cursor too: past half the point grab the click is
    // on the edge. Reported behaviour: the middle of a 5 mm edge, 8.3 px from the corner of an edge picked before,
    // set a radius at that corner (point grab 10 px beat edge grab 8 px) and the edge was never added.
    if let Some((d, _, _)) = best {
        if d > grab / 2.0 {
            let on_edge =
                project.regen_edges.get(&body).is_some_and(|es| es.iter().any(|e| screen_dist_seg(screen, scr.at(e.a).0, scr.at(e.b).0) <= qymcad_ui_state::grab::grab(scr.set, Grab::Curve)));
            if on_edge {
                return None;
            }
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
        if qymcad_core::feature::is_identity12(&wt) {
            v
        } else {
            qymcad_core::feature::apply12(&wt, v)
        }
    };
    // ON A COLLINEAR OVERLAP (a long seam edge containing a short one) both project into the same
    // screen line. The first one met used to win (usually the long one), so the short one could not be
    // selected by a click. Now, at an ALMOST equal distance (within 2 px), the SHORT edge is preferred
    // as the more specific one: over the overlapping stretch the short one is chosen, and the long one
    // is available where it is ALONE. Both are reachable.
    let mut best: Option<(f32, f32, usize)> = None; // (the least distance, the screen length, the index)
    for (i, poly) in edges.polys.iter().enumerate() {
        let pts: Vec<Pos2> = poly.iter().map(|p| qymcad_ui_state::Screen { cam: &cam, set, rect, basis: &basis }.at(tp(p)).0).collect();
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
    // A BODY UNDER THE POINTER IS TAKEN BEFORE ANY PLANE behind it or in front of it: a base plane (XY, XZ, YZ) or a
    // datum is picked, and lit, only where no body is under the pointer. Reported behaviour: the yellow square of a base
    // plane at the origin stood over the faces and edges a sketch or a datum was to be put on, in a part and in an
    // assembly alike. The depth of a plane was the depth of the middle of its square, not of the point under the
    // pointer, so a slanted plane came out nearer than the face it hid.
    if let Some((_, b, k)) = face_under_cursor(pn, rect, screen) {
        return Some(SketchPlane::Face(b, k));
    }
    let h = plane_pick_half_size(pn);
    let mut plane_best: Option<(f64, SketchPlane)> = None;
    for (sp, fr) in sketch_plane_candidates(pn) {
        let corners = [fr.lift(Point2::new(-h, -h)), fr.lift(Point2::new(h, -h)), fr.lift(Point2::new(h, h)), fr.lift(Point2::new(-h, h))];
        let pr: Vec<(Pos2, f64)> = corners.iter().map(|p| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at([p.x, p.y, p.z])).collect();
        if point_in_tri(screen, pr[0].0, pr[1].0, pr[2].0) || point_in_tri(screen, pr[0].0, pr[2].0, pr[3].0) {
            let depth = pr.iter().map(|(_, d)| *d).sum::<f64>() / 4.0;
            if plane_best.is_none_or(|(bd, _)| depth < bd) {
                plane_best = Some((depth, sp));
            }
        }
    }
    plane_best.map(|(_, sp)| sp)
}

/// Picking the axis of a circular array or of a datum axis by a click: the nearest DATUM AXIS (a line)
/// OR a STRAIGHT edge of ANY visible body (`axis_edges`) within the threshold, otherwise the axis of the
/// cylindrical face under the cursor.
pub fn pick_axis_at(pn: &Painting, rect: Rect, screen: Pos2) -> Option<AxisHit> {
    use qymcad_core::feature::apply12;
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
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
        let dist = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(w).0.distance(pos);
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
    for (_mi, body) in qymcad_ui_state::shown_bodies(pn) {
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
            if is_identity12(&wt) {
                v
            } else {
                apply12(&wt, v)
            }
        };
        for (k, (poly, id)) in polys.iter().zip(ids.iter().copied()).enumerate() {
            if id == 0 || poly.len() < 2 {
                continue;
            }
            // A CORNER IS WHERE EDGES MEET, not where the B-rep starts a closed curve or runs a seam: the rim of a hole
            // has one vertex, on its seam, and a click on the rim took it - "vertex" instead of a circle of 10
            let (first, last) = (poly[0], poly[poly.len() - 1]);
            let closed = (0..3).all(|i| (first[i] - last[i]).abs() < 1e-5);
            if closed || edges.smooth.get(k).copied().unwrap_or(false) {
                continue;
            }
            for vert in [&poly[0], &poly[poly.len() - 1]] {
                let w = tp(vert);
                let d = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(w).0.distance(pos);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, w));
                }
            }
        }
    }
    best.filter(|(d, _)| *d <= grab(pn.set, Grab::Point)).map(|(_, w)| w)
}

/// The nearest WORLD point on an edge of a visible body to the cursor (within the grab). It
/// complements `pick_vertex_pos` (the vertices) — the origin of a sketch snaps not only to corners but
/// to any point on an edge.
pub fn pick_edge_point(pn: &Painting, rect: Rect, pos: Pos2) -> Option<[f64; 3]> {
    use qymcad_core::feature::{apply12, is_identity12};
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let mut best: Option<(f32, [f64; 3])> = None;
    for (_mi, body) in qymcad_ui_state::shown_bodies(pn) {
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
            if is_identity12(&wt) {
                v
            } else {
                apply12(&wt, v)
            }
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
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
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
        if (pn.mirror.in_hand() || pn.section.pick) && body_is_ghost(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, mi) {
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
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
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

/// A CORNER OR AN EDGE UNDER THE CURSOR, as the selection takes it with nothing in hand: a corner within the point
/// grab first, then an edge within the curve grab - each only where no face stands in front of it. A corner is where
/// edges meet: the seam of a closed rim and a smooth junction are not corners.
pub fn edge_or_corner_under(pn: &Painting, rect: Rect, pos: Pos2) -> Option<Sel> {
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let mut corner: Option<(f32, Sel, [f64; 3])> = None;
    let mut edge: Option<(f32, Sel, [f64; 3])> = None;
    for (_mi, body) in qymcad_ui_state::shown_bodies(pn) {
        if !body_bbox_hit(pn, body, rect, pos, &basis, 12.0) {
            continue;
        }
        let Some(edges) = body_edges_cached(pn.cache, pn.live, pn.regen, body) else { continue };
        let wt = pn.project.body_display_transform(body, ctx);
        let tp = |p: &[f32; 3]| -> [f64; 3] {
            let v = [p[0] as f64, p[1] as f64, p[2] as f64];
            if qymcad_core::feature::is_identity12(&wt) {
                v
            } else {
                qymcad_core::feature::apply12(&wt, v)
            }
        };
        for (k, (poly, id)) in edges.polys.iter().zip(edges.ids.iter().copied()).enumerate() {
            if id == 0 || poly.len() < 2 {
                continue;
            }
            let (first, last) = (poly[0], poly[poly.len() - 1]);
            let closed = (0..3).all(|i| (first[i] - last[i]).abs() < 1e-5);
            if !closed && !edges.smooth.get(k).copied().unwrap_or(false) {
                for (end, vert) in [(false, &first), (true, &last)] {
                    let w = tp(vert);
                    let d = scr.at(w).0.distance(pos);
                    if corner.as_ref().is_none_or(|(bd, _, _)| d < *bd) {
                        corner = Some((d, Sel::Vertex(body, id, end), w));
                    }
                }
            }
            for seg in poly.windows(2) {
                let (a3, b3) = (tp(&seg[0]), tp(&seg[1]));
                let (pa, pb) = (scr.at(a3).0, scr.at(b3).0);
                let ab = pb - pa;
                let t = if ab.length_sq() > 1e-6 { ((pos - pa).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
                let d = (pa + ab * t).distance(pos);
                if edge.as_ref().is_none_or(|(bd, _, _)| d < *bd) {
                    let td = t as f64;
                    edge = Some((d, Sel::Edge(body, id), [a3[0] + (b3[0] - a3[0]) * td, a3[1] + (b3[1] - a3[1]) * td, a3[2] + (b3[2] - a3[2]) * td]));
                }
            }
        }
    }
    let seen = |hit: Option<(f32, Sel, [f64; 3])>, within: f32| hit.filter(|(d, _, w)| *d <= within && point_not_hidden(pn, rect, *w)).map(|(_, sel, _)| sel);
    seen(corner, grab(pn.set, Grab::Point)).or_else(|| seen(edge, grab(pn.set, Grab::Curve)))
}

/// The edge under the cursor among ALL the visible bodies (for picking the axis of a connector) -> (the
/// body, the persistent id of the edge).
pub fn pick_edge_any(pn: &Painting, rect: Rect, pos: Pos2) -> Option<(Id, u32)> {
    let basis = pn.cam.basis();
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let mut best: Option<(f32, Id, u32)> = None;
    for (_mi, body) in qymcad_ui_state::shown_bodies(pn) {
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
            if qymcad_core::feature::is_identity12(&wt) {
                v
            } else {
                qymcad_core::feature::apply12(&wt, v)
            }
        };
        for (poly, id) in polys.iter().zip(ids.iter().copied()) {
            if id == 0 {
                continue;
            }
            let sp: Vec<Pos2> = poly.iter().map(|p| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(tp(p)).0).collect();
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

/// IS POINT `w` IN SIGHT: no face stands in front of it where it is drawn, within 0.5 mm for the silhouette. A vertex
/// or an edge beats a face only when it is not hidden behind one.
///
/// THE FACE IS ASKED AT THE POINT ITSELF, and its depth is the one at that pixel. The measure asked at the pointer, a
/// few pixels off the corner, and took the depth of a point standing for the face's triangle (13.3, 0, 3.3) - 4.7 mm
/// nearer than the corner (0, 0, 10) of a block in plain sight, which was taken as its face.
pub fn point_not_hidden(pn: &Painting, rect: Rect, w: [f64; 3]) -> bool {
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let (at, d) = scr.at(w);
    // the depth of the face drawn at that very pixel, not of a point standing for its triangle
    face_under_cursor(pn, rect, at).is_none_or(|(fd, _, _)| d <= fd + 0.5)
}

/// A RAY INTO A FACE WITH NO SIDE EFFECTS: (the body, the persistent id of the face, the point of the hit in the
/// world). Picking a face for a command also changes the selection and the highlight; measuring and the right button
/// need none of that and are harmed by it - measure a gap and lose the selection of the part.
pub fn pick_face_ray(pn: &Painting, rect: Rect, screen: Pos2) -> Option<(qymcad_core::model::Id, u32, [f64; 3])> {
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let ctx = current_ctx_id(pn.active_path, pn.project);
    let consumed = consumed_bodies(pn.project);
    let mut best: Option<(f64, usize, usize, [f64; 3])> = None;
    // A TRIANGLE NOT WHOLLY IN FRONT OF THE EYE IS NOT UNDER THE POINTER: in perspective a point behind the eye has no
    // place on the screen, and projected all the same a part standing behind the eye was hit with the nearest depth
    // there is. Its piece in front of the eye is left unpicked; what the eye is that close to, it is not aiming at.
    let inv_d = persp_inv_d_eye(&pn.cam, pn.set, rect.height() * 0.5);
    let ahead = |p: [f64; 3]| inv_d <= 0.0 || v_dot(v_sub(p, pn.cam.target), basis.2) * inv_d > -0.999;
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
            if section_tri_hidden(pn.section, wa, wb, wc) || !(ahead(wa) && ahead(wb) && ahead(wc)) {
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
                    // the perspective factor of each corner, as `Screen::at` draws it
                    let inv = persp_inv_d_eye(&pn.cam, pn.set, rect.height() * 0.5);
                    let f = |d: f64| 1.0 / (1.0 + d * inv).max(0.05);
                    let corner = |screen, world, depth| TriCorner { screen, world, persp: f(depth) };
                    let hit = tri_world_at(screen, [corner(pa, wa, da), corner(pb, wb, db), corner(pc, wc, dc)]);
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
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
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
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
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
    for (_mi, body) in qymcad_ui_state::shown_bodies(pn) {
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
            if qymcad_core::feature::is_identity12(&wt) {
                v
            } else {
                qymcad_core::feature::apply12(&wt, v)
            }
        };
        for (poly, id) in polys.iter().zip(ids.iter().copied()) {
            if id == 0 || poly.len() < 2 {
                continue;
            }
            for (end, vert) in [(false, &poly[0]), (true, &poly[poly.len() - 1])] {
                let d = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(tp(vert)).0.distance(pos);
                if best.is_none_or(|(bd, _, _, _)| d < bd) {
                    best = Some((d, body, id, end));
                }
            }
        }
    }
    best.filter(|(d, _, _, _)| *d <= grab(pn.set, Grab::Point)).map(|(_, b, id, e)| (b, id, e))
}

/// Shading a triangle, plus the branches for a highlighted body and for a ghosted one.
/// One source of colour for the CPU raster and for the GPU pass. `n` is the normal turned to the eye, and the
/// lights are `scene_lights` of the camera.
///
/// THE DEPTH OF THE SHADOW COMES FROM THE SCHEME. `0.4 + 0.6 * |n . light|` used to stand here, and the
/// function had no `&self` — it simply could not look at the scheme. Against a dark background a shadow
/// down to 40% brightness reads very well; against a light one that same shadow turns the part into a
/// dark blot.
/// `ghost_alpha` is an ARGUMENT for the same reason as the scheme: a function without `&self` can ask
/// about neither. Hiding the setting in a constant here would mean a second source of truth about a
/// ghost's opacity — exactly the case that once kept the scheme from reaching the shading at all.
pub fn shade_tri(pal: &qymcad_scheme::Palette, ghost_alpha: u8, hot: bool, ghost: bool, base: [u8; 3], n: [f64; 3], lights: &qymcad_ui_state::SceneLights) -> Color32 {
    let lit = qymcad_scheme::lit(pal.shade_floor_body, lights.on(n) as f32);
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
