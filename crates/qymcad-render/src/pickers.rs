//! WHAT IS PICKED IN THE VIEW, LIT: planes, axes, joints, the projection of edges.

use super::*;

/// THE DRIVEN GEOMETRY OF PROJECTIONS — in a colour of its own over the ordinary kind.
///
/// A projection enters a profile on a par with one's own geometry (it is what gets extruded), so it
/// is drawn as an ordinary outline. But a person must SEE that it is a view of the part rather than
/// something they drew: otherwise it is not clear why the line does not drag with the mouse. A projected curve made
/// construction is drawn pink, dashed and thin. A projection whose source is gone is ordinary geometry and is not drawn
/// here at all.
pub fn draw_projection_overlay(project: &Project, scheme: &SchemeUi, sel: Sel, view: View2d, painter: &egui::Painter, rect: Rect) {
    let sh = qymcad_ui_state::Sheet { view, rect };
    use qymcad_core::model::EntityKind;
    let Sel::Sketch(si) = sel else { return };
    let Some(s) = project.sketches.get(si) else { return };
    if s.projections.is_empty() {
        return;
    }
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt = |id: Id| points_by_id.get(&id).copied().map(|q| Point2::new(q.x, q.y));
    for proj in &s.projections {
        let stroke = Stroke::new(1.8, scheme.pal.sketch_driven());
        for eid in &proj.entities {
            let Some(e) = s.entities.iter().find(|e| e.id == *eid) else { continue };
            // PROJECTED AND CONSTRUCTION: pink, dashed and thin - told from a projection that makes a contour, and from
            // construction geometry drawn by hand
            let construction = e.construction;
            let stroke = if construction { Stroke::new(1.0, scheme.pal.sketch_driven_construction()) } else { stroke };
            let path: Vec<Pos2> = match e.kind {
                EntityKind::Line { a, b } => pt(a).zip(pt(b)).map(|(pa, pb)| vec![sh.at(pa), sh.at(pb)]).unwrap_or_default(),
                EntityKind::Circle { center, r } => pt(center)
                    .map(|c| qymcad_core::geom::tessellate_arc(c.x, c.y, r, 0.0, std::f64::consts::TAU, true, 0.02).iter().map(|q| sh.at(Point2::new(q.x, q.y))).collect())
                    .unwrap_or_default(),
                EntityKind::Arc { center, a, b, ccw } => match (pt(center), pt(a), pt(b)) {
                    (Some(c), Some(pa), Some(pb)) => {
                        let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                        let (a0, a1) = ((pa.y - c.y).atan2(pa.x - c.x), (pb.y - c.y).atan2(pb.x - c.x));
                        qymcad_core::geom::tessellate_arc(c.x, c.y, r, a0, a1, ccw, 0.02).iter().map(|q| sh.at(Point2::new(q.x, q.y))).collect()
                    }
                    _ => Vec::new(),
                },
                EntityKind::Ellipse { .. } => Vec::new(),
            };
            if path.len() < 2 {
                continue;
            }
            if construction {
                painter.extend(egui::Shape::dashed_line(&path, stroke, 6.0, 4.0));
            } else {
                painter.add(egui::Shape::line(path, stroke));
            }
        }
        // THE DRIVEN NODES as small circles: they can be snapped to, but not dragged
        for pid in &proj.points {
            if let Some(q) = pt(*pid) {
                painter.circle_stroke(sh.at(q), 3.0, Stroke::new(1.2, scheme.pal.sketch_driven()));
            }
        }
    }
}

