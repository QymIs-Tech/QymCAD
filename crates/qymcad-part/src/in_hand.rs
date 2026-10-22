//! THE TOOLS OF A PART IN HAND, as the frame drives them (`qymcad_ui_state::tool_in_hand`): what each is now, what its
//! Apply, Cancel and a step back do, and what its fields follow every frame. The bar, the box, the keys and the words
//! are the frame's.

use qymcad_ui_state::{Described, Did, FramedTool, NotApplied, Notice, Stepped};
use super::*;

/// THE TOOL OF A PART IN HAND, if one is: a command, a boolean of bodies, a pattern of components, or a step typed at a
/// gizmo - the order the place of the command bar showed them in.
pub fn in_hand<'a>(pc: &'a mut qymcad_ui_state::PartCtx<'_>) -> Option<Box<dyn FramedTool + 'a>> {
    if let Some(kind) = pc.armed.part_tool() {
        return Some(Box::new(CommandTool { pc, kind, asks: Vec::new(), free: Vec::new() }));
    }
    if pc.boolean.pick.is_some() || bool_edited(pc).is_some() {
        return Some(Box::new(BoolTool { pc }));
    }
    if pc.carr.mode.is_some() {
        return Some(Box::new(CompArrayTool { pc, asks: Vec::new() }));
    }
    let (on, ax, rot) = pc.body_giz.num?;
    Some(Box::new(GizmoTool { pc, on, step: GizmoStep { ax, rot, by: 0.0, at: [0.0; 3] }, asks: Vec::new() }))
}

/// THE TOOL OF THE PLACE OF THE SECTION IN HAND, if one is: the measure, a command waiting for its sketch, the pencil
/// waiting for a plane, the mirrored copy of a part, or the section.
pub fn plane_in_hand<'a>(pb: &'a mut PlaneBarCtx<'_>) -> Option<Box<dyn FramedTool + 'a>> {
    let tool = PlaneTool::of(pb)?;
    Some(Box::new(PlaneTools { pb, tool }))
}

/// A FEATURE COMMAND IN HAND.
struct CommandTool<'a, 'b> {
    pc: &'a mut qymcad_ui_state::PartCtx<'b>,
    kind: qymcad_ui_state::PartTool,
    /// what the frame kept of the names typed under the fields
    asks: Vec<qymcad_ui_state::FieldAsk>,
    /// the numbers of the bar in the box - the starts of a thread, the sides of a prism: no key of the feature holds
    /// them, their drivers are parameters
    free: Vec<&'static str>,
}

impl CommandTool<'_, '_> {
    /// Does Enter take an extrusion or a revolution from the flat choice of its contours, the contours picked, into 3D.
    fn takes_into_3d(&self) -> bool {
        takes_into_3d(self.pc)
    }
}

impl FramedTool for CommandTool<'_, '_> {
    fn describe(&mut self, ctx: &egui::Context) -> Described {
        command_step(self.pc, ctx);
        let mut d = describe(self.pc, ctx, self.kind);
        d.boxed = command_box(self.pc);
        self.free = if self.kind == qymcad_ui_state::PartTool::Recognise { Vec::new() } else { qymcad_ui_state::name_bar_numbers(&mut d) };
        // Enter in the flat choice of the contours takes the command into 3D, not out of the hand
        if self.takes_into_3d() {
            d.head.after = qymcad_ui_state::AfterApply::Stays;
        }
        d
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.pc.cmd.params
    }

