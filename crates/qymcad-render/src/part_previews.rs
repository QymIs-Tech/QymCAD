//! THE PREVIEWS OF THE COMMANDS OF A PART: arrays, primitives, blends, holes, drafts, threads, lofts, datums, mirrors, splits, sweeps, the faces a trial adds.

use super::*;

/// THE PATTERN PREVIEW: wireframe ghosts of the copies (the source body's bbox) at the instance
/// positions, before Enter. Linear: a 3D grid of i*d1 + j*d2 + k*d3. Circular: a rotation about the axis
/// (world Z or a datum). The original is not drawn.
/// It is drawn WHILE EDITING too: there the pattern result is hidden (`edit_result_body`) and the source
/// is visible, so the ghosts are the preview.
pub fn draw_array_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    if !matches!(pn.armed.cmd_kind(), 17 | 18) || !pn.mode_3d {
        return;
    }
    let Some(src) = qymcad_ui_state::selected_body(pn.project, &pn.sel) else { return };
    let Some(mi) = pn.project.mesh_index(src) else { return };
    let Some(bb) = pn.project.bodies[mi].mesh.bounds() else { return };
    let basis = pn.cam.basis();
    let st = Stroke::new(1.2, qymcad_scheme::a(pn.scheme.pal.preview_array(), 170));
    let (mn, mx) = (bb.min, bb.max);
    let base: [[f64; 3]; 8] = [[mn.x, mn.y, mn.z], [mx.x, mn.y, mn.z], [mx.x, mx.y, mn.z], [mn.x, mx.y, mn.z], [mn.x, mn.y, mx.z], [mx.x, mn.y, mx.z], [mx.x, mx.y, mx.z], [mn.x, mx.y, mx.z]];
    const EDGES: [(usize, usize); 12] = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    let draw_box = |wc: [[f64; 3]; 8]| {
        let pts: [Pos2; 8] = std::array::from_fn(|i| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(wc[i]).0);
        for (a, b) in EDGES {
            painter.line_segment([pts[a], pts[b]], st);
        }
    };
    if pn.armed.cmd_kind() == 17 {
        let (dx, dy, dz) = qymcad_ui_state::arr_vec(pn.arr.dir, qymcad_ui_state::cmd_val(pn.cmd, "step"));
        let (dx2, dy2, dz2, c2) = if pn.arr.two {
            let (a, b, c) = qymcad_ui_state::arr_vec(pn.arr.dir2, qymcad_ui_state::cmd_val(pn.cmd, "step2"));
            (a, b, c, pn.arr.count2.max(1))
        } else {
            (0.0, 0.0, 0.0, 1)
        };
        let (dx3, dy3, dz3, c3) = if pn.arr.two && pn.arr.three {
            let (a, b, c) = qymcad_ui_state::arr_vec(pn.arr.dir3, qymcad_ui_state::cmd_val(pn.cmd, "step3"));
            (a, b, c, pn.arr.count3.max(1))
        } else {
            (0.0, 0.0, 0.0, 1)
        };
        // past the limit the pattern will be refused, and drawing its copies one by one would hold every frame
        let (c1, c2, c3) = if qymcad_core::feature::pattern_instances(&[pn.arr.count, c2, c3]).is_some() { (pn.arr.count.max(1), c2, c3) } else { (1, 1, 1) };
        for i in 0..c1 {
            for j in 0..c2 {
                for l in 0..c3 {
                    if i == 0 && j == 0 && l == 0 {
                        continue; // the original is already drawn as the body (or as the source while editing)
                    }
                    let (tx, ty, tz) = (i as f64 * dx + j as f64 * dx2 + l as f64 * dx3, i as f64 * dy + j as f64 * dy2 + l as f64 * dy3, i as f64 * dz + j as f64 * dz2 + l as f64 * dz3);
                    draw_box(std::array::from_fn(|k| [base[k][0] + tx, base[k][1] + ty, base[k][2] + tz]));
                }
            }
        }
    } else {
        let c = pn.arr.count.max(1);
        let angle = if pn.arr.full { 360.0 } else { qymcad_ui_state::cmd_val(pn.cmd, "angle") };
        let step = if angle.abs() >= 359.9 { 360.0 / c as f64 } else { angle / c as f64 };
        let (org, dir) = if pn.arr.axis != 0 {
            pn.project.datum_axes.iter().find(|d| d.id == pn.arr.axis).map(|d| (d.origin(), d.dir())).unwrap_or(([0.0; 3], [0.0, 0.0, 1.0]))
        } else {
            ([0.0; 3], [0.0, 0.0, 1.0])
        };
        for i in 1..c {
            let ang = (i as f64 * step).to_radians();
            draw_box(std::array::from_fn(|k| qymcad_ui_state::rotate_pt_about_axis(org, dir, ang, base[k])));
        }
    }
}