/// THE ONLY renderer of the coordinate axes (it used to be duplicated in draw_sketch_grid).
/// Outside a sketch it is a thin grey cross. Inside a sketch it is the X (red) and Y (green) axes at full
/// length, the selected axis in orange, plus the origin marker.
pub fn draw_axes(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let sh = qymcad_ui_state::Sheet { view: pn.view, rect };
    let o = sh.at(Point2::new(0.0, 0.0));
    let editing = matches!(pn.sel, Sel::Sketch(si) if qymcad_ui_state::edit_si(pn.project, &pn.sketch_ses) == Some(si));
    if !editing {
        let ax = Stroke::new(1.0, pn.scheme.pal.sketch_axis_idle());
        painter.line_segment([Pos2::new(rect.left(), o.y), Pos2::new(rect.right(), o.y)], ax);
        painter.line_segment([Pos2::new(o.x, rect.top()), Pos2::new(o.x, rect.bottom())], ax);
        return;
    }
    // the axes as reference geometry: X red, Y green; the selected one orange and thicker
    let xsel = pn.sel_sk.items.contains(&(3, 0));
    let ysel = pn.sel_sk.items.contains(&(3, 1));
    let xst = if xsel { Stroke::new(2.2, pn.scheme.pal.selected()) } else { Stroke::new(1.2, pn.scheme.pal.sketch_axis_x()) };
    let yst = if ysel { Stroke::new(2.2, pn.scheme.pal.selected()) } else { Stroke::new(1.2, pn.scheme.pal.sketch_axis_y()) };
    painter.line_segment([Pos2::new(rect.left(), o.y), Pos2::new(rect.right(), o.y)], xst);
    painter.line_segment([Pos2::new(o.x, rect.top()), Pos2::new(o.x, rect.bottom())], yst);
    // the origin marker - it lights up on hover
    let hot = pn.cursor.is_some_and(|c| sh.at(c).distance(o) <= 11.0);
    let oc = pn.scheme.pal.active();
    painter.circle_stroke(o, if hot { 5.0 } else { 3.5 }, Stroke::new(1.6, oc));
    if hot {
        painter.circle_filled(o, 2.0, oc);
    }
}

/// Draw ONE plane or face of the click-pick in the given colour: a world or datum plane as a square frame
/// (+/-60 in its frame); a part's face as a fill of its triangles (as one mesh, with no "needles" from the
/// fan triangulation).
pub fn draw_pick_plane(pn: &Painting, painter: &egui::Painter, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), sp: &qymcad_core::feature::SketchPlane, col: Color32) {
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis };
    use qymcad_core::feature::SketchPlane;
    if let SketchPlane::Face(body, key) = sp {
        if let Some(mi) = pn.project.mesh_index(*body) {
            if let Some(face) = pn.project.bodies.get(mi).and_then(|b| b.faces.get(key.index as usize)) {
                let mesh = &pn.project.bodies[mi].mesh;
                let wt = pn.project.body_display_transform(*body, qymcad_ui_state::current_ctx_id(pn.active_path, pn.project));
                let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
                let fill = qymcad_scheme::a(col, 90);
                let mut hm = egui::Mesh::default();
                for &tri in &face.triangles {
                    let t = mesh.triangle(tri as usize);
                    let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
                    // BACK-FACING triangles: a face can WRAP AROUND (on a cylinder the whole lateral
                    // surface is one B-rep face), so half of its triangles look AWAY from the camera.
                    // Without culling, the fill is drawn there as well (the painter has no Z test against
                    // the scene) and "shines through" the body - reported as the inner faces of cylinders
                    // lighting up. The same backface-cull formula as in rasterize_3d, including the ray
                    // FROM THE EYE in perspective (a shared `fwd` lies towards the edges of the frame, see
                    // `view_dir_at`).
                    let n = qymcad_ui_state::v_norm(qymcad_ui_state::v_cross(qymcad_ui_state::v_sub(wb, wa), qymcad_ui_state::v_sub(wc, wa)));
                    if qymcad_ui_state::v_dot(n, qymcad_ui_state::view_dir_at(&pn.cam, wa, basis.2, qymcad_ui_state::persp_inv_d_eye(&pn.cam, pn.set, rect.height() * 0.5))) >= 0.0 {
                        continue;
                    }
                    let b = hm.vertices.len() as u32;
                    for w in [wa, wb, wc] {
                        hm.colored_vertex(scr.at(w).0, fill);
                    }
                    hm.add_triangle(b, b + 1, b + 2);
                }
                if !hm.is_empty() {
                    painter.add(egui::Shape::mesh(hm));
                }
            }
        }
        return;
    }
    // a world or datum plane: a square frame from the candidate's frame (scaled by the scene, see pick)
    let h = qymcad_ui_state::plane_pick_half_size(pn);
    for (cand, fr) in qymcad_ui_state::sketch_plane_candidates(pn) {
        if &cand != sp {
            continue;
        }
        let corners = [fr.lift(Point2::new(-h, -h)), fr.lift(Point2::new(h, -h)), fr.lift(Point2::new(h, h)), fr.lift(Point2::new(-h, h))];
        let poly: Vec<Pos2> = corners.iter().map(|p| scr.at([p.x, p.y, p.z]).0).collect();
        let fill = qymcad_scheme::a(col, 70);
        painter.add(egui::Shape::convex_polygon(poly, fill, Stroke::new(2.0, col)));
    }
}