    fn bar_texts(&mut self) -> Option<&mut std::collections::HashMap<&'static str, String>> {
        Some(&mut *self.pc.bar_exprs)
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.pc.draw()
    }

    fn took(&mut self, did: Did) {
        act(self.pc, did);
    }

    fn take_asks(&mut self, asks: Vec<qymcad_ui_state::FieldAsk>) {
        self.asks = asks;
    }

    /// Enter of an extrusion or a revolution in the flat choice of its contours, the contours picked, takes the command
    /// into 3D to set its size there - the gizmo or the field; the next Enter applies it.
    fn apply(&mut self) -> Result<(), NotApplied> {
        if self.takes_into_3d() {
            let pc = &mut *self.pc;
            *pc.mode_3d = true;
            *pc.status = Notice::info(qymcad_i18n::tr("in-drag-or-type"));
            return Ok(());
        }
        // THE NAMES TYPED UNDER THE FIELDS go on the numbers of the node the command made or edits, as the command
        // wrote them
        let before: Vec<Id> = self.pc.project.timeline.iter().map(|n| n.id).collect();
        let edited = self.pc.cmd.edit;
        apply_feat_cmd_kept(self.pc)?;
        let node = edited.or_else(|| self.pc.project.timeline.iter().map(|n| n.id).find(|id| !before.contains(id)));
        if qymcad_ui_state::name_free_values(self.pc.project, &self.asks, &self.free) {
            qymcad_ui_state::invalidate(self.pc.regen);
        }
        if let Some(node) = node {
            for ask in self.asks.iter().filter(|a| !self.free.contains(&a.field.as_str())) {
                let target = qymcad_core::model::DimTarget::Feature { node, key: ask.field.clone() };
                if ask.renamed(&qymcad_i18n::name(&self.pc.project.name_of_target(&target))) {
                    qymcad_ui_state::give_name(self.pc.project, target, &ask.name);
                }
            }
        }
        Ok(())
    }

    fn cancel(&mut self) {
        qymcad_ui_state::cancel_feat_cmd(self.pc);
    }

    /// The command made its feature: put down, the picks let go, the view given back (`put_down_command`).
    fn put_down(&mut self) {
        put_down_command(self.pc);
    }

    /// The choice of a contour of a sweep or a loft is let go first, returning to 3D; then a reference being picked -
    /// an axis, a line, the reference of a chamfer, the neutral face of a draft; then the command.
    fn back(&mut self) -> Stepped {
        let pc = &mut *self.pc;
        if pc.picking.contour().is_some() {
            pc.picking.clear();
            qymcad_ui_state::return_view(pc); // cancelling an action does not touch the camera
            *pc.status = Notice::info(qymcad_i18n::tr("in-contour-cancelled"));
            return Stepped::Back;
        }
        if pc.rev.pick_line {
            qymcad_ui_state::return_view(pc);
        }
        let picking = [&mut pc.arr.axis_pick, &mut pc.rev.pick_axis, &mut pc.rev.pick_line, &mut pc.chamfer.pick_ref, &mut pc.draft.pick_neutral];
        if !picking.iter().any(|p| **p) {
            return Stepped::NothingLeft;
        }
        for p in picking {
            *p = false;
        }
        *pc.status = Notice::info(qymcad_i18n::tr("in-axis-pick-cancelled"));
        Stepped::Back
    }

    /// The fields follow the options: the second side of an extent, the tube thinner than the ring, the second step and
    /// the angle of a pattern, the origin and direction of an axis typed by hand, the recess of a counterbore.
    fn after(&mut self) {
        let pc = &mut *self.pc;
        sync_dir_cmd_params(&*pc.armed, &mut *pc.cmd, &*pc.project);
        sync_torus_params(&*pc.armed, &mut *pc.cmd, &*pc.project);
        sync_array_params(*pc.arr, &*pc.armed, &mut *pc.cmd, &*pc.project);
        sync_datum_axis_params(&*pc.armed, &mut *pc.cmd, &*pc.datum);
        sync_hole_params(&*pc.armed, &mut *pc.cmd, *pc.hole, &*pc.project);
        take_counts(&*pc.armed, &*pc.cmd, Counts { arr: &mut *pc.arr, prim: &mut *pc.prim });
    }
}

