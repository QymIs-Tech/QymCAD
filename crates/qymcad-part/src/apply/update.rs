//! A feature already in the timeline changed by its command: the datum and every other kind.

use qymcad_ui_state::Notice;
use super::*;

/// Update an existing DATUM with the parameters of the command (reopened by a double click): it edits
/// `project.planes`, `points` and `axes` plus the feature dims (the offset and the coordinates stay
/// parametric). Resolving happens in `qymcad_ui_state::regenerate_all` after the apply.
pub fn update_datum_feat(
    armed: &qymcad_ui_state::Armed,
    cmd: &qymcad_ui_state::FeatCommand,
    datum: &qymcad_ui_state::DatumCommand,
    edges: &qymcad_ui_state::EdgeCache,
    project: &mut Project,
    status: &mut qymcad_ui_state::Notice,
    fid: Id,
) -> Option<Id> {
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::{AxisDef, PlaneDef};
    match armed.cmd_kind() {
        20 => {
            let dist = qymcad_ui_state::cmd_val(cmd, "dist");
            let pi = project.planes.iter().position(|p| p.id == fid)?;
            match datum.plane_pick {
                Some(SketchPlane::World(bp)) => project.planes[pi].def = PlaneDef::OffsetBase { base: bp, dist },
                Some(SketchPlane::Face(body, key)) => project.planes[pi].def = PlaneDef::OffsetFace { body, face: key, dist },
                Some(SketchPlane::Datum(did)) => project.planes[pi].def = PlaneDef::OffsetPlane { plane: did, dist },
                _ => match &mut project.planes[pi].def {
                    PlaneDef::OffsetBase { dist: d, .. } | PlaneDef::OffsetFace { dist: d, .. } | PlaneDef::OffsetPlane { dist: d, .. } => *d = dist,
                    PlaneDef::Manual | PlaneDef::FaceGone | PlaneDef::PlaneGone => {}
                },
            }
            turn_datum_plane(cmd, project, fid);
            if let Err(e) = store_cmd_exprs(cmd, project, fid) {
                // the offset, the tilts and the turn stay parametric
                return say_formula_refused(status, &e);
            }
            Some(fid)
        }
        21 => {
            use qymcad_core::model::PointDef;
            let pi = project.datum_points.iter().position(|p| p.id == fid)?;
            if datum.pt_mode == 1 {
                // "at a vertex" mode: picking another vertex makes a new associative reference; otherwise
                // the previous one IS KEPT
                if let Some((body, edge, end, at)) = datum.pt_vert {
                    project.datum_points[pi].def = PointDef::AtVertex { body, edge, end };
                    project.datum_points[pi].at = at;
                }
                return Some(fid);
            }
            let at = [qymcad_ui_state::cmd_val(cmd, "x"), qymcad_ui_state::cmd_val(cmd, "y"), qymcad_ui_state::cmd_val(cmd, "z")];
            project.datum_points[pi].at = at;
            project.datum_points[pi].def = PointDef::Manual; // back to hand-typed coordinates
            if let Err(e) = store_cmd_exprs(cmd, project, fid) {
                // x, y and z stay parametric
                return say_formula_refused(status, &e);
            }
            Some(fid)
        }
        22 => {
            let ai = project.datum_axes.iter().position(|a| a.id == fid)?;
            match datum.axis_mode {
                1 => {
                    // editing the coordinates by hand IS A CHANGE OF DEFINITION to a manual one rather than
                    // a note written beside the parametric definition (that would create a second truth).
                    let o = [qymcad_ui_state::cmd_val(cmd, "ox"), qymcad_ui_state::cmd_val(cmd, "oy"), qymcad_ui_state::cmd_val(cmd, "oz")];
                    let dv = [qymcad_ui_state::cmd_val(cmd, "dx"), qymcad_ui_state::cmd_val(cmd, "dy"), qymcad_ui_state::cmd_val(cmd, "dz")];
                    project.datum_axes[ai].set_manual(o, dv);
                }
                2 => {
                    if datum.axis_pts.len() == 2 {
                        let (p0, p1) = (datum.axis_pts[0], datum.axis_pts[1]);
                        if p0.0 != 0 && p1.0 != 0 {
                            project.datum_axes[ai].def = AxisDef::TwoPoints { a: p0.0, b: p1.0 };
                        } else {
                            let d = [p1.1[0] - p0.1[0], p1.1[1] - p0.1[1], p1.1[2] - p0.1[2]];
                            project.datum_axes[ai].set_manual(p0.1, d);
                        }
                    }
                }
                // mode 0: a new reference (a click) replaces the definition; without a re-pick the existing
                // definition IS KEPT
                _ => match datum.axis_hit {
                    Some(qymcad_ui_state::AxisHit::Edge(i)) => {
                        if let Some(&(body, edge, _)) = edges.axes.get(i) {
                            project.datum_axes[ai].def = AxisDef::FromEdge { body, edge };
                        }
                    }
                    Some(qymcad_ui_state::AxisHit::Face(body, fid2)) => project.datum_axes[ai].def = AxisDef::FromFace { body, face: fid2 },
                    _ => {} // nothing was re-picked, so the definition stays as it is
                },
            }
            Some(fid)
        }
        _ => None,
    }
}