/// A WIREFRAME preview of the primitive being created, at the origin, from the command's current sizes.
/// It needs no kernel, so it is cheap and updates on the fly. The primitive placements are the OCCT
/// defaults (a cube/prism centred in XY from z=0; a cylinder/cone along +Z from z=0; a sphere/torus
/// centred at the origin).
pub fn draw_prim_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    // only while CREATING (there is no body). While editing (feat_edit) the body is already drawn, so the wireframe is not duplicated.
    if !(10..=15).contains(&pn.armed.cmd_kind()) || pn.cmd.edit.is_some() {
        return;
    }
    let basis = pn.cam.basis();
    let frame = pn.prim.frame; // the preview sits in the oriented placement frame (otherwise at the origin)
    let st = Stroke::new(1.5, pn.scheme.pal.preview_prim());
    let v = |k: &str| qymcad_ui_state::cmd_val(pn.cmd, k);
    let pr = |q: [f64; 3]| {
        let w = match frame {
            Some(m) => qymcad_core::feature::apply12(&m, q),
            None => q,
        };
        qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(w).0
    };
    let ring_xy = |r: f64, z: f64, n: usize| -> Vec<Pos2> {
        (0..=n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                pr([r * a.cos(), r * a.sin(), z])
            })
            .collect()
    };
    let poly = |painter: &egui::Painter, pts: &[Pos2]| {
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], st);
        }
    };
    let seg = |painter: &egui::Painter, a: [f64; 3], b: [f64; 3]| painter.line_segment([pr(a), pr(b)], st);
    match pn.armed.cmd_kind() {
        10 => {
            let (hx, hy, dz) = (v("dx") / 2.0, v("dy") / 2.0, v("dz"));
            let c = [[-hx, -hy], [hx, -hy], [hx, hy], [-hx, hy]];
            for &z in &[0.0, dz] {
                let r: Vec<Pos2> = c.iter().chain(std::iter::once(&c[0])).map(|p| pr([p[0], p[1], z])).collect();
                poly(painter, &r);
            }
            for p in c {
                seg(painter, [p[0], p[1], 0.0], [p[0], p[1], dz]);
            }
        }
        11 => {
            let (r, h) = (v("r"), v("h"));
            poly(painter, &ring_xy(r, 0.0, 48));
            poly(painter, &ring_xy(r, h, 48));
            for a in [0.0, 90.0, 180.0, 270.0_f64] {
                let a = a.to_radians();
                seg(painter, [r * a.cos(), r * a.sin(), 0.0], [r * a.cos(), r * a.sin(), h]);
            }
        }
        12 => {
            let r = v("r");
            poly(painter, &ring_xy(r, 0.0, 48));
            let ring_v = |swap: bool| -> Vec<Pos2> {
                (0..=48)
                    .map(|i| {
                        let a = std::f64::consts::TAU * i as f64 / 48.0;
                        let (c, s) = (r * a.cos(), r * a.sin());
                        pr(if swap { [0.0, c, s] } else { [c, 0.0, s] })
                    })
                    .collect()
            };
            poly(painter, &ring_v(false));
            poly(painter, &ring_v(true));
        }
        13 => {
            let (r1, r2, h) = (v("r1"), v("r2"), v("h"));
            if r1 > 1e-6 {
                poly(painter, &ring_xy(r1, 0.0, 48));
            }
            if r2 > 1e-6 {
                poly(painter, &ring_xy(r2, h, 48));
            }
            for a in [0.0, 90.0, 180.0, 270.0_f64] {
                let a = a.to_radians();
                seg(painter, [r1 * a.cos(), r1 * a.sin(), 0.0], [r2 * a.cos(), r2 * a.sin(), h]);
            }
        }
        14 => {
            let (mr, tr) = (v("major"), v("minor"));
            poly(painter, &ring_xy(mr + tr, 0.0, 64));
            poly(painter, &ring_xy((mr - tr).max(0.0), 0.0, 64));
        }
        15 => {
            let (r, h, n) = (v("r"), v("h"), pn.prim.n.max(3) as usize);
            poly(painter, &ring_xy(r, 0.0, n));
            poly(painter, &ring_xy(r, h, n));
            for i in 0..n {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                seg(painter, [r * a.cos(), r * a.sin(), 0.0], [r * a.cos(), r * a.sin(), h]);
            }
        }
        _ => {}
    }
}

/// Draw a body's edges (for picking): the selected ones (by id) in orange, the rest in grey. ONLY under
/// the Chamfer/Fillet command (outside them edges are not pickable, so the view is not littered with a
/// mesh and orange edges).
pub fn draw_body_edges(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    if !matches!(pn.armed.cmd_kind(), 4 | 5 | 32) || pn.edges.polys.is_empty() {
        return;
    }
    let basis = pn.cam.basis();
    let wt = pn.edges.body.map(|b| pn.project.body_display_transform(b, qymcad_ui_state::current_ctx_id(pn.active_path, pn.project))).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
    let tp = |p: &[f32; 3]| -> [f64; 3] {
        let v = [p[0] as f64, p[1] as f64, p[2] as f64];
        if qymcad_core::feature::is_identity12(&wt) {
            v
        } else {
            qymcad_core::feature::apply12(&wt, v)
        }
    };
    let picked: Vec<bool> = pn.edges.ids.iter().map(|id| *id != 0 && pn.gsel.edges.contains(id)).collect();
    for i in edges_to_draw(&pn.edges.polys, &picked, EDGE_SEGMENTS_DRAWN) {
        let poly = &pn.edges.polys[i];
        let sel = picked.get(i).copied().unwrap_or(false);
        let (col, w) = if sel { (pn.scheme.pal.selected(), 2.6) } else { (pn.scheme.pal.edge_idle(), 1.0) };
        let pts: Vec<Pos2> = poly.iter().map(|p| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(tp(p)).0).collect();
        for k in 0..pts.len().saturating_sub(1) {
            painter.line_segment([pts[k], pts[k + 1]], Stroke::new(w, col));
        }
    }
}

/// How many pieces of edges a frame draws for picking. Reported behaviour: the window fell over on a chamfer of a body
/// made from a mesh as it is - every side of its triangles an edge, and the frame's geometry grew to 402 601 368 bytes
/// against the graphics card's limit of 268 435 456.
pub const EDGE_SEGMENTS_DRAWN: usize = 200_000;

/// Which of the edges `polys` to draw: every picked one, and the others in order while the pieces drawn stay within
/// `budget`.
pub fn edges_to_draw(polys: &[Vec<[f32; 3]>], picked: &[bool], budget: usize) -> Vec<usize> {
    let pieces = |i: usize| polys[i].len().saturating_sub(1);
    let mut out: Vec<usize> = (0..polys.len()).filter(|&i| picked.get(i).copied().unwrap_or(false)).collect();
    let mut used: usize = out.iter().map(|&i| pieces(i)).sum();
    for i in 0..polys.len() {
        if picked.get(i).copied().unwrap_or(false) {
            continue;
        }
        if used + pieces(i) > budget {
            break;
        }
        used += pieces(i);
        out.push(i);
    }
    out
}

/// THE FILLET OR THE CHAMFER BEFORE ENTER: along every picked edge, the lines where it meets the faces and its section
/// at both ends, from the value in the field. A chamfer by a leg and an angle takes its second leg as leg * tan(angle),
/// by two legs its second field; the first leg lies on the reference face when one is named. With none named the kernel's
/// own order of the faces decides which leg goes where: the outline is drawn only until the trial has built the chamfer,
/// whose face then stands for the preview.
pub fn draw_edge_blend_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let Some(body) = pn.edges.body else { return };
    let Some(mesh) = pn.project.mesh_index(body).map(|mi| &pn.project.bodies[mi].mesh) else { return };
    let blend = if pn.armed.cmd_kind() == 4 {
        qymcad_ui_state::Blend::Round(qymcad_ui_state::cmd_val(pn.cmd, "radius"))
    } else {
        use qymcad_core::feature::ChamferMode;
        let d = qymcad_ui_state::cmd_val(pn.cmd, "dist");
        let d2 = qymcad_ui_state::cmd_val(pn.cmd, "d2");
        let second = match pn.chamfer.mode {
            ChamferMode::Symmetric => d,
            ChamferMode::TwoDist => d2,
            ChamferMode::DistAngle => d * d2.to_radians().tan(),
        };
        // the kernel lays the first leg on the reference face when one is named; with none named its own order of the
        // faces decides, which the mesh does not tell - there the face the trial built stands for the preview alone
        let reference =
            (pn.chamfer.ref_face != 0).then(|| pn.project.mesh_index(body).and_then(|mi| pn.project.bodies[mi].faces.iter().find(|f| f.id == pn.chamfer.ref_face)).map(|f| f.normal)).flatten();
        if pn.chamfer.mode != ChamferMode::Symmetric && reference.is_none() && qymcad_ui_state::trial_faces(painter.ctx()).is_some() {
            return;
        }
        qymcad_ui_state::Blend::Cut(d, second, reference)
    };
    let wt = pn.project.body_display_transform(body, qymcad_ui_state::current_ctx_id(pn.active_path, pn.project));
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let col = qymcad_scheme::a(pn.scheme.pal.preview(), 220);
    for (i, poly) in pn.edges.polys.iter().enumerate() {
        if !pn.edges.ids.get(i).is_some_and(|id| *id != 0 && pn.gsel.edges.contains(id)) {
            continue;
        }
        for line in qymcad_ui_state::edge_blend_outline(mesh, poly, blend) {
            let pts: Vec<Pos2> = line.iter().map(|p| scr.at(qymcad_core::feature::apply12(&wt, *p)).0).collect();
            painter.add(egui::Shape::line(pts, Stroke::new(1.5, col)));
        }
    }
}