/// THE BOX OF THE COMMAND IN HAND, at the geometry, when it has one now: in 3D, with fields, standing by what it sizes.
/// A pattern or a hole sizes what is picked: before the pick there is nothing to size, and a box beside the body would
/// lie over the very place to be clicked - the centre of the face a hole is put on.
fn command_box(pc: &mut qymcad_ui_state::PartCtx) -> Option<qymcad_ui_state::BoxSpec> {
    let kind = pc.armed.part_tool()?;
    if pc.cmd.params.is_empty() || !*pc.mode_3d || (kind.sizes_what_is_picked() && !cmd_ready(pc)) {
        return None;
    }
    let place = cmd_place(pc)?;
    // in a symmetric chamfer the d2 field (the second leg or the angle) takes no part, so it is hidden
    let hidden = if kind == qymcad_ui_state::PartTool::Chamfer && pc.chamfer.mode == qymcad_core::feature::ChamferMode::Symmetric { vec!["d2"] } else { Vec::new() };
    // a one-shot focus of the first field: Enter works straight away, what it holds selected
    let focus = qymcad_ui_state::Focus::take(&mut pc.cmd.focus);
    Some(qymcad_ui_state::BoxSpec { place, layout: command_layout(pc), hidden, focus, names: field_names(pc) })
}

/// THE NAME OF EVERY FIELD OF THE COMMAND, as typed so far or as the feature reopened holds it: a number of a feature is
/// named as a dimension of a sketch is, in the row under its field.
fn field_names(pc: &qymcad_ui_state::PartCtx) -> Vec<qymcad_ui_state::FieldName> {
    // THE SETTINGS OF RECOGNISING A MESH are how it is read, not values of the model: nothing reads them by a name
    if pc.armed.part_tool() == Some(qymcad_ui_state::PartTool::Recognise) {
        return Vec::new();
    }
    pc.cmd
        .params
        .iter()
        .map(|p| {
            let target = pc.cmd.edit.map(|node| qymcad_core::model::DimTarget::Feature { node, key: p.key.clone() });
            let had = target.as_ref().map(|t| qymcad_i18n::name(&pc.project.name_of_target(t))).unwrap_or_default();
            qymcad_ui_state::FieldName::of(&p.key, had, target)
        })
        .collect()
}

/// The boolean node being edited, while it is still a boolean.
fn bool_edited(pc: &qymcad_ui_state::PartCtx) -> Option<usize> {
    let ti = pc.boolean.edit?;
    matches!(pc.project.timeline.get(ti).map(|n| &n.kind), Some(qymcad_core::feature::FeatureKind::BodyBoolean { .. })).then_some(ti)
}

/// A BOOLEAN OF BODIES: made - body A picked, B and the kind awaited - or reopened, its kind changed as it is chosen.
struct BoolTool<'a, 'b> {
    pc: &'a mut qymcad_ui_state::PartCtx<'b>,
}

impl FramedTool for BoolTool<'_, '_> {
    fn describe(&mut self, _ctx: &egui::Context) -> Described {
        bool_described(self.pc)
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.pc.cmd.params
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.pc.draw()
    }

    fn took(&mut self, did: Did) {
        let Did::Chose { index, .. } = did else { return };
        let op = bool_kind_at(index);
        if let Some((a, _)) = self.pc.boolean.pick {
            self.pc.boolean.pick = Some((a, op));
        } else if let Some(ti) = bool_edited(self.pc) {
            set_bool_kind(self.pc, ti, op);
        }
    }

    fn apply(&mut self) -> Result<(), NotApplied> {
        if self.pc.boolean.pick.is_some() {
            return apply_boolean(self.pc);
        }
        *self.pc.status = Notice::info(qymcad_i18n::tr("in-bool-done"));
        Ok(())
    }

    /// Made, it is let go; reopened, the node is left with the kind it had.
    fn cancel(&mut self) {
        if self.pc.boolean.pick.is_some() {
            self.put_down();
            *self.pc.status = Notice::info(qymcad_i18n::tr("in-bool-cancelled"));
        } else {
            bool_edit_cancel(self.pc);
        }
    }

    /// The bodies taken and the node reopened are let go.
    fn put_down(&mut self) {
        let b = &mut *self.pc.boolean;
        (b.pick, b.b, b.edit) = (None, None, None);
    }
}