/// Update existing feature `fid` with the parameters of the active command (edit mode).
pub fn update_feat(pc: &mut qymcad_ui_state::PartCtx, fid: Id) -> Option<Id> {
    use qymcad_core::feature::FeatureKind;
    // THE DATUMS edit `project.planes`, `points` and `axes` plus the feature dims rather than
    // `node.kind`, so they take a separate path
    if matches!(pc.armed.cmd_kind(), 20..=22) {
        return update_datum_feat(pc.armed, pc.cmd, pc.datum, pc.edges, pc.project, pc.status, fid);
    }
    // take everything out of the command state in advance, to avoid borrowing self twice
    let (h, ang, r, dist, th, dia, dep) = (
        qymcad_ui_state::cmd_val(pc.cmd, "height"),
        qymcad_ui_state::cmd_val(pc.cmd, "angle"),
        qymcad_ui_state::cmd_val(pc.cmd, "radius"),
        qymcad_ui_state::cmd_val(pc.cmd, "dist"),
        qymcad_ui_state::cmd_val(pc.cmd, "thickness"),
        qymcad_ui_state::cmd_val(pc.cmd, "diameter"),
        qymcad_ui_state::cmd_val(pc.cmd, "depth"),
    );
    // the "vertex -> radius" table is taken BEFORE the node is edited (otherwise self is borrowed twice)
    let vtable: Vec<(qymcad_core::refs::Ref, f64)> =
        fillet_vertex_table(pc.cmd).into_iter().map(|(desc, val)| (qymcad_core::refs::Ref::one(desc, qymcad_core::refs::Fingerprint::default()), val)).collect();
    let dist_v = qymcad_ui_state::cmd_val(pc.cmd, "dist"); // "push a face": the offset, which may be an expression
    let (rec_tol, rec_sharp) = (qymcad_ui_state::cmd_val(pc.cmd, "tol"), qymcad_ui_state::cmd_val(pc.cmd, "sharp")); // the recognition's tolerance and sharp edge
    let ch_d2 = qymcad_ui_state::cmd_val(pc.cmd, "d2"); // the second leg or the angle of the chamfer, in degrees
    let (ch_mode, ch_flip) = (pc.chamfer.mode, pc.chamfer.flip); // the mode plus the side of the reference face
    let patch_tangent = pc.opts.patch_tangent; // a patch by position or smooth
    let (stitch_now, trim_now) = (pc.stitch_parts.clone(), (pc.trim.keep, pc.trim.tool)); // the sheets of a stitch; what a trim cuts, keeps and cuts with
    let surface_now = *pc.repl_surface; // the sheet a face replacement puts in
    let ch_ref_face = pc.chamfer.ref_face; // the hand-picked reference face (0 means automatic)
    let profile = pc.gsel.profiles.iter().copied().next().unwrap_or(0);
    // ALL the picked contours go into editing a multi-contour node. The edit puts them ALL into the
    // `profiles` of the node rather than a single contour.
    let (edit_contours, edit_fill): (Vec<Id>, Vec<Id>) = match pc.cmd.sketch.filter(|&si| si < pc.project.sketches.len()) {
        Some(si) => {
            // nesting is not filtered - every picked contour goes as a region of its own (see `apply`)
            (qymcad_ui_state::sketch_closed_contours(pc.project, si).into_iter().filter(|c| pc.gsel.profiles.contains(c)).collect(), Vec::new())
        }
        None => (Vec::new(), Vec::new()),
    };
    let edges: Vec<u32> = pc.gsel.edges.iter().copied().collect();
    let picked_edges = pc.gsel.recorded(&edges);
    let faces_set: Vec<u32> = pc.gsel.faces.iter().copied().collect(); // the shell: a multiple selection by id
    let shell_side = pc.opts.shell_side; // the shell: which way the wall goes
    let draft_neutral = pc.draft.neutral; // the draft: the neutral face, 0 means unset
    let draft_flip = pc.draft.flip; // the draft: the direction of the pull
                                    // the mirror: the picked plane resolves to (plane, datum, face)
    let mirror_resolved = pc.mirror.plane.map(qymcad_ui_state::resolve_mirror_plane);
    let mirror_keep = pc.opts.mirror_keep;
    // THE CUT: the new plane plus how many pieces it will yield. It is computed BEFORE the timeline is
    // borrowed; if it disagrees with the number of pieces the feature has, the edit is rejected below -
    // the bodies of the cut have already spread through the timeline (they are referred to and they are
    // visible), and their number cannot be changed in place.
    let (split_resolved, split_pieces) = if pc.armed.cmd_kind() == 27 || pc.armed.cmd_kind() == 29 {
        let n = (pc.armed.cmd_kind() == 27)
            .then(|| {
                qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)
                    .and_then(|src| split_piece_count(pc, src))
            })
            .flatten();
        let r = pc.split.plane.map(qymcad_ui_state::resolve_mirror_plane);
        (r, n)
    } else {
        (None, None)
    };
    let split_offset = qymcad_ui_state::cmd_val(pc.cmd, "offset");
    let thickness_v = qymcad_ui_state::cmd_val(pc.cmd, "thickness"); // the thickening
                                                                     // the array: gather the vector, the angle and the count BEFORE the timeline is borrowed
    let (a_dir, a_dir2, a_dir3, a_two, a_three, a_axis, a_full) = (pc.arr.dir, pc.arr.dir2, pc.arr.dir3, pc.arr.two, pc.arr.three, pc.arr.axis, pc.arr.full);
    let (a_count, a_count2, a_count3) = (pc.arr.count.max(1), pc.arr.count2.max(1), pc.arr.count3.max(1));
    let (a_dx, a_dy, a_dz) = qymcad_ui_state::arr_vec(a_dir, qymcad_ui_state::cmd_val(pc.cmd, "step"));
    let (a_dx2, a_dy2, a_dz2) = if a_two { qymcad_ui_state::arr_vec(a_dir2, qymcad_ui_state::cmd_val(pc.cmd, "step2")) } else { (0.0, 0.0, 0.0) };
    let a_c2 = if a_two { a_count2 } else { 1 };
    let (a_dx3, a_dy3, a_dz3) = if a_two && a_three { qymcad_ui_state::arr_vec(a_dir3, qymcad_ui_state::cmd_val(pc.cmd, "step3")) } else { (0.0, 0.0, 0.0) };
    let a_c3 = if a_two && a_three { a_count3 } else { 1 };
    let a_angle = if a_full { 360.0 } else { qymcad_ui_state::cmd_val(pc.cmd, "angle") };
    let (a_step_txt, a_step2_txt, a_step3_txt) = (cmd_txt(pc.cmd, "step"), cmd_txt(pc.cmd, "step2"), cmd_txt(pc.cmd, "step3"));
    // the primitives: values under the same keys the regen uses (radii)
    let (p_dx, p_dy, p_dz) = (qymcad_ui_state::cmd_val(pc.cmd, "dx"), qymcad_ui_state::cmd_val(pc.cmd, "dy"), qymcad_ui_state::cmd_val(pc.cmd, "dz"));
    let (p_r, p_r1, p_r2, p_ph, p_major, p_minor) = (
        qymcad_ui_state::cmd_val(pc.cmd, "r"),
        qymcad_ui_state::cmd_val(pc.cmd, "r1"),
        qymcad_ui_state::cmd_val(pc.cmd, "r2"),
        qymcad_ui_state::cmd_val(pc.cmd, "h"),
        qymcad_ui_state::cmd_val(pc.cmd, "major"),
        qymcad_ui_state::cmd_val(pc.cmd, "minor"),
    );
    let p_n = pc.prim.n.max(3);
    // if ANOTHER face is picked while editing, it is updated on the feature. Only for the hole: the face
    // goes in by its persistent `FaceKey` (the shell is now a multiple selection by id - `faces_set`
    // above)
    let hole_face: Option<(qymcad_core::feature::FaceKey, [f64; 3], [f64; 3])> = match *pc.sel {
        qymcad_ui_state::Sel::Face(mi, fi) => pc.project.bodies.get(mi).and_then(|b| b.faces.get(fi)).map(|face| {
            let k = qymcad_core::feature::FaceKey { index: fi as u32, centroid: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, id: face.id };
            (k, hole_centre(k.centroid, k.normal, qymcad_ui_state::cmd_val(pc.cmd, "off_u"), qymcad_ui_state::cmd_val(pc.cmd, "off_v")), k.normal)
        }),
        _ => None,
    };
    let reach = qymcad_ui_state::cmd_reach(pc.cmd, *pc.feat);
    let down = if pc.cmd.extent.two_sided() { qymcad_ui_state::cmd_val(pc.cmd, "down").abs() } else { 0.0 };
    let through = pc.cmd.extent.through();
    let occt = match pc.feat.op {
        1 => 1,
        3 => 2,
        _ => 0,
    };
    let axis = pc.rev.axis;
    let axis_datum_rev = pc.rev.axis_datum; // K4
    let axis_line_rev = pc.rev.axis_line; // 64
    let hole_kind = pc.hole.kind; // F1
    let hole_flip = pc.hole.flip; // the direction in "by sketch" mode
    let (hole_dia2, hole_depth2) = if pc.hole.kind != 0 { (qymcad_ui_state::cmd_val(pc.cmd, "dia2"), qymcad_ui_state::cmd_val(pc.cmd, "depth2")) } else { (0.0, 0.0) };
    // THE THREAD: the values of the parameters and options BEFORE the timeline is borrowed (the angle and
    // the depth have already been taken)
    let (t_pitch, t_length) = (qymcad_ui_state::cmd_val(pc.cmd, "pitch"), qymcad_ui_state::cmd_val(pc.cmd, "length"));
    let (t_starts, t_left) = (pc.thread.starts.max(1), pc.thread.left);
    let t_spec = thread_spec(pc.cmd, *pc.thread); // the whole record, so an edit and an apply cannot differ
    let (t_outer, t_thickness, t_edge_r) = (qymcad_ui_state::cmd_val(pc.cmd, "outer"), qymcad_ui_state::cmd_val(pc.cmd, "thickness"), qymcad_ui_state::cmd_val(pc.cmd, "edge_r")); // the auger
    let (t_lead_in, t_lead_out) = (qymcad_ui_state::cmd_val(pc.cmd, "lead_in"), qymcad_ui_state::cmd_val(pc.cmd, "lead_out"));
    let (sw_prof, sw_path, sw_pcid, sw_hcid) = (pc.sweep.prof_sid, pc.sweep.path_sid, pc.sweep.prof_cid, pc.sweep.path_cid); // captured before the mutable borrow
    let (lf_sids, lf_cids, lf_ruled) = (pc.loft.sids.clone(), pc.loft.cids.clone(), pc.loft.ruled); // the loft, captured before the mutable borrow
                                                                                                    // the kind of loft result maps to (src, op). For a boolean the target is the active body of the
                                                                                                    // context. When EDITING a loft that is already a boolean, the previous target is kept (`active_body`
                                                                                                    // would skip it as consumed); switching from "new body" to "cut" takes the active body.
    let lf_prev_src = pc.project.timeline.iter().find(|n| n.id == fid).and_then(|n| match n.kind {
        FeatureKind::Loft { src, .. } => Some(src),
        _ => None,
    });
    let (lf_src, lf_op): (Id, u8) = if pc.loft.result == 0 {
        (0, 0)
    } else {
        let src = match lf_prev_src {
            Some(s) if s != 0 => s, // already a boolean - do not change the target body
            _ => qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }).unwrap_or(0),
        };
        (src, pc.loft.result - 1)
    };
    // THE CUT rejects the edit if the plane now divides the body into a different number of pieces
    let mut bad_pieces: Option<(usize, usize)> = None;
    let found = if let Some(node) = pc.project.timeline.iter_mut().find(|n| n.id == fid) {
        match &mut node.kind {
            FeatureKind::Loft { sketches, contours, ruled, src, op, .. } => {
                // update the set of sections and contours, the kind of faces and the kind of result (two
                // or more sections are required)
                if lf_sids.len() >= 2 {
                    *sketches = lf_sids.clone();
                    *contours = lf_cids.clone();
                }
                *ruled = lf_ruled;
                *src = lf_src; // 0 means a separate body; otherwise a boolean with the active or previous body
                *op = lf_op;
            }
            FeatureKind::Sweep { sketch, profiles, path_sketch, path, .. } => {
                // update the profile, the path and the picked contours; either sketch may be changed
                if sw_prof != 0 {
                    *sketch = sw_prof;
                }
                if sw_path != 0 {
                    *path_sketch = sw_path;
                }
                *profiles = if !edit_contours.is_empty() {
                    edit_contours.clone()
                } else if sw_pcid != 0 {
                    vec![sw_pcid]
                } else {
                    Vec::new()
                };
                *path = sw_hcid;
            }
            FeatureKind::Extrude { profiles: prs, height, reach: rch, down: dn, fill: fl_field, .. } => {
                // ALL the picked contours go into the node (a multi-contour feature is edited whole)
                if !edit_contours.is_empty() {
                    *prs = edit_contours.clone();
                    *fl_field = edit_fill.clone();
                } else if profile != 0 {
                    *prs = vec![profile];
                }
                *height = h;
                *rch = reach;
                *dn = down;
            }
            FeatureKind::Combine { profiles: prs, height, op, extent: ext, down: dnc, fill: fl_field, .. } => {
                if !edit_contours.is_empty() {
                    *prs = edit_contours.clone();
                    *fl_field = edit_fill.clone();
                } else if profile != 0 {
                    *prs = vec![profile];
                }
                *height = h.abs(); // a positive MAGNITUDE; the direction comes from flip, symmetry or the two sides
                *op = occt;
                *ext = qymcad_core::feature::Extent { through, reach };
                *dnc = down;
            }
            FeatureKind::Revolve { profiles: prs, axis: ax, angle, axis_datum, axis_line: al, reach: rch, .. } => {
                // the edit puts ALL the picked contours into the node rather than the first one to hand
                if !edit_contours.is_empty() {
                    *prs = edit_contours.clone();
                } else if profile != 0 {
                    *prs = vec![profile];
                }
                *ax = axis;
                *angle = ang;
                *axis_datum = axis_datum_rev; // K4
                *al = axis_line_rev; // the centre line of the sketch (0 means a datum or X/Y)
                *rch = reach;
            }
            FeatureKind::Fillet { radius, edges: e, at_vertices, .. } => {
                *radius = r;
                *e = picked_edges.clone();
                *at_vertices = vtable; // the "vertex -> radius" table
            }
            FeatureKind::Chamfer { dist: d, edges: e, mode, d2, flip, ref_face, .. } => {
                *d = dist;
                *e = if ch_mode != qymcad_core::feature::ChamferMode::Symmetric && !edges.is_empty() { qymcad_core::refs::Ref::picks(&edges) } else { picked_edges.clone() };
                *mode = ch_mode; // the mode: symmetric, two distances, or a leg plus an angle
                *d2 = ch_d2;
                *flip = ch_flip;
                *ref_face = ch_ref_face; // the hand-picked reference face
            }
            FeatureKind::Shell { thickness, faces, side, .. } => {
                *thickness = th;
                if !faces_set.is_empty() {
                    *faces = qymcad_core::refs::Ref::picks(&faces_set); // a multiple face selection; a hand pick is a query of ids
                }
                *side = shell_side; // which way the wall goes
            }
            FeatureKind::MeshRecognised { tol, sharp, .. } => {
                *tol = rec_tol;
                *sharp = rec_sharp;
            }
            FeatureKind::Thicken { face, thickness, .. } => {
                *thickness = thickness_v;
                if let Some(&id) = faces_set.first() {
                    *face = id; // the face can be reassigned by a click, without recreating the feature
                }
            }
            FeatureKind::SplitFace { plane, datum, offset, face, .. } => {
                if let Some((pl, dt, fc)) = split_resolved {
                    *plane = pl;
                    *datum = dt;
                    *face = fc;
                }
                *offset = split_offset;
            }
            FeatureKind::SplitBody { plane, datum, offset, bodies, face, .. } => {
                // THE NUMBER OF PIECES MUST NOT CHANGE: later features may already refer to the existing
                // bodies, and creating or removing a body through an edit would break those references
                // silently. An honest refusal leaves the cut to be recreated by hand.
                if let Some(n) = split_pieces {
                    if n != bodies.len() {
                        bad_pieces = Some((n, bodies.len()));
                    }
                }
                if bad_pieces.is_none() {
                    if let Some((pl, dt, fc)) = split_resolved {
                        *plane = pl;
                        *datum = dt;
                        *face = fc;
                    }
                    *offset = split_offset;
                }
            }
            FeatureKind::Patch { edges: e, tangent, .. } => {
                if !edges.is_empty() {
                    *e = picked_edges.clone();
                }
                *tangent = patch_tangent;
            }
            FeatureKind::FaceCopy { faces, .. } => {
                if !faces_set.is_empty() {
                    *faces = qymcad_core::refs::Ref::picks(&faces_set.to_vec());
                }
            }
            FeatureKind::OffsetSurface { faces, dist, .. } => {
                *dist = dist_v;
                if !faces_set.is_empty() {
                    *faces = qymcad_core::refs::Ref::picks(&faces_set.to_vec());
                }
            }
            FeatureKind::Stitch { parts, tol, .. } => {
                *tol = rec_tol;
                if stitch_now.len() >= 2 {
                    *parts = stitch_now.clone();
                }
            }
            FeatureKind::SurfaceReplace { faces, surface, .. } => {
                if !faces_set.is_empty() {
                    *faces = qymcad_core::refs::Ref::picks(&faces_set.to_vec());
                }
                if let Some(s) = surface_now {
                    *surface = s;
                }
            }
            FeatureKind::Trim { src, tool, keep, .. } => {
                if let (Some((s, k)), Some(t)) = trim_now {
                    (*src, *keep, *tool) = (s, k, t);
                }
            }
            FeatureKind::RemoveFace { faces, .. } => {
                if !faces_set.is_empty() {
                    // the set of faces is reassigned by clicks without recreating the feature; a hand-picked
                    // set is a query of `Id`s and is replaced whole rather than edited one by one
                    *faces = qymcad_core::refs::Ref::picks(&faces_set.to_vec());
                }
            }
            FeatureKind::PushFace { face, dist, .. } => {
                *dist = dist_v;
                if let Some(&id) = faces_set.first() {
                    *face = qymcad_core::refs::Ref::one(id, face.hint); // the face is reassigned by a click
                }
            }
            FeatureKind::Draft { faces, neutral, angle, flip, .. } => {
                // update the set of tilted faces (by id), the neutral face, the angle and the direction
                if !faces_set.is_empty() {
                    *faces = qymcad_core::refs::Ref::picks(&faces_set);
                }
                if draft_neutral != 0 {
                    *neutral = qymcad_core::refs::Ref::one(draft_neutral, neutral.hint);
                }
                *angle = ang;
                *flip = draft_flip;
            }
            FeatureKind::Hole { face, point, normal, diameter, depth, kind, dia2, depth2, flip, .. } => {
                *diameter = dia;
                *depth = dep;
                *kind = hole_kind; // F1
                *dia2 = hole_dia2;
                *depth2 = hole_depth2;
                *flip = hole_flip; // the drilling direction in "by sketch" mode
                if let Some((k, p, nrm)) = hole_face {
                    // editing the face recomputes the reference to it along with the centre and the normal
                    // of the hole. A hand pick is a query of `Id`: one particular face was pointed at.
                    *face = qymcad_core::refs::Ref::one(k.id, qymcad_core::refs::Fingerprint { centroid: k.centroid, normal: k.normal });
                    *point = p;
                    *normal = nrm;
                }
            }
            FeatureKind::Thread { spec, length, lead_in, lead_out, .. } => {
                // editing a thread means the standard and the size (the core computes the geometry)
                *spec = t_spec;
                *length = t_length;
                *lead_in = t_lead_in;
                *lead_out = t_lead_out;
            }
            FeatureKind::Auger { spec, length, lead_in, lead_out, .. } => {
                // editing an auger (the shaft is taken from the geometry during the regen)
                spec.outer_d = t_outer;
                spec.pitch = t_pitch;
                spec.thickness = t_thickness;
                spec.edge_r = t_edge_r;
                spec.starts = t_starts;
                spec.left = t_left;
                *length = t_length;
                *lead_in = t_lead_in;
                *lead_out = t_lead_out;
            }
            FeatureKind::Mirror { plane, keep, datum, face, .. } => {
                *keep = mirror_keep;
                if let Some((pl, dt, fc)) = mirror_resolved {
                    *plane = pl;
                    *datum = dt;
                    *face = fc;
                }
            }
            FeatureKind::LinearArray { dx, dy, dz, count, dx2, dy2, dz2, count2, dx3, dy3, dz3, count3, .. } => {
                *dx = a_dx;
                *dy = a_dy;
                *dz = a_dz;
                *count = a_count;
                *dx2 = a_dx2;
                *dy2 = a_dy2;
                *dz2 = a_dz2;
                *count2 = a_c2;
                *dx3 = a_dx3;
                *dy3 = a_dy3;
                *dz3 = a_dz3;
                *count3 = a_c3;
            }
            FeatureKind::CircularArray { count, angle, axis, .. } => {
                *count = a_count;
                *angle = a_angle;
                *axis = a_axis;
            }
            // the primitives: the sizes are written back on reopening (the keys are the regen's keys)
            FeatureKind::Box3 { dx, dy, dz, .. } => {
                *dx = p_dx;
                *dy = p_dy;
                *dz = p_dz;
            }
            FeatureKind::Cylinder { r, h, .. } => {
                *r = p_r;
                *h = p_ph;
            }
            FeatureKind::Sphere { r, .. } => {
                *r = p_r;
            }
            FeatureKind::Cone { r1, r2, h, .. } => {
                *r1 = p_r1;
                *r2 = p_r2;
                *h = p_ph;
            }
            FeatureKind::Torus { major, minor, .. } => {
                *major = p_major;
                *minor = p_minor;
            }
            FeatureKind::Prism { r, n, h, .. } => {
                *r = p_r;
                *n = p_n;
                *h = p_ph;
            }
            _ => return None,
        }
        node.dirty = true;
        true
    } else {
        false
    };
    if !found {
        return None;
    }
    if let Some((got, had)) = bad_pieces {
        // the feature was left untouched - the edit was not applied AT ALL rather than half applied
        *pc.status = Notice::warning(qymcad_i18n::tr2("cmd-split-count-changed", "got", &got.to_string(), "had", &had.to_string()));
        return None;
    }
    if let Err(e) = store_cmd_exprs(pc.cmd, pc.project, fid) {
        return formula_refused(pc, &e);
    } // store or clear the dimension expressions (the logical step, step2 and angle included)
    if matches!(pc.armed.cmd_kind(), 17 | 18) {
        if let Err(e) = store_bar_counts(pc.bar_exprs, pc.project, fid) {
            return formula_refused(pc, &e);
        } // the counts typed in the bar, as the steps are
    }
    // the array: the step expressions go onto the components of the vector (the regen reads those), and the
    // angle is cleared for a full circle
    match pc.armed.cmd_kind() {
        17 => {
            if let Err(e) = qymcad_ui_state::store_arr_component(pc.project, fid, ["dx", "dy", "dz"], a_dir, a_step_txt.clone()) {
                return formula_refused(pc, &e);
            }
            if a_two {
                if let Err(e) = qymcad_ui_state::store_arr_component(pc.project, fid, ["dx2", "dy2", "dz2"], a_dir2, a_step2_txt.clone()) {
                    return formula_refused(pc, &e);
                }
            } else {
                for k in ["dx2", "dy2", "dz2"] {
                    pc.project.clear_feat_formula(&qymcad_core::model::FeatNumber { node: fid, key: k });
                }
            }
            if a_two && a_three {
                if let Err(e) = qymcad_ui_state::store_arr_component(pc.project, fid, ["dx3", "dy3", "dz3"], a_dir3, a_step3_txt.clone()) {
                    return formula_refused(pc, &e);
                }
            } else {
                for k in ["dx3", "dy3", "dz3"] {
                    pc.project.clear_feat_formula(&qymcad_core::model::FeatNumber { node: fid, key: k });
                }
            }
        }
        18 if a_full => {
            pc.project.clear_feat_formula(&qymcad_core::model::FeatNumber { node: fid, key: "angle" });
        }
        _ => {}
    }
    // the step opened and applied again is the person's word on what it takes: a mark of a reference found by place goes
    pc.project.accept_found_by_place(fid);
    Some(fid)
}
