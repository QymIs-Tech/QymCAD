//! THE PROPERTIES PANEL: what a feature, a contour, a plane or a mate IS, rather than how it is edited.
//!
//! Editing lives in the command - the top bar and the popup at the geometry - and that is where the preview,
//! the cancel and the formulas are. This side answers "what is this", "what does it stand on", "who depends
//! on it" and "is it all right".

use super::*;


/// The name of a component, for a row of the panel. It used to be a closure taking `&Self` - the whole
/// application to reach one field - and that alone kept the panel inside the crate declaring `App`.
fn comp_name(project: &qymcad_core::model::Project, id: Id) -> String {
    project.components.iter().find(|c| c.id == id).map(|c| crate::i18n::name(&c.name)).unwrap_or_default()
}

impl App {
    /// The properties column, through its own doorway.
    pub(crate) fn properties_panel(&mut self, ui: &mut egui::Ui) {
        let mut asks = Vec::new();
        crate::gui::panels_props::properties_panel(&mut self.props_ctx(&mut asks), ui);
        self.do_props_asks(asks);
    }








}

pub(crate) fn contour_props(ed: qymcad_ui_state::Editing, array: &mut ArrayTool, boolean: &mut BoolCommand, deferred: &mut DeferredUi, set: &mut Settings, ui: &mut egui::Ui, i: usize) {
    let lin = crate::gui::props_card::lineage_of(ed.project, ed.project.contour_id(i));
    props_header(ui, ph::POLYGON, "props-contour", NameSlot::None, &lin);
    let (closed, npts, area, bb, centroid) = {
        let c = &ed.project.contours[i];
        (c.closed, c.points.len(), c.area(), c.bbox(), c.centroid())
    };
    ui.label(crate::i18n::trn("cp-summary", &[("state", &if closed { crate::i18n::tr("cp-closed") } else { crate::i18n::tr("cp-open") }), ("n", &npts.to_string()), ("area", &crate::i18n::num(area, 1))]));
    ui.separator();

    // 2D edits: they create copies of the contour (offset, mirror, pattern)
    egui::CollapsingHeader::new(crate::i18n::tr("cp-edits")).id_salt(("edits", i)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("cp-offset-mm"));
            ui.add(egui::DragValue::new(&mut set.defaults.offset_2d).speed(0.2).range(0.1..=500.0));
        });
        ui.horizontal(|ui| {
            let d = set.defaults.offset_2d;
            if ui.button(crate::i18n::tr("cp-outwards")).clicked() {
                let res = qymcad_core::offset::offset_to_side(&ed.project.contours[i], d, true);
                ed.project.add_contours(res);
                qymcad_ui_state::invalidate(ed.regen);
            }
            if ui.button(crate::i18n::tr("cp-inwards")).clicked() {
                let res = qymcad_core::offset::offset_to_side(&ed.project.contours[i], d, false);
                ed.project.add_contours(res);
                qymcad_ui_state::invalidate(ed.regen);
            }
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("cp-mirror"));
            if ui.button(crate::i18n::tr("cp-by-x")).clicked() {
                let mut c = ed.project.contours[i].clone();
                c.mirror(true, centroid.x);
                ed.project.add_contour(c);
                qymcad_ui_state::invalidate(ed.regen);
            }
            if ui.button(crate::i18n::tr("cp-by-y")).clicked() {
                let mut c = ed.project.contours[i].clone();
                c.mirror(false, centroid.y);
                ed.project.add_contour(c);
                qymcad_ui_state::invalidate(ed.regen);
            }
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("cp-array-n"));
            ui.add(egui::DragValue::new(&mut array.n).range(2..=100));
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("cp-step-xy"));
            ui.add(egui::DragValue::new(&mut array.dx).speed(0.5));
            ui.add(egui::DragValue::new(&mut array.dy).speed(0.5));
        });
        if ui.button(crate::i18n::tr("cp-make-array")).clicked() {
            let base = ed.project.contours[i].clone();
            for k in 1..array.n {
                let mut c = base.clone();
                c.translate(array.dx * k as f64, array.dy * k as f64);
                ed.project.add_contour(c);
            }
            qymcad_ui_state::invalidate(ed.regen);
        }
        // a 2D boolean (trim or region) against another contour
        let nc = ed.project.contours.len();
        if nc >= 2 && ed.project.contours[i].closed {
            ui.separator();
            if boolean.other2d == i || boolean.other2d >= nc {
                boolean.other2d = (i + 1) % nc;
            }
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("cp-with-contour"));
                ui.add(egui::DragValue::new(&mut boolean.other2d).range(1..=nc).custom_formatter(|n, _| format!("{}", n as usize + 1)));
            });
            ui.horizontal(|ui| {
                let ops: [(&str, u8); 3] = [(&crate::i18n::tr("cp-cut"), 0), (&crate::i18n::tr("cp-union"), 1), (&crate::i18n::tr("cp-intersect"), 2)];
                for (lbl, op) in ops {
                    if ui.button(lbl).clicked() {
                        let oi = boolean.other2d.min(nc - 1);
                        if oi != i && ed.project.contours[oi].closed {
                            let res = qymcad_core::offset::boolean_contours(&ed.project.contours[i], &ed.project.contours[oi], op);
                            let cnt = res.len();
                            ed.project.add_contours(res);
                            *ed.status = crate::i18n::tr1("cp-bool-result", "n", &cnt.to_string());
                            qymcad_ui_state::invalidate(ed.regen);
                        }
                    }
                }
            });
        }
    });
    ui.separator();

    // moving by the minimum corner
    if let Some(bb) = bb {
        ui.label(crate::i18n::tr("cp-position-mm"));
        let (mut nx, mut ny) = (bb.min.x, bb.min.y);
        let (mut dx, mut dy) = (0.0, 0.0);
        egui::Grid::new(("cpos", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            ui.label("X");
            if ui.add(egui::DragValue::new(&mut nx).speed(1.0)).changed() {
                dx = nx - bb.min.x;
            }
            ui.end_row();
            ui.label("Y");
            if ui.add(egui::DragValue::new(&mut ny).speed(1.0)).changed() {
                dy = ny - bb.min.y;
            }
            ui.end_row();
        });
        if dx != 0.0 || dy != 0.0 {
            if let Some(c) = ed.project.contours.get_mut(i) {
                c.translate(dx, dy);
            }
            qymcad_ui_state::invalidate(ed.regen);
        }
    }

    ui.separator();
    ui.label(crate::i18n::tr("cp-rotation"));
    ui.horizontal(|ui| {
        for (lbl, ang) in [("CCW 90", 90.0), ("CCW 15", 15.0), ("CW 15", -15.0), ("CW 90", -90.0)] {
            if ui.button(lbl).clicked() {
                if let Some(c) = ed.project.contours.get_mut(i) {
                c.rotate(centroid, ang);
            }
                qymcad_ui_state::invalidate(ed.regen);
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("cp-scale"));
        if ui.button("× 1.1").clicked() {
            if let Some(c) = ed.project.contours.get_mut(i) {
                c.scale(centroid, 1.1);
            }
            qymcad_ui_state::invalidate(ed.regen);
        }
        if ui.button("÷ 1.1").clicked() {
            if let Some(c) = ed.project.contours.get_mut(i) {
                c.scale(centroid, 1.0 / 1.1);
            }
            qymcad_ui_state::invalidate(ed.regen);
        }
    });

    ui.separator();
    if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("props-delete-contour"))).clicked() {
        qymcad_ui_state::ask_delete(deferred, Sel::Contour(i));
    }
}