/// A PATTERN OF COMPONENTS.
struct CompArrayTool<'a, 'b> {
    pc: &'a mut qymcad_ui_state::PartCtx<'b>,
    /// what the frame kept of the drivers typed in its box
    asks: Vec<qymcad_ui_state::FieldAsk>,
}

impl FramedTool for CompArrayTool<'_, '_> {
    /// Its box stands clear of the source part: its edges are what the axis is picked on, and a box laid over them took
    /// the click meant for one.
    fn describe(&mut self, _ctx: &egui::Context) -> Described {
        // THE PART AWAITED: taken with none selected, the tool waits for one, and the part picked is taken at once
        if self.pc.carr.src == 0 {
            match comp_array_source(self.pc) {
                Ok(src) => begin_comp_array(self.pc, self.pc.carr.mode.unwrap_or(qymcad_ui_state::PatternKind::Linear), src),
                Err(_) => return comp_array_waiting(self.pc),
            }
        }
        let mut d = comp_array_described(self.pc);
        if !self.pc.cmd.params.is_empty() && *self.pc.mode_3d {
            let mut spec = qymcad_ui_state::BoxSpec::by(qymcad_ui_state::Place::by(qymcad_ui_state::By::Part(self.pc.carr.src)));
            spec.layout = comp_array_layout(self.pc);
            // its values are named as every value of a box is; no node holds them, their drivers are parameters
            spec.names = self.pc.cmd.params.iter().map(|p| qymcad_ui_state::FieldName::of(&p.key, String::new(), None)).collect();
            d.boxed = Some(spec);
        }
        d
    }

    fn take_asks(&mut self, asks: Vec<qymcad_ui_state::FieldAsk>) {
        self.asks = asks;
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.pc.cmd.params
    }

    fn bar_texts(&mut self) -> Option<&mut std::collections::HashMap<&'static str, String>> {
        Some(&mut *self.pc.bar_exprs)
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.pc.draw()
    }

    fn took(&mut self, did: Did) {
        comp_array_act(self.pc, did);
    }

    fn apply(&mut self) -> Result<(), NotApplied> {
        apply_comp_array_kept(self.pc)?;
        let free: Vec<&str> = self.asks.iter().map(|a| a.field.as_str()).collect();
        if self.asks.iter().any(|a| !a.name.trim().is_empty()) {
            qymcad_ui_state::begin_edit(self.pc.edits, self.pc.project, qymcad_i18n::tr("sk-driver-step"));
            qymcad_ui_state::name_free_values(self.pc.project, &self.asks, &free);
            qymcad_ui_state::commit_edit_if_changed(&mut self.pc.rebuild());
        }
        Ok(())
    }

    fn cancel(&mut self) {
        self.put_down();
        *self.pc.status = Notice::info(qymcad_i18n::tr("msg-comp-array-cancelled"));
    }

    /// The pattern made, or cancelled: put down (`put_down_comp_array`).
    fn put_down(&mut self) {
        put_down_comp_array(self.pc);
    }

    /// An axis being picked is let go first.
    fn back(&mut self) -> Stepped {
        if !self.pc.arr.axis_pick {
            return Stepped::NothingLeft;
        }
        self.pc.arr.axis_pick = false;
        *self.pc.status = Notice::info(qymcad_i18n::tr("in-axis-pick-cancelled"));
        Stepped::Back
    }

    /// The fields follow the options - a second and a third direction, a full turn - and the counts follow the fields.
    fn after(&mut self) {
        sync_comp_array_params(*self.pc.arr, self.pc.carr.mode, self.pc.cmd);
        take_array_counts(self.pc.cmd, self.pc.arr);
    }
}

/// The catalogue keys of the field at a gizmo: a turn about an axis, or a shift along it - X, Y, Z.
const GIZMO_TURNS: [&str; 3] = ["giz-rotation-x", "giz-rotation-y", "giz-rotation-z"];
const GIZMO_SHIFTS: [&str; 3] = ["giz-offset-x", "giz-offset-y", "giz-offset-z"];

