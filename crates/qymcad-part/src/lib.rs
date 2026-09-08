//! THE PART WORKBENCH: the feature commands.
//!
//! Extrude, revolve, sweep, loft, fillet, chamfer, shell, the array, the thread, the hole, the split -
//! their option bars, their popups at the geometry, their previews and what they do to the document. Every
//! function here works over `qymcad_ui_state::PartCtx` and asks the application for nothing.

use egui::{Pos2, Rect};
use egui_phosphor::regular as ph;
use qymcad_core::model::{ArrayAxis, Id, Project};
use qymcad_ui_state::WinKind;


/// THE DESCRIPTOR OF A REFERENCE, IF IT IS A MANUAL PICK.
///
/// The highlight of a selected face in a command can show one particular face. A query of the sort "every
/// wall of this feature" gives it nothing to show - so it returns `None` rather than lying with a
/// highlight.
fn face_desc_of(r: &qymcad_core::refs::Ref) -> Option<u32> {
    match r.query {
        qymcad_core::refs::Query::Id(d) if d != 0 => Some(d),
        _ => None,
    }
}

/// The axis (0 for X, 1 for Y, 2 for Z) from a vector - for reopening an array to edit it.
pub fn axis_of(d: [f64; 3]) -> u8 {
    let (ax, ay, az) = (d[0].abs(), d[1].abs(), d[2].abs());
    if az >= ax && az >= ay {
        2
    } else if ay >= ax {
        1
    } else {
        0
    }
}

/// A SENSIBLE DEFAULT FOR THE DIRECTION: a cut from a sketch ON A FACE goes INTO the body (the negative
/// normal), otherwise it would cut the void. It is a derived value, so it is computed FROM THE CURRENT
/// parameters of the command rather than remembered when the command opens: the operation is chosen in
/// the bar, after opening.
pub fn smart_flip(cmd: u8, op: u8, plane: &qymcad_core::feature::SketchPlane) -> bool {
    cmd == 1 && op == 2 && matches!(plane, qymcad_core::feature::SketchPlane::Face(..))
}

/// A command parameter taken from a feature dimension: the text is the expression if there is one, or
/// the number; the value is evaluated.
pub fn cmd_param_from(project: &qymcad_core::model::Project, fid: Id, label: &'static str, key: &str, num: f64, lo: f64, hi: f64) -> qymcad_ui_state::CmdParam {
    // A NUMBER IN A FIELD GOES THROUGH THE COMMON DOOR. This used to be `{num:.2}`: two decimals in one
    // field, three in another, the whole truth about an f64 in a third. There is one rule for the whole
    // project.
    let txt = project.feat_dim(fid, key).map(|s| s.to_string()).unwrap_or_else(|| qymcad_core::expr::fmt_num(num));
    let val = qymcad_core::expr::eval(&txt, &project.param_map()).unwrap_or(num);
    let mut p = qymcad_ui_state::CmdParam::new(label, key, num, lo, hi);
    p.val = val;
    p.txt = txt;
    p
}

/// Reopen a command on an EXISTING feature (a double click in the tree). It loads the dimensions, the
/// expressions and the selection of the feature into the command; Enter then updates it in place
/// (`update_feat`).
///
/// ONLY LIVE REFERENCES - what is on screen NOW and can be picked.
///
/// Reported: a chamfer on four edges, then the sketch was rebuilt and the chamfer went red; a double
/// click, four edges picked again, Enter - and the feature then showed eight edges.
///
/// Editing used to raise ALL of the previous descriptors into the selection, including those that no
/// longer resolve: they cannot be highlighted (there is no geometry) and cannot be removed (there is
/// nothing to click), and on writing they went into the feature together with the new ones. The program
/// did something other than what was asked, and said so only through a number in the tree.
///
/// The rule is the same for ALL features with references, which is why it lives in one method: split it
/// across the branches and it will drift.
pub fn live_picks(project: &qymcad_core::model::Project, body: Id, r: &qymcad_core::refs::Ref, faces: bool) -> std::collections::HashSet<u32> {
    let live: std::collections::HashSet<u32> = if faces {
        project.regen_faces.get(&body).map(|fs| fs.iter().map(|f| f.id).collect()).unwrap_or_default()
    } else {
        project.regen_edges.get(&body).map(|es| es.iter().map(|e| e.id).collect()).unwrap_or_default()
    };
    // THERE MAY BE NO LIVE GEOMETRY AT ALL (the cache is not built yet) - then there is nothing to
    // discard and nothing to judge by: it is handed back as it is, otherwise editing would lose honest
    // references for no reason.
    let picked = r.query.picked_descs();
    if live.is_empty() {
        return picked.into_iter().collect();
    }
    picked.into_iter().filter(|d| live.contains(d)).collect()
}

/// The raw text of a command field (an expression or a number), by key.
pub fn cmd_txt(cmd: &qymcad_ui_state::FeatCommand, key: &str) -> String {
    cmd.params.iter().find(|p| p.key == key).map(|p| p.txt.clone()).unwrap_or_default()
}

/// THE "VERTEX -> RADIUS" TABLE, built from the fields of the command.
///
/// The fields live by the ordinary dimension mechanism - with expressions, parametrics and Enter/Esc -
/// while which place a field belongs to is stated in its key: `at{vertex descriptor}`. No second store
/// had to be created for this.
pub fn fillet_vertex_table(cmd: &qymcad_ui_state::FeatCommand) -> Vec<(u32, f64)> {
    cmd
        .params
        .iter()
        .filter_map(|p| p.key.strip_prefix("at").and_then(|d| d.parse::<u32>().ok()).map(|desc| (desc, p.val)))
        .filter(|(_, v)| *v > 1e-9)
        .collect()
}

/// Apply the SWEEP: the profile (`sweep_prof_sid`) along the path (`sweep_path_sid`). Both sketches are
/// entered into the timeline if they are not there yet, then the Sweep node follows. The contours come
/// from the selection in the bar (`sweep_prof_cid` and `sweep_path_cid`; 0 means the first suitable one
/// is taken automatically).
pub fn apply_sweep_cmd(feat: qymcad_ui_state::FeatTarget, project: &mut Project, status: &mut String, sweep: qymcad_ui_state::SweepParams) -> Option<Id> {
    if sweep.prof_sid == 0 || sweep.path_sid == 0 {
        *status = qymcad_i18n::tr("msg-need-profile-path");
        return None;
    }
    if sweep.prof_sid == sweep.path_sid {
        *status = qymcad_i18n::tr("msg-profile-path-differ");
        return None;
    }
    let prof_name = project.sketches.iter().find(|s| s.id == sweep.prof_sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_default();
    let path_name = project.sketches.iter().find(|s| s.id == sweep.path_sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_default();
    project.add_sketch_node(sweep.prof_sid, prof_name);
    project.add_sketch_node(sweep.path_sid, path_name);
    // the profile contours and the boolean go IN ONE node (see the revolve: the reason is the same).
    let profiles: Vec<Id> = if sweep.prof_cid != 0 { vec![sweep.prof_cid] } else { Vec::new() };
    // a sweep carries AN OPERATION: add (0) becomes a pad, plus cut (2) and intersect (3)
    let occt = match feat.op {
        2 => 0,
        3 => 2,
        _ => 1,
    };
    let part = project.sketch_owner(sweep.prof_sid).and_then(|o| project.active_body(o)).unwrap_or(0);
    let (src, op) = if part != 0 && matches!(feat.op, 1..=3) { (part, occt) } else { (0, 1) };
    let body = project.add_sweep_multi_op(sweep.prof_sid, profiles, sweep.path_sid, sweep.path_cid, src, op);
    (body != 0).then(|| if src == 0 { project.finish_base_body(body, 1) } else { body })
}

/// The name of the operation for the undo step - the same one shown on the button.
pub fn feat_cmd_name(armed: &qymcad_ui_state::Armed, feat: qymcad_ui_state::FeatTarget) -> String {
    match armed.cmd_kind() {
        1 => match feat.op {
            2 => qymcad_i18n::tr("f-cut"),
            3 => qymcad_i18n::tr("f-intersection"),
            _ => qymcad_i18n::tr("f-extrusion"),
        },
        3 => qymcad_i18n::tr("f-revolution"),
        4 => qymcad_i18n::tr("f-fillet"),
        5 => qymcad_i18n::tr("f-chamfer"),
        6 => qymcad_i18n::tr("f-shell"),
        8 => qymcad_i18n::tr("f-sweep"),
        9 => qymcad_i18n::tr("f-loft"),
        16 => qymcad_i18n::tr("f-mirror"),
        23 => qymcad_i18n::tr("f-draft"),
        25 => qymcad_i18n::tr("f-push-face"),
        26 => qymcad_i18n::tr("f-remove-face"),
        30 => qymcad_i18n::tr("f-face-copy"),
        31 => qymcad_i18n::tr("f-surface-replace"),
        32 => qymcad_i18n::tr("f-patch"),
        24 => qymcad_i18n::tr("f-thread"),
        _ => qymcad_i18n::tr("f-operation"),
    }
}

/// A CUSTOM thread opens two fields - the profile angle and the depth of the groove; every other
/// standard removes them. This is called when THE STANDARD CHANGES: the fields used to be added only in
/// `qymcad_ui_state::set_thread_params`, that is, when a face was picked, so switching to a custom thread showed nothing
/// and the choice had no effect at all. The values already typed are preserved.
pub fn sync_custom_params(cmd: &mut qymcad_ui_state::FeatCommand, thread: qymcad_ui_state::ThreadParams) {
    let custom = !thread.auger && qymcad_ui_state::thread_standard(thread.form) == qymcad_core::thread::ThreadStandard::Custom;
    let has = cmd.params.iter().any(|p| p.key == "angle");
    if custom && !has {
        cmd.params.push(qymcad_ui_state::CmdParam::new("f-profile-angle", "angle", 60.0, 5.0, 170.0));
        // The depth opens on the one that will be cut, not on zero: zero here means "nothing was typed" and
        // the core cuts 0.6 of the pitch, so a field at 0.00 stood over a groove 1.50 deep.
        let depth = thread_spec(cmd, thread).geometry().depth;
        cmd.params.push(qymcad_ui_state::CmdParam::new("f-thread-depth", "depth", depth, 0.0, 1000.0));
    } else if !custom && has {
        cmd.params.retain(|p| p.key != "angle" && p.key != "depth");
    }
}

/// THE THREAD THE COMMAND DESCRIBES, in one place.
///
/// Three places built this record: applying the command, editing an existing feature, and the bar's own
/// line about the geometry and about what the mating part needs. The third built a record of its own and
/// filled the rest of the fields from `Default`. For the five standards taken from a table that costs
/// nothing - the fields left out are not part of their profile. `Custom` is the one standard whose depth
/// and angle come from the person, so exactly there the two numbers that mattered were dropped.
///
/// Reported behaviour: "a custom thread is broken when trying to make a bolt and a nut - that is, when you
/// set the profile and the angle yourself." Measured at Ø20 x 5 with a depth of 2.5 typed in: the nut needs
/// a hole of 15.00 mm and the bar named 14.00, having used `Default`'s 0.6 of the pitch. A nut bored to
/// what the bar says is a millimetre undersize and does not go on.
pub fn thread_spec(cmd: &qymcad_ui_state::FeatCommand, thread: qymcad_ui_state::ThreadParams) -> qymcad_core::thread::ThreadSpec {
    let v = |k: &str| qymcad_ui_state::cmd_val(cmd, k);
    qymcad_core::thread::ThreadSpec {
        standard: qymcad_ui_state::thread_standard(thread.form),
        // Before a cylinder is picked there is no size field yet and the bar still has to say something:
        // the size of whatever the command is standing on.
        nominal_d: if v("nominal") > 0.0 { v("nominal") } else { thread.radius * 2.0 },
        pitch: v("pitch"),
        starts: thread.starts.max(1),
        left: thread.left,
        internal: thread.internal,
        fit: v("fit"),
        crest_r: (v("crest_r") > 1e-9).then(|| v("crest_r")),
        root_r: (v("root_r") > 1e-9).then(|| v("root_r")),
        custom_depth: v("depth"),
        // A profile of one's own with no angle typed is the 60-degree V a table would give.
        custom_angle: if v("angle") > 1.0 { v("angle") } else { 60.0 },
    }
}

/// Apply the trim: the sheet with the "keep" point plus the tool.
pub fn apply_trim_cmd(project: &mut Project, status: &mut String, trim: &qymcad_ui_state::TrimTool) -> Option<Id> {
    let Some((src, keep)) = trim.keep else {
        *status = qymcad_i18n::tr("msg-trim-pick-sheet");
        return None;
    };
    let Some(tool) = trim.tool else {
        *status = qymcad_i18n::tr("msg-trim-pick-tool");
        return None;
    };
    Some(project.add_trim(src, tool, keep))
}

/// The diameter and depth fields of the RECESS (a counterbore or a countersink) at the geometry - they
/// appear for any hole type other than the plain one.
pub fn sync_hole_params(armed: &qymcad_ui_state::Armed, cmd: &mut qymcad_ui_state::FeatCommand, hole: qymcad_ui_state::HoleCommand, project: &Project) {
    if armed.cmd_kind() != 7 {
        return;
    }
    let want = hole.kind != 0;
    let has = cmd.params.iter().any(|p| p.key == "dia2");
    if want && !has {
        let (d2, dp2) = match cmd.edit {
            Some(fid) => (cmd_param_from(project, fid, "f-recess-d", "dia2", 12.0, 0.1, 10000.0), cmd_param_from(project, fid, "f-recess-depth", "depth2", 4.0, 0.1, 10000.0)),
            None => (qymcad_ui_state::CmdParam::new("f-recess-d", "dia2", 12.0, 0.1, 10000.0), qymcad_ui_state::CmdParam::new("f-recess-depth", "depth2", 4.0, 0.1, 10000.0)),
        };
        cmd.params.push(d2);
        cmd.params.push(dp2);
    } else if !want && has {
        cmd.params.retain(|p| p.key != "dia2" && p.key != "depth2");
    }
}

/// A datum POINT: the X, Y and Z fields at the geometry in coordinate mode, removed in "at a vertex"
/// mode.
pub fn sync_datum_point_params(armed: &qymcad_ui_state::Armed, cmd: &mut qymcad_ui_state::FeatCommand, datum: &qymcad_ui_state::DatumCommand) {
    if armed.cmd_kind() != 21 {
        return;
    }
    let want_coords = datum.pt_mode == 0;
    let has_coords = cmd.params.iter().any(|p| p.key == "x");
    if want_coords && !has_coords {
        cmd.params = vec![qymcad_ui_state::CmdParam::new("X", "x", 0.0, -1e7, 1e7), qymcad_ui_state::CmdParam::new("Y", "y", 0.0, -1e7, 1e7), qymcad_ui_state::CmdParam::new("Z", "z", 0.0, -1e7, 1e7)];
    } else if !want_coords && has_coords {
        cmd.params.clear();
    }
}

/// Synchronise the axis fields at the geometry with the mode (a click on an edge or a face against a
/// hand-typed origin and direction).
pub fn sync_datum_axis_params(armed: &qymcad_ui_state::Armed, cmd: &mut qymcad_ui_state::FeatCommand, datum: &qymcad_ui_state::DatumCommand) {
    if armed.cmd_kind() != 22 {
        return;
    }
    let want_manual = datum.axis_mode == 1;
    let has_manual = cmd.params.iter().any(|p| p.key == "ox");
    if want_manual && !has_manual {
        cmd.params = vec![
            qymcad_ui_state::CmdParam::new("O.x", "ox", 0.0, -1e7, 1e7), qymcad_ui_state::CmdParam::new("O.y", "oy", 0.0, -1e7, 1e7), qymcad_ui_state::CmdParam::new("O.z", "oz", 0.0, -1e7, 1e7),
            qymcad_ui_state::CmdParam::new("Dir.x", "dx", 0.0, -1e7, 1e7), qymcad_ui_state::CmdParam::new("Dir.y", "dy", 0.0, -1e7, 1e7), qymcad_ui_state::CmdParam::new("Dir.z", "dz", 1.0, -1e7, 1e7),
        ];
    } else if !want_manual && has_manual {
        cmd.params.clear();
    }
}

/// Synchronise the array fields at the geometry with the options of the bar: a second step for a linear
/// array with a second direction, and an angle for a circular one that is NOT a full circle (the same way
/// `sync_dir_cmd_params` handles the second side of an extrude).
pub fn sync_array_params(arr: qymcad_ui_state::ArrayParams, armed: &qymcad_ui_state::Armed, cmd: &mut qymcad_ui_state::FeatCommand, project: &Project) {
    match armed.cmd_kind() {
        17 => {
            let has2 = cmd.params.iter().any(|p| p.key == "step2");
            if arr.two && !has2 {
                let prm = match cmd.edit {
                    Some(fid) => cmd_param_from(project, fid, "f-pitch2", "step2", 25.0, 0.01, 100000.0),
                    None => qymcad_ui_state::CmdParam::new("f-pitch2", "step2", 25.0, 0.01, 100000.0),
                };
                cmd.params.push(prm);
            } else if !arr.two && has2 {
                cmd.params.retain(|p| p.key != "step2");
            }
            let has3 = cmd.params.iter().any(|p| p.key == "step3");
            if arr.two && arr.three && !has3 {
                let prm = match cmd.edit {
                    Some(fid) => cmd_param_from(project, fid, "f-pitch3", "step3", 25.0, 0.01, 100000.0),
                    None => qymcad_ui_state::CmdParam::new("f-pitch3", "step3", 25.0, 0.01, 100000.0),
                };
                cmd.params.push(prm);
            } else if (!arr.three || !arr.two) && has3 {
                cmd.params.retain(|p| p.key != "step3");
            }
        }
        18 => {
            let has_ang = cmd.params.iter().any(|p| p.key == "angle");
            if !arr.full && !has_ang {
                let prm = match cmd.edit {
                    Some(fid) => cmd_param_from(project, fid, "f-angle", "angle", 360.0, 1.0, 360.0),
                    None => qymcad_ui_state::CmdParam::new("f-angle", "angle", 360.0, 1.0, 360.0),
                };
                cmd.params.push(prm);
            } else if arr.full && has_ang {
                cmd.params.retain(|p| p.key != "angle");
            }
        }
        _ => {}
    }
}

/// Store the dimensions of a command on feature `body`: an expression stays parametric, a plain number
/// removes the expression (this matters while editing, when a formula gets replaced by a number).
pub fn store_cmd_exprs(cmd: &qymcad_ui_state::FeatCommand, project: &mut Project, body: Id) {
    for p in cmd.params.clone() {
        let t = p.txt.trim().to_string();
        if !t.is_empty() && t.parse::<f64>().is_err() {
            project.set_feat_dim(body, &p.key, t);
        } else {
            project.set_feat_dim(body, &p.key, String::new());
        }
    }
}

/// Synchronise the DIRECTION expression fields at the geometry: the second side (`down`) appears as a
/// field in the popup at the geometry ONLY in the two-sided mode (`qymcad_ui_state::ExtentMode::TwoSided`) of an extrude
/// or a cut, and is removed otherwise. The value is mirrored into `feat_down`, which the preview, the
/// apply and the update all read. When a feature is reopened the seed takes the stored expression
/// (`cmd_param_from`). The height is already a field - so every distance of a command is edited at the
/// geometry rather than by drag values in the top bar.
pub fn sync_dir_cmd_params(armed: &qymcad_ui_state::Armed, cmd: &mut qymcad_ui_state::FeatCommand, project: &Project) {
    if armed.cmd_kind() != 1 {
        return;
    }
    let want_down = cmd.extent.two_sided();
    let has_down = cmd.params.iter().any(|p| p.key == "down");
    if want_down && !has_down {
        let seed = cmd.down.max(0.1);
        let prm = match cmd.edit {
            Some(fid) => cmd_param_from(project, fid, "f-second-side", "down", seed, 0.1, 10000.0),
            None => qymcad_ui_state::CmdParam::new("f-second-side", "down", seed, 0.1, 10000.0),
        };
        cmd.params.push(prm); // after the height, so it becomes the second line of the popup
    } else if !want_down && has_down {
        cmd.params.retain(|p| p.key != "down");
    }
    if want_down {
        cmd.down = qymcad_ui_state::cmd_val(cmd, "down"); // mirrored for the preview, the apply and the update
    }
}

/// Whether every expression of the command's parameters is valid. An invalid one (empty or broken)
/// leaves `p.val` at its OLD value; the command must not be applied then - otherwise a stale value would
/// be applied silently. This gates Apply (both Enter and the buttons) and marks the field in the popup.
pub fn cmd_exprs_valid(cmd: &qymcad_ui_state::FeatCommand, project: &Project) -> bool {
    let vars = project.param_map();
    cmd.params.iter().all(|p| qymcad_core::expr::eval(&p.txt, &vars).is_ok())
}

/// The "what to pick" hint for the active command, while nothing has been picked yet.
pub fn cmd_hint(armed: &qymcad_ui_state::Armed, gsel: &qymcad_ui_state::GeomSelection, trim: &qymcad_ui_state::TrimTool) -> String {
    match armed.cmd_kind() {
        1 | 3 => qymcad_i18n::tr("hint-closed-contour"),
        8 => qymcad_i18n::tr("hint-path-sketch"),
        9 => qymcad_i18n::tr("hint-loft-sections"),
        4 | 5 => qymcad_i18n::tr("hint-body-edges"),
        6 => qymcad_i18n::tr("hint-faces-thickness"),
        7 => qymcad_i18n::tr("hint-body-face"),
        24 => qymcad_i18n::tr("hint-cyl-or-hole"),
        23 => qymcad_i18n::tr("hint-draft-faces"),
        25 => qymcad_i18n::tr("hint-flat-face-offset"),
        30 => qymcad_i18n::tr("hint-face-copy"),
        32 => qymcad_i18n::tr("hint-patch"),
        33 => qymcad_i18n::tr("hint-stitch"),
        34 => qymcad_i18n::tr(if trim.keep.is_none() { "hint-trim" } else { "hint-trim-tool" }),
        31 => qymcad_i18n::tr(if gsel.faces.is_empty() { "hint-surface-replace" } else { "hint-surface-replace-pick" }),
        27 => qymcad_i18n::tr("hint-cut-plane"),
        29 => qymcad_i18n::tr("hint-split-face-plane"),
        28 => qymcad_i18n::tr("hint-face-thickness"),
        26 => qymcad_i18n::tr("hint-feature-faces"),
        16 => qymcad_i18n::tr("hint-mirror-plane"),
        17 => qymcad_i18n::tr("hint-count-dir-step"),
        18 => qymcad_i18n::tr("hint-count-axis-angle"),
        20 => qymcad_i18n::tr("hint-plane-offset"),
        22 => qymcad_i18n::tr("hint-edge-or-manual"),
        _ => String::new(),
    }
}

/// Update an existing DATUM with the parameters of the command (reopened by a double click): it edits
/// `project.planes`, `points` and `axes` plus the feature dims (the offset and the coordinates stay
/// parametric). Resolving happens in `qymcad_ui_state::regenerate_all` after the apply.
pub fn update_datum_feat(armed: &qymcad_ui_state::Armed, cmd: &qymcad_ui_state::FeatCommand, datum: &qymcad_ui_state::DatumCommand, edges: &qymcad_ui_state::EdgeCache, project: &mut Project, fid: Id) -> Option<Id> {
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
                    PlaneDef::Manual => {}
                },
            }
            store_cmd_exprs(cmd, project, fid); // `dist` stays parametric
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
            store_cmd_exprs(cmd, project, fid); // x, y and z stay parametric
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

/// A hole: a cylinder cut at the centre of the picked face, perpendicular to it.
pub fn apply_hole_cmd(cmd: &qymcad_ui_state::FeatCommand, hole: qymcad_ui_state::HoleCommand, project: &mut Project, sel: qymcad_ui_state::Sel, status: &mut String) -> Option<Id> {
    // the kind plus the diameter and depth of the recess (for a counterbore or a countersink)
    let (dia2, depth2) = if hole.kind != 0 { (qymcad_ui_state::cmd_val(cmd, "dia2"), qymcad_ui_state::cmd_val(cmd, "depth2")) } else { (0.0, 0.0) };
    // "by sketch" mode drills many holes into the picked body, at the isolated points of a sketch
    if hole.mode == 1 {
        let Some(sid) = hole.sketch else {
            *status = qymcad_i18n::tr("msg-pick-sketch-points");
            return None;
        };
        let Some(src) = qymcad_ui_state::selected_body(project, &sel) else {
            *status = qymcad_i18n::tr("msg-pick-body-drill");
            return None;
        };
        if project.sketch_isolated_points(sid).is_empty() {
            *status = qymcad_i18n::tr("msg-no-isolated-points");
            return None;
        }
        let body = project.add_hole_from_sketch(src, sid, qymcad_core::model::HoleTool { kind: hole.kind, diameter: qymcad_ui_state::cmd_val(cmd, "diameter"), depth: qymcad_ui_state::cmd_val(cmd, "depth"), dia2: dia2, depth2: depth2 }, hole.flip);
        store_cmd_exprs(cmd, project, body);
        return Some(body);
    }
    let qymcad_ui_state::Sel::Face(mi, fi) = sel else {
        *status = qymcad_i18n::tr("msg-pick-face-centre");
        return None;
    };
    let src = project.mesh_id(mi)?;
    let face = project.bodies.get(mi).and_then(|b| b.faces.get(fi))?;
    // the face is referred to by its persistent id, so the hole holds on to it through a rebuild
    let key = qymcad_core::feature::FaceKey { index: fi as u32, centroid: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, id: face.id };
    let body = project.add_hole_typed(src, key, qymcad_core::model::HoleTool { kind: hole.kind, diameter: qymcad_ui_state::cmd_val(cmd, "diameter"), depth: qymcad_ui_state::cmd_val(cmd, "depth"), dia2: dia2, depth2: depth2 });
    store_cmd_exprs(cmd, project, body);
    Some(body)
}

/// Apply the thread: `add_thread` on the picked rim, plus storing the parameter expressions.
pub fn apply_thread_cmd(cmd: &qymcad_ui_state::FeatCommand, project: &mut Project, status: &mut String, thread: qymcad_ui_state::ThreadParams) -> Option<Id> {
    let src = thread.src?;
    if thread.edge == 0 {
        *status = qymcad_i18n::tr("msg-pick-cyl-first");
        return None;
    }
    // the input is a standard plus a size, as in a professional CAD; the geometry (the diameters, the
    // depth, the profile) is computed by the core from the formulas of the standard rather than
    // estimated by eye.
    let body = if thread.auger {
        let spec = qymcad_core::thread::AugerSpec {
            shaft_d: thread.radius * 2.0, // taken from the actual geometry; the regen refines it
            outer_d: qymcad_ui_state::cmd_val(cmd, "outer"),
            pitch: qymcad_ui_state::cmd_val(cmd, "pitch"),
            thickness: qymcad_ui_state::cmd_val(cmd, "thickness"),
            starts: thread.starts.max(1),
            left: thread.left,
            edge_r: qymcad_ui_state::cmd_val(cmd, "edge_r"),
        };
        project.add_auger(src, thread.edge, spec, qymcad_ui_state::cmd_val(cmd, "length"), qymcad_ui_state::cmd_val(cmd, "lead_in"), qymcad_ui_state::cmd_val(cmd, "lead_out"))
    } else {
        project.add_thread(src, thread.edge, thread_spec(cmd, thread), qymcad_ui_state::cmd_val(cmd, "length"), qymcad_ui_state::cmd_val(cmd, "lead_in"), qymcad_ui_state::cmd_val(cmd, "lead_out"))
    };
    store_cmd_exprs(cmd, project, body);
    Some(body)
}

/// Apply the stitch: two or more sheets.
pub fn apply_stitch_cmd(cmd: &qymcad_ui_state::FeatCommand, project: &mut Project, status: &mut String, stitch_parts: &[Id]) -> Option<Id> {
    if stitch_parts.len() < 2 {
        *status = qymcad_i18n::tr("msg-stitch-needs-two");
        return None;
    }
    let body = project.add_stitch(stitch_parts.to_vec(), qymcad_ui_state::cmd_val(cmd, "tol"));
    store_cmd_exprs(cmd, project, body); // the tolerance stays parametric, like every number of a feature
    Some(body)
}

/// Apply the active datum command (Enter). Parametric: the offset and the coordinates are expressions
/// stored as feature dimensions.
pub fn apply_datum_cmd(armed: &qymcad_ui_state::Armed, cmd: &qymcad_ui_state::FeatCommand, datum: &qymcad_ui_state::DatumCommand, edges: &qymcad_ui_state::EdgeCache, project: &mut Project, status: &mut String) -> Option<Id> {
    use qymcad_core::feature::SketchPlane;
    match armed.cmd_kind() {
        20 => {
            let dist = qymcad_ui_state::cmd_val(cmd, "dist");
            let id = match datum.plane_pick {
                Some(SketchPlane::World(bp)) => project.add_offset_plane(bp, dist),
                Some(SketchPlane::Face(body, key)) => project.add_plane_from_face(body, key, dist),
                Some(SketchPlane::Datum(did)) => project.add_offset_from_plane(did, dist), // parametric
                None => {
                    *status = qymcad_i18n::tr("msg-click-base-plane");
                    return None;
                }
            };
            store_cmd_exprs(cmd, project, id); // the `dist` offset stays parametric in all three cases
            Some(id)
        }
        21 => {
            // "at a vertex" gives an associative point that travels with the vertex; otherwise the
            // coordinates, which stay parametric
            if datum.pt_mode == 1 {
                let Some((body, edge, end, at)) = datum.pt_vert else {
                    *status = qymcad_i18n::tr("msg-click-vertex");
                    return None;
                };
                return Some(project.add_point_at_vertex(at, body, edge, end));
            }
            let at = [qymcad_ui_state::cmd_val(cmd, "x"), qymcad_ui_state::cmd_val(cmd, "y"), qymcad_ui_state::cmd_val(cmd, "z")];
            let id = project.add_point_at(at);
            store_cmd_exprs(cmd, project, id); // x, y and z stay parametric
            Some(id)
        }
        22 => {
            // mode 2, "two points": if BOTH are datum points the axis is a parametric TwoPoints;
            // otherwise it is a manual one built from the coordinates
            if datum.axis_mode == 2 {
                if datum.axis_pts.len() < 2 {
                    *status = qymcad_i18n::tr("msg-click-two-points");
                    return None;
                }
                let (p0, p1) = (datum.axis_pts[0], datum.axis_pts[1]);
                if p0.0 != 0 && p1.0 != 0 {
                    return Some(project.add_axis_two_points(p0.0, p1.0)); // associative to the points
                }
                let d = [p1.1[0] - p0.1[0], p1.1[1] - p0.1[1], p1.1[2] - p0.1[2]];
                if d[0].abs() + d[1].abs() + d[2].abs() < 1e-9 {
                    *status = qymcad_i18n::tr("msg-points-coincide");
                    return None;
                }
                return Some(project.add_axis_manual(p0.1, d));
            }
            // mode 0, "by an edge or a face", gives an ASSOCIATIVE axis that travels with its source
            if datum.axis_mode == 0 {
                return match datum.axis_hit {
                    Some(qymcad_ui_state::AxisHit::Edge(i)) => edges.axes.get(i).map(|&(body, edge, _)| project.add_axis_from_edge(body, edge)),
                    Some(qymcad_ui_state::AxisHit::Face(body, fid)) => Some(project.add_axis_from_face(body, fid)),
                    Some(qymcad_ui_state::AxisHit::Datum(_)) | None => match datum.axis_ref {
                        Some((o, d)) => Some(project.add_axis_manual(o, d)),
                        None => {
                            *status = qymcad_i18n::tr("msg-click-edge-axis");
                            None
                        }
                    },
                };
            }
            let (o, d) = ([qymcad_ui_state::cmd_val(cmd, "ox"), qymcad_ui_state::cmd_val(cmd, "oy"), qymcad_ui_state::cmd_val(cmd, "oz")], [qymcad_ui_state::cmd_val(cmd, "dx"), qymcad_ui_state::cmd_val(cmd, "dy"), qymcad_ui_state::cmd_val(cmd, "dz")]);
            if d[0].abs() + d[1].abs() + d[2].abs() < 1e-9 {
                *status = qymcad_i18n::tr("msg-axis-not-set");
                return None;
            }
            Some(project.add_axis_manual(o, d))
        }
        _ => None,
    }
}

/// Turn on interactive moving (op=1), copying (op=2) or rotating (op=3): pick, then the base point or
/// centre, then the target (for a rotation, the angle in the popup).
pub fn start_move_tool(t: &mut qymcad_ui_state::Tools, status: &mut String, op: u8) {
    let cur = t.armed.move_op();
    qymcad_ui_state::exit_draw_tools(&mut t.reborrow());
    let qymcad_ui_state::Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat: _, pending_import: _, picking: _, place: _, sel_sk, tool } = t;
    sel_sk.modify = None;
    **armed = if cur == op { qymcad_ui_state::Armed::None } else { qymcad_ui_state::Armed::Move(op) };
    tool.move_base = None;
    let has_sel = sel_sk.items.iter().any(|(k, _)| *k == 1);
    let what = if op == 3 { qymcad_i18n::tr("f-rotation-centre") } else { qymcad_i18n::tr("f-base-point") };
    *status = if armed.move_op() == 0 {
        qymcad_i18n::tr("msg-cancelled")
    } else if has_sel {
        qymcad_i18n::tr1("cmd-click-what", "what", &what)
    } else {
        qymcad_i18n::tr1("cmd-select-then-click", "what", &what)
    };
}