/// Highlighting the plane candidates while a sketch plane is being chosen (translucent squares, brighter under the cursor).
pub fn draw_sketch_plane_picker(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    // active while choosing a sketch plane, while placing an import, in the MIRROR (16) / DATUM PLANE (20)
    // / SPLIT (27) command, while picking a MIRRORED PART and while picking a SECTION - one click-pick for all
    if !(pn.picking.is_sketch_plane()
        || pn.picking.replace_sketch().is_some()
        || pn.pending_import.curves.is_some()
        || pn.armed.cmd_kind() == 16
        || pn.armed.cmd_kind() == 20
        || pn.armed.cmd_kind() == 27
        || pn.armed.cmd_kind() == 29
        || pn.mirror.in_hand()
        || pn.section.pick)
        || !pn.mode_3d
    {
        return;
    }
    let basis = pn.cam.basis();
    // the plane ALREADY CHOSEN is lit PERMANENTLY (in blue), the candidate under the cursor in yellow.
    // That way both what is chosen (a FACE included) and what is under the cursor right now are visible -
    // before, a chosen face could not be seen at all.
    let picked = match pn.armed.cmd_kind() {
        16 => pn.mirror.plane,
        20 => pn.datum.plane_pick,
        27 | 29 => pn.split.plane,
        _ => None,
    };
    let hovered = painter.ctx().pointer_hover_pos().and_then(|p| qymcad_pick::pick_sketch_plane_at(pn, rect, p));
    // An earlier edition drew ALL the candidate planes PERMANENTLY (blue frames). Reported behaviour:
    // squares that cannot be selected, and planes lighting up that have nothing to do with anything -
    // the wall of a cylinder among them. Both complaints have one cause: the painter overlay is drawn
    // WITHOUT a depth test against the 3D scene (the GPU and software rasters run EARLIER, the overlay is
    // 2D shapes ON TOP with no Z), while pick_sketch_plane_at gives priority to ANY body under the cursor
    // (even when the face itself was not recognised) - so a square COULD be drawn over or through a body
    // that actually intercepts the click (and cannot be selected), and could SHINE THROUGH curved faces
    // (a cylinder) the plane does not touch at all. It was removed: what is shown is ONLY what is really
    // under the cursor now - which is also what will be selected.
    // first the chosen one (blue), then - if it differs - the one under the cursor (yellow) on top
    if let Some(sp) = &picked {
        if hovered.as_ref() != Some(sp) {
            draw_pick_plane(pn, painter, rect, &basis, sp, pn.scheme.pal.plane_face());
        }
    }
    if let Some(sp) = &hovered {
        draw_pick_plane(pn, painter, rect, &basis, sp, pn.scheme.pal.highlight());
        // the origin snap marker - only for a NEW sketch (not for a mirror, a datum or an import)
        if pn.picking.is_sketch_plane() {
            if let Some(pos) = painter.ctx().pointer_hover_pos() {
                if let Some((uv, fr)) = qymcad_pick::sketch_origin_snap(pn, rect, pos, sp)
                    .zip(qymcad_ui_state::world_frame_of_plane(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, sp))
                {
                    let w = fr.lift(uv);
                    let s = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at([w.x, w.y, w.z]).0;
                    painter.circle_filled(s, 4.0, pn.scheme.pal.snap_point());
                    painter.circle_stroke(s, 6.5, egui::Stroke::new(1.5, pn.scheme.pal.emphasis()));
                }
            }
        }
    }
}