/// EXACT NUMERIC INPUT AT THE GIZMO of a body or a part: a formula at the geometry - millimetres along an axis or degrees
/// about it - and Apply makes a PARAMETRIC move.
struct GizmoTool<'a, 'b> {
    pc: &'a mut qymcad_ui_state::PartCtx<'b>,
    on: qymcad_ui_state::GizmoOf,
    step: GizmoStep,
    /// what the frame kept of the driver typed
    asks: Vec<qymcad_ui_state::FieldAsk>,
}

impl FramedTool for GizmoTool<'_, '_> {
    fn describe(&mut self, _ctx: &egui::Context) -> Described {
        let pc = &mut *self.pc;
        let (o, _) = match self.on {
            qymcad_ui_state::GizmoOf::Body(mi) => qymcad_ui_state::body_gizmo_geometry(&*pc.body_giz, *pc.cam, &*pc.project, pc.set, mi),
            qymcad_ui_state::GizmoOf::Part(comp) => qymcad_ui_state::gizmo_geometry(*pc.cam, *pc.comp_giz, &*pc.project, comp),
        };
        self.step.at = o;
        // zero moves nothing
        if pc.body_giz.num_field.is_empty() {
            let label = if self.step.rot { GIZMO_TURNS } else { GIZMO_SHIFTS }[(self.step.ax as usize).min(2)];
            let mut field = qymcad_ui_state::CmdParam::new(label, "giz", 0.0, -100000.0, 100000.0).nonzero();
            field.txt = String::new();
            pc.body_giz.num_field = vec![field];
        }
        let icon = if self.step.rot { ph::ARROWS_CLOCKWISE } else { ph::ARROWS_OUT_CARDINAL };
        let name = qymcad_i18n::tr(if self.step.rot { "tool-rotate" } else { "tool-move" });
        let head = qymcad_ui_state::Head { icon, name, ends: qymcad_ui_state::Ends::ApplyCancel, after: qymcad_ui_state::AfterApply::Select };
        // its gizmo is kept clear of as every one (`keep_in_sight`)
        let mut spec = qymcad_ui_state::BoxSpec::by(qymcad_ui_state::Place::by(qymcad_ui_state::By::Point(o)));
        spec.focus = qymcad_ui_state::Focus::take(&mut pc.body_giz.num_focus);
        // the move typed is named as every value of a box is; no node holds it by a name, its driver is a parameter
        spec.names = vec![qymcad_ui_state::FieldName::of("giz", String::new(), None)];
        Described { head, bar: Vec::new(), gate: qymcad_ui_state::Gate::Open, says: Vec::new(), boxed: Some(spec) }
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.pc.body_giz.num_field
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.pc.draw()
    }

    fn apply(&mut self) -> Result<(), NotApplied> {
        let by = self.pc.body_giz.num_field.first().map_or(0.0, |p| p.val);
        gizmo_move(self.pc, self.on, GizmoStep { by, ..self.step });
        if self.asks.iter().any(|a| !a.name.trim().is_empty()) {
            qymcad_ui_state::begin_edit(self.pc.edits, self.pc.project, qymcad_i18n::tr("sk-driver-step"));
            qymcad_ui_state::name_free_values(self.pc.project, &self.asks, &["giz"]);
            qymcad_ui_state::commit_edit_if_changed(&mut self.pc.rebuild());
        }
        Ok(())
    }

    fn take_asks(&mut self, asks: Vec<qymcad_ui_state::FieldAsk>) {
        self.asks = asks;
    }

    /// The move typed is let go: nothing was made on the way.
    fn cancel(&mut self) {
        self.put_down();
    }

    fn put_down(&mut self) {
        self.pc.body_giz.num = None;
        self.pc.body_giz.num_field.clear();
    }
}

/// THE TOOLS OF THE PLACE OF THE SECTION: the measure, the wait for a sketch's plane, the pencil, the mirror copy of a
/// part, the section.
struct PlaneTools<'a, 'b> {
    pb: &'a mut PlaneBarCtx<'b>,
    tool: PlaneTool,
}

