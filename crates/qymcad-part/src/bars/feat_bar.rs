//! The bar of a feature command: its fields, modes and buttons, tool by tool.

use qymcad_ui_state::Notice;
use super::*;

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
            25 => &qymcad_i18n::tr("f-push-face"),
            26 => &qymcad_i18n::tr("feat-name-remove-face"),
            27 => &qymcad_i18n::tr("feat-name-split-body"),
            28 => &qymcad_i18n::tr("feat-name-thicken"),
            29 => &qymcad_i18n::tr("feat-name-split-face"),
            30 => &qymcad_i18n::tr("feat-name-face-copy"),
            36 => &qymcad_i18n::tr("feat-name-offset-surface"),
            31 => &qymcad_i18n::tr("f-surface-replace"),
            32 => &qymcad_i18n::tr("feat-name-patch"),
            33 => &qymcad_i18n::tr("f-stitch"),
            34 => &qymcad_i18n::tr("feat-name-trim"),
            35 => &qymcad_i18n::tr("f-recognise"),
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
                    // THE BUTTON SHOWS AND TURNS THE DIRECTION IN FORCE. A cut on a face goes into the body by itself,
                    // worked out only at Enter, so the button stood released over a cut already turned into the body
                    // and its first press turned it... into the body. Reported behaviour: a square on the top cut 5
                    // up, away from the block, still took 500 mm^3 out of it.
                    if pc.feat.flip_auto {
                        if let Some(si) = pc.cmd.sketch.filter(|i| *i < pc.project.sketches.len()) {
                            pc.feat.flip = smart_flip(pc.armed.cmd_kind(), pc.feat.op, &pc.project.sketches[si].plane);
                        }
                    }
                    if ui.selectable_label(pc.feat.flip, format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-flip-btn"))).on_hover_text(qymcad_i18n::tr("cmd-reverse-hint")).clicked() {
                        let f = !pc.feat.flip;
                        pc.feat.set_flip(f); // the direction was set by hand
                    }
                }
                35 => {
                    // WHAT COMES OUT OF THE MESH: the exact surfaces found on it, or the mesh as it is - a
                    // polyhedron of its flat faces. The choice belongs in the bar of the tool, where a person is
                    // already looking; as buttons in a node's properties it was never found at all.
                    if pc.cmd.edit.is_none() {
                        ui.label(qymcad_i18n::tr("cmd-operation"));
                        ui.selectable_value(&mut pc.feat.op, 0u8, qymcad_i18n::tr("cmd-recognise-exact"));
                        ui.selectable_value(&mut pc.feat.op, 1u8, qymcad_i18n::tr("cmd-recognise-asis"));
                    }
                    // A HEAVY MESH SAYS SO before Enter: how many triangles, and that the field "Simplify" makes it lighter
                    if let Some(n) = pc.recognise.src.and_then(|src| pc.project.mesh_index(src)).map(|mi| pc.project.bodies[mi].mesh.tris.len()).filter(|&n| n >= HEAVY_MESH) {
                        says.warn(qymcad_i18n::tr1("recognise-heavy", "n", &n.to_string()));
                    }
                    // WHAT WAS FOUND, before Enter: the regions by kind, and those that fit no surface
                    recognise_preview(pc, ctx);
                    if pc.recognise.src.is_some() {
                        ui.separator();
                        match pc.recognise.ready(|f| f.counts) {
                            Some(counts) => {
                                for (k, key) in
                                    ["recognise-planes", "recognise-cylinders", "recognise-cones", "recognise-spheres", "recognise-tori", "recognise-none", "recognise-free", "recognise-helices"]
                                        .into_iter()
                                        .enumerate()
                                {
                                    if counts[k] > 0 {
                                        ui.label(qymcad_i18n::tr1(key, "n", &counts[k].to_string()));
                                    }
                                }
                            }
                            None => {
                                ui.label(qymcad_i18n::tr("recognise-counting"));
                            }
                        }
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
                            if ui.selectable_label(on, format!("{} {name}", ph::LINE_SEGMENT)).on_hover_text(qymcad_i18n::tr("cmd-revolve-any-line-hint")).clicked() {
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
                                    *pc.status = Notice::info(qymcad_i18n::tr("cmd-revolve-pick-line"));
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
                            *pc.status = Notice::info(qymcad_i18n::tr("cmd-pick-axis-hint"));
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
                        if ui
                            .selectable_label(act, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr2("cmd-pick-contour-n", "cur", &cur.to_string(), "total", &prof_cands.len().to_string())))
                            .on_hover_text(qymcad_i18n::tr("cmd-open-sketch-flat"))
                            .clicked()
                        {
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
                        pc.project
                            .sketches
                            .iter()
                            .find(|s| s.id == pc.sweep.path_sid)
                            .map(|s| format!("{} {}", qymcad_i18n::tr1("cmd-path-is", "name", &qymcad_i18n::name(&s.name)), ph::CHECK))
                            .unwrap_or_else(|| qymcad_i18n::tr("cmd-path-unset"))
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
                        if ui
                            .selectable_label(act, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr2("cmd-pick-contour-n", "cur", &cur.to_string(), "total", &path_cands.len().to_string())))
                            .on_hover_text(qymcad_i18n::tr("cmd-open-path-flat"))
                            .clicked()
                        {
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
                            if ui
                                .selectable_label(act, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr2("cmd-pick-contour-short", "cur", &cur.to_string(), "total", &cands.len().to_string())))
                                .on_hover_text(qymcad_i18n::tr("cmd-open-section-flat"))
                                .clicked()
                            {
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
                        let has =
                            qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: &*pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path }).is_some();
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
                    let st = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme },
                        ui,
                        "thread_starts",
                        pc.thread.starts.max(1) as f64,
                        qymcad_ui_state::NumFormat { lo: 1.0, hi: 12.0, integer: true, suffix: "", nonzero: false },
                    );
                    pc.thread.starts = (st as u32).max(1);
                    ui.separator();
                    if ui
                        .selectable_label(pc.thread.left, format!("{} {}", ph::ARROWS_COUNTER_CLOCKWISE, qymcad_i18n::tr("cmd-left-thread")))
                        .on_hover_text(qymcad_i18n::tr("cmd-thread-left"))
                        .clicked()
                    {
                        pc.thread.left = !pc.thread.left;
                    }
                    ui.separator();
                    // THE MODE: a thread (a groove) or an AUGER (a helical ribbon outwards)
                    if ui.selectable_label(!pc.thread.auger, format!("{} {}", ph::SPIRAL, qymcad_i18n::tr("cmd-thread-btn"))).clicked() && pc.thread.auger {
                        pc.thread.auger = false;
                        qymcad_ui_state::set_thread_params(&mut *pc.cmd, *pc.thread);
                        // the modes have different fields
                    }
                    if ui.selectable_label(pc.thread.auger, format!("{} {}", ph::SPIRAL, qymcad_i18n::tr("cmd-auger-btn"))).on_hover_text(qymcad_i18n::tr("cmd-auger-hint")).clicked()
                        && !pc.thread.auger
                    {
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
                            qymcad_i18n::trn(
                                "cmd-thread-geom",
                                &[
                                    ("pitch", &qymcad_i18n::num(g.pitch, 2)),
                                    ("d2", &qymcad_i18n::num(g.pitch_d, 2)),
                                    ("d3", &qymcad_i18n::num(g.minor_d, 2)),
                                    ("depth", &qymcad_i18n::num(g.depth, 2)),
                                ],
                            ),
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
                    if let (0, Some(taken)) = (pc.hole.mode, taken_line(pc)) {
                        ui.label(taken);
                    }
                    if pc.hole.mode == 1 {
                        // "from a sketch": the selected sketch is taken, the number of marker points is shown, plus a normal flip
                        if let qymcad_ui_state::Sel::Sketch(si) = *pc.sel {
                            if let Some(s) = pc.project.sketches.get(si) {
                                pc.hole.sketch = Some(s.id);
                            }
                        }
                        let n = pc.hole.sketch.map(|sid| pc.project.sketch_isolated_points(sid).len()).unwrap_or(0);
                        let txt = match pc.hole.sketch {
                            Some(sid) => pc
                                .project
                                .sketches
                                .iter()
                                .find(|s| s.id == sid)
                                .map(|s| qymcad_i18n::tr2("cmd-sketch-points", "name", &qymcad_i18n::name(&s.name), "n", &n.to_string()))
                                .unwrap_or_else(|| qymcad_i18n::tr("cmd-sketch-unset")),
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
                    pc.prim.n = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme },
                        ui,
                        "prim_n",
                        pc.prim.n as f64,
                        qymcad_ui_state::NumFormat { lo: 3.0, hi: 64.0, integer: true, suffix: "", nonzero: false },
                    ) as u32;
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
                        Some(SketchPlane::Datum(id)) => pc
                            .project
                            .planes
                            .iter()
                            .find(|p| p.id == *id)
                            .map(|p| qymcad_i18n::tr1("cmd-datum-named", "name", &qymcad_i18n::name(&p.name)))
                            .unwrap_or_else(|| qymcad_i18n::tr("cmd-datum")),
                        Some(SketchPlane::Face(b, _)) => qymcad_i18n::tr1("cmd-body-face-n", "b", &b.to_string()),
                        None => qymcad_i18n::tr("cmd-pick-plane"),
                    };
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-plane-is", "what", &picked)).color(pc.scheme.pal.hint()));
                }
                17 => {
                    if let Some(taken) = taken_line(pc) {
                        ui.label(taken);
                    }
                    // A linear pattern: the count + the direction (X/Y/Z) + a grid (a second direction);
                    // THE STEP is an expression field at the geometry (a popup). Ghost previews on the canvas.
                    ui.label(qymcad_i18n::tr("cmd-copies"));
                    pc.arr.count = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme },
                        ui,
                        "arr_count",
                        pc.arr.count as f64,
                        qymcad_ui_state::NumFormat { lo: 2.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
                    ) as u32; // one copy is the body alone
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-direction"));
                    ui.selectable_value(&mut pc.arr.dir, 0u8, "X");
                    ui.selectable_value(&mut pc.arr.dir, 1u8, "Y");
                    ui.selectable_value(&mut pc.arr.dir, 2u8, "Z");
                    ui.separator();
                    ui.checkbox(&mut pc.arr.two, qymcad_i18n::tr("cmd-dir2"));
                    if pc.arr.two {
                        pc.arr.count2 = qymcad_ui_state::num_or_expr(
                            &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme },
                            ui,
                            "arr_count2",
                            pc.arr.count2 as f64,
                            qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
                        ) as u32;
                        ui.selectable_value(&mut pc.arr.dir2, 0u8, "X");
                        ui.selectable_value(&mut pc.arr.dir2, 1u8, "Y");
                        ui.selectable_value(&mut pc.arr.dir2, 2u8, "Z");
                        ui.separator();
                        ui.checkbox(&mut pc.arr.three, qymcad_i18n::tr("cmd-dir3"));
                        if pc.arr.three {
                            pc.arr.count3 = qymcad_ui_state::num_or_expr(
                                &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme },
                                ui,
                                "arr_count3",
                                pc.arr.count3 as f64,
                                qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
                            ) as u32;
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
                    if let Some(taken) = taken_line(pc) {
                        ui.label(taken);
                    }
                    // A circular pattern: the count + the axis by CLICK-PICK (a datum axis or a straight edge)
                    // + a full circle; THE ANGLE is an expression field at the geometry (when it is not a full
                    // circle). Ghost previews.
                    ui.label(qymcad_i18n::tr("cmd-copies"));
                    pc.arr.count = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme },
                        ui,
                        "arr_count",
                        pc.arr.count as f64,
                        qymcad_ui_state::NumFormat { lo: 2.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
                    ) as u32; // one copy is the body alone
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-axis"));
                    let axname = if pc.arr.axis == 0 {
                        qymcad_i18n::tr("cmd-axis-world-z")
                    } else {
                        pc.project.datum_axes.iter().find(|d| d.id == pc.arr.axis).map(|d| qymcad_i18n::name(&d.name)).unwrap_or_else(|| qymcad_i18n::tr("cmd-axis-world-z"))
                    };
                    // the axis is CLICK-PICKED, as the mirror plane is, rather than chosen from a combo box
                    if ui.selectable_label(pc.arr.axis_pick, format!("{} {axname}", ph::CROSSHAIR)).on_hover_text(qymcad_i18n::tr("cmd-revolve-pick-axis3d")).clicked() {
                        pc.arr.axis_pick = !pc.arr.axis_pick;
                        if pc.arr.axis_pick {
                            refresh_axis_edges(pc); // the straight edges of EVERY visible body, as axis candidates
                            *pc.status = Notice::info(qymcad_i18n::tr("cmd-array-axis-hint"));
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
                        Some(SketchPlane::Datum(id)) => pc
                            .project
                            .planes
                            .iter()
                            .find(|p| p.id == *id)
                            .map(|p| qymcad_i18n::tr1("cmd-datum-named", "name", &qymcad_i18n::name(&p.name)))
                            .unwrap_or_else(|| qymcad_i18n::tr("cmd-datum")),
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
                    if let Some(taken) = taken_line(pc) {
                        ui.label(taken);
                    }
                }
            }
            // a value the geometry refuses closes this Apply as it closes the popup's (`trial`)
            let refused = trial_refusal(pc, ui.ctx()) != Trial::Clear;
            qymcad_ui_state::end_of_bar(ui, |ui| {
                // the button is disabled while a dimension's expression is invalid (nothing stale is applied)
                if confirm_button(
                    &pc.scheme.pal,
                    ui,
                    cmd_ready(pc) && cmd_exprs_valid(&*pc.cmd, &*pc.project) && qymcad_ui_state::bar_fields_valid(ui.ctx()) && !refused,
                    format!("{} {}", ph::CHECK, qymcad_i18n::tr("cmd-apply-enter")),
                ) {
                    apply = true;
                }
                if refuse_button(&pc.scheme.pal, ui, format!("{} {}", ph::X, qymcad_i18n::tr("cmd-cancel-esc"))) {
                    cancel = true;
                }
                // the "pick contours" button belongs to the 3D step (setting the size) only - inside the
                // half-sketcher (mode_3d=false) contours are ALREADY being picked, so duplicating it there
                // serves nothing.
                if matches!(pc.armed.cmd_kind(), 1 | 3) && pc.cmd.sketch.is_some() && *pc.mode_3d {
                    // how many contours are taken, named in the bar: a click on one in the view adds or lets it go
                    ui.label(qymcad_i18n::tr1("cmd-profiles-n", "n", &pc.gsel.profiles.len().to_string()));
                    // THE KEY IN THE HINT FOLLOWS THE FOCUS STATE: "U" or "Alt+U". Otherwise the rule that
                    // a focused field needs Alt would stay a secret, and someone who pressed `U` in a field
                    // and got nothing would not try a second time.
                    let hint = format!(
                        "{}  ({})",
                        qymcad_i18n::tr("cmd-back-to-contours"),
                        qymcad_ui_state::hotkey_hint(
                            &qymcad_ui_state::DrawCtx { cam: pc.cam, set: &*pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path },
                            ctx,
                            "part.contour-reselect"
                        )
                    );
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
        sync_torus_params(&*pc.armed, &mut *pc.cmd, &*pc.project); // the tube thinner than the ring
                                                                   // the pattern: a second step when there is a second direction, and an angle when it is not a full circle
        sync_array_params(*pc.arr, &*pc.armed, &mut *pc.cmd, &*pc.project);
        // the datum axis: the origin and direction fields in manual mode
        sync_datum_axis_params(&*pc.armed, &mut *pc.cmd, &*pc.datum);
        sync_hole_params(&*pc.armed, &mut *pc.cmd, *pc.hole, &*pc.project); // the recess fields for a counterbore or a countersink
    }
}