/// The mate glyphs in the 3D view: a line A <-> B (what is joined to what) + a badge with the kind's icon
/// IN THE MIDDLE; the selected one (Sel::Joint) and the one under the cursor (hover_joint) are lit, which
/// ties the view to the list.
pub fn draw_joints(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    if !pn.mode_3d {
        return;
    }
    let basis = pn.cam.basis();
    for j in &pn.project.joints {
        if !qymcad_ui_state::joint_visible(pn.active_path, pn.project, pn.set, pn.workbench, j) {
            continue; // switched off by the checkbox, or a joint of another context (a nested subassembly) - not drawn
        }
        let Some((a, b)) = qymcad_ui_state::joint_endpoints(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, j, rect, &basis) else {
            continue;
        };
        let mid = Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
        let sel = pn.sel == Sel::Joint(j.id);
        let hot = pn.hover.joint == Some(j.id);
        let (r, col) = if sel {
            (10.0, pn.scheme.pal.active())
        } else if hot {
            (9.0, pn.scheme.pal.joint_hover())
        } else {
            (8.0, pn.scheme.pal.joint_idle())
        };
        // the line between connectors A and B (it shows what is joined to what) + the end dots
        if a.distance(b) > 2.0 {
            let lc = qymcad_scheme::a(col, if sel || hot { 210 } else { 95 });
            painter.line_segment([a, b], Stroke::new(if sel || hot { 1.8 } else { 1.0 }, lc));
            painter.circle_filled(a, 2.0, lc);
            painter.circle_filled(b, 2.0, lc);
        }
        painter.circle_filled(mid, r, qymcad_scheme::a(pn.scheme.pal.glyph_backing(), 225));
        painter.circle_stroke(mid, r, Stroke::new(if sel || hot { 2.0 } else { 1.4 }, col));
        paint_joint_glyph(painter, mid, r, j.kind, col);
    }
}