/// The mates panel: create a mate between two parts of an assembly + a list with the parameters.
/// Editing one recomputes the positions.
pub(crate) fn joints_panel(pr: &mut qymcad_ui_state::PropsCtx, ui: &mut egui::Ui) {
    use qymcad_core::feature::{AnchorRef, JointKind, MateItem, MateState};
    ui.label(egui::RichText::new(crate::i18n::tr("jp-title")).strong());
    // a warning about a CONFLICT among the assembly mates (the solver did not converge, so the parts were left where they were).
    if pr.project.mates_conflict {
        ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr("jp-conflict"))).color(pr.scheme.pal.error()).small());
    }
    let ctx = qymcad_ui_state::current_ctx_id(pr.active_path, pr.project);
    let children = pr.project.component_children(ctx);

    if children.len() >= 2 {
        // CREATION HAPPENS THROUGH THE COMMAND ONLY. There used to be two A/B drop-downs, a kind selector and
        // a "create at the origins" button: the joint appeared instantly, with no geometry picked by a click,
        // no preview and no Esc. That contradicted the rule the whole rest of the interface is built on and
        // taught people to go the wrong way - while the "at the origins" method did not disappear at all, it
        // became a fourth kind of anchor inside the command itself.
        //
        // A button that STARTS a command is not the same as a button that creates an object: it leads into the
        // shared path rather than around it.
        if ui.button(format!("{}  {}", ph::MAGNET, crate::i18n::tr("jp-start-joint"))).on_hover_text(crate::i18n::tr("jp-start-joint-hint")).clicked() {
            pr.ask.push(qymcad_ui_state::PropsAsk::JointPick);
        }
    } else {
        ui.label(egui::RichText::new(crate::i18n::tr("jp-need-two-parts")).weak());
    }

    // grounding sits in the Mates panel itself (rather than a checkbox to be hunted for in the right panel).
    // A click on a part fixes or releases it as the root of the placement tree; the anchor icon means grounded
    // (the same glyph appears in the viewport).
    if !children.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new(crate::i18n::tr("jp-grounding")).strong());
        for &c in &children {
            let g = pr.project.is_grounded(c);
            let icon = if g { ph::ANCHOR } else { ph::ANCHOR_SIMPLE };
            if ui.selectable_label(g, format!("{icon} {}", comp_name(pr.project, c))).on_hover_text(crate::i18n::tr("jp-grounding-hint")).clicked() {
                pr.project.set_grounded(c, !g);
                qymcad_ui_state::mark_dirty_for_rebuild(&mut pr.rebuild()); // the document is marked; the scheduler does the computing
            }
        }
        // GROUNDING THAT DOES NOT HOLD IS SAID OUT LOUD.
        //
        // A part grounded INSIDE a moving subassembly travels with it: the word "grounded" has been said, yet
        // it has no world immobility. Measured on a real machine: one frame part was grounded inside the X beam
        // and travelled 100.000 mm along with the gantry. This is not forbidden - the arrangement is legitimate
        // when a unit is assembled on its own - but a person must know.
        let lying: Vec<String> = pr.project.grounded_inside_moving().into_iter().map(|c| comp_name(pr.project, c)).collect();
        if !lying.is_empty() {
            ui.label(egui::RichText::new(crate::i18n::tr1("jp-grounded-inside-moving", "list", &lying.join(", "))).color(pr.scheme.pal.hint()).small())
                .on_hover_text(crate::i18n::tr("jp-grounded-inside-moving-hint"));
        }
    }

    ui.separator();
    let ctx_now = qymcad_ui_state::current_ctx_id(pr.active_path, pr.project);
    // THE ANCHORS GET A LIST OF THEIR OWN. A connector became an element in its own right, and an element
    // that is not in a list does not exist for a person: it cannot be selected, corrected or removed.
    ui.horizontal(|ui| {
        if ui.button(format!("{}  {}", ph::CROSSHAIR, crate::i18n::tr("j-conn-new"))).on_hover_text(crate::i18n::tr("j-conn-new-hint")).clicked() {
            pr.ask.push(qymcad_ui_state::PropsAsk::ConnPick);
        }
    });
    let mut kill_conn: Option<Id> = None;
    let conns: Vec<(Id, Id, String, usize)> = pr
        .project
        .connectors
        .iter()
        // THE STANDALONE ONES ONLY. Anchors created FOR a joint are edited inside that joint and need no
        // row of their own: in an assembly with five joints the list would swell by ten rows nobody created.
        // Implicit connectors belong inside their own mate rather than beside it.
        .filter(|c| c.standalone && pr.project.component_is_within(c.owner, ctx_now))
        .map(|c| (c.id, c.owner, crate::i18n::name(&c.name), pr.project.connector_users(c.id).len()))
        .collect();
    for (cid, owner, cname, users) in conns {
        ui.horizontal(|ui| {
            let sel = *pr.sel_conn == Some(cid);
            if ui.selectable_label(sel, format!("{} {cname} — {}", ph::CROSSHAIR, comp_name(pr.project, owner))).clicked() {
                *pr.sel_conn = if sel { None } else { Some(cid) };
            }
            // HOW MANY JOINTS HANG ON IT - which is also the answer to why it cannot be deleted.
            if users > 0 {
                ui.label(egui::RichText::new(crate::i18n::tr1("j-conn-users", "n", &users.to_string())).weak().small());
            }
            if ui.small_button(ph::X).clicked() {
                kill_conn = Some(cid);
            }
        });
        if *pr.sel_conn == Some(cid) {
            qymcad_assembly::one_connector_controls(pr.joint, pr.project, ui, "", cid);
        }
    }
    if let Some(cid) = kill_conn {
        pr.ask.push(qymcad_ui_state::PropsAsk::DeleteConnector(cid));
        qymcad_ui_state::mark_dirty_for_rebuild(&mut pr.rebuild()); // the document is marked; the scheduler does the computing
    }
    // ONE MATE TIMELINE.
    //
    // There used to be THREE SEPARATE LOOPS - constraints, relations and joints - each with its own row, its
    // own delete button and its own idea of whether an element was sound. Three ideas about one thing drift
    // apart silently, and a person sees what is not there.
    //
    // The list comes from the core (`Project::mate_timeline`): the panel no longer decides what is in it or
    // what state that is in, and so it cannot decide differently from the solver.
    let ctx = qymcad_ui_state::current_ctx_id(pr.active_path, pr.project);
    let mut to_del: Option<(Id, MateItem)> = None;
    let mut changed = false;
    let at_limit = pr.project.joints_at_limit();
    let mut put_to_limit = false;
    for e in pr.project.mate_timeline(ctx) {
        // THE ICON GOES BY KIND, one scheme for the whole timeline.
        let icon = match e.item {
            MateItem::Joint => ph::MAGNET,
            MateItem::Relation => ph::GEAR_SIX,
            MateItem::Constraint => match e.kind_label {
                "constraint-kind-width" => ph::ARROWS_OUT_LINE_HORIZONTAL,
                "constraint-kind-tangent" => ph::CIRCLE_HALF_TILT,
                _ => ph::SELECTION_ALL,
            },
        };
        // A ROW IS CALLED BY ITS OWN NAME, with the kind and the parts beside it. The name is what the
        // element is called in conversation and searched for by; the kind is what it does. Hiding the name in
        // a tooltip took away the only way of telling one joint from another just like it.
        let parts: Vec<String> = e.touches.iter().map(|m| comp_name(pr.project, *m)).collect();
        let selected = matches!(e.item, MateItem::Joint) && *pr.sel == Sel::Joint(e.id);
        ui.horizontal(|ui| {
            let in_relation = pr.joint.relation_pick.as_ref().is_some_and(|p| p.picks.iter().any(|(id, _)| *id == e.id));
            let resp = ui.selectable_label(selected || in_relation, format!("{icon} {}", crate::i18n::name(&e.name)));
            ui.label(egui::RichText::new(format!("{}: {}", crate::i18n::tr(e.kind_label), parts.join(" <-> "))).weak().small());
            if resp.clicked() && matches!(e.item, MateItem::Joint) {
                // THE TOOL IN HAND OUTRANKS THE SELECTION: while a relation is being gathered, a click on a
                // joint means "take this degree of freedom" rather than "show this joint". Otherwise there
                // would be no way to point at mates at all - they are not geometry and cannot be picked in
                // the viewport.
                if pr.joint.relation_pick.is_some() {
                    pr.ask.push(qymcad_ui_state::PropsAsk::RelationPick(e.id));
                } else {
                    *pr.sel = Sel::Joint(e.id);
                }
            }
            if resp.hovered() && matches!(e.item, MateItem::Joint) {
                pr.hover.joint = Some(e.id);
            }
            // THE STATE IS SAID IN THE SAME WORDS FOR ALL THREE KINDS. While there was no such mark, dead
            // joints looked exactly like healthy ones in the list: a person sees a joint, the part does not
            // move, and there is no explanation.
            match e.state {
                MateState::Faulty(why) => {
                    ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr(why))).color(pr.scheme.pal.error_mild()).small())
                        .on_hover_text(crate::i18n::tr(&format!("{why}-hint")));
                }
                MateState::Violated => {
                    ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr("jp-dof-conflict"))).color(pr.scheme.pal.error()).small());
                }
                MateState::Ok => {}
            }
            // BEING AT A LIMIT IS NOT A FAULT, BUT IT HAS TO BE SAID. A limit clamps the given value
            // silently: 40 was typed, the part stopped at 20, and from the outside that is indistinguishable
            // from the program not listening.
            if at_limit.iter().any(|(id, _)| *id == e.id) {
                ui.label(egui::RichText::new(format!("{} {}", ph::ARROWS_IN_LINE_HORIZONTAL, crate::i18n::tr("jp-at-limit"))).color(pr.scheme.pal.hint()).small())
                    .on_hover_text(crate::i18n::tr("jp-at-limit-hint"));
            }
            // "GO TO THE LIMIT" appears only where a limit is set: a mechanism is inspected at its EXTREME
            // positions, and that is where it runs into its neighbour.
            if matches!(e.item, MateItem::Joint) {
                let bounds: Vec<(usize, bool)> = pr
                    .project
                    .joints
                    .iter()
                    .find(|x| x.id == e.id)
                    .map(|j| {
                        (0..3usize)
                            .flat_map(|s| [(s, false, j.limit_min[s]), (s, true, j.limit_max[s])])
                            .filter_map(|(s, up, b)| b.map(|_| (s, up)))
                            .collect()
                    })
                    .unwrap_or_default();
                if !bounds.is_empty() {
                    ui.menu_button(ph::ARROWS_IN_LINE_HORIZONTAL, |ui| {
                        for (slot, up) in bounds {
                            let what = crate::i18n::tr(if up { "jp-limit-upper" } else { "jp-limit-lower" });
                            if ui.button(&what).on_hover_text(crate::i18n::tr("jp-apply-limit-hint")).clicked() {
                                if pr.project.apply_limit_position(e.id, slot, up) {
                                    put_to_limit = true;
                                }
                                ui.close();
                            }
                        }
                    })
                    .response
                    .on_hover_text(crate::i18n::tr("jp-apply-limit"));
                }
            }
            if ui.small_button(ph::X).clicked() {
                to_del = Some((e.id, e.item));
            }
        });
        // A RELATION tells what it ties together and with what number - it has no motion of its own.
        if matches!(e.item, MateItem::Relation) {
            if let Some(r) = pr.project.relations.iter().find(|r| r.id == e.id) {
                let unit = crate::i18n::tr(if r.kind.value_is_per_turn() { "j-relation-per-turn" } else { "j-relation-ratio" });
                ui.label(egui::RichText::new(format!("{unit}: {}", r.value)).weak().small());
            }
        }
        let Some(j) = pr.project.joints.iter().find(|x| x.id == e.id).cloned() else { continue };
        let j = &j;
        let ob = pr.project.connector(j.b).map(|c| c.owner).unwrap_or(0);
        // changing a joint's KIND on the fly (the anchors are kept; an incompatibility between the kind and the anchors goes to the status line).
        {
            let mut newk = j.kind;
            egui::ComboBox::from_id_salt(("jkind", j.id)).selected_text(crate::i18n::tr(j.kind.label())).show_ui(ui, |ui| {
                ui.label(egui::RichText::new(crate::i18n::tr("jp-assembly-kind")).weak().small());
                for kk in [JointKind::Rigid, JointKind::Revolute, JointKind::Slider, JointKind::Cylindrical, JointKind::Planar, JointKind::Ball, JointKind::PinSlot, JointKind::Parallel] {
                    ui.selectable_value(&mut newk, kk, crate::i18n::tr(kk.label()));
                }
                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::tr("jp-mechanical-kind")).weak().small());
                for kk in [JointKind::Revolute, JointKind::Slider, JointKind::Cylindrical, JointKind::Planar, JointKind::Ball, JointKind::PinSlot, JointKind::Parallel] {
                    ui.selectable_value(&mut newk, kk, crate::i18n::tr(kk.label()));
                }
            });
            if newk != j.kind {
                changed |= qymcad_ui_state::change_joint_kind(pr.project, pr.status, j.id, newk);
            }
        }
        // a rigid face-to-face joint gets a SIDE toggle (coplanar or face-to-face, through a 180 degree turn)
        let face_rigid = matches!(j.kind, JointKind::Rigid)
            && pr.project.connector(j.a).is_some_and(|c| matches!(c.anchor, AnchorRef::FaceCenter(..)))
            && pr.project.connector(j.b).is_some_and(|c| matches!(c.anchor, AnchorRef::FaceCenter(..)));
        // a joint of a nested subassembly (whose home is not the root) can be lifted into the root for global control
        let nested = pr.project.joint_home(j).is_some_and(|h| h != pr.project.root);
        {
            if let Some(jj) = pr.project.joints.iter_mut().find(|x| x.id == j.id) {
                ui.horizontal(|ui| {
                    // THE SAME slot widget as in the popup at the glyph (joints.rs): a readout while the
                    // degree is free, a driver once one is set. There used to be a second copy of this editor
                    // here, and the two drifted apart silently.
                    if matches!(jj.kind, JointKind::Revolute | JointKind::Cylindrical | JointKind::PinSlot | JointKind::Ball | JointKind::Planar) {
                        changed |= super::joints::joint_slot_drag(ui, jj, 0, 1.0);
                    }
                    // an offset exists on Rigid as well (the face-to-face gap), not only on the sliding kinds.
                    // On Ball, offset and offset2 are the ANGLES rx and ry.
                    if matches!(jj.kind, JointKind::Rigid | JointKind::Slider | JointKind::Cylindrical | JointKind::PinSlot | JointKind::Planar | JointKind::Ball) {
                        changed |= super::joints::joint_slot_drag(ui, jj, 1, 0.5);
                    }
                    // the second freedom of Planar (the Y offset) or of Ball (the Y angle, ry).
                    if matches!(jj.kind, JointKind::Planar | JointKind::Ball) {
                        changed |= super::joints::joint_slot_drag(ui, jj, 2, 0.5);
                    }
                    // THE SIDE toggle. A coincidence always goes down the constraint path (the flip works
                    // through the directed residual na+nb, see mate_solver). A rigid face-to-face joint has a
                    // flip of its own down the tree (mate_min_target). Planar is NOT included: in a purely
                    // mechanical assembly it goes down the tree with no flip support, so the checkbox would be
                    // dead.
                    if face_rigid {
                        changed |= ui.checkbox(&mut jj.flip, crate::i18n::tr("jp-flip-side")).on_hover_text(crate::i18n::tr("jp-flip-side-hint")).changed();
                    }
                    if nested {
                        changed |= ui.checkbox(&mut jj.global, crate::i18n::tr("jp-drive-from-root")).on_hover_text(crate::i18n::tr("jp-drive-from-root-hint")).changed();
                    }
                });
            }
        }
        // the parametric angle and offset fields are expressions over the global variables (like sketch
        // dimensions): they are stored in feat_dims under the joint's id and evaluated in regenerate.
        if matches!(j.kind, JointKind::Revolute | JointKind::Cylindrical | JointKind::PinSlot | JointKind::Ball | JointKind::Planar) {
            qymcad_ui_state::dim_expr_field_in(&mut pr.rebuild(), ui, j.id, "angle", "");
        }
        if matches!(j.kind, JointKind::Rigid | JointKind::Slider | JointKind::Cylindrical | JointKind::PinSlot | JointKind::Planar | JointKind::Ball) {
            qymcad_ui_state::dim_expr_field_in(&mut pr.rebuild(), ui, j.id, "offset", "");
        }
        if matches!(j.kind, JointKind::Planar | JointKind::Ball) {
            qymcad_ui_state::dim_expr_field_in(&mut pr.rebuild(), ui, j.id, "offset2", "");
        }
        // limits (min and max) on a joint's free slots - a stop within a range.
        if let Some(jj) = pr.project.joints.iter_mut().find(|x| x.id == j.id) {
            let free = jj.kind.free_slots();
            if free.iter().any(|&f| f) {
                let mut lim_changed = false;
                egui::CollapsingHeader::new(crate::i18n::tr("jp-limits")).id_salt(("jlim", j.id)).default_open(false).show(ui, |ui| {
                    // THE SAME limits widget as in the popup at the glyph (joints.rs). The index is the
                    // slot's number and is passed on, so it is the value rather than a way in.
                    for (slot, _) in free.iter().enumerate().filter(|(_, f)| **f) {
                        lim_changed |= super::joints::joint_slot_limits(ui, jj, slot);
                    }
                });
                changed |= lim_changed;
            }
        }
        // the driven part's true degrees of freedom (counting THE WHOLE STACK of joints) + a status, as in a
        // sketch.
        //
        // AN ARGUING JOINT DEFINES NOTHING. The count of degrees is computed from the stack of joints and does
        // not ask whether they solve together: both arguing joints used to read "0 - fully defined", in green,
        // RIGHT BESIDE a red conflict warning. The program said "trouble" and "all in order" at the same time -
        // the worst answer of all.
        if pr.project.mates_violated.contains(&j.id) {
            ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr("jp-dof-conflict"))).color(pr.scheme.pal.error()).small());
        } else {
            let d = pr.project.component_dof(ob);
            let (lbl, col) = if d == 0 { (&crate::i18n::tr("jp-defined"), pr.scheme.pal.ok()) } else { (&crate::i18n::tr("jp-underdefined"), pr.scheme.pal.underdefined()) };
            ui.label(egui::RichText::new(crate::i18n::tr2("jp-dof", "d", &d.to_string(), "state", lbl)).color(col).small());
        }
    }
    if put_to_limit {
        // SETTING A VALUE MUST NOT SOIL THE DOCUMENT: without a solve the part would stay where it was while
        // the field showed the new number - exactly the divergence where the field says one thing and the part
        // another.
        pr.project.solve_joints();
        changed = true;
    }
    if let Some((id, item)) = to_del {
        // ONE CORE METHOD PER KIND. There used to be a copy of the cleanup here, and it counted orphans by the
        // joint list of THE CURRENT assembly: delete a joint in the root, and the connectors of every
        // subassembly's joints went with it while those joints stayed dead forever. That is how one document
        // ended up with five joints on two connectors.
        match item {
            MateItem::Joint => pr.project.delete_joint(id),
            MateItem::Constraint => pr.project.delete_group(id),
            MateItem::Relation => pr.project.delete_relation(id),
        }
        changed = true;
    }
    if changed {
        qymcad_ui_state::mark_dirty_for_rebuild(&mut pr.rebuild()); // the document is marked; the scheduler does the computing
    }
}

