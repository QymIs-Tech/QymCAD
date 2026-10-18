//! THE STEPS OF THE TOOLS of a part: options of their bars, their picks, their ends.

use crate::Notice;
use super::*;

/// Restore the view to the state it was left in. With nothing to restore (the sub-mode was entered
/// without `borrow_view`), the old behaviour applies: back to 3D, but WITHOUT refitting.
pub fn return_view(pc: &mut PartCtx) {
    match pc.view_restore.take() {
        Some((m, cam, view)) => {
            *pc.mode_3d = m;
            *pc.cam = cam;
            *pc.view = view;
        }
        None => *pc.mode_3d = true,
    }
}

/// A 3x4 row-major transform that is approximately the identity (no rotation and no translation).
pub fn is_identity12(m: &[f64; 12]) -> bool {
    const ID: [f64; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    m.iter().zip(ID.iter()).all(|(a, b)| (a - b).abs() < 1e-9)
}

/// A large icon button for the left tool bar. Returns whether it was clicked.
/// The icon button of the workbench's left panel. A FIXED size (`add_sized`, not `min_size`): wide
/// glyphs (the move and expand arrows) must not inflate the button and break the grid, or the row
/// stops holding exactly two columns. The size is THE SAME as `sym_button`'s (40x34) — both grids mix
/// inside one `horizontal_wrapped`, and a mismatch of widths (34 against 38) broke the wrap onto two
/// columns. The usable width of the tools panel is 108 - 16 (padding) - 6 (the floating scrollbar) =
/// 86 px: two columns (40+3+40=83) fit with room to spare and hold at exactly two.
/// ENTER WITH NOTHING YET TO APPLY is refused in words: the tool's own ask repeated after "not yet". A key that did
/// nothing and said nothing left a person pressing it again.
pub fn enter_not_ready(ctx: &egui::Context, status: &mut Notice, ask: &str) {
    if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        *status = Notice::refusal(qymcad_i18n::tr1("cmd-not-ready", "ask", ask));
    }
}

/// WHAT A TOOL BUTTON ASKS FOR: its tool when the hand is free of it, the hand put down when that tool is already in
/// it - a sketch tool back to the arrow, a command cancelled - as the button of every tool of a professional CAD does.
pub fn take_or_drop(held: bool, ask: BarAsk) -> BarAsk {
    match (held, ask) {
        (false, ask) => ask,
        (true, BarAsk::SketchTool(_)) => BarAsk::SketchSelectMode,
        (true, _) => BarAsk::CancelAllTools(Then::Nothing),
    }
}

pub fn icon_tool(ui: &mut egui::Ui, icon: IconId, tip: &str, active: bool) -> bool {
    let img = icon_image(ui, icon, 22.0);
    let btn = egui::Button::image(img).selected(active);

    ui.add_sized(egui::vec2(40.0, 34.0), btn).on_hover_text(tip).clicked()
}

/// The direction of an offset vector, as (dir 0/1/2, a signed step) — for reopening a linear pattern.
pub fn arr_dir_of(dx: f64, dy: f64, dz: f64) -> (u8, f64) {
    let (ax, ay, az) = (dx.abs(), dy.abs(), dz.abs());
    if az >= ax && az >= ay {
        (2, dz)
    } else if ay >= ax {
        (1, dy)
    } else {
        (0, dx)
    }
}

/// THE SIZE OF A SKETCH CHAMFER as the tool holds it: the mode, the first leg (`fillet`) and the second value.
pub fn chamfer_legs(prefs: &SketchToolPrefs) -> qymcad_core::model::ChamferLegs {
    qymcad_core::model::ChamferLegs { mode: prefs.chamfer_mode, first: prefs.fillet, second: prefs.chamfer_second }
}

/// THE CHAMFER'S MODE CHOSEN ON THE BAR, with the second value set to what the new mode means: the angle 45 deg, the
/// second leg the first one - a number in degrees is no leg, nor a leg an angle.
pub fn set_chamfer_mode(prefs: &mut SketchToolPrefs, mode: qymcad_core::feature::ChamferMode) {
    if prefs.chamfer_mode == mode {
        return;
    }
    prefs.chamfer_mode = mode;
    prefs.chamfer_second = match mode {
        qymcad_core::feature::ChamferMode::DistAngle => 45.0,
        _ => prefs.fillet,
    };
}

/// The caption of a fillet's field, and the word of its mode on the bar: the radius, the chord or the arc length.
pub fn fillet_label(by: qymcad_core::model::FilletBy) -> &'static str {
    use qymcad_core::model::FilletBy;
    match by {
        FilletBy::Radius => "opt-radius",
        FilletBy::Chord => "opt-fillet-chord",
        FilletBy::ArcLength => "opt-fillet-arc-length",
    }
}

