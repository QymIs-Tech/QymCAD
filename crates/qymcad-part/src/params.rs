//! THE PARAMETERS OF A COMMAND: its fields made, kept in step with its options, read and stored.

use super::*;

/// THE DESCRIPTOR OF A REFERENCE, IF IT IS A MANUAL PICK.
///
/// The highlight of a selected face in a command can show one particular face. A query of the sort "every
/// wall of this feature" gives it nothing to show - so it returns `None` rather than lying with a
/// highlight.
pub(crate) fn face_desc_of(r: &qymcad_core::refs::Ref) -> Option<u32> {
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
pub fn smart_flip(cmd: qymcad_ui_state::PartTool, op: u8, plane: &qymcad_core::feature::SketchPlane) -> bool {
    cmd == qymcad_ui_state::PartTool::Extrude && op == 2 && matches!(plane, qymcad_core::feature::SketchPlane::Face(..))
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

/// THE EDGES OF A REOPENED FILLET, CHAMFER OR PATCH, picked again the way they were recorded.
///
/// A list is a list of edges and goes through [`live_picks`]. A description ("every edge of this face") holds the
/// number of a FACE: taken as an edge it was dropped as unknown - the pick came out empty, which the kernel reads as
/// every edge - or, with no edges known yet, handed on as an edge that does not exist. So the description goes back
/// into the selection, and its edges, resolved on the live body, are what is highlighted. Reported behaviour: a
/// fillet of the top face of a cylinder reopened with no edge highlighted and both rims in the preview.
///
/// The edges of a body just opened are taken from its live B-rep first, for either kind: the corners of a variable
/// radius are named by them too, and without them a reopened fillet lost the field of its corner and Enter wrote an
/// empty table back.
pub(crate) fn restore_edge_picks(pc: &mut qymcad_ui_state::PartCtx, src: Id, r: &qymcad_core::refs::Ref) {
    qymcad_ui_state::ensure_model_edges(&mut pc.rebuild(), src);
    if r.query.is_pick_list() {
        pc.gsel.described = None;
        pc.gsel.edges = live_picks(&*pc.project, src, r, false);
        return;
    }
    pc.gsel.edges = pc.project.resolve_edge_refs(src, r, "ref-what-fillet-edge").map(|v| v.into_iter().collect()).unwrap_or_default();
    pc.gsel.described = Some(r.query.clone());
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
    cmd.params.iter().filter_map(|p| p.key.strip_prefix("at").and_then(|d| d.parse::<u32>().ok()).map(|desc| (desc, p.val))).filter(|(_, v)| *v > 1e-9).collect()
}

/// The name of the operation for the undo step - the same one shown on the button.
pub fn feat_cmd_name(armed: &qymcad_ui_state::Armed, feat: qymcad_ui_state::FeatTarget) -> String {
    match armed.part_tool() {
        Some(qymcad_ui_state::PartTool::Extrude) => match feat.op {
            2 => qymcad_i18n::tr("f-cut"),
            3 => qymcad_i18n::tr("f-intersection"),
            _ => qymcad_i18n::tr("f-extrusion"),
        },
        Some(qymcad_ui_state::PartTool::Revolve) => qymcad_i18n::tr("f-revolution"),
        Some(qymcad_ui_state::PartTool::Fillet) => qymcad_i18n::tr("f-fillet"),
        Some(qymcad_ui_state::PartTool::Chamfer) => qymcad_i18n::tr("f-chamfer"),
        Some(qymcad_ui_state::PartTool::Shell) => qymcad_i18n::tr("f-shell"),
        Some(qymcad_ui_state::PartTool::Sweep) => qymcad_i18n::tr("f-sweep"),
        Some(qymcad_ui_state::PartTool::Loft) => qymcad_i18n::tr("f-loft"),
        Some(qymcad_ui_state::PartTool::Mirror) => qymcad_i18n::tr("f-mirror"),
        Some(qymcad_ui_state::PartTool::Draft) => qymcad_i18n::tr("f-draft"),
        Some(qymcad_ui_state::PartTool::PushFace) => qymcad_i18n::tr("f-push-face"),
        Some(qymcad_ui_state::PartTool::RemoveFace) => qymcad_i18n::tr("f-remove-face"),
        Some(qymcad_ui_state::PartTool::FaceCopy) => qymcad_i18n::tr("f-face-copy"),
        Some(qymcad_ui_state::PartTool::OffsetSurface) => qymcad_i18n::tr("feat-name-offset-surface"),
        Some(qymcad_ui_state::PartTool::SurfaceReplace) => qymcad_i18n::tr("f-surface-replace"),
        Some(qymcad_ui_state::PartTool::Patch) => qymcad_i18n::tr("f-patch"),
        Some(qymcad_ui_state::PartTool::Thread) => qymcad_i18n::tr("f-thread"),
        Some(qymcad_ui_state::PartTool::Recognise) => qymcad_i18n::tr("f-recognise"),
        _ => qymcad_i18n::tr("f-operation"),
    }
}

/// THE FIELDS OF A THREAD PUT ON A CYLINDER: sized from the geometry, and no longer than the cylinder - a thread of one
/// and a half diameters on a short shaft came up refused before anything was typed.
pub fn seed_thread_params(pc: &mut qymcad_ui_state::PartCtx) {
    qymcad_ui_state::set_thread_params(pc.cmd, *pc.thread);
    let t = *pc.thread;
    let Some(span) = t.src.filter(|_| !t.auger).and_then(|src| pc.project.thread_face_span(src, (t.axis.0, t.axis.1, t.radius))) else { return };
    if let Some(p) = pc.cmd.params.iter_mut().find(|p| p.key == "length").filter(|p| p.val > span + 1e-6) {
        (p.val, p.txt) = (span, qymcad_core::expr::fmt_num(span));
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
        cmd.params.push(qymcad_ui_state::CmdParam::new("th-profile-angle", "angle", 60.0, 5.0, 170.0));
        // The depth opens on the one that will be cut, not on zero: zero here means "nothing was typed" and
        // the core cuts 0.6 of the pitch, so a field at 0.00 stood over a groove 1.50 deep.
        let depth = thread_spec(cmd, thread).geometry().depth;
        cmd.params.push(qymcad_ui_state::CmdParam::new("th-depth", "depth", depth, 0.0, 1000.0));
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

/// The diameter and depth fields of the RECESS (a counterbore or a countersink) at the geometry - they
/// appear for any hole type other than the plain one.
pub fn sync_hole_params(armed: &qymcad_ui_state::Armed, cmd: &mut qymcad_ui_state::FeatCommand, hole: qymcad_ui_state::HoleCommand, project: &Project) {
    if armed.part_tool() != Some(qymcad_ui_state::PartTool::Hole) {
        return;
    }
    let want = hole.kind != 0;
    let has = cmd.params.iter().any(|p| p.key == "dia2");
    if want && !has {
        // twice the hole, as a professional CAD offers it (a hole of 5 gets a recess of 10): a fixed 12 left every hole
        // of 12 and more with a recess no wider than itself
        let recess = 2.0 * qymcad_ui_state::cmd_val(cmd, "diameter");
        let (d2, dp2) = match cmd.edit {
            Some(fid) => (cmd_param_from(project, fid, "f-recess-d", "dia2", recess, 0.1, 10000.0), cmd_param_from(project, fid, "f-recess-depth", "depth2", 4.0, 0.1, 10000.0)),
            None => (qymcad_ui_state::CmdParam::new("f-recess-d", "dia2", recess, 0.1, 10000.0), qymcad_ui_state::CmdParam::new("f-recess-depth", "depth2", 4.0, 0.1, 10000.0)),
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
    if armed.part_tool() != Some(qymcad_ui_state::PartTool::DatumPoint) {
        return;
    }
    let want_coords = datum.pt_mode == 0;
    let has_coords = cmd.params.iter().any(|p| p.key == "x");
    if want_coords && !has_coords {
        cmd.params = vec![
            qymcad_ui_state::CmdParam::new("X", "x", 0.0, -1e7, 1e7),
            qymcad_ui_state::CmdParam::new("Y", "y", 0.0, -1e7, 1e7),
            qymcad_ui_state::CmdParam::new("Z", "z", 0.0, -1e7, 1e7),
        ];
    } else if !want_coords && has_coords {
        cmd.params.clear();
    }
}

/// Synchronise the axis fields at the geometry with the mode (a click on an edge or a face against a
/// hand-typed origin and direction).
pub fn sync_datum_axis_params(armed: &qymcad_ui_state::Armed, cmd: &mut qymcad_ui_state::FeatCommand, datum: &qymcad_ui_state::DatumCommand) {
    if armed.part_tool() != Some(qymcad_ui_state::PartTool::DatumAxis) {
        return;
    }
    let want_manual = datum.axis_mode == 1;
    let has_manual = cmd.params.iter().any(|p| p.key == "ox");
    if want_manual && !has_manual {
        cmd.params = vec![
            qymcad_ui_state::CmdParam::new("O.x", "ox", 0.0, -1e7, 1e7),
            qymcad_ui_state::CmdParam::new("O.y", "oy", 0.0, -1e7, 1e7),
            qymcad_ui_state::CmdParam::new("O.z", "oz", 0.0, -1e7, 1e7),
            qymcad_ui_state::CmdParam::new("Dir.x", "dx", 0.0, -1e7, 1e7),
            qymcad_ui_state::CmdParam::new("Dir.y", "dy", 0.0, -1e7, 1e7),
            qymcad_ui_state::CmdParam::new("Dir.z", "dz", 1.0, -1e7, 1e7),
        ];
    } else if !want_manual && has_manual {
        cmd.params.clear();
    }
}

/// Synchronise the array fields at the geometry with the options of the bar: a second step for a linear
/// array with a second direction, and an angle for a circular one that is NOT a full circle (the same way
/// `sync_dir_cmd_params` handles the second side of an extrude).
pub fn sync_array_params(arr: qymcad_ui_state::ArrayParams, armed: &qymcad_ui_state::Armed, cmd: &mut qymcad_ui_state::FeatCommand, project: &Project) {
    match armed.part_tool() {
        Some(qymcad_ui_state::PartTool::LinearPattern) => {
            let counts = [
                MoreCount { wanted: arr.two, key: "count2", label: "f-copies2", now: arr.count2 },
                MoreCount { wanted: arr.two && arr.three, key: "count3", label: "f-copies3", now: arr.count3 },
            ];
            for c in counts {
                let has = cmd.params.iter().any(|p| p.key == c.key);
                if c.wanted && !has {
                    let prm = match cmd.edit {
                        Some(fid) => cmd_param_from(project, fid, c.label, c.key, f64::from(c.now.max(2)), 1.0, 512.0),
                        None => qymcad_ui_state::CmdParam::new(c.label, c.key, f64::from(c.now.max(2)), 1.0, 512.0),
                    };
                    cmd.params.push(prm.whole());
                } else if !c.wanted && has {
                    cmd.params.retain(|p| p.key != c.key);
                }
            }
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
        Some(qymcad_ui_state::PartTool::CircularPattern) => {
            let has_ang = cmd.params.iter().any(|p| p.key == "angle");
            if !has_ang {
                let prm = match cmd.edit {
                    Some(fid) => cmd_param_from(project, fid, "f-angle", "angle", 360.0, 1.0, 360.0),
                    None => qymcad_ui_state::CmdParam::new("f-angle", "angle", 360.0, 1.0, 360.0),
                };
                cmd.params.push(prm);
            }
        }
        _ => {}
    }
}

/// THE COUNT OF A SECOND OR THIRD DIRECTION OF A PATTERN: whether its field is wanted, its key and caption, and the count
/// the pattern holds now.
struct MoreCount {
    wanted: bool,
    key: &'static str,
    label: &'static str,
    now: u32,
}

/// THE COUNTS OF THE COMMAND IN HAND TAKEN FROM ITS FIELDS: the copies of a pattern, the sides of a prism - read by its
/// preview and its apply.
pub fn take_counts(armed: &qymcad_ui_state::Armed, cmd: &qymcad_ui_state::FeatCommand, counts: Counts) {
    match armed.part_tool() {
        Some(qymcad_ui_state::PartTool::Prism) => {
            if let Some(v) = count_of(cmd, "n", 3) {
                counts.prim.n = v;
            }
        }
        Some(qymcad_ui_state::PartTool::LinearPattern) | Some(qymcad_ui_state::PartTool::CircularPattern) => take_array_counts(cmd, counts.arr),
        _ => {}
    }
}

/// THE COUNTS OF A PATTERN TAKEN FROM ITS FIELDS - of bodies or of parts alike.
pub fn take_array_counts(cmd: &qymcad_ui_state::FeatCommand, arr: &mut qymcad_ui_state::ArrayParams) {
    arr.count = count_of(cmd, "count", 1).unwrap_or(arr.count);
    arr.count2 = count_of(cmd, "count2", 1).unwrap_or(arr.count2);
    arr.count3 = count_of(cmd, "count3", 1).unwrap_or(arr.count3);
}

/// The whole number field `key` of a command holds, `at_least` at the least; none where the command has no such field.
fn count_of(cmd: &qymcad_ui_state::FeatCommand, key: &str, at_least: u32) -> Option<u32> {
    cmd.params.iter().find(|p| p.key == key).map(|p| (p.val.round().max(f64::from(at_least))) as u32)
}

/// WHERE THE COUNTS OF A COMMAND ARE KEPT: the pattern's and the prism's own.
pub struct Counts<'a> {
    pub arr: &'a mut qymcad_ui_state::ArrayParams,
    pub prim: &'a mut qymcad_ui_state::PrimParams,
}

/// Store the dimensions of a command on feature `body` through the one door of a feature's number
/// (`Project::write_feat_formula`): an expression stays parametric, a plain number removes the expression (this matters
/// while editing, when a formula gets replaced by a number). The first refusal stops the store and is handed back.
pub fn store_cmd_exprs(cmd: &qymcad_ui_state::FeatCommand, project: &mut Project, body: Id) -> Result<(), qymcad_core::errors::ExprError> {
    for p in &cmd.params {
        project.write_feat_formula(&qymcad_core::model::FeatNumber { node: body, key: &p.key }, &p.txt, qymcad_core::model::PlainNumber::ClearsFormula)?;
    }
    Ok(())
}

/// THE TUBE OF A TORUS STAYS THINNER THAN ITS RING: the tube field's upper limit follows the ring field, so a tube as
/// thick as the ring or thicker is refused beside the field, in words, before Enter. Reported behaviour: a ring of 12
/// took a tube of 120 and stood green as a body passing through itself.
pub fn sync_torus_params(armed: &qymcad_ui_state::Armed, cmd: &mut qymcad_ui_state::FeatCommand, project: &Project) {
    if armed.part_tool() != Some(qymcad_ui_state::PartTool::Torus) {
        return;
    }
    let vars = project.param_map();
    let Some(ring) = cmd.params.iter().find(|p| p.key == "major").and_then(|p| qymcad_ui_state::field_value(p, &vars).ok()) else { return };
    if let Some(tube) = cmd.params.iter_mut().find(|p| p.key == "minor") {
        tube.hi = (ring - 1e-3).max(tube.lo);
    }
}

/// Synchronise the DIRECTION expression fields at the geometry: the second side (`down`) appears as a
/// field in the popup at the geometry ONLY in the two-sided mode (`qymcad_ui_state::ExtentMode::TwoSided`) of an extrude
/// or a cut, and is removed otherwise. The value is mirrored into `feat_down`, which the preview, the
/// apply and the update all read. When a feature is reopened the seed takes the stored expression
/// (`cmd_param_from`). The height is already a field - so every distance of a command is edited at the
/// geometry rather than by drag values in the top bar.
pub fn sync_dir_cmd_params(armed: &qymcad_ui_state::Armed, cmd: &mut qymcad_ui_state::FeatCommand, project: &Project) {
    if armed.part_tool() != Some(qymcad_ui_state::PartTool::Extrude) {
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
    cmd.params.iter().all(|p| qymcad_ui_state::field_value(p, &vars).is_ok() && field_refused_by_document(p, project, cmd.edit).is_none())
}

/// WHAT THE DOCUMENT WOULD REFUSE OF A FIELD OF A COMMAND OPENED ON NODE `edit`, in words - the question of the door the
/// field is written through (`Project::check_feat_text`): a formula that comes back to the name given to this very
/// number, or reads a name with no value. Taken, `depth*2` written for the height named `depth` left `depth` with no
/// value and the extrusion with no height. A new command's numbers have no name yet, and what `field_value` refuses is
/// said by it first.
pub fn field_refused_by_document(p: &qymcad_ui_state::CmdParam, project: &Project, edit: Option<Id>) -> Option<String> {
    let at = qymcad_core::model::FeatNumber { node: edit?, key: &p.key };
    project.check_feat_text(&at, &p.txt).err().map(|e| qymcad_i18n::error_words::expr_error_text(&e))
}

/// The "what to pick" hint for the active command, while nothing has been picked yet.
/// WHAT THE COMMAND HAS TAKEN, named in its bar: the edges or the faces picked, the plane a split is made along. A pick
/// counted only in the status line leaves the bar the same before the click and after it.
pub(crate) fn taken_line(pc: &qymcad_ui_state::PartCtx) -> Option<String> {
    if !pc.gsel.edges.is_empty() {
        Some(qymcad_i18n::tr1("cmd-taken-edges", "n", &pc.gsel.edges.len().to_string()))
    } else if !pc.gsel.faces.is_empty() {
        Some(qymcad_i18n::tr1("cmd-taken-faces", "n", &pc.gsel.faces.len().to_string()))
    } else if pc.armed.part_tool().is_some_and(qymcad_ui_state::PartTool::splits) && pc.split.plane.is_some() {
        Some(qymcad_i18n::tr("cmd-taken-plane"))
    } else if pc.armed.part_tool() == Some(qymcad_ui_state::PartTool::Stitch) && !pc.stitch_parts.is_empty() {
        Some(qymcad_i18n::tr1("cmd-taken-sheets", "n", &pc.stitch_parts.len().to_string()))
    } else if let (true, qymcad_ui_state::Sel::Mesh(mi)) = (pc.armed.part_tool().is_some_and(qymcad_ui_state::PartTool::is_a_pattern), *pc.sel) {
        let name = pc.project.bodies.get(mi).map(|b| qymcad_i18n::name(&b.name)).unwrap_or_default();
        Some(qymcad_i18n::tr1("cmd-taken-body", "name", &name))
    } else {
        None
    }
}

pub fn cmd_hint(armed: &qymcad_ui_state::Armed, gsel: &qymcad_ui_state::GeomSelection, trim: &qymcad_ui_state::TrimTool) -> String {
    match armed.part_tool() {
        Some(qymcad_ui_state::PartTool::Extrude) | Some(qymcad_ui_state::PartTool::Revolve) => qymcad_i18n::tr("hint-closed-contour"),
        Some(qymcad_ui_state::PartTool::Sweep) => qymcad_i18n::tr("hint-path-sketch"),
        Some(qymcad_ui_state::PartTool::Loft) => qymcad_i18n::tr("hint-loft-sections"),
        Some(qymcad_ui_state::PartTool::Fillet) | Some(qymcad_ui_state::PartTool::Chamfer) => qymcad_i18n::tr("hint-body-edges"),
        Some(qymcad_ui_state::PartTool::Shell) => qymcad_i18n::tr("hint-faces-thickness"),
        Some(qymcad_ui_state::PartTool::Hole) => qymcad_i18n::tr("hint-body-face"),
        Some(qymcad_ui_state::PartTool::Thread) => qymcad_i18n::tr("hint-cyl-or-hole"),
        Some(qymcad_ui_state::PartTool::Draft) => qymcad_i18n::tr("hint-draft-faces"),
        Some(qymcad_ui_state::PartTool::PushFace) => qymcad_i18n::tr("hint-flat-face-offset"),
        Some(qymcad_ui_state::PartTool::FaceCopy) => qymcad_i18n::tr("hint-face-copy"),
        Some(qymcad_ui_state::PartTool::OffsetSurface) => qymcad_i18n::tr("msg-offset-surface"),
        Some(qymcad_ui_state::PartTool::Patch) => qymcad_i18n::tr("hint-patch"),
        Some(qymcad_ui_state::PartTool::Stitch) => qymcad_i18n::tr("hint-stitch"),
        Some(qymcad_ui_state::PartTool::Recognise) => qymcad_i18n::tr("hint-recognise"),
        Some(qymcad_ui_state::PartTool::Trim) => qymcad_i18n::tr(if trim.keep.is_none() { "hint-trim" } else { "hint-trim-tool" }),
        Some(qymcad_ui_state::PartTool::SurfaceReplace) => qymcad_i18n::tr(if gsel.faces.is_empty() { "hint-surface-replace" } else { "hint-surface-replace-pick" }),
        Some(qymcad_ui_state::PartTool::SplitBody) => qymcad_i18n::tr("hint-cut-plane"),
        Some(qymcad_ui_state::PartTool::SplitFace) => qymcad_i18n::tr("hint-split-face-plane"),
        Some(qymcad_ui_state::PartTool::Thicken) => qymcad_i18n::tr("hint-face-thickness"),
        Some(qymcad_ui_state::PartTool::RemoveFace) => qymcad_i18n::tr("hint-feature-faces"),
        Some(qymcad_ui_state::PartTool::Mirror) => qymcad_i18n::tr("hint-mirror-plane"),
        Some(qymcad_ui_state::PartTool::LinearPattern) => qymcad_i18n::tr("hint-count-dir-step"),
        Some(qymcad_ui_state::PartTool::CircularPattern) => qymcad_i18n::tr("hint-count-axis-angle"),
        Some(qymcad_ui_state::PartTool::DatumPlane) => qymcad_i18n::tr("hint-plane-offset"),
        Some(qymcad_ui_state::PartTool::DatumPoint) => qymcad_i18n::tr("cmd-pick-vertex"),
        Some(qymcad_ui_state::PartTool::DatumAxis) => qymcad_i18n::tr("hint-edge-or-manual"),
        _ => String::new(),
    }
}

/// THE FIELDS OF A NEW DATUM PLANE: its offset, its tilts about its X and Y, its turn about its normal - each one also
/// dragged by its arrow or its ring (`qymcad_ui_state::handles`).
pub fn datum_plane_fields() -> Vec<qymcad_ui_state::CmdParam> {
    vec![
        qymcad_ui_state::CmdParam::new("f-offset", "dist", 10.0, -100000.0, 100000.0),
        qymcad_ui_state::CmdParam::new("f-tilt-x", "tilt_x", 0.0, -360.0, 360.0),
        qymcad_ui_state::CmdParam::new("f-tilt-y", "tilt_y", 0.0, -360.0, 360.0),
        qymcad_ui_state::CmdParam::new("f-turn", "turn", 0.0, -360.0, 360.0),
    ]
}

/// The fields of datum plane `fid` reopened: as it stands, each with its expression.
pub(crate) fn datum_plane_fields_of(project: &Project, fid: Id, pl: &qymcad_core::model::WorkPlane, dist: f64) -> Vec<qymcad_ui_state::CmdParam> {
    vec![
        cmd_param_from(project, fid, "f-offset", "dist", dist, -100000.0, 100000.0),
        cmd_param_from(project, fid, "f-tilt-x", "tilt_x", pl.tilt.about_x, -360.0, 360.0),
        cmd_param_from(project, fid, "f-tilt-y", "tilt_y", pl.tilt.about_y, -360.0, 360.0),
        cmd_param_from(project, fid, "f-turn", "turn", pl.rot_deg, -360.0, 360.0),
    ]
}

/// The tilts and the turn of the command written into datum plane `id`.
pub(crate) fn turn_datum_plane(cmd: &qymcad_ui_state::FeatCommand, project: &mut Project, id: Id) {
    if let Some(pl) = project.planes.iter_mut().find(|p| p.id == id) {
        pl.tilt = qymcad_core::feature::PlaneTilt { about_x: qymcad_ui_state::cmd_val(cmd, "tilt_x"), about_y: qymcad_ui_state::cmd_val(cmd, "tilt_y") };
        pl.rot_deg = qymcad_ui_state::cmd_val(cmd, "turn");
    }
}

/// Whether the command is ready to apply (the required selection exists) - for highlighting the button
/// and the field.
pub fn cmd_ready(pc: &mut qymcad_ui_state::PartCtx) -> bool {
    // A FILLET AND A CHAMFER ROUND THE EDGES PICKED, an edited one too: one whose edges were all lost opens with none, and
    // to the kernel an empty list is every edge - its preview lit the whole part green, and Enter would have rounded it
    if pc.cmd.edit.is_some() && pc.armed.part_tool().is_some_and(qymcad_ui_state::PartTool::blends_edges) {
        return !pc.gsel.edges.is_empty();
    }
    if pc.cmd.edit.is_some() {
        return true; // editing an existing feature - the input is already valid
    }
    match pc.armed.part_tool() {
        Some(qymcad_ui_state::PartTool::Extrude) | Some(qymcad_ui_state::PartTool::Revolve) => {
            !pc.gsel.profiles.is_empty() || pc.cmd.sketch.map(|si| qymcad_ui_state::sketch_closed_contours(pc.project, si).len() == 1).unwrap_or(false)
        }
        // sweep: a profile and a path - a sketch, or edges of a body
        Some(qymcad_ui_state::PartTool::Sweep) => pc.sweep.prof_sid != 0 && (pc.sweep.path_sid != 0 || !pc.gsel.edges.is_empty()),
        Some(qymcad_ui_state::PartTool::Loft) => pc.loft.sids.len() >= 2, // loft: at least two sections
        // a fillet and a chamfer round the edges PICKED: Enter with none picked used to round every edge of the body
        // without a word
        Some(qymcad_ui_state::PartTool::Fillet) | Some(qymcad_ui_state::PartTool::Chamfer) => !pc.gsel.edges.is_empty(),
        Some(qymcad_ui_state::PartTool::Shell) => !pc.gsel.faces.is_empty(), // shell: a multiple selection of faces
        Some(qymcad_ui_state::PartTool::Draft) => !pc.gsel.faces.is_empty() && pc.draft.neutral != 0, // draft: faces plus the neutral face
        Some(qymcad_ui_state::PartTool::PushFace)
        | Some(qymcad_ui_state::PartTool::RemoveFace)
        | Some(qymcad_ui_state::PartTool::Thicken)
        | Some(qymcad_ui_state::PartTool::FaceCopy)
        | Some(qymcad_ui_state::PartTool::OffsetSurface) => !pc.gsel.faces.is_empty(), // push, remove, thicken, copy or offset a face
        // surface replace: BOTH the faces of the base AND the surface are needed - a node without either
        // makes no sense
        Some(qymcad_ui_state::PartTool::SurfaceReplace) => !pc.gsel.faces.is_empty() && pc.repl_surface.is_some(),
        Some(qymcad_ui_state::PartTool::Patch) => pc.edges.body.is_some_and(|b| patch_boundary(&*pc.project, b, &*pc.gsel)), // patch: a boundary of edges
        Some(qymcad_ui_state::PartTool::Stitch) => pc.stitch_parts.len() >= 2,                                               // stitch: at least two sheets
        Some(qymcad_ui_state::PartTool::Recognise) => pc.recognise.src.is_some(),                                            // recognise: one mesh piece is picked
        Some(qymcad_ui_state::PartTool::Trim) => pc.trim.keep.is_some() && pc.trim.tool.is_some(),                           // trim: what is cut and what cuts it
        Some(qymcad_ui_state::PartTool::Thread) => pc.thread.edge != 0,                                                      // thread: the rim of a cylinder or a hole is picked

        // hole: "by face" needs a face picked; "by sketch" needs a body plus a sketch with isolated points
        Some(qymcad_ui_state::PartTool::Hole) if pc.hole.mode == 1 => {
            qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_some()
                && pc.hole.sketch.map(|sid| !pc.project.sketch_isolated_points(sid).is_empty()).unwrap_or(false)
        }
        Some(qymcad_ui_state::PartTool::Hole) => matches!(pc.sel, qymcad_ui_state::Sel::Face(..)),
        Some(k) if k.is_a_primitive() => true,                                // primitives: the sizes are in fields, no selection is needed
        Some(qymcad_ui_state::PartTool::Mirror) => pc.mirror.plane.is_some(), // mirror: a plane is picked by click
        Some(qymcad_ui_state::PartTool::SplitBody) | Some(qymcad_ui_state::PartTool::SplitFace) => pc.split.plane.is_some(), // splitting a body or dividing faces: a plane is picked
        // a pattern repeats the body PICKED: Enter with nothing picked used to repeat the part's body without a click
        Some(qymcad_ui_state::PartTool::LinearPattern) | Some(qymcad_ui_state::PartTool::CircularPattern) => {
            matches!(pc.sel, qymcad_ui_state::Sel::Face(..)) || qymcad_ui_state::selected_body(pc.project, pc.sel).is_some()
        }
        Some(qymcad_ui_state::PartTool::DatumPlane) => pc.datum.plane_pick.is_some(), // datum plane: a reference is picked by click
        Some(qymcad_ui_state::PartTool::DatumPoint) => pc.datum.pt_mode == 0 || pc.datum.pt_vert.is_some(), // coordinates in fields, or a vertex picked
        Some(qymcad_ui_state::PartTool::DatumAxis) => pc.datum.axis_mode == 1 || (pc.datum.axis_mode == 2 && pc.datum.axis_pts.len() == 2) || pc.datum.axis_ref.is_some(),
        _ => false,
    }
}