pub(crate) fn plane_props(pr: &mut qymcad_ui_state::PropsCtx, ui: &mut egui::Ui, i: usize) {
    use qymcad_core::feature::BasePlane;
    use qymcad_core::model::PlaneDef;
    let mut remove = false;
    let mut changed = false;
    let mut start_pick_face = false;
    let pid = pr.project.planes[i].id;
    let lin = crate::gui::props_card::lineage_of(pr.project, Some(pid));
    if let Some(n) = props_header(ui, ph::PROJECTOR_SCREEN, "pp-title", NameSlot::Editable(pr.project.planes[i].name.clone()), &lin) {
        pr.project.planes[i].name = n;
    }
    {
        let p = &mut pr.project.planes[i];
        // THE KIND of definition: manual, offset from a base plane, or offset from a face
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("pp-kind"));
            let is_offb = matches!(p.def, PlaneDef::OffsetBase { .. });
            let is_offf = matches!(p.def, PlaneDef::OffsetFace { .. });
            if ui.selectable_label(!is_offb && !is_offf, crate::i18n::tr("pp-manual")).clicked() && (is_offb || is_offf) {
                p.def = PlaneDef::Manual;
                changed = true;
            }
            if ui.selectable_label(is_offb, crate::i18n::tr("pp-from-plane")).clicked() && !is_offb {
                p.def = PlaneDef::OffsetBase { base: BasePlane::XY, dist: 20.0 };
                changed = true;
            }
            if ui.selectable_label(is_offf, crate::i18n::tr("pp-from-face")).clicked() {
                start_pick_face = true; // a face pick is needed (after this block)
            }
        });
    }
    if let PlaneDef::OffsetFace { body, face, mut dist } = pr.project.planes[i].def {
        let nm = pr.project.mesh_index(body).map(|mi| crate::i18n::name(&pr.project.mesh_name(mi))).unwrap_or_else(|| crate::i18n::tr1("pp-body-n", "b", &body.to_string()));
        ui.label(egui::RichText::new(crate::i18n::tr1("pp-from-body-face", "name", &nm)).weak().small());
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("pp-offset-mm"));
            changed |= ui.add(egui::DragValue::new(&mut dist).speed(0.5).range(-100000.0..=100000.0)).changed();
        });
        pr.project.planes[i].def = PlaneDef::OffsetFace { body, face, dist };
        qymcad_ui_state::dim_expr_field_in(&mut pr.rebuild(), ui, pid, "dist", "");
        if ui.button(format!("{} {}", ph::SELECTION_PLUS, crate::i18n::tr("props-pick-other-face"))).clicked() {
            start_pick_face = true;
        }
        let p = &pr.project.planes[i];
        ui.label(egui::RichText::new(crate::i18n::tr2("pp-origin-normal", "o", &format!("[{}, {}, {}]", crate::i18n::num(p.origin[0],1), crate::i18n::num(p.origin[1],1), crate::i18n::num(p.origin[2],1)), "n", &format!("[{}, {}, {}]", crate::i18n::num(p.normal[0],2), crate::i18n::num(p.normal[1],2), crate::i18n::num(p.normal[2],2)))).weak().small());
    } else if let PlaneDef::OffsetBase { mut base, mut dist } = pr.project.planes[i].def {
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("pp-from"));
            changed |= ui.selectable_value(&mut base, BasePlane::XY, "XY").changed();
            changed |= ui.selectable_value(&mut base, BasePlane::XZ, "XZ").changed();
            changed |= ui.selectable_value(&mut base, BasePlane::YZ, "YZ").changed();
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("pp-offset-mm"));
            changed |= ui.add(egui::DragValue::new(&mut dist).speed(0.5).range(-100000.0..=100000.0)).changed();
        });
        pr.project.planes[i].def = PlaneDef::OffsetBase { base, dist };
        qymcad_ui_state::dim_expr_field_in(&mut pr.rebuild(), ui, pid, "dist", ""); // the distance is an expression over the global variables (parametric)
        // the origin and normal are derived, so they are shown for information
        let p = &pr.project.planes[i];
        ui.label(egui::RichText::new(crate::i18n::tr2("pp-origin-normal", "o", &format!("[{}, {}, {}]", crate::i18n::num(p.origin[0],1), crate::i18n::num(p.origin[1],1), crate::i18n::num(p.origin[2],1)), "n", &format!("[{}, {}, {}]", crate::i18n::num(p.normal[0],2), crate::i18n::num(p.normal[1],2), crate::i18n::num(p.normal[2],2)))).weak().small());
    } else {
        // MANUAL: direct editors for the origin, the normal and the roll
        let p = &mut pr.project.planes[i];
        ui.label(crate::i18n::tr("pp-origin-mm"));
        egui::Grid::new(("plo", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            changed |= drag(ui, "X", &mut p.origin[0], 1.0, -10000.0..=10000.0);
            changed |= drag(ui, "Y", &mut p.origin[1], 1.0, -10000.0..=10000.0);
            changed |= drag(ui, "Z", &mut p.origin[2], 1.0, -10000.0..=10000.0);
        });
        ui.separator();
        ui.label(crate::i18n::tr("pp-normal"));
        egui::Grid::new(("pln", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            changed |= drag(ui, "Nx", &mut p.normal[0], 0.05, -1.0..=1.0);
            changed |= drag(ui, "Ny", &mut p.normal[1], 0.05, -1.0..=1.0);
            changed |= drag(ui, "Nz", &mut p.normal[2], 0.05, -1.0..=1.0);
        });
        let nl = (p.normal[0].powi(2) + p.normal[1].powi(2) + p.normal[2].powi(2)).sqrt();
        if nl > 1e-6 {
            for k in 0..3 {
                p.normal[k] /= nl;
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("pp-rot-about-normal"));
            changed |= ui.add(egui::DragValue::new(&mut p.rot_deg).speed(1.0).range(-180.0..=180.0).suffix("°")).changed();
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button(crate::i18n::tr("pp-table-xy")).clicked() {
                p.normal = [0.0, 0.0, 1.0];
                changed = true;
            }
            if ui.button("XZ").clicked() {
                p.normal = [0.0, 1.0, 0.0];
                changed = true;
            }
            if ui.button("YZ").clicked() {
                p.normal = [1.0, 0.0, 0.0];
                changed = true;
            }
        });
    }
    if changed {
        pr.datum.regen_pending = true; // the plane moved, so the consumers are rebuilt (debounced on release)
    }
    if start_pick_face {
        pr.picking.set_plane_face(Some(pid)); // reassign this plane's face
        *pr.mode_3d = true;
        *pr.status = crate::i18n::tr("pp-pick-face-hint");
    }
    ui.separator();
    // a sketch straight on this plane (drawn in its own frame)
    if ui.button(format!("{} {}", ph::PENCIL, crate::i18n::tr("props-new-sketch-here"))).clicked() {
        let pid = pr.project.planes[i].id;
        let pid = if pid == 0 {
            let id = pr.project.alloc_id();
            pr.project.planes[i].id = id;
            id
        } else {
            pid
        };
        pr.ask.push(qymcad_ui_state::PropsAsk::SketchOnDatum(pid));
        return;
    }
    if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("props-delete-plane"))).clicked() {
        remove = true;
    }
    if remove {
        qymcad_ui_state::ask_delete(pr.deferred, Sel::Plane(i)); // the removal itself is in `execute_delete`, by the same path the tree uses
    }
}