/// THE HOLE BEFORE ENTER: its bore and recess drawn where the rebuild drills them - at the centre of the picked face,
/// or at every free point of the picked sketch - from the values in the fields.
pub fn draw_hole_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let v = |k: &str| qymcad_ui_state::cmd_val(pn.cmd, k);
    let (dia2, depth2) = if pn.hole.kind != 0 { (v("dia2"), v("depth2")) } else { (0.0, 0.0) };
    let tool = qymcad_core::model::HoleTool { kind: pn.hole.kind, diameter: v("diameter"), depth: v("depth"), dia2, depth2 };
    let frames: Vec<[f64; 12]> = if pn.hole.mode == 1 {
        pn.hole.sketch.map(|sid| pn.project.sketch_hole_points(sid, pn.hole.flip)).unwrap_or_default()
    } else {
        let qymcad_ui_state::Sel::Face(mi, fi) = pn.sel else { return };
        let (Some(body), Some(face)) = (pn.project.mesh_id(mi), pn.project.bodies.get(mi).and_then(|b| b.faces.get(fi))) else { return };
        let wt = pn.project.body_display_transform(body, qymcad_ui_state::current_ctx_id(pn.active_path, pn.project));
        let pl = qymcad_core::feature::PlaneFrame::from_origin_normal([face.centroid.x, face.centroid.y, face.centroid.z], face.normal, 0.0).matrix12();
        vec![qymcad_core::feature::compose12(&wt, &pl)]
    };
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let col = qymcad_scheme::a(pn.scheme.pal.preview(), 220);
    for pl in &frames {
        for line in qymcad_ui_state::hole_outline(pl, tool) {
            painter.add(egui::Shape::line(line.iter().map(|p| scr.at(*p).0).collect(), Stroke::new(1.5, col)));
        }
    }
}

/// THE DRAFT BEFORE ENTER: every picked face drawn leaning as the rebuild will tilt it, about the neutral face, from
/// the angle in the field. Nothing is drawn until the neutral face is picked: without it there is no pull to lean by.
pub(crate) fn draft_preview_lines(pn: &Painting) -> Vec<Vec<[f64; 3]>> {
    let Some(body) = pn.gsel.faces_body else { return Vec::new() };
    let Some(b) = pn.project.mesh_index(body).map(|mi| &pn.project.bodies[mi]) else { return Vec::new() };
    let Some(np) = (pn.draft.neutral != 0).then(|| b.faces.iter().find(|f| f.id == pn.draft.neutral)).flatten() else { return Vec::new() };
    let pull = if pn.draft.flip { [-np.normal[0], -np.normal[1], -np.normal[2]] } else { np.normal };
    let angle = qymcad_ui_state::cmd_val(pn.cmd, "angle");
    let wt = pn.project.body_display_transform(body, qymcad_ui_state::current_ctx_id(pn.active_path, pn.project));
    b.faces
        .iter()
        .filter(|f| f.id != 0 && pn.gsel.faces.contains(&f.id))
        .flat_map(|f| qymcad_ui_state::draft_outline(&b.mesh, f, [np.centroid.x, np.centroid.y, np.centroid.z], pull, angle))
        .map(|line| line.iter().map(|p| qymcad_core::feature::apply12(&wt, *p)).collect())
        .collect()
}

pub fn draw_draft_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let col = qymcad_scheme::a(pn.scheme.pal.preview(), 220);
    for line in draft_preview_lines(pn) {
        painter.add(egui::Shape::line(line.iter().map(|p| scr.at(*p).0).collect(), Stroke::new(1.5, col)));
    }
}

/// The thread preview: a ghost of the HELIX (the crest of the turn) along the axis from the rim over
/// the length, from the current parameters. Light (a polyline), it builds no geometry - the real body
/// appears on Enter.
pub fn draw_thread_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let Some(src) = pn.thread.src else { return };
    if pn.thread.edge == 0 {
        return;
    }
    let (center, axis) = pn.thread.axis;
    let r = pn.thread.radius;
    let pitch = qymcad_ui_state::cmd_val(pn.cmd, "pitch").max(0.05);
    let length = qymcad_ui_state::cmd_val(pn.cmd, "length").max(0.1);
    let lead = pitch * pn.thread.starts.max(1) as f64;
    let scale = |a: [f64; 3], s: f64| [a[0] * s, a[1] * s, a[2] * s];
    let add = |a: [f64; 3], b: [f64; 3]| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
    let ax = qymcad_ui_state::v_norm(axis);
    let refx = if ax[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let u = qymcad_ui_state::v_norm(qymcad_ui_state::v_sub(refx, scale(ax, qymcad_ui_state::v_dot(refx, ax)))); // the radial basis, perpendicular to the axis
    let v = qymcad_ui_state::v_cross(ax, u);
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let wt = pn.project.body_display_transform(src, qymcad_ui_state::current_ctx_id(pn.active_path, pn.project));
    let turns = length / lead;
    let n = ((turns * 24.0).ceil() as usize).clamp(8, 4000);
    let sign = if pn.thread.left { -1.0 } else { 1.0 };
    let col = pn.scheme.pal.preview_axis();
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let frac = i as f64 / n as f64;
            let a = sign * frac * turns * std::f64::consts::TAU;
            let radial = add(scale(u, a.cos()), scale(v, a.sin()));
            let p = add(add(center, scale(ax, frac * length)), scale(radial, r));
            scr.at(qymcad_core::feature::apply12(&wt, p)).0
        })
        .collect();
    painter.add(egui::Shape::line(pts, Stroke::new(1.4, col)));
    // the thread axis, dashed over the length
    let a0 = scr.at(qymcad_core::feature::apply12(&wt, center)).0;
    let a1 = scr.at(qymcad_core::feature::apply12(&wt, add(center, scale(ax, length)))).0;
    painter.add(egui::Shape::dashed_line(&[a0, a1], Stroke::new(0.8, col), 5.0, 4.0));
}