/// THE MOST A CORNER TAKES, in the way its size is given: the cut or the leg of a chamfer, or the radius, the chord or
/// the arc length of a fillet - the largest radius worked into the chord or the arc of the same turn. `None` where the
/// corner is not one of two lines.
pub fn corner_limit_in(project: &Project, si: usize, pid: Id, tool: qymcad_core::model::CornerTool, by: qymcad_core::model::FilletBy) -> Option<f64> {
    let l = project.corner_limit(si, pid, tool)?;
    if tool != qymcad_core::model::CornerTool::Fillet || by == qymcad_core::model::FilletBy::Radius {
        return Some(l);
    }
    project.corner_sweep(si, pid).map(|sweep| qymcad_core::model::FilletSize::of_radius(by, l, sweep))
}

/// THE CORNER TOOL IN HAND, with the way the bar reads a chamfer's first value.
pub fn corner_tool(chamfer: bool, prefs: &SketchToolPrefs) -> qymcad_core::model::CornerTool {
    if chamfer {
        qymcad_core::model::CornerTool::Chamfer(prefs.chamfer_mode)
    } else {
        qymcad_core::model::CornerTool::Fillet
    }
}

/// The size of the fillet the bar holds: the way it is given and the value in that way.
pub fn fillet_size(prefs: &SketchToolPrefs) -> qymcad_core::model::FilletSize {
    qymcad_core::model::FilletSize { by: prefs.fillet_by, value: prefs.fillet }
}

/// The label of a chamfer's first field: the size of an equal chamfer, the first of two legs, or the length along the
/// line the angle is measured from.
pub fn chamfer_d1_label(mode: qymcad_core::feature::ChamferMode) -> &'static str {
    use qymcad_core::feature::ChamferMode;
    match mode {
        ChamferMode::Symmetric => "opt-chamfer-size",
        ChamferMode::TwoDist => "cmd-leg1",
        ChamferMode::DistAngle => "cmd-chamfer-length",
    }
}

/// The label of a chamfer's second field: its meaning depends on the mode — the second leg, or the angle.
pub fn chamfer_d2_label(mode: qymcad_core::feature::ChamferMode) -> &'static str {
    use qymcad_core::feature::ChamferMode;
    match mode {
        ChamferMode::DistAngle => "cmd-angle-deg",
        _ => "cmd-leg2",
    }
}

/// The switch index for a standard (the reverse mapping, used while editing a feature).
pub fn thread_standard_idx(s: qymcad_core::thread::ThreadStandard) -> u8 {
    use qymcad_core::thread::ThreadStandard as S;
    match s {
        S::TrapezoidalTr => 1,
        S::Acme => 2,
        S::RoundRd => 3,
        S::Buttress => 4,
        S::Custom => 5,
        S::MetricIso => 0,
    }
}

/// The plane a mirror or a split has picked, as the node stores it: (`plane` a world plane 0/1/2, `datum` Id, `face`).
/// A world plane gives (0/1/2, 0, None), a datum (0, id, None), a face (0, 0, Some(body, key)) - the node reads its
/// plane off the face itself, one node of the timeline, where a face used to make a datum plane of its own beside it.
/// The same one is used both when creating and when editing.
pub fn resolve_mirror_plane(sp: qymcad_core::feature::SketchPlane) -> (u8, Id, Option<(Id, qymcad_core::feature::FaceKey)>) {
    use qymcad_core::feature::{BasePlane, SketchPlane};
    match sp {
        SketchPlane::World(BasePlane::XY) => (0, 0, None),
        SketchPlane::World(BasePlane::XZ) => (1, 0, None),
        SketchPlane::World(BasePlane::YZ) => (2, 0, None),
        SketchPlane::Datum(id) => (0, id, None),
        SketchPlane::Face(body, key) => (0, 0, Some((body, key))),
    }
}

/// The plane a stored mirror or split stands on, back as the pick of its command - for reopening it.
pub fn op_plane_pick(plane: u8, datum: Id, face: Option<(Id, qymcad_core::feature::FaceKey)>) -> qymcad_core::feature::SketchPlane {
    use qymcad_core::feature::{BasePlane, SketchPlane};
    match (face, datum) {
        (Some((body, key)), _) => SketchPlane::Face(body, key),
        (None, d) if d != 0 => SketchPlane::Datum(d),
        _ => SketchPlane::World(match plane {
            1 => BasePlane::XZ,
            2 => BasePlane::YZ,
            _ => BasePlane::XY,
        }),
    }
}

