//! THE BARS ACROSS THE TOP: the menu, the tool bars, the options of the active tool, the command bars.
//!
//! They used to sit inside the frame's body, where "what the frame does" and "what it draws" could not be told
//! apart and editing one command touched the whole life cycle of the frame.

use super::*;

impl App {
    /// THE TOP BAR OF A COMPONENT PATTERN (in an assembly) - the same interaction as the body pattern: the
    /// count and the direction here, the step or the angle in an expression field at the geometry, Enter applies.
    ///
    /// The bars used to be 645 lines sitting straight inside `update`: two thirds of what was left in the frame
    /// after the earlier extractions. While a panel lives in the frame's body, "what the frame does" and "what it
    /// draws" cannot be told apart, and editing one command touches the whole life cycle of the frame.
    pub(super) fn comp_array_bar(&mut self, ctx: &egui::Context) {
        if self.carr.mode == 0 {
            return;
        }
        let (mut apply, mut cancel) = (false, false);
        egui::TopBottomPanel::top("comp_array_bar").frame(self.tool_bar_frame()).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                let title = if self.carr.mode == 2 { crate::i18n::tr("cmd-comp-circ-array") } else { crate::i18n::tr("cmd-comp-lin-array") };
                ui.label(egui::RichText::new(format!("{} {title}", ph::STACK)).strong());
                ui.separator();
                let name = self.project.components.iter().find(|c| c.id == self.carr.src).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
                ui.label(egui::RichText::new(crate::i18n::tr1("cmd-source", "name", &name)).weak());
                ui.separator();
                ui.label(crate::i18n::tr("cmd-copies"));
                self.arr.count = self.num_or_expr(ui, "carr_count", self.arr.count as f64, 1.0, 512.0, true, "") as u32;
                ui.separator();
                if self.carr.mode == 2 {
                    ui.label(crate::i18n::tr("cmd-axis"));
                    ui.selectable_value(&mut self.carr.axis, 0u8, "X");
                    ui.selectable_value(&mut self.carr.axis, 1u8, "Y");
                    ui.selectable_value(&mut self.carr.axis, 2u8, "Z");
                    ui.separator();
                    let was = self.arr.full;
                    ui.checkbox(&mut self.arr.full, crate::i18n::tr("cmd-full-circle-short"));
                    if was != self.arr.full {
                        // the angle field appears and disappears with the checkbox, as in the body pattern
                        self.cmd.params = if self.arr.full { vec![] } else { vec![CmdParam::new("cmd-angle", "cangle", 180.0, -3600.0, 3600.0)] };
                    }
                } else {
                    ui.label(crate::i18n::tr("cmd-direction"));
                    ui.selectable_value(&mut self.carr.dir, 0u8, "X");
                    ui.selectable_value(&mut self.carr.dir, 1u8, "Y");
                    ui.selectable_value(&mut self.carr.dir, 2u8, "Z");
                }
                ui.separator();
                apply = ui.button(format!("{} {}", ph::CHECK, crate::i18n::tr("cmd-apply-enter"))).clicked();
                cancel = ui.button(format!("{} {}", ph::X, crate::i18n::tr("cmd-cancel-btn"))).clicked();
            });
        });
        if apply {
            self.apply_comp_array();
        } else if cancel {
            self.carr = CompArrayCmd::default();
            self.cmd.params.clear();
            self.status = crate::i18n::tr("msg-comp-array-cancelled");
        }
    }

    pub(super) fn feat_command_bar(&mut self, ctx: &egui::Context) {
        // the top row of the active Part command: the options + Apply/Cancel.
        // The SIZE itself is set on the canvas (the gizmo arrow or a field at the geometry), not here.
        if self.cmd.active() {
            let title = match self.cmd.kind {
                1 => &crate::i18n::tr("cmd-extrude"),
                3 => &crate::i18n::tr("cmd-revolve"),
                8 => &crate::i18n::tr("cmd-sweep"),
                9 => &crate::i18n::tr("cmd-loft"),
                4 => &crate::i18n::tr("cmd-fillet"),
                5 => &crate::i18n::tr("cmd-chamfer"),
                6 => &crate::i18n::tr("cmd-shell"),
                7 => &crate::i18n::tr("cmd-hole"),
                10 => &crate::i18n::tr("cmd-box"),
                11 => &crate::i18n::tr("cmd-cylinder"),
                12 => &crate::i18n::tr("cmd-sphere"),
                13 => &crate::i18n::tr("cmd-cone"),
                14 => &crate::i18n::tr("cmd-torus"),
                15 => &crate::i18n::tr("cmd-prism"),
                16 => &crate::i18n::tr("cmd-mirror"),
                17 => &crate::i18n::tr("cmd-linear-array"),
                18 => &crate::i18n::tr("cmd-circular-array"),
                20 => &crate::i18n::tr("cmd-plane"),
                21 => &crate::i18n::tr("cmd-point"),
                22 => &crate::i18n::tr("cmd-axis"),
                23 => &crate::i18n::tr("cmd-draft"),
                24 => &crate::i18n::tr("cmd-thread"),
                _ => &crate::i18n::tr("cmd-command"),
            };
            let (mut apply, mut cancel, mut reselect) = (false, false, false);
            egui::TopBottomPanel::top("feat_cmd_bar").frame(self.tool_bar_frame()).show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{} {title}", ph::CUBE)).strong());
                    ui.separator();
                    match self.cmd.kind {
                        1 => {
                            ui.label(&crate::i18n::tr("cmd-operation"));
                            // a Part is one body, so there is "Add" (material into the single body, seeding it if
                            // it is the first). The former "New" and "Join" merged into "Add" - bodies are no
                            // longer bred.
                            ui.selectable_value(&mut self.feat.op, 0u8, &crate::i18n::tr("cmd-add"));
                            ui.selectable_value(&mut self.feat.op, 2u8, &crate::i18n::tr("cmd-cut"));
                            ui.selectable_value(&mut self.feat.op, 3u8, &crate::i18n::tr("cmd-intersect"));
                            ui.separator();
                            ui.label(&crate::i18n::tr("cmd-extent"));
                            ui.selectable_value(&mut self.cmd.extent, ExtentMode::Length, &crate::i18n::tr("cmd-to-length"));
                            ui.selectable_value(&mut self.cmd.extent, ExtentMode::Symmetric, &crate::i18n::tr("cmd-symmetric"));
                            ui.selectable_value(&mut self.cmd.extent, ExtentMode::TwoSided, &crate::i18n::tr("cmd-two-sides"));
                            if self.feat.op != 0 {
                                ui.selectable_value(&mut self.cmd.extent, ExtentMode::Through, &crate::i18n::tr("cmd-through-all"));
                            } else if self.cmd.extent.through() {
                                self.cmd.extent = ExtentMode::Length; // "through all" only applies to operations on a body
                            }
                            if self.cmd.extent.two_sided() {
                                // the second side's distance is an expression field at the geometry (a popup), not here
                                ui.label(egui::RichText::new(&crate::i18n::tr("cmd-second-side-note")).weak().small());
                            }
                            ui.separator();
                            if ui.selectable_label(self.feat.flip, format!("{} {}", ph::ARROWS_DOWN_UP, crate::i18n::tr("cmd-flip-btn"))).on_hover_text(&crate::i18n::tr("cmd-reverse-hint")).clicked() {
                                let f = !self.feat.flip;
                                self.feat.set_flip(f); // the direction was set by hand
                            }
                        }
                        3 => {
                            // A revolve operation on the part's single body: Add (a boss), Cut or Intersect.
                            ui.label(&crate::i18n::tr("cmd-operation"));
                            ui.selectable_value(&mut self.feat.op, 0u8, &crate::i18n::tr("cmd-add"));
                            ui.selectable_value(&mut self.feat.op, 2u8, &crate::i18n::tr("cmd-cut"));
                            ui.selectable_value(&mut self.feat.op, 3u8, &crate::i18n::tr("cmd-intersect"));
                            ui.separator();
                            // how the angle is laid out: to one side, or symmetrically about half of it; Flip reverses it
                            ui.label(&crate::i18n::tr("cmd-angle"));
                            ui.selectable_value(&mut self.cmd.extent, ExtentMode::Length, &crate::i18n::tr("cmd-one-side"));
                            ui.selectable_value(&mut self.cmd.extent, ExtentMode::Symmetric, &crate::i18n::tr("cmd-symmetric"));
                            if ui.selectable_label(self.feat.flip, format!("{} {}", ph::ARROWS_DOWN_UP, crate::i18n::tr("cmd-flip-btn"))).on_hover_text(&crate::i18n::tr("cmd-flip-angle-hint")).clicked() {
                                self.feat.flip = !self.feat.flip;
                            }
                            ui.separator();
                            ui.label(&crate::i18n::tr("cmd-axis"));
                            // the axis comes from a sketch CENTRELINE, from an edge, a cylindrical face or a datum, or from X-Y.
                            let axis_src = if self.rev.axis_line != 0 {
                                Some(crate::i18n::tr("cmd-sketch-centreline"))
                            } else if self.rev.axis_datum != 0 {
                                Some(self.project.datum_axes.iter().find(|d| d.id == self.rev.axis_datum).map(|d| crate::i18n::name(&d.name)).unwrap_or_else(|| crate::i18n::tr("cmd-axis-lower")))
                            } else {
                                None
                            };
                            match axis_src {
                                Some(name) => {
                                    ui.colored_label(self.scheme.pal.connector(), format!("{} {name}", ph::CROSSHAIR));
                                    if ui.small_button(format!("{} X/Y", ph::X)).on_hover_text(&crate::i18n::tr("cmd-reset-axis-sketch")).clicked() {
                                        self.rev.axis_datum = 0;
                                        self.rev.axis_line = 0;
                                        self.rev.pick_axis = false;
                                    }
                                }
                                None => {
                                    ui.selectable_value(&mut self.rev.axis, 0u8, "X");
                                    ui.selectable_value(&mut self.rev.axis, 1u8, "Y");
                                }
                            }
                            // A sketch CENTRELINE is a reliable choice made BY A BUTTON (with no click in 3D).
                            // For a sphere: a construction diameter line through the circle's centre IN the sketch plane.
                            let axis_lines = self.cmd.sketch.map(|si| self.profile_axis_lines(si)).unwrap_or_default();
                            if !axis_lines.is_empty() {
                                ui.separator();
                                if axis_lines.len() == 1 {
                                    let l = axis_lines[0];
                                    let on = self.rev.axis_line == l;
                                    let name = self.axis_line_label(self.cmd.sketch.unwrap_or(0), l, 1);
                                    if ui
                                        .selectable_label(on, format!("{} {name}", ph::LINE_SEGMENT))
                                        .on_hover_text(&crate::i18n::tr("cmd-revolve-any-line-hint"))
                                        .clicked()
                                    {
                                        self.rev.axis_line = if on { 0 } else { l };
                                        if self.rev.axis_line != 0 {
                                            self.rev.axis_datum = 0;
                                        }
                                    }
                                } else {
                                    // With SEVERAL lines the choice is made BY CLICKING the line itself in the
                                    // sketch rather than from a list of numbered lines: a number does not tell
                                    // which of them is the one wanted.
                                    let si_cur = self.cmd.sketch.unwrap_or(0);
                                    if ui
                                        .selectable_label(self.rev.pick_line, format!("{} {}", ph::CROSSHAIR, crate::i18n::tr("cmd-pick-axis-sketch")))
                                        .on_hover_text(&crate::i18n::tr("cmd-revolve-flat-hint"))
                                        .clicked()
                                    {
                                        self.rev.pick_line = !self.rev.pick_line;
                                        if self.rev.pick_line {
                                            self.rev.pick_axis = false;
                                            self.mode_3d = false; // the flat half-sketcher: the lines are visible and clickable
                                            self.view.initialized = false;
                                            self.status = crate::i18n::tr("cmd-revolve-pick-line");
                                        }
                                    }
                                    if self.rev.axis_line != 0 {
                                        let name = self.axis_line_label(si_cur, self.rev.axis_line, axis_lines.iter().position(|l| *l == self.rev.axis_line).map(|i| i + 1).unwrap_or(1));
                                        ui.label(egui::RichText::new(format!("{} {name}", ph::CHECK)).color(self.scheme.pal.hint()));
                                        if ui.small_button(&crate::i18n::tr("cmd-reset-lower")).clicked() {
                                            self.rev.axis_line = 0;
                                        }
                                    }
                                    let _cur_unused = if self.rev.axis_line != 0 {
                                        axis_lines
                                            .iter()
                                            .position(|l| *l == self.rev.axis_line)
                                            .map(|i| self.axis_line_label(si_cur, self.rev.axis_line, i + 1))
                                            .unwrap_or_else(|| crate::i18n::tr("cmd-sketch-axis"))
                                    } else {
                                        crate::i18n::tr("cmd-axis-from-sketch")
                                    };
                                    let _ = _cur_unused;
                                    if self.rev.axis_line != 0 {
                                        self.rev.axis_datum = 0;
                                    }
                                }
                            }
                            ui.separator();
                            // the regular 3D axis pick: a STRAIGHT edge of a body, a CYLINDRICAL face or a datum axis
                            if ui.selectable_label(self.rev.pick_axis, format!("{} {}", ph::CROSSHAIR, crate::i18n::tr("cmd-pick-axis-3d"))).on_hover_text(&crate::i18n::tr("cmd-revolve-axis-hint")).clicked() {
                                self.rev.pick_axis = !self.rev.pick_axis;
                                if self.rev.pick_axis {
                                    self.rev.pick_line = false;
                                    // The axis candidates (datum axes, edges) are drawn and hit-tested ONLY in 3D.
                                    // Staying in the flat half-sketcher would make the button do nothing: there
                                    // would be nothing to click.
                                    self.mode_3d = true;
                                    self.view.initialized = false;
                                    self.refresh_axis_edges(); // the straight edges of every visible body are axis candidates
                                    self.status = crate::i18n::tr("cmd-pick-axis-hint");
                                }
                            }
                        }
                        8 => {
                            // a sweep operation on the single body: Add, Cut or Intersect
                            ui.label(&crate::i18n::tr("cmd-operation"));
                            ui.selectable_value(&mut self.feat.op, 0u8, &crate::i18n::tr("cmd-add"));
                            ui.selectable_value(&mut self.feat.op, 2u8, &crate::i18n::tr("cmd-cut"));
                            ui.selectable_value(&mut self.feat.op, 3u8, &crate::i18n::tr("cmd-intersect"));
                            ui.separator();
                            // Sweep: the profile readout + the contour choice + picking the path (a click on a sketch in the tree)
                            let prof_name = self.project.sketches.iter().find(|s| s.id == self.sweep.prof_sid).map(|s| crate::i18n::name(&s.name)).unwrap_or_else(|| "—".into());
                            ui.label(egui::RichText::new(crate::i18n::tr1("cmd-profile-is", "name", &prof_name)).color(self.scheme.pal.hint()));
                            // choosing the profile's contour through the HALF-SKETCHER (a click on the contour), when there is more than one
                            let prof_cands = self.project.sweep_profile_contours(self.sweep.prof_sid);
                            if prof_cands.len() > 1 {
                                let cur = prof_cands.iter().position(|c| *c == self.sweep.prof_cid).map(|i| i + 1).unwrap_or(1);
                                let act = self.picking.contour() == Some(ContourSlot::SweepProfile);
                                if ui.selectable_label(act, format!("{} {}", ph::CROSSHAIR, crate::i18n::tr2("cmd-pick-contour-n", "cur", &cur.to_string(), "total", &prof_cands.len().to_string()))).on_hover_text(&crate::i18n::tr("cmd-open-sketch-flat")).clicked() {
                                    self.begin_contour_pick(ContourSlot::SweepProfile, self.sweep.prof_sid);
                                }
                            }
                            ui.separator();
                            // the path is taken from the tree selection while the pick is active (as for a hole driven by a sketch)
                            if self.sweep.pick_path {
                                if let Sel::Sketch(si) = self.sel {
                                    if let Some(s) = self.project.sketches.get(si) {
                                        if s.id != self.sweep.prof_sid {
                                            self.sweep.path_sid = s.id;
                                            self.sweep.path_cid = 0; // reset the contour choice on a new sketch
                                            self.sweep.pick_path = false;
                                        }
                                    }
                                }
                            }
                            let path_txt = if self.sweep.path_sid != 0 {
                                self.project.sketches.iter().find(|s| s.id == self.sweep.path_sid).map(|s| format!("{} {}", crate::i18n::tr1("cmd-path-is", "name", &crate::i18n::name(&s.name)), ph::CHECK)).unwrap_or_else(|| crate::i18n::tr("cmd-path-unset"))
                            } else {
                                crate::i18n::tr("cmd-pick-path-sketch")
                            };
                            if ui.selectable_label(self.sweep.pick_path, format!("{} {path_txt}", ph::LINE_SEGMENT)).on_hover_text(&crate::i18n::tr("cmd-sweep-pick-path")).clicked() {
                                self.sweep.pick_path = !self.sweep.pick_path;
                            }
                            // choosing the path's contour through the HALF-SKETCHER (a click on the contour), when there is more than one
                            let path_cands = self.project.sweep_path_contours(self.sweep.path_sid);
                            if path_cands.len() > 1 {
                                let cur = path_cands.iter().position(|c| *c == self.sweep.path_cid).map(|i| i + 1).unwrap_or(1);
                                let act = self.picking.contour() == Some(ContourSlot::SweepPath);
                                if ui.selectable_label(act, format!("{} {}", ph::CROSSHAIR, crate::i18n::tr2("cmd-pick-contour-n", "cur", &cur.to_string(), "total", &path_cands.len().to_string()))).on_hover_text(&crate::i18n::tr("cmd-open-path-flat")).clicked() {
                                    self.begin_contour_pick(ContourSlot::SweepPath, self.sweep.path_sid);
                                }
                            }
                            ui.separator();
                            ui.label(egui::RichText::new(&crate::i18n::tr("cmd-sweep-auto-hint")).weak().small());
                        }
                        9 => {
                            // Loft: an ordered list of sections + adding by a click + ruled/smooth.
                            // A new section is taken on the EDGE of a change in the tree selection (while the pick
                            // is active). Only when the selected sketch changes - otherwise a section removed by
                            // its cross would come straight back while the same sketch stayed selected in the tree.
                            if self.loft.pick {
                                let cur = if let Sel::Sketch(si) = self.sel { self.project.sketches.get(si).map(|s| s.id) } else { None };
                                if cur != self.loft.pick_last {
                                    self.loft.pick_last = cur;
                                    if let Some(sid) = cur {
                                        if !self.loft.sids.contains(&sid) && !self.project.sweep_profile_contours(sid).is_empty() {
                                            self.loft.sids.push(sid);
                                            self.loft.cids.push(0);
                                        }
                                    }
                                }
                            }
                            // the sections in order: the number + the name + a contour switch + delete
                            let mut remove: Option<usize> = None;
                            for i in 0..self.loft.sids.len() {
                                let sid = self.loft.sids[i];
                                let name = self.project.sketches.iter().find(|s| s.id == sid).map(|s| crate::i18n::name(&s.name)).unwrap_or_else(|| "—".into());
                                ui.label(egui::RichText::new(format!("{}:{name}", i + 1)).color(self.scheme.pal.hint()));
                                let cands = self.project.sweep_profile_contours(sid);
                                if cands.len() > 1 {
                                    let cur = cands.iter().position(|c| *c == self.loft.cids[i]).map(|k| k + 1).unwrap_or(1);
                                    let act = self.picking.contour() == Some(ContourSlot::LoftSection(i));
                                    if ui.selectable_label(act, format!("{} {}", ph::CROSSHAIR, crate::i18n::tr2("cmd-pick-contour-short", "cur", &cur.to_string(), "total", &cands.len().to_string()))).on_hover_text(&crate::i18n::tr("cmd-open-section-flat")).clicked() {
                                        self.begin_contour_pick(ContourSlot::LoftSection(i), sid);
                                    }
                                }
                                if ui.small_button(ph::X).on_hover_text(&crate::i18n::tr("cmd-remove-section")).clicked() {
                                    remove = Some(i);
                                }
                                ui.separator();
                            }
                            if let Some(i) = remove {
                                self.loft.sids.remove(i);
                                self.loft.cids.remove(i);
                            }
                            // the "add a section" button (picking in the tree)
                            if ui.selectable_label(self.loft.pick, format!("{} {}", ph::PLUS, crate::i18n::tr("cmd-add-section"))).on_hover_text(&crate::i18n::tr("cmd-loft-pick-hint")).clicked() {
                                self.loft.pick = !self.loft.pick;
                                self.loft.pick_last = None; // the pick is on, so the current selection may be taken on the next edge
                            }
                            ui.separator();
                            // the kind of surface between the sections
                            ui.label(&crate::i18n::tr("cmd-faces"));
                            ui.selectable_value(&mut self.loft.ruled, false, &crate::i18n::tr("cmd-smooth"));
                            ui.selectable_value(&mut self.loft.ruled, true, &crate::i18n::tr("cmd-ruled"));
                            ui.separator();
                            // the kind of result: a separate body, or a boolean with the active body
                            ui.label(&crate::i18n::tr("cmd-result"));
                            ui.selectable_value(&mut self.loft.result, 0u8, &crate::i18n::tr("cmd-add"));
                            ui.selectable_value(&mut self.loft.result, 1u8, &crate::i18n::tr("cmd-cut"));
                            ui.selectable_value(&mut self.loft.result, 2u8, &crate::i18n::tr("cmd-union"));
                            ui.selectable_value(&mut self.loft.result, 3u8, &crate::i18n::tr("cmd-intersection"));
                            // a surface through the sections: the same loft, not closed into a body
                            ui.selectable_value(&mut self.loft.result, 4u8, &crate::i18n::tr("cmd-surface"));
                            if self.loft.result != 0 && self.loft.result != 4 {
                                let has = self.current_body().is_some();
                                let (txt, col) = if has { (&crate::i18n::tr("cmd-bool-with-active"), self.scheme.pal.hint()) } else { (&crate::i18n::tr("cmd-no-active-body"), self.scheme.pal.warning()) };
                                ui.label(egui::RichText::new(txt).color(col).small());
                            }
                            ui.separator();
                            ui.label(egui::RichText::new(crate::i18n::tr1("cmd-sections-n", "n", &self.loft.sids.len().to_string())).weak().small());
                        }
                        5 => {
                            // Chamfer: the mode (symmetric, two distances, leg and angle) + the side of the reference face
                            use qymcad_core::feature::ChamferMode;
                            ui.label(&crate::i18n::tr("cmd-mode"));
                            let prev = self.chamfer.mode;
                            ui.selectable_value(&mut self.chamfer.mode, ChamferMode::Symmetric, &crate::i18n::tr("cmd-symmetric"));
                            ui.selectable_value(&mut self.chamfer.mode, ChamferMode::TwoDist, &crate::i18n::tr("cmd-two-distances"));
                            ui.selectable_value(&mut self.chamfer.mode, ChamferMode::DistAngle, &crate::i18n::tr("cmd-leg-angle"));
                            if self.chamfer.mode != prev {
                                // the meaning of field d2 changed (mm <-> degrees), so both the label AND the value
                                // at the geometry are updated (45 mm as a second leg, or 1.5 deg as an angle, are
                                // both meaningless, hence the mode's default)
                                let def = if self.chamfer.mode == ChamferMode::DistAngle { 45.0 } else { 1.5 };
                                if let Some(p) = self.cmd.params.iter_mut().find(|p| p.key == "d2") {
                                    p.set_label(Self::chamfer_d2_label(self.chamfer.mode));
                                    p.val = def;
                                    p.txt = format!("{def:.2}");
                                }
                            }
                            if self.chamfer.mode != ChamferMode::Symmetric {
                                ui.separator();
                                if ui.selectable_label(self.chamfer.flip, format!("{} {}", ph::ARROWS_DOWN_UP, crate::i18n::tr("cmd-side-btn"))).on_hover_text(&crate::i18n::tr("cmd-swap-ref-face")).clicked() {
                                    self.chamfer.flip = !self.chamfer.flip;
                                }
                                // picking the reference face by hand: a click on a face in 3D. It overrides "Side"
                                // for the edges adjacent to that face. Clicking the button again, or Reset, clears it.
                                ui.separator();
                                let lbl = if self.chamfer.ref_face != 0 {
                                    format!("{} {} {}", ph::CUBE, crate::i18n::tr("cmd-ref-face"), ph::CHECK)
                                } else {
                                    format!("{} {}", ph::CUBE, crate::i18n::tr("cmd-ref-face"))
                                };
                                if ui.selectable_label(self.chamfer.pick_ref, lbl).on_hover_text(&crate::i18n::tr("cmd-chamfer-ref-hint")).clicked() {
                                    self.chamfer.pick_ref = !self.chamfer.pick_ref;
                                }
                                if self.chamfer.ref_face != 0 && ui.small_button(&crate::i18n::tr("cmd-reset")).on_hover_text(&crate::i18n::tr("cmd-neutral-auto-back")).clicked() {
                                    self.chamfer.ref_face = 0;
                                    self.chamfer.pick_ref = false;
                                }
                                ui.label(egui::RichText::new(&crate::i18n::tr("cmd-asym-note")).weak().small());
                            }
                            ui.separator();
                            // A COUNT LIES WHEN THE SELECTION IS DESCRIBED. "Edges: 4" is a snapshot of today,
                            // while the description "every edge of this face" will take a fifth one tomorrow. What
                            // is actually recorded is written out in words.
                            let what = match &self.gsel.described {
                                Some(_) => crate::i18n::tr1("expand-described", "what", &crate::i18n::tr("expand-face-edges")),
                                None => crate::i18n::tr1("cmd-edges-n", "n", &self.gsel.edges.len().to_string()),
                            };
                            ui.label(egui::RichText::new(what).weak());
                        }
                        32 => {
                            // PATCH: smooth, or by position. A switch rather than a checkbox in the corner: these
                            // are two DIFFERENT surfaces on one boundary, and the choice is visible before Enter.
                            if ui.selectable_label(!self.opts.patch_tangent, &crate::i18n::tr("cmd-patch-flat")).clicked() {
                                self.opts.patch_tangent = false;
                            }
                            if ui.selectable_label(self.opts.patch_tangent, &crate::i18n::tr("cmd-patch-tangent")).clicked() {
                                self.opts.patch_tangent = true;
                            }
                            ui.separator();
                            let what = match &self.gsel.described {
                                Some(_) => crate::i18n::tr1("expand-described", "what", &crate::i18n::tr("expand-face-edges")),
                                None => crate::i18n::tr1("cmd-edges-n", "n", &self.gsel.edges.len().to_string()),
                            };
                            ui.label(egui::RichText::new(what).weak());
                        }
                        6 => {
                            // Shell: the direction of the thickness + the count of the multi-selected faces
                            ui.label(&crate::i18n::tr("cmd-thickness"));
                            // a three-position mode: inwards, outwards or centred
                            use qymcad_core::feature::ShellSide;
                            for (side, word) in [(ShellSide::Inward, "cmd-inwards"), (ShellSide::Outward, "cmd-outwards"), (ShellSide::Centred, "cmd-centred")] {
                                if ui.selectable_label(self.opts.shell_side == side, &crate::i18n::tr(word)).clicked() {
                                    self.opts.shell_side = side;
                                }
                            }
                            ui.separator();
                            ui.label(egui::RichText::new(crate::i18n::tr1("cmd-open-faces-n", "n", &self.gsel.faces.len().to_string())).color(self.scheme.pal.hint()));
                        }
                        23 => {
                            // Draft: the set of faces to tilt + the neutral face (the fixed section plane) + a flip
                            ui.label(egui::RichText::new(crate::i18n::tr1("cmd-draft-faces-n", "n", &self.gsel.faces.len().to_string())).color(self.scheme.pal.selected()));
                            ui.separator();
                            let lbl = if self.draft.neutral != 0 {
                                format!("{} {} {}", ph::CUBE, crate::i18n::tr("cmd-neutral-face"), ph::CHECK)
                            } else {
                                format!("{} {}", ph::CUBE, crate::i18n::tr("cmd-neutral-face"))
                            };
                            if ui.selectable_label(self.draft.pick_neutral, lbl).on_hover_text(&crate::i18n::tr("cmd-draft-neutral-hint")).clicked() {
                                self.draft.pick_neutral = !self.draft.pick_neutral;
                            }
                            if self.draft.neutral != 0 && ui.small_button(&crate::i18n::tr("cmd-reset")).on_hover_text(&crate::i18n::tr("cmd-clear-neutral")).clicked() {
                                self.draft.neutral = 0;
                                self.draft.pick_neutral = false;
                            }
                            ui.separator();
                            if ui.selectable_label(self.draft.flip, format!("{} {}", ph::ARROWS_DOWN_UP, crate::i18n::tr("cmd-flip-back"))).on_hover_text(&crate::i18n::tr("cmd-flip-draft-hint")).clicked() {
                                self.draft.flip = !self.draft.flip;
                            }
                            ui.separator();
                            ui.label(egui::RichText::new(&crate::i18n::tr("cmd-angle-field-hint")).weak().small());
                        }
                        24 => {
                            // Thread: inner/outer + the number of starts + the hand; the pitch, length, angle and depth live at the geometry
                            ui.label(&crate::i18n::tr("cmd-kind"));
                            ui.selectable_value(&mut self.thread.internal, false, &crate::i18n::tr("cmd-thread-external")).on_hover_text(&crate::i18n::tr("cmd-on-cylinder"));
                            ui.selectable_value(&mut self.thread.internal, true, &crate::i18n::tr("cmd-thread-internal")).on_hover_text(&crate::i18n::tr("cmd-in-hole"));
                            ui.separator();
                            ui.label(&crate::i18n::tr("cmd-starts"));
                            // The number of starts is a dimension of the part as well, and it must be parametric:
                            // a two-start thread driven by a global variable is an ordinary thing.
                            let st = self.num_or_expr(ui, "thread_starts", self.thread.starts.max(1) as f64, 1.0, 12.0, true, "");
                            self.thread.starts = (st as u32).max(1);
                            ui.separator();
                            if ui.selectable_label(self.thread.left, format!("{} {}", ph::ARROWS_COUNTER_CLOCKWISE, crate::i18n::tr("cmd-left-thread"))).on_hover_text(&crate::i18n::tr("cmd-thread-left")).clicked() {
                                self.thread.left = !self.thread.left;
                            }
                            ui.separator();
                            // THE MODE: a thread (a groove) or an AUGER (a helical ribbon outwards)
                            if ui.selectable_label(!self.thread.auger, format!("{} {}", ph::SPIRAL, crate::i18n::tr("cmd-thread-btn"))).clicked() && self.thread.auger {
                                self.thread.auger = false;
                                self.set_thread_params(); // the modes have different fields
                            }
                            if ui.selectable_label(self.thread.auger, format!("{} {}", ph::SPIRAL, crate::i18n::tr("cmd-auger-btn"))).on_hover_text(&crate::i18n::tr("cmd-auger-hint")).clicked() && !self.thread.auger {
                                self.thread.auger = true;
                                self.set_thread_params();
                            }
                            ui.separator();
                            if self.thread.auger {
                                ui.label(egui::RichText::new(&crate::i18n::tr("cmd-auger-fields-hint")).weak().small());
                            } else {
                                // THE STANDARD (the thread's type): the geometry is computed by the model core
                                ui.label(&crate::i18n::tr("cmd-standard"));
                                use qymcad_core::thread::ThreadStandard as TS;
                                for (idx, std) in [(0u8, TS::MetricIso), (1, TS::TrapezoidalTr), (2, TS::Acme), (3, TS::RoundRd), (4, TS::Buttress), (5, TS::Custom)] {
                                    let short = match idx {
                                        1 => "Tr",
                                        2 => "ACME",
                                        3 => "Rd",
                                        4 => &crate::i18n::tr("cmd-stop"),
                                        5 => &crate::i18n::tr("cmd-custom"),
                                        _ => "M",
                                    };
                                    if ui.selectable_label(self.thread.form == idx, short).on_hover_text(crate::i18n::tr(std.label())).clicked() {
                                        self.thread.form = idx;
                                        self.sync_custom_params(); // "custom" opens the angle and the depth, the others hide them
                                    }
                                }
                                ui.separator();
                                // a hint about the actual geometry of the chosen size
                                let spec = qymcad_core::thread::ThreadSpec {
                                    standard: Self::thread_standard(self.thread.form),
                                    nominal_d: if self.cmd_val("nominal") > 0.0 { self.cmd_val("nominal") } else { self.thread.radius * 2.0 },
                                    pitch: self.cmd_val("pitch"),
                                    internal: self.thread.internal,
                                    fit: self.cmd_val("fit"),
                                    ..Default::default()
                                };
                                let g = spec.geometry();
                                ui.label(
                                    egui::RichText::new(crate::i18n::trn("cmd-thread-geom", &[("pitch", &crate::i18n::num(g.pitch, 2)), ("d2", &crate::i18n::num(g.pitch_d, 2)), ("d3", &crate::i18n::num(g.minor_d, 2)), ("depth", &crate::i18n::num(g.depth, 2))]))
                                        .color(self.scheme.pal.hint())
                                        .small(),
                                )
                                .on_hover_text(&crate::i18n::tr("cmd-thread-std-hint"));
                                // WHAT THE MATING PART NEEDS, said out loud. Asked for plainly: a person must
                                // see what diameter of shaft or hole this thread wants and with what parameters
                                // to make its counterpart - otherwise the numbers get looked up in a table, and
                                // a table does not know about the fit that was typed in here.
                                let (own, mate) = spec.blank_diameters();
                                let key = if self.thread.internal { "cmd-thread-mate-internal" } else { "cmd-thread-mate-external" };
                                ui.label(
                                    egui::RichText::new(crate::i18n::tr2(key, "own", &crate::i18n::num(own, 2), "mate", &crate::i18n::num(mate, 2)))
                                        .color(self.scheme.pal.hint())
                                        .small(),
                                )
                                .on_hover_text(&crate::i18n::tr("cmd-thread-mate-hint"));
                                // A PROFILE THAT DOES NOT FIT THE PITCH is said out loud, with the numbers to
                                // change. It used to be taken in silence and built as rubbish: the passes
                                // overlap, eat the turn between them and leave flat plates that mate with
                                // nothing.
                                // THE CLEARANCE THAT DOES NOT FIT is named too. It saturates against the
                                // pitch without a word, and two different numbers typed in then give one and
                                // the same body - the pair binds and nothing explains why.
                                if let Some((asked, given)) = spec.fit_overflow() {
                                    ui.label(
                                        egui::RichText::new(crate::i18n::trn(
                                            "cmd-thread-fit-capped",
                                            &[
                                                ("asked", &crate::i18n::num(asked, 2)),
                                                ("given", &crate::i18n::num(given, 3)),
                                                // WHAT IS LEFT OVER, said as a DIAMETER correction. The
                                                // missing clearance is measured along the flank, and a radial
                                                // move is not worth the same: a flank stands at the half-angle,
                                                // so `radial_relief` converts one into the other. Twice it,
                                                // because a diameter has two sides.
                                                ("rest", &crate::i18n::num(spec.radial_relief() * 2.0, 2)),
                                            ],
                                        ))
                                        .color(self.scheme.pal.error_mild())
                                        .small(),
                                    );
                                }
                                if let Some((width, max_depth, min_pitch)) = spec.profile_overflow() {
                                    ui.label(
                                        egui::RichText::new(crate::i18n::trn(
                                            "cmd-thread-too-wide",
                                            &[
                                                ("width", &crate::i18n::num(width, 2)),
                                                ("pitch", &crate::i18n::num(spec.geometry().pitch, 2)),
                                                ("depth", &crate::i18n::num(max_depth, 2)),
                                                ("minpitch", &crate::i18n::num(min_pitch, 2)),
                                            ],
                                        ))
                                        .color(self.scheme.pal.error_mild())
                                        .small(),
                                    );
                                }
                            }
                            ui.separator();
                            let tgt = if self.thread.edge == 0 {
                                crate::i18n::tr("cmd-thread-pick-hint")
                            } else {
                                crate::i18n::tr1("cmd-actual-diameter", "d", &crate::i18n::num(self.thread.radius * 2.0, 1))
                            };
                            ui.label(egui::RichText::new(tgt).weak().small());
                        }
                        7 => {
                            // Hole: the placement mode (a face or a sketch) + the type; the diameter and depth live at the geometry
                            ui.label(&crate::i18n::tr("cmd-placement"));
                            ui.selectable_value(&mut self.hole.mode, 0u8, &crate::i18n::tr("cmd-by-face"));
                            ui.selectable_value(&mut self.hole.mode, 1u8, &crate::i18n::tr("cmd-by-sketch"));
                            ui.separator();
                            ui.label(&crate::i18n::tr("cmd-kind"));
                            ui.selectable_value(&mut self.hole.kind, 0u8, &crate::i18n::tr("cmd-simple"));
                            ui.selectable_value(&mut self.hole.kind, 1u8, &crate::i18n::tr("cmd-counterbore"));
                            ui.selectable_value(&mut self.hole.kind, 2u8, &crate::i18n::tr("cmd-countersink"));
                            ui.separator();
                            if self.hole.mode == 1 {
                                // "from a sketch": the selected sketch is taken, the number of marker points is shown, plus a normal flip
                                if let Sel::Sketch(si) = self.sel {
                                    if let Some(s) = self.project.sketches.get(si) {
                                        self.hole.sketch = Some(s.id);
                                    }
                                }
                                let n = self.hole.sketch.map(|sid| self.project.sketch_isolated_points(sid).len()).unwrap_or(0);
                                let txt = match self.hole.sketch {
                                    Some(sid) => self.project.sketches.iter().find(|s| s.id == sid).map(|s| crate::i18n::tr2("cmd-sketch-points", "name", &crate::i18n::name(&s.name), "n", &n.to_string())).unwrap_or_else(|| crate::i18n::tr("cmd-sketch-unset")),
                                    None => crate::i18n::tr("cmd-pick-sketch-points"),
                                };
                                let col = if n > 0 { self.scheme.pal.hint() } else { self.scheme.pal.hint_action() };
                                ui.label(egui::RichText::new(txt).color(col));
                                ui.separator();
                                ui.checkbox(&mut self.hole.flip, &crate::i18n::tr("cmd-flip"));
                            } else {
                                ui.label(egui::RichText::new(&crate::i18n::tr("cmd-hole-face-hint")).weak());
                            }
                        }
                        15 => {
                            // Prism: the number of sides + a hint; the diameter and the height are fields at the geometry
                            ui.label(&crate::i18n::tr("cmd-sides"));
                            self.prim.n = self.num_or_expr(ui, "prim_n", self.prim.n as f64, 3.0, 64.0, true, "") as u32;
                            ui.separator();
                            ui.label(egui::RichText::new(&crate::i18n::tr("cmd-dia-height-hint")).weak());
                        }
                        10..=14 => {
                            ui.label(egui::RichText::new(&crate::i18n::tr("cmd-sizes-hint")).weak());
                        }
                        16 => {
                            // Mirror: keep the original + CLICK-PICK the plane in the viewport + a readout of what was picked
                            use qymcad_core::feature::SketchPlane;
                            ui.checkbox(&mut self.opts.mirror_keep, &crate::i18n::tr("cmd-with-original"));
                            ui.separator();
                            let picked = match &self.mirror.plane {
                                Some(SketchPlane::World(bp)) => crate::i18n::tr1("cmd-world-plane", "plane", ["XY", "XZ", "YZ"][*bp as usize]),
                                Some(SketchPlane::Datum(id)) => self.project.planes.iter().find(|p| p.id == *id).map(|p| crate::i18n::tr1("cmd-datum-named", "name", &p.name)).unwrap_or_else(|| crate::i18n::tr("cmd-datum")),
                                Some(SketchPlane::Face(b, _)) => crate::i18n::tr1("cmd-body-face-n", "b", &b.to_string()),
                                None => crate::i18n::tr("cmd-pick-plane"),
                            };
                            ui.label(egui::RichText::new(crate::i18n::tr1("cmd-plane-is", "what", &picked)).color(self.scheme.pal.hint()));
                        }
                        17 => {
                            // A linear pattern: the count + the direction (X/Y/Z) + a grid (a second direction);
                            // THE STEP is an expression field at the geometry (a popup). Ghost previews on the canvas.
                            ui.label(&crate::i18n::tr("cmd-copies"));
                            self.arr.count = self.num_or_expr(ui, "arr_count", self.arr.count as f64, 1.0, 512.0, true, "") as u32;
                            ui.separator();
                            ui.label(&crate::i18n::tr("cmd-direction"));
                            ui.selectable_value(&mut self.arr.dir, 0u8, "X");
                            ui.selectable_value(&mut self.arr.dir, 1u8, "Y");
                            ui.selectable_value(&mut self.arr.dir, 2u8, "Z");
                            ui.separator();
                            ui.checkbox(&mut self.arr.two, &crate::i18n::tr("cmd-dir2"));
                            if self.arr.two {
                                self.arr.count2 = self.num_or_expr(ui, "arr_count2", self.arr.count2 as f64, 1.0, 512.0, true, "") as u32;
                                ui.selectable_value(&mut self.arr.dir2, 0u8, "X");
                                ui.selectable_value(&mut self.arr.dir2, 1u8, "Y");
                                ui.selectable_value(&mut self.arr.dir2, 2u8, "Z");
                                ui.separator();
                                ui.checkbox(&mut self.arr.three, &crate::i18n::tr("cmd-dir3"));
                                if self.arr.three {
                                    self.arr.count3 = self.num_or_expr(ui, "arr_count3", self.arr.count3 as f64, 1.0, 512.0, true, "") as u32;
                                    ui.selectable_value(&mut self.arr.dir3, 0u8, "X");
                                    ui.selectable_value(&mut self.arr.dir3, 1u8, "Y");
                                    ui.selectable_value(&mut self.arr.dir3, 2u8, "Z");
                                }
                            } else {
                                self.arr.three = false; // the third direction only sits on top of the second (a full 3D grid)
                            }
                            ui.separator();
                            ui.label(egui::RichText::new(&crate::i18n::tr("cmd-pitch-field-hint")).weak());
                        }
                        18 => {
                            // A circular pattern: the count + the axis by CLICK-PICK (a datum axis or a straight edge)
                            // + a full circle; THE ANGLE is an expression field at the geometry (when it is not a full
                            // circle). Ghost previews.
                            ui.label(&crate::i18n::tr("cmd-copies"));
                            self.arr.count = self.num_or_expr(ui, "arr_count", self.arr.count as f64, 1.0, 512.0, true, "") as u32;
                            ui.separator();
                            ui.label(&crate::i18n::tr("cmd-axis"));
                            let axname = if self.arr.axis == 0 { crate::i18n::tr("cmd-axis-world-z") } else { self.project.datum_axes.iter().find(|d| d.id == self.arr.axis).map(|d| crate::i18n::name(&d.name)).unwrap_or_else(|| crate::i18n::tr("cmd-axis-world-z")) };
                            // the axis is CLICK-PICKED, as the mirror plane is, rather than chosen from a combo box
                            if ui.selectable_label(self.arr.axis_pick, format!("{} {axname}", ph::CROSSHAIR)).on_hover_text(&crate::i18n::tr("cmd-revolve-pick-axis3d")).clicked() {
                                self.arr.axis_pick = !self.arr.axis_pick;
                                if self.arr.axis_pick {
                                    self.refresh_axis_edges(); // the straight edges of EVERY visible body, as axis candidates
                                    self.status = crate::i18n::tr("cmd-array-axis-hint");
                                }
                            }
                            if self.arr.axis != 0 && ui.small_button("Z").on_hover_text(&crate::i18n::tr("cmd-reset-axis-z")).clicked() {
                                self.arr.axis = 0;
                                self.arr.axis_pick = false;
                            }
                            ui.separator();
                            ui.checkbox(&mut self.arr.full, &crate::i18n::tr("cmd-full-circle"));
                            if !self.arr.full {
                                ui.label(egui::RichText::new(&crate::i18n::tr("cmd-angle-field-lower")).weak());
                            }
                        }
                        20 => {
                            // A datum PLANE: click-pick a base plane, a datum or a face + the offset at the geometry
                            use qymcad_core::feature::SketchPlane;
                            let picked = match &self.datum.plane_pick {
                                Some(SketchPlane::World(bp)) => crate::i18n::tr1("cmd-world-plane", "plane", ["XY", "XZ", "YZ"][*bp as usize]),
                                Some(SketchPlane::Datum(id)) => self.project.planes.iter().find(|p| p.id == *id).map(|p| crate::i18n::tr1("cmd-datum-named", "name", &p.name)).unwrap_or_else(|| crate::i18n::tr("cmd-datum")),
                                Some(SketchPlane::Face(b, _)) => crate::i18n::tr1("cmd-body-face-n", "b", &b.to_string()),
                                None => crate::i18n::tr("cmd-pick-plane"),
                            };
                            ui.label(egui::RichText::new(crate::i18n::tr1("cmd-from-is", "what", &picked)).color(self.scheme.pal.hint()));
                            ui.separator();
                            ui.label(egui::RichText::new(&crate::i18n::tr("cmd-offset-field-hint")).weak());
                        }
                        21 => {
                            // A datum POINT: X/Y/Z coordinates, or associatively at a vertex (by a click)
                            let prev = self.datum.pt_mode;
                            ui.label(&crate::i18n::tr("cmd-method"));
                            ui.selectable_value(&mut self.datum.pt_mode, 0u8, &crate::i18n::tr("cmd-coordinates"));
                            ui.selectable_value(&mut self.datum.pt_mode, 1u8, &crate::i18n::tr("cmd-to-vertex")).on_hover_text(&crate::i18n::tr("cmd-vertex-assoc-hint"));
                            if prev != self.datum.pt_mode {
                                self.sync_datum_point_params();
                            }
                            ui.separator();
                            if self.datum.pt_mode == 1 {
                                let r = if self.datum.pt_vert.is_some() { format!("{} {}", crate::i18n::tr("cmd-vertex-picked"), ph::CHECK) } else { crate::i18n::tr("cmd-pick-vertex") };
                                ui.label(egui::RichText::new(r).color(self.scheme.pal.hint()));
                            } else {
                                ui.label(egui::RichText::new(&crate::i18n::tr("cmd-xyz-hint")).weak());
                            }
                        }
                        22 => {
                            // A datum AXIS: by a click on an edge or a face, by two points, or by hand (origin and direction at the geometry)
                            ui.label(&crate::i18n::tr("cmd-method"));
                            ui.selectable_value(&mut self.datum.axis_mode, 0u8, &crate::i18n::tr("cmd-by-edge-face"));
                            ui.selectable_value(&mut self.datum.axis_mode, 2u8, &crate::i18n::tr("cmd-two-points"));
                            ui.selectable_value(&mut self.datum.axis_mode, 1u8, &crate::i18n::tr("cmd-manual"));
                            ui.separator();
                            match self.datum.axis_mode {
                                0 => {
                                    let r = if self.datum.axis_ref.is_some() { format!("{} {}", crate::i18n::tr("cmd-axis-picked"), ph::CHECK) } else { crate::i18n::tr("cmd-pick-edge-cyl") };
                                    ui.label(egui::RichText::new(r).color(self.scheme.pal.hint()));
                                }
                                2 => {
                                    ui.label(egui::RichText::new(crate::i18n::tr1("cmd-points-n", "n", &self.datum.axis_pts.len().to_string())).color(self.scheme.pal.hint()));
                                }
                                _ => {
                                    ui.label(egui::RichText::new(&crate::i18n::tr("cmd-origin-dir-hint")).weak());
                                }
                            }
                        }
                        _ => {
                            ui.label(egui::RichText::new(self.cmd_hint()).weak());
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // the button is disabled while a dimension's expression is invalid (nothing stale is applied)
                        if ui.add_enabled(self.cmd_ready() && self.cmd_exprs_valid(), egui::Button::new(egui::RichText::new(format!("{} {}", ph::CHECK, crate::i18n::tr("cmd-apply-enter"))).strong())).clicked() {
                            apply = true;
                        }
                        if ui.button(&crate::i18n::tr("cmd-cancel-esc")).clicked() {
                            cancel = true;
                        }
                        // the "pick contours" button belongs to the 3D step (setting the size) only - inside the
                        // half-sketcher (mode_3d=false) contours are ALREADY being picked, so duplicating it there
                        // serves nothing.
                        if matches!(self.cmd.kind, 1 | 3) && self.cmd.sketch.is_some() && self.mode_3d {
                            // THE KEY IN THE HINT FOLLOWS THE FOCUS STATE: "U" or "Alt+U". Otherwise the rule that
                            // a focused field needs Alt would stay a secret, and someone who pressed `U` in a field
                            // and got nothing would not try a second time.
                            let hint = format!("{}  ({})", crate::i18n::tr("cmd-back-to-contours"), self.hotkey_hint(ctx, "part.contour-reselect"));
                            if ui.button(format!("{} {}", ph::POLYGON, crate::i18n::tr("cmd-pick-contours"))).on_hover_text(&hint).clicked() {
                                reselect = true;
                            }
                        }
                    });
                });
            });
            if apply {
                self.apply_feat_cmd();
            }
            if cancel {
                self.cancel_feat_cmd();
            }
            if reselect {
                self.enter_contour_reselect();
            }
            // after the bar (the extent mode is chosen) the "second side" field at the geometry is synced
            self.sync_dir_cmd_params();
            // the pattern: a second step when there is a second direction, and an angle when it is not a full circle
            self.sync_array_params();
            // the datum axis: the origin and direction fields in manual mode
            self.sync_datum_axis_params();
            self.sync_hole_params(); // the recess fields for a counterbore or a countersink
        }
    }

    /// THE SECTION CONTROL BAR: offset, tilts, flip, switching off.
    ///
    /// This is PANEL DRAWING rather than a phase of the frame - yet it used to sit in the middle of `update`,
    /// mixed in with the prologue, the keyboard and holding the selection. While panels live in the shared body,
    /// "what the frame does" cannot be told from "what it draws", and editing one touches the other.
    pub(super) fn section_bar(&mut self, ctx: &egui::Context) {
        // THE SECTION: the control bar (offset, tilts, flip, off) for as long as the section is active
        if self.section.plane.is_some() {
            let mut changed = false;
            let mut off = false;
            egui::TopBottomPanel::top("section_bar").frame(self.tool_bar_frame()).show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{} {}", ph::SQUARE_HALF, crate::i18n::tr("sec-btn"))).strong());
                    ui.separator();
                    ui.label(&crate::i18n::tr("sec-offset"));
                    changed |= ui.add(egui::DragValue::new(&mut self.section.offset).speed(0.5).suffix(crate::i18n::tr("unit-mm-suffix"))).changed();
                    ui.label(&crate::i18n::tr("sec-tilt1"));
                    changed |= ui.add(egui::DragValue::new(&mut self.section.rot[0]).speed(0.5).range(-89.0..=89.0).suffix("°")).changed();
                    ui.label(&crate::i18n::tr("sec-tilt2"));
                    changed |= ui.add(egui::DragValue::new(&mut self.section.rot[1]).speed(0.5).range(-89.0..=89.0).suffix("°")).changed();
                    if ui.button(format!("{} {}", ph::ARROWS_DOWN_UP, crate::i18n::tr("sec-flip-btn"))).on_hover_text(&crate::i18n::tr("sec-flip-hint")).clicked() {
                        if let Some((_, n)) = &mut self.section.plane {
                            n[0] = -n[0];
                            n[1] = -n[1];
                            n[2] = -n[2];
                        }
                        self.section.offset = -self.section.offset;
                        changed = true;
                    }
                    if ui.button(format!("{} {}", ph::X, crate::i18n::tr("sec-off-btn"))).clicked() {
                        off = true;
                    }
                });
            });
            if off {
                self.section.plane = None;
                changed = true;
            }
            if changed {
                self.invalidate();
            }
        }
    }

    pub(super) fn menu_bar(&mut self, ctx: &egui::Context) {
        // The menu items belonging to CAM (the machine, the tools, the G-code, the setup, the rapids) appear only
        // when the machining module is enabled. That module is under development and hidden by default.
        let cam = self.set.cam_tab_enabled;
        egui::TopBottomPanel::top("menubar").show(ctx, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            egui::menu::bar(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.menu_button(crate::i18n::tr("menu-file"), |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    if ui.button(format!("{}  {}", ph::FILE, crate::i18n::tr("file-new"))).clicked() {
                        self.request_nav(Nav::New, ctx);
                        ui.close_menu();
                    }
                    // TEMPLATES: the item is disabled rather than hidden, as the recent files are. An empty
                    // submenu explains itself, while a vanishing item leaves one guessing whether it ever existed.
                    let tpls = crate::templates::list();
                    ui.add_enabled_ui(!tpls.is_empty(), |ui| {
                        ui.menu_button(format!("{}  {}", ph::FILE_TEXT, crate::i18n::tr("file-new-from-template")), |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                            for (name, path) in &tpls {
                                if ui.button(name).on_hover_text(path).clicked() {
                                    self.request_nav(Nav::NewFromTemplate(path.clone()), ctx);
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                    if ui.button(format!("{}  {}", ph::PACKAGE, crate::i18n::tr("file-save-as-template"))).on_hover_text(&crate::i18n::tr("file-save-as-template-hint")).clicked() {
                        self.win.tpl_name = self.project.meta.title.clone();
                        self.win.save_template = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(format!("{}  {}", ph::FOLDER_OPEN, crate::i18n::tr("file-open"))).clicked() {
                        self.request_nav(Nav::OpenDialog, ctx);
                        ui.close_menu();
                    }
                    // RECENT FILES: a basic expectation of any program that has files. The item is disabled
                    // rather than hidden: an empty submenu explains itself, while a vanishing item leaves one
                    // guessing whether it ever existed.
                    let recent = self.set.recent.clone();
                    ui.add_enabled_ui(!recent.is_empty(), |ui| {
                        ui.menu_button(format!("{}  {}", ph::CLOCK_COUNTER_CLOCKWISE, crate::i18n::tr("file-recent")), |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                            for path in &recent {
                                // the row shows THE FILE NAME with the full path in the tooltip: paths are longer than the menu
                                let name = std::path::Path::new(path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
                                if ui.button(name).on_hover_text(path).clicked() {
                                    self.request_nav(Nav::OpenPath(path.clone()), ctx);
                                    ui.close_menu();
                                }
                            }
                            ui.separator();
                            if ui.button(format!("{}  {}", ph::TRASH, crate::i18n::tr("file-recent-clear"))).clicked() {
                                self.set.recent.clear();
                                ui.close_menu();
                            }
                        });
                    });
                    if ui.add(egui::Button::new(format!("{}  {}", ph::FLOPPY_DISK, crate::i18n::tr("file-save"))).shortcut_text("Ctrl+S")).clicked() {
                        self.save_project();
                        ui.close_menu();
                    }
                    if ui.button(format!("{}  {}", ph::FILE_TEXT, crate::i18n::tr("file-doc-props"))).clicked() {
                        self.win.doc_props = true;
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new(format!("{}  {}", ph::FLOPPY_DISK, crate::i18n::tr("file-save-as"))).shortcut_text("Ctrl+Shift+S")).clicked() {
                        self.save_project_as();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(format!("{}  {}", ph::FILE, crate::i18n::tr("file-import-dxf"))).clicked() {
                        self.pick_dxf();
                        ui.close_menu();
                    }
                    if ui.button(format!("{}  {}", ph::CUBE, crate::i18n::tr("file-import-stl"))).clicked() {
                        self.pick_stl();
                        ui.close_menu();
                    }
                    if ui.button(format!("{}  {}", ph::CUBE, crate::i18n::tr("file-import-step"))).clicked() {
                        self.pick_step();
                        ui.close_menu();
                    }
                    if ui.button(format!("{}  {}", ph::POLYGON, crate::i18n::tr("file-import-svg"))).clicked() {
                        self.pick_svg();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(format!("{}  {}", ph::EXPORT, crate::i18n::tr("file-export-step"))).on_hover_text(&crate::i18n::tr("menu-export-step-hint")).clicked() {
                        self.export_step(ExportTarget::Project);
                        ui.close_menu();
                    }
                    if ui.button(format!("{}  {}", ph::EXPORT, crate::i18n::tr("file-export-stl"))).on_hover_text(&crate::i18n::tr("menu-export-stl-hint")).clicked() {
                        self.stl_export = Some(ExportTarget::Project);
                        ui.close_menu();
                    }
                    if cam {
                        ui.separator();
                        if ui.add_enabled(self.cam_job.gcode.is_some(), egui::Button::new(format!("{}  {}", ph::EXPORT, crate::i18n::tr("menu-export-gcode")))).clicked() {
                            self.export();
                            ui.close_menu();
                        }
                        if ui.add_enabled(self.cam_job.gcode.is_some(), egui::Button::new(format!("{}  {}", ph::FILE_TEXT, crate::i18n::tr("menu-setup-sheet")))).clicked() {
                            self.export_setup_sheet();
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui.button(format!("{}  {}", ph::SIGN_OUT, crate::i18n::tr("file-quit"))).clicked() {
                        self.request_nav(Nav::Exit, ctx);
                    }
                });
                ui.menu_button(crate::i18n::tr("menu-edit"), |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    // THE OPERATION'S NAME IN THE MENU: what exactly will be undone is visible - a step knows its
                    // own name, because it was created by a command rather than by the frame.
                    let undo_label = match self.edits.undo.last() {
                        Some(s) => format!("{}  {}", ph::ARROW_COUNTER_CLOCKWISE, crate::i18n::tr1("menu-undo-named", "what", &s.name)),
                        None => format!("{}  {}", ph::ARROW_COUNTER_CLOCKWISE, crate::i18n::tr("menu-undo")),
                    };
                    if ui.add_enabled(!self.edits.undo.is_empty(), egui::Button::new(undo_label).shortcut_text("Ctrl+Z")).clicked() {
                        self.undo();
                        ui.close_menu();
                    }
                    let redo_label = match self.edits.redo.last() {
                        Some(s) => format!("{}  {}", ph::ARROW_CLOCKWISE, crate::i18n::tr1("menu-redo-named", "what", &s.name)),
                        None => format!("{}  {}", ph::ARROW_CLOCKWISE, crate::i18n::tr("menu-redo")),
                    };
                    if ui.add_enabled(!self.edits.redo.is_empty(), egui::Button::new(redo_label).shortcut_text("Ctrl+Shift+Z")).clicked() {
                        self.redo();
                        ui.close_menu();
                    }
                    ui.separator();
                    // The clipboard: sketches, parts and subassemblies in the tree, or geometry in the sketch editor.
                    let can_copy = self.clipboard_can_copy();
                    let can_paste = self.clip.tree.is_some() || self.clip.geom.is_some();
                    if ui.add_enabled(can_copy, egui::Button::new(format!("{}  {}", ph::COPY, crate::i18n::tr("menu-copy"))).shortcut_text("Ctrl+C")).clicked() {
                        self.clipboard_copy(false);
                        ui.close_menu();
                    }
                    if ui.add_enabled(can_copy, egui::Button::new(format!("{}  {}", ph::SCISSORS, crate::i18n::tr("menu-cut"))).shortcut_text("Ctrl+X")).clicked() {
                        self.clipboard_copy(true);
                        ui.close_menu();
                    }
                    if ui.add_enabled(can_paste, egui::Button::new(format!("{}  {}", ph::CLIPBOARD, crate::i18n::tr("win-insert"))).shortcut_text("Ctrl+V")).clicked() {
                        self.clipboard_paste();
                        ui.close_menu();
                    }
                    ui.separator();
                    // REBUILD EVERYTHING. The file stores finished meshes and computes nothing anew on opening -
                    // that is fast, but it means a part built by an older version of the kernel stays as it was.
                    // Reported behaviour: a thread profile was fixed, the CAD restarted, and the same ragged part
                    // appeared - nobody had recomputed its mesh. Without this command, fixing that meant poking
                    // every feature by hand.
                    if ui
                        .add_enabled(!self.project.timeline.is_empty(), egui::Button::new(format!("{}  {}", ph::ARROWS_CLOCKWISE, crate::i18n::tr("menu-rebuild"))))
                        .on_hover_text(&crate::i18n::tr("menu-rebuild-hint"))
                        .clicked()
                    {
                        self.rebuild_everything();
                        ui.close_menu();
                    }
                });
                ui.menu_button(crate::i18n::tr("menu-view"), |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    ui.checkbox(&mut self.mode_3d, format!("{}  {}", ph::CUBE, crate::i18n::tr("menu-orbit3d")));
                    if cam {
                        ui.checkbox(&mut self.set.show_rapids, &crate::i18n::tr("settings-rapids-short"));
                    }
                    if ui.button(format!("{}  {}", ph::CORNERS_OUT, crate::i18n::tr("menu-fit-view"))).clicked() {
                        self.view.initialized = false;
                        self.cam.init = false;
                        ui.close_menu();
                    }
                    if cam && ui.add_enabled(self.cam_job.gcode.is_some(), egui::Button::new(format!("{}  {}", ph::CODE, crate::i18n::tr("menu-gcode-window")))).clicked() {
                        self.win.gcode = !self.win.gcode;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.label(crate::i18n::tr("settings-scheme"));
                    // THE SCHEMES COME FROM THE LIVE LIST (the built-in ones and the user's), and the label from
                    // `title()`: a built-in scheme has NO name of its own, it comes from the language catalogue.
                    // This used to be `p.name`, and after the identifier and the label were separated the menu
                    // items were left as bare icons with no words.
                    let rows: Vec<(String, String, bool)> = self.scheme.all.iter().map(|p| (p.id.clone(), p.title(), p.light)).collect();
                    for (id, title, light) in rows {
                        let icon = if crate::palette::store::is_builtin(&id) {
                            if light { ph::SUN } else { ph::MOON }
                        } else {
                            ph::PENCIL_SIMPLE
                        };
                        if ui.button(format!("{icon}  {title}")).clicked() {
                            self.set.scheme = id.clone();
                            self.apply_theme(ctx);
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button(crate::i18n::tr("menu-windows"), |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    if ui.button(format!("{}  {}", ph::GEAR, crate::i18n::tr("win-settings"))).clicked() {
                        self.win.settings = !self.win.settings;
                        ui.close_menu();
                    }
                    if ui.button(format!("{}  {}", ph::PACKAGE, crate::i18n::tr("win-parts-library"))).clicked() {
                        self.toggle_parts_library();
                        ui.close_menu();
                    }
                    if ui.button(format!("{}  {}", ph::HOUSE, crate::i18n::tr("win-start"))).clicked() {
                        self.win.start_asked = true; // it was ASKED for rather than raising itself - see `start_screen_visible`
                        ui.close_menu();
                    }
                    if cam {
                        if ui.button(format!("{}  {}", ph::WRENCH, crate::i18n::tr("menu-machine"))).clicked() {
                            self.win.machines = !self.win.machines;
                            ui.close_menu();
                        }
                        if ui.button(format!("{}  {}", ph::SCREWDRIVER, crate::i18n::tr("menu-tools"))).clicked() {
                            self.win.tools = !self.win.tools;
                            ui.close_menu();
                        }
                        if ui.add_enabled(self.cam_job.gcode.is_some(), egui::Button::new(format!("{}  G-code", ph::CODE))).clicked() {
                            self.win.gcode = !self.win.gcode;
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button(crate::i18n::tr("menu-help"), |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    if ui.button(format!("{} {}", ph::BOOK_OPEN, crate::i18n::tr("help-title"))).clicked() {
                        self.open_help("index");
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(format!("{} {}", ph::KEYBOARD, crate::i18n::tr("help-hotkeys"))).clicked() {
                        self.win.hotkeys = true;
                        ui.close_menu();
                    }
                    if ui.button(format!("{} {}", ph::BUG, crate::i18n::tr("help-report"))).clicked() {
                        self.win.report = true;
                        ui.close_menu();
                    }
                    if ui.button(format!("{} {}", ph::INFO, crate::i18n::tr("help-about"))).clicked() {
                        self.win.about = true;
                        ui.close_menu();
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    if let Some(p) = &self.dxf_path {
                        let fname = std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                        ui.label(egui::RichText::new(format!("{} {fname}", ph::FILE)).weak());
                    }
                });
            });
        });
    }

    pub(super) fn toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // CONTEXT BREADCRUMBS instead of tabs (drilling in and out): Assembly > Part > ... (> Sketch)
                self.ensure_active_path();
                let path = self.active_path.clone();
                for (i, cid) in path.iter().enumerate() {
                    if i > 0 {
                        ui.label("›");
                    }
                    let name = self.project.components.iter().find(|c| c.id == *cid).map(|c| crate::i18n::name(&c.name)).unwrap_or_else(|| crate::i18n::tr("wb-assembly"));
                    let icon = if i == 0 { ph::STACK } else if self.project.component_kind(*cid) == Some(qymcad_core::feature::ComponentKind::Assembly) { ph::STACK } else { ph::CUBE };
                    let is_here = i + 1 == path.len() && self.sketch_ses.editing.is_none() && !self.cam_mode;
                    if ui.selectable_label(is_here, format!("{icon} {name}")).clicked() {
                        self.goto_path_index(i);
                    }
                }
                if let Some(sid) = self.sketch_ses.editing {
                    ui.label("›");
                    let nm = self.project.sketches.iter().find(|s| s.id == sid).map(|s| crate::i18n::name(&s.name)).unwrap_or_else(|| crate::i18n::tr("wb-sketch"));
                    ui.label(egui::RichText::new(format!("{} {nm}", ph::PENCIL)).color(self.scheme.pal.hint()));
                }
                if self.sketch_ses.editing.is_some() || self.active_path.len() > 1 {
                    if ui.button(&crate::i18n::tr("wb-finish")).on_hover_text(&crate::i18n::tr("wb-finish-hint")).clicked() {
                        self.exit_context();
                    }
                }
                ui.separator();
                // the global parameters are reachable from any workbench
                if ui.selectable_label(self.win.params, &crate::i18n::tr("wb-params")).on_hover_text(&crate::i18n::tr("wb-params-hint")).clicked() {
                    self.win.params = !self.win.params;
                }
                // CAM is a global mode, outside the drill stack. The tab appears only when it is enabled in the
                // settings (the module is under development). If it gets disabled, CAM is left by force.
                if self.set.cam_tab_enabled {
                    ui.separator();
                    if ui.selectable_label(self.cam_mode, format!("{} {}", ph::GEAR, crate::i18n::tr("cam-tab"))).on_hover_text(&crate::i18n::tr("cam-tab-hint")).clicked() {
                        self.cam_mode = !self.cam_mode;
                        self.sync_workbench();
                    }
                } else if self.cam_mode {
                    self.cam_mode = false;
                    self.sync_workbench();
                }
                if !self.cam_mode && matches!(self.sel, Sel::Op(_) | Sel::Setup(_) | Sel::Tool(_) | Sel::Machine | Sel::Stock) {
                    self.sel = Sel::None;
                }
                ui.separator();
                let cam = self.cam_mode;

                if cam {
                    // --- the CAM tools ---
                    if ui.button(format!("{} {}", ph::GEAR, crate::i18n::tr("cam-generate"))).clicked() {
                        self.generate();
                    }
                    if ui.add_enabled(self.cam_job.gcode.is_some(), egui::Button::new(ph::EXPORT)).on_hover_text(&crate::i18n::tr("cam-export-gcode")).clicked() {
                        self.export();
                    }
                    if ui.add_enabled(self.cam_job.gcode.is_some(), egui::Button::new(ph::CODE)).on_hover_text(&crate::i18n::tr("cam-gcode-window")).clicked() {
                        self.win.gcode = !self.win.gcode;
                    }
                    if ui.selectable_label(self.win.sim, format!("{} {}", ph::CUBE, crate::i18n::tr("cam-simulation"))).on_hover_text(&crate::i18n::tr("cam-simulation-hint")).clicked() {
                        self.toggle_sim();
                    }
                    ui.separator();
                } else if let Some(pts) = self.pending_import.draw_pts.as_ref() {
                    // a sketch is being drawn: finish or cancel
                    let n = pts.len();
                    ui.label(crate::i18n::tr1("wb-points-n", "n", &n.to_string()));
                    if ui.button(&crate::i18n::tr("wb-close")).clicked() {
                        self.finish_drawing(true);
                    }
                    if ui.button(&crate::i18n::tr("wb-line")).clicked() {
                        self.finish_drawing(false);
                    }
                    if ui.button(&crate::i18n::tr("wb-cancel")).clicked() {
                        self.pending_import.draw_pts = None;
                    }
                    ui.separator();
                }

                // the manual 2D/3D switch belongs to the CAM workbench only: in CAD the view is automatic
                // (2D while editing a sketch, 3D otherwise), and the buttons only get in the way.
                if self.cam_mode {
                    ui.selectable_value(&mut self.mode_3d, false, format!("{} 2D", ph::GRID_FOUR));
                    ui.selectable_value(&mut self.mode_3d, true, format!("{} 3D", ph::CUBE));
                    ui.separator();
                }
                // Snap and the grid: while DRAWING a sketch (the cursor) AND in 3D inside a Part or an Assembly (snapping the gizmo to the grid).
                let snap_ctx = self.sketch_ses.editing.is_some() || (self.mode_3d && matches!(self.workbench, Workbench::Part | Workbench::Assembly));
                if snap_ctx {
                    ui.toggle_value(&mut self.set.snap.on, format!("{} Snap", ph::MAGNET)).on_hover_text(&crate::i18n::tr("wb-snap-hint"));
                    if self.set.snap.on {
                        ui.add(egui::DragValue::new(&mut self.set.snap.grid).speed(0.5).range(0.1..=100.0).prefix(&crate::i18n::tr("wb-grid")).suffix(crate::i18n::tr("unit-mm-suffix")));
                        // the gizmo's ROTATION step belongs to 3D inside a Part or an Assembly (a sketch has no use for it)
                        if self.mode_3d && matches!(self.workbench, Workbench::Part | Workbench::Assembly) {
                            ui.add(
                                egui::DragValue::new(&mut self.set.snap.rot_deg)
                                    .speed(1.0)
                                    .range(0.5..=90.0)
                                    .prefix(&crate::i18n::tr("wb-rotation"))
                                    .suffix(crate::i18n::tr("unit-deg-suffix")),
                            );
                        }
                    }
                    // the automatic constraints belong to drawing a sketch only
                    if self.sketch_ses.editing.is_some() {
                        ui.toggle_value(&mut self.set.auto_constrain, format!("{} {}", ph::MAGIC_WAND, crate::i18n::tr("wb-auto-constraints"))).on_hover_text(&crate::i18n::tr("wb-auto-constraints-hint"));
                    }
                    ui.separator();
                }
                // "in context" applies inside ANY component (a part or a subassembly), not at the root: top-down
                // references to the neighbours + showing the PARENT datums. The root has no ancestors, so it is hidden there.
                if self.current_ctx_id() != self.project.root {
                    ui.toggle_value(&mut self.win.context, format!("{} {}", ph::STACK, crate::i18n::tr("wb-in-context"))).on_hover_text(&crate::i18n::tr("wb-in-context-hint"));
                    ui.separator();
                }
                if cam {
                    ui.separator();
                    ui.label(&crate::i18n::tr("cam-post"));
                    egui::ComboBox::from_id_salt("posttb")
                        .selected_text(self.project.machine.post.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.project.machine.post, PostKind::Mach3, "Mach3");
                            ui.selectable_value(&mut self.project.machine.post, PostKind::Grbl, "GRBL");
                            ui.selectable_value(&mut self.project.machine.post, PostKind::LinuxCnc, "LinuxCNC");
                        });
                }
            });
        });
    }

    pub(super) fn wb_toolbar(&mut self, ctx: &egui::Context) {
        // Two columns of buttons (wrapping by width) + vertical scrolling - reliable at any window size.
        // The width 94 = 16 (the 8+8 margins) + 6 (the floating scrollbar) + 72 (two 34-wide button columns + a
        // 3-point gap + a margin). Before that, 90 with 38-wide buttons could not fit the second column, giving
        // one column and a wide empty strip on the right.
        // show_separator_line(false): by default egui draws a faint vertical line at the right edge of ANY panel,
        // and it read as a stray gap between the tools and the tree.
        egui::SidePanel::left("wbtools").exact_width(108.0).resizable(false).show_separator_line(false).show(ctx, |ui| {
            ui.add_space(6.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                ui.horizontal_wrapped(|ui| {
                match self.workbench {
                    Workbench::Sketch => {
                        // a full-width category label, which therefore starts a new row (breaking the columns)
                        let cat = |ui: &mut egui::Ui, s: &str| {
                            ui.add_sized([84.0, 12.0], egui::Label::new(egui::RichText::new(s).size(8.5).weak()));
                        };
                        // "Finish" lives in the breadcrumbs (one place for it); here there are only sketch tools
                        // --- Creation ---
                        cat(ui, &crate::i18n::tr("tb-group-create"));
                        if Self::icon_tool(ui, ph::CURSOR, &crate::i18n::tr("tb-select-hint"), self.in_select_mode()) {
                            self.sketch_select_mode();
                        }
                        if Self::icon_tool(ui, ph::DOT, &crate::i18n::tr("tb-point-hint"), self.tool.kind == 5) {
                            self.set_sk_tool(5);
                        }
                        if Self::icon_tool(ui, ph::LINE_SEGMENT, &crate::i18n::tr("tb-line-hint"), self.tool.kind == 1) {
                            self.set_sk_tool(1);
                        }
                        if Self::icon_tool(ui, ph::RECTANGLE, &crate::i18n::tr("tb-rect-hint"), self.tool.kind == 2) {
                            self.set_sk_tool(2);
                        }
                        if Self::icon_tool(ui, ph::CIRCLE, &crate::i18n::tr("tb-circle-hint"), self.tool.kind == 3) {
                            self.set_sk_tool(3);
                        }
                        if sym_button(ui, Gly::Circle3, &crate::i18n::tr("tb-circle-3pt"), self.tool.kind == 10) {
                            self.set_sk_tool(10);
                        }
                        if Self::icon_tool(ui, ph::PATH, &crate::i18n::tr("tb-arc-hint"), self.tool.kind == 4) {
                            self.set_sk_tool(4);
                        }
                        if Self::icon_tool(ui, ph::HEXAGON, &crate::i18n::tr("tb-polygon-hint"), self.tool.kind == 6) {
                            self.set_sk_tool(6);
                        }
                        if Self::icon_tool(ui, ph::PILL, &crate::i18n::tr("tb-slot-hint"), self.tool.kind == 7) {
                            self.set_sk_tool(7);
                        }
                        if sym_button(ui, Gly::Ellipse, &crate::i18n::tr("tb-ellipse-hint"), self.tool.kind == 8) {
                            self.set_sk_tool(8);
                        }
                        if sym_button(ui, Gly::Spline, &crate::i18n::tr("tb-spline-hint"), self.tool.kind == 9) {
                            self.set_sk_tool(9);
                        }
                        if sym_button(ui, Gly::Text, &crate::i18n::tr("tb-text-hint"), self.tool.kind == 11) {
                            self.set_sk_tool(11);
                        }
                        // --- The line kind ---
                        cat(ui, &crate::i18n::tr("tb-type"));
                        if sym_button(ui, Gly::Construction, &crate::i18n::tr("tb-construction-hint"), self.tool.construction) {
                            self.tool.construction = !self.tool.construction;
                        }
                        // --- Editing and replication (over the selected entities) ---
                        cat(ui, &crate::i18n::tr("tb-group-edit"));
                        if Self::icon_tool(ui, ph::TRASH, &crate::i18n::tr("tb-delete-hint"), self.sel_sk.modify == Some(0)) {
                            self.modify_button(0);
                        }
                        if sym_button(ui, Gly::Mirror, &crate::i18n::tr("tb-mirror-sketch-hint"), self.sel_sk.modify == Some(1)) {
                            self.modify_button(1);
                        }
                        if sym_button(ui, Gly::ArrayLin, &crate::i18n::tr("tb-lin-array-hint"), self.pat.op == 1) {
                            self.start_pattern(1);
                        }
                        if sym_button(ui, Gly::ArrayCirc, &crate::i18n::tr("tb-circ-array-hint"), self.pat.op == 2) {
                            self.start_pattern(2);
                        }
                        if Self::icon_tool(
                            ui,
                            ph::PROJECTOR_SCREEN,
                            &crate::i18n::tr("tb-project-body-hint"),
                            self.tool.click_op == 6,
                        ) {
                            self.set_click_op(6);
                            self.status = crate::i18n::tr("tb-project-hint");
                        }
                        if sym_button(ui, Gly::Fillet, &crate::i18n::tr("tb-fillet-sketch-hint"), self.tool.click_op == 4) {
                            self.set_click_op(4);
                        }
                        if sym_button(ui, Gly::Chamfer, &crate::i18n::tr("tb-chamfer-sketch-hint"), self.tool.click_op == 5) {
                            self.set_click_op(5);
                        }
                        if Self::icon_tool(ui, ph::BOUNDING_BOX, &crate::i18n::tr("tb-fillet-all-hint"), false) {
                            self.fillet_all_corners();
                        }
                        if sym_button(ui, Gly::Offset, &crate::i18n::tr("tb-offset-hint"), self.sel_sk.modify == Some(6)) {
                            self.modify_button(6);
                        }
                        if Self::icon_tool(ui, ph::ARROWS_OUT_CARDINAL, &crate::i18n::tr("tb-move-hint"), self.tool.move_op == 1) {
                            self.start_move_tool(1);
                        }
                        if Self::icon_tool(ui, ph::COPY, &crate::i18n::tr("tb-copy-hint"), self.tool.move_op == 2) {
                            self.start_move_tool(2);
                        }
                        if Self::icon_tool(ui, ph::ARROWS_CLOCKWISE, &crate::i18n::tr("tb-rotate-hint"), self.tool.move_op == 3) {
                            self.start_move_tool(3);
                        }
                        if sym_button(ui, Gly::Trim, &crate::i18n::tr("tb-trim-hint"), self.tool.click_op == 1) {
                            self.set_click_op(1);
                        }
                        if sym_button(ui, Gly::Extend, &crate::i18n::tr("tb-extend-hint"), self.tool.click_op == 2) {
                            self.set_click_op(2);
                        }
                        if sym_button(ui, Gly::Break, &crate::i18n::tr("tb-break-hint"), self.tool.click_op == 3) {
                            self.set_click_op(3);
                        }
                        // --- Dimensions ---
                        cat(ui, &crate::i18n::tr("tb-group-dim"));
                        if sym_button(ui, Gly::DimLin, &crate::i18n::tr("tb-dim-hint"), self.dim.kind == 1) {
                            self.set_dim_tool(1);
                        }
                        if sym_button(ui, Gly::DimAng, &crate::i18n::tr("tb-dim-angle-hint"), self.dim.kind == 2) {
                            self.set_dim_tool(2);
                        }
                        if sym_button(ui, Gly::DimRad, &crate::i18n::tr("tb-dim-radius-hint"), self.dim.kind == 3) {
                            self.set_dim_tool(3);
                        }
                        if Self::icon_tool(ui, ph::RULER, &crate::i18n::tr("tb-measure-hint"), self.measure.on) {
                            let on = !self.measure.on;
                            self.sketch_select_mode();
                            self.measure.on = on;
                            self.measure.pts.clear();
                            if on {
                                self.mode_3d = false;
                            }
                        }
                        // --- Constraints ---
                        cat(ui, &crate::i18n::tr("tb-group-constraints"));
                        let cons: [(Gly, u8, &str); 12] = [
                            (Gly::Coincident, 0, &crate::i18n::tr("con-coincident-hint")),
                            (Gly::Horiz, 1, &crate::i18n::tr("con-horizontal-hint")),
                            (Gly::Vert, 2, &crate::i18n::tr("con-vertical-hint")),
                            (Gly::Parallel, 3, &crate::i18n::tr("con-parallel-hint")),
                            (Gly::Perp, 4, &crate::i18n::tr("con-perpendicular-hint")),
                            (Gly::Equal, 5, &crate::i18n::tr("con-equal")),
                            (Gly::Collinear, 7, &crate::i18n::tr("con-collinear-hint")),
                            (Gly::Concentric, 8, &crate::i18n::tr("con-concentric-hint")),
                            (Gly::Tangent, 9, &crate::i18n::tr("con-tangent-hint")),
                            (Gly::Symmetric, 10, &crate::i18n::tr("con-symmetric-hint")),
                            (Gly::Midpoint, 11, &crate::i18n::tr("con-midpoint-hint")),
                            (Gly::Fix, 6, &crate::i18n::tr("con-fix")),
                        ];
                        for (g, code, tip) in cons {
                            if sym_button(ui, g, tip, self.sel_sk.constraint == Some(code)) {
                                self.constraint_button(code);
                            }
                        }
                    }
                    Workbench::Part => {
                        // a full-width category label, breaking the columns (as in the Sketch)
                        let cat = |ui: &mut egui::Ui, s: &str| {
                            ui.add_sized([84.0, 12.0], egui::Label::new(egui::RichText::new(s).size(8.5).weak()));
                        };
                        // --- Create (a sketch + the datums) ---
                        cat(ui, &crate::i18n::tr("tb-group-create"));
                        self.create_panel_sketch_button(ui);
                        self.create_panel_common(ui);
                        // --- From a sketch ---
                        cat(ui, &crate::i18n::tr("tb-group-sketch3d"));
                        if Self::icon_tool(ui, ph::CUBE, &crate::i18n::tr("tb-extrude-hint"), self.cmd.kind == 1) {
                            self.feat.op = 0;
                            self.start_feat_cmd(1);
                        }
                        if Self::icon_tool(ui, ph::ARROWS_CLOCKWISE, &crate::i18n::tr("tb-revolve-hint"), self.cmd.kind == 3) {
                            self.start_feat_cmd(3);
                        }
                        if Self::icon_tool(ui, ph::PATH, &crate::i18n::tr("tb-sweep-hint"), self.cmd.kind == 8) {
                            self.start_feat_cmd(8);
                        }
                        if Self::icon_tool(ui, ph::STACK, &crate::i18n::tr("tb-loft-hint"), self.cmd.kind == 9) {
                            self.start_feat_cmd(9);
                        }
                        // --- The 3D primitives (a command: sizes at the geometry + a preview + Enter/Esc) ---
                        cat(ui, &crate::i18n::tr("tb-group-prim"));
                        if Self::icon_tool(ui, ph::CUBE_TRANSPARENT, &crate::i18n::tr("tb-box-hint"), self.cmd.kind == 10) {
                            self.start_prim_cmd(10);
                        }
                        if Self::icon_tool(ui, ph::CYLINDER, &crate::i18n::tr("tb-cylinder-hint"), self.cmd.kind == 11) {
                            self.start_prim_cmd(11);
                        }
                        if Self::icon_tool(ui, ph::SPHERE, &crate::i18n::tr("tb-sphere-hint"), self.cmd.kind == 12) {
                            self.start_prim_cmd(12);
                        }
                        if Self::icon_tool(ui, ph::TRAFFIC_CONE, &crate::i18n::tr("tb-cone-hint"), self.cmd.kind == 13) {
                            self.start_prim_cmd(13);
                        }
                        if Self::icon_tool(ui, ph::CIRCLE_NOTCH, &crate::i18n::tr("tb-torus-hint"), self.cmd.kind == 14) {
                            self.start_prim_cmd(14);
                        }
                        if Self::icon_tool(ui, ph::HEXAGON, &crate::i18n::tr("tb-prism-hint"), self.cmd.kind == 15) {
                            self.start_prim_cmd(15);
                        }
                        // --- Operations on a body ---
                        cat(ui, &crate::i18n::tr("tb-body"));
                        if Self::icon_tool(ui, ph::CIRCLE_HALF, &crate::i18n::tr("tb-fillet-body-hint"), self.cmd.kind == 4) {
                            self.start_feat_cmd(4);
                        }
                        if Self::icon_tool(ui, ph::TRIANGLE, &crate::i18n::tr("tb-chamfer-body-hint"), self.cmd.kind == 5) {
                            self.start_feat_cmd(5);
                        }
                        if Self::icon_tool(ui, ph::BOUNDING_BOX, &crate::i18n::tr("tb-shell-hint"), self.cmd.kind == 6) {
                            self.start_feat_cmd(6);
                        }
                        if Self::icon_tool(ui, ph::SQUARE_HALF, &crate::i18n::tr("tb-section-hint-bar"), self.section.pick || self.section.plane.is_some()) {
                            self.toggle_section();
                        }
                        if Self::icon_tool(ui, ph::CIRCLE, &crate::i18n::tr("tb-hole-hint"), self.cmd.kind == 7) {
                            self.start_feat_cmd(7);
                        }
                        if Self::icon_tool(ui, ph::ANGLE, &crate::i18n::tr("tb-draft-hint"), self.cmd.kind == 23) {
                            self.start_feat_cmd(23);
                        }
                        if Self::icon_tool(ui, ph::ARROWS_OUT_LINE_VERTICAL, &crate::i18n::tr("tb-push-face-hint"), self.cmd.kind == 25) {
                            self.start_feat_cmd(25);
                        }
                        if Self::icon_tool(ui, ph::ERASER, &crate::i18n::tr("tb-remove-face-hint"), self.cmd.kind == 26) {
                            self.start_feat_cmd(26);
                        }
                        if Self::icon_tool(ui, ph::STACK_SIMPLE, &crate::i18n::tr("tb-thicken-hint"), self.cmd.kind == 28) {
                            self.start_feat_cmd(28);
                        }
                        // the bridge from the parametric side into the design layer: a face becomes a surface
                        if Self::icon_tool(ui, ph::COPY_SIMPLE, &crate::i18n::tr("tb-face-copy-hint"), self.cmd.kind == 30) {
                            self.start_feat_cmd(30);
                        }
                        // the far end of that bridge: a surface goes back into the body
                        if Self::icon_tool(ui, ph::SWAP, &crate::i18n::tr("tb-surface-replace-hint"), self.cmd.kind == 31) {
                            self.start_feat_cmd(31);
                        }
                        // the first shape the body did not have: a surface built from the edges
                        if Self::icon_tool(ui, ph::BANDAIDS, &crate::i18n::tr("tb-patch-hint"), self.cmd.kind == 32) {
                            self.start_feat_cmd(32);
                        }
                        // pieces of surface become one, and a shell that closes becomes a body
                        if Self::icon_tool(ui, ph::INTERSECT_SQUARE, &crate::i18n::tr("tb-stitch-hint"), self.cmd.kind == 33) {
                            self.start_feat_cmd(33);
                        }
                        // trim a surface with the neighbouring geometry
                        if Self::icon_tool(ui, ph::SCISSORS, &crate::i18n::tr("tb-trim-surface-hint"), self.cmd.kind == 34) {
                            self.start_feat_cmd(34);
                        }
                        if Self::icon_tool(ui, ph::SQUARE_SPLIT_HORIZONTAL, &crate::i18n::tr("tb-split-body-hint"), self.cmd.kind == 27) {
                            self.start_feat_cmd(27);
                        }
                        if Self::icon_tool(
                            ui,
                            ph::RULER,
                            &crate::i18n::tr("tb-measure3d-hint"),
                            self.m3.on,
                        ) {
                            self.toggle_measure_3d();
                        }
                        if Self::icon_tool(ui, ph::GRID_FOUR, &crate::i18n::tr("tb-split-face-hint"), self.cmd.kind == 29) {
                            self.start_feat_cmd(29);
                        }
                        if Self::icon_tool(ui, ph::SPIRAL, &crate::i18n::tr("tb-thread-hint"), self.cmd.kind == 24) {
                            self.start_feat_cmd(24);
                        }
                        if Self::icon_tool(ui, ph::INTERSECT, &crate::i18n::tr("tb-bool-bodies-hint"), self.boolean.pick.is_some()) {
                            if let Some(a) = self.selected_body() {
                                self.boolean.pick = Some((a, 0));
                                self.status = crate::i18n::tr("tb-bool-pick-b");
                            } else {
                                self.boolean.pick = None;
                                self.status = crate::i18n::tr("tb-pick-body-a-first");
                            }
                        }
                        // THE MIRROR AND THE BODY PATTERNS LIVE HERE, UNDER "BODY". A separate "Patterns" category
                        // used to stand beside it showing the same two icons as "Body" above: those were COMPONENT
                        // patterns (an assembly tool), these are BODY patterns. What that looked like was
                        // duplicates - and it was worse than duplicates, because two buttons that looked alike did
                        // different things. The component ones moved into the Assembly, where they belong.
                        if Self::icon_tool(ui, ph::FLIP_HORIZONTAL, &crate::i18n::tr("tb-mirror-body-hint"), self.cmd.kind == 16) {
                            self.start_feat_cmd(16);
                        }
                        if Self::icon_tool(ui, ph::DOTS_THREE_OUTLINE, &crate::i18n::tr("tb-lin-array-body-hint"), self.cmd.kind == 17) {
                            self.start_feat_cmd(17);
                        }
                        if Self::icon_tool(ui, ph::CIRCLES_THREE, &crate::i18n::tr("tb-circ-array-body-hint"), self.cmd.kind == 18) {
                            self.start_feat_cmd(18);
                        }
                    }
                    Workbench::Assembly => {
                        let cat = |ui: &mut egui::Ui, s: &str| {
                            ui.add_sized([84.0, 12.0], egui::Label::new(egui::RichText::new(s).size(8.5).weak()));
                        };
                        // --- Create ---
                        cat(ui, &crate::i18n::tr("tb-group-create"));
                        self.create_panel_common(ui); // the datums; a sketch is inert in an assembly, so it was removed
                        if Self::icon_tool(ui, ph::CUBE, &crate::i18n::tr("tb-new-part-hint"), false) {
                            let id = self.project.add_part(crate::i18n::tr1("node-part-n", "n", &self.project.components.len().to_string()));
                            self.enter_component(id);
                        }
                        if Self::icon_tool(ui, ph::STACK, &crate::i18n::tr("tb-new-subassembly-hint"), false) {
                            let id = self.project.add_assembly(crate::i18n::tr1("node-assembly-n", "n", &self.project.components.len().to_string()));
                            self.enter_component(id);
                        }
                        if Self::icon_tool(ui, ph::FILE, &crate::i18n::tr("tb-insert-component-hint"), false) {
                            self.pick_step();
                        }
                        // COMPONENT PATTERNS ARE AN ASSEMBLY TOOL. They used to sit in the PART workbench, where
                        // there are no components, and there they duplicated the look of the body patterns.
                        if Self::icon_tool(ui, ph::DOTS_THREE_OUTLINE, &crate::i18n::tr("tb-comp-lin-array-hint"), self.carr.mode == 1) {
                            self.start_comp_array(1);
                        }
                        if Self::icon_tool(ui, ph::CIRCLES_THREE, &crate::i18n::tr("tb-comp-circ-array-hint"), self.carr.mode == 2) {
                            self.start_comp_array(2);
                        }
                        if Self::icon_tool(ui, ph::FLIP_HORIZONTAL, &crate::i18n::tr("tb-mirror-part-hint"), self.mirror.part.is_some()) {
                            // both a PART and a SUBASSEMBLY are accepted (the whole subtree is mirrored)
                            let src = match self.sel {
                                Sel::Component(ci) => self.project.components.get(ci).map(|c| c.id),
                                _ => self.selected_body().and_then(|b| self.project.body_owner(b)),
                            };
                            match src {
                                Some(comp) => {
                                    self.cancel_all_tools();
                                    self.mirror.part = Some(comp);
                                    self.status = crate::i18n::tr("tb-mirror-pick-plane");
                                }
                                None => self.status = crate::i18n::tr("tb-pick-part-first"),
                            }
                        }
                        if Self::icon_tool(ui, ph::SQUARE_HALF, &crate::i18n::tr("tb-section-hint"), self.section.pick || self.section.plane.is_some()) {
                            if self.section.plane.is_some() || self.section.pick {
                                self.section.plane = None;
                                self.section.pick = false;
                                self.section.drag = false;
                                self.section.drag_anchor = None;
                                self.invalidate();
                                self.status = crate::i18n::tr("tb-section-off");
                            } else {
                                self.cancel_all_tools();
                                self.section.pick = true;
                                self.status = crate::i18n::tr("tb-section-pick");
                            }
                        }
                        // --- The mates: buttons per mate kind, as in the sketcher. Clicking a kind starts the
                        // face pick with that kind (face A, then face B). The list and the editing of existing ones
                        // live in the properties panel on the right, as the sketch dimensions do.
                        cat(ui, &crate::i18n::tr("tb-group-joint"));
                        {
                            // ONE BUTTON, with the kind chosen in the top bar. There used to be seven buttons, one
                            // per kind, while the kind was changed by a combo box in that same bar anyway: the
                            // choice was made twice and took up the whole category.
                            let tip = crate::i18n::tr1("jt-joint-tip", "kind", &crate::i18n::tr(self.joint.new_kind.label()));
                            if Self::icon_tool(ui, ph::MAGNET, &tip, self.joint.pick_faces) {
                                self.start_joint_pick();
                            }
                            // the Ground tool: a click on a part fixes or releases it.
                            if Self::icon_tool(ui, ph::ANCHOR, &crate::i18n::tr("tb-ground-hint"), self.joint.ground_pick) {
                                self.start_ground_pick();
                            }
                            // Group: fasten a set of parts to one another without pairwise joints.
                            if Self::icon_tool(ui, ph::SELECTION_ALL, &crate::i18n::tr("j-group-tip"), self.joint.group_pick.is_some()) {
                                self.start_group_pick();
                            }
                            // Width: place a part midway between two walls.
                            if Self::icon_tool(ui, ph::ARROWS_OUT_LINE_HORIZONTAL, &crate::i18n::tr("j-width-tip"), self.joint.width_pick.is_some()) {
                                self.start_width_pick();
                            }
                            // Tangent: lay a cylinder onto a plane.
                            if Self::icon_tool(ui, ph::CIRCLE_HALF_TILT, &crate::i18n::tr("j-tangent-tip"), self.joint.tangent_pick.is_some()) {
                                self.start_tangent_pick();
                            }
                            // Relation: tie the degrees of freedom of two mates together.
                            if Self::icon_tool(ui, ph::GEAR_SIX, &crate::i18n::tr("j-relation-tip"), self.joint.relation_pick.is_some()) {
                                self.start_relation_pick();
                            }
                        }
                    }
                    Workbench::Cam => {
                        if Self::icon_tool(ui, ph::GEAR, &crate::i18n::tr("tb-gcode-generate"), false) {
                            self.generate();
                        }
                        if Self::icon_tool(ui, ph::CUBE, &crate::i18n::tr("tb-simulation"), self.win.sim) {
                            self.toggle_sim();
                        }
                        if Self::icon_tool(ui, ph::EXPORT, &crate::i18n::tr("tb-gcode-export"), false) {
                            self.export();
                        }
                    }
                }
                });
            });
        });
    }

    pub(super) fn tool_options_bar(&mut self, ctx: &egui::Context) {
        if self.edit_si().is_none() {
            return;
        }
        egui::TopBottomPanel::top("sk_tool_opts").frame(self.tool_bar_frame()).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                // the name of the active tool or operation
                let name = if self.pat.op != 0 {
                    if self.pat.op == 1 { crate::i18n::tr("tool-lin-array") } else { crate::i18n::tr("tool-circ-array") }
                } else if self.tool.move_op != 0 {
                    if self.tool.move_op == 1 { crate::i18n::tr("tool-move") } else { crate::i18n::tr("tool-copy") }
                } else if self.tool.click_op != 0 {
                    match self.tool.click_op {
                        1 => crate::i18n::tr("tool-trim"),
                        2 => crate::i18n::tr("tool-extend"),
                        3 => crate::i18n::tr("tool-break"),
                        4 => crate::i18n::tr("tool-fillet"),
                        5 => crate::i18n::tr("tool-chamfer"),
                        _ => "—".to_string(),
                    }
                } else if self.dim.kind != 0 {
                    crate::i18n::tr("tool-dim")
                } else if self.measure.on {
                    crate::i18n::tr("tool-measure")
                } else if self.tool.kind != 0 {
                    match self.tool.kind {
                        1 => crate::i18n::tr("tool-line"),
                        2 => crate::i18n::tr("tool-rect"),
                        3 => crate::i18n::tr("tool-circle"),
                        4 => crate::i18n::tr("tool-arc"),
                        5 => crate::i18n::tr("tool-point"),
                        6 => crate::i18n::tr("tool-polygon"),
                        7 => crate::i18n::tr("tool-slot"),
                        _ => "—".to_string(),
                    }
                } else {
                    match self.tool.modify {
                        1 => crate::i18n::tr("tool-fillet"),
                        2 => crate::i18n::tr("tool-chamfer"),
                        3 => crate::i18n::tr("tool-offset"),
                        4 => crate::i18n::tr("tool-mirror"),
                        5 => crate::i18n::tr("tool-lin-array-short"),
                        6 => crate::i18n::tr("tool-circ-array-short"),
                        _ => crate::i18n::tr("tool-select"),
                    }
                };
                ui.label(egui::RichText::new(name).strong());
                ui.separator();
                // ONLY the parameters of the active tool or operation
                if self.tool.kind != 0 {
                    ui.checkbox(&mut self.tool.construction, &crate::i18n::tr("opt-construction-short")).on_hover_text(&crate::i18n::tr("opt-construction-hint"));
                    if self.tool.kind == 11 {
                        ui.separator();
                        ui.label(&crate::i18n::tr("tool-text"));
                        ui.add(egui::TextEdit::singleline(&mut self.tool_prefs.text).desired_width(140.0));
                        ui.label(&crate::i18n::tr("opt-height-short"));
                        self.tool_prefs.text_h = self.num_or_expr(ui, "text_h", self.tool_prefs.text_h, 1.0, 1000.0, false, &crate::i18n::tr("unit-mm-suffix"));
                        if ui.button(&crate::i18n::tr("opt-font")).on_hover_text(&crate::i18n::tr("opt-pick-font")).clicked() {
                            self.pick_font();
                        }
                        ui.checkbox(&mut self.tool_prefs.text_note, &crate::i18n::tr("opt-note")).on_hover_text(&crate::i18n::tr("opt-text-as-note"));
                    }
                    if self.tool.kind == 6 {
                        ui.separator();
                        ui.label(&crate::i18n::tr("opt-sides"));
                        self.tool_prefs.poly_n = self.num_or_expr(ui, "poly_n", self.tool_prefs.poly_n as f64, 3.0, 64.0, true, "") as u32;
                        ui.selectable_value(&mut self.tool_prefs.poly_mode, 0u8, &crate::i18n::tr("opt-polygon-inscribed"));
                        ui.selectable_value(&mut self.tool_prefs.poly_mode, 1u8, &crate::i18n::tr("opt-polygon-circumscribed"));
                        ui.selectable_value(&mut self.tool_prefs.poly_mode, 2u8, &crate::i18n::tr("opt-by-edge"));
                        if self.tool_prefs.poly_mode == 2 {
                            ui.label(&crate::i18n::tr("opt-edge"));
                            self.tool_prefs.poly_edge = self.num_or_expr(ui, "poly_edge", self.tool_prefs.poly_edge, 0.1, 10000.0, false, &crate::i18n::tr("unit-mm-suffix"));
                        }
                    }
                    if self.tool.kind == 2 {
                        ui.separator();
                        ui.label(&crate::i18n::tr("opt-rect-mode"));
                        if ui.selectable_label(self.tool_prefs.rect_mode == 0, &crate::i18n::tr("opt-rect-2corners")).clicked() { self.tool_prefs.rect_mode = 0; self.tool.pts.clear(); }
                        if ui.selectable_label(self.tool_prefs.rect_mode == 1, &crate::i18n::tr("opt-rect-centre")).clicked() { self.tool_prefs.rect_mode = 1; self.tool.pts.clear(); }
                        if ui.selectable_label(self.tool_prefs.rect_mode == 2, &crate::i18n::tr("opt-rect-3pt")).on_hover_text(&crate::i18n::tr("opt-rect-rotated")).clicked() { self.tool_prefs.rect_mode = 2; self.tool.pts.clear(); }
                    }
                    if self.tool.kind == 3 {
                        ui.separator();
                        ui.label(&crate::i18n::tr("opt-circle-mode"));
                        if ui.selectable_label(self.tool_prefs.circ_mode == 0, &crate::i18n::tr("opt-circle-centre-radius")).clicked() { self.tool_prefs.circ_mode = 0; self.tool.pts.clear(); self.tool.circ_tan = None; }
                        if ui.selectable_label(self.tool_prefs.circ_mode == 1, &crate::i18n::tr("opt-circle-2pt")).on_hover_text(&crate::i18n::tr("opt-circle-diameter-ends")).clicked() { self.tool_prefs.circ_mode = 1; self.tool.pts.clear(); self.tool.circ_tan = None; }
                        if ui.selectable_label(self.tool_prefs.circ_mode == 2, &crate::i18n::tr("opt-tangent-m")).on_hover_text(&crate::i18n::tr("opt-tangent-hint")).clicked() { self.tool_prefs.circ_mode = 2; self.tool.pts.clear(); self.tool.circ_tan = None; }
                    }
                    if self.tool.kind == 4 {
                        ui.separator();
                        ui.label(&crate::i18n::tr("opt-arc-mode"));
                        if ui.selectable_label(self.tool_prefs.arc_mode == 0, &crate::i18n::tr("opt-arc-cse")).clicked() { self.tool_prefs.arc_mode = 0; self.tool.pts.clear(); }
                        if ui.selectable_label(self.tool_prefs.arc_mode == 1, &crate::i18n::tr("opt-rect-3pt")).on_hover_text(&crate::i18n::tr("opt-arc-3pt")).clicked() { self.tool_prefs.arc_mode = 1; self.tool.pts.clear(); }
                        if ui.selectable_label(self.tool_prefs.arc_mode == 2, &crate::i18n::tr("opt-tangent")).on_hover_text(&crate::i18n::tr("opt-arc-smooth-hint")).clicked() { self.tool_prefs.arc_mode = 2; self.tool.pts.clear(); }
                    }
                } else if self.dim.kind != 0 {
                    ui.label(egui::RichText::new(&crate::i18n::tr("opt-dim-hint")).weak());
                } else if self.tool.click_op != 0 {
                    if self.tool.click_op == 4 || self.tool.click_op == 5 {
                        ui.label(if self.tool.click_op == 4 { crate::i18n::tr("opt-radius") } else { crate::i18n::tr("opt-chamfer-size") });
                        self.tool_prefs.fillet = self.num_or_expr(ui, "sk_fillet", self.tool_prefs.fillet, 0.01, 10000.0, false, &crate::i18n::tr("unit-mm-suffix"));
                    }
                    if self.tool.click_op == 6 {
                        // WHAT IS TAKEN: one edge under the cursor, or the whole outline of the sketch's host face
                        if ui.selectable_label(!self.tool.proj_face, &crate::i18n::tr("opt-edge")).on_hover_text(&crate::i18n::tr("opt-project-edge-hint")).clicked() {
                            self.tool.proj_face = false;
                        }
                        if ui
                            .selectable_label(self.tool.proj_face, &crate::i18n::tr("opt-face-outline"))
                            .on_hover_text(&crate::i18n::tr("opt-face-outline-hint"))
                            .clicked()
                        {
                            self.tool.proj_face = true;
                        }
                    }
                    let h = match self.tool.click_op {
                        1 => &crate::i18n::tr("opt-trim-hint"),
                        2 => &crate::i18n::tr("opt-extend-hint"),
                        3 => &crate::i18n::tr("opt-break-hint"),
                        4 => &crate::i18n::tr("opt-fillet-hint"),
                        5 => &crate::i18n::tr("opt-chamfer-hint"),
                        6 => &crate::i18n::tr("opt-project-hint"),
                        _ => "",
                    };
                    ui.label(egui::RichText::new(h).weak());
                } else if self.pat.op != 0 {
                    // the pattern: the parameters + a hint (the preview is live, Enter applies)
                    ui.label(&crate::i18n::tr("opt-count"));
                    self.sk_pat.count = self.num_or_expr(ui, "skpat_count", self.sk_pat.count as f64, 2.0, 200.0, true, "") as u32;
                    if self.pat.op == 1 {
                        self.sk_pat.dx = self.num_or_expr(ui, "skpat_dx", self.sk_pat.dx, -100000.0, 100000.0, false, "");
                        self.sk_pat.dy = self.num_or_expr(ui, "skpat_dy", self.sk_pat.dy, -100000.0, 100000.0, false, "");
                        ui.separator();
                        ui.label(&crate::i18n::tr("opt-rows"));
                        self.sk_pat.count2 = self.num_or_expr(ui, "skpat_count2", self.sk_pat.count2 as f64, 1.0, 200.0, true, "") as u32;
                        if self.sk_pat.count2 > 1 {
                            self.sk_pat.dx2 = self.num_or_expr(ui, "skpat_dx2", self.sk_pat.dx2, -100000.0, 100000.0, false, "");
                            self.sk_pat.dy2 = self.num_or_expr(ui, "skpat_dy2", self.sk_pat.dy2, -100000.0, 100000.0, false, "");
                        }
                    } else {
                        self.sk_pat.angle = self.num_or_expr(ui, "skpat_angle", self.sk_pat.angle, -360.0, 360.0, false, "°");
                        ui.label(egui::RichText::new(if self.pat.center.is_some() { crate::i18n::tr("opt-centre-set") } else { crate::i18n::tr("opt-click-rotation-centre") }).weak());
                    }
                    ui.label(egui::RichText::new(if self.pat.edit.is_some() { crate::i18n::tr("opt-enter-update") } else { crate::i18n::tr("opt-enter-apply") }).weak());
                } else {
                    match self.tool.modify {
                        1 | 2 => {
                            ui.label(if self.tool.modify == 1 { crate::i18n::tr("opt-radius") } else { crate::i18n::tr("opt-chamfer-size") });
                            self.tool_prefs.fillet = self.num_or_expr(ui, "sk_fillet", self.tool_prefs.fillet, 0.01, 10000.0, false, &crate::i18n::tr("unit-mm-suffix"));
                        }
                        3 => {
                            ui.label(&crate::i18n::tr("opt-distance"));
                            self.tool_prefs.offset = self.num_or_expr(ui, "sk_offset", self.tool_prefs.offset, -10000.0, 10000.0, false, &crate::i18n::tr("unit-mm-suffix"));
                        }
                        5 => {
                            ui.label(&crate::i18n::tr("opt-count"));
                            self.sk_pat.count = self.num_or_expr(ui, "skpat_count", self.sk_pat.count as f64, 2.0, 200.0, true, "") as u32;
                            self.sk_pat.dx = self.num_or_expr(ui, "skpat_dx", self.sk_pat.dx, -100000.0, 100000.0, false, "");
                            self.sk_pat.dy = self.num_or_expr(ui, "skpat_dy", self.sk_pat.dy, -100000.0, 100000.0, false, "");
                        }
                        6 => {
                            ui.label(&crate::i18n::tr("opt-count"));
                            self.sk_pat.count = self.num_or_expr(ui, "skpat_count", self.sk_pat.count as f64, 2.0, 200.0, true, "") as u32;
                            self.sk_pat.angle = self.num_or_expr(ui, "skpat_angle", self.sk_pat.angle, -360.0, 360.0, false, "°");
                        }
                        4 => {
                            ui.label(egui::RichText::new(&crate::i18n::tr("opt-mirror-axis-hint")).weak());
                        }
                        _ => {
                            ui.checkbox(&mut self.tool.construction, &crate::i18n::tr("opt-construction-short"));
                            ui.label(egui::RichText::new(&crate::i18n::tr("opt-select-hint")).weak());
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(crate::i18n::tr1("opt-selected-n", "n", &self.sel_sk.items.len().to_string())).weak());
                });
            });
        });
    }
}