/// The live loft preview: the section outlines + the longitudinal edges between neighbouring sections.
pub fn draw_loft_preview(cam: Cam3, loft: &LoftParams, project: &Project, scheme: &SchemeUi, set: &Settings, painter: &egui::Painter, rect: Rect) {
    let sections = qymcad_ui_state::loft_preview(loft, project);
    if sections.is_empty() {
        return;
    }
    let basis = cam.basis();
    let sp = |p: [f64; 3]| qymcad_ui_state::Screen { cam: &cam, set, rect, basis: &basis }.at(p).0;
    let scol = qymcad_scheme::a(scheme.pal.preview(), 220);
    for sec in &sections {
        let m = sec.len();
        for k in 0..m {
            painter.line_segment([sp(sec[k]), sp(sec[(k + 1) % m])], Stroke::new(1.8, scol));
        }
    }
    // the longitudinal edges between neighbouring sections (sampled by loop parameter, so the lengths may differ)
    for pair in sections.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if a.is_empty() || b.is_empty() {
            continue;
        }
        let steps = a.len().max(b.len());
        for s in 0..steps {
            let ia = s * a.len() / steps;
            let ib = s * b.len() / steps;
            painter.line_segment([sp(a[ia]), sp(b[ib])], Stroke::new(1.0, scol));
        }
    }
}

/// MEASURING IN 3D: markers on the picked elements, a leader between them and a plate with the number.
///
/// The number lives both in the status line and here, from ONE source (`measure_text`): two wordings of one
/// measurement drift apart silently, and then the line says one thing while the geometry says another.
pub fn draw_measure_3d(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    if !pn.m3.on || pn.m3.picks.is_empty() {
        return;
    }
    let basis = pn.cam.basis();
    let col = pn.scheme.pal.measure();
    let pts: Vec<Pos2> = pn.m3.picks.iter().map(|p| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(p.at).0).collect();
    for sp in &pts {
        painter.circle_stroke(*sp, 6.0, Stroke::new(2.0, col));
        painter.circle_filled(*sp, 2.5, col);
    }
    if pts.len() == 2 {
        painter.add(egui::Shape::dashed_line(&pts, Stroke::new(1.6, col), 7.0, 5.0));
    }
    let text = qymcad_ui_state::measure_text(pn);
    let at = pts.last().copied().unwrap_or(rect.center()) + egui::vec2(12.0, -18.0);
    let font = egui::FontId::proportional(13.0);
    let galley = painter.layout_no_wrap(text, font, pn.scheme.pal.plate_text());
    let pad = egui::vec2(6.0, 4.0);
    let bg = egui::Rect::from_min_size(at, galley.size() + pad * 2.0);
    painter.rect_filled(bg, 4.0, qymcad_scheme::a(pn.scheme.pal.measure(), 235));
    painter.galley(at + pad, galley, pn.scheme.pal.plate_text());
}

pub fn draw_comp_array_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    use qymcad_core::feature::apply12;
    if pn.carr.mode == 0 || !pn.mode_3d {
        return;
    }
    let Some(src_body) = pn.project.active_body(pn.carr.src) else { return };
    let Some(mi) = pn.project.mesh_index(src_body) else { return };
    let Some(bb) = pn.project.bodies[mi].mesh.bounds() else { return };
    let ctx = qymcad_ui_state::current_ctx_id(pn.active_path, pn.project);
    // THE GHOST IS COMPUTED THE WAY THE COPY WILL LATER LAND.
    //
    // This used to be `mat_mul12(step, wt)`, where `wt` is the body's transform IN THE VIEW FRAME. The
    // finished pattern places a copy differently: `mat_mul12(step, base)` in the source's PARENT frame
    // (`resolve_comp_patterns`). The two agreed only while the parent sat at the origin; as soon as the
    // source lay in an assembly with a position of its own, the ghosts drifted off the step.
    let (base, parent) =
        pn.project.components.iter().find(|c| c.id == pn.carr.src).map(|c| (c.transform, c.parent.unwrap_or(pn.project.root))).unwrap_or((qymcad_core::feature::PLACE_IDENTITY, pn.project.root));
    let pre = pn.project.relative_transform(parent, ctx);
    // EDITING A FINISHED PATTERN: the copies already stand on the screen. Drawing ghosts on top of them
    // would show twice as many frames as there are bodies.
    let already: usize = match pn.project.comp_patterns().iter().find(|p| p.id == pn.carr.edit) {
        Some(p) => p.copies.len(),
        None => 0,
    };
    let basis = pn.cam.basis();
    let st = Stroke::new(1.3, qymcad_scheme::a(pn.scheme.pal.preview_datum(), 190));
    let (mn, mx) = (bb.min, bb.max);
    let corners: [[f64; 3]; 8] = [[mn.x, mn.y, mn.z], [mx.x, mn.y, mn.z], [mx.x, mx.y, mn.z], [mn.x, mx.y, mn.z], [mn.x, mn.y, mx.z], [mx.x, mn.y, mx.z], [mx.x, mx.y, mx.z], [mn.x, mx.y, mx.z]];
    const EDGES: [(usize, usize); 12] = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    // GHOSTS ONLY FOR THE COPIES (i from 1): instance zero is the source itself, which is on screen anyway
    for m in comp_array_ghosts(pn, &pre, &base, already) {
        let pts: Vec<Pos2> = corners.iter().map(|c| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(apply12(&m, *c)).0).collect();
        for (a, b) in EDGES {
            painter.line_segment([pts[a], pts[b]], st);
        }
    }
}

/// WHERE THE COPIES WILL LAND - one computation for the ghost and for the check alike.
///
/// Without a preview the command is as blind as push-face once was: the count and the step are set, but
/// where the row will stand can only be seen by applying it and undoing.
///
/// It is computed by exactly the same means as the finished pattern (`resolve_comp_patterns`): the step is
/// applied in the source's PARENT frame, not in the view frame. Let those two computations drift apart and
/// the ghost would again point somewhere other than where the copies later land.
pub fn comp_array_ghosts(pn: &Painting, pre: &[f64; 12], base: &[f64; 12], already: usize) -> Vec<[f64; 12]> {
    use qymcad_core::feature::mat_mul12;
    let kind = qymcad_ui_state::comp_array_kind(pn.arr, pn.carr, pn.cmd);
    (1..kind.count())
        .filter(|i| (*i as usize) > already) // the copy already stands in the document - a ghost would be its twin
        .map(|i| mat_mul12(pre, &mat_mul12(&kind.step_transform(i), base)))
        .collect()
}