/// A FEATURE'S PROPERTIES ARE SHOWN, NOT EDITED.
///
/// There used to be 450 lines of editors here: a `DragValue` per parameter, applied INSTANTLY - with no
/// preview, no Enter/Esc and no expressions. That gave two ways of editing one feature with different
/// capabilities, and what was available depended on which way one had arrived at it. Editing lives in the
/// command (the top bar + the popup at the geometry): that is where the preview, the cancel and the formulas
/// are.
///
/// The panel answers the questions "what is this", "what does it stand on", "who depends on it" and "is it
/// all right" - and offers two buttons: edit and delete.
pub(crate) fn feature_props(pr: &mut qymcad_ui_state::PropsCtx, ui: &mut egui::Ui, ti: usize) {
    let Some(node) = pr.project.timeline.get(ti).cloned() else { return };
    let fid = node.id;
    // THE FEATURE'S KIND GOES IN THE HEADING and the node's name below it. The heading used to read
    // "feature properties" - the same for all forty kinds - and what was actually selected had to be read
    // from the line below.
    let lin = crate::gui::props_card::lineage_of(pr.project, Some(fid));
    props_header(ui, ph::STACK, &crate::gui::feat_default_name(&node.kind), NameSlot::Fixed(node.name.clone()), &lin);

    // THE STATE: a rebuild error and the rollback - the reasons a feature may fail to build
    if let Some(err) = pr.project.regen_errors.get(&fid) {
        ui.separator();
        let text = crate::gui::error_words::error_text(err);
        ui.label(egui::RichText::new(format!("{} {text}", ph::WARNING)).color(pr.scheme.pal.error_mild()).small());
    }
    if pr.project.rollback.is_some_and(|r| r <= ti) {
        ui.label(egui::RichText::new(crate::i18n::tr("fp-below-rollback")).weak().small());
    }

    ui.separator();
    ui.horizontal(|ui| {
        if ui.button(format!("{} {}", ph::PENCIL_SIMPLE, crate::i18n::tr("props-edit"))).on_hover_text(crate::i18n::tr("fp-edit-hint")).clicked() {
            pr.ask.push(qymcad_ui_state::PropsAsk::EditFeature(fid));
        }
        if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("props-delete"))).clicked() {
            qymcad_ui_state::ask_delete(pr.deferred, Sel::Feature(ti)); // ask and delete, by the same path the tree uses
        }
    });

    // A FEATURE'S NUMBERS LIVE HERE, AND EVERY ONE OF THEM CAN BECOME A DRIVER.
    //
    // What was asked for: features should have all of this, not only sketches. Before that there were no
    // numeric fields in a feature's properties at all: an extrude's height could only be corrected by
    // reopening the command, and there was nowhere at all to NAME it as a driver. The parameter list comes
    // from the same table the rebuild applies them from (`FeatureKind::dims`), so it cannot drift from it.
    let dims = node.kind.dims();
    if !dims.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new(crate::i18n::tr("fp-params")).strong());
        for (key, _) in dims {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(key).weak());
            });
            qymcad_ui_state::dim_expr_field_in(&mut pr.rebuild(), ui, fid, key, "");
        }
    }
}