/// Turn on the array feature (op=1 linear, 2 circular): pick, then parameters, then Enter, with a
/// preview.
pub fn start_pattern(t: &mut qymcad_ui_state::Tools, status: &mut String, op: u8) {
    let cur = t.armed.pat_op();
    qymcad_ui_state::exit_draw_tools(&mut t.reborrow()); // drops the other modes (move, click-op) and `pat.op`
    let qymcad_ui_state::Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat, pending_import: _, picking: _, place: _, sel_sk, tool: _ } = t;
    sel_sk.modify = None;
    **armed = if cur == op { qymcad_ui_state::Armed::None } else { qymcad_ui_state::Armed::Pattern(op) };
    pat.edit = None;
    pat.center = None;
    *status = if armed.pat_op() == 0 {
        qymcad_i18n::tr("msg-cancelled")
    } else if armed.pat_op() == 2 {
        qymcad_i18n::tr("msg-circ-array-sketch")
    } else {
        qymcad_i18n::tr("msg-lin-array-sketch")
    };
}

/// The top bar of a body-to-body boolean, in the style of the sketcher: the kind of operation, the hint
/// about picking body B, and a cancel. It is shown while body A is picked and the click on B is awaited
/// (`bool_pick`).
pub fn bool_tool_bar(pc: &mut qymcad_ui_state::PartCtx, ui: &mut egui::Ui) {
    use qymcad_core::feature::FeatureKind;
    // CREATING: body A is picked, the click on B and the choice of kind are awaited
    if let Some((a, mut op)) = pc.boolean.pick {
        let a_name = pc.project.mesh_index(a).map(|mi| qymcad_i18n::name(&pc.project.mesh_name(mi))).unwrap_or_else(|| "?".into());
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::INTERSECT, qymcad_i18n::tr("bool-bodies-btn"))).strong());
            ui.separator();
            ui.label(qymcad_i18n::tr("f-kind"));
            ui.selectable_value(&mut op, 0u8, qymcad_i18n::tr("f-cut-ab"));
            ui.selectable_value(&mut op, 1u8, qymcad_i18n::tr("f-union"));
            ui.selectable_value(&mut op, 2u8, qymcad_i18n::tr("f-intersect"));
            ui.separator();
            ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-bool-a-is", "name", &a_name)).color(pc.scheme.pal.hint()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("f-cancel-esc")).clicked() {
                    cancel = true;
                }
            });
        });
        pc.boolean.pick = if cancel { None } else { Some((a, op)) };
        return;
    }
    // EDITING: a double click on the boolean node changes its KIND in the bar (as reopening the command
    // does for an extrude)
    if let Some(ti) = pc.boolean.edit {
        let Some(FeatureKind::BodyBoolean { op, .. }) = pc.project.timeline.get(ti).map(|n| n.kind.clone()) else {
            pc.boolean.edit = None;
            return;
        };
        let mut op = op;
        let (mut done, mut changed) = (false, false);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::INTERSECT, qymcad_i18n::tr("bool-bodies-edit"))).strong());
            ui.separator();
            ui.label(qymcad_i18n::tr("f-kind"));
            changed |= ui.selectable_value(&mut op, 0u8, qymcad_i18n::tr("f-cut-ab")).changed();
            changed |= ui.selectable_value(&mut op, 1u8, qymcad_i18n::tr("f-union")).changed();
            changed |= ui.selectable_value(&mut op, 2u8, qymcad_i18n::tr("f-intersect")).changed();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("f-done")).clicked() {
                    done = true;
                }
            });
        });
        if changed {
            if let Some(FeatureKind::BodyBoolean { op: o, .. }) = pc.project.timeline.get_mut(ti).map(|n| &mut n.kind) {
                *o = op;
            }
            pc.project.timeline[ti].dirty = true;
            qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the planner does the counting
        }
        if done || pc.project.timeline.get(ti).is_none_or(|n| !matches!(n.kind, FeatureKind::BodyBoolean { .. })) {
            pc.boolean.edit = None;
        }
    }
}

/// A fillet (4) or a chamfer (5) on the picked edges of the picked body (an empty pick means every edge).
pub fn apply_edge_cmd(pc: &mut qymcad_ui_state::PartCtx, cmd: u8) -> Option<Id> {
    // the target of a fillet or a chamfer is the body whose PERSISTENT edge ids actually sit in the edge
    // selection (`pc.edges.body`, kept in step by `refresh_edges`) rather than `qymcad_ui_state::selected_body()`:
    // otherwise, if `pc.sel` moved to another body between picking the edges and pressing Enter, the
    // chamfer would go to ids belonging to someone else or to none at all.
    // A part is one body, so when neither edges nor a body are picked, THE SINGLE body of the part is
    // taken (`active_body`), and the tool works straight away - a chamfer on EVERY edge, for instance -
    // without a "pick a body" step.
    let ctx = qymcad_ui_state::current_ctx_id(pc.active_path, pc.project);
    // the fallback through `qymcad_ui_state::current_body()` sees the body of the active part even WITHOUT entering it
    let Some(body) = pc.edges.body.or_else(|| qymcad_ui_state::selected_body(pc.project, pc.sel)).or_else(|| qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path })) else {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return None;
    };
    // a hard gate: the body is alive and not consumed by a chain, or it would be a ghost branch...
    if pc.project.consumed_bodies().contains(&body) {
        *pc.status = qymcad_i18n::tr("msg-body-consumed");
        return None;
    }
    // ...and belongs to the current context (a body of a neighbouring part must not be filleted from
    // inside this one).
    if pc.project.body_owner(body).is_some_and(|o| !pc.project.component_is_within(o, ctx)) {
        *pc.status = qymcad_i18n::tr("msg-edge-other-part");
        return None;
    }
    let edges: Vec<u32> = pc.gsel.edges.iter().copied().collect();
    // IT IS RECORDED THE WAY IT WAS PICKED. If there is a description ("every edge of this face", "every
    // edge parallel to this one"), the description goes in: it survives an edit that adds elements.
    // Otherwise the list of picked ids goes in.
    let described: Option<qymcad_core::refs::Ref> = pc.gsel.described.clone().map(qymcad_core::refs::Ref::many);
    let last = if cmd == 4 {
        let r = qymcad_ui_state::cmd_val(pc.cmd, "radius");
        // THE "VERTEX -> RADIUS" TABLE. It works with a description and with a list alike: the radius is
        // set at a point rather than along an edge, so it needs no direction of an edge.
        let at = fillet_vertex_table(pc.cmd);
        if !at.is_empty() {
            let q = described.clone().unwrap_or_else(|| qymcad_core::refs::Ref::picks(&edges));
            pc.project.add_fillet_at_vertices(body, r, q, at)
        } else if let Some(q) = described {
            pc.project.add_fillet_ref(body, r, q)
        } else {
            pc.project.add_fillet(body, r, edges)
        }
    } else {
        use qymcad_core::feature::ChamferMode;
        // the asymmetric modes (two distances, or a leg plus an angle) work only on picked edges;
        // otherwise it is symmetric
        if pc.chamfer.mode != ChamferMode::Symmetric && !edges.is_empty() {
            pc.project.add_chamfer_ex(body, qymcad_ui_state::cmd_val(pc.cmd, "dist"), qymcad_core::model::ChamferShape { mode: pc.chamfer.mode, d2: qymcad_ui_state::cmd_val(pc.cmd, "d2"), flip: pc.chamfer.flip, ref_face: pc.chamfer.ref_face }, edges)
        } else {
            match described {
                Some(q) => pc.project.add_chamfer_ref(body, qymcad_ui_state::cmd_val(pc.cmd, "dist"), q),
                None => pc.project.add_chamfer(body, qymcad_ui_state::cmd_val(pc.cmd, "dist"), edges),
            }
        }
    };
    store_cmd_exprs(pc.cmd, pc.project, last); // radius, radius2, dist and d2 stay parametric
    Some(last)
}

/// Apply the LOFT: a body through a set of sketch sections (`loft_sids`, two or more). Every section is
/// entered into the timeline if it is not there yet, then the Loft node follows, with the picked contours
/// (`loft_cids`) and the kind of faces (`loft_ruled`).
pub fn apply_loft_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    if pc.loft.sids.len() < 2 {
        *pc.status = qymcad_i18n::tr("msg-loft-needs-two");
        return None;
    }
    // A cut, a pad or an intersection through sections: the target is the active body of the context.
    // "Surface" is a fourth kind of result rather than a separate tool. The question is the same one -
    // what should come out - and splitting it across two buttons would mean asking for a tool to be
    // chosen before that question has been answered.
    let surface = pc.loft.result == 4;
    let (mut src, mut op): (Id, u8) = (0, 0);
    if pc.loft.result != 0 && !surface {
        match qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }) {
            Some(b) => {
                src = b;
                op = pc.loft.result - 1; // 1 becomes a cut (0), 2 a union (1), 3 an intersection (2)
            }
            None => {
                *pc.status = qymcad_i18n::tr("msg-loft-no-body");
            }
        }
    }
    let (sids, cids) = (pc.loft.sids.clone(), pc.loft.cids.clone());
    for &sid in &sids {
        let name = pc.project.sketches.iter().find(|s| s.id == sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_default();
        pc.project.add_sketch_node(sid, name);
    }
    let body = pc.project.add_loft(sids, cids, pc.loft.ruled, src, op, surface);
    if body == 0 {
        return None;
    }
    // a SOLID loft (src == 0) is a material base and goes into the single body of the part; a cut loft
    // (src != 0) is already a boolean
    Some(if src == 0 { pc.project.finish_base_body(body, 1) } else { body })
}

pub fn start_draft_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return;
    }
    pc.cmd.open(pc.armed, 23, *pc.mode_3d); // a clean slate, then open
    
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear(); // the faces to tilt are picked from scratch: a draft needs no pre-selection
    pc.gsel.faces_body = None;
    pc.cmd.params = vec![qymcad_ui_state::CmdParam::new("f-angle", "angle", 3.0, -60.0, 60.0)];
    *pc.status = qymcad_i18n::tr("msg-draft");
}

/// Apply the removal of faces.
///
/// APPLY THE FACE COPY: the faces are recorded AS A QUERY - a description if there is one, otherwise a
/// list of picks. A copy must travel with its base exactly as everything else does.
pub fn apply_face_copy_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc.gsel.faces_body.or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    if pc.gsel.faces.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-faces-not-found");
        return None;
    }
    let picks: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    let q = pc.gsel.described.clone().map(qymcad_core::refs::Ref::many).unwrap_or_else(|| qymcad_core::refs::Ref::picks(&picks));
    Some(pc.project.add_face_copy(src, q))
}

/// APPLY THE PATCH: the edges go in as a query (a description if there is one), otherwise as a list of
/// picks.
pub fn apply_patch_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc.edges.body.or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    if pc.gsel.edges.len() < 2 {
        *pc.status = qymcad_i18n::tr("msg-patch-needs-edges");
        return None;
    }
    let picks: Vec<u32> = pc.gsel.edges.iter().copied().collect();
    let q = pc.gsel.described.clone().map(qymcad_core::refs::Ref::many).unwrap_or_else(|| qymcad_core::refs::Ref::picks(&picks));
    Some(pc.project.add_patch(src, q, pc.opts.patch_tangent))
}

/// Apply the cut: the plane (as a reference - a datum or a world one) plus the offset. The number of
/// pieces is counted BEFORE the feature is created: the timeline must create exactly as many bodies as
/// will come out.
pub fn apply_split_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    if pc.split.plane.is_none() {
        *pc.status = qymcad_i18n::tr("msg-click-cut-plane");
        return None;
    }
    let Some(pieces) = split_piece_count(pc, src).filter(|&n| n >= 2) else {
        *pc.status = qymcad_i18n::tr("msg-plane-cuts-nothing");
        return None;
    };
    let sp = pc.split.plane?;
    let (plane, datum) = qymcad_ui_state::resolve_mirror_plane(pc.project, sp); // the same reference the mirror uses: a datum or a world plane
    let offset = qymcad_ui_state::cmd_val(pc.cmd, "offset");
    let parts = pc.project.add_split_body(src, plane, datum, offset, pieces);
    let first = *parts.first()?;
    store_cmd_exprs(pc.cmd, pc.project, first); // the offset stays parametric
    *pc.status = qymcad_i18n::tr1("cmd-split-done", "n", &pieces.to_string());
    Some(first)
}

/// The cutting plane in THE LOCAL FRAME of the body (where its B-rep lives), with the offset already
/// applied. The pick gives world coordinates, and without the conversion a cut inside an assembly would
/// drift by the transform of the component.
pub fn split_plane_local(pc: &mut qymcad_ui_state::PartCtx, src: Id) -> Option<([f64; 3], [f64; 3])> {
    use qymcad_core::feature::{apply12, apply12_dir, mat_inv12};
    let sp = pc.split.plane?;
    let (o, n) = qymcad_ui_state::mirror_plane_world(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, &sp)?;
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l < 1e-9 {
        return None;
    }
    let u = [n[0] / l, n[1] / l, n[2] / l];
    let d = qymcad_ui_state::cmd_val(pc.cmd, "offset");
    let ow = [o[0] + u[0] * d, o[1] + u[1] * d, o[2] + u[2] * d];
    let inv = mat_inv12(&pc.project.body_display_transform(src, qymcad_ui_state::current_ctx_id(pc.active_path, pc.project)));
    Some((apply12(&inv, ow), apply12_dir(&inv, u)))
}

/// How many pieces a cutting plane yields - counted by the LIVE B-rep. It cannot be derived from the
/// plane itself: one plane cuts a U-shaped part into three, and "always two halves" would lose a piece.
/// `None` means the plane does not cut the body at all.
pub fn split_piece_count(pc: &mut qymcad_ui_state::PartCtx, src: Id) -> Option<usize> {
    let (o, n) = split_plane_local(pc, src)?;
    qymcad_ui_state::ensure_brep(&mut pc.rebuild()); // there is nothing to cut with while there is no live B-rep
    let sh = pc.live.shapes.get(&src)?;
    sh.split_by_plane(o, n, 0).map(|v| v.len())
}

/// Apply the thickening: one selected face plus a thickness.
pub fn apply_thicken_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    // THE BODY IS TAKEN FROM THE FACE THAT WAS CLICKED rather than from the selection in the tree. Now
    // that surfaces exist, a sheet lives in the scene alongside the part, and it is the sheet that gets
    // thickened: with the part selected in the tree, the face of the sheet would be looked for on the
    // part and not found.
    let src = pc.gsel.faces_body.or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    let Some(fid) = pc.gsel.faces.iter().copied().next() else {
        *pc.status = qymcad_i18n::tr("msg-click-face-thicken");
        return None;
    };
    let t = qymcad_ui_state::cmd_val(pc.cmd, "thickness");
    if t.abs() < 1e-9 {
        *pc.status = qymcad_i18n::tr("msg-zero-thickness");
        return None;
    }
    let body = pc.project.add_thicken(src, fid, t);
    store_cmd_exprs(pc.cmd, pc.project, body); // the thickness stays parametric
    Some(body)
}

/// Apply the division of faces.
pub fn apply_split_face_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    if pc.split.plane.is_none() {
        *pc.status = qymcad_i18n::tr("msg-click-plane");
        return None;
    }
    // WHAT WILL COME OUT IS CHECKED BEFORE THE FEATURE IS CREATED: a plane that misses the body divides
    // nothing, and a node in the timeline that is certain to go red is of no use to anyone.
    let (o, n) = split_plane_local(pc, src)?;
    qymcad_ui_state::ensure_brep(&mut pc.rebuild());
    if pc.live.shapes.get(&src).and_then(|sh| sh.split_faces(o, n)).is_none() {
        *pc.status = qymcad_i18n::tr("msg-plane-splits-nothing");
        return None;
    }
    let sp = pc.split.plane?;
    let (plane, datum) = qymcad_ui_state::resolve_mirror_plane(pc.project, sp);
    let offset = qymcad_ui_state::cmd_val(pc.cmd, "offset");
    let body = pc.project.add_split_face(src, plane, datum, offset);
    store_cmd_exprs(pc.cmd, pc.project, body);
    *pc.status = qymcad_i18n::tr("msg-faces-split");
    Some(body)
}

/// A command on a finished body (a fillet or a chamfer works on edges; a shell or a hole on a face).
pub fn start_body_cmd(pc: &mut qymcad_ui_state::PartCtx, cmd: u8) {
    // a part is one body, so a command on a body works on THE SINGLE body of the part without clicking it
    // first. It starts if there is a body at all (either selected or the active one); it refuses only
    // when there is no body whatsoever.
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return;
    }
    pc.cmd.open(pc.armed, cmd, *pc.mode_3d); // a clean slate, then open
     // a new feature, not an edit
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear(); // a shell picks faces from scratch
    pc.gsel.faces_body = None; // the scope of the multiple face selection
    pc.opts.shell_side = qymcad_core::feature::ShellSide::Inward; // a shell goes INWARDS by default
    pc.chamfer.mode = qymcad_core::feature::ChamferMode::Symmetric; // a chamfer is symmetric by default
    pc.chamfer.flip = false;
    pc.chamfer.ref_face = 0; // the reference face is automatic until picked by hand
    pc.chamfer.pick_ref = false;
    pc.hole.kind = 0; // a hole is plain by default
    pc.hole.mode = 0; // by default "on a face"
    pc.hole.sketch = None;
    pc.hole.flip = false;
    pc.cmd.params = match cmd {
        // ONE field: the base radius. A variable fillet is set BY VERTICES: click a vertex of the
        // selection and it gets a field of its own. There is no second "radius 2" field any more: it
        // described a single edge with a direction and was incompatible with a selection in principle.
        4 => vec![qymcad_ui_state::CmdParam::new("f-radius", "radius", 2.0, 0.05, 1000.0)],
        // the leg d1 is always there; d2 is either the second leg (two distances) or the angle in
        // degrees (a leg plus an angle), enabled by the mode chosen in the top panel
        5 => vec![qymcad_ui_state::CmdParam::new("f-leg", "dist", 1.5, 0.05, 1000.0), qymcad_ui_state::CmdParam::new(qymcad_ui_state::chamfer_d2_label(qymcad_core::feature::ChamferMode::Symmetric), "d2", 1.5, 0.0, 1000.0)],
        6 => vec![qymcad_ui_state::CmdParam::new("f-thickness", "thickness", 2.0, 0.1, 1000.0)],
        _ => vec![qymcad_ui_state::CmdParam::new("f-diameter", "diameter", 6.0, 0.1, 10000.0), qymcad_ui_state::CmdParam::new("f-depth", "depth", 15.0, 0.1, 10000.0)],
    };
    *pc.status = match cmd {
        4 => qymcad_i18n::tr("msg-fillet"),
        5 => qymcad_i18n::tr("msg-chamfer"),
        6 => qymcad_i18n::tr("msg-shell"),
        _ => qymcad_i18n::tr("msg-hole"),
    };
}

/// Create the array from the options picked by the command. Associative; the step and the angle stay
/// parametric.
pub fn apply_array_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    if pc.armed.cmd_kind() == 18 {
        let angle = if pc.arr.full { 360.0 } else { qymcad_ui_state::cmd_val(pc.cmd, "angle") };
        let body = pc.project.add_circular_array_axis(src, pc.arr.count.max(1), angle, pc.arr.axis);
        if pc.arr.full {
            pc.project.set_feat_dim(body, "angle", String::new());
        } else {
            store_cmd_exprs(pc.cmd, pc.project, body); // the `angle` key stays parametric (the regen reads the feature dims)
        }
        Some(body)
    } else {
        let (dx, dy, dz) = qymcad_ui_state::arr_vec(pc.arr.dir, qymcad_ui_state::cmd_val(pc.cmd, "step"));
        let (dx2, dy2, dz2, c2) = if pc.arr.two {
            let (a, b, c) = qymcad_ui_state::arr_vec(pc.arr.dir2, qymcad_ui_state::cmd_val(pc.cmd, "step2"));
            (a, b, c, pc.arr.count2.max(1))
        } else {
            (0.0, 0.0, 0.0, 1)
        };
        let (dx3, dy3, dz3, c3) = if pc.arr.two && pc.arr.three {
            let (a, b, c) = qymcad_ui_state::arr_vec(pc.arr.dir3, qymcad_ui_state::cmd_val(pc.cmd, "step3"));
            (a, b, c, pc.arr.count3.max(1))
        } else {
            (0.0, 0.0, 0.0, 1)
        };
        let body = pc.project.add_linear_array_grid3(src, [ArrayAxis { d: [dx, dy, dz], count: pc.arr.count.max(1) }, ArrayAxis { d: [dx2, dy2, dz2], count: c2 }, ArrayAxis { d: [dx3, dy3, dz3], count: c3 }]);
        store_cmd_exprs(pc.cmd, pc.project, body); // the logical `step`, `step2` and `step3` so that reopening returns the TEXT of the expression
        // THE TEXTS ARE TAKEN FIRST, and the document is written afterwards. Reading the command and
        // writing the document in one expression borrows the application twice at once; taken apart,
        // the order of the two is also said out loud instead of being left to the compiler.
        let (d1, d2, d3) = (pc.arr.dir, pc.arr.dir2, pc.arr.dir3);
        let (t1, t2, t3) = (cmd_txt(pc.cmd, "step"), cmd_txt(pc.cmd, "step2"), cmd_txt(pc.cmd, "step3"));
        let (two, three) = (pc.arr.two, pc.arr.three);
        qymcad_ui_state::store_arr_component(pc.project, body, ["dx", "dy", "dz"], d1, t1);
        if two {
            qymcad_ui_state::store_arr_component(pc.project, body, ["dx2", "dy2", "dz2"], d2, t2);
        }
        if two && three {
            qymcad_ui_state::store_arr_component(pc.project, body, ["dx3", "dy3", "dz3"], d3, t3);
        }
        Some(body)
    }
}

/// Create the mirror from the plane picked by the command. Associative.
pub fn apply_mirror_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    let Some(sp) = pc.mirror.plane else {
        *pc.status = qymcad_i18n::tr("msg-click-mirror-plane");
        return None;
    };
    let (plane, datum) = qymcad_ui_state::resolve_mirror_plane(pc.project, sp);
    Some(pc.project.add_mirror(src, plane, pc.opts.mirror_keep, datum))
}

/// Mirror as A COMMAND: pick a body, then the top bar (keep the original or not), then PICK BY CLICK the
/// mirror plane, datum or face in the viewport (as when picking a sketch plane), then the preview, then
/// Enter.
pub fn start_mirror_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return;
    }
    pc.cmd.open(pc.armed, 16, *pc.mode_3d); // a clean slate, then open
    
    *pc.mode_3d = true;
    pc.opts.mirror_keep = true;
    pc.mirror.plane = None;
    pc.cmd.params.clear();
    *pc.status = qymcad_i18n::tr("msg-mirror-pick");
}

/// Apply the accumulated drag of the component gizmo: `transform = accumulated * start` ->
/// `set_component_transform` (cheap, with no rebuild of bodies). The start is pinned, so the motion is
/// smooth; snapping comes from the panel or from Ctrl.
pub fn apply_comp_giz(pc: &mut qymcad_ui_state::PartCtx) {
    qymcad_ui_state::begin_edit(pc.edits, pc.project, qymcad_i18n::tr("status-move-component")); // THE BOUNDARY OF AN OPERATION
    let Some((comp, _, _, _)) = pc.comp_giz.drag else { return };
    if let Some(t) = qymcad_ui_state::comp_giz_accum(pc.comp_giz, pc.set, pc.comp_giz.snap) {
        pc.project.set_component_transform(comp, t);
        qymcad_ui_state::invalidate_placement(pc.regen); // THE PLACEMENT moved; the shape of the bodies did not change
    }
        qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// Whether the command is ready to apply (the required selection exists) - for highlighting the button
/// and the field.
pub fn cmd_ready(pc: &mut qymcad_ui_state::PartCtx) -> bool {
    if pc.cmd.edit.is_some() {
        return true; // editing an existing feature - the input is already valid
    }
    match pc.armed.cmd_kind() {
        1 | 3 => !pc.gsel.profiles.is_empty() || pc.cmd.sketch.map(|si| qymcad_ui_state::sketch_closed_contours(pc.project, si).len() == 1).unwrap_or(false),
        8 => pc.sweep.prof_sid != 0 && pc.sweep.path_sid != 0, // sweep: a profile and a path
        9 => pc.loft.sids.len() >= 2, // loft: at least two sections
        4 => qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_some(),
        // chamfer: a body exists (a part has one body); the asymmetric modes need edges TO BE PICKED
        5 => qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_some() && (pc.chamfer.mode == qymcad_core::feature::ChamferMode::Symmetric || !pc.gsel.edges.is_empty()),
        6 => !pc.gsel.faces.is_empty(), // shell: a multiple selection of faces
        23 => !pc.gsel.faces.is_empty() && pc.draft.neutral != 0, // draft: faces plus the neutral face
        25 | 26 | 28 | 30 => !pc.gsel.faces.is_empty(), // push, remove, thicken or copy a face
        // surface replace: BOTH the faces of the base AND the surface are needed - a node without either
        // makes no sense
        31 => !pc.gsel.faces.is_empty() && pc.repl_surface.is_some(),
        32 => pc.gsel.edges.len() >= 2, // patch: a boundary of edges (one cannot define it)
        33 => pc.stitch_parts.len() >= 2, // stitch: at least two sheets
        34 => pc.trim.keep.is_some() && pc.trim.tool.is_some(), // trim: what is cut and what cuts it
        24 => pc.thread.edge != 0, // thread: the rim of a cylinder or a hole is picked

        // hole: "by face" needs a face picked; "by sketch" needs a body plus a sketch with isolated points
        7 if pc.hole.mode == 1 => qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_some() && pc.hole.sketch.map(|sid| !pc.project.sketch_isolated_points(sid).is_empty()).unwrap_or(false),
        7 => matches!(pc.sel, qymcad_ui_state::Sel::Face(..)),
        10..=15 => true, // primitives: the sizes are in fields, no selection is needed
        16 => pc.mirror.plane.is_some(), // mirror: a plane is picked by click
        27 | 29 => pc.split.plane.is_some(), // splitting a body or dividing faces: a plane is picked
        17 | 18 => qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_some(), // array: the part has a body (the options are in the bar)
        20 => pc.datum.plane_pick.is_some(),  // datum plane: a reference is picked by click
        21 => pc.datum.pt_mode == 0 || pc.datum.pt_vert.is_some(), // coordinates in fields, or a vertex picked
        22 => pc.datum.axis_mode == 1 || (pc.datum.axis_mode == 2 && pc.datum.axis_pts.len() == 2) || pc.datum.axis_ref.is_some(),
        _ => false,
    }
}

pub fn cmd_anchor_screen(pc: &mut qymcad_ui_state::PartCtx, rect: Rect) -> Option<Pos2> {
    let basis = pc.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: pc.cam, set: pc.set, rect: rect, basis: &basis };
    match pc.armed.cmd_kind() {
        1 | 3 => {
            let (base, dir, h) = qymcad_ui_state::feat_cmd_axis(pc.cmd, pc.gsel, pc.project)?;
            let dir = if pc.armed.cmd_kind() == 1 && pc.feat.flip { [-dir[0], -dir[1], -dir[2]] } else { dir };
            let p = if pc.armed.cmd_kind() == 3 { base } else { [base[0] + dir[0] * h, base[1] + dir[1] * h, base[2] + dir[2] * h] };
            Some(scr.at(p).0)
        }
        // EVERYTHING AIMED BY CLICKING A BODY KEEPS ITS POPUP BESIDE THE BODY.
        //
        // The fillet and the chamfer already did that, while the hole put its field right at the face
        // and the thread at the rim: that is, over what has to be aimed at next. The cuts had no anchor
        // at all - there was nowhere to show the offset field, although the command has that parameter.
        //
        // The shell, the draft, pushing and removing a face, thickening - all belong here. What helps
        // with aiming is not the popup but THE HANDLE: it sits on the geometry and covers nothing.
        4 | 5 | 6 | 7 | 17 | 18 | 23 | 24 | 25 | 26 | 27 | 28 | 29 | 30 | 31 | 32 | 33 | 34 => {
            let b = pc.stitch_parts.first().copied().or(pc.trim.keep.map(|(b, _)| b)).or(pc.gsel.faces_body).or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
            qymcad_ui_state::body_side_anchor(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, b, rect, &basis)
        }
        // datum plane (20): at the origin of the picked reference; point (21): at (x,y,z); axis (22): at
        // the origin of the reference or of the hand-typed one
        20 => {
            let p = pc.datum.plane_pick.as_ref().and_then(|sp| qymcad_ui_state::mirror_plane_world(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, sp)).map(|(o, _)| o).unwrap_or([0.0, 0.0, 0.0]);
            Some(scr.at(p).0)
        }
        21 => Some(scr.at([qymcad_ui_state::cmd_val(pc.cmd, "x"), qymcad_ui_state::cmd_val(pc.cmd, "y"), qymcad_ui_state::cmd_val(pc.cmd, "z")]).0),
        22 => {
            let p = if pc.datum.axis_mode == 1 { [qymcad_ui_state::cmd_val(pc.cmd, "ox"), qymcad_ui_state::cmd_val(pc.cmd, "oy"), qymcad_ui_state::cmd_val(pc.cmd, "oz")] } else { pc.datum.axis_ref.map(|(o, _)| o).unwrap_or([0.0, 0.0, 0.0]) };
            Some(scr.at(p).0)
        }
        // primitives: the popup goes BESIDE - from the body while editing (like the chamfer), from the
        // placement point while creating (it used to sit at the centre of the primitive and covered it).
        10..=15 => {
            if pc.cmd.edit.is_some() {
                if let Some(b) = qymcad_ui_state::selected_body(pc.project, pc.sel) {
                    if let Some(a) = qymcad_ui_state::body_side_anchor(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, b, rect, &basis) {
                        return Some(a);
                    }
                }
            }
            let p = scr.at(pc.prim.place.unwrap_or([0.0, 0.0, 0.0])).0;
            Some(Pos2::new(p.x + 90.0, p.y - 20.0)) // beside the placement point, not over the primitive
        }
        _ => None,
    }
}

pub fn start_remove_face_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return;
    }
    pc.cmd.open(pc.armed, 26, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    pc.cmd.params = vec![]; // no parameters: the operation is defined solely by which faces are picked
    *pc.status = qymcad_i18n::tr("msg-remove-face");
}

/// STITCH SHEETS: click the surfaces, then Enter. The operation has no number other than the tolerance -
/// a stitch either meets along the edges or it does not.
pub fn start_stitch_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    pc.cmd.open(pc.armed, 33, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    pc.stitch_parts.clear();
    pc.cmd.params = vec![qymcad_ui_state::CmdParam::new("f-stitch-tol", "tol", 0.01, 1e-6, 10.0)];
    *pc.status = qymcad_i18n::tr("msg-stitch");
}

/// TRIM A SURFACE: click THE part of the sheet that stays, then the tool body.
pub fn start_trim_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    pc.cmd.open(pc.armed, 34, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    pc.trim.keep = None;
    pc.trim.tool = None;
    pc.cmd.params = vec![];
    *pc.status = qymcad_i18n::tr("msg-trim");
}

/// APPLY THE FACE REPLACEMENT: the faces go in as a query (a description if there is one), the surface
/// as a body.
pub fn apply_surface_replace_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc.gsel.faces_body.or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    let surface = (*pc.repl_surface)?;
    if pc.gsel.faces.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-faces-not-found");
        return None;
    }
    let picks: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    let q = pc.gsel.described.clone().map(qymcad_core::refs::Ref::many).unwrap_or_else(|| qymcad_core::refs::Ref::picks(&picks));
    Some(pc.project.add_surface_replace(src, q, surface))
}

/// SPLIT A BODY: click a plane, a datum or a face, set the offset at the geometry, and Enter breaks the
/// body into independent pieces.
pub fn start_split_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return;
    }
    pc.cmd.open(pc.armed, 27, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed and returned on exit
    *pc.mode_3d = true;
    pc.split.plane = None;
    pc.cmd.params = vec![qymcad_ui_state::CmdParam::new("f-offset", "offset", 0.0, -100000.0, 100000.0)];
    *pc.status = qymcad_i18n::tr("msg-split-body");
}

/// SPLIT FACES: the same reference plane a body cut uses, but the body stays ONE - only the faces are
/// divided. That is how an area is marked out for painting or machining without breaking the part
/// apart.
pub fn start_split_face_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return;
    }
    pc.cmd.open(pc.armed, 29, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.split.plane = None;
    pc.cmd.params = vec![qymcad_ui_state::CmdParam::new("f-offset", "offset", 0.0, -100000.0, 100000.0)];
    *pc.status = qymcad_i18n::tr("msg-split-face");
}

