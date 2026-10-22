//! THE BARS OF THE PART: the tool bar, the options of a command, the bars of a section and of a pattern of components.

use qymcad_ui_state::{AfterApply, Described, Did, Ends, Gate, Head, Notice, Opt, OptKey};
use super::*;

/// The kinds of a body boolean, as a group of the bar, `op` in force.
fn bool_kinds(op: u8) -> Opt {
    Opt::modes(OptKey::of("bool_kind"), Some("f-kind"), &["f-cut-ab", "f-union", "f-intersect"], usize::from(op.min(2)))
}

/// The kind of a body boolean chosen at place `index` of its group.
pub(crate) fn bool_kind_at(index: usize) -> u8 {
    u8::try_from(index).unwrap_or(2).min(2)
}

/// A BODY-TO-BODY BOOLEAN AS ITS BAR DESCRIBES IT: the kind of operation, the bodies taken. Making one, body A is picked
/// and body B is awaited (`bool_pick`); a node reopened by a double click changes its kind here, and Cancel puts back the
/// kind it had.
pub(crate) fn bool_described(pc: &qymcad_ui_state::PartCtx) -> Described {
    use qymcad_core::feature::FeatureKind;
    let head = Head { icon: ph::INTERSECT, name: qymcad_i18n::tr("bool-bodies-btn"), ends: Ends::ApplyCancel, after: AfterApply::Select };
    // CREATING: body A is picked, the click on B and the choice of kind are awaited
    if let Some((a, op)) = pc.boolean.pick {
        let a_name = pc.project.mesh_index(a).map(|mi| qymcad_i18n::name(&pc.project.mesh_name(mi))).unwrap_or_else(|| "?".into());
        let b_name = pc.boolean.b.and_then(|b| pc.project.mesh_index(b)).map(|mi| qymcad_i18n::name(&pc.project.mesh_name(mi)));
        let said = match &b_name {
            Some(b) => qymcad_i18n::tr2("cmd-bool-a-b", "a", &a_name, "b", b),
            None => qymcad_i18n::tr1("cmd-bool-a-is", "name", &a_name),
        };
        // Enter before body B is taken has nothing to make
        let gate = if b_name.is_some() { Gate::Open } else { Gate::Waiting(qymcad_i18n::tr("tb-bool-pick-b")) };
        return Described { head, bar: vec![bool_kinds(op), Opt::Readout(said)], gate, says: vec![], boxed: None };
    }
    // EDITING: a double click on the boolean node changes its KIND in the bar (as reopening the command does for an
    // extrude)
    let op = match pc.boolean.edit.and_then(|ti| pc.project.timeline.get(ti)).map(|n| &n.kind) {
        Some(FeatureKind::BodyBoolean { op, .. }) => *op,
        _ => 0,
    };
    Described { head, bar: vec![bool_kinds(op)], gate: Gate::Open, says: vec![], boxed: None }
}

/// THE KIND OF BOOLEAN NODE `ti` SET TO `op`: one step of undo, like any edit of a node.
pub(crate) fn set_bool_kind(pc: &mut qymcad_ui_state::PartCtx, ti: usize, op: u8) {
    use qymcad_core::feature::FeatureKind;
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("f-operation"));
    if let Some(FeatureKind::BodyBoolean { op: o, .. }) = pc.project.timeline.get_mut(ti).map(|n| &mut n.kind) {
        *o = op;
    }
    pc.project.timeline[ti].dirty = true;
    qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the planner does the counting
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// PUT THE EDIT OF A BOOLEAN NODE DOWN, the node left with the kind it had when it was reopened - Cancel and Esc alike.
pub fn bool_edit_cancel(pc: &mut qymcad_ui_state::PartCtx) {
    use qymcad_core::feature::FeatureKind;
    if let Some(ti) = pc.boolean.edit.take() {
        if matches!(pc.project.timeline.get(ti).map(|n| &n.kind), Some(FeatureKind::BodyBoolean { op, .. }) if *op != pc.boolean.was) {
            set_bool_kind(pc, ti, pc.boolean.was);
        }
    }
    *pc.status = Notice::info(qymcad_i18n::tr("msg-cancelled"));
}