/// Store the pattern step's expression on the ACTIVE component of the vector (regeneration follows
/// that one) and clear the others, through the one door of a feature's number (`Project::write_feat_formula`): a plain
/// number clears the expression, and then the feature's stored number applies. A refusal is handed back.
pub fn store_arr_component(project: &mut qymcad_core::model::Project, body: Id, keys: [&str; 3], dir: u8, txt: String) -> Result<(), qymcad_core::errors::ExprError> {
    for (i, k) in keys.iter().enumerate() {
        let at = qymcad_core::model::FeatNumber { node: body, key: k };
        if i as u8 == dir {
            project.write_feat_formula(&at, &txt, qymcad_core::model::PlainNumber::ClearsFormula)?;
        } else {
            project.clear_feat_formula(&at);
        }
    }
    Ok(())
}

/// The autosave path sits next to the document (`name.autosave.qcad`); an unnamed document goes to temp.
///
/// FROM THE STEM, NOT FROM THE WHOLE NAME. `format!("{path}.autosave.qcad")` wrote the extension twice -
/// `Filter-v2.qcad.autosave.qcad` - and a second autosave over the first gave
/// `Filter-v2.qcad.autosave.qcad.bak`. A name nobody can read is a name nobody checks.
pub fn autosave_path(project_path: &Option<String>) -> String {
    match &project_path {
        Some(p) => {
            let base = std::path::Path::new(&p);
            base.with_extension("").to_string_lossy().into_owned() + ".autosave.qcad"
        }
        None => std::env::temp_dir().join("qymcad-unsaved.autosave.qcad").to_string_lossy().into_owned(),
    }
}

/// The accumulated transform of a component's gizmo (relative to the START transform): a shift along an
/// axis, or a rotation about a ring through the fixed origin; `snap` rounds to the step (the grid or
/// the angle). Unified with the body gizmo.
pub fn comp_giz_accum(comp_giz: &CompGizmo, set: &Settings, snap: bool) -> Option<[f64; 12]> {
    let (_, start, origin, amt) = comp_giz.drag?;
    if let Some(ax) = comp_giz.axis {
        let step = set.snap.grid.max(0.01);
        let dmm = if snap { (amt / step).round() * step } else { amt };
        let mut t = qymcad_core::feature::PLACE_IDENTITY;
        t[[3, 7, 11][ax as usize]] = dmm;
        Some(compose12(&t, &start))
    } else if let Some(ax) = comp_giz.ring {
        let step = set.snap.rot_deg.max(0.1);
        let deg = if snap { (amt / step).round() * step } else { amt };
        Some(compose12(&rot_about_point(ax, deg, origin), &start))
    } else {
        None
    }
}

/// The screen anchor of the active command (where to stick the input field): for extrude and revolve
/// it is at the tip of the arrow; for a fillet or a chamfer at the first selected edge; for a shell
/// or a hole at the centre of the face.
/// The screen anchor of a command popup TO THE SIDE of body `b`: the top-right corner of its
/// projected bounding box, plus a margin. The popup used to be anchored at the CENTRE of the box, so
/// it covered the part and what was needed could not be picked behind it. Now it grows to the right
/// of the body rather than on top of it.
pub fn body_side_anchor(dc: &DrawCtx, b: Id, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) -> Option<Pos2> {
    let mi = dc.project.mesh_index(b)?;
    let bb = dc.project.bodies[mi].mesh.bounds()?;
    // where the body is DRAWN: through its placement in the context (an imported part stands where it was put)
    let wt = dc.project.body_display_transform(b, current_ctx_id(dc.active_path, dc.project));
    let mut body = Rect::NOTHING;
    for &x in &[bb.min.x, bb.max.x] {
        for &y in &[bb.min.y, bb.max.y] {
            for &z in &[bb.min.z, bb.max.z] {
                body.extend_with(Screen { cam: dc.cam, set: dc.set, rect, basis }.at(qymcad_core::feature::apply12(&wt, [x, y, z])).0);
            }
        }
    }
    if !body.is_positive() {
        return None;
    }
    // BESIDE THE BODY, WHERE THE POPUP FITS CLEAR OF IT: right, left, above, below - the first place where a popup of
    // about 240 x 140 (a caption, a field or four and a button) stands wholly inside the view and off the body. Right
    // of the body used to be the only place, pulled back over the body when it filled the view: the popup covered the
    // face the next hole had to be clicked on. Where no place is clear, right of it as before.
    const W: f32 = 240.0;
    const H: f32 = 140.0;
    let view = rect.shrink(4.0);
    let places = [
        Pos2::new(body.max.x + 14.0, body.min.y.max(view.min.y + 32.0)),
        Pos2::new(body.min.x - 14.0 - W, body.min.y.max(view.min.y + 32.0)),
        Pos2::new(body.min.x, body.min.y - 14.0 - H),
        Pos2::new(body.min.x, body.max.y + 14.0),
    ];
    let clear = |p: &Pos2| {
        let r = Rect::from_min_size(*p + egui::vec2(10.0, -10.0), egui::vec2(W, H));
        view.contains_rect(r) && !r.intersects(body)
    };
    Some(places.into_iter().find(clear).unwrap_or(places[0]))
}