/// THE DATUM COMMAND PREVIEW: 20 a plane as a square (offset from the reference), 21 a point as a cross, 22 an axis as a line.
pub fn draw_datum_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    if !matches!(pn.armed.cmd_kind(), 20..=22) || !pn.mode_3d {
        return;
    }
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let st = Stroke::new(1.6, qymcad_scheme::a(pn.scheme.pal.preview_datum(), 205));
    match pn.armed.cmd_kind() {
        20 => {
            // the plane as Enter makes it - offset, tilted and turned (`datum_plane_made`), the square along its own axes
            let Some(m) = qymcad_ui_state::datum_plane_made(pn) else { return };
            let f = qymcad_core::feature::PlaneFrame::world_aligned(m.origin, m.normal, m.turn);
            let (c, u, v) = (m.origin, f.x, f.y);
            let h = 30.0;
            let corner = |su: f64, sv: f64| [c[0] + u[0] * su * h + v[0] * sv * h, c[1] + u[1] * su * h + v[1] * sv * h, c[2] + u[2] * su * h + v[2] * sv * h];
            let poly: Vec<Pos2> = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)].iter().map(|p| scr.at(*p).0).collect();
            painter.add(egui::Shape::convex_polygon(poly, qymcad_scheme::a(pn.scheme.pal.preview_datum(), 45), st));
        }
        21 => {
            use qymcad_core::feature::{apply12, is_identity12};
            // the preview point's position: "at a vertex" -> the world position of the picked vertex; "coordinates" -> the X/Y/Z fields
            let p = if pn.datum.pt_mode == 1 {
                match pn.datum.pt_vert {
                    Some((body, _, _, at)) => {
                        let wt = pn.project.body_display_transform(body, qymcad_ui_state::current_ctx_id(pn.active_path, pn.project));
                        if is_identity12(&wt) {
                            at
                        } else {
                            apply12(&wt, at)
                        }
                    }
                    None => {
                        // nothing picked yet - only the snap highlight of the candidate under the cursor
                        if let Some(w) = painter.ctx().pointer_hover_pos().and_then(|hp| qymcad_pick::pick_vertex_pos(pn, rect, hp)) {
                            painter.circle_stroke(scr.at(w).0, 5.0, Stroke::new(2.0, pn.scheme.pal.highlight()));
                        }
                        return;
                    }
                }
            } else {
                [qymcad_ui_state::cmd_val(pn.cmd, "x"), qymcad_ui_state::cmd_val(pn.cmd, "y"), qymcad_ui_state::cmd_val(pn.cmd, "z")]
            };
            let d = 5.0;
            for ax in 0..3 {
                let (mut a, mut b) = (p, p);
                a[ax] -= d;
                b[ax] += d;
                painter.line_segment([scr.at(a).0, scr.at(b).0], st);
            }
            painter.circle_filled(scr.at(p).0, 3.0, pn.scheme.pal.preview_datum());
            // snap: highlight the nearest VERTEX under the cursor (the "coordinates" mode)
            if pn.datum.pt_mode == 0 {
                if let Some(w) = painter.ctx().pointer_hover_pos().and_then(|hp| qymcad_pick::pick_vertex_pos(pn, rect, hp)) {
                    painter.circle_stroke(scr.at(w).0, 5.0, Stroke::new(2.0, pn.scheme.pal.highlight()));
                }
            }
        }
        22 => {
            let od = if pn.datum.axis_mode == 1 {
                Some((
                    [qymcad_ui_state::cmd_val(pn.cmd, "ox"), qymcad_ui_state::cmd_val(pn.cmd, "oy"), qymcad_ui_state::cmd_val(pn.cmd, "oz")],
                    [qymcad_ui_state::cmd_val(pn.cmd, "dx"), qymcad_ui_state::cmd_val(pn.cmd, "dy"), qymcad_ui_state::cmd_val(pn.cmd, "dz")],
                ))
            } else {
                pn.datum.axis_ref
            };
            if let Some((o, d)) = od {
                if d[0].abs() + d[1].abs() + d[2].abs() > 1e-9 {
                    let (s, e) = qymcad_ui_state::axis_segment(o, d, 45.0);
                    painter.line_segment([scr.at(s).0, scr.at(e).0], Stroke::new(2.4, pn.scheme.pal.preview_datum()));
                }
            }
            // the "two points" mode: the points gathered so far + the snap highlight of the candidate (a datum point or a vertex) under the cursor
            if pn.datum.axis_mode == 2 {
                for (_, w) in &pn.datum.axis_pts {
                    painter.circle_filled(scr.at(*w).0, 4.0, pn.scheme.pal.preview_datum());
                }
                let hov = painter.ctx().pointer_hover_pos().and_then(|hp| qymcad_pick::pick_datum_point_at(pn, rect, hp).map(|(_, w)| w).or_else(|| qymcad_pick::pick_vertex_pos(pn, rect, hp)));
                if let Some(w) = hov {
                    painter.circle_stroke(scr.at(w).0, 5.0, Stroke::new(2.0, pn.scheme.pal.highlight()));
                }
            }
        }
        _ => {}
    }
}