/// Highlighting faces while a mate is being picked: the selected face A in green, the one under the
/// cursor in blue. The face transform is the display one (the active context's frame, as for render and pick).
pub fn draw_joint_pick_highlight(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    // EVERY TOOL THAT ASKS FOR GEOMETRY, not only the mate pick.
    //
    // The highlight is needed while a joint is being PICKED (joint_pick_faces) and while an existing
    // joint's ANCHOR is being CHANGED (joint_edit_repick) - otherwise edges and faces do not light up
    // under the cursor and the anchor is chosen blind.
    //
    // The modes used to be listed here by name, and every new tool was forgotten: first the highlight
    // went silent while a secondary axis was being specified (reported behaviour: moving the cursor over
    // the part lit nothing at all), and then the `every_picking_tool_highlights` guard found FOUR more of
    // the same - a separate anchor, a group, a width, a tangency. A person was aiming blind in each of them.
    //
    // ONE LIST IS ASKED (`gui/assembly_tools.rs`) rather than the modes being listed here: a list of its
    // own in every place is exactly the illness that left five tools without a highlight and five that
    // Esc would not release.
    //
    // EDITING A JOINT IS ALSO A REASON TO LIGHT UP. No tool need be in hand at all while a person looks at
    // a joint and edits it: they must see WHERE its anchors sit.
    // a relation takes mates from the list, not geometry, but the anchors of the mates it has taken are lit all the same
    if !qymcad_ui_state::assembly_wants_geometry(pn) && pn.joint.edit.is_none() && pn.joint.relation_pick.is_none() {
        return;
    }
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let ctx = qymcad_ui_state::current_ctx_id(pn.active_path, pn.project);
    use qymcad_core::feature::AnchorRef;
    let hl = |body: Id, fi: usize, col: Color32| {
        if let Some(mi) = pn.project.mesh_index(body) {
            if let Some(face) = pn.project.bodies.get(mi).and_then(|b| b.faces.get(fi)) {
                let mesh = &pn.project.bodies[mi].mesh;
                let wt = pn.project.body_display_transform(body, ctx);
                let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
                // one fill mesh - with no "needles" from the radial edges of the fan triangulation (as in the picker)
                let mut hm = egui::Mesh::default();
                for &tri in &face.triangles {
                    let t = mesh.triangle(tri as usize);
                    let base = hm.vertices.len() as u32;
                    for v in &t {
                        hm.colored_vertex(scr.at(tp([v.x, v.y, v.z])).0, col);
                    }
                    hm.add_triangle(base, base + 1, base + 2);
                }
                if !hm.is_empty() {
                    painter.add(egui::Shape::mesh(hm));
                }
            }
        }
    };
    // highlighting an EDGE by its persistent id
    let hl_edge = |body: Id, eid: u32, col: Color32| {
        // THE EDGES COME FROM THE CACHE. There used to be a direct call into the kernel here, while the
        // highlight is drawn EVERY FRAME for as long as a joint is being picked - even with the mouse
        // standing still. On a real part that is a full extraction of the edges from the B-rep three
        // times per frame, and that is where "placed a mate, waited, the CAD stopped answering" came
        // from. The data are the same; the price is now paid once per rebuild.
        if let Some(edges) = qymcad_pick::body_edges_cached(pn.cache, pn.live, pn.regen, body) {
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
                if id == eid {
                    let sp: Vec<Pos2> = poly.iter().map(|p| scr.at(tp(p)).0).collect();
                    painter.add(egui::Shape::line(sp, Stroke::new(3.0, col)));
                }
            }
            // a CIRCULAR edge (the rim of a hole or a cylinder): the CENTRE AXIS is drawn - a concentric
            // anchor rather than a point on the rim. The centre and the axis come from the regen cache
            // (the true circle from OCCT).
            if let Some(e) = pn.project.regen_edges.get(&body).and_then(|es| es.iter().find(|e| e.id == eid)) {
                if e.is_circular() {
                    let (c, ax) = (e.center, e.axis);
                    let l = (e.radius * 2.0).max(6.0);
                    let a = [c[0] - ax[0] * l, c[1] - ax[1] * l, c[2] - ax[2] * l];
                    let b = [c[0] + ax[0] * l, c[1] + ax[1] * l, c[2] + ax[2] * l];
                    let (pa, pb) = (scr.at(tp(&[a[0] as f32, a[1] as f32, a[2] as f32])).0, scr.at(tp(&[b[0] as f32, b[1] as f32, b[2] as f32])).0);
                    painter.add(egui::Shape::dashed_line(&[pa, pb], Stroke::new(1.5, col), 6.0, 4.0));
                    painter.circle_filled(scr.at(tp(&[c[0] as f32, c[1] as f32, c[2] as f32])).0, 3.5, col);
                }
            }
        }
    };
    // highlighting a VERTEX (an edge end) with a dot
    let hl_vert = |body: Id, eid: u32, end: bool, col: Color32| {
        if let Some(edges) = qymcad_pick::body_edges_cached(pn.cache, pn.live, pn.regen, body) {
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
                if id == eid && poly.len() >= 2 {
                    let vtx = if end { &poly[poly.len() - 1] } else { &poly[0] };
                    painter.circle_filled(scr.at(tp(vtx)).0, 5.0, col);
                }
            }
        }
    };
    let green = qymcad_scheme::a(pn.scheme.pal.joint_pick_a(), 200);
    let blue = qymcad_scheme::a(pn.scheme.pal.joint_pick_b(), 200);
    // THE ANCHORS OF THE JOINT BEING EDITED - both of them, each in its own colour: A green, B blue, as when picking.
    if let Some(jid) = pn.joint.edit {
        if let Some(j) = pn.project.joints.iter().find(|x| x.id == jid) {
            for (cid, col) in [(j.a, green), (j.b, blue)] {
                let Some(anchor) = pn.project.connector(cid).map(|c| c.anchor.clone()) else { continue };
                match anchor {
                    AnchorRef::FaceCenter(b, k) => hl(b, k.index as usize, col),
                    AnchorRef::EdgeMid(b, eid) => hl_edge(b, eid, col),
                    AnchorRef::Vertex(b, eid, end) => hl_vert(b, eid, end, col),
                    _ => {}
                }
            }
        }
    }
    // WHAT THE OTHER TOOLS HAVE TAKEN is lit as the first anchor of a mate is: the surfaces of a tangency and the walls of
    // a width, the parts of a group whole, and the anchors of the mates a relation ties. The count in the bar grew while
    // nothing on the canvas said what had been taken.
    let taken: Vec<&AnchorRef> = pn.joint.tangent_pick.iter().chain(pn.joint.width_pick.iter()).flatten().map(|(_, a)| a).collect();
    for anchor in taken {
        match anchor {
            AnchorRef::FaceCenter(b, k) => hl(*b, k.index as usize, green),
            AnchorRef::EdgeMid(b, eid) => hl_edge(*b, *eid, green),
            AnchorRef::Vertex(b, eid, end) => hl_vert(*b, *eid, *end, green),
            _ => {}
        }
    }
    for comp in pn.joint.group_pick.iter().flatten() {
        for b in pn.project.component_bodies(*comp) {
            let n = pn.project.mesh_index(b).and_then(|mi| pn.project.bodies.get(mi)).map_or(0, |x| x.faces.len());
            for fi in 0..n {
                hl(b, fi, green);
            }
        }
    }
    for (jid, _) in pn.joint.relation_pick.iter().flat_map(|r| r.picks.iter()) {
        if let Some(j) = pn.project.joints.iter().find(|x| x.id == *jid) {
            for cid in [j.a, j.b] {
                match pn.project.connector(cid).map(|c| c.anchor.clone()) {
                    Some(AnchorRef::FaceCenter(b, k)) => hl(b, k.index as usize, green),
                    Some(AnchorRef::EdgeMid(b, eid)) => hl_edge(b, eid, green),
                    Some(AnchorRef::Vertex(b, eid, end)) => hl_vert(b, eid, end, green),
                    _ => {}
                }
            }
        }
    }
    // the anchor A that is already fixed
    if let Some((_, anchor)) = &pn.joint.pick_first {
        match anchor {
            AnchorRef::FaceCenter(b, k) => hl(*b, k.index as usize, green),
            AnchorRef::EdgeMid(b, eid) => hl_edge(*b, *eid, green),
            AnchorRef::Vertex(b, eid, end) => hl_vert(*b, *eid, *end, green),
            _ => {}
        }
    }
    // under the cursor, ONLY the parts of the active context. The ghost (show_context) bodies of other
    // subassemblies are dimmed for a reason: under a joint tool they are inactive and are not highlighted,
    // so that they do not get in the way. A joint is hung inside the context anyway.
    let in_ctx = |body: Id| pn.project.body_owner(body).is_some_and(|o| pn.project.component_is_within(o, ctx));
    // WHAT IS LIT IS EXACTLY WHAT THE CLICK WILL TAKE. There used to be a parsing of the anchor mode of
    // its own here, and it diverged from the click's parsing the moment the kind of anchor started being
    // chosen under the cursor: a person saw a face lit up and a corner was taken. There is one door -
    // `infer_mate_anchor`.
    if let Some(pos) = painter.ctx().pointer_hover_pos() {
        if rect.contains(pos) {
            // THE DOOR IS CHOSEN BY THE TOOL: specifying an axis takes an edge, specifying an anchor
            // takes the nearest snap point. Lighting one thing and taking another is a lie.
            let under = if pn.joint.axis_pick.is_some() { qymcad_pick::infer_axis_anchor(pn, rect, pos) } else { qymcad_pick::infer_mate_anchor(pn, rect, pos) };
            match under {
                Some((body, AnchorRef::FaceCenter(_, key))) if in_ctx(body) => hl(body, key.index as usize, blue),
                Some((body, AnchorRef::EdgeMid(_, eid))) if in_ctx(body) => hl_edge(body, eid, blue),
                Some((body, AnchorRef::Vertex(_, eid, end))) if in_ctx(body) => hl_vert(body, eid, end, blue),
                _ => {}
            }
        }
    }
}