/// WHAT THE BAR OF THE TOOLS WAITING ON A PLANE TOUCHES: the section, the mirrored copy of a part, the pencil, a command
/// waiting for its sketch and the measure; and the view the box of the section stands in.
pub struct PlaneBarCtx<'a> {
    pub regen: &'a mut qymcad_ui_state::Rebuilding,
    pub section: &'a mut qymcad_ui_state::SectionTool,
    pub mirror: &'a mut qymcad_ui_state::MirrorParams,
    pub picking: &'a mut qymcad_ui_state::Picking,
    pub project: &'a mut qymcad_core::model::Project,
    pub edits: &'a mut qymcad_ui_state::Edits,
    pub active_path: &'a [qymcad_core::model::Id],
    pub status: &'a mut qymcad_ui_state::Notice,
    pub m3: &'a mut qymcad_ui_state::Measure3,
    pub scheme: &'a qymcad_ui_state::SchemeUi,
    pub cam: &'a qymcad_ui_state::Cam3,
    pub set: &'a qymcad_ui_state::Settings,
    pub canvas: egui::Rect,
}

/// THE TOOL IN HAND OF THE PLACE OF THE SECTION, as its bar describes it: the measure, a command waiting for its
/// sketch, the pencil waiting for a plane, the mirrored copy of a part, or the section itself.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlaneTool {
    Measure,
    SketchFor(qymcad_ui_state::PartTool),
    Pencil,
    MirrorPart,
    Section,
}

impl PlaneTool {
    /// The tool of the place in hand now; none when the place is empty.
    pub(crate) fn of(pb: &PlaneBarCtx) -> Option<PlaneTool> {
        if pb.m3.on {
            Some(PlaneTool::Measure)
        } else if let Some(kind) = pb.picking.sketch_for() {
            Some(PlaneTool::SketchFor(kind))
        } else if pb.picking.is_sketch_plane() {
            Some(PlaneTool::Pencil)
        } else if pb.mirror.in_hand() {
            Some(PlaneTool::MirrorPart)
        } else if pb.section.pick || pb.section.plane.is_some() {
            Some(PlaneTool::Section)
        } else {
            None
        }
    }
}

impl PlaneBarCtx<'_> {
    /// Drawing, through this workbench's own fields - as in the part and the sketch.
    pub fn draw(&self) -> qymcad_ui_state::DrawCtx<'_> {
        qymcad_ui_state::DrawCtx { cam: self.cam, set: self.set, scheme: self.scheme, project: self.project, active_path: self.active_path }
    }
}