/// Restore a thread operation's axis and radius from a circular edge while the feature is being EDITED
/// (a double click). The axis is oriented into the body, because the kernel gives a rim an arbitrary normal.
pub fn restore_thread_axis(project: &mut Project, thread: &mut ThreadParams, src: Id, edge: u32) {
    if let Some((c, ax0, r)) = project.regen_edges.get(&src).and_then(|es| es.iter().find(|e| e.id == edge && e.is_circular())).map(|e| (e.center, e.axis, e.radius)) {
        let mut ax = ax0;
        if let Some(bb) = project.mesh_index(src).and_then(|mi| project.bodies[mi].mesh.bounds()) {
            let cc = [(bb.min.x + bb.max.x) * 0.5, (bb.min.y + bb.max.y) * 0.5, (bb.min.z + bb.max.z) * 0.5];
            let tob = [cc[0] - c[0], cc[1] - c[1], cc[2] - c[2]];
            if tob[0] * ax[0] + tob[1] * ax[1] + tob[2] * ax[2] < 0.0 {
                ax = [-ax[0], -ax[1], -ax[2]];
            }
        }
        thread.axis = (c, ax);
        thread.radius = r;
    }
}

pub fn delete_contour(project: &mut Project, regen: &mut Rebuilding, sel: &mut Sel, view: &mut View2d, i: usize) {
    project.remove_contour(i);
    *sel = Sel::None;
    invalidate(regen);
    view.initialized = false;
}

/// Cancel the active Part command (Esc) — a clean reset.
/// CLEARING A FEATURE COMMAND'S PICKS — the single place where every subsystem's picks go out.
///
/// A 3D feature command accumulates references in its own subsystem: a sweep's profile and path, a
/// loft's sections, a draft's neutral face, a mirror's plane, a pattern's axis, a datum's references.
/// This reset used to be written out separately in the apply and in the cancel, while the command's
/// start did not reset at all — so going Sweep, then Loft, then Sweep again without cancelling showed
/// the picks from the previous round. Now the start, the apply and the cancel all call ONE reset: a
/// command always begins clean.
pub fn clear_feat_picks(p: FeatPicks) {
    p.thread.src = None;
    p.thread.edge = 0;
    p.mirror.plane = None;
    p.arr.axis_pick = false;
    p.datum.plane_pick = None;
    p.datum.axis_ref = None;
    p.datum.axis_hit = None;
    p.datum.axis_pts.clear();
    p.datum.pt_vert = None;
    p.sweep.prof_sid = 0;
    p.sweep.path_sid = 0;
    p.sweep.pick_path = false;
    p.sweep.prof_cid = 0;
    p.sweep.path_cid = 0;
    p.loft.sids.clear();
    p.loft.cids.clear();
    p.loft.pick = false;
    p.loft.pick_last = None;
    p.loft.ruled = false;
    p.picking.clear();
    p.draft.neutral = 0;
    p.draft.pick_neutral = false;
    p.draft.flip = false;
    p.stitch_parts.clear(); // the sheets picked for stitching are a pick like everything above
    p.recognise.src = None;
    p.trim.keep = None;
    p.trim.tool = None;
}

/// Borrow the view for a flat sub-mode: remember the state it was left in. A repeat call does not
/// overwrite it — the first entry holds the state to come back to.
pub fn borrow_view(cam: Cam3, mode_3d: bool, view: View2d, view_restore: &mut Option<(bool, Cam3, View2d)>) {
    if view_restore.is_none() {
        *view_restore = Some((mode_3d, cam, view));
    }
}

/// THE DOCUMENT CHANGED WITH AUTHORITY, but no undo step is created.
///
/// A third kind of change to the document: neither an edit made by hand nor a rebuild, but
/// NAVIGATION — entering a component changes `active_component`, and that is stored in the file.
/// Nobody expects to undo a move like that ("Undo: entering a subassembly" is nonsense), yet it must
/// not be declared an edit outside a boundary either: that is exactly why the application panicked on
/// entering a subassembly. The method exists so that such places are VISIBLE and named, rather than
/// hidden behind a silent update of the key.
pub fn doc_touched_without_undo(edits: &mut Edits, project: &Project) {
    edits.committed_key = doc_key(project);
    // the context stood in is part of the state a step puts again: a redo of "New part" is taken from this baseline,
    // and one left in the assembly put the part back with the window outside it
    edits.baseline.project.active_component = project.active_component;
}