/// A shell: the picked faces become open and the body becomes hollow (a multiple selection by id, plus
/// a direction).
pub fn apply_shell_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    // the source is the body the picked faces belong to; the faces are the multiple selection, by
    // persistent id. It falls back to the selection when the body of the faces is not pinned yet.
    let src = pc.gsel.faces_body.or_else(|| match *pc.sel {
        qymcad_ui_state::Sel::Face(mi, _) | qymcad_ui_state::Sel::Mesh(mi) => pc.project.mesh_id(mi),
        _ => qymcad_ui_state::selected_body(pc.project, pc.sel),
    }).or_else(|| qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }))?; // a part is one body; `qymcad_ui_state::current_body` falls back without entering it
    let faces: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    if faces.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-click-face-open");
        return None;
    }
    let body = pc.project.add_shell_mode(src, qymcad_ui_state::cmd_val(pc.cmd, "thickness"), faces, pc.opts.shell_side);
    store_cmd_exprs(pc.cmd, pc.project, body);
    Some(body)
}

/// REPLACE A FACE WITH A SURFACE: the node that sews the design layer to the timeline. Two picks in one
/// gesture - the faces of the base and the sheet; neither can be mistaken for anything else, so there is
/// no mode to switch between them.
pub fn start_surface_replace_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return;
    }
    pc.cmd.open(pc.armed, 31, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    *pc.repl_surface = None;
    pc.cmd.params = vec![];
    *pc.status = qymcad_i18n::tr("msg-surface-replace");
}

/// REMOVE A FACE AND HEAL: click the faces of an element (a hole, a boss), and Enter takes them away.
///
/// A FACE COPY: the faces of a body become a separate SURFACE while the body stays where it is. It is
/// the bridge from the parametric side into the design layer. There are no parameters - the operation is
/// defined solely by which faces are picked.
pub fn start_face_copy_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return;
    }
    pc.cmd.open(pc.armed, 30, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    pc.cmd.params = vec![];
    *pc.status = qymcad_i18n::tr("msg-face-copy");
}

/// THICKEN: click a face, set the thickness at the geometry, and Enter grows the face into A PLATE.
/// The plate is glued to the part, because a part is ONE body. As a separate body it was painted in a
/// different colour, and one part came out on screen as two.
pub fn start_thicken_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return;
    }
    pc.cmd.open(pc.armed, 28, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    pc.cmd.params = vec![qymcad_ui_state::CmdParam::new("f-thickness", "thickness", 2.0, -100000.0, 100000.0)];
    *pc.status = qymcad_i18n::tr("msg-thicken");
}

/// Apply the component array (Enter): create a new one or update the one being edited.
pub fn apply_comp_array(pc: &mut qymcad_ui_state::PartCtx) {
    let kind = qymcad_ui_state::comp_array_kind(*pc.arr, pc.carr, pc.cmd);
    qymcad_ui_state::begin_edit(pc.edits, pc.project, if pc.carr.edit != 0 { qymcad_i18n::tr("status-edit-comp-array") } else { qymcad_i18n::tr("f-comp-array") });
    let ok = if pc.carr.edit != 0 {
        pc.project.set_comp_pattern(pc.carr.edit, kind)
    } else {
        pc.project.add_comp_pattern(pc.carr.src, kind) != 0
    };
    *pc.status = if ok { qymcad_i18n::tr1("cmd-comp-array-done", "n", &pc.arr.count.max(1).to_string()) } else { qymcad_i18n::tr("msg-array-no-body") };
    *pc.carr = qymcad_ui_state::CompArrayCmd::default();
    pc.cmd.params.clear();
    qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild());
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// DRAFT: tilt the faces of a body relative to a neutral face. With the body selected, click the faces
/// to tilt (a multiple selection) and pick the reference with the neutral-face button, then the angle,
/// then Enter.
///
/// PUSH A FACE - direct modelling: click a flat face, set the offset, Enter.
///
/// The same command experience as everywhere: the bar on top, a field at the geometry (with an
/// expression), the face picked by click, Enter/Esc. A positive offset adds material, a negative one
/// cuts it away.
pub fn start_push_face_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return;
    }
    pc.cmd.open(pc.armed, 25, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed and returned on exit
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear(); // the face is picked afresh: a pre-selection would only confuse here
    pc.gsel.faces_body = None;
    pc.cmd.params = vec![qymcad_ui_state::CmdParam::new("f-offset2", "dist", 5.0, -100000.0, 100000.0)];
    *pc.status = qymcad_i18n::tr("msg-push-face");
}

/// A DRAFT: tilt the picked faces of the body relative to the neutral face (`draft_neutral`) by an
/// angle.
pub fn apply_draft_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc.gsel.faces_body.or_else(|| match *pc.sel {
        qymcad_ui_state::Sel::Face(mi, _) | qymcad_ui_state::Sel::Mesh(mi) => pc.project.mesh_id(mi),
        _ => qymcad_ui_state::selected_body(pc.project, pc.sel),
    }).or_else(|| qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }))?; // a part is one body; `qymcad_ui_state::current_body` falls back without entering it
    let faces: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    if faces.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-click-face-draft");
        return None;
    }
    if pc.draft.neutral == 0 {
        *pc.status = qymcad_i18n::tr("msg-pick-neutral");
        return None;
    }
    let body = pc.project.add_draft(src, faces, pc.draft.neutral, qymcad_ui_state::cmd_val(pc.cmd, "angle"), pc.draft.flip);
    store_cmd_exprs(pc.cmd, pc.project, body); // the angle stays parametric
    Some(body)
}

/// A THREAD: a real thread on a cylinder or inside a hole. With a body present, click a CYLINDRICAL
/// face; the top bar carries the side, the number of starts and the hand, while the popup at the
/// geometry carries the pitch, the length, the angle and the depth.
pub fn start_thread_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-cylinder");
        return;
    }
    pc.cmd.open(pc.armed, 24, *pc.mode_3d); // a clean slate, then open
    
    *pc.mode_3d = true;
    pc.thread.src = None;
    pc.thread.edge = 0;
    pc.thread.internal = false;
    pc.thread.starts = 1;
    pc.thread.left = false;
    pc.thread.form = 0;
    pc.thread.auger = false;
    qymcad_ui_state::set_thread_params(pc.cmd, *pc.thread);
    *pc.status = qymcad_i18n::tr("msg-thread");
}

/// A primitive as A COMMAND: 10 box, 11 cylinder, 12 sphere, 13 cone, 14 torus, 15 prism. The sizes are
/// expression fields at the geometry (the popup) plus a wireframe PREVIEW; Enter creates, Esc cancels.
/// By default it sits at the origin; a click on a vertex, a datum point, a plane or a face PLACES it
/// (orienting the base along the normal of the plane).
pub fn start_prim_cmd(pc: &mut qymcad_ui_state::PartCtx, code: u8) {
    pc.cmd.focus = true;
    pc.boolean.edit = None;
    pc.cmd.open(pc.armed, code, *pc.mode_3d); // a clean slate, then open
    
    *pc.mode_3d = true;
    pc.prim.n = 6;
    pc.prim.place = None;
    pc.prim.frame = None;
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    pc.gsel.edges.clear();
    // THE KEYS match those of the regen and of the feature properties (r/r1/r2/major/minor/dx/dy/dz/h -
    // RADII), otherwise the expressions would not bind (the regen reads `r`, not `dia`) and reopening
    // would go out of step.
    pc.cmd.params = match code {
        10 => vec![qymcad_ui_state::CmdParam::new("f-length-x", "dx", 20.0, 0.1, 100000.0), qymcad_ui_state::CmdParam::new("f-width-y", "dy", 20.0, 0.1, 100000.0), qymcad_ui_state::CmdParam::new("f-height-z", "dz", 20.0, 0.1, 100000.0)],
        11 => vec![qymcad_ui_state::CmdParam::new("f-radius", "r", 10.0, 0.05, 100000.0), qymcad_ui_state::CmdParam::new("f-height", "h", 20.0, 0.1, 100000.0)],
        12 => vec![qymcad_ui_state::CmdParam::new("f-radius", "r", 10.0, 0.05, 100000.0)],
        13 => vec![qymcad_ui_state::CmdParam::new("f-radius-bottom", "r1", 10.0, 0.0, 100000.0), qymcad_ui_state::CmdParam::new("f-radius-top", "r2", 0.0, 0.0, 100000.0), qymcad_ui_state::CmdParam::new("f-height", "h", 20.0, 0.1, 100000.0)],
        14 => vec![qymcad_ui_state::CmdParam::new("f-ring-r", "major", 12.0, 0.1, 100000.0), qymcad_ui_state::CmdParam::new("f-tube-r", "minor", 4.0, 0.1, 100000.0)],
        15 => vec![qymcad_ui_state::CmdParam::new("f-radius-circ", "r", 10.0, 0.1, 100000.0), qymcad_ui_state::CmdParam::new("f-height", "h", 20.0, 0.1, 100000.0)],
        _ => vec![],
    };
    *pc.status = qymcad_i18n::tr("msg-primitive");
}

/// LOFT (through sections) - a body through two or more sketch sections. The first section is the
/// selected sketch; after that, clicking sketches in the tree adds sections in order. Every section must
/// have a closed contour.
pub fn start_loft_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    let si = match *pc.sel {
        qymcad_ui_state::Sel::Sketch(si) => Some(si),
        _ => pc.cmd.sketch,
    };
    let Some(si) = si.filter(|&si| si < pc.project.sketches.len()) else {
        // WAITS LIKE THE REST. It used to write the line and return, and a line in the status bar is not an
        // answer to a click.
        *pc.picking = qymcad_ui_state::Picking::SketchFor(9);
        *pc.status = qymcad_i18n::tr("msg-loft-pick-first");
        return;
    };
    if qymcad_ui_state::sketch_closed_contours(pc.project, si).is_empty() {
        *pc.status = qymcad_i18n::tr("msg-section-no-contour");
        return;
    }
    pc.cmd.open(pc.armed, 9, *pc.mode_3d); // a clean slate, then open
    
    pc.cmd.sketch = Some(si);
    pc.loft.sids = vec![pc.project.sketches[si].id];
    pc.loft.cids = vec![0];
    pc.loft.ruled = false;
    pc.loft.pick = true; // the next sections are expected to be added by clicking in the tree
    pc.loft.pick_last = Some(pc.project.sketches[si].id); // the first section is already in the set - no duplicate
    pc.loft.result = 0; // by default a separate new body
    pc.cmd.params.clear();
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed for the command and returned on exit - no refitting
    *pc.mode_3d = true;
    *pc.status = qymcad_i18n::tr("msg-loft-pick");
}

/// SWEEP - a profile (the selected sketch with a closed contour) along a path (a second sketch, picked
/// in the tree). The profile usually lies on a plane at the start of the path, roughly perpendicular
/// to it.
pub fn start_sweep_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    // the profile is the sketch selected in the tree (or the one captured earlier)
    let si = match *pc.sel {
        qymcad_ui_state::Sel::Sketch(si) => Some(si),
        _ => pc.cmd.sketch,
    };
    let Some(si) = si.filter(|&si| si < pc.project.sketches.len()) else {
        *pc.picking = qymcad_ui_state::Picking::SketchFor(8); // waits like the rest
        *pc.status = qymcad_i18n::tr("msg-sweep-pick-profile");
        return;
    };
    if qymcad_ui_state::sketch_closed_contours(pc.project, si).is_empty() {
        *pc.status = qymcad_i18n::tr("msg-profile-no-contour");
        return;
    }
    pc.cmd.open(pc.armed, 8, *pc.mode_3d); // a clean slate, then open
    
    pc.feat.op = 0; // a sweep starts as "add" (the op is chosen in the bar)
    pc.cmd.sketch = Some(si);
    pc.sweep.prof_sid = pc.project.sketches[si].id;
    pc.sweep.path_sid = 0;
    pc.sweep.pick_path = true; // the path pick is expected straight away
    pc.cmd.params.clear();
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed for the command and returned on exit - no refitting
    *pc.mode_3d = true;
    *pc.status = qymcad_i18n::tr("msg-sweep-pick-path");
}

/// Extrude or revolve along the picked contours. Returns the last body created.
pub fn apply_sketch_cmd(pc: &mut qymcad_ui_state::PartCtx, cmd: u8) -> Option<Id> {
    let si = pc.cmd.sketch.filter(|&si| si < pc.project.sketches.len())?;
    let sid = pc.project.sketches[si].id;
    // THE CONTEXT: a feature built from a sketch belongs to the component that OWNS the sketch (the part)
    // rather than to the active context. Extruding from AN ASSEMBLY (or without having entered the part)
    // would make `body_parent()` create a NEW part and put the feature there while the sketch stayed in
    // the old one - a cross-component reference is forbidden, and the sketch would not extrude. The owner
    // of the sketch is made active, so the feature lands in its part and the body builds.
    if let Some(owner) = pc.project.sketch_owner(sid) {
        pc.project.set_active_component(Some(owner));
    }
    let closed = qymcad_ui_state::sketch_closed_contours(pc.project, si);
    let mut targets: Vec<Id> = closed.iter().copied().filter(|c| pc.gsel.profiles.contains(c)).collect();
    if targets.is_empty() {
        // no contours are picked explicitly, so THE WHOLE sketch (every closed contour) is extruded in ONE
        // operation. "Extrude the sketch" means the sketch entire; with two or more contours it used to
        // demand a click on every shape, which read as the sketch not extruding at all. Now all the
        // contours make one node, a multi-profile.
        if closed.is_empty() {
            *pc.status = qymcad_i18n::tr("msg-no-closed-contour");
            return None;
        }
        targets = closed.clone();
    }
    // NESTING IS NO LONGER FILTERED. Every picked contour is A REGION OF ITS OWN (itself minus its direct
    // children), and the regions are merged by the profile fuse in the core. qymcad_ui_state::Picking the outer and the
    // middle of three nested contours gives a plate with a hole the size of the small one; picking two
    // concentric circles gives a solid disc. It works at any depth. A nested picked contour used to be
    // thrown into the `fill` of the outer one, and everything deeper was lost - the part came out solid,
    // which was reported as not being able to extrude with a hole in the middle.
    let fill: Vec<Id> = Vec::new();
    // The target body is looked for IN THE CONTEXT OF THE SKETCH OWNER (where the feature will land)
    // rather than in the navigation context of the interface: without having entered the part by a double
    // click, a cut reported the part as empty and every extrude made a NEW body instead of the single
    // body of the part.
    let owner_body = pc.project.sketch_owner(sid).and_then(|o| pc.project.active_body(o));
    // A body is REQUIRED only for A CUT or AN INTERSECTION. A pad (op 1) with no body simply creates a new
    // one, as a join does in an empty part, rather than refusing silently with "build a body first".
    let need_target = (cmd == 1 || cmd == 3) && matches!(pc.feat.op, 2 | 3);
    let target = if need_target { owner_body.unwrap_or(0) } else { 0 };
    if need_target && target == 0 {
        *pc.status = format!("{} {}", ph::WARNING, qymcad_i18n::tr("cmd-cut-needs-body"));
        return None;
    }
    let reach = qymcad_ui_state::cmd_reach(pc.cmd, *pc.feat);
    // the second side is an expression field at the geometry (`qymcad_ui_state::cmd_val("down")`), not a drag value in the bar
    let down = if pc.cmd.extent.two_sided() { qymcad_ui_state::cmd_val(pc.cmd, "down").abs() } else { 0.0 };
    let through = pc.cmd.extent.through();
    let name = qymcad_i18n::name(&pc.project.sketches[si].name);
    pc.project.add_sketch_node(sid, name);
    let mut last = 0;
    // ALL the picked contours go through ONE operation and make ONE node (the core fuses N tools into a
    // single boolean against the body of the part). No chain of Extrude plus BodyBoolean or Combine, and
    // no collapsing.
    if cmd == 1 {
        let part = owner_body.unwrap_or(0); // the body of the part that OWNS the sketch (0 means an empty part, so create one)
        let h = qymcad_ui_state::cmd_val(pc.cmd, "height").abs();
        // console diagnostics ON REQUEST (`QYM_EXTRUDE_DEBUG=1`), as with the contour analysis. It used to
        // print unconditionally, and an ordinary session poured a line into stderr on every extrude.
        if std::env::var("QYM_EXTRUDE_DEBUG").is_ok() {
            eprintln!("[extrude] sid={sid} targets={targets:?} fill={fill:?} part={part} h={h} op={} reach={reach:?} down={down} through={through} active={:?}", pc.feat.op, pc.project.active_component);
        }
        last = if part == 0 {
            // AN EMPTY part gets A NEW body: an Extrude node plus `finish_base_body` - THE SAME path that
            // revolve takes (in an empty part `finish_base_body` simply returns the seed body). That makes
            // an extrude in an empty part behave like a revolve, which works, and like a sketch on an
            // existing body.
            let e = pc.project.add_extrude_multi(sid, targets.clone(), h, reach, down, fill.clone());
            if e != 0 {
                pc.project.finish_base_body(e, 1)
            } else {
                0
            }
        } else {
            // with a body present: add (0) becomes a pad (1); pad (1) stays 1; cut (2) becomes a cut (0);
            // intersect (3) becomes an intersection (2)
            let occt = match pc.feat.op {
                2 => 0,
                3 => 2,
                _ => 1,
            };
            pc.project.add_combine_multi_op(part, sid, targets.clone(), qymcad_core::model::CombineSpan { height: h, down: down, extent: qymcad_core::feature::Extent { through, reach }, fill: &fill }, occt)
        };
        if last != 0 {
            store_cmd_exprs(pc.cmd, pc.project, last); // the dimensions (the height) go onto the node of the operation
        } else {
            qymcad_ui_state::cmd_fail(pc.cmd_failed, pc.status, format!("{} {}", ph::WARNING, qymcad_i18n::tr("cmd-op-failed")));
        }
    } else {
        // A REVOLVE puts ALL the contours and the boolean into ONE node, exactly as an extrude does.
        //
        // There used to be a loop over the contours here: a `Revolve` per contour (which is a NEW body,
        // that is, an add) plus a `BodyBoolean` per cut. Two contours produced four timeline nodes, which
        // read as the revolve falling apart into two features that add instead of cutting; editing opened
        // only one contour, and deleting either of them took the whole chain below with it.
        let part = owner_body.unwrap_or(0);
        let occt = match pc.feat.op {
            2 => 0,
            3 => 2,
            _ => 1,
        };
        // A pad in AN EMPTY part is simply a new body: there is nothing to boolean against.
        let (rev_src, rev_op) = if part != 0 && matches!(pc.feat.op, 1..=3) { (part, occt) } else { (0, 1) };
        let body = pc.project.add_revolve_multi_op(sid, targets.clone(), qymcad_core::model::RevolveAxis { axis: pc.rev.axis, datum: pc.rev.axis_datum, line: pc.rev.axis_line }, qymcad_core::model::RevolveTurn { angle: qymcad_ui_state::cmd_val(pc.cmd, "angle"), reach: qymcad_ui_state::cmd_reach(pc.cmd, *pc.feat) }, rev_src, rev_op);
        if body != 0 {
            store_cmd_exprs(pc.cmd, pc.project, body);
            // a new body (nothing to boolean) is merged into the single body of the part, as with an extrude
            last = if rev_src == 0 { pc.project.finish_base_body(body, 1) } else { body };
        }
    }
    (last != 0).then_some(last)
}

pub fn apply_remove_face_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    if pc.gsel.faces.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-click-faces-remove");
        return None;
    }
    let keys: Vec<qymcad_core::feature::FaceKey> = pc
        .project
        .mesh_index(src)
        .and_then(|mi| pc.project.bodies.get(mi))
        .map(|b| {
            b.faces
                .iter()
                .filter(|f| pc.gsel.faces.contains(&f.id))
                .map(|f| qymcad_core::feature::FaceKey {
                    index: 0,
                    centroid: [f.centroid.x, f.centroid.y, f.centroid.z],
                    normal: f.normal,
                    id: f.id,
                })
                .collect()
        })
        .unwrap_or_default();
    if keys.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-faces-not-found");
        return None;
    }
    Some(pc.project.add_remove_face(src, keys))
}

/// Apply "push a face": one selected face plus an offset.
pub fn apply_push_face_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    // THE BODY IS TAKEN FROM THE FACE THAT WAS CLICKED rather than from the selection in the tree. When
    // the two diverged, the search for the face key failed and the tool SILENTLY did nothing: no node, no
    // error, no message - it reads as "pressed it, and nothing happened". Thicken was mended the same
    // way.
    let src = pc.gsel.faces_body.or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    let fid = pc.gsel.faces.iter().copied().next();
    let Some(fid) = fid else {
        *pc.status = qymcad_i18n::tr("msg-click-face-push");
        return None;
    };
    // the face key is taken from the built body: the offset is later resolved BY NAME, not by index
    let key = pc
        .project
        .mesh_index(src)
        .and_then(|mi| pc.project.bodies.get(mi))
        .and_then(|b| b.faces.iter().find(|f| f.id == fid))
        .map(|f| qymcad_core::feature::FaceKey {
            index: 0,
            centroid: [f.centroid.x, f.centroid.y, f.centroid.z],
            normal: f.normal,
            id: f.id,
        });
    // AND IF THE FACE WAS NOT FOUND AFTER ALL, SAY SO. A silent return from here was exactly that
    // "pressed it, and nothing": silence is indistinguishable from a broken program.
    let Some(key) = key else {
        *pc.status = qymcad_i18n::tr("msg-faces-not-found");
        return None;
    };
    let dist = qymcad_ui_state::cmd_val(pc.cmd, "dist");
    if dist.abs() < 1e-9 {
        *pc.status = qymcad_i18n::tr("msg-zero-offset");
        return None;
    }
    let body = pc.project.add_push_face(src, key, dist);
    store_cmd_exprs(pc.cmd, pc.project, body); // the offset stays parametric
    Some(body)
}

/// An array as A COMMAND: pick a body, then the top bar (the count, the direction, the axis), then the
/// STEP or ANGLE as an expression field AT THE GEOMETRY, then ghost previews of the copies, then Enter.
/// 17 is linear (a grid), 18 is circular.
pub fn start_array_cmd(pc: &mut qymcad_ui_state::PartCtx, cmd: u8) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return;
    }
    pc.cmd.open(pc.armed, cmd, *pc.mode_3d); // a clean slate, then open
    
    *pc.mode_3d = true;
    pc.arr.count = if cmd == 18 { 6 } else { 3 };
    pc.arr.dir = 0;
    pc.arr.two = false;
    pc.arr.count2 = 2;
    pc.arr.dir2 = 1;
    pc.arr.three = false;
    pc.arr.count3 = 2;
    pc.arr.dir3 = 2;
    pc.arr.axis = 0;
    pc.arr.full = true;
    pc.arr.axis_pick = false;
    // a full circular array asks no angle (the sync adds the field once "full" is switched off)
    pc.cmd.params = if cmd == 18 { vec![] } else { vec![qymcad_ui_state::CmdParam::new("f-pitch", "step", 25.0, 0.01, 100000.0)] };
    *pc.status = if cmd == 18 {
        qymcad_i18n::tr("msg-circ-array")
    } else {
        qymcad_i18n::tr("msg-lin-array")
    };
}

/// A command driven by a sketch (extrude or revolve): pick a profile, set the size on the canvas, Enter.
/// THE CLICK THAT NAMES BODY B for a body-to-body boolean.
///
/// `hit` is the body the click landed on, worked out by the caller: finding it needs the whole scene and
/// acting on it needs the document, and the two borrows cannot be held at once.
///
/// Moved out of the frame because it is a decision about the DOCUMENT - which two bodies are combined -
/// and the frame's business is only where the click landed.
pub fn take_boolean_pick(pc: &mut qymcad_ui_state::PartCtx, hit: Option<Id>) {
    let Some((a, op)) = pc.boolean.pick else { return };
    match hit {
        Some(b) if b != a => {
            let res = pc.project.add_body_boolean(a, b, op);
            pc.boolean.pick = None;
            qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the scheduler does the computing
            let (project, sel, view) = (&mut *pc.project, &mut *pc.sel, &mut *pc.view);
            qymcad_ui_state::select_body(project, sel, view, res);
            *pc.status = qymcad_i18n::tr("vp-bool-created");
        }
        Some(_) => *pc.status = qymcad_i18n::tr("vp-this-is-a"),
        None => *pc.status = qymcad_i18n::tr("vp-miss-body-b"),
    }
}

/// THE CLICK THAT NAMES THE SKETCH a waiting tool asked for.
///
/// `hit` is what the click landed on, worked out by the caller: the answer needs the whole scene and the
/// act needs the document, and the two borrows cannot be held at once.
///
/// A MISS SAYS SO rather than putting the tool down. Pointing at nothing is an aim that missed, not a
/// change of mind; putting the tool down on it would make every stray click cancel the command.
pub fn name_the_sketch(pc: &mut qymcad_ui_state::PartCtx, hit: Option<usize>) {
    match hit {
        Some(si) => *pc.sel = qymcad_ui_state::Sel::Sketch(si), // the frame's own step continues the command
        None => *pc.status = qymcad_i18n::tr("msg-pick-sketch-first"),
    }
}

/// A COMMAND WAITING FOR A SKETCH TAKES THE FIRST ONE THAT GETS SELECTED.
///
/// ONE PLACE, called once a frame after the panels have drawn. A sketch can be picked in the tree or in
/// the viewport, and putting "if something is waiting, continue it" into both would be two copies of one
/// decision - they drift apart at the first edit of one of them. Here the wait does not care where the
/// selection came from.
pub fn take_sketch_if_waiting(pc: &mut qymcad_ui_state::PartCtx) {
    let Some(kind) = pc.picking.sketch_for() else { return };
    let qymcad_ui_state::Sel::Sketch(_) = *pc.sel else { return };
    pc.picking.clear(); // cleared BEFORE the command starts: it opens its own picking, and a stale wait would outlive it
    // BY KIND, because the four tools that need a sketch open through three different doors. Sending them
    // all to `start_sketch_cmd` would open an extrude where a loft was asked for.
    match kind {
        8 => start_sweep_cmd(pc),
        9 => start_loft_cmd(pc),
        k => start_sketch_cmd(pc, k),
    }
}

pub fn start_sketch_cmd(pc: &mut qymcad_ui_state::PartCtx, cmd: u8) {
    let si = match *pc.sel {
        qymcad_ui_state::Sel::Sketch(si) => Some(si),
        _ => pc.cmd.sketch,
    };
    let Some(si) = si.filter(|&si| si < pc.project.sketches.len()) else {
        // THE TOOL IS TAKEN IN HAND AND WAITS instead of not starting. It used to write the line below and
        // return, and a line in the status bar is not an answer to a click: what a person sees is that they
        // pressed and nothing happened. The wait is answered by `take_sketch_if_waiting`.
        *pc.picking = qymcad_ui_state::Picking::SketchFor(cmd);
        *pc.status = qymcad_i18n::tr("msg-pick-sketch-first");
        return;
    };
    let closed = qymcad_ui_state::sketch_closed_contours(pc.project, si);
    if closed.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-no-closed-contour");
        return;
    }
    let was_3d = pc.cmd.prev_3d;
    pc.cmd.open(pc.armed, cmd, was_3d); // the checks passed - the command is open
    pc.cmd.edit = None; // a new feature, not an edit
    if cmd == 3 {
        pc.feat.op = 0; // revolve starts as "add" (for extrude the op is set by the tool button)
    }
    // CLEARING THE STALE STATE OF THE PREVIOUS COMMAND: "through", symmetry and the second side must NOT
    // travel silently into the new one (a through cut followed by an extrude in an empty part built a
    // 2000 mm body or broke silently). A new command means a clean one-sided extent.
    pc.cmd.extent = qymcad_ui_state::ExtentMode::default();
    // a sensible default for the direction: a CUT on a face of the part goes INTO the body (the negative
    // normal), otherwise it would cut outwards into the void. Everything else goes outwards (the
    // positive normal); dragging the gizmo or pressing flip reverses it, and the preview equals the
    // result. The command opens with the direction still automatic - it is computed on apply.
    *pc.feat = qymcad_ui_state::FeatTarget::opened(pc.feat.op);
    pc.cmd.sketch = Some(si);
    pc.gsel.profiles.clear();
    pc.rev.axis_datum = 0; // by default the X or Y of the sketch
    pc.rev.axis_line = 0; // no centre line is picked
    pc.rev.pick_axis = false; // the axis-picking sub-mode is off
    pc.rev.pick_line = false;
    pc.cmd.params = match cmd {
        3 => vec![qymcad_ui_state::CmdParam::new("f-angle", "angle", pc.rev.angle, 1.0, 360.0)],
        _ => vec![qymcad_ui_state::CmdParam::new("f-length", "height", pc.set.defaults.extrude_h.max(0.1), 0.1, 10000.0)],
    };
    // ALL closed contours are selected - the sketch is extruded WHOLE, in one operation - so the 3D
    // preview and the gizmo appear at once. With two or more contours this used to fall back to a flat
    // pick by click, and extruding a sketch did not work.
    for &c in &closed {
        pc.gsel.profiles.insert(c);
    }
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed for the command and returned on exit - no refitting
    *pc.mode_3d = true;
    let what = if cmd == 3 { qymcad_i18n::tr("f-revolve") } else { qymcad_i18n::tr("f-extrude") };
    *pc.status = qymcad_i18n::tr1("cmd-pick-contours-hint", "what", &what);
    // WITH SOMETHING TO CHOOSE, THE COMMAND ASKS BEFORE IT SHOWS A SIZE.
    //
    // Reported behaviour: to change which contours are used a person had to find the "Pick contours"
    // button, or U/Alt+U, having already been shown a body built out of ALL of them. Now the creation of a
    // feature starts where the choice is - the flat view, where a click lands on a contour.
    //
    // ONLY WITH TWO OR MORE, and only when CREATING. A picker over a single candidate is a step that can
    // end one way, which is worse than no step at all; and an edit has already been told which contours the
    // feature is made of, so it opens on the size, with re-picking left on the button.
    if closed.len() > 1 {
        enter_contour_reselect(pc);
    }
}

/// CREATE A PRIMITIVE from the fields of the running command.
///
/// The closure here used to take `&Self` for one reason: it needed `cmd` while `project` was borrowed
/// mutably in the same expression, and naming the whole application was the way round it. Over a context
/// the two are separate places, so the closure takes the command alone and the document stays free.
pub fn apply_prim_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let cmd = pc.cmd.clone();
    let v = |k: &str| qymcad_ui_state::cmd_val(&cmd, k);
    let body = match pc.armed.cmd_kind() {
        10 => pc.project.add_box(v("dx"), v("dy"), v("dz")),
        11 => pc.project.add_cylinder(v("r"), v("h")),
        12 => pc.project.add_sphere(v("r")),
        13 => pc.project.add_cone(v("r1"), v("r2"), v("h")),
        14 => pc.project.add_torus(v("major"), v("minor")),
        15 => pc.project.add_prism(v("r"), pc.prim.n.max(3), v("h")),
        _ => return None,
    };
    store_cmd_exprs(&cmd, pc.project, body);
    // placement: an oriented move of the primitive into the picked frame (the base sits on the surface)
    let placed = match pc.prim.frame {
        Some(m) if !qymcad_ui_state::is_identity12(&m) => pc.project.add_move(body, m),
        _ => body,
    };
    // A PART IS ONE BODY - the primitive is merged into the single body of the part (the first one seeds it)
    Some(pc.project.finish_base_body(placed, 1))
}