pub(crate) fn properties_panel(pr: &mut qymcad_ui_state::PropsCtx, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(4.0);
        // the CAM properties (the machine, the stock, the tool, the setup, the operation) belong to the CAM workbench only
        match *pr.sel {
            Sel::Mesh(i) if i < pr.project.bodies.len() => crate::gui::mesh_props(&mut *pr.deferred, &mut *pr.project, &mut *pr.regen, ui, i),
            Sel::Face(mi, fi) if pr.project.bodies.get(mi).is_some_and(|b| fi < b.faces.len()) => crate::gui::face_props(&mut *pr.project, ui, mi, fi),
            Sel::Contour(i) if i < pr.project.contours.len() => crate::gui::panels_props::contour_props(qymcad_ui_state::editing_in!(pr), &mut *pr.array, &mut *pr.boolean, &mut *pr.deferred, &mut *pr.set, ui, i),
            Sel::Sketch(i) if i < pr.project.sketches.len() => crate::gui::sketching::sketch_props(pr, ui, i),
            Sel::Plane(i) if i < pr.project.planes.len() => plane_props(pr, ui, i),
            Sel::DatumPoint(i) if i < pr.project.datum_points.len() => crate::gui::datum_point_props(&mut *pr.datum, &mut *pr.deferred, &mut *pr.project, ui, i),
            Sel::DatumAxis(i) if i < pr.project.datum_axes.len() => datum_axis_props(pr, ui, i),
            Sel::Component(i) if i < pr.project.components.len() => component_props(pr, ui, i),
            Sel::Feature(i) if i < pr.project.timeline.len() => feature_props(pr, ui, i),
                                    _ => {
                ui.heading(crate::i18n::tr("props-title"));
                ui.label(egui::RichText::new(crate::i18n::tr("props-pick-in-tree")).weak());
                                        ui.separator();
                ui.label(egui::RichText::new(crate::i18n::tr("props-new-sketch-on")).strong());
                use qymcad_core::feature::BasePlane;
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::tr("plane-xy-table")).clicked() {
                        pr.ask.push(qymcad_ui_state::PropsAsk::SketchOnBasePlane(BasePlane::XY));
                    }
                    if ui.button(crate::i18n::tr("plane-xz-front")).clicked() {
                        pr.ask.push(qymcad_ui_state::PropsAsk::SketchOnBasePlane(BasePlane::XZ));
                    }
                    if ui.button(crate::i18n::tr("plane-yz-side")).clicked() {
                        pr.ask.push(qymcad_ui_state::PropsAsk::SketchOnBasePlane(BasePlane::YZ));
                    }
                });
            
            }
        }
        // the mates are always available in an Assembly context
        if matches!(pr.workbench, Workbench::Assembly) {
            ui.separator();
            joints_panel(pr, ui);
        }
    });
}