/// The command did not apply: say so and record the fact, so the operation can be rolled back.
pub fn cmd_fail(cmd_failed: &mut bool, status: &mut Notice, text: String) {
    *status = Notice::error(text);
    *cmd_failed = true;
}

/// Make sure the active context path is sound: it starts at the root and holds no deleted nodes.
pub fn ensure_active_path(active_path: &mut Vec<Id>, project: &mut Project) {
    let root = project.ensure_root();
    let comp_ids: std::collections::HashSet<Id> = project.components.iter().map(|c| c.id).collect();
    if active_path.first() != Some(&root) {
        *active_path = vec![root];
    }
    active_path.retain(|id| *id == root || comp_ids.contains(id));
    if active_path.is_empty() {
        *active_path = vec![root];
    }
}

/// WHAT A NUMBER FIELD IN A TOOL BAR NEEDS: the text being typed, the document it may name parameters
/// from, and the colours. Three fields - and the field was a method of `App` only because the buffer is.
pub struct ExprBarCtx<'a> {
    pub bar_exprs: &'a mut std::collections::HashMap<&'static str, String>,
    pub project: &'a qymcad_core::model::Project,
    pub scheme: &'a SchemeUi,
}

/// THE LOOK OF A SCHEME FOR THE PANELS, whatever theme the system says it is in. egui keeps a look for its dark theme
/// and one for its light, follows the system's between them, and `set_visuals` fills only the one in use: Windows
/// telling its light theme on the first frames turned a dark scheme's panels into the factory light look. Reported
/// behaviour (Windows 10): the chosen theme did not survive a restart - kept in the settings, not shown.
pub fn put_look(ctx: &egui::Context, pal: &qymcad_scheme::Palette) {
    let v = qymcad_scheme::visuals(pal);
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("cad_active_palette"), pal.clone()));
}

pub fn apply_theme(scheme: &mut SchemeUi, set: &Settings, ctx: &egui::Context) {
    // A SCHEME SETS BOTH THE CANVAS PALETTE AND THE LOOK OF `egui` ITSELF. These used to be two
    // unrelated things: the theme changed the buttons while the canvas stayed dark — exactly what was
    // reported.
    scheme.pal = scheme.all.iter().find(|p| p.id == set.scheme).cloned().unwrap_or_else(qymcad_scheme::dark);
    // THE LOOK OF `egui` ASKS THE SCHEME rather than choosing between light and dark by itself:
    // otherwise a scheme that colours the interface would colour only the canvas, and half the window
    // would stay factory-coloured. Schemes with no interface colours of their own get exactly that
    // same factory look.
    put_look(ctx, &scheme.pal);
    let stack = icons::get_active_icon_stack(ctx);
    icons::set_active_icon_stack(ctx, stack, &scheme.pal);
    // THE INTERFACE SCALE IS NOT APPLIED HERE: it has nothing to do with the theme. The coupling was
    // hidden and harmful — because of it "adopt the settings" would work even without its own call to
    // the scale, and the guard would stay silent. The scale is applied by those whose business it is:
    // `adopt_settings` and the slider in the window.
}

/// Ctrl+C and Ctrl+X. While editing a sketch with entities selected, this copies or cuts the
/// GEOMETRY (waiting for the base point to be clicked). Otherwise it acts on a tree node (a sketch, a
/// part or a subassembly).
/// Whether there is anything to copy right now (used to enable the Edit menu items).
pub fn clipboard_can_copy(project: &Project, sel: Sel, sel_sk: &SketchSelection, sketch_ses: SketchSession) -> bool {
    if edit_si(project, &sketch_ses).is_some() {
        return sel_sk.items.iter().any(|(k, _)| *k == 1);
    }
    match sel {
        Sel::Sketch(_) => true,
        Sel::Component(ci) => project.components.get(ci).map(|c| c.id) != Some(project.root),
        _ => false,
    }
}