/// THE TOOL OF THE PLACE OF THE SECTION AS ITS BAR DESCRIBES IT. Each of them waited with only the status line saying
/// so, and the bar stood empty over a tool held.
pub(crate) fn plane_tool_described(pb: &PlaneBarCtx, tool: &PlaneTool) -> Described {
    let head = |icon, name: &str, ends, after| Head { icon, name: qymcad_i18n::tr(name), ends, after };
    // what is pointed at, not typed: Enter before it has nothing to do
    let waiting = |head: Head, ask: String| Described { head, bar: vec![Opt::Readout(ask.clone())], gate: Gate::Waiting(ask), says: vec![], boxed: None };
    match tool {
        // the measure: its name, what is taken, the answer, the way out; the answer comes with the second element
        PlaneTool::Measure => {
            let head = head(ph::RULER, "hotkey-part-i", Ends::Close, AfterApply::Stays);
            if pb.m3.picks.len() < 2 {
                let mut d = waiting(head, qymcad_i18n::tr("m3-hint-short"));
                d.bar.extend(pb.m3.picks.iter().map(|p| Opt::Readout(p.what.clone())));
                return d;
            }
            let taken: Vec<String> = pb.m3.picks.iter().map(|p| p.what.clone()).collect();
            Described { head, bar: vec![Opt::Readout(taken.join(" - ")), Opt::Readout(pb.status.words.clone())], gate: Gate::Open, says: vec![], boxed: None }
        }
        PlaneTool::SketchFor(kind) => {
            let name = match kind {
                qymcad_ui_state::PartTool::Revolve => "cmd-revolve",
                qymcad_ui_state::PartTool::Sweep => "cmd-sweep",
                qymcad_ui_state::PartTool::Loft => "cmd-loft",
                _ => "cmd-extrude",
            };
            waiting(head(ph::CUBE, name, Ends::ApplyCancel, AfterApply::Stays), qymcad_i18n::tr("g-pick-sketch-for"))
        }
        PlaneTool::Pencil => waiting(head(ph::PENCIL_SIMPLE, "status-new-sketch", Ends::ApplyCancel, AfterApply::Stays), qymcad_i18n::tr("g-sketch-pick-plane")),
        // the mirrored copy of a part waits for the part, then for a plane
        PlaneTool::MirrorPart => {
            let head = head(ph::FLIP_HORIZONTAL, "mp-title", Ends::ApplyCancel, AfterApply::Select);
            let name = pb.mirror.part.and_then(|src| pb.project.components.iter().find(|c| c.id == src)).map(|c| qymcad_i18n::name(&c.name));
            match (name, pb.mirror.at.is_some()) {
                (Some(name), true) => Described { head, bar: vec![Opt::Readout(qymcad_i18n::tr1("mp-plane-taken-of", "name", &name))], gate: Gate::Open, says: vec![], boxed: None },
                (Some(name), false) => waiting(head, qymcad_i18n::tr1("mp-pick", "name", &name)),
                (None, _) => waiting(head, qymcad_i18n::tr("tb-mirror-pick-part")),
            }
        }
        // THE SECTION: while it is placed - the plane picked by a click - it is a tool in hand, applied or cancelled;
        // once applied it stays on under any command, with its offset, tilts and flip, until it is turned off. The
        // offset and the tilts are typed at the plane.
        PlaneTool::Section => {
            let ends = if pb.section.pick { Ends::ApplyCancel } else { Ends::Close };
            let head = head(ph::SQUARE_HALF, "sec-btn", ends, AfterApply::Select);
            if pb.section.plane.is_none() {
                return waiting(head, qymcad_i18n::tr("tb-section-pick"));
            }
            let flip = Opt::press(OptKey::of("flip"), format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("sec-flip-btn"))).hint(qymcad_i18n::tr("sec-flip-hint"));
            Described { head, bar: vec![flip], gate: Gate::Open, says: vec![], boxed: None }
        }
    }
}

/// The other half of the section shown: the plane turned over, its offset with it.
pub(crate) fn section_flip(section: &mut qymcad_ui_state::SectionTool) {
    if let Some((_, n)) = &mut section.plane {
        *n = [-n[0], -n[1], -n[2]];
        section.offset = -section.offset;
    }
}

/// THE SECTION TAKES THE VALUES TYPED at its plane: the offset and the two tilts, formulas as every field of a tool. They
/// stood in the bar as numbers dragged by the mouse, with no formula and no place by the plane they move.
pub(crate) fn section_takes_fields(pb: &mut PlaneBarCtx) {
    let v = |key: &str| pb.section.fields.iter().find(|p| p.key == key).map_or(0.0, |p| p.val);
    let offset = v("offset");
    let rot = [v("tilt1"), v("tilt2")];
    if offset != pb.section.offset || rot != pb.section.rot {
        pb.section.offset = offset;
        pb.section.rot = rot;
        qymcad_ui_state::invalidate(pb.regen);
    }
}

/// THE FIELDS OF THE SECTION FOLLOW ITS VALUES: made the first time, and a value changed elsewhere - the arrow dragged,
/// the plane turned over or picked anew - written into its field; a formula typed stays while its value holds.
pub(crate) fn section_fields_follow(section: &mut qymcad_ui_state::SectionTool) {
    use qymcad_ui_state::CmdParam;
    if section.fields.is_empty() {
        section.fields = vec![
            CmdParam::new("sec-offset", "offset", 0.0, -100000.0, 100000.0),
            CmdParam::new("sec-tilt1", "tilt1", 0.0, -89.0, 89.0),
            CmdParam::new("sec-tilt2", "tilt2", 0.0, -89.0, 89.0),
        ];
    }
    for (p, v) in section.fields.iter_mut().zip([section.offset, section.rot[0], section.rot[1]]) {
        if (p.val - v).abs() > 1e-9 {
            (p.val, p.txt) = (v, qymcad_core::expr::fmt_num(v));
        }
    }
}