/// Highlighting the axis candidates while the circular pattern's axis is being clicked: the datum axes +
/// the straight edges of the source body; the one under the cursor and the selected one are brighter. The
/// same single click-pick as the mirror plane highlight.
pub fn draw_axis_picker(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    use qymcad_core::feature::{apply12, is_identity12};
    // active while picking the pattern axis (18), the Revolve axis (3, 64) OR in the DATUM AXIS command by edge/face (22, mode 0)
    let active = (pn.armed.cmd_kind() == 18 && pn.arr.axis_pick) || (pn.armed.cmd_kind() == 3 && pn.rev.pick_axis) || (pn.armed.cmd_kind() == 22 && pn.datum.axis_mode == 0);
    if !active || !pn.mode_3d {
        return;
    }
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let ctx = qymcad_ui_state::current_ctx_id(pn.active_path, pn.project);
    let hovered = painter.ctx().pointer_hover_pos().and_then(|p| qymcad_pick::pick_axis_at(pn, rect, p));
    for d in &pn.project.datum_axes {
        if let Some(wt) = qymcad_ui_state::datum_render_transform(pn, d.id) {
            let (s, e) = qymcad_ui_state::axis_segment(d.origin(), d.dir(), 45.0);
            let a = scr.at(apply12(&wt, s)).0;
            let b = scr.at(apply12(&wt, e)).0;
            let hot = matches!(hovered, Some(AxisHit::Datum(id)) if id == d.id) || pn.arr.axis == d.id;
            let col = if hot { pn.scheme.pal.highlight() } else { pn.scheme.pal.preview() };
            painter.line_segment([a, b], Stroke::new(if hot { 2.8 } else { 1.4 }, col));
        }
    }
    // ONLY THE EDGE UNDER THE CURSOR, NOT ALL OF THEM AT ONCE.
    //
    // This used to draw EVERY eligible edge of the assembly as a pale line, with the one under the cursor
    // brighter. On two cubes that is a hint; on a real machine it is a solid green wall of thousands of
    // lines, behind which neither the part nor the highlight itself can be seen.
    //
    // What is shown is what a person pointed at - as everywhere else in the highlighting.
    if let Some(AxisHit::Edge(k)) = hovered {
        if let Some((body, _id, poly)) = pn.edges.axes.get(k) {
            let wt = pn.project.body_display_transform(*body, ctx);
            let pts: Vec<Pos2> = poly.iter().map(|p| scr.at(apply12(&wt, [p[0] as f64, p[1] as f64, p[2] as f64])).0).collect();
            for i in 0..pts.len().saturating_sub(1) {
                painter.line_segment([pts[i], pts[i + 1]], Stroke::new(2.8, pn.scheme.pal.highlight()));
            }
        }
    }
    // a CYLINDRICAL face under the cursor: a fill + its axis (so that it is visible the click takes the hole's axis)
    if let Some(AxisHit::Face(body, fid)) = hovered {
        if let Some(mi) = pn.project.mesh_index(body) {
            if let Some(fi) = pn.project.bodies.get(mi).and_then(|b| b.faces.iter().position(|f| f.id == fid)) {
                let mesh = &pn.project.bodies[mi].mesh;
                let fwt = pn.project.body_display_transform(body, ctx);
                let ftp = |v: [f64; 3]| if is_identity12(&fwt) { v } else { apply12(&fwt, v) };
                let fill = qymcad_scheme::a(pn.scheme.pal.highlight(), 80);
                let mut hm = egui::Mesh::default();
                for &tri in &pn.project.bodies[mi].faces[fi].triangles {
                    let t = mesh.triangle(tri as usize);
                    let b = hm.vertices.len() as u32;
                    for v in &t {
                        hm.colored_vertex(scr.at(ftp([v.x, v.y, v.z])).0, fill);
                    }
                    hm.add_triangle(b, b + 1, b + 2);
                }
                if !hm.is_empty() {
                    painter.add(egui::Shape::mesh(hm));
                }
                // the face axis (world origin/dir) as a line
                if let Some((lo, ld)) = pn.live.shapes.get(&body).and_then(|s| s.face_axis(fid)) {
                    let ow = ftp(lo);
                    let z = if is_identity12(&fwt) { [0.0; 3] } else { apply12(&fwt, [0.0, 0.0, 0.0]) };
                    let dw = if is_identity12(&fwt) { ld } else { [apply12(&fwt, ld)[0] - z[0], apply12(&fwt, ld)[1] - z[1], apply12(&fwt, ld)[2] - z[2]] };
                    let (s, e) = qymcad_ui_state::axis_segment(ow, dw, 45.0);
                    painter.line_segment([scr.at(s).0, scr.at(e).0], Stroke::new(2.8, pn.scheme.pal.highlight()));
                }
            }
        }
    }
}

/// THE PREVIEW OF THE MIRROR: a ghost wireframe of the body reflected through the chosen plane,
/// before Enter (both creation AND editing: while editing, the result is hidden by
/// `edit_result_body` and the source is visible, so the ghost is the preview of the mirror).
pub fn draw_mirror_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    if pn.armed.cmd_kind() != 16 || !pn.mode_3d {
        return;
    }
    let Some(sp) = pn.mirror.plane else { return };
    let Some((o, n)) = qymcad_ui_state::mirror_plane_world(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, &sp) else { return };
    let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if nl < 1e-9 {
        return;
    }
    let nn = [n[0] / nl, n[1] / nl, n[2] / nl];
    let Some(src) = qymcad_ui_state::selected_body(pn.project, &pn.sel) else { return };
    let Some(mi) = pn.project.mesh_index(src) else { return };
    let Some(bb) = pn.project.bodies[mi].mesh.bounds() else { return };
    let basis = pn.cam.basis();
    let ctx = qymcad_ui_state::current_ctx_id(pn.active_path, pn.project);
    let wt = pn.project.body_display_transform(src, ctx);
    let st = Stroke::new(1.3, qymcad_scheme::a(pn.scheme.pal.preview_datum(), 190));
    let (mn, mx) = (bb.min, bb.max);
    let base: [[f64; 3]; 8] = [[mn.x, mn.y, mn.z], [mx.x, mn.y, mn.z], [mx.x, mx.y, mn.z], [mn.x, mx.y, mn.z], [mn.x, mn.y, mx.z], [mx.x, mn.y, mx.z], [mx.x, mx.y, mx.z], [mn.x, mx.y, mx.z]];
    const EDGES: [(usize, usize); 12] = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    // reflecting a point through the plane (o, nn): p' = p - 2*((p-o).nn)*nn (local -> world by the display transform first)
    let reflect = |p: [f64; 3]| -> [f64; 3] {
        let pw = qymcad_core::feature::apply12(&wt, p);
        let d = (pw[0] - o[0]) * nn[0] + (pw[1] - o[1]) * nn[1] + (pw[2] - o[2]) * nn[2];
        [pw[0] - 2.0 * d * nn[0], pw[1] - 2.0 * d * nn[1], pw[2] - 2.0 * d * nn[2]]
    };
    let pts: [Pos2; 8] = std::array::from_fn(|i| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(reflect(base[i])).0);
    for (a, b) in EDGES {
        painter.line_segment([pts[a], pts[b]], st);
    }
}