/// Update existing feature `fid` with the parameters of the active command (edit mode).
pub fn update_feat(pc: &mut qymcad_ui_state::PartCtx, fid: Id) -> Option<Id> {
    use qymcad_core::feature::FeatureKind;
    // THE DATUMS edit `project.planes`, `points` and `axes` plus the feature dims rather than
    // `node.kind`, so they take a separate path
    if matches!(pc.armed.cmd_kind(), 20..=22) {
        return update_datum_feat(pc.armed, pc.cmd, pc.datum, pc.edges, pc.project, fid);
    }
    // take everything out of the command state in advance, to avoid borrowing self twice
    let (h, ang, r, dist, th, dia, dep) = (qymcad_ui_state::cmd_val(pc.cmd, "height"), qymcad_ui_state::cmd_val(pc.cmd, "angle"), qymcad_ui_state::cmd_val(pc.cmd, "radius"), qymcad_ui_state::cmd_val(pc.cmd, "dist"), qymcad_ui_state::cmd_val(pc.cmd, "thickness"), qymcad_ui_state::cmd_val(pc.cmd, "diameter"), qymcad_ui_state::cmd_val(pc.cmd, "depth"));
    // the "vertex -> radius" table is taken BEFORE the node is edited (otherwise self is borrowed twice)
    let vtable: Vec<(qymcad_core::refs::Ref, f64)> = fillet_vertex_table(pc.cmd)
        .into_iter()
        .map(|(desc, val)| (qymcad_core::refs::Ref::one(desc, qymcad_core::refs::Fingerprint::default()), val))
        .collect();
    let dist_v = qymcad_ui_state::cmd_val(pc.cmd, "dist"); // "push a face": the offset, which may be an expression
    let ch_d2 = qymcad_ui_state::cmd_val(pc.cmd, "d2"); // the second leg or the angle of the chamfer, in degrees
    let (ch_mode, ch_flip) = (pc.chamfer.mode, pc.chamfer.flip); // the mode plus the side of the reference face
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
    let faces_set: Vec<u32> = pc.gsel.faces.iter().copied().collect(); // the shell: a multiple selection by id
    let shell_side = pc.opts.shell_side; // the shell: which way the wall goes
    let draft_neutral = pc.draft.neutral; // the draft: the neutral face, 0 means unset
    let draft_flip = pc.draft.flip; // the draft: the direction of the pull
    // the mirror: the picked plane resolves to (plane, datum); a face creates a datum plane
    let mirror_resolved: Option<(u8, Id)> = pc.mirror.plane.map(|sp| qymcad_ui_state::resolve_mirror_plane(pc.project, sp));
    let mirror_keep = pc.opts.mirror_keep;
    // THE CUT: the new plane plus how many pieces it will yield. It is computed BEFORE the timeline is
    // borrowed; if it disagrees with the number of pieces the feature has, the edit is rejected below -
    // the bodies of the cut have already spread through the timeline (they are referred to and they are
    // visible), and their number cannot be changed in place.
    let (split_resolved, split_pieces) = if pc.armed.cmd_kind() == 27 || pc.armed.cmd_kind() == 29 {
        let n = (pc.armed.cmd_kind() == 27).then(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).and_then(|src| split_piece_count(pc, src))).flatten();
        let r = pc.split.plane.map(|sp| qymcad_ui_state::resolve_mirror_plane(pc.project, sp));
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
    let (p_r, p_r1, p_r2, p_ph, p_major, p_minor) = (qymcad_ui_state::cmd_val(pc.cmd, "r"), qymcad_ui_state::cmd_val(pc.cmd, "r1"), qymcad_ui_state::cmd_val(pc.cmd, "r2"), qymcad_ui_state::cmd_val(pc.cmd, "h"), qymcad_ui_state::cmd_val(pc.cmd, "major"), qymcad_ui_state::cmd_val(pc.cmd, "minor"));
    let p_n = pc.prim.n.max(3);
    // if ANOTHER face is picked while editing, it is updated on the feature. Only for the hole: the face
    // goes in by its persistent `FaceKey` (the shell is now a multiple selection by id - `faces_set`
    // above)
    let hole_face: Option<(qymcad_core::feature::FaceKey, [f64; 3], [f64; 3])> = match *pc.sel {
        qymcad_ui_state::Sel::Face(mi, fi) => pc.project.bodies.get(mi).and_then(|b| b.faces.get(fi)).map(|face| {
            let k = qymcad_core::feature::FaceKey { index: fi as u32, centroid: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, id: face.id };
            (k, k.centroid, k.normal)
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
                *e = qymcad_core::refs::Ref::picks(&edges); // a hand-picked set of edges is a query built from ids
                *at_vertices = vtable; // the "vertex -> radius" table
            }
            FeatureKind::Chamfer { dist: d, edges: e, mode, d2, flip, ref_face, .. } => {
                *d = dist;
                *e = qymcad_core::refs::Ref::picks(&edges); // a hand-picked set of edges is a query built from ids
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
            FeatureKind::Thicken { face, thickness, .. } => {
                *thickness = thickness_v;
                if let Some(&id) = faces_set.first() {
                    *face = id; // the face can be reassigned by a click, without recreating the feature
                }
            }
            FeatureKind::SplitFace { plane, datum, offset, .. } => {
                if let Some((pl, dt)) = split_resolved {
                    *plane = pl;
                    *datum = dt;
                }
                *offset = split_offset;
            }
            FeatureKind::SplitBody { plane, datum, offset, bodies, .. } => {
                // THE NUMBER OF PIECES MUST NOT CHANGE: later features may already refer to the existing
                // bodies, and creating or removing a body through an edit would break those references
                // silently. An honest refusal leaves the cut to be recreated by hand.
                if let Some(n) = split_pieces {
                    if n != bodies.len() {
                        bad_pieces = Some((n, bodies.len()));
                    }
                }
                if bad_pieces.is_none() {
                    if let Some((pl, dt)) = split_resolved {
                        *plane = pl;
                        *datum = dt;
                    }
                    *offset = split_offset;
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
            FeatureKind::Mirror { plane, keep, datum, .. } => {
                *keep = mirror_keep;
                if let Some((pl, dt)) = mirror_resolved {
                    *plane = pl;
                    *datum = dt;
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
        *pc.status = qymcad_i18n::tr2("cmd-split-count-changed", "got", &got.to_string(), "had", &had.to_string());
        return None;
    }
    store_cmd_exprs(pc.cmd, pc.project, fid); // store or clear the dimension expressions (the logical step, step2 and angle included)
    // the array: the step expressions go onto the components of the vector (the regen reads those), and the
    // angle is cleared for a full circle
    match pc.armed.cmd_kind() {
        17 => {
            qymcad_ui_state::store_arr_component(pc.project, fid, ["dx", "dy", "dz"], a_dir, a_step_txt.clone());
            if a_two {
                qymcad_ui_state::store_arr_component(pc.project, fid, ["dx2", "dy2", "dz2"], a_dir2, a_step2_txt.clone());
            } else {
                for k in ["dx2", "dy2", "dz2"] {
                    pc.project.set_feat_dim(fid, k, String::new());
                }
            }
            if a_two && a_three {
                qymcad_ui_state::store_arr_component(pc.project, fid, ["dx3", "dy3", "dz3"], a_dir3, a_step3_txt.clone());
            } else {
                for k in ["dx3", "dy3", "dz3"] {
                    pc.project.set_feat_dim(fid, k, String::new());
                }
            }
        }
        18
            if a_full => {
                pc.project.set_feat_dim(fid, "angle", String::new());
            }
        _ => {}
    }
    Some(fid)
}

pub fn apply_feat_cmd_inner(pc: &mut qymcad_ui_state::PartCtx) {
    // THE DERIVED VALUE IS COMPUTED HERE rather than when the command opens: the operation may have
    // been switched in the bar after opening (see `smart_flip`).
    if pc.feat.flip_auto {
        if let Some(si) = pc.cmd.sketch.filter(|i| *i < pc.project.sketches.len()) {
            pc.feat.flip = smart_flip(pc.armed.cmd_kind(), pc.feat.op, &pc.project.sketches[si].plane);
        }
    }
    qymcad_ui_state::ensure_brep(&mut pc.rebuild()); // after opening from a bundle there are no live shapes yet - bring the cache up once
    // A sketch command (extrude or revolve) takes two steps when SEVERAL contours are picked: while we
    // are in the flat half-sketcher (`mode_3d = false`) and the profiles ARE picked, Apply or Enter first
    // CONFIRMS the selection and moves to 3D (the gizmo plus the size popup) rather than creating the
    // feature silently at the default height. Otherwise the Apply button (which is enabled as soon as the
    // profile selection is non-empty) would apply 10 mm straight away during a Ctrl multi-pick, with no
    // gizmo and no popup, and would breed a chain of nodes. A single contour already moves to 3D at the
    // start.
    if matches!(pc.armed.cmd_kind(), 1 | 3) && pc.cmd.edit.is_none() && !*pc.mode_3d && !pc.gsel.profiles.is_empty() {
        qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed for the command and returned on exit
        *pc.mode_3d = true;
        *pc.status = qymcad_i18n::tr("msg-profile-picked");
        return;
    }
    // a hard backstop: DO NOT apply while any dimension carries an invalid expression (a stale value
    // would go through otherwise). The gate is duplicated on the buttons, but Enter can arrive past
    // them.
    if !cmd_exprs_valid(pc.cmd, pc.project) {
        *pc.status = qymcad_i18n::tr1("cmd-expr-error", "icon", ph::X);
        return;
    }
    // editing an existing feature (a double click) updates it rather than creating a new one
    let last = if let Some(fid) = pc.cmd.edit {
        update_feat(pc, fid)
    } else {
        match pc.armed.cmd_kind() {
            1 | 3 => apply_sketch_cmd(pc, pc.armed.cmd_kind()),
            8 => apply_sweep_cmd(*pc.feat, pc.project, pc.status, *pc.sweep),
            9 => apply_loft_cmd(pc),
            4 | 5 => apply_edge_cmd(pc, pc.armed.cmd_kind()),
            6 => apply_shell_cmd(pc),
            23 => apply_draft_cmd(pc),
            25 => apply_push_face_cmd(pc),
            26 => apply_remove_face_cmd(pc),
            30 => apply_face_copy_cmd(pc),
            31 => apply_surface_replace_cmd(pc),
            32 => apply_patch_cmd(pc),
            33 => apply_stitch_cmd(pc.cmd, pc.project, pc.status, pc.stitch_parts),
            34 => apply_trim_cmd(pc.project, pc.status, pc.trim),
            27 => apply_split_cmd(pc),
            28 => apply_thicken_cmd(pc),
            29 => apply_split_face_cmd(pc),
            24 => apply_thread_cmd(pc.cmd, pc.project, pc.status, *pc.thread),
            7 => apply_hole_cmd(pc.cmd, *pc.hole, pc.project, *pc.sel, pc.status),
            10..=15 => apply_prim_cmd(pc), // the primitives
            16 => apply_mirror_cmd(pc),   // the mirror
            17 | 18 => apply_array_cmd(pc), // the array
            20..=22 => apply_datum_cmd(pc.armed, pc.cmd, pc.datum, pc.edges, pc.project, pc.status), // a datum plane, point or axis
            _ => None,
        }
    };
    let Some(_body) = last else { return }; // on failure we stay in the command (the status is already set)
    // A CLEAN finish: the command is dropped and the selection is cleared, so neither the gizmo arrows
    // nor the right-hand panel remain
    pc.cmd.open(pc.armed, 0, *pc.mode_3d); // a clean slate, then open
    
    qymcad_ui_state::clear_feat_picks(qymcad_ui_state::feat_picks_in!(pc));
    // back from the flat half-sketcher into 3D: RETURN the view that was borrowed rather than refitting
    // it. Refitting threw away everything that had been set up by hand, and finishing a command looked
    // as if the viewport had flown off.
    qymcad_ui_state::return_view(pc);
    *pc.status = qymcad_i18n::tr("f-done");
    // AFTER the status: if the regen fails, `qymcad_ui_state::regenerate_all` writes its own rebuild message - do NOT
    // overwrite it with "done" (otherwise the feature silently fails to build while the screen says
    // "done", which reads as nothing happening at all).
    qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the planner does the counting
    *pc.sel = qymcad_ui_state::Sel::None;
}

/// Open the HALF-SKETCHER for picking the contour of a sweep or loft slot (`sid` is the slot's sketch).
/// The same mechanism as in Extrude: the sketch is shown flat and a click on a contour fills the slot
/// (see the 2D click handler and `set_contour_slot`). It replaces cycling through contours with arrows.
pub fn begin_contour_pick(pc: &mut qymcad_ui_state::PartCtx, slot: qymcad_ui_state::ContourSlot, sid: Id) {
    let Some(si) = pc.project.sketch_index(sid) else {
        *pc.status = qymcad_i18n::tr("g-slot-sketch-missing");
        return;
    };
    *pc.picking = qymcad_ui_state::Picking::Contour(slot);
    pc.cmd.sketch = Some(si);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore); // the view is borrowed while the contour is picked and restored on the way out
    *pc.mode_3d = false;
    pc.view.initialized = false;
    pc.cmd.focus = false;
    let what = match slot {
        qymcad_ui_state::ContourSlot::SweepProfile => "g-slot-profile",
        qymcad_ui_state::ContourSlot::SweepPath => "g-slot-path",
        qymcad_ui_state::ContourSlot::LoftSection(_) => "g-slot-section",
    };
    *pc.status = qymcad_i18n::tr1("g-contour-pick-of", "what", &qymcad_i18n::tr(what));
}

pub fn enter_contour_reselect(pc: &mut qymcad_ui_state::PartCtx) {
    if !matches!(pc.armed.cmd_kind(), 1 | 3) || pc.cmd.sketch.is_none() {
        return;
    }
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore); // the view is borrowed while contours are picked and restored on the way out
    *pc.mode_3d = false; // back to the flat half-sketcher: a click on a contour changes the set of profiles
    pc.view.initialized = false;
    pc.cmd.focus = false;
    *pc.status = qymcad_i18n::tr("g-contour-reselect");
}

/// The straight edges of ALL visible (unconsumed) bodies go into `axis_edges` (the body plus a local
/// polyline), so that the axis of a pattern or a datum can be picked on an edge of ANY body rather than
/// only the selected one. Called when the pick begins.
pub fn refresh_axis_edges(pc: &mut qymcad_ui_state::PartCtx) {
    qymcad_ui_state::ensure_brep(&mut pc.rebuild()); // the candidates are edges of the LIVE B-rep; without it the list is empty and there is nothing to click
    pc.edges.axes.clear();
    let consumed = qymcad_ui_state::consumed_bodies(&*pc.project);
    for mi in 0..pc.project.bodies.len() {
        if !qymcad_ui_state::body_shown(qymcad_ui_state::body_view_in!(pc), mi) {
            continue;
        }
        let Some(b) = pc.project.mesh_id(mi) else { continue };
        if consumed.contains(&b) {
            continue;
        }
        let Some(shape) = pc.live.shapes.get(&b) else { continue };
        let (polys, ids) = shape.edges_with_ids();
        for (poly, id) in polys.into_iter().zip(ids) {
            if id != 0 && qymcad_ui_state::is_straight_poly(&poly) {
                pc.edges.axes.push((b, id, poly));
            }
        }
    }
}

pub fn apply_feat_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    // THE BOUNDARY OF THE OPERATION: everything the command does to the document is one undo step with
    // a name of its own. It opens HERE rather than in `start_feat_cmd`: opening a dialog does not change
    // the document and there is nothing to undo there. It closes at the end: success commits, a refusal
    // rolls back and leaves no trace.
    let name = feat_cmd_name(&*pc.armed, *pc.feat);
    // OPENED AND CLOSED BY NAME, without the guard. `qymcad_ui_state::commit_edit` counts the nesting itself
    // (`edits.depth`), so the boundary holds the same as before; what goes is the guard's hold on the
    // WHOLE application, which existed for this one call and no other.
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, name);
    *pc.cmd_failed = false;
    apply_feat_cmd_inner(pc);
    // A FACT, NOT A GUESS FROM TEXT. This used to search the status line for a substring meaning "did
    // not succeed" - checking what was written for a person instead of what actually happened. Once the
    // interface was translated the status would no longer match, and a failed operation would land in
    // the undo history as a successful one: Ctrl+Z then rolls back the wrong thing.
    if *pc.cmd_failed {
        qymcad_ui_state::abort_edit(&mut pc.rebuild());
    } else {
        qymcad_ui_state::commit_edit(&mut pc.rebuild());
    }
}

/// Apply the world transform `accum` to the body of mesh `mi`: if the body is already a Move feature,
/// accumulate into its matrix (no chain of Move nodes); otherwise create a new Move for a B-rep, or
/// shift a raw mesh directly.
pub fn apply_body_move(pc: &mut qymcad_ui_state::PartCtx, mi: usize, accum: [f64; 12]) {
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("status-move-body")); // THE BOUNDARY OF AN OPERATION
    use qymcad_core::feature::FeatureKind;
    let Some(body) = pc.project.mesh_id(mi) else { return };
    let is_move = pc.project.timeline.iter().any(|n| n.id == body && matches!(n.kind, FeatureKind::Move { .. }));
    if is_move {
        pc.project.accumulate_move(body, &accum);
        qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the planner does the counting
        qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, body);
    } else {
        qymcad_ui_state::move_body_at(&mut pc.rebuild(), mi, accum); // a B-rep gets a new Move feature (and selection); a raw mesh is shifted directly
    }
    qymcad_ui_state::after_placement_change(&mut pc.rebuild()); // the source moved -> rebuild whoever consumes it
        qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// A PATCH: stretch a surface over the picked edges. The first tool of this layer that creates a shape
/// the body did not have.
pub fn start_patch_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return;
    }
    pc.cmd.open(pc.armed, 32, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.faces.clear();
    pc.gsel.edges.clear();
    refresh_edges(pc);
    pc.cmd.params = vec![];
    *pc.status = qymcad_i18n::tr("msg-patch");
}