impl FramedTool for PlaneTools<'_, '_> {
    /// The section has its offset and its two tilts typed at its plane, formulas as every field of a tool; its arrow is
    /// kept clear of as every handle (`keep_in_sight`).
    fn describe(&mut self, _ctx: &egui::Context) -> Described {
        let mut d = plane_tool_described(self.pb, &self.tool);
        if self.tool == PlaneTool::Section {
            if let Some((o, _)) = qymcad_ui_state::section_eff(self.pb.section) {
                section_fields_follow(self.pb.section);
                let mut spec = qymcad_ui_state::BoxSpec::by(qymcad_ui_state::Place::by(qymcad_ui_state::By::Point(o)));
                spec.layout = section_layout();
                d.boxed = Some(spec);
            }
        }
        d
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.pb.section.fields
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.pb.draw()
    }

    fn took(&mut self, did: Did) {
        if matches!(did, Did::Pressed { key } if key.name == "flip") {
            section_flip(self.pb.section);
        }
    }

    fn apply(&mut self) -> Result<(), NotApplied> {
        let pb = &mut *self.pb;
        match self.tool {
            PlaneTool::MirrorPart => mirror_part_apply(pb.mirror, pb.project, pb.edits, pb.regen, pb.active_path, pb.status),
            PlaneTool::Section => {
                *pb.status = Notice::info(qymcad_i18n::tr("vp-section-on"));
                Ok(())
            }
            // what is pointed at, not applied
            PlaneTool::Measure | PlaneTool::SketchFor(_) | PlaneTool::Pencil => Ok(()),
        }
    }

    /// The mirrored copy made lets go of its part and plane; the section placed stays on, no longer in hand.
    fn put_down(&mut self) {
        match self.tool {
            PlaneTool::MirrorPart => self.pb.mirror.drop_part(),
            PlaneTool::Section => self.pb.section.pick = false,
            PlaneTool::Measure | PlaneTool::SketchFor(_) | PlaneTool::Pencil => {}
        }
    }

    fn cancel(&mut self) {
        let pb = &mut *self.pb;
        match self.tool {
            PlaneTool::Measure => {
                pb.m3.clear();
                *pb.status = Notice::info(qymcad_i18n::tr("in-measure-off"));
            }
            PlaneTool::SketchFor(_) | PlaneTool::Pencil => {
                *pb.status = Notice::info(pb.picking.cancel_key().map(qymcad_i18n::tr).unwrap_or_default());
                pb.picking.clear();
            }
            PlaneTool::MirrorPart => pb.mirror.drop_part(),
            // the status line said "the section is on" after it was switched off here: it says so now
            PlaneTool::Section => {
                if pb.section.pick {
                    section_cancel(pb.section, pb.regen);
                } else {
                    pb.section.plane = None;
                    qymcad_ui_state::invalidate(pb.regen);
                }
                *pb.status = Notice::info(qymcad_i18n::tr("tb-section-off"));
            }
        }
    }

    /// The measure lets go of what was clicked first, and only with nothing clicked leaves: things are usually measured
    /// several times in a row, and a miss does not throw one out of measuring. A section once on is no tool in hand:
    /// Esc there goes past it, to what Esc does with nothing in hand, and leaves the section on.
    fn back(&mut self) -> Stepped {
        let pb = &mut *self.pb;
        match self.tool {
            PlaneTool::Measure if !pb.m3.picks.is_empty() => {
                pb.m3.picks.clear();
                *pb.status = Notice::info(qymcad_i18n::tr("in-measure-hint"));
                Stepped::Back
            }
            PlaneTool::Section if !pb.section.pick => Stepped::Back,
            _ => Stepped::NothingLeft,
        }
    }

    /// The section takes the values typed into its fields.
    fn after(&mut self) {
        if self.tool == PlaneTool::Section {
            section_takes_fields(self.pb);
        }
    }
}