/// THE SPLIT PREVIEW: the cutting plane itself (a square sized by the body's extent) and the section line
/// along the body's edges. Without a preview the tool would be blind: before Enter neither where the cut
/// will run nor whether it hits the body at all would be visible.
pub fn draw_split_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    if !matches!(pn.armed.cmd_kind(), 27 | 29) || !pn.mode_3d {
        return;
    }
    let Some(sp) = pn.split.plane else { return };
    let Some((o, n)) = qymcad_ui_state::mirror_plane_world(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, &sp) else { return };
    let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if nl < 1e-9 {
        return;
    }
    let u3 = [n[0] / nl, n[1] / nl, n[2] / nl];
    let d = qymcad_ui_state::cmd_val(pn.cmd, "offset");
    let c = [o[0] + u3[0] * d, o[1] + u3[1] * d, o[2] + u3[2] * d];
    let Some(src) = qymcad_ui_state::op_target_body(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, pn.sel) else { return };
    let Some(mi) = pn.project.mesh_index(src) else { return };
    let Some(bb) = pn.project.bodies[mi].mesh.bounds() else { return };
    let basis = pn.cam.basis();
    let ctx = qymcad_ui_state::current_ctx_id(pn.active_path, pn.project);
    let wt = pn.project.body_display_transform(src, ctx);
    // THE SQUARE'S SIZE comes from the body's extent: a fixed 30 mm would look like a thread across a
    // large part and would cover the whole scene on a small one
    let half = {
        let c0 = qymcad_core::feature::apply12(&wt, [bb.min.x, bb.min.y, bb.min.z]);
        let c1 = qymcad_core::feature::apply12(&wt, [bb.max.x, bb.max.y, bb.max.z]);
        (((c1[0] - c0[0]).powi(2) + (c1[1] - c0[1]).powi(2) + (c1[2] - c0[2]).powi(2)).sqrt() * 0.5).max(1.0)
    };
    let up = if u3[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let normd = |a: [f64; 3]| {
        let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-9);
        [a[0] / l, a[1] / l, a[2] / l]
    };
    let uu = normd(cross(up, u3));
    let vv = cross(u3, uu);
    let corner = |su: f64, sv: f64| [c[0] + uu[0] * su * half + vv[0] * sv * half, c[1] + uu[1] * su * half + vv[1] * sv * half, c[2] + uu[2] * su * half + vv[2] * sv * half];
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let poly: Vec<Pos2> = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)].iter().map(|p| scr.at(*p).0).collect();
    let st = Stroke::new(1.6, qymcad_scheme::a(pn.scheme.pal.modify(), 220));
    painter.add(egui::Shape::convex_polygon(poly, qymcad_scheme::a(pn.scheme.pal.modify(), 40), st));

    // THE CUT LINE ACROSS THE BODY: the mesh edges the plane crosses give the outline of the future
    // seam - it shows at once whether the plane cuts the body or misses it.
    let mesh = &pn.project.bodies[mi].mesh;
    let side = |p: [f64; 3]| {
        let w = qymcad_core::feature::apply12(&wt, p);
        (w[0] - c[0]) * u3[0] + (w[1] - c[1]) * u3[1] + (w[2] - c[2]) * u3[2]
    };
    let cut = Stroke::new(2.0, pn.scheme.pal.cut_line());
    for ti in 0..mesh.tris.len() {
        let t = mesh.triangle(ti);
        let pts = [[t[0].x, t[0].y, t[0].z], [t[1].x, t[1].y, t[1].z], [t[2].x, t[2].y, t[2].z]];
        let ds = [side(pts[0]), side(pts[1]), side(pts[2])];
        let mut hits: Vec<Pos2> = Vec::new();
        for e in 0..3 {
            let (a, b) = (e, (e + 1) % 3);
            if (ds[a] > 0.0) == (ds[b] > 0.0) {
                continue;
            }
            let k = ds[a] / (ds[a] - ds[b]);
            let p = [pts[a][0] + (pts[b][0] - pts[a][0]) * k, pts[a][1] + (pts[b][1] - pts[a][1]) * k, pts[a][2] + (pts[b][2] - pts[a][2]) * k];
            hits.push(scr.at(qymcad_core::feature::apply12(&wt, p)).0);
        }
        if hits.len() == 2 {
            painter.line_segment([hits[0], hits[1]], cut);
        }
    }
}

/// THE VERTICES OF THE SELECTED EDGES - the points a radius is set at.
///
/// Without them a variable fillet would be a guessing game: a person does not know that a corner can be
/// clicked. The ones that already have a radius of their own are larger and in the active colour: a
/// display of state, not just an invitation.
pub fn draw_fillet_vertices(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    if pn.armed.cmd_kind() != 4 || pn.gsel.edges.is_empty() {
        return;
    }
    let Some(body) = pn.edges.body else { return };
    let picked: Vec<[[f64; 3]; 2]> = pn.project.regen_edges.get(&body).map(|es| es.iter().filter(|e| pn.gsel.edges.contains(&e.id)).map(|e| [e.a, e.b]).collect()).unwrap_or_default();
    if picked.is_empty() {
        return;
    }
    let basis = pn.cam.basis();
    for qymcad_core::model::VertexSpot { at: pt, name } in pn.project.vertex_spots(body) {
        let on_picked = picked.iter().flatten().any(|p| (p[0] - pt[0]).abs() < 1e-6 && (p[1] - pt[1]).abs() < 1e-6 && (p[2] - pt[2]).abs() < 1e-6);
        if !on_picked {
            continue;
        }
        // THE NAME IS ASKED FOR, NOT CREATED: drawing has no business adding to the document's table.
        let own = pn.project.names.vertex_desc(&name).is_some_and(|d| pn.cmd.params.iter().any(|p| p.key == format!("at{d}")));
        let sc = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(pt).0;
        let (r, col) = if own { (4.5, pn.scheme.pal.active()) } else { (3.0, pn.scheme.pal.handle_face()) };
        painter.circle_filled(sc, r, col);
    }
}

/// THE FACES THE COMMAND WILL ADD, as its trial built them, in the "will be added" colour over the part - the surface of
/// a fillet, a chamfer, the walls of a hole or of a shell - once the trial has answered that the value builds.
pub(crate) fn draw_trial_faces(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let Some(faces) = qymcad_ui_state::trial_faces(painter.ctx()) else { return };
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let col = qymcad_scheme::a(pn.scheme.pal.add(), 110);
    let mut hm = egui::Mesh::default();
    for t in faces.iter() {
        let base = hm.vertices.len() as u32;
        for v in t {
            hm.colored_vertex(scr.at(*v).0, col);
        }
        hm.add_triangle(base, base + 1, base + 2);
    }
    painter.add(egui::Shape::mesh(hm));
}