pub fn start_feat_cmd_edit(pc: &mut qymcad_ui_state::PartCtx, fid: Id) {
    use qymcad_core::feature::FeatureKind;
    let Some(node) = pc.project.timeline.iter().find(|n| n.id == fid).cloned() else { return };
    // EDITING AN EXISTING FEATURE is the same command, only with its parameters restored. It opens
    // through the same life cycle (`open` on every branch below) rather than by assigning a field:
    // otherwise the remains of the previous command seep into the edit. The view from BEFORE the edit is
    // remembered here - `open` carries it.
    let was_3d = *pc.mode_3d;
    pc.boolean.edit = None; // close the previous boolean edit mode (the arm below turns it on again)
    pc.boolean.pick = None;
    pc.gsel.profiles.clear();
    pc.gsel.edges.clear();
    match node.kind {
        FeatureKind::Extrude { sketch, ref profiles, height, reach, down, ref fill, .. } => {
            pc.cmd.open(pc.armed, 1, was_3d);
            pc.feat.op = 0;
            pc.feat.flip = reach == qymcad_core::feature::Reach::Backward; // restore the direction
            pc.cmd.sketch = pc.project.sketch_index(sketch);
            // ALL contours of the operation sit in the node itself (`profiles`), with no walking of the
            // chain. Editing means all of the contours.
            for &p in profiles {
                if p != 0 {
                    pc.gsel.profiles.insert(p);
                }
            }
            for &f in fill {
                pc.gsel.profiles.insert(f); // the filled inner ones are picked too (otherwise only one circle remains)
            }
            pc.cmd.extent = qymcad_ui_state::ExtentMode::from_extent(reach, down, false);
            if down.abs() > 1e-9 {
                pc.cmd.down = down;
            }
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-length", "height", height, 0.1, 10000.0)];
        }
        FeatureKind::Combine { sketch, ref profiles, height, op, extent, down, ref fill, .. } => {
            pc.cmd.open(pc.armed, 1, was_3d);
            pc.feat.op = match op {
                1 => 1,
                2 => 3,
                _ => 2,
            };
            pc.feat.flip = extent.reach == qymcad_core::feature::Reach::Backward; // restore the direction of the tool
            pc.cmd.sketch = pc.project.sketch_index(sketch);
            // ALL contours of the operation sit in the node itself (`profiles`), with no walking of the
            // chain. Editing means all of the contours.
            for &p in profiles {
                if p != 0 {
                    pc.gsel.profiles.insert(p);
                }
            }
            for &f in fill {
                pc.gsel.profiles.insert(f); // the filled inner ones are picked too
            }
            // restore the extent mode: through, symmetric, two-sided or to a length
            pc.cmd.extent = qymcad_ui_state::ExtentMode::from_extent(extent.reach, down, extent.through);
            if down.abs() > 1e-9 {
                pc.cmd.down = down;
            }
            // `sync_dir_cmd_params` adds the second-side field at the geometry, with the stored expression
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-depth", "height", height.abs(), 0.1, 10000.0)];
        }
        FeatureKind::Revolve { sketch, ref profiles, axis, angle, axis_datum, axis_line, reach, op, .. } => {
            pc.cmd.open(pc.armed, 3, was_3d);
            use qymcad_core::feature::Reach;
            pc.cmd.extent = if reach == Reach::BothWays { qymcad_ui_state::ExtentMode::Symmetric } else { qymcad_ui_state::ExtentMode::Length };
            pc.feat.flip = reach == Reach::Backward;
            pc.rev.axis = axis;
            pc.rev.axis_datum = axis_datum;
            pc.rev.axis_line = axis_line;
            pc.rev.pick_axis = false;
            pc.cmd.sketch = pc.project.sketch_index(sketch);
            // ALL contours of the node go into the selection: editing a multi-contour feature must open it
            // whole, otherwise Apply quietly reduces two areas to one.
            for c in profiles {
                pc.gsel.profiles.insert(*c);
            }
            // the core op (0 = cut, 1 = pad, 2 = intersect) maps to the bar switch (0 = add, 2 = cut,
            // 3 = intersect)
            pc.feat.op = match op {
                0 => 2,
                2 => 3,
                _ => 0,
            };
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-angle", "angle", angle, 1.0, 360.0)];
        }
        FeatureKind::Sweep { sketch, ref profiles, path_sketch, path, op, .. } => {
            // reopen the sweep: restore the profile, the path and the picked contours, with no picking
            pc.cmd.open(pc.armed, 8, was_3d);
            pc.cmd.sketch = pc.project.sketch_index(sketch);
            pc.sweep.prof_sid = sketch;
            pc.sweep.path_sid = path_sketch;
            pc.sweep.prof_cid = profiles.first().copied().unwrap_or(0);
            for c in profiles {
                pc.gsel.profiles.insert(*c);
            }
            pc.feat.op = match op {
                0 => 2,
                2 => 3,
                _ => 0,
            };
            pc.sweep.path_cid = path;
            pc.sweep.pick_path = false;
            pc.cmd.params.clear();
        }
        FeatureKind::Loft { sketches, contours, ruled, src, op, .. } => {
            // reopen the loft: restore the set of sections and the picked contours, with no picking
            pc.cmd.open(pc.armed, 9, was_3d);
            pc.cmd.sketch = sketches.first().and_then(|&s| pc.project.sketch_index(s));
            pc.loft.sids = sketches.clone();
            pc.loft.cids = (0..sketches.len()).map(|i| contours.get(i).copied().unwrap_or(0)).collect();
            pc.loft.ruled = ruled;
            pc.loft.result = if src == 0 { 0 } else { op + 1 }; // 0 = a new body; otherwise cut, union or intersect
            pc.loft.pick = false;
            pc.loft.pick_last = None;
            pc.cmd.params.clear();
        }
        FeatureKind::Fillet { src, radius, ref edges, ref at_vertices, .. } => {
            pc.cmd.open(pc.armed, 4, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            refresh_edges(pc); // pull up the edges of the body (this clears the selection), then restore it
            pc.gsel.edges = live_picks(&*pc.project, src, edges, false);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-radius", "radius", radius, 0.05, 1000.0)];
            // THE TABLE OF VERTICES - one field per vertex, each at its own place. The reference is
            // resolved against the live body: the name of a vertex is derived from its edges and survives
            // edits to its neighbours.
            let table: Vec<(qymcad_core::refs::Ref, f64)> = at_vertices.clone();
            for (r, val) in table {
                let Ok(found) = pc.project.resolve_vertex_refs(src, &r, "ref-what-fillet-vertex") else { continue };
                let Some(desc) = found.first().copied() else { continue };
                let Some(p) = pc.project.vertex_point(src, desc) else { continue };
                let key = format!("at{desc}");
                let prm = cmd_param_from(&*pc.project, fid, "f-radius-at-vertex", &key, val, 0.0, 1000.0);
                pc.cmd.params.push(prm.at(p));
            }
        }
        FeatureKind::Chamfer { src, dist, ref edges, mode, d2, flip, ref_face, .. } => {
            pc.cmd.open(pc.armed, 5, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            refresh_edges(pc);
            pc.gsel.edges = live_picks(&*pc.project, src, edges, false);
            pc.chamfer.mode = mode; // restore the mode, the side and the second parameter
            pc.chamfer.flip = flip;
            pc.chamfer.ref_face = ref_face; // restore the hand-picked reference face
            pc.chamfer.pick_ref = false;
            use qymcad_core::feature::ChamferMode;
            let d2_def = if d2 > 0.0 { d2 } else if mode == ChamferMode::DistAngle { 45.0 } else { 1.5 };
            pc.cmd.params = vec![
                cmd_param_from(&*pc.project, fid, "f-leg", "dist", dist, 0.05, 1000.0),
                cmd_param_from(&*pc.project, fid, qymcad_ui_state::chamfer_d2_label(mode), "d2", d2_def, 0.0, 1000.0),
            ];
        }
        FeatureKind::Shell { src, thickness, ref faces, side, .. } => {
            pc.cmd.open(pc.armed, 6, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            // restore the MULTIPLE selection of open faces by their PERSISTENT ids - the highlight follows
            // the face selection
            pc.gsel.faces = live_picks(&*pc.project, src, faces, true);
            pc.gsel.faces_body = Some(src); // the faces belong to the body of the feature (that is the highlight scope)
            pc.opts.shell_side = side; // which way the wall goes
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-thickness", "thickness", thickness, 0.1, 1000.0)];
        }
        FeatureKind::Thicken { face, thickness, .. } => {
            pc.cmd.open(pc.armed, 28, was_3d);
            qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore);
            *pc.mode_3d = true;
            pc.gsel.faces.clear();
            pc.gsel.faces.insert(face);
            pc.gsel.faces_body = node.parent.and_then(|_| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: &*pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path }, *pc.sel));
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-thickness", "thickness", thickness, -100000.0, 100000.0)];
            *pc.status = qymcad_i18n::tr("msg-edit-thicken");
        }
        FeatureKind::SplitFace { plane, datum, offset, .. } => {
            pc.cmd.open(pc.armed, 29, was_3d);
            qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore);
            *pc.mode_3d = true;
            pc.split.plane = Some(if datum != 0 {
                qymcad_core::feature::SketchPlane::Datum(datum)
            } else {
                use qymcad_core::feature::{BasePlane, SketchPlane};
                SketchPlane::World(match plane {
                    1 => BasePlane::XZ,
                    2 => BasePlane::YZ,
                    _ => BasePlane::XY,
                })
            });
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-offset", "offset", offset, -100000.0, 100000.0)];
            *pc.status = qymcad_i18n::tr("msg-edit-split-face");
        }
        FeatureKind::SplitBody { plane, datum, offset, .. } => {
            // EDITING A CUT: restore the reference to the plane and the offset, exactly as at creation, so
            // that pressing Enter again does not recreate the feature from scratch and lose the link to the
            // datum.
            pc.cmd.open(pc.armed, 27, was_3d);
            qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore);
            *pc.mode_3d = true;
            pc.split.plane = Some(if datum != 0 {
                qymcad_core::feature::SketchPlane::Datum(datum)
            } else {
                use qymcad_core::feature::{BasePlane, SketchPlane};
                SketchPlane::World(match plane {
                    1 => BasePlane::XZ,
                    2 => BasePlane::YZ,
                    _ => BasePlane::XY,
                })
            });
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-offset", "offset", offset, -100000.0, 100000.0)];
            *pc.status = qymcad_i18n::tr("msg-edit-split-body");
        }
        FeatureKind::RemoveFace { src, ref faces, .. } => {
            pc.cmd.open(pc.armed, 26, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.gsel.faces = live_picks(&*pc.project, src, faces, true);
            pc.gsel.faces_body = Some(src);
            pc.cmd.params = vec![];
        }
        FeatureKind::PushFace { src, ref face, dist, .. } => {
            // EDITING THE FEATURE: the picked face is restored by its persistent id and the offset as an
            // expression if there was one (`cmd_param_from` takes the text from the feature dims).
            pc.cmd.open(pc.armed, 25, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.gsel.faces = live_picks(&*pc.project, src, face, true);
            pc.gsel.faces_body = Some(src);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-offset2", "dist", dist, -100000.0, 100000.0)];
        }
        FeatureKind::Draft { src, ref faces, neutral, angle, flip, .. } => {
            pc.cmd.open(pc.armed, 23, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            // restore the set of tilted faces and the neutral one by their persistent ids
            pc.gsel.faces = live_picks(&*pc.project, src, faces, true);
            pc.gsel.faces_body = Some(src); // the faces belong to the body of the feature (that is the highlight scope)
            pc.draft.neutral = live_picks(&*pc.project, src, &neutral, true).into_iter().next().unwrap_or(0);
            pc.draft.pick_neutral = false;
            pc.draft.flip = flip;
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-angle", "angle", angle, -60.0, 60.0)];
        }
        FeatureKind::Hole { src, face, diameter, depth, kind, dia2, depth2, sketch, flip, .. } => {
            pc.cmd.open(pc.armed, 7, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.hole.kind = kind; // F1
            pc.hole.mode = if sketch != 0 { 1 } else { 0 }; // the placement mode
            pc.hole.sketch = if sketch != 0 { Some(sketch) } else { None };
            pc.hole.flip = flip;
            if sketch != 0 {
                // "by sketch": highlight the sketch of marks itself (no face pick is needed)
                if let Some(si) = pc.project.sketch_index(sketch) {
                    *pc.sel = qymcad_ui_state::Sel::Sketch(si);
                }
            } else if let Some((mi, fi)) = face_desc_of(&face).and_then(|d| qymcad_pick::resolve_face_sel(&*pc.project, src, &qymcad_core::feature::FaceKey { index: 0, centroid: [0.0; 3], normal: [0.0, 0.0, 1.0], id: d })) {
                // restore THE FACE SELECTION by its persistent `FaceKey`, then highlight it
                *pc.sel = qymcad_ui_state::Sel::Face(mi, fi);
            }
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-diameter", "diameter", diameter, 0.1, 10000.0), cmd_param_from(&*pc.project, fid, "f-depth", "depth", depth, 0.1, 10000.0)];
            if kind != 0 {
                pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-recess-d", "dia2", dia2, 0.1, 10000.0));
                pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-recess-depth", "depth2", depth2, 0.1, 10000.0));
            }
        }
        // A THREAD: a double click reopens it with its parameters; the rim, the axis and the radius are
        // restored from `regen_edges`
        FeatureKind::Thread { src, edge, spec, length, lead_in, lead_out, .. } => {
            pc.cmd.open(pc.armed, 24, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.thread.src = Some(src);
            pc.thread.edge = edge;
            pc.thread.auger = false;
            pc.thread.internal = spec.internal;
            pc.thread.starts = spec.starts.max(1);
            pc.thread.left = spec.left;
            pc.thread.form = qymcad_ui_state::thread_standard_idx(spec.standard);
            qymcad_ui_state::restore_thread_axis(&mut *pc.project, &mut *pc.thread, src, edge);
            // THE SET OF FIELDS IS THE SAME AS AT CREATION. The fillets were missing here entirely: they
            // could be set only when the feature was first built, and on editing they vanished silently.
            pc.cmd.params = vec![
                cmd_param_from(&*pc.project, fid, "f-nominal-d", "nominal", spec.nominal_d, 0.5, 1000.0),
                cmd_param_from(&*pc.project, fid, "f-pitch-std", "pitch", spec.pitch, 0.0, 100.0),
                cmd_param_from(&*pc.project, fid, "f-length", "length", length, 0.1, 10000.0),
                cmd_param_from(&*pc.project, fid, "f-fit-clearance", "fit", spec.fit, 0.0, 5.0),
                cmd_param_from(&*pc.project, fid, "f-lead-in", "lead_in", lead_in, 0.0, 10000.0),
                cmd_param_from(&*pc.project, fid, "f-lead-out", "lead_out", lead_out, 0.0, 10000.0),
                cmd_param_from(&*pc.project, fid, "f-crest-fillet", "crest_r", spec.crest_r.unwrap_or(0.0), 0.0, 100.0),
                cmd_param_from(&*pc.project, fid, "f-root-fillet", "root_r", spec.root_r.unwrap_or(0.0), 0.0, 100.0),
            ];
            if spec.standard == qymcad_core::thread::ThreadStandard::Custom {
                pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-profile-angle", "angle", spec.custom_angle, 5.0, 170.0));
                pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-thread-depth", "depth", spec.custom_depth, 0.0, 1000.0));
            }
        }
        // AN AUGER - the same command, a mode of the timeline
        FeatureKind::Auger { src, edge, spec, length, lead_in, lead_out, .. } => {
            pc.cmd.open(pc.armed, 24, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.thread.src = Some(src);
            pc.thread.edge = edge;
            pc.thread.auger = true;
            pc.thread.starts = spec.starts.max(1);
            pc.thread.left = spec.left;
            qymcad_ui_state::restore_thread_axis(&mut *pc.project, &mut *pc.thread, src, edge);
            pc.cmd.params = vec![
                cmd_param_from(&*pc.project, fid, "f-outer-d", "outer", spec.outer_d, 0.5, 2000.0),
                cmd_param_from(&*pc.project, fid, "f-pitch", "pitch", spec.pitch, 0.1, 1000.0),
                cmd_param_from(&*pc.project, fid, "f-length", "length", length, 0.1, 10000.0),
                cmd_param_from(&*pc.project, fid, "f-flight-thickness", "thickness", spec.thickness, 0.1, 100.0),
                cmd_param_from(&*pc.project, fid, "f-edge-fillet", "edge_r", spec.edge_r, 0.0, 50.0),
                cmd_param_from(&*pc.project, fid, "f-taper-in", "lead_in", lead_in, 0.0, 10000.0),
                cmd_param_from(&*pc.project, fid, "f-taper-out", "lead_out", lead_out, 0.0, 10000.0),
            ];
        }
        // A MIRROR: a double click reopens the command; the plane is clicked in the viewport again
        FeatureKind::Mirror { src, plane, keep, datum, .. } => {
            use qymcad_core::feature::{BasePlane, SketchPlane};
            pc.cmd.open(pc.armed, 16, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.opts.mirror_keep = keep;
            pc.mirror.plane = Some(if datum != 0 {
                SketchPlane::Datum(datum)
            } else {
                SketchPlane::World(match plane {
                    1 => BasePlane::XZ,
                    2 => BasePlane::YZ,
                    _ => BasePlane::XY,
                })
            });
            pc.cmd.params.clear();
        }
        // THE PRIMITIVES: a double click reopens the command with the current sizes (the fields are the
        // keys the regen uses)
        FeatureKind::Box3 { dx, dy, dz, .. } => {
            pc.cmd.open(pc.armed, 10, was_3d);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-length-x", "dx", dx, 0.1, 100000.0), cmd_param_from(&*pc.project, fid, "f-width-y", "dy", dy, 0.1, 100000.0), cmd_param_from(&*pc.project, fid, "f-height-z", "dz", dz, 0.1, 100000.0)];
        }
        FeatureKind::Cylinder { r, h, .. } => {
            pc.cmd.open(pc.armed, 11, was_3d);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-radius", "r", r, 0.05, 100000.0), cmd_param_from(&*pc.project, fid, "f-height", "h", h, 0.1, 100000.0)];
        }
        FeatureKind::Sphere { r, .. } => {
            pc.cmd.open(pc.armed, 12, was_3d);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-radius", "r", r, 0.05, 100000.0)];
        }
        FeatureKind::Cone { r1, r2, h, .. } => {
            pc.cmd.open(pc.armed, 13, was_3d);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-radius-bottom", "r1", r1, 0.0, 100000.0), cmd_param_from(&*pc.project, fid, "f-radius-top", "r2", r2, 0.0, 100000.0), cmd_param_from(&*pc.project, fid, "f-height", "h", h, 0.1, 100000.0)];
        }
        FeatureKind::Torus { major, minor, .. } => {
            pc.cmd.open(pc.armed, 14, was_3d);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-ring-r", "major", major, 0.1, 100000.0), cmd_param_from(&*pc.project, fid, "f-tube-r", "minor", minor, 0.1, 100000.0)];
        }
        FeatureKind::Prism { r, n, h, .. } => {
            pc.cmd.open(pc.armed, 15, was_3d);
            pc.prim.n = n;
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-radius-circ", "r", r, 0.1, 100000.0), cmd_param_from(&*pc.project, fid, "f-height", "h", h, 0.1, 100000.0)];
        }
        // AN ARRAY: a double click reopens the command; the count, the direction and the axis live in the
        // bar, the step and the angle at the geometry
        FeatureKind::LinearArray { src, dx, dy, dz, count, dx2, dy2, dz2, count2, dx3, dy3, dz3, count3, .. } => {
            pc.cmd.open(pc.armed, 17, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            let (dir, step) = qymcad_ui_state::arr_dir_of(dx, dy, dz);
            pc.arr.dir = dir;
            pc.arr.count = count.max(1);
            let two = count2 > 1 && (dx2.abs() + dy2.abs() + dz2.abs()) > 1e-9;
            pc.arr.two = two;
            let (dir2, s2) = qymcad_ui_state::arr_dir_of(dx2, dy2, dz2);
            pc.arr.dir2 = if two { dir2 } else { 1 };
            pc.arr.count2 = count2.max(1);
            let three = two && count3 > 1 && (dx3.abs() + dy3.abs() + dz3.abs()) > 1e-9;
            pc.arr.three = three;
            let (dir3, s3) = qymcad_ui_state::arr_dir_of(dx3, dy3, dz3);
            pc.arr.dir3 = if three { dir3 } else { 2 };
            pc.arr.count3 = count3.max(1);
            // the step is the expression text from the logical key (`store_cmd_exprs`); the numeric
            // fallback is the magnitude of the component
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-pitch", "step", step, 0.01, 100000.0)];
            // the second and third steps are added AT ONCE with the right magnitude as a fallback (the sync
            // does not know the value)
            if two {
                pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-pitch2", "step2", s2, 0.01, 100000.0));
            }
            if three {
                pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-pitch3", "step3", s3, 0.01, 100000.0));
            }
        }
        FeatureKind::CircularArray { src, count, angle, axis, .. } => {
            pc.cmd.open(pc.armed, 18, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.arr.count = count.max(1);
            pc.arr.axis = axis;
            pc.arr.full = angle.abs() >= 359.9;
            pc.arr.two = false;
            pc.cmd.params = if pc.arr.full { vec![] } else { vec![cmd_param_from(&*pc.project, fid, "f-angle", "angle", angle, 1.0, 360.0)] };
        }
        // THE DATUMS: a double click reopens the command - a plane, a point or an axis is edited in place
        FeatureKind::Plane { plane } => {
            use qymcad_core::feature::SketchPlane;
            use qymcad_core::model::PlaneDef;
            let Some(pl) = pc.project.planes.iter().find(|p| p.id == plane).cloned() else { return };
            pc.datum.axis_ref = None;
            pc.datum.axis_mode = 0;
            match pl.def {
                PlaneDef::OffsetBase { base, dist } => {
                    pc.cmd.open(pc.armed, 20, was_3d);
                    pc.datum.plane_pick = Some(SketchPlane::World(base));
                    pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-offset", "dist", dist, -100000.0, 100000.0)];
                }
                PlaneDef::OffsetFace { body, face, dist } => {
                    pc.cmd.open(pc.armed, 20, was_3d);
                    pc.datum.plane_pick = Some(SketchPlane::Face(body, face));
                    pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-offset", "dist", dist, -100000.0, 100000.0)];
                }
                PlaneDef::OffsetPlane { plane: src, dist } => {
                    pc.cmd.open(pc.armed, 20, was_3d);
                    pc.datum.plane_pick = Some(SketchPlane::Datum(src));
                    pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-offset", "dist", dist, -100000.0, 100000.0)];
                }
                PlaneDef::Manual => {
                    *pc.status = qymcad_i18n::tr("msg-plane-manual");
                    return;
                }
            }
        }
        FeatureKind::DatumPoint { point } => {
            use qymcad_core::model::PointDef;
            let Some(dp) = pc.project.datum_points.iter().find(|p| p.id == point).cloned() else { return };
            pc.cmd.open(pc.armed, 21, was_3d);
            match dp.def {
                // an associative point: it reopens in "at a vertex" mode with the current reference as the
                // preview; clicking a new vertex replaces it, and without a re-pick the definition is kept
                PointDef::AtVertex { body, edge, end } => {
                    pc.datum.pt_mode = 1;
                    pc.datum.pt_vert = Some((body, edge, end, dp.at));
                    pc.cmd.params.clear();
                }
                PointDef::Manual => {
                    pc.datum.pt_mode = 0;
                    pc.datum.pt_vert = None;
                    pc.cmd.params = vec![
                        cmd_param_from(&*pc.project, fid, "X", "x", dp.at[0], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "Y", "y", dp.at[1], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "Z", "z", dp.at[2], -1e7, 1e7),
                    ];
                }
            }
        }
        FeatureKind::DatumAxis { axis } => {
            use qymcad_core::model::AxisDef;
            let Some(da) = pc.project.datum_axes.iter().find(|a| a.id == axis).cloned() else { return };
            match da.def {
                AxisDef::Manual { .. } => {
                    pc.cmd.open(pc.armed, 22, was_3d);
                    pc.datum.axis_mode = 1;
                    pc.datum.axis_ref = None;
                    pc.cmd.params = vec![
                        cmd_param_from(&*pc.project, fid, "O.x", "ox", da.origin()[0], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "O.y", "oy", da.origin()[1], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "O.z", "oz", da.origin()[2], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "Dir.x", "dx", da.dir()[0], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "Dir.y", "dy", da.dir()[1], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "Dir.z", "dz", da.dir()[2], -1e7, 1e7),
                    ];
                }
                // associative axes and two-point ones reopen in "by an edge or a face" mode: the current
                // axis is the preview, and without a re-pick the definition is kept; clicking a new
                // reference replaces it
                AxisDef::TwoPoints { .. } | AxisDef::FromEdge { .. } | AxisDef::FromFace { .. } => {
                    pc.cmd.open(pc.armed, 22, was_3d);
                    pc.datum.axis_mode = 0;
                    pc.datum.axis_hit = None;
                    pc.datum.axis_ref = Some((da.origin(), da.dir()));
                    refresh_axis_edges(pc);
                    pc.cmd.params.clear();
                }
            }
        }
        FeatureKind::BodyBoolean { .. } => {
            // a body boolean has no dimension popup - its KIND is edited in the top bar (the edit mode of
            // `bool_tool_bar`)
            pc.boolean.edit = pc.project.timeline.iter().position(|n| n.id == fid);
            if let Some(ti) = pc.boolean.edit {
                *pc.sel = qymcad_ui_state::Sel::Feature(ti);
            }
            *pc.status = qymcad_i18n::tr("msg-bool-switch");
            return;
        }
        _ => {
            // arrays, mirrors and moves have no command popup yet; they are edited in THE RIGHT-HAND PANEL
            // (the feature is already selected by the click)
            *pc.status = qymcad_i18n::tr("msg-params-on-right");
            return;
        }
    }
    pc.cmd.edit = Some(fid);
    // the highlighted row of the tree STAYS on THE FEATURE BEING EDITED: `qymcad_ui_state::select_body(src)` in the
    // branches above moved the selection to the SOURCE body, one row higher, and the highlight appeared
    // to jump up a level while editing.
    if let Some(ti) = pc.project.timeline.iter().position(|n| n.id == fid) {
        *pc.sel = qymcad_ui_state::Sel::Feature(ti);
    }
    *pc.mode_3d = true;
    *pc.status = qymcad_i18n::tr("msg-edit-feature");
}

/// The on-screen input field or fields for the dimensions of the active command, at the geometry (just
/// like sketch dimensions): a number OR an expression such as `w/2+3`. Enter applies, Esc cancels. One
/// mechanism for every part tool.
/// THE TWO BUTTONS A COMMAND ENDS WITH, in the colours the scheme gives them.
///
/// Reported behaviour: "highlight the Enter (apply) and Esc (cancel) buttons in the tool popups with
/// different colours, green and red say, so that it is clear". They used to differ by a tick glyph and a
/// bold face - a difference one READS rather than sees, and at the moment of pressing nobody is reading.
///
/// THE COLOUR GOES ON THE TEXT AND THE OUTLINE, not on the fill. A filled patch would need a text colour
/// contrasting against the patch itself, which is a second decision nobody guards; text on the panel
/// background is exactly the pair the scheme's legibility check already measures.
///
/// ONE PAIR FOR THREE PLACES. The point of the pair is that the two DIFFER, and a decision written out at
/// each of the three sites drifts at the first edit of one of them.
/// WHAT A COMMAND BAR SAYS, as opposed to what it offers to press.
///
/// Reported behaviour: "messages like these must be written on a new line. The text with the
/// information - a new line; if there is text with errors, that goes on a new line too, so on the third
/// one. And this is scale 1 - very small and unreadable even on my 2K monitor."
///
/// MEASURED ON THE THREAD BAR, opened with its own defaults - M10, coarse pitch, 0.20 mm of fit. It says
/// three sentences at once, 340 characters of prose, and every one of them stood IN the row of controls:
/// the row wraps by itself, so a sentence broke wherever the row happened to end and carried on under a
/// button. A person reading it has to find where it went.
///
/// The sentences are gathered here while the controls are drawn, and laid out under them: what the tool
/// tells on one line, what will not build on the next. Off that row they are also written at the ordinary
/// size - the small size was the price of squeezing prose in beside the buttons, and there is no such
/// price to pay on a line of one's own.
#[derive(Default)]
struct BarSays {
    /// what the tool is doing, or waiting for
    told: Vec<(String, Option<String>)>,
    /// what will not build the way it is set
    wrong: Vec<(String, Option<String>)>,
}

impl BarSays {
    /// What the tool is doing, or what it is waiting for.
    fn tell(&mut self, text: impl Into<String>) {
        self.told.push((text.into(), None));
    }

    /// The same, with the longer explanation the hover shows.
    fn tell_hover(&mut self, text: impl Into<String>, hover: impl Into<String>) {
        self.told.push((text.into(), Some(hover.into())));
    }

    /// What will not build as it is set, and what to change.
    fn wrong(&mut self, text: impl Into<String>) {
        self.wrong.push((text.into(), None));
    }

    /// Lay the sentences under the controls, one line for each kind.
    ///
    /// A line that has nothing to say takes no room at all: an empty row reserved for a hint that rarely
    /// comes would push the viewport down by its height on every command.
    fn show(self, ui: &mut egui::Ui, pal: &qymcad_scheme::Palette) {
        for (line, colour) in [(self.told, pal.hint()), (self.wrong, pal.error_mild())] {
            if line.is_empty() {
                continue;
            }
            ui.horizontal_wrapped(|ui| {
                for (i, (text, hover)) in line.into_iter().enumerate() {
                    if i > 0 {
                        ui.separator();
                    }
                    let r = ui.label(egui::RichText::new(text).color(colour));
                    if let Some(h) = hover {
                        r.on_hover_text(h);
                    }
                }
            });
        }
    }
}

fn confirm_button(pal: &qymcad_scheme::Palette, ui: &mut egui::Ui, enabled: bool, label: String) -> bool {
    let text = egui::RichText::new(label).color(pal.confirm()).strong();
    ui.add_enabled(enabled, egui::Button::new(text).stroke(egui::Stroke::new(1.0, pal.confirm()))).clicked()
}

fn refuse_button(pal: &qymcad_scheme::Palette, ui: &mut egui::Ui, label: String) -> bool {
    let text = egui::RichText::new(label).color(pal.refuse());
    ui.add(egui::Button::new(text).stroke(egui::Stroke::new(1.0, pal.refuse()))).clicked()
}

pub fn feat_cmd_popup(pc: &mut qymcad_ui_state::PartCtx, ctx: &egui::Context, rect: Rect) {
    if pc.armed.cmd_kind() == 0 || pc.cmd.params.is_empty() || !*pc.mode_3d {
        return;
    }
    let Some(anchor) = cmd_anchor_screen(pc, rect) else { return };
    let ready = cmd_ready(pc);
    let vars = pc.project.param_map();
    // in a symmetric chamfer the d2 field (the second leg or the angle) takes no part, so it is hidden
    let hide_d2 = pc.armed.cmd_kind() == 5 && pc.chamfer.mode == qymcad_core::feature::ChamferMode::Symmetric;
    let mut params = std::mem::take(&mut pc.cmd.params);
    let mut apply = false;
    // The cancel is taken here and acted on AFTER the popup is drawn: `qymcad_ui_state::cancel_feat_cmd` closes the
    // command whole, and the closure below is still holding its parameters.
    let mut cancel = false;
    // THE FIELDS AT THE GEOMETRY, EACH AT ITS OWN PLACE. The radius at a vertex is shown AT THAT VERTEX:
    // six identical fields in a common column cannot be told apart, and which of them is which corner is
    // the only thing that matters here.
    let basis = pc.cam.basis();
    for (i, p) in params.iter_mut().enumerate() {
        let Some(w) = p.at else { continue };
        let at = qymcad_ui_state::clamp_popup(qymcad_ui_state::Screen { cam: &*pc.cam, set: pc.set, rect: rect, basis: &basis }.at(w).0, rect);
        egui::Area::new(egui::Id::new(("feat_cmd_at", i))).fixed_pos(at + egui::vec2(8.0, -8.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(qymcad_i18n::tr(p.label_key()));
                    qymcad_ui_state::focus_edit(ui, &mut p.txt, 56.0, &qymcad_i18n::tr("f-number-or-expr"), false);
                    match qymcad_core::expr::eval(&p.txt, &vars) {
                        Ok(v) => p.val = v.clamp(p.lo, p.hi),
                        Err(e) => {
                            ui.colored_label(pc.scheme.pal.error_mild(), ph::X).on_hover_text(qymcad_i18n::error_words::expr_error_text(&e));
                        }
                    }
                });
            });
        });
    }
    let want_focus = std::mem::take(&mut pc.cmd.focus); // a one-shot auto-focus of field 0 (Enter plus selection)
    egui::Area::new(egui::Id::new("feat_cmd_popup")).fixed_pos(qymcad_ui_state::clamp_popup(anchor, rect) + egui::vec2(10.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            // WHAT EXACTLY IS WRONG AND IN WHICH FIELD. There used to be only a flag saying whether all
            // was well, and the bottom of the popup printed that a dimension expression could not be
            // evaluated - the same message for a typo, for an unknown name and for a division by zero.
            // With two or three fields there is no telling which one holds the mistake.
            let mut bad: Option<(String, String)> = None;
            let mut enter = false; // Enter was pressed in at least one field
            for (i, p) in params.iter_mut().enumerate() {
                if hide_d2 && p.key == "d2" {
                    continue; // d2 is hidden in the symmetric mode
                }
                if p.at.is_some() {
                    continue; // a field AT THE GEOMETRY is drawn by its own window below
                }
                ui.horizontal(|ui| {
                    ui.label(qymcad_i18n::tr(p.label_key()));
                    // THE SAME FIELD AS IN A SKETCH AND IN THE PARAMETER TABLE.
                    //
                    // A private `qymcad_ui_state::focus_edit` used to stand here - and the list of drivers did not exist in
                    // the popups of the part tools AT ALL. It was reported plainly: there is no drop-down
                    // with a search of parameters and drivers in the popups of the sketcher and part tools.
                    // The assumption at the time was that "the field in the bars is one for everybody, so
                    // it is wired everywhere". Not everywhere: the popup at the geometry draws fields of
                    // its own, and the check showed that only once it went THROUGH THE FRAME of every
                    // tool.
                    let fid = egui::Id::new(("cmdparam", pc.armed.cmd_kind(), p.key.clone()));
                    let o = qymcad_ui_state::expr_field_autofocus(ui, &*pc.project, fid, &p.txt, 74.0, &qymcad_i18n::tr("f-number-or-expr"), i == 0 && want_focus);
                    p.txt = o.text;
                    let te = o.resp;
                    match qymcad_core::expr::eval(&p.txt, &vars) {
                        Ok(v) => p.val = v.clamp(p.lo, p.hi),
                        Err(e) => {
                            // a broken expression leaves `p.val` alone (the old value stays), marks the field and blocks Apply
                            let msg = qymcad_i18n::error_words::expr_error_text(&e);
                            if bad.is_none() {
                                bad = Some((qymcad_i18n::tr(p.label_key()), msg.clone()));
                            }
                            ui.colored_label(pc.scheme.pal.error_mild(), ph::X).on_hover_text(&msg);
                        }
                    }
                    if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        enter = true;
                    }
                });
            }
            // Enter applies ONLY when EVERY field is valid (a stale value would go through otherwise)
            let all_ok = bad.is_none();
            if enter && all_ok {
                apply = true;
            }
            ui.horizontal(|ui| {
                if confirm_button(&pc.scheme.pal, ui, ready && all_ok, format!("{} {}", ph::CHECK, qymcad_i18n::tr("cmd-apply-enter"))) {
                    apply = true;
                }
                // THE BUTTON DOES WHAT THE KEY DOES. Reported behaviour: pressing it changes nothing -
                // the popup stays, the command stays. The click was read into an empty body, under a
                // comment claiming the cancel happened below; below there was only the apply. A button
                // that answers a click with nothing is worse than no button: it says the way out is
                // here, and it is not.
                if refuse_button(&pc.scheme.pal, ui, format!("{} {}", ph::X, qymcad_i18n::tr("cmd-cancel-esc"))) {
                    cancel = true;
                }
            });
            // AN ERROR MATTERS MORE THAN A HINT. The reason used to be shown only once the command was
            // ALREADY ready (a face or a contour picked) - that is, a typo in a field was reported after
            // everything else had been done. A typo concerns what is being typed right now, and there is
            // nothing to wait for.
            if let Some((field, msg)) = &bad {
                ui.label(egui::RichText::new(format!("{} {field}: {msg}", ph::X)).color(pc.scheme.pal.error_mild()).small());
            } else if !ready {
                ui.label(egui::RichText::new(cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim)).weak().small());
            }
        });
    });
    pc.cmd.params = params;
    if cancel {
        qymcad_ui_state::cancel_feat_cmd(pc);
    } else if apply {
        apply_feat_cmd(pc);
    }
}

/// Exact numeric input at the body gizmo: an EXPRESSION field at the geometry, as in the sketcher. Type
/// millimetres (a shift along an axis) or degrees (a rotation about an axis), and Enter applies a
/// PARAMETRIC Move feature; Esc cancels.
pub fn body_num_popup(pc: &mut qymcad_ui_state::PartCtx, ctx: &egui::Context, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
    let Some((mi, ax, rot)) = pc.body_giz.num else { return };
    let (o, _) = qymcad_ui_state::body_gizmo_geometry(&*pc.body_giz, *pc.cam, &*pc.project, pc.set, mi);
    let at = qymcad_ui_state::clamp_popup(qymcad_ui_state::Screen { cam: &*pc.cam, set: pc.set, rect: rect, basis: basis }.at(o).0, rect);
    let axn = ["X", "Y", "Z"][(ax as usize).min(2)];
    let label = if rot { qymcad_i18n::tr1("cmd-rotation-axis", "axis", axn) } else { qymcad_i18n::tr1("cmd-offset-axis", "axis", axn) };
    let preview = pc.project.eval_expr(&pc.body_giz.num_buf); // the preview lags by one frame - the buffer changes inside the field
    let mut apply = false;
    egui::Area::new(egui::Id::new("body_num")).fixed_pos(at + egui::vec2(12.0, -12.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).small());
                let r = qymcad_ui_state::focus_edit(ui, &mut pc.body_giz.num_buf, 74.0, &qymcad_i18n::tr("f-expression"), std::mem::take(&mut pc.body_giz.num_focus));
                if r.lost_focus() {
                    apply = true; // Enter OR a click elsewhere commits, as with renaming in place
                }
                if ui.small_button(ph::CHECK).clicked() {
                    apply = true;
                }
            });
            match &preview {
                Ok(v) => {
                    ui.label(egui::RichText::new(format!("= {v:.3}")).weak().small());
                }
                Err(e) if !pc.body_giz.num_buf.is_empty() => {
                    // THE REASON, not "it did not evaluate". A general phrase is the same for a typo, for
                    // an unknown name and for a division by zero - that is, it says nothing.
                    ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, qymcad_i18n::error_words::expr_error_text(e))).small().color(pc.scheme.pal.error_mild()));
                }
                _ => {}
            }
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        pc.body_giz.num = None;
        return;
    }
    if apply {
        if let Ok(v) = pc.project.eval_expr(&pc.body_giz.num_buf) {
            if v.abs() > 1e-9 {
                let t = if rot {
                    qymcad_ui_state::rot_about_point(ax, v, o)
                } else {
                    let mut t = qymcad_core::feature::PLACE_IDENTITY;
                    t[[3, 7, 11][(ax as usize).min(2)]] = v;
                    t
                };
                apply_body_move(pc, mi, t);
            }
        }
        pc.body_giz.num = None;
    }
}

pub fn refresh_edges(pc: &mut qymcad_ui_state::PartCtx) {
    // the B-rep cache is brought up ONLY when the edges are really needed - under the fillet and chamfer
    // commands or while edges are actively being picked. This method is called EVERY FRAME in 3D, and an
    // unconditional `ensure_brep` turned lazy B-rep building into eager: "Preparing B-rep" started right
    // after any project was opened, before anything had been done at all.
    //
    // Reported behaviour: the choice of a sketch origin on a face had disappeared. Binding a sketch's origin
    // takes VERTICES and EDGES from the live B-rep, which is no longer built on opening. Open a file, start a
    // new sketch, hover a face - there is nothing to snap to, the green marker never appears, and the origin
    // silently falls back to the default. qymcad_ui_state::Picking a sketch plane is as much "the edges are really needed" as
    // a chamfer is.
    if qymcad_ui_state::needs_live_brep(qymcad_ui_state::doing_in!(pc), &*pc.project, pc.set) {
        qymcad_ui_state::ensure_brep(&mut pc.rebuild());
    }
    // EDITING a fillet or a chamfer: the edges always belong to the feature's SOURCE BODY (a selection fix
    // put `sel` back on the node being edited, and `qymcad_ui_state::selected_body` then returned the OUTPUT body, so the edge
    // selection was cleared and the highlight vanished). The source of the feature being edited is aimed at
    // explicitly.
    let edit_src = pc.cmd.edit.and_then(|fid| {
        pc.project.timeline.iter().find(|n| n.id == fid).and_then(|n| match n.kind {
            qymcad_core::feature::FeatureKind::Fillet { src, .. } | qymcad_core::feature::FeatureKind::Chamfer { src, .. } => Some(src),
            _ => None,
        })
    });
    // a Part is one body, so under Chamfer/Fillet the edges of that single body are available at once,
    // without clicking the body first (press the button, then click edges). When no body is explicitly
    // selected, the context's active_body is taken.
    //
    // PATCH BELONGS HERE TOO. It gathers EDGES just as the fillet and the chamfer do, but it used to take the
    // body only from the tree selection: press Patch without selecting a part, and not a single edge could be
    // picked. To a person that is "the tool does not work", indistinguishable from having clicked the wrong place.
    let cur = edit_src.or_else(|| qymcad_ui_state::selected_body(&*pc.project, &*pc.sel)).or_else(|| if matches!(pc.armed.cmd_kind(), 4 | 5 | 32) { qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: &*pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path }) } else { None });
    let body_changed = cur != pc.edges.body;
    // the refresh happens not only when the body CHANGES but also when the same body is REBUILT (geom_rev
    // changed) - otherwise edge_polys and edge_ids stay with the old topology, and the highlight lands on
    // edges belonging to something else, or on ones that no longer exist.
    if body_changed || pc.edges.rev != pc.regen.geom_rev {
        pc.edges.body = cur;
        pc.edges.rev = pc.regen.geom_rev;
        if body_changed {
            pc.gsel.edges.clear(); // a different body - the edge selection is not carried over
        }
        let (polys, ids) = cur
            .and_then(|b| pc.live.shapes.get(&b).map(|s| s.edges_full_smooth()))
            .map(|(p, i, _, sm)| {
                if matches!(pc.armed.cmd_kind(), 4 | 5) {
                    // SMOOTH edges (the tangent seams of fillets) are NOT offered for a chamfer or a fillet -
                    // there is nothing there to round, and picking them only piled up red nodes
                    let (mut fp, mut fi) = (Vec::new(), Vec::new());
                    for k in 0..i.len() {
                        if !sm.get(k).copied().unwrap_or(false) {
                            fp.push(p[k].clone());
                            fi.push(i[k]);
                        }
                    }
                    (fp, fi)
                } else {
                    (p, i)
                }
            })
            .unwrap_or_default();
        pc.edges.polys = polys;
        pc.edges.ids = ids; // the persistent edge ids, parallel to the polylines
        // drop the picked ids the body no longer has (it was rebuilt), so that no phantoms are lit
        if !pc.gsel.edges.is_empty() {
            let live: std::collections::HashSet<u32> = pc.edges.ids.iter().copied().collect();
            pc.gsel.edges.retain(|id| live.contains(id));
        }
    }
}

/// Delete feature `ti` (together with its body), clear the caches and rebuild.
pub fn delete_feature(pc: &mut qymcad_ui_state::PartCtx, ti: usize) {
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("status-delete-feature")); // THE OPERATION BOUNDARY
    if ti >= pc.project.timeline.len() {
        return;
    }
    if pc.project.timeline[ti].kind.body().is_some() {
        // THE WHOLE OPERATION (its span) plus the consumers ahead of it, through the tested
        // `Project::delete_feature_op`: it also removes the sibling extrusions of a collapsed operation
        // (otherwise the part fell apart into orphan bodies and "deleted" nodes came back). Here we only
        // clear the shape cache by the list of what was removed.
        let nid = pc.project.timeline[ti].id;
        for db in pc.project.delete_feature_op(nid) {
            pc.live.shapes.remove(&db);
        }
    } else {
        pc.project.timeline.remove(ti); // a node with no body (a sketch or a datum in the timeline)
    }
    *pc.sel = qymcad_ui_state::Sel::None;
    resync_after_topology_change(pc);
        qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// Delete a BODY by mesh index `mi` (whichever way the body was selected — in 3D, in the tree, in a
/// panel): take the deepest existing node of the chain and cascade forward (`delete_feature`). An
/// imported mesh with no node is removed directly. One path for every place (the panel, Del, the
/// context menu), so nothing drifts apart.
pub fn delete_body_mesh(pc: &mut qymcad_ui_state::PartCtx, mi: usize) {
    match pc.project.mesh_id(mi).and_then(|b| qymcad_ui_state::lineage_delete_ti(&*pc.project, b)) {
        Some(ti) => delete_feature(pc, ti), // ask_delete-exempt: this is the executor itself, called by `execute_delete`
        None => {
            // an imported mesh with no feature in the timeline is removed directly
            pc.project.remove_mesh(mi); // the faces and the visibility go with the body — one record
            *pc.sel = qymcad_ui_state::Sel::None;
            qymcad_ui_state::invalidate(&mut *pc.regen);
        }
    }
}

/// Delete a SKETCH (one path for the tree and for the panel): the core's `delete_sketch` (cascading to
/// the bodies built on it), then clearing the editing session and the selection, then a resync
/// (regeneration plus prune).
pub fn delete_sketch_full(pc: &mut qymcad_ui_state::PartCtx, sid: Id) {
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("status-delete-sketch")); // THE OPERATION BOUNDARY
    let removed = pc.project.delete_sketch(sid);
    if pc.sketch_ses.editing == Some(sid) {
        pc.sketch_ses.editing = None;
    }
    *pc.sel = qymcad_ui_state::Sel::None;
    resync_after_topology_change(pc);
    *pc.status = if removed.is_empty() { qymcad_i18n::tr("sketch-deleted") } else { qymcad_i18n::tr1("sketch-deleted-n", "n", &removed.len().to_string()) };
        qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// Bring the application's caches (faces, visibility) into line with the current list of meshes after a
/// change of topology (deleting a sketch or a feature) and rebuild the bodies from the timeline — the
/// mesh indices may have shifted.
pub fn resync_after_topology_change(pc: &mut qymcad_ui_state::PartCtx) {
    // The faces are RE-HUNG by body Id rather than cleared. A `vec![Vec::new(); ...]` plus a FORCED
    // regeneration used to stand here (and only that regeneration filled the faces back in) — meaning
    // EVERY deletion of a node rebuilt and re-tessellated the WHOLE document. On an assembly of 1170
    // imported solids that is tens of seconds of freeze per Delete. The mesh indices shift after a
    // deletion, but body Ids are stable, so that is what they are laid out by; only what the deletion
    // actually made dirty needs rebuilding.
    rebuild_faces_from_cache(pc);
    pc.gsel.edges.clear();
    pc.edges.body = None;
    // THE TOPOLOGY CHANGED — THE DERIVED CACHES ARE STALE, whether anything was rebuilt or not.
    //
    // Reported behaviour: delete "Push face" from the tree and the body disappears from the viewport
    // until Edit -> Rebuild everything. The cause: `geom_rev` (the key of every derived cache) only
    // ticks inside a rebuild that actually happened, and deleting a leaf modifier leaves no dirty node
    // at all — there is nothing to compute, and the scheduler honestly does nothing. The
    // `qymcad_ui_state::consumed_bodies` cache meanwhile stays as it was on the previous frame, where the source body was
    // still consumed by the deleted feature — and `qymcad_ui_state::body_shown` hides the ONLY remaining body. Bodies
    // would vanish the same way after deleting any modifying feature (remove face, split body, chamfer,
    // shell).
    //
    // So the counter is advanced HERE: a change of topology is itself the event "the derived data is
    // invalid", and it need not coincide with a rebuild of the geometry.
    qymcad_ui_state::invalidate(&mut *pc.regen);
    qymcad_ui_state::regenerate_all(&mut pc.rebuild()); // only the dirty nodes (the deletion cascade has already marked them)
    qymcad_ui_state::detect_missing_faces(&mut *pc.live, &mut *pc.project); // mesh-based detection ONLY for raw meshes with no B-rep
    pc.view.initialized = false;
}

/// Lay the faces out of the `faces_by_body` cache onto the CURRENT mesh indices. The cache is keyed by
/// body Id and so survives the deletions and reorderings that shift the indices — the faces used to be
/// restored after a change of topology only by a full forced regeneration.
pub fn rebuild_faces_from_cache(pc: &mut qymcad_ui_state::PartCtx) {
    let live: std::collections::HashSet<Id> = pc.project.bodies.iter().map(|b| b.id).collect();
    pc.live.faces.retain(|b, _| live.contains(b));
    // THE FACES COME BACK BY ID, one call each: the model decides what a body's faces are, the
    // cache only remembers them.
    let restore: Vec<(qymcad_core::model::Id, Vec<qymcad_core::geom::MeshFace>)> =
        pc.project.bodies.iter().filter_map(|b| pc.live.faces.get(&b.id).map(|f| (b.id, f.clone()))).collect();
    for (id, faces) in restore {
        pc.project.set_body_faces(id, faces);
    }
}