/// PUT THE SECTION TOOL DOWN: a section still being placed goes with it (Esc, Cancel, another tool taken); one
/// already applied stays until "Off".
pub fn section_cancel(section: &mut qymcad_ui_state::SectionTool, regen: &mut qymcad_ui_state::Rebuilding) {
    if section.pick && section.plane.is_some() {
        section.plane = None;
        qymcad_ui_state::invalidate(regen);
    }
    section.pick = false;
    section.drag = false;
    section.drag_anchor = None;
}

/// THE PART A PATTERN OF COMPONENTS REPEATS: the component selected, or the part of the body selected - not the root,
/// and a part with a body; otherwise what is missing, said.
pub fn comp_array_source(pc: &mut qymcad_ui_state::PartCtx) -> Result<Id, Notice> {
    let src = match *pc.sel {
        qymcad_ui_state::Sel::Component(ci) => pc.project.components.get(ci).map(|c| c.id),
        _ => qymcad_ui_state::selected_body(&*pc.project, &*pc.sel).and_then(|b| pc.project.body_owner(b)),
    };
    let Some(src) = src.filter(|c| *c != pc.project.root) else {
        return Err(Notice::info(qymcad_i18n::tr("msg-pick-part-first")));
    };
    if pc.project.active_body(src).is_none() {
        return Err(Notice::refusal(qymcad_i18n::tr("msg-part-no-body")));
    }
    Ok(src)
}

/// TAKE A PATTERN OF COMPONENTS of part `src` in hand: a row of three or a full turn of six, one direction, the axis of
/// the assembly; the counts and the step at the geometry.
pub fn begin_comp_array(pc: &mut qymcad_ui_state::PartCtx, mode: qymcad_ui_state::PatternKind, src: Id) {
    *pc.carr = qymcad_ui_state::CompArrayCmd { mode: Some(mode), src, dir: 0, axis: 2, edit: 0 };
    pc.gsel.before.spend_object(); // the part picked is the pattern's object, not a click of it
    pc.arr.count = if mode == qymcad_ui_state::PatternKind::Circular { 6 } else { 3 };
    (pc.arr.two, pc.arr.three, pc.arr.axis) = (false, false, 0);
    *pc.mode_3d = true;
    pc.cmd.params.clear();
    sync_comp_array_params(*pc.arr, Some(mode), pc.cmd);
    *pc.status = Notice::info(qymcad_i18n::tr(if mode == qymcad_ui_state::PatternKind::Circular { "msg-comp-circ-array" } else { "msg-comp-lin-array" }));
}

/// TAKE A PATTERN OF COMPONENTS: of the part selected, its preview at once; with none, the tool waits for a part to be
/// picked, as every tool waits for what it works on (`comp_array_waiting`).
pub fn take_comp_array(pc: &mut qymcad_ui_state::PartCtx, mode: qymcad_ui_state::PatternKind, src: Option<Id>) {
    match src {
        Some(src) => begin_comp_array(pc, mode, src),
        None => {
            *pc.carr = qymcad_ui_state::CompArrayCmd { mode: Some(mode), src: 0, dir: 0, axis: 2, edit: 0 };
            pc.cmd.params.clear();
            *pc.status = Notice::info(qymcad_i18n::tr("msg-pick-part-first"));
        }
    }
}

/// A PATTERN OF COMPONENTS WAITING FOR ITS PART: its name, what to pick, and nothing to apply yet.
pub(crate) fn comp_array_waiting(pc: &qymcad_ui_state::PartCtx) -> Described {
    let title = if pc.carr.is_circular() { "cmd-comp-circ-array" } else { "cmd-comp-lin-array" };
    let ask = qymcad_i18n::tr("msg-pick-part-first");
    let head = Head { icon: ph::STACK, name: qymcad_i18n::tr(title), ends: Ends::Close, after: AfterApply::Select };
    Described { head, bar: vec![Opt::Readout(ask.clone())], gate: Gate::Waiting(ask), says: Vec::new(), boxed: None }
}