pub fn draw_feat_cmd_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    if pn.armed.commanding() && pn.mode_3d {
        draw_trial_faces(pn, painter, rect);
    }
    if pn.armed.cmd_kind() == 8 && pn.mode_3d {
        draw_sweep_preview(pn, painter, rect);
        return;
    }
    if pn.armed.cmd_kind() == 9 && pn.mode_3d {
        draw_loft_preview(pn.cam, pn.loft, pn.project, pn.scheme, pn.set, painter, rect);
        return;
    }
    if pn.armed.cmd_kind() == 24 && pn.mode_3d {
        draw_thread_preview(pn, painter, rect);
        return;
    }
    if matches!(pn.armed.cmd_kind(), 4 | 5) && pn.mode_3d {
        draw_edge_blend_preview(pn, painter, rect);
        return;
    }
    if pn.armed.cmd_kind() == 7 && pn.mode_3d {
        draw_hole_preview(pn, painter, rect);
        return;
    }
    if pn.armed.cmd_kind() == 23 && pn.mode_3d {
        draw_draft_preview(pn, painter, rect);
        return;
    }
    // THE CONTOURS A REVOLUTION TAKES are outlined where they lie: a pick is lit, as it is in the professional systems,
    // and a click that lets one go takes its outline away
    if pn.armed.cmd_kind() == 3 && pn.mode_3d {
        if let (Some(si), false) = (pn.cmd.sketch, pn.gsel.profiles.is_empty()) {
            if let Some(f) = pn.project.sketch_frame(si) {
                let basis = pn.cam.basis();
                let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
                let col = pn.scheme.pal.preview();
                // THE TURN FOLLOWS THE ANGLE: every corner of the profile traces its arc about the same axis the
                // rebuild turns about, from the same start (symmetric -> -angle/2, flipped -> -angle), and the profile
                // is drawn again where the turn ends
                let angle = qymcad_ui_state::cmd_val(pn.cmd, "angle");
                let sid = pn.project.sketches[si].id;
                let rev = &pn.rev;
                let (ax_o, ax_d, _) = pn.project.revolve_axis_local(sid, qymcad_core::model::RevolveAxis { axis: rev.axis, datum: rev.axis_datum, line: rev.axis_line });
                let theta0 = match qymcad_ui_state::cmd_reach(pn.cmd, pn.feat) {
                    qymcad_core::feature::Reach::BothWays => -angle / 2.0,
                    qymcad_core::feature::Reach::Backward => -angle,
                    qymcad_core::feature::Reach::Forward => 0.0,
                };
                let pl = f.matrix12();
                let turned = |x: f64, y: f64, deg: f64| {
                    let local = qymcad_core::feature::apply12(&qymcad_core::feature::rot12_axis(ax_o, ax_d, deg), [x, y, 0.0]);
                    scr.at(qymcad_core::feature::apply12(&pl, local)).0
                };
                let steps = ((angle.abs() / 5.0).ceil() as usize).clamp(2, 72);
                let ghost = qymcad_scheme::a(col, 150);
                for cid in &pn.gsel.profiles {
                    let Some(xy) = pn.project.contour_profile_xy(*cid) else { continue };
                    let m = xy.len() / 2;
                    let at = |k: usize| {
                        let p = f.lift(Point2::new(xy[2 * k], xy[2 * k + 1]));
                        scr.at([p.x, p.y, p.z]).0
                    };
                    for k in 0..m {
                        painter.line_segment([at(k), at((k + 1) % m)], Stroke::new(2.5, col));
                    }
                    if angle.abs() < 1e-9 {
                        continue;
                    }
                    for k in 0..m {
                        let arc: Vec<Pos2> = (0..=steps).map(|i| turned(xy[2 * k], xy[2 * k + 1], theta0 + angle * i as f64 / steps as f64)).collect();
                        painter.add(egui::Shape::line(arc, Stroke::new(1.0, ghost)));
                        let (a, b) = (turned(xy[2 * k], xy[2 * k + 1], theta0 + angle), turned(xy[2 * ((k + 1) % m)], xy[2 * ((k + 1) % m) + 1], theta0 + angle));
                        painter.line_segment([a, b], Stroke::new(1.5, ghost));
                    }
                }
            }
        }
        return;
    }
    if pn.armed.cmd_kind() == 0 || pn.gsel.profiles.is_empty() || !pn.mode_3d {
        return;
    }
    let Some(si) = pn.cmd.sketch else { return };
    let Some(f) = pn.project.sketch_frame(si) else { return };
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let n = f.normal();
    let h = qymcad_ui_state::cmd_val(pn.cmd, "height");
    // the preview extent is EXACTLY the one the rebuild uses: direction/flip/symmetry/two sides. The
    // distances come from the expression fields at the geometry (cmd_val), with no lag, so preview = result.
    let down = if pn.cmd.extent.two_sided() { qymcad_ui_state::cmd_val(pn.cmd, "down").abs() } else { 0.0 };
    let (start, total) = qymcad_core::feature::extrude_extent(h, down, qymcad_ui_state::cmd_reach(pn.cmd, pn.feat));
    let col = qymcad_scheme::a(pn.scheme.pal.preview(), 220);
    // THE PICKED REGIONS FILLED ON THE SKETCH'S PLANE, where the extrusion starts, as on the flat sheet
    for cid in &pn.gsel.profiles {
        let base = |p: Point2| {
            let w = f.lift(p);
            scr.at([w.x + n[0] * start, w.y + n[1] * start, w.z + n[2] * start]).0
        };
        fill_region(painter, &region_triangles(pn.project, *cid), base, qymcad_scheme::a(pn.scheme.pal.contour_profile(), 100));
    }
    for cid in &pn.gsel.profiles {
        let Some(xy) = pn.project.contour_profile_xy(*cid) else { continue };
        let m = xy.len() / 2;
        if m < 2 {
            continue;
        }
        let liftp = |k: usize, off: f64| -> Pos2 {
            let p = f.lift(Point2::new(xy[2 * k], xy[2 * k + 1]));
            scr.at([p.x + n[0] * off, p.y + n[1] * off, p.z + n[2] * off]).0
        };
        for k in 0..m {
            let k2 = (k + 1) % m;
            painter.line_segment([liftp(k, start), liftp(k2, start)], Stroke::new(1.5, col));
            painter.line_segment([liftp(k, start + total), liftp(k2, start + total)], Stroke::new(1.5, col));
            painter.line_segment([liftp(k, start), liftp(k, start + total)], Stroke::new(1.0, col));
        }
    }
    // every arrow of the command, from the same geometry the press takes them by: the first side along the direction
    // in force (flip -> the negated normal), so preview = result, and the second side the other way
    for arrow in qymcad_ui_state::feat_cmd_arrows(pn.cmd, pn.gsel, pn.project, pn.feat.flip) {
        let s0 = scr.at(arrow.base).0;
        let s1 = scr.at(arrow.tip()).0;
        let acol = if pn.cmd.drag == Some(arrow.key) { pn.scheme.pal.active() } else { pn.scheme.pal.handle() };
        painter.line_segment([s0, s1], Stroke::new(2.5, acol));
        let d = s1 - s0;
        let len = d.length().max(1.0);
        let u = d / len;
        let perp = egui::vec2(-u.y, u.x);
        painter.add(egui::Shape::convex_polygon(vec![s1, s1 - u * 12.0 + perp * 5.0, s1 - u * 12.0 - perp * 5.0], acol, Stroke::NONE));
        painter.circle_filled(s1, 5.0, acol);
    }
}

/// Draw the live sweep preview (the path + the carried profile sections + the longitudinal edges).
pub fn draw_sweep_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let Some(qymcad_ui_state::SweepPreview { path, sections }) = qymcad_ui_state::sweep_preview(pn.project, pn.sweep) else { return };
    let basis = pn.cam.basis();
    let sp = |p: [f64; 3]| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(p).0;
    // the path is a bright line
    let pcol = pn.scheme.pal.active();
    for w in path.windows(2) {
        painter.line_segment([sp(w[0]), sp(w[1])], Stroke::new(2.0, pcol));
    }
    // the profile sections (closed loops) + the longitudinal edges between neighbouring stations
    let scol = qymcad_scheme::a(pn.scheme.pal.preview(), 220);
    for sec in &sections {
        let m = sec.len();
        for k in 0..m {
            painter.line_segment([sp(sec[k]), sp(sec[(k + 1) % m])], Stroke::new(1.5, scol));
        }
    }
    for pair in sections.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        for k in 0..a.len().min(b.len()) {
            painter.line_segment([sp(a[k]), sp(b[k])], Stroke::new(1.0, scol));
        }
    }
}