/// Fillet ALL the corners of the current sketch's contour with the `sk_fillet` radius.
pub fn fillet_all_corners(project: &Project, corner: &mut CornerInput, picking: &mut Picking, sel: Sel, sel_sk: &SketchSelection, status: &mut Notice, tool_prefs: &SketchToolPrefs) {
    // A COMMAND. With a selection, the popup opens on it straight away; without one, the mode becomes "click a shape".
    if let Sel::Sketch(si) = sel {
        let only: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
        // the tool is held either way: after Enter it waits for the next shape, as a tool of the sketch stays in hand
        *picking = Picking::FilletAll;
        if only.is_empty() {
            *status = Notice::info(qymcad_i18n::tr("g-fillet-all-hint"));
        } else {
            // THE CHOSEN LINES GO IN AS THE WHOLE SET, and the corners they make are the corners one answer cuts:
            // this command is the same act as naming the lines with Shift, said in one word.
            corner.at = Some((si, 0, false));
            corner.pos = None; // the centre of the canvas
            corner.buf = format!("{}", (tool_prefs.fillet * 1000.0).round() / 1000.0);
            corner.focus = true;
            corner.shifted = false;
            corner.set.follow(project, si, &only);
        }
    }
}

/// Whether the selection mode is active (no tool is in hand).
///
/// Three fields had to agree for this to be true, and it was three of the nine that could disagree.
/// Now it is one question to one field.
pub fn in_select_mode(armed: &Armed) -> bool {
    !armed.any()
}

pub fn cancel_feat_cmd(pc: &mut PartCtx) {
    // THE MODE IS READ BEFORE THE CLOSE. `cmd.close()` resets the command to its default state, and
    // `prev_3d` along with it, to `false`. The old code read it AFTERWARDS and so compared the current
    // view against a false "it was flat": working in 3D, Esc out of ANY command threw you into a flat
    // projection with a refit. Reported behaviour: open the editing of an operation, press Esc, and the
    // viewport breaks into some two-dimensional projection — and so with everything.
    let prev_3d = pc.cmd.prev_3d;
    pc.cmd.close(pc.armed); // the command is closed as a whole, not just a field zeroed out
    pc.cmd.edit = None;
    end_feat_cmd_state(crate::feat_picks_in!(pc), pc.cmd, pc.gsel);
    // The view borrowed for the command is RESTORED whole (see `borrow_view`): the camera belongs to
    // the person, and cancelling an action does not touch it. If nothing was borrowed, simply restore
    // the mode, WITHOUT refitting: refitting is exactly what throws away a carefully arranged view.
    if pc.view_restore.is_some() {
        return_view(pc);
    } else if *pc.mode_3d != prev_3d {
        *pc.mode_3d = prev_3d;
    }
    *pc.status = Notice::info(qymcad_i18n::tr("msg-cancelled"));
}

/// An edge polyline is nearly STRAIGHT (and so fit to be an axis): every point lies on the line from the
/// first to the last. An arc or a circle (a deviation above 1% of the length plus 0.05 mm) is rejected,
/// otherwise the chord of a circle would become an "axis".
pub fn is_straight_poly(poly: &[[f32; 3]]) -> bool {
    if poly.len() < 2 {
        return false;
    }
    let a = poly[0];
    let b = *poly.last().unwrap();
    let ab = [(b[0] - a[0]) as f64, (b[1] - a[1]) as f64, (b[2] - a[2]) as f64];
    let len = (ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2]).sqrt();
    if len < 1e-4 {
        return false;
    }
    let u = [ab[0] / len, ab[1] / len, ab[2] / len];
    for p in poly {
        let ap = [(p[0] - a[0]) as f64, (p[1] - a[1]) as f64, (p[2] - a[2]) as f64];
        let t = ap[0] * u[0] + ap[1] * u[1] + ap[2] * u[2];
        let dev = ((ap[0] - t * u[0]).powi(2) + (ap[1] - t * u[1]).powi(2) + (ap[2] - t * u[2]).powi(2)).sqrt();
        if dev > len * 0.01 + 0.05 {
            return false;
        }
    }
    true
}

/// The node index for DELETING the whole chain of body `body`: walk the lineage BACK towards the root,
/// stopping at the DEEPEST body that still HAS a node (for a torus plus a Move that is the torus's
/// node; for a dangling Move whose source is gone, the Move's own node). `delete_feature` then cascades
/// forward and prune finishes the job.
pub fn lineage_delete_ti(project: &qymcad_core::model::Project, body: Id) -> Option<usize> {
    let mut b = body;
    let mut best = project.timeline.iter().position(|n| n.kind.body() == Some(b));
    for _ in 0..256 {
        let Some(src) = project.timeline.iter().find(|n| n.kind.body() == Some(b)).and_then(|n| n.kind.consumed_body()) else { break };
        match project.timeline.iter().position(|n| n.kind.body() == Some(src)) {
            Some(ti) => {
                best = Some(ti);
                b = src;
            }
            None => break, // a source with no node (a dangling one): go no deeper
        }
    }
    best
}