/// The properties of a component or a part: the name, whether it is active, its contents, deletion.
pub(crate) fn component_props(pr: &mut qymcad_ui_state::PropsCtx, ui: &mut egui::Ui, ci: usize) {
    use qymcad_core::feature::FeatureKind;
    let cid = pr.project.components[ci].id;
    let lin = crate::gui::props_card::lineage_of(&*pr.project, Some(cid));
    // The ROOT's name is only displayed: it is a catalogue key rather than the document's text (`migrate_root`).
    let slot = if cid == pr.project.root { NameSlot::Fixed(pr.project.components[ci].name.clone()) } else { NameSlot::Editable(pr.project.components[ci].name.clone()) };
    if let Some(n) = props_header(ui, ph::CUBE_TRANSPARENT, "props-component", slot, &lin) {
        pr.project.components[ci].name = n;
    }
    let active = qymcad_ui_state::current_ctx_id(pr.active_path, &*pr.project) == cid;
    if active {
        ui.label(egui::RichText::new(crate::i18n::tr("comp-active-context")).color(pr.scheme.pal.hint()));
        if pr.active_path.len() > 1 && ui.button(crate::i18n::tr("comp-go-up")).clicked() {
            pr.ask.push(qymcad_ui_state::PropsAsk::ExitContext);
        }
    } else {
        ui.label(egui::RichText::new(crate::i18n::tr("comp-not-active")).weak());
        if ui.button(format!("{} {}", ph::CUBE, crate::i18n::tr("props-enter-component"))).clicked() {
            pr.ask.push(qymcad_ui_state::PropsAsk::SetContext(cid));
        }
    }
    // the contents
    let (mut sk, mut ft) = (0, 0);
    for n in &pr.project.timeline {
        if n.parent == Some(cid) {
            // COUNTED BY WHETHER IT YIELDS A BODY rather than from a list of kinds: an enumeration silently
            // lost every new feature (the fillet, the chamfer, the shell, the split were all missing from the contents).
            if matches!(n.kind, FeatureKind::Sketch { .. }) {
                sk += 1;
            } else if !n.kind.bodies().is_empty() {
                ft += 1;
            }
        }
    }
    ui.label(crate::i18n::tr2("comp-counts", "sk", &sk.to_string(), "ft", &ft.to_string()));

    // placement inside an assembly: for non-root components only (the root is the document's frame).
    // It moves Component.transform (it does NOT rebuild the bodies); the render and the pick already account for it.
    if cid != pr.project.root {
        ui.separator();
        ui.label(egui::RichText::new(crate::i18n::tr("comp-placement")).strong());
        let mut t = pr.project.component_transform(cid);
        let mut moved = false;
        ui.horizontal(|ui| {
            ui.label("X");
            moved |= ui.add(egui::DragValue::new(&mut t[3]).speed(0.5).suffix(crate::i18n::tr("unit-mm-suffix"))).changed();
            ui.label("Y");
            moved |= ui.add(egui::DragValue::new(&mut t[7]).speed(0.5).suffix(crate::i18n::tr("unit-mm-suffix"))).changed();
            ui.label("Z");
            moved |= ui.add(egui::DragValue::new(&mut t[11]).speed(0.5).suffix(crate::i18n::tr("unit-mm-suffix"))).changed();
        });
        if moved {
            pr.project.set_component_transform(cid, t);
            qymcad_ui_state::after_placement_change(&mut pr.rebuild());
        }
        let mut rot = pr.opts.rot_deg;
        let mut rot_axis: Option<u8> = None;
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("comp-rotation"));
            ui.add(egui::DragValue::new(&mut rot).speed(1.0).suffix("°").range(-360.0..=360.0));
            if ui.button(format!("{}X", ph::ARROW_CLOCKWISE)).clicked() {
                rot_axis = Some(0);
            }
            if ui.button(format!("{}Y", ph::ARROW_CLOCKWISE)).clicked() {
                rot_axis = Some(1);
            }
            if ui.button(format!("{}Z", ph::ARROW_CLOCKWISE)).clicked() {
                rot_axis = Some(2);
            }
        });
        pr.opts.rot_deg = rot;
        if let Some(ax) = rot_axis {
            pr.project.rotate_component(cid, ax, rot);
            qymcad_ui_state::after_placement_change(&mut pr.rebuild());
        }
        let mut grounded = pr.project.is_grounded(cid);
        let mut reset = false;
        let mut g_changed = false;
        ui.horizontal(|ui| {
            if ui.button(crate::i18n::tr("comp-reset-placement")).clicked() {
                reset = true;
            }
            g_changed = ui.checkbox(&mut grounded, crate::i18n::tr("comp-grounded")).on_hover_text(crate::i18n::tr("comp-grounded-hint")).changed();
        });
        if reset {
            pr.project.set_component_transform(cid, qymcad_core::feature::PLACE_IDENTITY);
            qymcad_ui_state::after_placement_change(&mut pr.rebuild());
        }
        if g_changed {
            pr.project.set_grounded(cid, grounded);
        }
    }

    // This component's external (top-down) references: the list + breaking them
    let ext: Vec<(Id, Id)> = pr.project.external_refs.iter().filter(|r| r.from_component == cid).filter_map(|r| r.source_body().map(|b| (r.id, b))).collect();
    if !ext.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new(crate::i18n::tr("comp-external-refs")).strong());
        let mut del: Option<Id> = None;
        for (rid, body) in &ext {
            let on = pr.project.body_owner(*body).and_then(|o| pr.project.components.iter().find(|c| c.id == o)).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::tr1("comp-on-face-of", "name", &on)).small());
                if ui.small_button(ph::X).on_hover_text(crate::i18n::tr("comp-break-ref-hint")).clicked() {
                    del = Some(*rid);
                }
            });
        }
        if let Some(rid) = del {
            // not a raw deletion (which would leave a sketch on another part's face without authorisation, an
            // isolation error), but an honest break: the sketch planes are frozen into a copy in place
            let frozen = pr.project.break_external_ref(rid);
            qymcad_ui_state::mark_dirty_for_rebuild(&mut pr.rebuild()); // the document is marked; the scheduler does the computing
            *pr.status = crate::i18n::tr1("comp-ref-broken", "n", &frozen.to_string());
        }
    }

    ui.separator();
    // Deleting a part from an assembly removes it WHOLE (with everything inside it), with a confirmation. Its
    // builds are NOT spilled into the root: what is wanted is the part gone, not its nodes scattered.
    if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("props-delete-component"))).clicked() {
        pr.deferred.delete = Some(Sel::Component(ci));
    }
}