/// THE SECTION CONTROL BAR: offset, tilts, flip, switching off.
///
/// This is PANEL DRAWING rather than a phase of the frame - yet it used to sit in the middle of `update`,
/// mixed in with the prologue, the keyboard and holding the selection. While panels live in the shared body,
/// "what the frame does" cannot be told from "what it draws", and editing one touches the other.
pub fn section_bar(regen: &mut qymcad_ui_state::Rebuilding, section: &mut qymcad_ui_state::SectionTool, ui: &mut egui::Ui) {
    // THE SECTION: the control bar (offset, tilts, flip, off) for as long as the section is active
    if section.plane.is_some() {
        let mut changed = false;
        let mut off = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::SQUARE_HALF, qymcad_i18n::tr("sec-btn"))).strong());
            ui.separator();
            ui.label(qymcad_i18n::tr("sec-offset"));
            changed |= ui.add(egui::DragValue::new(&mut section.offset).speed(0.5).suffix(qymcad_i18n::tr("unit-mm-suffix"))).changed();
            ui.label(qymcad_i18n::tr("sec-tilt1"));
            changed |= ui.add(egui::DragValue::new(&mut section.rot[0]).speed(0.5).range(-89.0..=89.0).suffix("°")).changed();
            ui.label(qymcad_i18n::tr("sec-tilt2"));
            changed |= ui.add(egui::DragValue::new(&mut section.rot[1]).speed(0.5).range(-89.0..=89.0).suffix("°")).changed();
            if ui.button(format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("sec-flip-btn"))).on_hover_text(qymcad_i18n::tr("sec-flip-hint")).clicked() {
                if let Some((_, n)) = &mut section.plane {
                    n[0] = -n[0];
                    n[1] = -n[1];
                    n[2] = -n[2];
                }
                section.offset = -section.offset;
                changed = true;
            }
            if ui.button(format!("{} {}", ph::X, qymcad_i18n::tr("sec-off-btn"))).clicked() {
                off = true;
            }
        });
        if off {
            section.plane = None;
            changed = true;
        }
        if changed {
            qymcad_ui_state::invalidate(regen);
        }
    }
}

/// THE TOP BAR OF A COMPONENT PATTERN (in an assembly) - the same interaction as the body pattern: the
/// count and the direction here, the step or the angle in an expression field at the geometry, Enter applies.
///
/// The bars used to be 645 lines sitting straight inside `update`: two thirds of what was left in the frame
/// after the earlier extractions. While a panel lives in the frame's body, "what the frame does" and "what it
/// draws" cannot be told apart, and editing one command touches the whole life cycle of the frame.
pub fn comp_array_bar(pc: &mut qymcad_ui_state::PartCtx, ui: &mut egui::Ui) {
    if pc.carr.mode == 0 {
        return;
    }
    let (mut apply, mut cancel) = (false, false);
    ui.horizontal_wrapped(|ui| {
        let title = if pc.carr.mode == 2 { qymcad_i18n::tr("cmd-comp-circ-array") } else { qymcad_i18n::tr("cmd-comp-lin-array") };
        ui.label(egui::RichText::new(format!("{} {title}", ph::STACK)).strong());
        ui.separator();
        let name = pc.project.components.iter().find(|c| c.id == pc.carr.src).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_default();
        ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-source", "name", &name)).weak());
        ui.separator();
        ui.label(qymcad_i18n::tr("cmd-copies"));
        pc.arr.count = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: pc.bar_exprs, project: pc.project, scheme: pc.scheme }, ui, "carr_count", pc.arr.count as f64, qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "" }) as u32;
        ui.separator();
        if pc.carr.mode == 2 {
            ui.label(qymcad_i18n::tr("cmd-axis"));
            ui.selectable_value(&mut pc.carr.axis, 0u8, "X");
            ui.selectable_value(&mut pc.carr.axis, 1u8, "Y");
            ui.selectable_value(&mut pc.carr.axis, 2u8, "Z");
            ui.separator();
            let was = pc.arr.full;
            ui.checkbox(&mut pc.arr.full, qymcad_i18n::tr("cmd-full-circle-short"));
            if was != pc.arr.full {
                // the angle field appears and disappears with the checkbox, as in the body pattern
                pc.cmd.params = if pc.arr.full { vec![] } else { vec![qymcad_ui_state::CmdParam::new("cmd-angle", "cangle", 180.0, -3600.0, 3600.0)] };
            }
        } else {
            ui.label(qymcad_i18n::tr("cmd-direction"));
            ui.selectable_value(&mut pc.carr.dir, 0u8, "X");
            ui.selectable_value(&mut pc.carr.dir, 1u8, "Y");
            ui.selectable_value(&mut pc.carr.dir, 2u8, "Z");
        }
        ui.separator();
        apply = confirm_button(&pc.scheme.pal, ui, true, format!("{} {}", ph::CHECK, qymcad_i18n::tr("cmd-apply-enter")));
        cancel = refuse_button(&pc.scheme.pal, ui, format!("{} {}", ph::X, qymcad_i18n::tr("cmd-cancel-btn")));
    });
    if apply {
        apply_comp_array(pc);
    } else if cancel {
        *pc.carr = qymcad_ui_state::CompArrayCmd::default();
        pc.cmd.params.clear();
        *pc.status = qymcad_i18n::tr("msg-comp-array-cancelled");
    }
}

/// IS THIS TOOL IN HAND - open, OR waiting for the sketch it needs?
///
/// Reported behaviour: "press the tool and it stays highlighted, with the status asking for a sketch." It
/// did not: the wait lives in `Picking`, and the button lit from `armed` alone, so a tool that WAS waiting
/// looked untouched. A tool that acts and does not look taken is the same trouble as one that looks taken
/// and does not act.
fn tool_is_taken(bc: &qymcad_ui_state::BarCtx, kind: u8) -> bool {
    bc.armed.cmd_kind() == kind || bc.picking.sketch_for() == Some(kind)
}

pub fn feat_command_bar(pc: &mut qymcad_ui_state::PartCtx, ui: &mut egui::Ui) {
    // The panel lives inside a `Ui` now; the context is still wanted for windows,
    // input and viewport commands, and it comes from the same place.
    let ctx = &ui.ctx().clone();
    // the top row of the active Part command: the options + Apply/Cancel.
    // The SIZE itself is set on the canvas (the gizmo arrow or a field at the geometry), not here.
    if pc.armed.commanding() {
        let title = match pc.armed.cmd_kind() {
            1 => &qymcad_i18n::tr("cmd-extrude"),
            3 => &qymcad_i18n::tr("cmd-revolve"),
            8 => &qymcad_i18n::tr("cmd-sweep"),
            9 => &qymcad_i18n::tr("cmd-loft"),
            4 => &qymcad_i18n::tr("cmd-fillet"),
            5 => &qymcad_i18n::tr("cmd-chamfer"),
            6 => &qymcad_i18n::tr("cmd-shell"),
            7 => &qymcad_i18n::tr("cmd-hole"),
            10 => &qymcad_i18n::tr("cmd-box"),
            11 => &qymcad_i18n::tr("cmd-cylinder"),
            12 => &qymcad_i18n::tr("cmd-sphere"),
            13 => &qymcad_i18n::tr("cmd-cone"),
            14 => &qymcad_i18n::tr("cmd-torus"),
            15 => &qymcad_i18n::tr("cmd-prism"),
            16 => &qymcad_i18n::tr("cmd-mirror"),
            17 => &qymcad_i18n::tr("cmd-linear-array"),
            18 => &qymcad_i18n::tr("cmd-circular-array"),
            20 => &qymcad_i18n::tr("cmd-plane"),
            21 => &qymcad_i18n::tr("cmd-point"),
            22 => &qymcad_i18n::tr("cmd-axis"),
            23 => &qymcad_i18n::tr("cmd-draft"),
            24 => &qymcad_i18n::tr("cmd-thread"),
            _ => &qymcad_i18n::tr("cmd-command"),
        };
        let (mut apply, mut cancel, mut reselect) = (false, false, false);
        // The sentences the bar says are gathered here and laid out under the controls, not between them.
        let mut says = BarSays::default();
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {title}", ph::CUBE)).strong());
            ui.separator();
            match pc.armed.cmd_kind() {
                1 => {
                    ui.label(qymcad_i18n::tr("cmd-operation"));
                    // a Part is one body, so there is "Add" (material into the single body, seeding it if
                    // it is the first). The former "New" and "Join" merged into "Add" - bodies are no
                    // longer bred.
                    ui.selectable_value(&mut pc.feat.op, 0u8, qymcad_i18n::tr("cmd-add"));
                    ui.selectable_value(&mut pc.feat.op, 2u8, qymcad_i18n::tr("cmd-cut"));
                    ui.selectable_value(&mut pc.feat.op, 3u8, qymcad_i18n::tr("cmd-intersect"));
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-extent"));
                    ui.selectable_value(&mut pc.cmd.extent, qymcad_ui_state::ExtentMode::Length, qymcad_i18n::tr("cmd-to-length"));
                    ui.selectable_value(&mut pc.cmd.extent, qymcad_ui_state::ExtentMode::Symmetric, qymcad_i18n::tr("cmd-symmetric"));
                    ui.selectable_value(&mut pc.cmd.extent, qymcad_ui_state::ExtentMode::TwoSided, qymcad_i18n::tr("cmd-two-sides"));
                    if pc.feat.op != 0 {
                        ui.selectable_value(&mut pc.cmd.extent, qymcad_ui_state::ExtentMode::Through, qymcad_i18n::tr("cmd-through-all"));
                    } else if pc.cmd.extent.through() {
                        pc.cmd.extent = qymcad_ui_state::ExtentMode::Length; // "through all" only applies to operations on a body
                    }
                    if pc.cmd.extent.two_sided() {
                        // the second side's distance is an expression field at the geometry (a popup), not here
                        says.tell(qymcad_i18n::tr("cmd-second-side-note"));
                    }
                    ui.separator();
                    if ui.selectable_label(pc.feat.flip, format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-flip-btn"))).on_hover_text(qymcad_i18n::tr("cmd-reverse-hint")).clicked() {
                        let f = !pc.feat.flip;
                        pc.feat.set_flip(f); // the direction was set by hand
                    }
                }
                3 => {
                    // A revolve operation on the part's single body: Add (a boss), Cut or Intersect.
                    ui.label(qymcad_i18n::tr("cmd-operation"));
                    ui.selectable_value(&mut pc.feat.op, 0u8, qymcad_i18n::tr("cmd-add"));
                    ui.selectable_value(&mut pc.feat.op, 2u8, qymcad_i18n::tr("cmd-cut"));
                    ui.selectable_value(&mut pc.feat.op, 3u8, qymcad_i18n::tr("cmd-intersect"));
                    ui.separator();
                    // how the angle is laid out: to one side, or symmetrically about half of it; Flip reverses it
                    ui.label(qymcad_i18n::tr("cmd-angle"));
                    ui.selectable_value(&mut pc.cmd.extent, qymcad_ui_state::ExtentMode::Length, qymcad_i18n::tr("cmd-one-side"));
                    ui.selectable_value(&mut pc.cmd.extent, qymcad_ui_state::ExtentMode::Symmetric, qymcad_i18n::tr("cmd-symmetric"));
                    if ui.selectable_label(pc.feat.flip, format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-flip-btn"))).on_hover_text(qymcad_i18n::tr("cmd-flip-angle-hint")).clicked() {
                        pc.feat.flip = !pc.feat.flip;
                    }
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-axis"));
                    // the axis comes from a sketch CENTRELINE, from an edge, a cylindrical face or a datum, or from X-Y.
                    let axis_src = if pc.rev.axis_line != 0 {
                        Some(qymcad_i18n::tr("cmd-sketch-centreline"))
                    } else if pc.rev.axis_datum != 0 {
                        Some(pc.project.datum_axes.iter().find(|d| d.id == pc.rev.axis_datum).map(|d| qymcad_i18n::name(&d.name)).unwrap_or_else(|| qymcad_i18n::tr("cmd-axis-lower")))
                    } else {
                        None
                    };
                    match axis_src {
                        Some(name) => {
                            ui.colored_label(pc.scheme.pal.connector(), format!("{} {name}", ph::CROSSHAIR));
                            let pair = format!("{}/{}", qymcad_ui_state::sketch_axis_name(&*pc.project, pc.cmd.sketch, 0), qymcad_ui_state::sketch_axis_name(&*pc.project, pc.cmd.sketch, 1));
                            if ui.small_button(format!("{} {pair}", ph::X)).on_hover_text(qymcad_i18n::tr("cmd-reset-axis-sketch")).clicked() {
                                pc.rev.axis_datum = 0;
                                pc.rev.axis_line = 0;
                                pc.rev.pick_axis = false;
                            }
                        }
                        None => {
                            // The two axes of the sketch's own plane, named by where they point in the
                            // world: on the front plane the second one IS the world Z, and calling it Y
                            // hid the axis a person came looking for.
                            for a in [0u8, 1] {
                                let name = qymcad_ui_state::sketch_axis_name(&*pc.project, pc.cmd.sketch, a);
                                ui.selectable_value(&mut pc.rev.axis, a, name).on_hover_text(qymcad_i18n::tr1("cmd-revolve-about-axis", "axis", name));
                            }
                        }
                    }
                    // A sketch CENTRELINE is a reliable choice made BY A BUTTON (with no click in 3D).
                    // For a sphere: a construction diameter line through the circle's centre IN the sketch plane.
                    let axis_lines = pc.cmd.sketch.map(|si| qymcad_ui_state::profile_axis_lines(&*pc.project, si)).unwrap_or_default();
                    if !axis_lines.is_empty() {
                        ui.separator();
                        if axis_lines.len() == 1 {
                            let l = axis_lines[0];
                            let on = pc.rev.axis_line == l;
                            let name = qymcad_ui_state::axis_line_label(&*pc.project, pc.cmd.sketch.unwrap_or(0), l, 1);
                            if ui
                                .selectable_label(on, format!("{} {name}", ph::LINE_SEGMENT))
                                .on_hover_text(qymcad_i18n::tr("cmd-revolve-any-line-hint"))
                                .clicked()
                            {
                                pc.rev.axis_line = if on { 0 } else { l };
                                if pc.rev.axis_line != 0 {
                                    pc.rev.axis_datum = 0;
                                }
                            }
                        } else {
                            // With SEVERAL lines the choice is made BY CLICKING the line itself in the
                            // sketch rather than from a list of numbered lines: a number does not tell
                            // which of them is the one wanted.
                            let si_cur = pc.cmd.sketch.unwrap_or(0);
                            if ui
                                .selectable_label(pc.rev.pick_line, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr("cmd-pick-axis-sketch")))
                                .on_hover_text(qymcad_i18n::tr("cmd-revolve-flat-hint"))
                                .clicked()
                            {
                                pc.rev.pick_line = !pc.rev.pick_line;
                                if pc.rev.pick_line {
                                    pc.rev.pick_axis = false;
                                    *pc.mode_3d = false; // the flat half-sketcher: the lines are visible and clickable
                                    pc.view.initialized = false;
                                    *pc.status = qymcad_i18n::tr("cmd-revolve-pick-line");
                                }
                            }
                            if pc.rev.axis_line != 0 {
                                let name = qymcad_ui_state::axis_line_label(&*pc.project, si_cur, pc.rev.axis_line, axis_lines.iter().position(|l| *l == pc.rev.axis_line).map(|i| i + 1).unwrap_or(1));
                                ui.label(egui::RichText::new(format!("{} {name}", ph::CHECK)).color(pc.scheme.pal.hint()));
                                if ui.small_button(qymcad_i18n::tr("cmd-reset-lower")).clicked() {
                                    pc.rev.axis_line = 0;
                                }
                            }
                            let _cur_unused = if pc.rev.axis_line != 0 {
                                axis_lines
                                    .iter()
                                    .position(|l| *l == pc.rev.axis_line)
                                    .map(|i| qymcad_ui_state::axis_line_label(&*pc.project, si_cur, pc.rev.axis_line, i + 1))
                                    .unwrap_or_else(|| qymcad_i18n::tr("cmd-sketch-axis"))
                            } else {
                                qymcad_i18n::tr("cmd-axis-from-sketch")
                            };
                            let _ = _cur_unused;
                            if pc.rev.axis_line != 0 {
                                pc.rev.axis_datum = 0;
                            }
                        }
                    }
                    ui.separator();
                    // the regular 3D axis pick: a STRAIGHT edge of a body, a CYLINDRICAL face or a datum axis
                    if ui.selectable_label(pc.rev.pick_axis, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr("cmd-pick-axis-3d"))).on_hover_text(qymcad_i18n::tr("cmd-revolve-axis-hint")).clicked() {
                        pc.rev.pick_axis = !pc.rev.pick_axis;
                        if pc.rev.pick_axis {
                            pc.rev.pick_line = false;
                            // The axis candidates (datum axes, edges) are drawn and hit-tested ONLY in 3D.
                            // Staying in the flat half-sketcher would make the button do nothing: there
                            // would be nothing to click.
                            *pc.mode_3d = true;
                            pc.view.initialized = false;
                            refresh_axis_edges(pc); // the straight edges of every visible body are axis candidates
                            *pc.status = qymcad_i18n::tr("cmd-pick-axis-hint");
                        }
                    }
                }
                8 => {
                    // a sweep operation on the single body: Add, Cut or Intersect
                    ui.label(qymcad_i18n::tr("cmd-operation"));
                    ui.selectable_value(&mut pc.feat.op, 0u8, qymcad_i18n::tr("cmd-add"));
                    ui.selectable_value(&mut pc.feat.op, 2u8, qymcad_i18n::tr("cmd-cut"));
                    ui.selectable_value(&mut pc.feat.op, 3u8, qymcad_i18n::tr("cmd-intersect"));
                    ui.separator();
                    // Sweep: the profile readout + the contour choice + picking the path (a click on a sketch in the tree)
                    let prof_name = pc.project.sketches.iter().find(|s| s.id == pc.sweep.prof_sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_else(|| "—".into());
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-profile-is", "name", &prof_name)).color(pc.scheme.pal.hint()));
                    // choosing the profile's contour through the HALF-SKETCHER (a click on the contour), when there is more than one
                    let prof_cands = pc.project.sweep_profile_contours(pc.sweep.prof_sid);
                    if prof_cands.len() > 1 {
                        let cur = prof_cands.iter().position(|c| *c == pc.sweep.prof_cid).map(|i| i + 1).unwrap_or(1);
                        let act = pc.picking.contour() == Some(qymcad_ui_state::ContourSlot::SweepProfile);
                        if ui.selectable_label(act, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr2("cmd-pick-contour-n", "cur", &cur.to_string(), "total", &prof_cands.len().to_string()))).on_hover_text(qymcad_i18n::tr("cmd-open-sketch-flat")).clicked() {
                            begin_contour_pick(pc, qymcad_ui_state::ContourSlot::SweepProfile, pc.sweep.prof_sid);
                        }
                    }
                    ui.separator();
                    // the path is taken from the tree selection while the pick is active (as for a hole driven by a sketch)
                    if pc.sweep.pick_path {
                        if let qymcad_ui_state::Sel::Sketch(si) = *pc.sel {
                            if let Some(s) = pc.project.sketches.get(si) {
                                if s.id != pc.sweep.prof_sid {
                                    pc.sweep.path_sid = s.id;
                                    pc.sweep.path_cid = 0; // reset the contour choice on a new sketch
                                    pc.sweep.pick_path = false;
                                }
                            }
                        }
                    }
                    let path_txt = if pc.sweep.path_sid != 0 {
                        pc.project.sketches.iter().find(|s| s.id == pc.sweep.path_sid).map(|s| format!("{} {}", qymcad_i18n::tr1("cmd-path-is", "name", &qymcad_i18n::name(&s.name)), ph::CHECK)).unwrap_or_else(|| qymcad_i18n::tr("cmd-path-unset"))
                    } else {
                        qymcad_i18n::tr("cmd-pick-path-sketch")
                    };
                    if ui.selectable_label(pc.sweep.pick_path, format!("{} {path_txt}", ph::LINE_SEGMENT)).on_hover_text(qymcad_i18n::tr("cmd-sweep-pick-path")).clicked() {
                        pc.sweep.pick_path = !pc.sweep.pick_path;
                    }
                    // choosing the path's contour through the HALF-SKETCHER (a click on the contour), when there is more than one
                    let path_cands = pc.project.sweep_path_contours(pc.sweep.path_sid);
                    if path_cands.len() > 1 {
                        let cur = path_cands.iter().position(|c| *c == pc.sweep.path_cid).map(|i| i + 1).unwrap_or(1);
                        let act = pc.picking.contour() == Some(qymcad_ui_state::ContourSlot::SweepPath);
                        if ui.selectable_label(act, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr2("cmd-pick-contour-n", "cur", &cur.to_string(), "total", &path_cands.len().to_string()))).on_hover_text(qymcad_i18n::tr("cmd-open-path-flat")).clicked() {
                            begin_contour_pick(pc, qymcad_ui_state::ContourSlot::SweepPath, pc.sweep.path_sid);
                        }
                    }
                    ui.separator();
                    says.tell(qymcad_i18n::tr("cmd-sweep-auto-hint"));
                }
                9 => {
                    // Loft: an ordered list of sections + adding by a click + ruled/smooth.
                    // A new section is taken on the EDGE of a change in the tree selection (while the pick
                    // is active). Only when the selected sketch changes - otherwise a section removed by
                    // its cross would come straight back while the same sketch stayed selected in the tree.
                    if pc.loft.pick {
                        let cur = if let qymcad_ui_state::Sel::Sketch(si) = *pc.sel { pc.project.sketches.get(si).map(|s| s.id) } else { None };
                        if cur != pc.loft.pick_last {
                            pc.loft.pick_last = cur;
                            if let Some(sid) = cur {
                                if !pc.loft.sids.contains(&sid) && !pc.project.sweep_profile_contours(sid).is_empty() {
                                    pc.loft.sids.push(sid);
                                    pc.loft.cids.push(0);
                                }
                            }
                        }
                    }
                    // the sections in order: the number + the name + a contour switch + delete
                    let mut remove: Option<usize> = None;
                    for i in 0..pc.loft.sids.len() {
                        let sid = pc.loft.sids[i];
                        let name = pc.project.sketches.iter().find(|s| s.id == sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_else(|| "—".into());
                        ui.label(egui::RichText::new(format!("{}:{name}", i + 1)).color(pc.scheme.pal.hint()));
                        let cands = pc.project.sweep_profile_contours(sid);
                        if cands.len() > 1 {
                            let cur = cands.iter().position(|c| *c == pc.loft.cids[i]).map(|k| k + 1).unwrap_or(1);
                            let act = pc.picking.contour() == Some(qymcad_ui_state::ContourSlot::LoftSection(i));
                            if ui.selectable_label(act, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr2("cmd-pick-contour-short", "cur", &cur.to_string(), "total", &cands.len().to_string()))).on_hover_text(qymcad_i18n::tr("cmd-open-section-flat")).clicked() {
                                begin_contour_pick(pc, qymcad_ui_state::ContourSlot::LoftSection(i), sid);
                            }
                        }
                        if ui.small_button(ph::X).on_hover_text(qymcad_i18n::tr("cmd-remove-section")).clicked() {
                            remove = Some(i);
                        }
                        ui.separator();
                    }
                    if let Some(i) = remove {
                        pc.loft.sids.remove(i);
                        pc.loft.cids.remove(i);
                    }
                    // the "add a section" button (picking in the tree)
                    if ui.selectable_label(pc.loft.pick, format!("{} {}", ph::PLUS, qymcad_i18n::tr("cmd-add-section"))).on_hover_text(qymcad_i18n::tr("cmd-loft-pick-hint")).clicked() {
                        pc.loft.pick = !pc.loft.pick;
                        pc.loft.pick_last = None; // the pick is on, so the current selection may be taken on the next edge
                    }
                    ui.separator();
                    // the kind of surface between the sections
                    ui.label(qymcad_i18n::tr("cmd-faces"));
                    ui.selectable_value(&mut pc.loft.ruled, false, qymcad_i18n::tr("cmd-smooth"));
                    ui.selectable_value(&mut pc.loft.ruled, true, qymcad_i18n::tr("cmd-ruled"));
                    ui.separator();
                    // the kind of result: a separate body, or a boolean with the active body
                    ui.label(qymcad_i18n::tr("cmd-result"));
                    ui.selectable_value(&mut pc.loft.result, 0u8, qymcad_i18n::tr("cmd-add"));
                    ui.selectable_value(&mut pc.loft.result, 1u8, qymcad_i18n::tr("cmd-cut"));
                    ui.selectable_value(&mut pc.loft.result, 2u8, qymcad_i18n::tr("cmd-union"));
                    ui.selectable_value(&mut pc.loft.result, 3u8, qymcad_i18n::tr("cmd-intersection"));
                    // a surface through the sections: the same loft, not closed into a body
                    ui.selectable_value(&mut pc.loft.result, 4u8, qymcad_i18n::tr("cmd-surface"));
                    if pc.loft.result != 0 && pc.loft.result != 4 {
                        let has = qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: &*pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path }).is_some();
                        if has {
                            says.tell(qymcad_i18n::tr("cmd-bool-with-active"));
                        } else {
                            says.wrong(qymcad_i18n::tr("cmd-no-active-body"));
                        }
                    }
                    ui.separator();
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-sections-n", "n", &pc.loft.sids.len().to_string())).weak().small());
                }
                5 => {
                    // Chamfer: the mode (symmetric, two distances, leg and angle) + the side of the reference face
                    use qymcad_core::feature::ChamferMode;
                    ui.label(qymcad_i18n::tr("cmd-mode"));
                    let prev = pc.chamfer.mode;
                    ui.selectable_value(&mut pc.chamfer.mode, ChamferMode::Symmetric, qymcad_i18n::tr("cmd-symmetric"));
                    ui.selectable_value(&mut pc.chamfer.mode, ChamferMode::TwoDist, qymcad_i18n::tr("cmd-two-distances"));
                    ui.selectable_value(&mut pc.chamfer.mode, ChamferMode::DistAngle, qymcad_i18n::tr("cmd-leg-angle"));
                    if pc.chamfer.mode != prev {
                        // the meaning of field d2 changed (mm <-> degrees), so both the label AND the value
                        // at the geometry are updated (45 mm as a second leg, or 1.5 deg as an angle, are
                        // both meaningless, hence the mode's default)
                        let def = if pc.chamfer.mode == ChamferMode::DistAngle { 45.0 } else { 1.5 };
                        if let Some(p) = pc.cmd.params.iter_mut().find(|p| p.key == "d2") {
                            p.set_label(qymcad_ui_state::chamfer_d2_label(pc.chamfer.mode));
                            p.val = def;
                            p.txt = format!("{def:.2}");
                        }
                    }
                    if pc.chamfer.mode != ChamferMode::Symmetric {
                        ui.separator();
                        if ui.selectable_label(pc.chamfer.flip, format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-side-btn"))).on_hover_text(qymcad_i18n::tr("cmd-swap-ref-face")).clicked() {
                            pc.chamfer.flip = !pc.chamfer.flip;
                        }
                        // picking the reference face by hand: a click on a face in 3D. It overrides "Side"
                        // for the edges adjacent to that face. Clicking the button again, or Reset, clears it.
                        ui.separator();
                        let lbl = if pc.chamfer.ref_face != 0 {
                            format!("{} {} {}", ph::CUBE, qymcad_i18n::tr("cmd-ref-face"), ph::CHECK)
                        } else {
                            format!("{} {}", ph::CUBE, qymcad_i18n::tr("cmd-ref-face"))
                        };
                        if ui.selectable_label(pc.chamfer.pick_ref, lbl).on_hover_text(qymcad_i18n::tr("cmd-chamfer-ref-hint")).clicked() {
                            pc.chamfer.pick_ref = !pc.chamfer.pick_ref;
                        }
                        if pc.chamfer.ref_face != 0 && ui.small_button(qymcad_i18n::tr("cmd-reset")).on_hover_text(qymcad_i18n::tr("cmd-neutral-auto-back")).clicked() {
                            pc.chamfer.ref_face = 0;
                            pc.chamfer.pick_ref = false;
                        }
                        says.tell(qymcad_i18n::tr("cmd-asym-note"));
                    }
                    ui.separator();
                    // A COUNT LIES WHEN THE SELECTION IS DESCRIBED. "Edges: 4" is a snapshot of today,
                    // while the description "every edge of this face" will take a fifth one tomorrow. What
                    // is actually recorded is written out in words.
                    let what = match &pc.gsel.described {
                        Some(_) => qymcad_i18n::tr1("expand-described", "what", &qymcad_i18n::tr("expand-face-edges")),
                        None => qymcad_i18n::tr1("cmd-edges-n", "n", &pc.gsel.edges.len().to_string()),
                    };
                    ui.label(egui::RichText::new(what).weak());
                }
                32 => {
                    // PATCH: smooth, or by position. A switch rather than a checkbox in the corner: these
                    // are two DIFFERENT surfaces on one boundary, and the choice is visible before Enter.
                    if ui.selectable_label(!pc.opts.patch_tangent, qymcad_i18n::tr("cmd-patch-flat")).clicked() {
                        pc.opts.patch_tangent = false;
                    }
                    if ui.selectable_label(pc.opts.patch_tangent, qymcad_i18n::tr("cmd-patch-tangent")).clicked() {
                        pc.opts.patch_tangent = true;
                    }
                    ui.separator();
                    let what = match &pc.gsel.described {
                        Some(_) => qymcad_i18n::tr1("expand-described", "what", &qymcad_i18n::tr("expand-face-edges")),
                        None => qymcad_i18n::tr1("cmd-edges-n", "n", &pc.gsel.edges.len().to_string()),
                    };
                    ui.label(egui::RichText::new(what).weak());
                }
                6 => {
                    // Shell: the direction of the thickness + the count of the multi-selected faces
                    ui.label(qymcad_i18n::tr("cmd-thickness"));
                    // a three-position mode: inwards, outwards or centred
                    use qymcad_core::feature::ShellSide;
                    for (side, word) in [(ShellSide::Inward, "cmd-inwards"), (ShellSide::Outward, "cmd-outwards"), (ShellSide::Centred, "cmd-centred")] {
                        if ui.selectable_label(pc.opts.shell_side == side, qymcad_i18n::tr(word)).clicked() {
                            pc.opts.shell_side = side;
                        }
                    }
                    ui.separator();
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-open-faces-n", "n", &pc.gsel.faces.len().to_string())).color(pc.scheme.pal.hint()));
                }
                23 => {
                    // Draft: the set of faces to tilt + the neutral face (the fixed section plane) + a flip
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-draft-faces-n", "n", &pc.gsel.faces.len().to_string())).color(pc.scheme.pal.selected()));
                    ui.separator();
                    let lbl = if pc.draft.neutral != 0 {
                        format!("{} {} {}", ph::CUBE, qymcad_i18n::tr("cmd-neutral-face"), ph::CHECK)
                    } else {
                        format!("{} {}", ph::CUBE, qymcad_i18n::tr("cmd-neutral-face"))
                    };
                    if ui.selectable_label(pc.draft.pick_neutral, lbl).on_hover_text(qymcad_i18n::tr("cmd-draft-neutral-hint")).clicked() {
                        pc.draft.pick_neutral = !pc.draft.pick_neutral;
                    }
                    if pc.draft.neutral != 0 && ui.small_button(qymcad_i18n::tr("cmd-reset")).on_hover_text(qymcad_i18n::tr("cmd-clear-neutral")).clicked() {
                        pc.draft.neutral = 0;
                        pc.draft.pick_neutral = false;
                    }
                    ui.separator();
                    if ui.selectable_label(pc.draft.flip, format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-flip-back"))).on_hover_text(qymcad_i18n::tr("cmd-flip-draft-hint")).clicked() {
                        pc.draft.flip = !pc.draft.flip;
                    }
                    ui.separator();
                    says.tell(qymcad_i18n::tr("cmd-angle-field-hint"));
                }
                24 => {
                    // Thread: inner/outer + the number of starts + the hand; the pitch, length, angle and depth live at the geometry
                    ui.label(qymcad_i18n::tr("cmd-kind"));
                    ui.selectable_value(&mut pc.thread.internal, false, qymcad_i18n::tr("cmd-thread-external")).on_hover_text(qymcad_i18n::tr("cmd-on-cylinder"));
                    ui.selectable_value(&mut pc.thread.internal, true, qymcad_i18n::tr("cmd-thread-internal")).on_hover_text(qymcad_i18n::tr("cmd-in-hole"));
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-starts"));
                    // The number of starts is a dimension of the part as well, and it must be parametric:
                    // a two-start thread driven by a global variable is an ordinary thing.
                    let st = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme }, ui, "thread_starts", pc.thread.starts.max(1) as f64, qymcad_ui_state::NumFormat { lo: 1.0, hi: 12.0, integer: true, suffix: "" });
                    pc.thread.starts = (st as u32).max(1);
                    ui.separator();
                    if ui.selectable_label(pc.thread.left, format!("{} {}", ph::ARROWS_COUNTER_CLOCKWISE, qymcad_i18n::tr("cmd-left-thread"))).on_hover_text(qymcad_i18n::tr("cmd-thread-left")).clicked() {
                        pc.thread.left = !pc.thread.left;
                    }
                    ui.separator();
                    // THE MODE: a thread (a groove) or an AUGER (a helical ribbon outwards)
                    if ui.selectable_label(!pc.thread.auger, format!("{} {}", ph::SPIRAL, qymcad_i18n::tr("cmd-thread-btn"))).clicked() && pc.thread.auger {
                        pc.thread.auger = false;
                        qymcad_ui_state::set_thread_params(&mut *pc.cmd, *pc.thread); // the modes have different fields
                    }
                    if ui.selectable_label(pc.thread.auger, format!("{} {}", ph::SPIRAL, qymcad_i18n::tr("cmd-auger-btn"))).on_hover_text(qymcad_i18n::tr("cmd-auger-hint")).clicked() && !pc.thread.auger {
                        pc.thread.auger = true;
                        qymcad_ui_state::set_thread_params(&mut *pc.cmd, *pc.thread);
                    }
                    ui.separator();
                    if pc.thread.auger {
                        says.tell(qymcad_i18n::tr("cmd-auger-fields-hint"));
                    } else {
                        // THE STANDARD (the thread's type): the geometry is computed by the model core
                        ui.label(qymcad_i18n::tr("cmd-standard"));
                        use qymcad_core::thread::ThreadStandard as TS;
                        for (idx, std) in [(0u8, TS::MetricIso), (1, TS::TrapezoidalTr), (2, TS::Acme), (3, TS::RoundRd), (4, TS::Buttress), (5, TS::Custom)] {
                            let short = match idx {
                                1 => "Tr",
                                2 => "ACME",
                                3 => "Rd",
                                4 => &qymcad_i18n::tr("cmd-stop"),
                                5 => &qymcad_i18n::tr("cmd-custom"),
                                _ => "M",
                            };
                            if ui.selectable_label(pc.thread.form == idx, short).on_hover_text(qymcad_i18n::tr(std.label())).clicked() {
                                pc.thread.form = idx;
                                sync_custom_params(&mut *pc.cmd, *pc.thread); // "custom" opens the angle and the depth, the others hide them
                            }
                        }
                        ui.separator();
                        // a hint about the actual geometry of the chosen size
                        let spec = thread_spec(&*pc.cmd, *pc.thread);
                        let g = spec.geometry();
                        says.tell_hover(
                            qymcad_i18n::trn("cmd-thread-geom", &[("pitch", &qymcad_i18n::num(g.pitch, 2)), ("d2", &qymcad_i18n::num(g.pitch_d, 2)), ("d3", &qymcad_i18n::num(g.minor_d, 2)), ("depth", &qymcad_i18n::num(g.depth, 2))]),
                            qymcad_i18n::tr("cmd-thread-std-hint"),
                        );
                        // WHAT THE MATING PART NEEDS, said out loud. Asked for plainly: a person must
                        // see what diameter of shaft or hole this thread wants and with what parameters
                        // to make its counterpart - otherwise the numbers get looked up in a table, and
                        // a table does not know about the fit that was typed in here.
                        let (own, mate) = spec.blank_diameters();
                        let key = if pc.thread.internal { "cmd-thread-mate-internal" } else { "cmd-thread-mate-external" };
                        says.tell_hover(qymcad_i18n::tr2(key, "own", &qymcad_i18n::num(own, 2), "mate", &qymcad_i18n::num(mate, 2)), qymcad_i18n::tr("cmd-thread-mate-hint"));
                        // A PROFILE THAT DOES NOT FIT THE PITCH is said out loud, with the numbers to
                        // change. It used to be taken in silence and built as rubbish: the passes
                        // overlap, eat the turn between them and leave flat plates that mate with
                        // nothing.
                        // THE CLEARANCE THAT DOES NOT FIT is named too. It saturates against the
                        // pitch without a word, and two different numbers typed in then give one and
                        // the same body - the pair binds and nothing explains why.
                        if let Some((asked, given)) = spec.fit_overflow() {
                            says.wrong(qymcad_i18n::trn(
                                "cmd-thread-fit-capped",
                                &[
                                    ("asked", &qymcad_i18n::num(asked, 2)),
                                    ("given", &qymcad_i18n::num(given, 3)),
                                    // WHAT IS LEFT OVER, said as a DIAMETER correction. The missing
                                    // clearance is measured along the flank, and a radial move is not
                                    // worth the same: a flank stands at the half-angle, so
                                    // `radial_relief` converts one into the other. Twice it, because a
                                    // diameter has two sides.
                                    ("rest", &qymcad_i18n::num(spec.radial_relief() * 2.0, 2)),
                                ],
                            ));
                        }
                        if let Some((width, max_depth, min_pitch)) = spec.profile_overflow() {
                            says.wrong(qymcad_i18n::trn(
                                "cmd-thread-too-wide",
                                &[
                                    ("width", &qymcad_i18n::num(width, 2)),
                                    ("pitch", &qymcad_i18n::num(spec.geometry().pitch, 2)),
                                    ("depth", &qymcad_i18n::num(max_depth, 2)),
                                    ("minpitch", &qymcad_i18n::num(min_pitch, 2)),
                                ],
                            ));
                        }
                    }
                    ui.separator();
                    let tgt = if pc.thread.edge == 0 {
                        qymcad_i18n::tr("cmd-thread-pick-hint")
                    } else {
                        qymcad_i18n::tr1("cmd-actual-diameter", "d", &qymcad_i18n::num(pc.thread.radius * 2.0, 1))
                    };
                    says.tell(tgt);
                }
                7 => {
                    // Hole: the placement mode (a face or a sketch) + the type; the diameter and depth live at the geometry
                    ui.label(qymcad_i18n::tr("cmd-placement"));
                    ui.selectable_value(&mut pc.hole.mode, 0u8, qymcad_i18n::tr("cmd-by-face"));
                    ui.selectable_value(&mut pc.hole.mode, 1u8, qymcad_i18n::tr("cmd-by-sketch"));
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-kind"));
                    ui.selectable_value(&mut pc.hole.kind, 0u8, qymcad_i18n::tr("cmd-simple"));
                    ui.selectable_value(&mut pc.hole.kind, 1u8, qymcad_i18n::tr("cmd-counterbore"));
                    ui.selectable_value(&mut pc.hole.kind, 2u8, qymcad_i18n::tr("cmd-countersink"));
                    ui.separator();
                    if pc.hole.mode == 1 {
                        // "from a sketch": the selected sketch is taken, the number of marker points is shown, plus a normal flip
                        if let qymcad_ui_state::Sel::Sketch(si) = *pc.sel {
                            if let Some(s) = pc.project.sketches.get(si) {
                                pc.hole.sketch = Some(s.id);
                            }
                        }
                        let n = pc.hole.sketch.map(|sid| pc.project.sketch_isolated_points(sid).len()).unwrap_or(0);
                        let txt = match pc.hole.sketch {
                            Some(sid) => pc.project.sketches.iter().find(|s| s.id == sid).map(|s| qymcad_i18n::tr2("cmd-sketch-points", "name", &qymcad_i18n::name(&s.name), "n", &n.to_string())).unwrap_or_else(|| qymcad_i18n::tr("cmd-sketch-unset")),
                            None => qymcad_i18n::tr("cmd-pick-sketch-points"),
                        };
                        let col = if n > 0 { pc.scheme.pal.hint() } else { pc.scheme.pal.hint_action() };
                        ui.label(egui::RichText::new(txt).color(col));
                        ui.separator();
                        ui.checkbox(&mut pc.hole.flip, qymcad_i18n::tr("cmd-flip"));
                    } else {
                        says.tell(qymcad_i18n::tr("cmd-hole-face-hint"));
                    }
                }
                15 => {
                    // Prism: the number of sides + a hint; the diameter and the height are fields at the geometry
                    ui.label(qymcad_i18n::tr("cmd-sides"));
                    pc.prim.n = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme }, ui, "prim_n", pc.prim.n as f64, qymcad_ui_state::NumFormat { lo: 3.0, hi: 64.0, integer: true, suffix: "" }) as u32;
                    ui.separator();
                    says.tell(qymcad_i18n::tr("cmd-dia-height-hint"));
                }
                10..=14 => {
                    says.tell(qymcad_i18n::tr("cmd-sizes-hint"));
                }
                16 => {
                    // Mirror: keep the original + CLICK-PICK the plane in the viewport + a readout of what was picked
                    use qymcad_core::feature::SketchPlane;
                    ui.checkbox(&mut pc.opts.mirror_keep, qymcad_i18n::tr("cmd-with-original"));
                    ui.separator();
                    let picked = match &pc.mirror.plane {
                        Some(SketchPlane::World(bp)) => qymcad_i18n::tr1("cmd-world-plane", "plane", ["XY", "XZ", "YZ"][*bp as usize]),
                        Some(SketchPlane::Datum(id)) => pc.project.planes.iter().find(|p| p.id == *id).map(|p| qymcad_i18n::tr1("cmd-datum-named", "name", &p.name)).unwrap_or_else(|| qymcad_i18n::tr("cmd-datum")),
                        Some(SketchPlane::Face(b, _)) => qymcad_i18n::tr1("cmd-body-face-n", "b", &b.to_string()),
                        None => qymcad_i18n::tr("cmd-pick-plane"),
                    };
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-plane-is", "what", &picked)).color(pc.scheme.pal.hint()));
                }
                17 => {
                    // A linear pattern: the count + the direction (X/Y/Z) + a grid (a second direction);
                    // THE STEP is an expression field at the geometry (a popup). Ghost previews on the canvas.
                    ui.label(qymcad_i18n::tr("cmd-copies"));
                    pc.arr.count = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme }, ui, "arr_count", pc.arr.count as f64, qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "" }) as u32;
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-direction"));
                    ui.selectable_value(&mut pc.arr.dir, 0u8, "X");
                    ui.selectable_value(&mut pc.arr.dir, 1u8, "Y");
                    ui.selectable_value(&mut pc.arr.dir, 2u8, "Z");
                    ui.separator();
                    ui.checkbox(&mut pc.arr.two, qymcad_i18n::tr("cmd-dir2"));
                    if pc.arr.two {
                        pc.arr.count2 = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme }, ui, "arr_count2", pc.arr.count2 as f64, qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "" }) as u32;
                        ui.selectable_value(&mut pc.arr.dir2, 0u8, "X");
                        ui.selectable_value(&mut pc.arr.dir2, 1u8, "Y");
                        ui.selectable_value(&mut pc.arr.dir2, 2u8, "Z");
                        ui.separator();
                        ui.checkbox(&mut pc.arr.three, qymcad_i18n::tr("cmd-dir3"));
                        if pc.arr.three {
                            pc.arr.count3 = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme }, ui, "arr_count3", pc.arr.count3 as f64, qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "" }) as u32;
                            ui.selectable_value(&mut pc.arr.dir3, 0u8, "X");
                            ui.selectable_value(&mut pc.arr.dir3, 1u8, "Y");
                            ui.selectable_value(&mut pc.arr.dir3, 2u8, "Z");
                        }
                    } else {
                        pc.arr.three = false; // the third direction only sits on top of the second (a full 3D grid)
                    }
                    ui.separator();
                    says.tell(qymcad_i18n::tr("cmd-pitch-field-hint"));
                }
                18 => {
                    // A circular pattern: the count + the axis by CLICK-PICK (a datum axis or a straight edge)
                    // + a full circle; THE ANGLE is an expression field at the geometry (when it is not a full
                    // circle). Ghost previews.
                    ui.label(qymcad_i18n::tr("cmd-copies"));
                    pc.arr.count = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme }, ui, "arr_count", pc.arr.count as f64, qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "" }) as u32;
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-axis"));
                    let axname = if pc.arr.axis == 0 { qymcad_i18n::tr("cmd-axis-world-z") } else { pc.project.datum_axes.iter().find(|d| d.id == pc.arr.axis).map(|d| qymcad_i18n::name(&d.name)).unwrap_or_else(|| qymcad_i18n::tr("cmd-axis-world-z")) };
                    // the axis is CLICK-PICKED, as the mirror plane is, rather than chosen from a combo box
                    if ui.selectable_label(pc.arr.axis_pick, format!("{} {axname}", ph::CROSSHAIR)).on_hover_text(qymcad_i18n::tr("cmd-revolve-pick-axis3d")).clicked() {
                        pc.arr.axis_pick = !pc.arr.axis_pick;
                        if pc.arr.axis_pick {
                            refresh_axis_edges(pc); // the straight edges of EVERY visible body, as axis candidates
                            *pc.status = qymcad_i18n::tr("cmd-array-axis-hint");
                        }
                    }
                    if pc.arr.axis != 0 && ui.small_button("Z").on_hover_text(qymcad_i18n::tr("cmd-reset-axis-z")).clicked() {
                        pc.arr.axis = 0;
                        pc.arr.axis_pick = false;
                    }
                    ui.separator();
                    ui.checkbox(&mut pc.arr.full, qymcad_i18n::tr("cmd-full-circle"));
                    if !pc.arr.full {
                        says.tell(qymcad_i18n::tr("cmd-angle-field-lower"));
                    }
                }
                20 => {
                    // A datum PLANE: click-pick a base plane, a datum or a face + the offset at the geometry
                    use qymcad_core::feature::SketchPlane;
                    let picked = match &pc.datum.plane_pick {
                        Some(SketchPlane::World(bp)) => qymcad_i18n::tr1("cmd-world-plane", "plane", ["XY", "XZ", "YZ"][*bp as usize]),
                        Some(SketchPlane::Datum(id)) => pc.project.planes.iter().find(|p| p.id == *id).map(|p| qymcad_i18n::tr1("cmd-datum-named", "name", &p.name)).unwrap_or_else(|| qymcad_i18n::tr("cmd-datum")),
                        Some(SketchPlane::Face(b, _)) => qymcad_i18n::tr1("cmd-body-face-n", "b", &b.to_string()),
                        None => qymcad_i18n::tr("cmd-pick-plane"),
                    };
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-from-is", "what", &picked)).color(pc.scheme.pal.hint()));
                    ui.separator();
                    says.tell(qymcad_i18n::tr("cmd-offset-field-hint"));
                }
                21 => {
                    // A datum POINT: X/Y/Z coordinates, or associatively at a vertex (by a click)
                    let prev = pc.datum.pt_mode;
                    ui.label(qymcad_i18n::tr("cmd-method"));
                    ui.selectable_value(&mut pc.datum.pt_mode, 0u8, qymcad_i18n::tr("cmd-coordinates"));
                    ui.selectable_value(&mut pc.datum.pt_mode, 1u8, qymcad_i18n::tr("cmd-to-vertex")).on_hover_text(qymcad_i18n::tr("cmd-vertex-assoc-hint"));
                    if prev != pc.datum.pt_mode {
                        sync_datum_point_params(&*pc.armed, &mut *pc.cmd, &*pc.datum);
                    }
                    ui.separator();
                    if pc.datum.pt_mode == 1 {
                        let r = if pc.datum.pt_vert.is_some() { format!("{} {}", qymcad_i18n::tr("cmd-vertex-picked"), ph::CHECK) } else { qymcad_i18n::tr("cmd-pick-vertex") };
                        ui.label(egui::RichText::new(r).color(pc.scheme.pal.hint()));
                    } else {
                        says.tell(qymcad_i18n::tr("cmd-xyz-hint"));
                    }
                }
                22 => {
                    // A datum AXIS: by a click on an edge or a face, by two points, or by hand (origin and direction at the geometry)
                    ui.label(qymcad_i18n::tr("cmd-method"));
                    ui.selectable_value(&mut pc.datum.axis_mode, 0u8, qymcad_i18n::tr("cmd-by-edge-face"));
                    ui.selectable_value(&mut pc.datum.axis_mode, 2u8, qymcad_i18n::tr("cmd-two-points"));
                    ui.selectable_value(&mut pc.datum.axis_mode, 1u8, qymcad_i18n::tr("cmd-manual"));
                    ui.separator();
                    match pc.datum.axis_mode {
                        0 => {
                            let r = if pc.datum.axis_ref.is_some() { format!("{} {}", qymcad_i18n::tr("cmd-axis-picked"), ph::CHECK) } else { qymcad_i18n::tr("cmd-pick-edge-cyl") };
                            ui.label(egui::RichText::new(r).color(pc.scheme.pal.hint()));
                        }
                        2 => {
                            ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-points-n", "n", &pc.datum.axis_pts.len().to_string())).color(pc.scheme.pal.hint()));
                        }
                        _ => {
                            says.tell(qymcad_i18n::tr("cmd-origin-dir-hint"));
                        }
                    }
                }
                _ => {
                    says.tell(cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // the button is disabled while a dimension's expression is invalid (nothing stale is applied)
                if confirm_button(&pc.scheme.pal, ui, cmd_ready(pc) && cmd_exprs_valid(&*pc.cmd, &*pc.project), format!("{} {}", ph::CHECK, qymcad_i18n::tr("cmd-apply-enter"))) {
                    apply = true;
                }
                if refuse_button(&pc.scheme.pal, ui, format!("{} {}", ph::X, qymcad_i18n::tr("cmd-cancel-esc"))) {
                    cancel = true;
                }
                // the "pick contours" button belongs to the 3D step (setting the size) only - inside the
                // half-sketcher (mode_3d=false) contours are ALREADY being picked, so duplicating it there
                // serves nothing.
                if matches!(pc.armed.cmd_kind(), 1 | 3) && pc.cmd.sketch.is_some() && *pc.mode_3d {
                    // THE KEY IN THE HINT FOLLOWS THE FOCUS STATE: "U" or "Alt+U". Otherwise the rule that
                    // a focused field needs Alt would stay a secret, and someone who pressed `U` in a field
                    // and got nothing would not try a second time.
                    let hint = format!("{}  ({})", qymcad_i18n::tr("cmd-back-to-contours"), qymcad_ui_state::hotkey_hint(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: &*pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path }, ctx, "part.contour-reselect"));
                    if ui.button(format!("{} {}", ph::POLYGON, qymcad_i18n::tr("cmd-pick-contours"))).on_hover_text(&hint).clicked() {
                        reselect = true;
                    }
                }
            });
        });
        says.show(ui, &pc.scheme.pal);
        if apply {
            apply_feat_cmd(pc);
        }
        if cancel {
            qymcad_ui_state::cancel_feat_cmd(pc);
        }
        if reselect {
            enter_contour_reselect(pc);
        }
        // after the bar (the extent mode is chosen) the "second side" field at the geometry is synced
        sync_dir_cmd_params(&*pc.armed, &mut *pc.cmd, &*pc.project);
        // the pattern: a second step when there is a second direction, and an angle when it is not a full circle
        sync_array_params(*pc.arr, &*pc.armed, &mut *pc.cmd, &*pc.project);
        // the datum axis: the origin and direction fields in manual mode
        sync_datum_axis_params(&*pc.armed, &mut *pc.cmd, &*pc.datum);
        sync_hole_params(&*pc.armed, &mut *pc.cmd, *pc.hole, &*pc.project); // the recess fields for a counterbore or a countersink
    }
}