/// Fill in the faces by mesh detection ONLY for bodies WITHOUT a B-rep (a raw imported STL): bodies
/// built by the timeline and those imported from STEP already have their faces from the B-rep topology.
/// The principle: faces are derived from the B-rep, and mesh detection applies only where there is no
/// B-rep at all.
pub fn detect_missing_faces(live: &mut LiveGeom, project: &mut Project) {
    for i in 0..project.bodies.len() {
        let has_brep = project.mesh_id(i).is_some_and(|id| live.shapes.contains_key(&id));
        let empty = project.bodies.get(i).is_none_or(|b| b.faces.is_empty());
        if !has_brep && empty {
            let f = qymcad_meshfit::raw_faces(&project.bodies[i].mesh);
            project.bodies[i].faces = f.clone();
            if let Some(body) = project.mesh_id(i) {
                live.faces.insert(body, f); // mesh detection goes into the cache by body Id as well
            }
        }
    }
}

/// WHETHER A LIVE B-REP IS NEEDED RIGHT NOW - as one list, not as a condition scattered across places.
///
/// A project opened from a bundle carries no live B-rep: it is built ON DEMAND. Whoever does not state the
/// demand silently gets NOTHING, and that looks like "the tool does not work". And so it was: a joint anchor
/// on an edge found nothing, a sketch on a face did not show that face's outline, and a thread answered a
/// click on a cylinder with "missed - click a cylindrical face". The list must be a single one, otherwise the
/// next edge-based tool will forget about it all over again.
/// WHAT THE INTERFACE IS IN THE MIDDLE OF: the command being run, what it has picked, the joint being
/// made, whether a click is awaited, the sketch session, and which workbench is showing.
///
/// Six things that together say what the user is doing, as opposed to what the document holds. Whether
/// a live B-rep has to be kept in memory is decided by exactly these.
#[derive(Clone, Copy)]
pub struct Doing<'a> {
    pub armed: &'a Armed,
    pub cmd: &'a FeatCommand,
    pub gsel: &'a GeomSelection,
    pub joint: &'a JointCommand,
    pub picking: Picking,
    pub sketch_ses: SketchSession,
    pub workbench: Workbench,
}

/// THE FLAT SHEET a sketch is drawn on: how it is panned and zoomed, and the rectangle it lands in.
///
/// The pair travels together everywhere a sketch point becomes a screen point.
#[derive(Clone, Copy)]
pub struct Sheet {
    pub view: View2d,
    pub rect: Rect,
}

impl Sheet {
    /// Where a point of the sketch lands on the screen.
    ///
    /// The sheet is held BY VALUE: `View2d` is a handful of numbers, so nothing here borrows the
    /// document, and the frame can be lifted into a local even in a function that goes on to change it.
    pub fn at(&self, p: Point2) -> Pos2 {
        let c = self.rect.center();
        Pos2::new(c.x + (p.x as f32 - self.view.center.x) * self.view.scale, c.y - (p.y as f32 - self.view.center.y) * self.view.scale)
    }
}