/// A PATTERN OF COMPONENTS AS ITS BAR DESCRIBES IT: the part repeated, then the options of a pattern of bodies.
/// THE TOP BAR OF A COMPONENT PATTERN (in an assembly) - the same bar as the pattern of bodies, described by the same
/// options: the direction or the axis here; the counts, the step or the angle in fields at the geometry; Enter applies.
pub(crate) fn comp_array_described(pc: &qymcad_ui_state::PartCtx) -> Described {
    let name = pc.project.components.iter().find(|c| c.id == pc.carr.src).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_default();
    let source = Some(qymcad_i18n::tr1("cmd-source", "name", &name));
    let mut says = Vec::new();
    let (title, bar) = if pc.carr.is_circular() {
        // AN AXIS OF ITS OWN, picked in the view as a pattern of bodies picks it, or one of the assembly's. Reported
        // behaviour: the axis of a circular pattern of parts could only be X, Y or Z through the origin of the assembly.
        ("cmd-comp-circ-array", circular_array_opts(&*pc.project, &*pc.arr, source, NoAxisPicked::OneOf(pc.carr.axis)))
    } else {
        // THE SECOND AND THIRD DIRECTIONS, as a pattern of bodies has them. Reported behaviour: a linear pattern of
        // parts ran one way only.
        ("cmd-comp-lin-array", linear_array_opts(source, &mut says))
    };
    let head = Head { icon: ph::STACK, name: qymcad_i18n::tr(title), ends: Ends::ApplyCancel, after: AfterApply::Select };
    Described { head, bar, gate: comp_array_gate(pc), says, boxed: None }
}

/// CAN THE PATTERN OF COMPONENTS BE APPLIED: the source is picked before the tool is taken, so what shuts the gate is a
/// value its fields cannot take, then a layout past the limit - said with the limit in words - and a row of one
/// instance, the source alone.
pub(crate) fn comp_array_gate(pc: &qymcad_ui_state::PartCtx) -> Gate {
    let kind = qymcad_ui_state::comp_array_kind(*pc.arr, pc.carr, pc.cmd);
    let gate = match kind.count() {
        0 => Gate::Wrong(qymcad_i18n::error_words::error_text(&qymcad_core::feature::pattern_too_large(&kind.counts()))),
        1 => Gate::Wrong(qymcad_i18n::tr("msg-comp-array-one")),
        _ => Gate::Open,
    };
    gate.with_fields(&pc.cmd.params, &*pc.project)
}

/// WHAT WAS DONE TO AN OPTION OF A PATTERN OF COMPONENTS: its direction and axis are its own, the rest is the pattern
/// of bodies'. A direction ticked on runs along the next axis, so the tick itself makes a grid.
pub(crate) fn comp_array_act(pc: &mut qymcad_ui_state::PartCtx, did: Did) {
    let axis = |index: usize| u8::try_from(index).unwrap_or(2).min(2);
    match did {
        Did::Chose { key, index } if key.name == "dir" => pc.carr.dir = axis(index),
        Did::Chose { key, index } if key.name == "carr_axis" => pc.carr.axis = axis(index),
        Did::Switched { key } if key.name == "dir2_on" || key.name == "dir3_on" => qymcad_ui_state::switch_direction(pc.arr, pc.carr.dir, qymcad_ui_state::MoreDirection::of(key.name)),
        d => act(pc, d),
    }
    if !pc.arr.two {
        pc.arr.three = false; // the third direction only sits on top of the second
    }
}