/// The shared Create buttons: THE DATUMS — they exist both in a Part and in an Assembly. A sketch is
/// added separately and ONLY in a Part (see `create_panel_part`).
///
/// SKETCHES ARE GONE FROM ASSEMBLIES. The button stood there marked "skeleton", so the intent of
/// top-down layout was there, but nothing could refer to that skeleton: a joint anchor understands
/// `Origin`, `BasePlane`, `FaceCenter`, `EdgeMid` and `Vertex` — there is no reference to a sketch, and
/// there is no extrude in the Assembly workbench either. A sketch there was inert: it could be drawn
/// but not used. Layout belongs in the part-modelling workbench.
///
/// DATUMS STAY IN AN ASSEMBLY — they are not inert: a mirrored copy of a component and the view's
/// section both consume them.
pub fn create_panel_common(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    use qymcad_core::feature::{BasePlane, SketchPlane};
    let _ = (BasePlane::XY, SketchPlane::default);
    // Datums are COMMANDS: an options bar, fields at the geometry, click-picked references, a preview, Enter and Esc
    if qymcad_ui_state::icon_tool(ui, ph::SELECTION_ALL, &qymcad_i18n::tr("g-datum-plane-hint"), bc.armed.cmd_kind() == 20) {
        bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(20));
    }
    if qymcad_ui_state::icon_tool(ui, ph::DOT, &qymcad_i18n::tr("g-datum-point-hint"), bc.armed.cmd_kind() == 21) {
        bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(21));
    }
    if qymcad_ui_state::icon_tool(ui, ph::LINE_SEGMENT, &qymcad_i18n::tr("g-datum-axis-hint"), bc.armed.cmd_kind() == 22) {
        bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(22));
    }
}

/// The one doorway into the properties panel.
/// The Sketch button — ONLY in the Part workbench (in an assembly a sketch is inert, see `create_panel_common`).
pub fn create_panel_sketch_button(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    if qymcad_ui_state::icon_tool(ui, ph::PENCIL_SIMPLE, &qymcad_i18n::tr("g-sketch-pick-hint"), bc.picking.is_sketch_plane()) {
        bc.ask.push(qymcad_ui_state::BarAsk::ToggleSketchPick);
    }
}


pub fn toolbar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        // CONTEXT BREADCRUMBS instead of tabs (drilling in and out): Assembly > Part > ... (> Sketch)
        qymcad_ui_state::ensure_active_path(&mut *bc.active_path, &mut *bc.project);
        let path = bc.active_path.clone();
        for (i, cid) in path.iter().enumerate() {
            if i > 0 {
                ui.label("›");
            }
            let name = bc.project.components.iter().find(|c| c.id == *cid).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_else(|| qymcad_i18n::tr("wb-assembly"));
            // the root and an assembly wear the same icon: both hold other things rather than being a shape
            let assembly = i == 0 || bc.project.component_kind(*cid) == Some(qymcad_core::feature::ComponentKind::Assembly);
            let icon = if assembly { ph::STACK } else { ph::CUBE };
            let is_here = i + 1 == path.len() && bc.sketch_ses.editing.is_none();
            if ui.selectable_label(is_here, format!("{icon} {name}")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::GotoPath(i));
            }
        }
        if let Some(sid) = bc.sketch_ses.editing {
            ui.label("›");
            let nm = bc.project.sketches.iter().find(|s| s.id == sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_else(|| qymcad_i18n::tr("wb-sketch"));
            ui.label(egui::RichText::new(format!("{} {nm}", ph::PENCIL)).color(bc.scheme.pal.hint()));
        }
        if (bc.sketch_ses.editing.is_some() || bc.active_path.len() > 1)
            && ui.button(qymcad_i18n::tr("wb-finish")).on_hover_text(qymcad_i18n::tr("wb-finish-hint")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::ExitContext);
            }
        ui.separator();
        // the global parameters are reachable from any workbench
        if ui.selectable_label(bc.win.is(WinKind::Params), qymcad_i18n::tr("wb-params")).on_hover_text(qymcad_i18n::tr("wb-params-hint")).clicked() {
            bc.win.toggle(WinKind::Params);
        }
        ui.separator();

        if let Some(pts) = bc.pending_import.draw_pts.as_ref() {
            // a sketch is being drawn: finish or cancel
            let n = pts.len();
            ui.label(qymcad_i18n::tr1("wb-points-n", "n", &n.to_string()));
            if ui.button(qymcad_i18n::tr("wb-close")).clicked() {
                qymcad_ui_state::finish_drawing(qymcad_ui_state::editing_in!(bc), &mut *bc.pending_import, *bc.sketch_ses, true);
            }
            if ui.button(qymcad_i18n::tr("wb-line")).clicked() {
                qymcad_ui_state::finish_drawing(qymcad_ui_state::editing_in!(bc), &mut *bc.pending_import, *bc.sketch_ses, false);
            }
            if ui.button(qymcad_i18n::tr("wb-cancel")).clicked() {
                bc.pending_import.draw_pts = None;
            }
            ui.separator();
        }

        // Snap and the grid: while DRAWING a sketch (the cursor) AND in 3D inside a Part or an Assembly (snapping the gizmo to the grid).
        let snap_ctx = bc.sketch_ses.editing.is_some() || (*bc.mode_3d && matches!(bc.workbench, qymcad_ui_state::Workbench::Part | qymcad_ui_state::Workbench::Assembly));
        if snap_ctx {
            ui.toggle_value(&mut bc.set.snap.on, format!("{} Snap", ph::MAGNET)).on_hover_text(qymcad_i18n::tr("wb-snap-hint"));
            if bc.set.snap.on {
                ui.add(egui::DragValue::new(&mut bc.set.snap.grid).speed(0.5).range(0.1..=100.0).prefix(qymcad_i18n::tr("wb-grid")).suffix(qymcad_i18n::tr("unit-mm-suffix")));
                // the gizmo's ROTATION step belongs to 3D inside a Part or an Assembly (a sketch has no use for it)
                if *bc.mode_3d && matches!(bc.workbench, qymcad_ui_state::Workbench::Part | qymcad_ui_state::Workbench::Assembly) {
                    ui.add(
                        egui::DragValue::new(&mut bc.set.snap.rot_deg)
                            .speed(1.0)
                            .range(0.5..=90.0)
                            .prefix(qymcad_i18n::tr("wb-rotation"))
                            .suffix(qymcad_i18n::tr("unit-deg-suffix")),
                    );
                }
            }
            // the automatic constraints belong to drawing a sketch only
            if bc.sketch_ses.editing.is_some() {
                ui.toggle_value(&mut bc.set.auto_constrain, format!("{} {}", ph::MAGIC_WAND, qymcad_i18n::tr("wb-auto-constraints"))).on_hover_text(qymcad_i18n::tr("wb-auto-constraints-hint"));
            }
            ui.separator();
        }
        // "in context" applies inside ANY component (a part or a subassembly), not at the root: top-down
        // references to the neighbours + showing the PARENT datums. The root has no ancestors, so it is hidden there.
        if qymcad_ui_state::current_ctx_id(&*bc.active_path, &*bc.project) != bc.project.root {
            ui.toggle_value(&mut bc.win.context, format!("{} {}", ph::STACK, qymcad_i18n::tr("wb-in-context"))).on_hover_text(qymcad_i18n::tr("wb-in-context-hint"));
            ui.separator();
        }
    });
}