pub fn needs_live_brep(d: Doing, project: &Project, set: &Settings) -> bool {
    let Doing { armed, cmd: _, gsel, joint, picking, sketch_ses, workbench } = d;
    // chamfer (4), fillet (5) and thread (24) work by edges and by face axes
    if matches!(armed.cmd_kind(), 4 | 5 | 24) || !gsel.edges.is_empty() {
        return true;
    }
    // the interference check computes the volume of the common part through the kernel - without a live B-rep no pairs can be found
    if set.show_interference && matches!(workbench, Workbench::Assembly) {
        return true;
    }
    // choosing or replacing a sketch plane snaps to edges and vertices
    if picking.is_sketch_plane() || picking.replace_sketch().is_some() {
        return true;
    }
    // A MATE ANCHOR OF ANY KIND, not only an edge or a vertex (both picking a joint and changing an anchor).
    //
    // This used to test `anchor_mode` against {1, 2}, that is, a face did not count as needing geometry. But
    // a SLIDER's default anchor is exactly a face - and its travel axis is taken from the face's PRINCIPAL
    // DIRECTION, which simply does not exist without a live B-rep. The document's first joint was placed
    // while `regen_faces` was still empty: the axis came from the world axes and the part moved off in the
    // wrong direction. The geometry was raised afterwards (a joint already placed demands it), but the part
    // stayed where it had come to rest - a minimal displacement does not move it for nothing. One wrong
    // second, and the assembly is crooked for good.
    if joint.pick_faces || joint.edit_repick.is_some() {
        return true;
    }
    // JOINTS ALREADY PLACED ON EDGES AND VERTICES COUNT TOO.
    //
    // Reported behaviour: the joints do not move and a slider's direction is simply wrong however it is
    // picked. Measured on that document: edges were gathered for 2 bodies out of 138, and all five joints on
    // edges (all three sliders) had no connector frame at all - their bodies are imported, and an import is
    // not raised without being asked. A connector's axis is read from `regen_edges`; no edges means no axis,
    // and the solver then moves the part not along the picked edge but along whatever it substitutes for the
    // emptiness.
    //
    // THE PRICE OF THIS DECISION IS STATED PLAINLY: an assembly whose joints sit on edges raises the live
    // B-rep on opening - the "Preparing B-rep" step that lazy building was meant to avoid. There is no
    // cheaper way: a joint without an axis is not "slightly slower", it is a wrong assembly.
    //
    // FACES TOO. An anchor on a face without live geometry gives a frame with no known roll: a rigid joint
    // then leaves the part with a degree of freedom, and a slider on a flat face does not know the principal
    // direction and travels anywhere. A check by matrix showed exactly that.
    if project.connectors.iter().any(|c| {
        matches!(c.anchor, qymcad_core::feature::AnchorRef::EdgeMid(..) | qymcad_core::feature::AnchorRef::Vertex(..) | qymcad_core::feature::AnchorRef::FaceCenter(..))
            && project.joints.iter().any(|j| j.a == c.id || j.b == c.id)
    }) {
        return true;
    }
    // an open sketch ON A FACE draws that face's outline
    sketch_ses.editing.and_then(|si| project.sketches.iter().find(|s| s.id == si)).is_some_and(|s| matches!(s.plane, qymcad_core::feature::SketchPlane::Face(..)))
}

pub fn finish_drawing(ed: Editing, pending_import: &mut PendingImport, sketch_ses: SketchSession, closed: bool) {
    if let Some(pts) = pending_import.draw_pts.take() {
        let min = if closed { 3 } else { 2 };
        if pts.len() >= min {
            // while editing, fill the active sketch; otherwise start a new one
            if let Some(si) = edit_si(ed.project, &sketch_ses) {
                if ed.project.fill_sketch_polyline(si, pts, closed) {
                    *ed.sel = Sel::Sketch(si);
                } else {
                    *ed.status = Notice::refusal(qymcad_i18n::tr("io-sketch-has-profile"));
                }
            } else {
                ed.project.add_line_sketch(qymcad_i18n::tr("io-sketch"), pts, closed);
                *ed.sel = Sel::Sketch(ed.project.sketches.len() - 1);
            }
            invalidate(ed.regen);
            ed.view.initialized = false;
        }
    }
}

/// FINISHING A FEATURE COMMAND (applied or cancelled) — both the aiming and the subject are cleared.
///
/// The difference from `clear_feat_picks`: the command's subject (the sketch, the profiles, the
/// parameters, the selected edges and faces) must NOT be cleared at the START — it is gathered BEFORE
/// the button is pressed, as a pre-selection. But once the command is over it has to be forgotten, or
/// the next one would pick up someone else's selection.
pub fn end_feat_cmd_state(picks: FeatPicks, cmd: &mut FeatCommand, gsel: &mut GeomSelection) {
    clear_feat_picks(picks);
    cmd.sketch = None;
    cmd.params.clear();
    gsel.profiles.clear();
    gsel.edges.clear();
    gsel.faces.clear();
    gsel.faces_body = None; // the scope of the multiple face selection
                            // UNFINISHED MENU GESTURES end together with the command too: waiting for the second face of a
                            // "between", the memory of the face and edge last pointed at. A wait that outlived its command
                            // would fire in the next one.
    gsel.between_first = None;
    gsel.last_face = None;
    gsel.last_edge = None;
    gsel.described = None;
}

/// THE KEY HINT, ALLOWING FOR FOCUS: "U" while the keyboard is free, and "Alt+U" while the caret sits
/// in an input field. Written the way this system writes keys (`⌥U` on a Mac).
///
/// The rule "hold Alt when focused" would be a secret without this hint, and secret mechanisms go
/// unused: `U` gets pressed once, nothing happens, and it is never tried again.
pub fn hotkey_hint(dc: &DrawCtx, ctx: &egui::Context, action: &str) -> String {
    let k = hotkey_key(dc.set, action);
    if k.is_empty() {
        return String::new();
    }
    // a Ctrl chord types nothing, so it works from a field as it is; only the rest need Alt there
    if ctx.egui_wants_keyboard_input() && !k.starts_with("Ctrl+") {
        key_label(&format!("Alt+{k}"))
    } else {
        key_label(&k)
    }
}