/// The properties of a datum AXIS: the name + the kind (manual or through two points) + an editor + delete.
pub(crate) fn datum_axis_props(pr: &mut qymcad_ui_state::PropsCtx, ui: &mut egui::Ui, i: usize) {
    use qymcad_core::model::AxisDef;
    let mut changed = false;
    let aid = pr.project.datum_axes[i].id;
    let lin = crate::gui::props_card::lineage_of(&*pr.project, Some(aid));
    if let Some(n) = props_header(ui, ph::LINE_SEGMENT, "props-datum-axis", NameSlot::Editable(pr.project.datum_axes[i].name.clone()), &lin) {
        pr.project.datum_axes[i].name = n;
    }
    // THE KIND: manual, or parametric through two datum points
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("pp-kind"));
        let is_2pt = matches!(pr.project.datum_axes[i].def, AxisDef::TwoPoints { .. });
        if ui.selectable_label(!is_2pt, crate::i18n::tr("pp-manual")).clicked() && is_2pt {
            // switching to manual mode FIXES the current coordinates as the definition
            let (o, d) = (pr.project.datum_axes[i].origin(), pr.project.datum_axes[i].dir());
            pr.project.datum_axes[i].set_manual(o, d);
            changed = true;
        }
        if ui.selectable_label(is_2pt, crate::i18n::tr("dax-two-points")).clicked() && !is_2pt {
            let pts: Vec<Id> = pr.project.datum_points.iter().map(|p| p.id).collect();
            pr.project.datum_axes[i].def = AxisDef::TwoPoints { a: pts.first().copied().unwrap_or(0), b: pts.get(1).copied().unwrap_or(0) };
            changed = true;
        }
    });
    if let AxisDef::TwoPoints { mut a, mut b } = pr.project.datum_axes[i].def {
        let pts: Vec<(Id, String)> = pr.project.datum_points.iter().map(|p| (p.id, crate::i18n::name(&p.name))).collect();
        if pts.len() < 2 {
            ui.label(egui::RichText::new(crate::i18n::tr("dax-need-points")).color(pr.scheme.pal.hint_action()).small());
        } else {
            let name_of = |id: Id| pts.iter().find(|(pid, _)| *pid == id).map(|(_, n)| n.clone()).unwrap_or_else(|| "—".into());
            ui.horizontal(|ui| {
                ui.label("A");
                egui::ComboBox::from_id_salt(("dax_a", aid)).selected_text(name_of(a)).show_ui(ui, |ui| {
                    for (pid, n) in &pts {
                        changed |= ui.selectable_value(&mut a, *pid, n).changed();
                    }
                });
                ui.label("B");
                egui::ComboBox::from_id_salt(("dax_b", aid)).selected_text(name_of(b)).show_ui(ui, |ui| {
                    for (pid, n) in &pts {
                        changed |= ui.selectable_value(&mut b, *pid, n).changed();
                    }
                });
            });
        }
        pr.project.datum_axes[i].def = AxisDef::TwoPoints { a, b };
        let d = &pr.project.datum_axes[i];
        ui.label(egui::RichText::new(crate::i18n::tr2("dax-origin-dir", "o", &format!("[{}, {}, {}]", crate::i18n::num(d.origin()[0],1), crate::i18n::num(d.origin()[1],1), crate::i18n::num(d.origin()[2],1)), "d", &format!("[{}, {}, {}]", crate::i18n::num(d.dir()[0],2), crate::i18n::num(d.dir()[1],2), crate::i18n::num(d.dir()[2],2)))).weak().small());
    } else {
        // a COPY is edited and the result put back through set_manual - the coordinates do not live beside a
        // parametric definition, they ARE the definition of a manual axis.
        let (mut o, mut dv) = (pr.project.datum_axes[i].origin(), pr.project.datum_axes[i].dir());
        ui.label(crate::i18n::tr("pp-origin-mm"));
        egui::Grid::new(("dao", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            changed |= drag(ui, "X", &mut o[0], 1.0, -100000.0..=100000.0);
            changed |= drag(ui, "Y", &mut o[1], 1.0, -100000.0..=100000.0);
            changed |= drag(ui, "Z", &mut o[2], 1.0, -100000.0..=100000.0);
        });
        ui.separator();
        ui.label(crate::i18n::tr("dax-direction"));
        egui::Grid::new(("dad", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            changed |= drag(ui, "Dx", &mut dv[0], 0.05, -1.0..=1.0);
            changed |= drag(ui, "Dy", &mut dv[1], 0.05, -1.0..=1.0);
            changed |= drag(ui, "Dz", &mut dv[2], 0.05, -1.0..=1.0);
        });
        let nl = (dv[0].powi(2) + dv[1].powi(2) + dv[2].powi(2)).sqrt();
        if nl > 1e-6 {
            for c in dv.iter_mut() {
                *c /= nl;
            }
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for (lbl, v) in [("X", [1.0, 0.0, 0.0]), ("Y", [0.0, 1.0, 0.0]), ("Z", [0.0, 0.0, 1.0])] {
                if ui.button(lbl).clicked() {
                    dv = v;
                    changed = true;
                }
            }
        });
        if changed {
            pr.project.datum_axes[i].set_manual(o, dv);
        }
    }
    if changed {
        pr.datum.regen_pending = true; // the axis moved, so the consumers are rebuilt (debounced on release)
    }
    ui.separator();
    if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("props-delete-axis"))).clicked() {
        qymcad_ui_state::ask_delete(&mut *pr.deferred, Sel::DatumAxis(i));
    }
}