pub fn wb_toolbar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    // Two columns of buttons (wrapping by width) + vertical scrolling - reliable at any window size.
    // The width 94 = 16 (the 8+8 margins) + 6 (the floating scrollbar) + 72 (two 34-wide button columns + a
    // 3-point gap + a margin). Before that, 90 with 38-wide buttons could not fit the second column, giving
    // one column and a wide empty strip on the right.
    // show_separator_line(false): by default egui draws a faint vertical line at the right edge of ANY panel,
    // and it read as a stray gap between the tools and the tree.
    ui.add_space(6.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
        ui.horizontal_wrapped(|ui| {
        match bc.workbench {
            qymcad_ui_state::Workbench::Sketch => {
                // a full-width category label, which therefore starts a new row (breaking the columns)
                let cat = |ui: &mut egui::Ui, s: &str| {
                    ui.add_sized([84.0, 12.0], egui::Label::new(egui::RichText::new(s).size(8.5).weak()));
                };
                // "Finish" lives in the breadcrumbs (one place for it); here there are only sketch tools
                // --- Creation ---
                cat(ui, &qymcad_i18n::tr("tb-group-create"));
                if qymcad_ui_state::icon_tool(ui, ph::CURSOR, &qymcad_i18n::tr("tb-select-hint"), qymcad_ui_state::in_select_mode(&bc.armed)) {
                    bc.ask.push(qymcad_ui_state::BarAsk::SketchSelectMode);
                }
                if qymcad_ui_state::icon_tool(ui, ph::DOT, &qymcad_i18n::tr("tb-point-hint"), bc.armed.draw_kind() == 5) {
                    bc.ask.push(qymcad_ui_state::BarAsk::SketchTool(5));
                }
                if qymcad_ui_state::icon_tool(ui, ph::LINE_SEGMENT, &qymcad_i18n::tr("tb-line-hint"), bc.armed.draw_kind() == 1) {
                    bc.ask.push(qymcad_ui_state::BarAsk::SketchTool(1));
                }
                if qymcad_ui_state::icon_tool(ui, ph::RECTANGLE, &qymcad_i18n::tr("tb-rect-hint"), bc.armed.draw_kind() == 2) {
                    bc.ask.push(qymcad_ui_state::BarAsk::SketchTool(2));
                }
                if qymcad_ui_state::icon_tool(ui, ph::CIRCLE, &qymcad_i18n::tr("tb-circle-hint"), bc.armed.draw_kind() == 3) {
                    bc.ask.push(qymcad_ui_state::BarAsk::SketchTool(3));
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::Circle3, &qymcad_i18n::tr("tb-circle-3pt"), bc.armed.draw_kind() == 10) {
                    bc.ask.push(qymcad_ui_state::BarAsk::SketchTool(10));
                }
                if qymcad_ui_state::icon_tool(ui, ph::PATH, &qymcad_i18n::tr("tb-arc-hint"), bc.armed.draw_kind() == 4) {
                    bc.ask.push(qymcad_ui_state::BarAsk::SketchTool(4));
                }
                if qymcad_ui_state::icon_tool(ui, ph::HEXAGON, &qymcad_i18n::tr("tb-polygon-hint"), bc.armed.draw_kind() == 6) {
                    bc.ask.push(qymcad_ui_state::BarAsk::SketchTool(6));
                }
                if qymcad_ui_state::icon_tool(ui, ph::PILL, &qymcad_i18n::tr("tb-slot-hint"), bc.armed.draw_kind() == 7) {
                    bc.ask.push(qymcad_ui_state::BarAsk::SketchTool(7));
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::Ellipse, &qymcad_i18n::tr("tb-ellipse-hint"), bc.armed.draw_kind() == 8) {
                    bc.ask.push(qymcad_ui_state::BarAsk::SketchTool(8));
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::Spline, &qymcad_i18n::tr("tb-spline-hint"), bc.armed.draw_kind() == 9) {
                    bc.ask.push(qymcad_ui_state::BarAsk::SketchTool(9));
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::Text, &qymcad_i18n::tr("tb-text-hint"), bc.armed.draw_kind() == 11) {
                    bc.ask.push(qymcad_ui_state::BarAsk::SketchTool(11));
                }
                // --- The line kind ---
                cat(ui, &qymcad_i18n::tr("tb-type"));
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::Construction, &qymcad_i18n::tr("tb-construction-hint"), bc.tool.construction) {
                    bc.tool.construction = !bc.tool.construction;
                }
                // --- Editing and replication (over the selected entities) ---
                cat(ui, &qymcad_i18n::tr("tb-group-edit"));
                if qymcad_ui_state::icon_tool(ui, ph::TRASH, &qymcad_i18n::tr("tb-delete-hint"), bc.sel_sk.modify == Some(0)) {
                    qymcad_ui_state::modify_button(qymcad_ui_state::editing_in!(bc), &mut qymcad_ui_state::tools_in!(bc), *bc.sk_pat, &*bc.tool_prefs, 0);
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::Mirror, &qymcad_i18n::tr("tb-mirror-sketch-hint"), bc.sel_sk.modify == Some(1)) {
                    qymcad_ui_state::modify_button(qymcad_ui_state::editing_in!(bc), &mut qymcad_ui_state::tools_in!(bc), *bc.sk_pat, &*bc.tool_prefs, 1);
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::ArrayLin, &qymcad_i18n::tr("tb-lin-array-hint"), bc.armed.pat_op() == 1) {
                    start_pattern(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 1);
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::ArrayCirc, &qymcad_i18n::tr("tb-circ-array-hint"), bc.armed.pat_op() == 2) {
                    start_pattern(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 2);
                }
                if qymcad_ui_state::icon_tool(
                    ui,
                    ph::PROJECTOR_SCREEN,
                    &qymcad_i18n::tr("tb-project-body-hint"),
                    bc.armed.click_op() == 6,
                ) {
                    qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 6);
                    *bc.status = qymcad_i18n::tr("tb-project-hint");
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::Fillet, &qymcad_i18n::tr("tb-fillet-sketch-hint"), bc.armed.click_op() == 4) {
                    qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 4);
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::Chamfer, &qymcad_i18n::tr("tb-chamfer-sketch-hint"), bc.armed.click_op() == 5) {
                    qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 5);
                }
                if qymcad_ui_state::icon_tool(ui, ph::BOUNDING_BOX, &qymcad_i18n::tr("tb-fillet-all-hint"), false) {
                    qymcad_ui_state::fillet_all_corners(&mut *bc.corner, &mut *bc.picking, *bc.sel, &*bc.sel_sk, &mut *bc.status, &*bc.tool_prefs);
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::Offset, &qymcad_i18n::tr("tb-offset-hint"), bc.sel_sk.modify == Some(6)) {
                    qymcad_ui_state::modify_button(qymcad_ui_state::editing_in!(bc), &mut qymcad_ui_state::tools_in!(bc), *bc.sk_pat, &*bc.tool_prefs, 6);
                }
                if qymcad_ui_state::icon_tool(ui, ph::ARROWS_OUT_CARDINAL, &qymcad_i18n::tr("tb-move-hint"), bc.armed.move_op() == 1) {
                    start_move_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 1);
                }
                if qymcad_ui_state::icon_tool(ui, ph::COPY, &qymcad_i18n::tr("tb-copy-hint"), bc.armed.move_op() == 2) {
                    start_move_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 2);
                }
                if qymcad_ui_state::icon_tool(ui, ph::ARROWS_CLOCKWISE, &qymcad_i18n::tr("tb-rotate-hint"), bc.armed.move_op() == 3) {
                    start_move_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 3);
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::Trim, &qymcad_i18n::tr("tb-trim-hint"), bc.armed.click_op() == 1) {
                    qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 1);
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::Extend, &qymcad_i18n::tr("tb-extend-hint"), bc.armed.click_op() == 2) {
                    qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 2);
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::Break, &qymcad_i18n::tr("tb-break-hint"), bc.armed.click_op() == 3) {
                    qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 3);
                }
                // --- Dimensions ---
                cat(ui, &qymcad_i18n::tr("tb-group-dim"));
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::DimLin, &qymcad_i18n::tr("tb-dim-hint"), bc.armed.dim_kind() == 1) {
                    qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, &*bc.project, *bc.sel, *bc.sketch_ses, &mut *bc.status, 1);
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::DimAng, &qymcad_i18n::tr("tb-dim-angle-hint"), bc.armed.dim_kind() == 2) {
                    qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, &*bc.project, *bc.sel, *bc.sketch_ses, &mut *bc.status, 2);
                }
                if qymcad_render::sym_button(ui, qymcad_ui_state::Gly::DimRad, &qymcad_i18n::tr("tb-dim-radius-hint"), bc.armed.dim_kind() == 3) {
                    qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, &*bc.project, *bc.sel, *bc.sketch_ses, &mut *bc.status, 3);
                }
                if qymcad_ui_state::icon_tool(ui, ph::RULER, &qymcad_i18n::tr("tb-measure-hint"), bc.armed.measuring()) {
                    let on = !bc.armed.measuring();
                    // THROUGH THE DOOR, in the order every other tool uses. Setting the flag here and asking
                    // for "back to selection" afterwards turned the tool on and then straight off again.
                    qymcad_ui_state::set_measure(&mut qymcad_ui_state::tools_in!(bc), on);
                    if on {
                        *bc.mode_3d = false;
                        *bc.status = qymcad_i18n::tr("in-measure-hint");
                    } else {
                        *bc.status = qymcad_i18n::tr("in-measure-off");
                    }
                }
                // --- Constraints ---
                cat(ui, &qymcad_i18n::tr("tb-group-constraints"));
                let cons: [(qymcad_ui_state::Gly, u8, &str); 12] = [
                    (qymcad_ui_state::Gly::Coincident, 0, &qymcad_i18n::tr("con-coincident-hint")),
                    (qymcad_ui_state::Gly::Horiz, 1, &qymcad_i18n::tr("con-horizontal-hint")),
                    (qymcad_ui_state::Gly::Vert, 2, &qymcad_i18n::tr("con-vertical-hint")),
                    (qymcad_ui_state::Gly::Parallel, 3, &qymcad_i18n::tr("con-parallel-hint")),
                    (qymcad_ui_state::Gly::Perp, 4, &qymcad_i18n::tr("con-perpendicular-hint")),
                    (qymcad_ui_state::Gly::Equal, 5, &qymcad_i18n::tr("con-equal")),
                    (qymcad_ui_state::Gly::Collinear, 7, &qymcad_i18n::tr("con-collinear-hint")),
                    (qymcad_ui_state::Gly::Concentric, 8, &qymcad_i18n::tr("con-concentric-hint")),
                    (qymcad_ui_state::Gly::Tangent, 9, &qymcad_i18n::tr("con-tangent-hint")),
                    (qymcad_ui_state::Gly::Symmetric, 10, &qymcad_i18n::tr("con-symmetric-hint")),
                    (qymcad_ui_state::Gly::Midpoint, 11, &qymcad_i18n::tr("con-midpoint-hint")),
                    (qymcad_ui_state::Gly::Fix, 6, &qymcad_i18n::tr("con-fix")),
                ];
                for (g, code, tip) in cons {
                    if qymcad_render::sym_button(ui, g, tip, bc.sel_sk.constraint == Some(code)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(code));
                    }
                }
            }
            qymcad_ui_state::Workbench::Part => {
                // a full-width category label, breaking the columns (as in the Sketch)
                let cat = |ui: &mut egui::Ui, s: &str| {
                    ui.add_sized([84.0, 12.0], egui::Label::new(egui::RichText::new(s).size(8.5).weak()));
                };
                // --- Create (a sketch + the datums) ---
                cat(ui, &qymcad_i18n::tr("tb-group-create"));
                create_panel_sketch_button(bc, ui);
                create_panel_common(bc, ui);
                // --- From a sketch ---
                cat(ui, &qymcad_i18n::tr("tb-group-sketch3d"));
                if qymcad_ui_state::icon_tool(ui, ph::CUBE, &qymcad_i18n::tr("tb-extrude-hint"), tool_is_taken(&bc, 1)) {
                    bc.feat.op = 0;
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(1));
                }
                if qymcad_ui_state::icon_tool(ui, ph::ARROWS_CLOCKWISE, &qymcad_i18n::tr("tb-revolve-hint"), tool_is_taken(&bc, 3)) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(3));
                }
                if qymcad_ui_state::icon_tool(ui, ph::PATH, &qymcad_i18n::tr("tb-sweep-hint"), tool_is_taken(&bc, 8)) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(8));
                }
                if qymcad_ui_state::icon_tool(ui, ph::STACK, &qymcad_i18n::tr("tb-loft-hint"), tool_is_taken(&bc, 9)) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(9));
                }
                // --- The 3D primitives (a command: sizes at the geometry + a preview + Enter/Esc) ---
                cat(ui, &qymcad_i18n::tr("tb-group-prim"));
                if qymcad_ui_state::icon_tool(ui, ph::CUBE_TRANSPARENT, &qymcad_i18n::tr("tb-box-hint"), bc.armed.cmd_kind() == 10) {
                    bc.ask.push(qymcad_ui_state::BarAsk::PrimCmd(10));
                }
                if qymcad_ui_state::icon_tool(ui, ph::CYLINDER, &qymcad_i18n::tr("tb-cylinder-hint"), bc.armed.cmd_kind() == 11) {
                    bc.ask.push(qymcad_ui_state::BarAsk::PrimCmd(11));
                }
                if qymcad_ui_state::icon_tool(ui, ph::SPHERE, &qymcad_i18n::tr("tb-sphere-hint"), bc.armed.cmd_kind() == 12) {
                    bc.ask.push(qymcad_ui_state::BarAsk::PrimCmd(12));
                }
                if qymcad_ui_state::icon_tool(ui, ph::TRAFFIC_CONE, &qymcad_i18n::tr("tb-cone-hint"), bc.armed.cmd_kind() == 13) {
                    bc.ask.push(qymcad_ui_state::BarAsk::PrimCmd(13));
                }
                if qymcad_ui_state::icon_tool(ui, ph::CIRCLE_NOTCH, &qymcad_i18n::tr("tb-torus-hint"), bc.armed.cmd_kind() == 14) {
                    bc.ask.push(qymcad_ui_state::BarAsk::PrimCmd(14));
                }
                if qymcad_ui_state::icon_tool(ui, ph::HEXAGON, &qymcad_i18n::tr("tb-prism-hint"), bc.armed.cmd_kind() == 15) {
                    bc.ask.push(qymcad_ui_state::BarAsk::PrimCmd(15));
                }
                // --- Operations on a body ---
                cat(ui, &qymcad_i18n::tr("tb-body"));
                if qymcad_ui_state::icon_tool(ui, ph::CIRCLE_HALF, &qymcad_i18n::tr("tb-fillet-body-hint"), bc.armed.cmd_kind() == 4) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(4));
                }
                if qymcad_ui_state::icon_tool(ui, ph::TRIANGLE, &qymcad_i18n::tr("tb-chamfer-body-hint"), bc.armed.cmd_kind() == 5) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(5));
                }
                if qymcad_ui_state::icon_tool(ui, ph::BOUNDING_BOX, &qymcad_i18n::tr("tb-shell-hint"), bc.armed.cmd_kind() == 6) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(6));
                }
                if qymcad_ui_state::icon_tool(ui, ph::SQUARE_HALF, &qymcad_i18n::tr("tb-section-hint-bar"), bc.section.pick || bc.section.plane.is_some()) {
                    bc.ask.push(qymcad_ui_state::BarAsk::ToggleSection);
                }
                if qymcad_ui_state::icon_tool(ui, ph::CIRCLE, &qymcad_i18n::tr("tb-hole-hint"), bc.armed.cmd_kind() == 7) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(7));
                }
                if qymcad_ui_state::icon_tool(ui, ph::ANGLE, &qymcad_i18n::tr("tb-draft-hint"), bc.armed.cmd_kind() == 23) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(23));
                }
                if qymcad_ui_state::icon_tool(ui, ph::ARROWS_OUT_LINE_VERTICAL, &qymcad_i18n::tr("tb-push-face-hint"), bc.armed.cmd_kind() == 25) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(25));
                }
                if qymcad_ui_state::icon_tool(ui, ph::ERASER, &qymcad_i18n::tr("tb-remove-face-hint"), bc.armed.cmd_kind() == 26) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(26));
                }
                if qymcad_ui_state::icon_tool(ui, ph::STACK_SIMPLE, &qymcad_i18n::tr("tb-thicken-hint"), bc.armed.cmd_kind() == 28) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(28));
                }
                // the bridge from the parametric side into the design layer: a face becomes a surface
                if qymcad_ui_state::icon_tool(ui, ph::COPY_SIMPLE, &qymcad_i18n::tr("tb-face-copy-hint"), bc.armed.cmd_kind() == 30) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(30));
                }
                // the far end of that bridge: a surface goes back into the body
                if qymcad_ui_state::icon_tool(ui, ph::SWAP, &qymcad_i18n::tr("tb-surface-replace-hint"), bc.armed.cmd_kind() == 31) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(31));
                }
                // the first shape the body did not have: a surface built from the edges
                if qymcad_ui_state::icon_tool(ui, ph::BANDAIDS, &qymcad_i18n::tr("tb-patch-hint"), bc.armed.cmd_kind() == 32) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(32));
                }
                // pieces of surface become one, and a shell that closes becomes a body
                if qymcad_ui_state::icon_tool(ui, ph::INTERSECT_SQUARE, &qymcad_i18n::tr("tb-stitch-hint"), bc.armed.cmd_kind() == 33) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(33));
                }
                // trim a surface with the neighbouring geometry
                if qymcad_ui_state::icon_tool(ui, ph::SCISSORS, &qymcad_i18n::tr("tb-trim-surface-hint"), bc.armed.cmd_kind() == 34) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(34));
                }
                if qymcad_ui_state::icon_tool(ui, ph::SQUARE_SPLIT_HORIZONTAL, &qymcad_i18n::tr("tb-split-body-hint"), bc.armed.cmd_kind() == 27) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(27));
                }
                if qymcad_ui_state::icon_tool(
                    ui,
                    ph::RULER,
                    &qymcad_i18n::tr("tb-measure3d-hint"),
                    bc.m3.on,
                ) {
                    bc.ask.push(qymcad_ui_state::BarAsk::ToggleMeasure3d);
                }
                if qymcad_ui_state::icon_tool(ui, ph::GRID_FOUR, &qymcad_i18n::tr("tb-split-face-hint"), bc.armed.cmd_kind() == 29) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(29));
                }
                if qymcad_ui_state::icon_tool(ui, ph::SPIRAL, &qymcad_i18n::tr("tb-thread-hint"), bc.armed.cmd_kind() == 24) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(24));
                }
                if qymcad_ui_state::icon_tool(ui, ph::INTERSECT, &qymcad_i18n::tr("tb-bool-bodies-hint"), bc.boolean.pick.is_some()) {
                    if let Some(a) = qymcad_ui_state::selected_body(&*bc.project, &*bc.sel) {
                        bc.boolean.pick = Some((a, 0));
                        *bc.status = qymcad_i18n::tr("tb-bool-pick-b");
                    } else {
                        bc.boolean.pick = None;
                        *bc.status = qymcad_i18n::tr("tb-pick-body-a-first");
                    }
                }
                // THE MIRROR AND THE BODY PATTERNS LIVE HERE, UNDER "BODY". A separate "Patterns" category
                // used to stand beside it showing the same two icons as "Body" above: those were COMPONENT
                // patterns (an assembly tool), these are BODY patterns. What that looked like was
                // duplicates - and it was worse than duplicates, because two buttons that looked alike did
                // different things. The component ones moved into the Assembly, where they belong.
                if qymcad_ui_state::icon_tool(ui, ph::FLIP_HORIZONTAL, &qymcad_i18n::tr("tb-mirror-body-hint"), bc.armed.cmd_kind() == 16) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(16));
                }
                if qymcad_ui_state::icon_tool(ui, ph::DOTS_THREE_OUTLINE, &qymcad_i18n::tr("tb-lin-array-body-hint"), bc.armed.cmd_kind() == 17) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(17));
                }
                if qymcad_ui_state::icon_tool(ui, ph::CIRCLES_THREE, &qymcad_i18n::tr("tb-circ-array-body-hint"), bc.armed.cmd_kind() == 18) {
                    bc.ask.push(qymcad_ui_state::BarAsk::FeatCmd(18));
                }
            }
            qymcad_ui_state::Workbench::Assembly => {
                let cat = |ui: &mut egui::Ui, s: &str| {
                    ui.add_sized([84.0, 12.0], egui::Label::new(egui::RichText::new(s).size(8.5).weak()));
                };
                // --- Create ---
                cat(ui, &qymcad_i18n::tr("tb-group-create"));
                create_panel_common(bc, ui); // the datums; a sketch is inert in an assembly, so it was removed
                if qymcad_ui_state::icon_tool(ui, ph::CUBE, &qymcad_i18n::tr("tb-new-part-hint"), false) {
                    let id = bc.project.add_part(qymcad_i18n::tr1("node-part-n", "n", &bc.project.components.len().to_string()));
                    bc.ask.push(qymcad_ui_state::BarAsk::EnterComponent(id));
                }
                if qymcad_ui_state::icon_tool(ui, ph::STACK, &qymcad_i18n::tr("tb-new-subassembly-hint"), false) {
                    let id = bc.project.add_assembly(qymcad_i18n::tr1("node-assembly-n", "n", &bc.project.components.len().to_string()));
                    bc.ask.push(qymcad_ui_state::BarAsk::EnterComponent(id));
                }
                if qymcad_ui_state::icon_tool(ui, ph::FILE, &qymcad_i18n::tr("tb-insert-component-hint"), false) {
                    bc.ask.push(qymcad_ui_state::BarAsk::PickStep);
                }
                // COMPONENT PATTERNS ARE AN ASSEMBLY TOOL. They used to sit in the PART workbench, where
                // there are no components, and there they duplicated the look of the body patterns.
                if qymcad_ui_state::icon_tool(ui, ph::DOTS_THREE_OUTLINE, &qymcad_i18n::tr("tb-comp-lin-array-hint"), bc.carr.mode == 1) {
                    bc.ask.push(qymcad_ui_state::BarAsk::CompArray(1));
                }
                if qymcad_ui_state::icon_tool(ui, ph::CIRCLES_THREE, &qymcad_i18n::tr("tb-comp-circ-array-hint"), bc.carr.mode == 2) {
                    bc.ask.push(qymcad_ui_state::BarAsk::CompArray(2));
                }
                if qymcad_ui_state::icon_tool(ui, ph::FLIP_HORIZONTAL, &qymcad_i18n::tr("tb-mirror-part-hint"), bc.mirror.part.is_some()) {
                    // both a PART and a SUBASSEMBLY are accepted (the whole subtree is mirrored)
                    let src = match *bc.sel {
                        qymcad_ui_state::Sel::Component(ci) => bc.project.components.get(ci).map(|c| c.id),
                        _ => qymcad_ui_state::selected_body(&*bc.project, &*bc.sel).and_then(|b| bc.project.body_owner(b)),
                    };
                    match src {
                        // THROUGH A DOOR, in the order that works: release the others, THEN take this one.
                        // Pushing the release and arming straight after put them the other way round - the
                        // release runs after the frame - and the button did nothing.
                        Some(comp) => bc.ask.push(qymcad_ui_state::BarAsk::MirrorPart(comp)),
                        None => *bc.status = qymcad_i18n::tr("tb-pick-part-first"),
                    }
                }
                // ONE DOOR, not a second copy of it. This block used to repeat what `toggle_section` does,
                // and the copy could not release the other tools from here, so it pushed the request and
                // armed the section straight after. The request runs AFTER the frame and clears exactly
                // that: switching the section ON through this button did nothing at all.
                if qymcad_ui_state::icon_tool(ui, ph::SQUARE_HALF, &qymcad_i18n::tr("tb-section-hint"), bc.section.pick || bc.section.plane.is_some()) {
                    bc.ask.push(qymcad_ui_state::BarAsk::ToggleSection);
                }
                // --- The mates: buttons per mate kind, as in the sketcher. Clicking a kind starts the
                // face pick with that kind (face A, then face B). The list and the editing of existing ones
                // live in the properties panel on the right, as the sketch dimensions do.
                cat(ui, &qymcad_i18n::tr("tb-group-joint"));
                {
                    // ONE BUTTON, with the kind chosen in the top bar. There used to be seven buttons, one
                    // per kind, while the kind was changed by a combo box in that same bar anyway: the
                    // choice was made twice and took up the whole category.
                    let tip = qymcad_i18n::tr1("jt-joint-tip", "kind", &qymcad_i18n::tr(bc.joint.new_kind.label()));
                    if qymcad_ui_state::icon_tool(ui, ph::MAGNET, &tip, bc.joint.pick_faces) {
                        bc.ask.push(qymcad_ui_state::BarAsk::JointPick);
                    }
                    // the Ground tool: a click on a part fixes or releases it.
                    if qymcad_ui_state::icon_tool(ui, ph::ANCHOR, &qymcad_i18n::tr("tb-ground-hint"), bc.joint.ground_pick) {
                        bc.ask.push(qymcad_ui_state::BarAsk::GroundPick);
                    }
                    // Group: fasten a set of parts to one another without pairwise joints.
                    if qymcad_ui_state::icon_tool(ui, ph::SELECTION_ALL, &qymcad_i18n::tr("j-group-tip"), bc.joint.group_pick.is_some()) {
                        bc.ask.push(qymcad_ui_state::BarAsk::GroupPick);
                    }
                    // Width: place a part midway between two walls.
                    if qymcad_ui_state::icon_tool(ui, ph::ARROWS_OUT_LINE_HORIZONTAL, &qymcad_i18n::tr("j-width-tip"), bc.joint.width_pick.is_some()) {
                        bc.ask.push(qymcad_ui_state::BarAsk::WidthPick);
                    }
                    // Tangent: lay a cylinder onto a plane.
                    if qymcad_ui_state::icon_tool(ui, ph::CIRCLE_HALF_TILT, &qymcad_i18n::tr("j-tangent-tip"), bc.joint.tangent_pick.is_some()) {
                        bc.ask.push(qymcad_ui_state::BarAsk::TangentPick);
                    }
                    // Relation: tie the degrees of freedom of two mates together.
                    if qymcad_ui_state::icon_tool(ui, ph::GEAR_SIX, &qymcad_i18n::tr("j-relation-tip"), bc.joint.relation_pick.is_some()) {
                        bc.ask.push(qymcad_ui_state::BarAsk::RelationPick);
                    }
                }
            }
        }
        });
    });
}

pub fn tool_options_bar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    if qymcad_ui_state::edit_si(&*bc.project, &*bc.sketch_ses).is_none() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        // the name of the active tool or operation
        let name = if bc.armed.pat_op() != 0 {
            if bc.armed.pat_op() == 1 { qymcad_i18n::tr("tool-lin-array") } else { qymcad_i18n::tr("tool-circ-array") }
        } else if bc.armed.move_op() != 0 {
            if bc.armed.move_op() == 1 { qymcad_i18n::tr("tool-move") } else { qymcad_i18n::tr("tool-copy") }
        } else if bc.armed.click_op() != 0 {
            match bc.armed.click_op() {
                1 => qymcad_i18n::tr("tool-trim"),
                2 => qymcad_i18n::tr("tool-extend"),
                3 => qymcad_i18n::tr("tool-break"),
                4 => qymcad_i18n::tr("tool-fillet"),
                5 => qymcad_i18n::tr("tool-chamfer"),
                _ => "—".to_string(),
            }
        } else if bc.armed.dim_kind() != 0 {
            qymcad_i18n::tr("tool-dim")
        } else if bc.armed.measuring() {
            qymcad_i18n::tr("tool-measure")
        } else if bc.armed.draw_kind() != 0 {
            match bc.armed.draw_kind() {
                1 => qymcad_i18n::tr("tool-line"),
                2 => qymcad_i18n::tr("tool-rect"),
                3 => qymcad_i18n::tr("tool-circle"),
                4 => qymcad_i18n::tr("tool-arc"),
                5 => qymcad_i18n::tr("tool-point"),
                6 => qymcad_i18n::tr("tool-polygon"),
                7 => qymcad_i18n::tr("tool-slot"),
                _ => "—".to_string(),
            }
        } else {
            match bc.armed.modify() {
                1 => qymcad_i18n::tr("tool-fillet"),
                2 => qymcad_i18n::tr("tool-chamfer"),
                3 => qymcad_i18n::tr("tool-offset"),
                4 => qymcad_i18n::tr("tool-mirror"),
                5 => qymcad_i18n::tr("tool-lin-array-short"),
                6 => qymcad_i18n::tr("tool-circ-array-short"),
                _ => qymcad_i18n::tr("tool-select"),
            }
        };
        ui.label(egui::RichText::new(name).strong());
        ui.separator();
        // ONLY the parameters of the active tool or operation
        if bc.armed.draw_kind() != 0 {
            ui.checkbox(&mut bc.tool.construction, qymcad_i18n::tr("opt-construction-short")).on_hover_text(qymcad_i18n::tr("opt-construction-hint"));
            if bc.armed.draw_kind() == 11 {
                ui.separator();
                ui.label(qymcad_i18n::tr("tool-text"));
                ui.add(egui::TextEdit::singleline(&mut bc.tool_prefs.text).desired_width(140.0));
                ui.label(qymcad_i18n::tr("opt-height-short"));
                bc.tool_prefs.text_h = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "text_h", bc.tool_prefs.text_h, qymcad_ui_state::NumFormat { lo: 1.0, hi: 1000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix") });
                if ui.button(qymcad_i18n::tr("opt-font")).on_hover_text(qymcad_i18n::tr("opt-pick-font")).clicked() {
                    bc.ask.push(qymcad_ui_state::BarAsk::PickFont);
                }
                ui.checkbox(&mut bc.tool_prefs.text_note, qymcad_i18n::tr("opt-note")).on_hover_text(qymcad_i18n::tr("opt-text-as-note"));
            }
            if bc.armed.draw_kind() == 6 {
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-sides"));
                bc.tool_prefs.poly_n = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "poly_n", bc.tool_prefs.poly_n as f64, qymcad_ui_state::NumFormat { lo: 3.0, hi: 64.0, integer: true, suffix: "" }) as u32;
                ui.selectable_value(&mut bc.tool_prefs.poly_mode, 0u8, qymcad_i18n::tr("opt-polygon-inscribed"));
                ui.selectable_value(&mut bc.tool_prefs.poly_mode, 1u8, qymcad_i18n::tr("opt-polygon-circumscribed"));
                ui.selectable_value(&mut bc.tool_prefs.poly_mode, 2u8, qymcad_i18n::tr("opt-by-edge"));
                if bc.tool_prefs.poly_mode == 2 {
                    ui.label(qymcad_i18n::tr("opt-edge"));
                    bc.tool_prefs.poly_edge = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "poly_edge", bc.tool_prefs.poly_edge, qymcad_ui_state::NumFormat { lo: 0.1, hi: 10000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix") });
                }
            }
            if bc.armed.draw_kind() == 2 {
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-rect-mode"));
                if ui.selectable_label(bc.tool_prefs.rect_mode == 0, qymcad_i18n::tr("opt-rect-2corners")).clicked() { bc.tool_prefs.rect_mode = 0; bc.tool.pts.clear(); }
                if ui.selectable_label(bc.tool_prefs.rect_mode == 1, qymcad_i18n::tr("opt-rect-centre")).clicked() { bc.tool_prefs.rect_mode = 1; bc.tool.pts.clear(); }
                if ui.selectable_label(bc.tool_prefs.rect_mode == 2, qymcad_i18n::tr("opt-rect-3pt")).on_hover_text(qymcad_i18n::tr("opt-rect-rotated")).clicked() { bc.tool_prefs.rect_mode = 2; bc.tool.pts.clear(); }
            }
            if bc.armed.draw_kind() == 3 {
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-circle-mode"));
                if ui.selectable_label(bc.tool_prefs.circ_mode == 0, qymcad_i18n::tr("opt-circle-centre-radius")).clicked() { bc.tool_prefs.circ_mode = 0; bc.tool.pts.clear(); bc.tool.circ_tan = None; }
                if ui.selectable_label(bc.tool_prefs.circ_mode == 1, qymcad_i18n::tr("opt-circle-2pt")).on_hover_text(qymcad_i18n::tr("opt-circle-diameter-ends")).clicked() { bc.tool_prefs.circ_mode = 1; bc.tool.pts.clear(); bc.tool.circ_tan = None; }
                if ui.selectable_label(bc.tool_prefs.circ_mode == 2, qymcad_i18n::tr("opt-tangent-m")).on_hover_text(qymcad_i18n::tr("opt-tangent-hint")).clicked() { bc.tool_prefs.circ_mode = 2; bc.tool.pts.clear(); bc.tool.circ_tan = None; }
            }
            if bc.armed.draw_kind() == 4 {
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-arc-mode"));
                if ui.selectable_label(bc.tool_prefs.arc_mode == 0, qymcad_i18n::tr("opt-arc-cse")).clicked() { bc.tool_prefs.arc_mode = 0; bc.tool.pts.clear(); }
                if ui.selectable_label(bc.tool_prefs.arc_mode == 1, qymcad_i18n::tr("opt-rect-3pt")).on_hover_text(qymcad_i18n::tr("opt-arc-3pt")).clicked() { bc.tool_prefs.arc_mode = 1; bc.tool.pts.clear(); }
                if ui.selectable_label(bc.tool_prefs.arc_mode == 2, qymcad_i18n::tr("opt-tangent")).on_hover_text(qymcad_i18n::tr("opt-arc-smooth-hint")).clicked() { bc.tool_prefs.arc_mode = 2; bc.tool.pts.clear(); }
            }
        } else if bc.armed.dim_kind() != 0 {
            ui.label(egui::RichText::new(qymcad_i18n::tr("opt-dim-hint")).weak());
        } else if bc.armed.click_op() != 0 {
            if bc.armed.click_op() == 4 || bc.armed.click_op() == 5 {
                ui.label(if bc.armed.click_op() == 4 { qymcad_i18n::tr("opt-radius") } else { qymcad_i18n::tr("opt-chamfer-size") });
                bc.tool_prefs.fillet = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "sk_fillet", bc.tool_prefs.fillet, qymcad_ui_state::NumFormat { lo: 0.01, hi: 10000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix") });
            }
            if bc.armed.click_op() == 6 {
                // WHAT IS TAKEN: one edge under the cursor, or the whole outline of the sketch's host face
                if ui.selectable_label(!bc.tool.proj_face, qymcad_i18n::tr("opt-edge")).on_hover_text(qymcad_i18n::tr("opt-project-edge-hint")).clicked() {
                    bc.tool.proj_face = false;
                }
                if ui
                    .selectable_label(bc.tool.proj_face, qymcad_i18n::tr("opt-face-outline"))
                    .on_hover_text(qymcad_i18n::tr("opt-face-outline-hint"))
                    .clicked()
                {
                    bc.tool.proj_face = true;
                }
            }
            let h = match bc.armed.click_op() {
                1 => &qymcad_i18n::tr("opt-trim-hint"),
                2 => &qymcad_i18n::tr("opt-extend-hint"),
                3 => &qymcad_i18n::tr("opt-break-hint"),
                4 => &qymcad_i18n::tr("opt-fillet-hint"),
                5 => &qymcad_i18n::tr("opt-chamfer-hint"),
                6 => &qymcad_i18n::tr("opt-project-hint"),
                _ => "",
            };
            ui.label(egui::RichText::new(h).weak());
        } else if bc.armed.pat_op() != 0 {
            // the pattern: the parameters + a hint (the preview is live, Enter applies)
            ui.label(qymcad_i18n::tr("opt-count"));
            bc.sk_pat.count = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "skpat_count", bc.sk_pat.count as f64, qymcad_ui_state::NumFormat { lo: 2.0, hi: 200.0, integer: true, suffix: "" }) as u32;
            if bc.armed.pat_op() == 1 {
                bc.sk_pat.dx = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "skpat_dx", bc.sk_pat.dx, qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: "" });
                bc.sk_pat.dy = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "skpat_dy", bc.sk_pat.dy, qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: "" });
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-rows"));
                bc.sk_pat.count2 = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "skpat_count2", bc.sk_pat.count2 as f64, qymcad_ui_state::NumFormat { lo: 1.0, hi: 200.0, integer: true, suffix: "" }) as u32;
                if bc.sk_pat.count2 > 1 {
                    bc.sk_pat.dx2 = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "skpat_dx2", bc.sk_pat.dx2, qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: "" });
                    bc.sk_pat.dy2 = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "skpat_dy2", bc.sk_pat.dy2, qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: "" });
                }
            } else {
                bc.sk_pat.angle = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "skpat_angle", bc.sk_pat.angle, qymcad_ui_state::NumFormat { lo: -360.0, hi: 360.0, integer: false, suffix: "°" });
                ui.label(egui::RichText::new(if bc.pat.center.is_some() { qymcad_i18n::tr("opt-centre-set") } else { qymcad_i18n::tr("opt-click-rotation-centre") }).weak());
            }
            ui.label(egui::RichText::new(if bc.pat.edit.is_some() { qymcad_i18n::tr("opt-enter-update") } else { qymcad_i18n::tr("opt-enter-apply") }).weak());
        } else {
            match bc.armed.modify() {
                1 | 2 => {
                    ui.label(if bc.armed.modify() == 1 { qymcad_i18n::tr("opt-radius") } else { qymcad_i18n::tr("opt-chamfer-size") });
                    bc.tool_prefs.fillet = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "sk_fillet", bc.tool_prefs.fillet, qymcad_ui_state::NumFormat { lo: 0.01, hi: 10000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix") });
                }
                3 => {
                    ui.label(qymcad_i18n::tr("opt-distance"));
                    bc.tool_prefs.offset = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "sk_offset", bc.tool_prefs.offset, qymcad_ui_state::NumFormat { lo: -10000.0, hi: 10000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix") });
                }
                5 => {
                    ui.label(qymcad_i18n::tr("opt-count"));
                    bc.sk_pat.count = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "skpat_count", bc.sk_pat.count as f64, qymcad_ui_state::NumFormat { lo: 2.0, hi: 200.0, integer: true, suffix: "" }) as u32;
                    bc.sk_pat.dx = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "skpat_dx", bc.sk_pat.dx, qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: "" });
                    bc.sk_pat.dy = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "skpat_dy", bc.sk_pat.dy, qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: "" });
                }
                6 => {
                    ui.label(qymcad_i18n::tr("opt-count"));
                    bc.sk_pat.count = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "skpat_count", bc.sk_pat.count as f64, qymcad_ui_state::NumFormat { lo: 2.0, hi: 200.0, integer: true, suffix: "" }) as u32;
                    bc.sk_pat.angle = qymcad_ui_state::num_or_expr(&mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme }, ui, "skpat_angle", bc.sk_pat.angle, qymcad_ui_state::NumFormat { lo: -360.0, hi: 360.0, integer: false, suffix: "°" });
                }
                4 => {
                    ui.label(egui::RichText::new(qymcad_i18n::tr("opt-mirror-axis-hint")).weak());
                }
                _ => {
                    ui.checkbox(&mut bc.tool.construction, qymcad_i18n::tr("opt-construction-short"));
                    ui.label(egui::RichText::new(qymcad_i18n::tr("opt-select-hint")).weak());
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(qymcad_i18n::tr1("opt-selected-n", "n", &bc.sel_sk.items.len().to_string())).weak());
        });
    });
}