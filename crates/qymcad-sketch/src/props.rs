//! THE PROPERTIES OF A SKETCH in the right panel.

use qymcad_ui_state::Notice;
use super::*;

pub fn sketch_props(pr: &mut qymcad_ui_state::PropsCtx, ui: &mut egui::Ui, si: usize) {
    let lin = qymcad_ui_state::lineage_of(&*pr.project, Some(pr.project.sketches[si].id));
    if let Some(n) = qymcad_ui_state::props_header(ui, ph::POLYGON, "sk-props", qymcad_ui_state::NameSlot::Editable(pr.project.sketches[si].name.clone()), &lin) {
        pr.project.sketches[si].name = n;
    }
    // the plane the sketch is placed on
    let pl = sketch_plane_words(&*pr.project, pr.project.sketches[si].plane);
    ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-on", "what", &pl)).weak().small());
    let cids = pr.project.sketches[si].contour_ids.clone();
    let src = pr.project.sketches[si].source;
    ui.label(qymcad_i18n::tr1("sk-contours-n", "n", &cids.len().to_string()));
    if let Some(srcid) = src {
        if let Some(sf) = pr.project.sources.iter().find(|x| x.id == srcid) {
            ui.label(egui::RichText::new(qymcad_i18n::tr2("sk-source", "name", &sf.name, "kb", &qymcad_i18n::num(sf.data.len() as f64 / 1024.0, 1))).weak().small());
        }
    }

    // --- while editing: a clean panel, with no clutter ---
    if qymcad_ui_state::edit_si(&*pr.project, &*pr.sketch_ses) == Some(si) {
        use qymcad_core::model::Constraint;
        // system points (the origin and the axes) and their `Fixed` constraints are hidden from the list
        // and the counter, and are never deleted
        let sys_pts: std::collections::HashSet<Id> = pr.project.sketches[si].system_ids().into_iter().collect();
        let is_sys = |c: &Constraint| matches!(c, Constraint::Fixed { p } if sys_pts.contains(p));
        let np = pr.project.sketches[si].points.iter().filter(|p| !sys_pts.contains(&p.id)).count();
        let nc = pr.project.sketches[si].constraints.iter().filter(|c| !is_sys(c)).count();
        ui.label(egui::RichText::new(qymcad_i18n::tr2("sk-counts", "np", &np.to_string(), "nc", &nc.to_string())).weak().small());
        // the degrees-of-freedom readout (the rank of the Jacobian, THROUGH THE CACHE: it is not computed
        // every frame)
        let diag = qymcad_ui_state::sketch_diag(&*pr.cache, &*pr.project, si);
        let (dof, redun) = diag.dof;
        let conflicts = diag.conflicts.len();
        let (dline, dcol) = sketch_dof_line(&*pr.cache, &*pr.project, pr.scheme, si);
        ui.label(egui::RichText::new(dline).color(dcol).small());
        if conflicts > 0 {
            // AN HONEST WORDING: it is A SET of constraints that conflicts, and no single one in it is
            // "the culprit" - any of them can be removed. It used to say that the dimensions contradicted
            // the geometry, although geometric constraints can conflict too, and the geometry has nothing
            // to do with it: it stands where the compromise between incompatible constraints put it.
            qymcad_ui_state::say(ui, &pr.scheme.pal, &qymcad_ui_state::Notice::error(qymcad_i18n::tr1("sk-conflicts-n", "n", &conflicts.to_string())), qymcad_ui_state::NoticeSize::Small);
            ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-conflict-advice", "icon", ph::RULER)).weak().small());
        } else if redun > 0 {
            ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-redundant-n", "n", &redun.to_string())).color(pr.scheme.pal.note()).small());
        }
        if dof > 0 {
            ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-dof-n", "n", &dof.to_string())).weak().small());
        }
        // the parts of the sketch its last solve had no time for (`Sketch::left_unsolved`)
        let left = pr.project.sketches[si].left_unsolved;
        if left > 0 {
            ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-unsolved-left", "n", &left.to_string())).color(pr.scheme.pal.note()).small());
        }
        ui.separator();
        ui.checkbox(&mut pr.win.constraints, qymcad_i18n::tr("sk-show-constraints")).on_hover_text(qymcad_i18n::tr("sk-show-constraints-hint"));
        ui.separator();
        // the parameters of the project (the named dimensions and formulas)
        if ui.button(format!("{} {}", ph::FUNCTION, qymcad_i18n::tr("sk-params-btn"))).on_hover_text(qymcad_i18n::tr("sk-params-hint")).clicked() {
            pr.win.open(WinKind::Params);
        }
        // stitching coincident points cures a corner that has fallen apart on shapes already drawn
        if ui.button(format!("{} {}", ph::LINK, qymcad_i18n::tr("sk-stitch-btn"))).on_hover_text(qymcad_i18n::tr("sk-merge-ends-hint")).clicked() {
            let tol = (10.0 / pr.view.scale as f64).clamp(1e-4, 1.0);
            let n = pr.project.merge_close_points(si, tol);
            pr.project.solve_sketch(si);
            qymcad_ui_state::invalidate(&mut *pr.regen);
            *pr.status = Notice::info(if n > 0 { qymcad_i18n::tr1("sk-stitched-n", "n", &n.to_string()) } else { qymcad_i18n::tr("sk-no-coincident-points") });
        }
        ui.label(egui::RichText::new(qymcad_i18n::tr("sk-welcome")).weak().small());
        ui.separator();
        ui.label(egui::RichText::new(qymcad_i18n::tr("sk-constraints-and-dims")).strong());
        ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-list-hint", "icon", ph::TRASH)).weak().small());
        if nc == 0 {
            ui.label(egui::RichText::new(qymcad_i18n::tr("sk-not-yet")).weak().small());
        }
        pr.hover.constraint = None;
        let mut cons = pr.project.sketches[si].constraints.clone();
        // REDUNDANT constraints (which ones exactly over-define the sketch) are marked in the list as
        // candidates for removal. BUT the harmless redundancy of CONSISTENT dimensions (reference ones,
        // not an error) is NOT painted red - that would be a false alarm. The warning appears only on
        // (a) a redundant GEOMETRIC constraint, which does need removing, and (b) a conflicting
        // dimension. They form an interdependent group: removing any marked one lifts the over-definition.
        let conflict_set = diag.conflicts.clone();
        let cur_sel = pr.gsel.constraint;
        let (mut rm, mut changed, mut sel_click, mut hov) = (None, false, None, None);
        let mut to_driven: Option<usize> = None;
        // Does the sketch contain fillets (structural tangencies)? Their Jacobian at the point of contact
        // is degenerate (parallel to the intrinsic of the arc), so the rank analysis falsely marks as
        // redundant not only the tangencies themselves but the constraints ENTANGLED with them (the
        // horizontals and verticals of a rectangle, the `PointOnLine` of a virtual corner). While fillets
        // are present, rank redundancy of GEOMETRIC constraints is unreliable, so none of them is painted
        // red (real contradictions of values are still caught by `sketch_conflicts`, which works on the
        // geometry and is reliable).
        let flagged = qymcad_ui_state::flagged_redundant(&*pr.cache, &*pr.project, si); // ONE rule for the list and for the canvas
        let holders = pr.project.sketches[si].corner_holders();
        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            for (ci, c) in cons.iter_mut().enumerate() {
                if is_sys(c) {
                    continue; // a system `Fixed` on the origin or an axis is not shown
                }
                if matches!(c, Constraint::Orientation { .. }) || holders.contains(&ci) {
                    continue; // the turn of a rectangle is the rectangle's own, and a cut corner's holders are the corner's
                }
                let is_sel = cur_sel == Some(ci);
                // is it a dimension? (consistent redundancy among dimensions is harmless and gets no warning)
                let is_dim = matches!(
                    c,
                    Constraint::Distance { .. }
                        | Constraint::Angle { .. }
                        | Constraint::DistancePL { .. }
                        | Constraint::AngleLines { .. }
                        | Constraint::ArcLength { .. }
                        | Constraint::Diameter { .. }
                        | Constraint::EdgeDistance { .. }
                );
                // which redundant ones to mark is decided by ONE rule, shared with the canvas glyphs
                let redun_geom = flagged.contains(&ci);
                // A CONFLICTING constraint gets a separate, sharper mark: it is an error (the sketch does
                // not solve), whereas redundancy only means "this one may be removed". Both used to give
                // the same glyph.
                let in_conflict = conflict_set.contains(&ci);
                let is_driven = c.is_driven();
                let flag = redun_geom || in_conflict;
                let row = ui
                    .horizontal(|ui| {
                        if in_conflict {
                            qymcad_ui_state::tone_mark(ui, &pr.scheme.pal, qymcad_ui_state::Tone::Error).on_hover_text(qymcad_i18n::tr("sk-conflict-hint"));
                        } else if flag {
                            qymcad_ui_state::tone_mark(ui, &pr.scheme.pal, qymcad_ui_state::Tone::Warning).on_hover_text(qymcad_i18n::tr("sk-overdefined-hint"));
                        }
                        match c {
                            Constraint::Distance { d, .. } => {
                                if ui.selectable_label(is_sel, qymcad_i18n::tr("sk-dim")).clicked() {
                                    sel_click = Some(ci);
                                }
                                // a dimension standing past the field is shown as it stands: put into the range on every frame, it
                                // was written and solved as an edit, and the window never came to rest
                                changed |= ui.add(egui::DragValue::new(d).speed(0.2).range(0.01..=100000.0).clamp_existing_to_range(false).suffix(qymcad_i18n::tr("unit-mm-suffix"))).changed();
                            }
                            Constraint::Angle { deg, .. } => {
                                if ui.selectable_label(is_sel, qymcad_i18n::tr("sk-angle")).clicked() {
                                    sel_click = Some(ci);
                                }
                                changed |= ui.add(egui::DragValue::new(deg).speed(0.5).range(0.1..=359.9).clamp_existing_to_range(false).suffix("°")).changed();
                            }
                            other => {
                                // THE PARTICIPANTS IN THE ROW: "Horizontal: Line 3". Without them a list of
                                // thirty constraints shows that constraints exist but gives no way to find the
                                // one wanted - four rows of "Horizontal" in a row are indistinguishable.
                                let parts =
                                    constraint_parts(&qymcad_ui_state::DrawCtx { cam: pr.cam, set: &*pr.set, scheme: pr.scheme, project: &*pr.project, active_path: pr.active_path }, si, other);
                                let text = if parts.is_empty() { qymcad_ui_state::constraint_label(other) } else { format!("{}: {}", qymcad_ui_state::constraint_label(other), parts.join(", ")) };
                                if ui.selectable_label(is_sel, text).clicked() {
                                    sel_click = Some(ci);
                                }
                            }
                        }
                        // RESOLVE A CONFLICT IN ONE CLICK: the conflicting dimension becomes a driven one - it
                        // stops driving the geometry but stays on the drawing and shows the actual value. That
                        // is the standard way out in a professional CAD; without it the only option left is to
                        // delete the dimension.
                        if in_conflict && is_dim && !is_driven && ui.small_button(ph::RULER).on_hover_text(qymcad_i18n::tr("sk-make-driven-hint")).clicked() {
                            to_driven = Some(ci);
                        }
                        if ui.small_button(ph::TRASH).on_hover_text(qymcad_i18n::tr("sk-delete")).clicked() {
                            rm = Some(ci);
                        }
                    })
                    .response;
                if ui.rect_contains_pointer(row.rect) {
                    hov = Some(ci);
                }
            }
        });
        pr.hover.constraint = hov;
        if let Some(ci) = sel_click {
            pr.gsel.constraint = Some(ci);
        }
        if changed {
            pr.project.sketches[si].constraints = cons;
            pr.project.solve_sketch(si);
            qymcad_ui_state::invalidate(&mut *pr.regen);
        }
        if let Some(ci) = to_driven {
            make_dim_driven(&mut *pr.project, &mut *pr.regen, &mut *pr.status, si, ci);
        }
        if let Some(ci) = rm {
            pr.project.delete_sketch_constraint(si, ci);
            pr.gsel.constraint = None;
            qymcad_ui_state::invalidate(&mut *pr.regen);
        }
        return;
    }

    // NOT in edit mode: only what is relevant. A compact summary plus a way into editing. Geometry,
    // dimensions and constraints are edited INSIDE the sketch (through Edit or a double click). A body is
    // made from a sketch by the Extrude or Revolve command on the toolbar - a parametric feature in the
    // timeline - rather than by one-off buttons here.
    let (dline, dcol) = sketch_dof_line(&*pr.cache, &*pr.project, pr.scheme, si);
    ui.separator();
    ui.label(egui::RichText::new(dline).color(dcol).small());
    ui.add_space(2.0);
    if ui.button(format!("{} {}", ph::PENCIL_SIMPLE, qymcad_i18n::tr("sk-edit-btn"))).clicked() {
        pr.ask.push(qymcad_ui_state::PropsAsk::EnterSketch(si));
    }
    ui.label(egui::RichText::new(qymcad_i18n::tr("sk-body-from-sketch")).weak().small());

    ui.separator();
    if ui.button(format!("{} {}", ph::TRASH, qymcad_i18n::tr("act-delete-sketch"))).clicked() {
        qymcad_ui_state::ask_delete(&mut *pr.deferred, qymcad_ui_state::Sel::Sketch(si));
        // the same path the tree takes - the question and the deletion
    }
}