/// REOPEN A PATTERN OF COMPONENTS for editing (a double click on its row): the command as it was applied - the counts,
/// the axis, the step or the angle, and the second and third directions of a grid.
pub fn open_comp_array_edit(pc: &mut qymcad_ui_state::PartCtx, pid: Id) {
    use qymcad_core::model::CompPatternKind;
    let Some(pat) = pc.project.comp_pattern(pid) else { return };
    let (mode, dir, axis) = match pat.kind {
        CompPatternKind::Linear { dir, .. } => (qymcad_ui_state::PatternKind::Linear, axis_of(dir), 2u8),
        CompPatternKind::Circular { dir, .. } => (qymcad_ui_state::PatternKind::Circular, 0u8, axis_of(dir)),
    };
    *pc.carr = qymcad_ui_state::CompArrayCmd { mode: Some(mode), src: pat.src, dir, axis, edit: pid };
    *pc.mode_3d = true;
    let values: Vec<FieldValue> = match pat.kind {
        CompPatternKind::Circular { angle, count, axis, .. } => {
            pc.arr.count = count.max(1);
            pc.arr.axis = axis;
            vec![FieldValue { key: "cangle", value: angle }]
        }
        CompPatternKind::Linear { step, count, more, .. } => {
            pc.arr.count = count.max(1);
            pc.arr.two = more[0].2 > 1;
            pc.arr.three = pc.arr.two && more[1].2 > 1;
            (pc.arr.count2, pc.arr.dir2, pc.arr.count3, pc.arr.dir3) = (more[0].2.max(1), axis_of(more[0].0), more[1].2.max(1), axis_of(more[1].0));
            vec![FieldValue { key: "cstep", value: step }, FieldValue { key: "cstep2", value: more[0].1 }, FieldValue { key: "cstep3", value: more[1].1 }]
        }
    };
    pc.cmd.params.clear();
    sync_comp_array_params(*pc.arr, Some(mode), pc.cmd);
    for f in values {
        if let Some(p) = pc.cmd.params.iter_mut().find(|p| p.key == f.key) {
            (p.val, p.txt) = (f.value, qymcad_core::expr::fmt_num(f.value));
        }
    }
    // A FORMULA OF A VALUE COMES BACK AS IT WAS TYPED, the counts' too: the node keeps it (`keep_item_values`)
    for p in pc.cmd.params.iter_mut() {
        if let Some(f) = pc.project.feat_dim(pid, &p.key).filter(|f| !f.trim().is_empty()) {
            p.txt = f.to_string();
        }
    }
    *pc.status = Notice::info(qymcad_i18n::tr("msg-edit-comp-array"));
}

/// A value a field of a command is opened with, by the key of the field.
struct FieldValue {
    key: &'static str,
    value: f64,
}

/// THE FIELDS OF A PATTERN OF COMPONENTS follow its options: the count always; a linear one's step, and a second and a
/// third count and step with the second and third direction ticked; a circular one's angle when it is not a full turn.
/// A field that stays keeps what is typed in it.
pub fn sync_comp_array_params(arr: qymcad_ui_state::ArrayParams, mode: Option<qymcad_ui_state::PatternKind>, cmd: &mut qymcad_ui_state::FeatCommand) {
    use qymcad_ui_state::CmdParam;
    let count = |label, key, n: u32| CmdParam::new(label, key, f64::from(n), 1.0, 512.0).whole();
    let step = |label, key| CmdParam::new(label, key, 30.0, 0.01, 100000.0);
    let mut want = vec![count("cmd-copies", "count", arr.count.max(1))];
    if mode == Some(qymcad_ui_state::PatternKind::Linear) {
        want.push(step("f-pitch", "cstep"));
        if arr.two {
            want.extend([count("f-copies2", "count2", arr.count2.max(2)), step("f-pitch2", "cstep2")]);
            if arr.three {
                want.extend([count("f-copies3", "count3", arr.count3.max(2)), step("f-pitch3", "cstep3")]);
            }
        }
    } else {
        // THE ANGLE IS ALWAYS A FIELD of the box, a full turn of 360 as it opens: a tick of a full circle stood in the bar
        want.push(CmdParam::new("f-angle", "cangle", 360.0, -3600.0, 3600.0));
    }
    cmd.params.retain(|p| want.iter().any(|w| w.key == p.key));
    for w in want {
        if !cmd.params.iter().any(|p| p.key == w.key) {
            cmd.params.push(w);
        }
    }
}

mod feat_bar;
pub(crate) use feat_bar::*;
mod commands;
pub use commands::*;
