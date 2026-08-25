//! THE PROPERTIES PANEL: what a feature, a contour, a plane or a mate IS, rather than how it is edited.
//!
//! Editing lives in the command - the top bar and the popup at the geometry - and that is where the preview,
//! the cancel and the formulas are. This side answers "what is this", "what does it stand on", "who depends
//! on it" and "is it all right".

use super::*;

impl App {
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
    pub(super) fn feature_props(&mut self, ui: &mut egui::Ui, ti: usize) {
        let Some(node) = self.project.timeline.get(ti).cloned() else { return };
        let fid = node.id;
        // THE FEATURE'S KIND GOES IN THE HEADING and the node's name below it. The heading used to read
        // "feature properties" - the same for all forty kinds - and what was actually selected had to be read
        // from the line below.
        let lin = self.lineage_of(Some(fid));
        props_header(ui, ph::STACK, &Self::feat_default_name(&node.kind), NameSlot::Fixed(node.name.clone()), &lin);

        // THE STATE: a rebuild error and the rollback - the reasons a feature may fail to build
        if let Some(err) = self.project.regen_errors.get(&fid) {
            ui.separator();
            let text = crate::i18n::error_text(err);
            ui.label(egui::RichText::new(format!("{} {text}", ph::WARNING)).color(self.scheme.pal.error_mild()).small());
        }
        if self.project.rollback.is_some_and(|r| r <= ti) {
            ui.label(egui::RichText::new(&crate::i18n::tr("fp-below-rollback")).weak().small());
        }

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(format!("{} {}", ph::PENCIL_SIMPLE, crate::i18n::tr("props-edit"))).on_hover_text(&crate::i18n::tr("fp-edit-hint")).clicked() {
                self.start_feat_cmd_edit(fid);
            }
            if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("props-delete"))).clicked() {
                self.ask_delete(Sel::Feature(ti)); // ask and delete, by the same path the tree uses
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
            ui.label(egui::RichText::new(&crate::i18n::tr("fp-params")).strong());
            for (key, _) in dims {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(key).weak());
                });
                self.dim_expr_field(ui, fid, key);
            }
        }
    }


    pub(super) fn contour_props(&mut self, ui: &mut egui::Ui, i: usize) {
        let lin = self.lineage_of(self.project.contour_id(i));
        props_header(ui, ph::POLYGON, "props-contour", NameSlot::None, &lin);
        let (closed, npts, area, bb, centroid) = {
            let c = &self.project.contours[i];
            (c.closed, c.points.len(), c.area(), c.bbox(), c.centroid())
        };
        ui.label(crate::i18n::trn("cp-summary", &[("state", &if closed { crate::i18n::tr("cp-closed") } else { crate::i18n::tr("cp-open") }), ("n", &npts.to_string()), ("area", &crate::i18n::num(area, 1))]));
        ui.separator();

        // 2D edits: they create copies of the contour (offset, mirror, pattern)
        egui::CollapsingHeader::new(&crate::i18n::tr("cp-edits")).id_salt(("edits", i)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("cp-offset-mm"));
                ui.add(egui::DragValue::new(&mut self.set.defaults.offset_2d).speed(0.2).range(0.1..=500.0));
            });
            ui.horizontal(|ui| {
                let d = self.set.defaults.offset_2d;
                if ui.button(&crate::i18n::tr("cp-outwards")).clicked() {
                    let res = qymcad_core::offset::offset_to_side(&self.project.contours[i], d, true);
                    self.project.add_contours(res);
                    self.invalidate();
                }
                if ui.button(&crate::i18n::tr("cp-inwards")).clicked() {
                    let res = qymcad_core::offset::offset_to_side(&self.project.contours[i], d, false);
                    self.project.add_contours(res);
                    self.invalidate();
                }
            });
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("cp-mirror"));
                if ui.button(&crate::i18n::tr("cp-by-x")).clicked() {
                    let mut c = self.project.contours[i].clone();
                    c.mirror(true, centroid.x);
                    self.project.add_contour(c);
                    self.invalidate();
                }
                if ui.button(&crate::i18n::tr("cp-by-y")).clicked() {
                    let mut c = self.project.contours[i].clone();
                    c.mirror(false, centroid.y);
                    self.project.add_contour(c);
                    self.invalidate();
                }
            });
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("cp-array-n"));
                ui.add(egui::DragValue::new(&mut self.array.n).range(2..=100));
            });
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("cp-step-xy"));
                ui.add(egui::DragValue::new(&mut self.array.dx).speed(0.5));
                ui.add(egui::DragValue::new(&mut self.array.dy).speed(0.5));
            });
            if ui.button(&crate::i18n::tr("cp-make-array")).clicked() {
                let base = self.project.contours[i].clone();
                for k in 1..self.array.n {
                    let mut c = base.clone();
                    c.translate(self.array.dx * k as f64, self.array.dy * k as f64);
                    self.project.add_contour(c);
                }
                self.invalidate();
            }
            // a 2D boolean (trim or region) against another contour
            let nc = self.project.contours.len();
            if nc >= 2 && self.project.contours[i].closed {
                ui.separator();
                if self.boolean.other2d == i || self.boolean.other2d >= nc {
                    self.boolean.other2d = (i + 1) % nc;
                }
                ui.horizontal(|ui| {
                    ui.label(&crate::i18n::tr("cp-with-contour"));
                    ui.add(egui::DragValue::new(&mut self.boolean.other2d).range(1..=nc).custom_formatter(|n, _| format!("{}", n as usize + 1)));
                });
                ui.horizontal(|ui| {
                    let ops: [(&str, u8); 3] = [(&crate::i18n::tr("cp-cut"), 0), (&crate::i18n::tr("cp-union"), 1), (&crate::i18n::tr("cp-intersect"), 2)];
                    for (lbl, op) in ops {
                        if ui.button(lbl).clicked() {
                            let oi = self.boolean.other2d.min(nc - 1);
                            if oi != i && self.project.contours[oi].closed {
                                let res = qymcad_core::offset::boolean_contours(&self.project.contours[i], &self.project.contours[oi], op);
                                let cnt = res.len();
                                self.project.add_contours(res);
                                self.status = crate::i18n::tr1("cp-bool-result", "n", &cnt.to_string());
                                self.invalidate();
                            }
                        }
                    }
                });
            }
        });
        ui.separator();

        // moving by the minimum corner
        if let Some(bb) = bb {
            ui.label(&crate::i18n::tr("cp-position-mm"));
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
                if let Some(c) = self.project.contours.get_mut(i) {
                    c.translate(dx, dy);
                }
                self.invalidate();
            }
        }

        ui.separator();
        ui.label(&crate::i18n::tr("cp-rotation"));
        ui.horizontal(|ui| {
            for (lbl, ang) in [("CCW 90", 90.0), ("CCW 15", 15.0), ("CW 15", -15.0), ("CW 90", -90.0)] {
                if ui.button(lbl).clicked() {
                    if let Some(c) = self.project.contours.get_mut(i) {
                    c.rotate(centroid, ang);
                }
                    self.invalidate();
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(&crate::i18n::tr("cp-scale"));
            if ui.button("× 1.1").clicked() {
                if let Some(c) = self.project.contours.get_mut(i) {
                    c.scale(centroid, 1.1);
                }
                self.invalidate();
            }
            if ui.button("÷ 1.1").clicked() {
                if let Some(c) = self.project.contours.get_mut(i) {
                    c.scale(centroid, 1.0 / 1.1);
                }
                self.invalidate();
            }
        });

        ui.separator();
        if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("props-delete-contour"))).clicked() {
            self.ask_delete(Sel::Contour(i));
        }
    }

    pub(super) fn plane_props(&mut self, ui: &mut egui::Ui, i: usize) {
        use qymcad_core::feature::BasePlane;
        use qymcad_core::model::PlaneDef;
        let mut remove = false;
        let mut changed = false;
        let mut start_pick_face = false;
        let pid = self.project.planes[i].id;
        let lin = self.lineage_of(Some(pid));
        if let Some(n) = props_header(ui, ph::PROJECTOR_SCREEN, "pp-title", NameSlot::Editable(self.project.planes[i].name.clone()), &lin) {
            self.project.planes[i].name = n;
        }
        {
            let p = &mut self.project.planes[i];
            // THE KIND of definition: manual, offset from a base plane, or offset from a face
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("pp-kind"));
                let is_offb = matches!(p.def, PlaneDef::OffsetBase { .. });
                let is_offf = matches!(p.def, PlaneDef::OffsetFace { .. });
                if ui.selectable_label(!is_offb && !is_offf, &crate::i18n::tr("pp-manual")).clicked() && (is_offb || is_offf) {
                    p.def = PlaneDef::Manual;
                    changed = true;
                }
                if ui.selectable_label(is_offb, &crate::i18n::tr("pp-from-plane")).clicked() && !is_offb {
                    p.def = PlaneDef::OffsetBase { base: BasePlane::XY, dist: 20.0 };
                    changed = true;
                }
                if ui.selectable_label(is_offf, &crate::i18n::tr("pp-from-face")).clicked() {
                    start_pick_face = true; // a face pick is needed (after this block)
                }
            });
        }
        if let PlaneDef::OffsetFace { body, face, mut dist } = self.project.planes[i].def {
            let nm = self.project.mesh_index(body).map(|mi| crate::i18n::name(&self.project.mesh_name(mi))).unwrap_or_else(|| crate::i18n::tr1("pp-body-n", "b", &body.to_string()));
            ui.label(egui::RichText::new(crate::i18n::tr1("pp-from-body-face", "name", &nm)).weak().small());
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("pp-offset-mm"));
                changed |= ui.add(egui::DragValue::new(&mut dist).speed(0.5).range(-100000.0..=100000.0)).changed();
            });
            self.project.planes[i].def = PlaneDef::OffsetFace { body, face, dist };
            self.dim_expr_field(ui, pid, "dist");
            if ui.button(format!("{} {}", ph::SELECTION_PLUS, crate::i18n::tr("props-pick-other-face"))).clicked() {
                start_pick_face = true;
            }
            let p = &self.project.planes[i];
            ui.label(egui::RichText::new(crate::i18n::tr2("pp-origin-normal", "o", &format!("[{}, {}, {}]", crate::i18n::num(p.origin[0],1), crate::i18n::num(p.origin[1],1), crate::i18n::num(p.origin[2],1)), "n", &format!("[{}, {}, {}]", crate::i18n::num(p.normal[0],2), crate::i18n::num(p.normal[1],2), crate::i18n::num(p.normal[2],2)))).weak().small());
        } else if let PlaneDef::OffsetBase { mut base, mut dist } = self.project.planes[i].def {
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("pp-from"));
                changed |= ui.selectable_value(&mut base, BasePlane::XY, "XY").changed();
                changed |= ui.selectable_value(&mut base, BasePlane::XZ, "XZ").changed();
                changed |= ui.selectable_value(&mut base, BasePlane::YZ, "YZ").changed();
            });
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("pp-offset-mm"));
                changed |= ui.add(egui::DragValue::new(&mut dist).speed(0.5).range(-100000.0..=100000.0)).changed();
            });
            self.project.planes[i].def = PlaneDef::OffsetBase { base, dist };
            self.dim_expr_field(ui, pid, "dist"); // the distance is an expression over the global variables (parametric)
            // the origin and normal are derived, so they are shown for information
            let p = &self.project.planes[i];
            ui.label(egui::RichText::new(crate::i18n::tr2("pp-origin-normal", "o", &format!("[{}, {}, {}]", crate::i18n::num(p.origin[0],1), crate::i18n::num(p.origin[1],1), crate::i18n::num(p.origin[2],1)), "n", &format!("[{}, {}, {}]", crate::i18n::num(p.normal[0],2), crate::i18n::num(p.normal[1],2), crate::i18n::num(p.normal[2],2)))).weak().small());
        } else {
            // MANUAL: direct editors for the origin, the normal and the roll
            let p = &mut self.project.planes[i];
            ui.label(&crate::i18n::tr("pp-origin-mm"));
            egui::Grid::new(("plo", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
                changed |= drag(ui, "X", &mut p.origin[0], 1.0, -10000.0..=10000.0);
                changed |= drag(ui, "Y", &mut p.origin[1], 1.0, -10000.0..=10000.0);
                changed |= drag(ui, "Z", &mut p.origin[2], 1.0, -10000.0..=10000.0);
            });
            ui.separator();
            ui.label(&crate::i18n::tr("pp-normal"));
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
                ui.label(&crate::i18n::tr("pp-rot-about-normal"));
                changed |= ui.add(egui::DragValue::new(&mut p.rot_deg).speed(1.0).range(-180.0..=180.0).suffix("°")).changed();
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(&crate::i18n::tr("pp-table-xy")).clicked() {
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
            self.datum.regen_pending = true; // the plane moved, so the consumers are rebuilt (debounced on release)
        }
        if start_pick_face {
            self.picking.set_plane_face(Some(pid)); // reassign this plane's face
            self.mode_3d = true;
            self.status = crate::i18n::tr("pp-pick-face-hint");
        }
        ui.separator();
        // a sketch straight on this plane (drawn in its own frame)
        if ui.button(format!("{} {}", ph::PENCIL, crate::i18n::tr("props-new-sketch-here"))).clicked() {
            let pid = self.project.planes[i].id;
            let pid = if pid == 0 {
                let id = self.project.alloc_id();
                self.project.planes[i].id = id;
                id
            } else {
                pid
            };
            self.create_sketch_on(qymcad_core::feature::SketchPlane::Datum(pid));
            return;
        }
        if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("props-delete-plane"))).clicked() {
            remove = true;
        }
        if remove {
            self.ask_delete(Sel::Plane(i)); // the removal itself is in `execute_delete`, by the same path the tree uses
        }
    }

    pub(super) fn op_editor(&mut self, ui: &mut egui::Ui, i: usize) {
        self.op_geometry_editor(ui, i);
        // the setup (the work coordinate system), when one is defined
        if !self.project.setups.is_empty() {
            let cur = self.project.operations[i].setup.min(self.project.setups.len() - 1);
            let cur_name = format!("{} · {}", self.project.setups[cur].name, self.project.setups[cur].wcs.label());
            let mut chosen = cur;
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("cam-setup"));
                egui::ComboBox::from_id_salt(("opsetup", i)).selected_text(cur_name).show_ui(ui, |ui| {
                    for si in 0..self.project.setups.len() {
                        let nm = format!("{} · {}", self.project.setups[si].name, self.project.setups[si].wcs.label());
                        ui.selectable_value(&mut chosen, si, nm);
                    }
                });
            });
            if chosen != cur {
                self.project.operations[i].setup = chosen;
                self.invalidate();
            }
        }
        let tool_numbers: Vec<u32> = self.project.tools.iter().map(|t| t.number).collect();
        let op = &mut self.project.operations[i];
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::CARET_RIGHT, op.name)).strong());
            if ui.small_button(format!("{} G-code", ph::EXPORT)).on_hover_text(&crate::i18n::tr("cam-export-this-op")).clicked() {
                self.io.export_op = Some(i);
            }
        });
        ui.horizontal(|ui| {
            ui.label(&crate::i18n::tr("cam-name"));
            crate::gui::name_edit(ui, &mut op.name);
        });

        // the tool
        ui.horizontal(|ui| {
            ui.label(&crate::i18n::tr("cam-tool-t"));
            egui::ComboBox::from_id_salt(("optool", i))
                .selected_text(format!("T{}", op.tool))
                .show_ui(ui, |ui| {
                    for n in &tool_numbers {
                        ui.selectable_value(&mut op.tool, *n, format!("T{n}"));
                    }
                });
        });

        // the geometry selection
        ui.horizontal(|ui| {
            let sel = if op.selection.is_empty() { crate::i18n::tr("cam-all") } else { format!("{}", op.selection.len()) };
            ui.label(crate::i18n::tr1("cam-contours-sel", "sel", &sel));
            if ui.small_button(&crate::i18n::tr("cam-clear")).clicked() {
                op.selection.clear();
            }
        });
        ui.label(egui::RichText::new(&crate::i18n::tr("cam-contour-pick-hint")).weak().small());

        ui.separator();
        // the heights
        egui::Grid::new(("h", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            drag(ui, &crate::i18n::tr("cam-safe-z"), &mut op.heights.clearance, 0.5, -10.0..=200.0);
            drag(ui, &crate::i18n::tr("cam-retract-z"), &mut op.heights.retract, 0.5, -10.0..=200.0);
            drag(ui, &crate::i18n::tr("cam-top-z"), &mut op.heights.top, 0.5, -200.0..=200.0);
            drag(ui, &crate::i18n::tr("cam-bottom-z"), &mut op.heights.bottom, 0.5, -500.0..=200.0);
            drag(ui, &crate::i18n::tr("cam-step-z"), &mut op.passes.stepdown, 0.1, 0.05..=50.0);
            drag(ui, &crate::i18n::tr("cam-step-xy"), &mut op.passes.stepover, 0.1, 0.05..=50.0);
            drag(ui, &crate::i18n::tr("cam-stock-to-leave"), &mut op.passes.stock_to_leave, 0.05, 0.0..=10.0);
        });

        ui.separator();
        // the modes + the feeds and speeds chosen by material
        let (td, tf) = self
            .project
            .tools
            .iter()
            .find(|t| t.number == op.tool)
            .map(|t| (t.diameter, t.flutes))
            .unwrap_or((3.0, 2));
        let mats = qymcad_core::feeds::materials();
        ui.horizontal(|ui| {
            ui.label(&crate::i18n::tr("cam-material"));
            egui::ComboBox::from_id_salt(("mat", i))
                .selected_text(crate::i18n::tr(mats[self.cam_job.material.min(mats.len() - 1)].name))
                .show_ui(ui, |ui| {
                    for (mi, m) in mats.iter().enumerate() {
                        ui.selectable_value(&mut self.cam_job.material, mi, crate::i18n::tr(m.name));
                    }
                });
            if ui.button(&crate::i18n::tr("cam-pick")).on_hover_text(&crate::i18n::tr("cam-calc-feeds")).clicked() {
                let r = qymcad_core::feeds::recommend(&mats[self.cam_job.material.min(mats.len() - 1)], td, tf);
                op.feeds.rpm = r.rpm.min(self.project.machine.max_rpm).round();
                op.feeds.cut = r.feed.min(self.project.machine.max_feed).round();
                op.feeds.plunge = r.plunge.round();
            }
        });
        egui::Grid::new(("f", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            drag(ui, &crate::i18n::tr("cam-rpm"), &mut op.feeds.rpm, 100.0, 1000.0..=60000.0);
            drag(ui, &crate::i18n::tr("cam-feed"), &mut op.feeds.cut, 10.0, 10.0..=10000.0);
            drag(ui, &crate::i18n::tr("cam-plunge"), &mut op.feeds.plunge, 10.0, 10.0..=5000.0);
        });
        ui.horizontal(|ui| {
            ui.label(&crate::i18n::tr("cam-coolant"));
            egui::ComboBox::from_id_salt(("cool", i))
                .selected_text(crate::i18n::tr(coolant_label(op.feeds.coolant)))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut op.feeds.coolant, CoolantMode::Off, &crate::i18n::tr("cam-coolant-off"));
                    ui.selectable_value(&mut op.feeds.coolant, CoolantMode::Mist, &crate::i18n::tr("cam-coolant-mist"));
                    ui.selectable_value(&mut op.feeds.coolant, CoolantMode::Flood, &crate::i18n::tr("cam-coolant-flood"));
                });
            egui::ComboBox::from_id_salt(("spin", i))
                .selected_text(if op.feeds.spindle_dir == SpindleDir::Cw { "M3 CW" } else { "M4 CCW" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut op.feeds.spindle_dir, SpindleDir::Cw, "M3 CW");
                    ui.selectable_value(&mut op.feeds.spindle_dir, SpindleDir::Ccw, "M4 CCW");
                });
        });

        ui.separator();
        // what is specific to the type
        match &mut op.kind {
            OpKind::Contour { side, tabs, ramp, climb, finish } => {
                ui.horizontal(|ui| {
                    ui.label(&crate::i18n::tr("cam-side"));
                    egui::ComboBox::from_id_salt(("side", i))
                        .selected_text(crate::i18n::tr(side_label(*side)))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(side, SideMode::Auto, &crate::i18n::tr("cam-side-auto"));
                            ui.selectable_value(side, SideMode::Outside, &crate::i18n::tr("cam-side-outside"));
                            ui.selectable_value(side, SideMode::Inside, &crate::i18n::tr("cam-side-inside"));
                            ui.selectable_value(side, SideMode::On, &crate::i18n::tr("cam-side-on"));
                        });
                });
                ui.checkbox(finish, &crate::i18n::tr("cam-finish-pass")).on_hover_text(&crate::i18n::tr("cam-finish-pass-hint"));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("cmd-direction"));
                    egui::ComboBox::from_id_salt(("climb", i))
                        .selected_text(if *climb { crate::i18n::tr("cam-climb") } else { crate::i18n::tr("cam-conventional") })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(climb, true, &crate::i18n::tr("cam-climb"));
                            ui.selectable_value(climb, false, &crate::i18n::tr("cam-conventional"));
                        });
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut ramp.enabled, &crate::i18n::tr("cam-ramp"));
                    if ramp.enabled {
                        ui.add(egui::DragValue::new(&mut ramp.angle_deg).speed(0.5).range(0.5..=45.0).suffix("°"));
                    }
                });
                ui.checkbox(&mut tabs.enabled, &crate::i18n::tr("cam-tabs"));
                if tabs.enabled {
                    egui::Grid::new(("tabs", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
                        ui.label(&crate::i18n::tr("cam-count"));
                        ui.add(egui::DragValue::new(&mut tabs.count).range(1..=32));
                        ui.end_row();
                        drag(ui, &crate::i18n::tr("cam-width"), &mut tabs.width, 0.5, 0.5..=50.0);
                        drag(ui, &crate::i18n::tr("cam-height"), &mut tabs.height, 0.1, 0.1..=20.0);
                    });
                }
            }
            OpKind::Drill { cycle, peck, dwell } => {
                ui.horizontal(|ui| {
                    ui.label(&crate::i18n::tr("cam-cycle"));
                    egui::ComboBox::from_id_salt(("dc", i))
                        .selected_text(drill_label(*cycle))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(cycle, DrillKind::Drill, &crate::i18n::tr("cam-g81"));
                            ui.selectable_value(cycle, DrillKind::DwellDrill, &crate::i18n::tr("cam-g82"));
                            ui.selectable_value(cycle, DrillKind::Peck, &crate::i18n::tr("cam-g83"));
                        });
                });
                if matches!(cycle, DrillKind::Peck) {
                    let mut q = peck.unwrap_or(2.0);
                    if drag_opt(ui, &crate::i18n::tr("cam-peck-q"), &mut q, 0.1, 0.1..=20.0) {
                        *peck = Some(q);
                    }
                }
                if matches!(cycle, DrillKind::DwellDrill) {
                    let mut d = dwell.unwrap_or(0.5);
                    if drag_opt(ui, &crate::i18n::tr("cam-dwell-p"), &mut d, 0.1, 0.0..=10.0) {
                        *dwell = Some(d);
                    }
                }
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-drill-centres")).weak().small());
                if ui.button(&crate::i18n::tr("cam-pick-round-holes")).clicked() {
                    op.selection = self
                        .project
                        .contours
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| c.as_circle().is_some())
                        .filter_map(|(idx, _)| self.project.contours.id_at(idx))
                        .collect();
                }
            }
            OpKind::Bore => {
                ui.label(&crate::i18n::tr("cam-bore-hint"));
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-bore-note")).weak().small());
                if ui.button(&crate::i18n::tr("cam-pick-round-holes")).clicked() {
                    op.selection = self
                        .project
                        .contours
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| c.as_circle().is_some())
                        .filter_map(|(idx, _)| self.project.contours.id_at(idx))
                        .collect();
                }
            }
            OpKind::Surface3D { .. } => {
                ui.label(crate::i18n::tr("cam-finish-raster"));
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-finish-raster-note")).weak().small());
            }
            OpKind::Rough3D { .. } => {
                ui.label(crate::i18n::tr("cam-rough-z"));
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-rough-z-note")).weak().small());
            }
            OpKind::Waterline3D { .. } => {
                ui.label(crate::i18n::tr("cam-waterline"));
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-waterline-note")).weak().small());
            }
            OpKind::Project3D { .. } => {
                ui.label(crate::i18n::tr("cam-engrave"));
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-engrave-note")).weak().small());
            }
            OpKind::Flat3D { .. } => {
                ui.label(crate::i18n::tr("cam-facing"));
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-facing-note")).weak().small());
            }
            OpKind::Adaptive2D => {
                ui.label(&crate::i18n::tr("cam-trochoidal"));
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-trochoidal-note")).weak().small());
            }
            OpKind::Pocket { dogbone } => {
                ui.checkbox(dogbone, &crate::i18n::tr("cam-dogbone"));
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-dogbone-note")).weak().small());
            }
            OpKind::Face | OpKind::Engrave => {
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-no-params")).weak().small());
            }
            OpKind::Slot => {
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-slot-note")).weak().small());
            }
        }
    }

    /// The mates panel: create a mate between two parts of an assembly + a list with the parameters.
    /// Editing one recomputes the positions.
    pub(super) fn joints_panel(&mut self, ui: &mut egui::Ui) {
        use qymcad_core::feature::{AnchorRef, JointKind, MateItem, MateState};
        ui.label(egui::RichText::new(&crate::i18n::tr("jp-title")).strong());
        // a warning about a CONFLICT among the assembly mates (the solver did not converge, so the parts were left where they were).
        if self.project.mates_conflict {
            ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr("jp-conflict"))).color(self.scheme.pal.error()).small());
        }
        let ctx = self.current_ctx_id();
        let children = self.project.component_children(ctx);
        let name_of = |s: &Self, id: Id| s.project.components.iter().find(|c| c.id == id).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();

        if children.len() >= 2 {
            // CREATION HAPPENS THROUGH THE COMMAND ONLY. There used to be two A/B drop-downs, a kind selector and
            // a "create at the origins" button: the joint appeared instantly, with no geometry picked by a click,
            // no preview and no Esc. That contradicted the rule the whole rest of the interface is built on and
            // taught people to go the wrong way - while the "at the origins" method did not disappear at all, it
            // became a fourth kind of anchor inside the command itself.
            //
            // A button that STARTS a command is not the same as a button that creates an object: it leads into the
            // shared path rather than around it.
            if ui.button(format!("{}  {}", ph::MAGNET, crate::i18n::tr("jp-start-joint"))).on_hover_text(&crate::i18n::tr("jp-start-joint-hint")).clicked() {
                self.start_joint_pick();
            }
        } else {
            ui.label(egui::RichText::new(&crate::i18n::tr("jp-need-two-parts")).weak());
        }

        // grounding sits in the Mates panel itself (rather than a checkbox to be hunted for in the right panel).
        // A click on a part fixes or releases it as the root of the placement tree; the anchor icon means grounded
        // (the same glyph appears in the viewport).
        if !children.is_empty() {
            ui.separator();
            ui.label(egui::RichText::new(&crate::i18n::tr("jp-grounding")).strong());
            for &c in &children {
                let g = self.project.is_grounded(c);
                let icon = if g { ph::ANCHOR } else { ph::ANCHOR_SIMPLE };
                if ui.selectable_label(g, format!("{icon} {}", name_of(self, c))).on_hover_text(&crate::i18n::tr("jp-grounding-hint")).clicked() {
                    self.project.set_grounded(c, !g);
                    self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
                }
            }
            // GROUNDING THAT DOES NOT HOLD IS SAID OUT LOUD.
            //
            // A part grounded INSIDE a moving subassembly travels with it: the word "grounded" has been said, yet
            // it has no world immobility. Measured on a real machine: one frame part was grounded inside the X beam
            // and travelled 100.000 mm along with the gantry. This is not forbidden - the arrangement is legitimate
            // when a unit is assembled on its own - but a person must know.
            let lying: Vec<String> = self.project.grounded_inside_moving().into_iter().map(|c| name_of(self, c)).collect();
            if !lying.is_empty() {
                ui.label(egui::RichText::new(crate::i18n::tr1("jp-grounded-inside-moving", "list", &lying.join(", "))).color(self.scheme.pal.hint()).small())
                    .on_hover_text(&crate::i18n::tr("jp-grounded-inside-moving-hint"));
            }
        }

        ui.separator();
        let ctx_now = self.current_ctx_id();
        // THE ANCHORS GET A LIST OF THEIR OWN. A connector became an element in its own right, and an element
        // that is not in a list does not exist for a person: it cannot be selected, corrected or removed.
        ui.horizontal(|ui| {
            if ui.button(format!("{}  {}", ph::CROSSHAIR, crate::i18n::tr("j-conn-new"))).on_hover_text(&crate::i18n::tr("j-conn-new-hint")).clicked() {
                self.start_conn_pick();
            }
        });
        let mut kill_conn: Option<Id> = None;
        let conns: Vec<(Id, Id, String, usize)> = self
            .project
            .connectors
            .iter()
            // THE STANDALONE ONES ONLY. Anchors created FOR a joint are edited inside that joint and need no
            // row of their own: in an assembly with five joints the list would swell by ten rows nobody created.
            // Implicit connectors belong inside their own mate rather than beside it.
            .filter(|c| c.standalone && self.project.component_is_within(c.owner, ctx_now))
            .map(|c| (c.id, c.owner, crate::i18n::name(&c.name), self.project.connector_users(c.id).len()))
            .collect();
        for (cid, owner, cname, users) in conns {
            ui.horizontal(|ui| {
                let sel = self.sel_conn == Some(cid);
                if ui.selectable_label(sel, format!("{} {cname} — {}", ph::CROSSHAIR, name_of(self, owner))).clicked() {
                    self.sel_conn = if sel { None } else { Some(cid) };
                }
                // HOW MANY JOINTS HANG ON IT - which is also the answer to why it cannot be deleted.
                if users > 0 {
                    ui.label(egui::RichText::new(crate::i18n::tr1("j-conn-users", "n", &users.to_string())).weak().small());
                }
                if ui.small_button(ph::X).clicked() {
                    kill_conn = Some(cid);
                }
            });
            if self.sel_conn == Some(cid) {
                self.one_connector_controls(ui, "", cid);
            }
        }
        if let Some(cid) = kill_conn {
            self.delete_connector_asked(cid);
            self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
        }
        // ONE MATE TIMELINE.
        //
        // There used to be THREE SEPARATE LOOPS - constraints, relations and joints - each with its own row, its
        // own delete button and its own idea of whether an element was sound. Three ideas about one thing drift
        // apart silently, and a person sees what is not there.
        //
        // The list comes from the core (`Project::mate_timeline`): the panel no longer decides what is in it or
        // what state that is in, and so it cannot decide differently from the solver.
        let ctx = self.current_ctx_id();
        let mut to_del: Option<(Id, MateItem)> = None;
        let mut changed = false;
        let at_limit = self.project.joints_at_limit();
        let mut put_to_limit = false;
        for e in self.project.mate_timeline(ctx) {
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
            let parts: Vec<String> = e.touches.iter().map(|m| name_of(self, *m)).collect();
            let selected = matches!(e.item, MateItem::Joint) && self.sel == Sel::Joint(e.id);
            ui.horizontal(|ui| {
                let in_relation = self.joint.relation_pick.as_ref().is_some_and(|p| p.picks.iter().any(|(id, _)| *id == e.id));
                let resp = ui.selectable_label(selected || in_relation, format!("{icon} {}", crate::i18n::name(&e.name)));
                ui.label(egui::RichText::new(format!("{}: {}", crate::i18n::tr(e.kind_label), parts.join(" <-> "))).weak().small());
                if resp.clicked() && matches!(e.item, MateItem::Joint) {
                    // THE TOOL IN HAND OUTRANKS THE SELECTION: while a relation is being gathered, a click on a
                    // joint means "take this degree of freedom" rather than "show this joint". Otherwise there
                    // would be no way to point at mates at all - they are not geometry and cannot be picked in
                    // the viewport.
                    if self.joint.relation_pick.is_some() {
                        self.relation_pick_click(e.id);
                    } else {
                        self.sel = Sel::Joint(e.id);
                    }
                }
                if resp.hovered() && matches!(e.item, MateItem::Joint) {
                    self.hover.joint = Some(e.id);
                }
                // THE STATE IS SAID IN THE SAME WORDS FOR ALL THREE KINDS. While there was no such mark, dead
                // joints looked exactly like healthy ones in the list: a person sees a joint, the part does not
                // move, and there is no explanation.
                match e.state {
                    MateState::Faulty(why) => {
                        ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr(why))).color(self.scheme.pal.error_mild()).small())
                            .on_hover_text(crate::i18n::tr(&format!("{why}-hint")));
                    }
                    MateState::Violated => {
                        ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr("jp-dof-conflict"))).color(self.scheme.pal.error()).small());
                    }
                    MateState::Ok => {}
                }
                // BEING AT A LIMIT IS NOT A FAULT, BUT IT HAS TO BE SAID. A limit clamps the given value
                // silently: 40 was typed, the part stopped at 20, and from the outside that is indistinguishable
                // from the program not listening.
                if at_limit.iter().any(|(id, _)| *id == e.id) {
                    ui.label(egui::RichText::new(format!("{} {}", ph::ARROWS_IN_LINE_HORIZONTAL, crate::i18n::tr("jp-at-limit"))).color(self.scheme.pal.hint()).small())
                        .on_hover_text(&crate::i18n::tr("jp-at-limit-hint"));
                }
                // "GO TO THE LIMIT" appears only where a limit is set: a mechanism is inspected at its EXTREME
                // positions, and that is where it runs into its neighbour.
                if matches!(e.item, MateItem::Joint) {
                    let bounds: Vec<(usize, bool)> = self
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
                                if ui.button(&what).on_hover_text(&crate::i18n::tr("jp-apply-limit-hint")).clicked() {
                                    if self.project.apply_limit_position(e.id, slot, up) {
                                        put_to_limit = true;
                                    }
                                    ui.close_menu();
                                }
                            }
                        })
                        .response
                        .on_hover_text(&crate::i18n::tr("jp-apply-limit"));
                    }
                }
                if ui.small_button(ph::X).clicked() {
                    to_del = Some((e.id, e.item));
                }
            });
            // A RELATION tells what it ties together and with what number - it has no motion of its own.
            if matches!(e.item, MateItem::Relation) {
                if let Some(r) = self.project.relations.iter().find(|r| r.id == e.id) {
                    let unit = crate::i18n::tr(if r.kind.value_is_per_turn() { "j-relation-per-turn" } else { "j-relation-ratio" });
                    ui.label(egui::RichText::new(format!("{unit}: {}", r.value)).weak().small());
                }
            }
            let Some(j) = self.project.joints.iter().find(|x| x.id == e.id).cloned() else { continue };
            let j = &j;
            let ob = self.project.connector(j.b).map(|c| c.owner).unwrap_or(0);
            // changing a joint's KIND on the fly (the anchors are kept; an incompatibility between the kind and the anchors goes to the status line).
            {
                let mut newk = j.kind;
                egui::ComboBox::from_id_salt(("jkind", j.id)).selected_text(crate::i18n::tr(j.kind.label())).show_ui(ui, |ui| {
                    ui.label(egui::RichText::new(&crate::i18n::tr("jp-assembly-kind")).weak().small());
                    for kk in [JointKind::Rigid, JointKind::Revolute, JointKind::Slider, JointKind::Cylindrical, JointKind::Planar, JointKind::Ball, JointKind::PinSlot, JointKind::Parallel] {
                        ui.selectable_value(&mut newk, kk, crate::i18n::tr(kk.label()));
                    }
                    ui.separator();
                    ui.label(egui::RichText::new(&crate::i18n::tr("jp-mechanical-kind")).weak().small());
                    for kk in [JointKind::Revolute, JointKind::Slider, JointKind::Cylindrical, JointKind::Planar, JointKind::Ball, JointKind::PinSlot, JointKind::Parallel] {
                        ui.selectable_value(&mut newk, kk, crate::i18n::tr(kk.label()));
                    }
                });
                if newk != j.kind {
                    changed |= self.change_joint_kind(j.id, newk);
                }
            }
            // a rigid face-to-face joint gets a SIDE toggle (coplanar or face-to-face, through a 180 degree turn)
            let face_rigid = matches!(j.kind, JointKind::Rigid)
                && self.project.connector(j.a).is_some_and(|c| matches!(c.anchor, AnchorRef::FaceCenter(..)))
                && self.project.connector(j.b).is_some_and(|c| matches!(c.anchor, AnchorRef::FaceCenter(..)));
            // a joint of a nested subassembly (whose home is not the root) can be lifted into the root for global control
            let nested = self.project.joint_home(j).is_some_and(|h| h != self.project.root);
            {
                if let Some(jj) = self.project.joints.iter_mut().find(|x| x.id == j.id) {
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
                            changed |= ui.checkbox(&mut jj.flip, &crate::i18n::tr("jp-flip-side")).on_hover_text(&crate::i18n::tr("jp-flip-side-hint")).changed();
                        }
                        if nested {
                            changed |= ui.checkbox(&mut jj.global, &crate::i18n::tr("jp-drive-from-root")).on_hover_text(&crate::i18n::tr("jp-drive-from-root-hint")).changed();
                        }
                    });
                }
            }
            // the parametric angle and offset fields are expressions over the global variables (like sketch
            // dimensions): they are stored in feat_dims under the joint's id and evaluated in regenerate.
            if matches!(j.kind, JointKind::Revolute | JointKind::Cylindrical | JointKind::PinSlot | JointKind::Ball | JointKind::Planar) {
                self.dim_expr_field(ui, j.id, "angle");
            }
            if matches!(j.kind, JointKind::Rigid | JointKind::Slider | JointKind::Cylindrical | JointKind::PinSlot | JointKind::Planar | JointKind::Ball) {
                self.dim_expr_field(ui, j.id, "offset");
            }
            if matches!(j.kind, JointKind::Planar | JointKind::Ball) {
                self.dim_expr_field(ui, j.id, "offset2");
            }
            // limits (min and max) on a joint's free slots - a stop within a range.
            if let Some(jj) = self.project.joints.iter_mut().find(|x| x.id == j.id) {
                let free = jj.kind.free_slots();
                if free.iter().any(|&f| f) {
                    let mut lim_changed = false;
                    egui::CollapsingHeader::new(&crate::i18n::tr("jp-limits")).id_salt(("jlim", j.id)).default_open(false).show(ui, |ui| {
                        for slot in 0..3usize {
                            if !free[slot] {
                                continue;
                            }
                            // THE SAME limits widget as in the popup at the glyph (joints.rs)
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
            if self.project.mates_violated.contains(&j.id) {
                ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr("jp-dof-conflict"))).color(self.scheme.pal.error()).small());
            } else {
                let d = self.project.component_dof(ob);
                let (lbl, col) = if d == 0 { (&crate::i18n::tr("jp-defined"), self.scheme.pal.ok()) } else { (&crate::i18n::tr("jp-underdefined"), self.scheme.pal.underdefined()) };
                ui.label(egui::RichText::new(crate::i18n::tr2("jp-dof", "d", &d.to_string(), "state", &lbl)).color(col).small());
            }
        }
        if put_to_limit {
            // SETTING A VALUE MUST NOT SOIL THE DOCUMENT: without a solve the part would stay where it was while
            // the field showed the new number - exactly the divergence where the field says one thing and the part
            // another.
            self.project.solve_joints();
            changed = true;
        }
        if let Some((id, item)) = to_del {
            // ONE CORE METHOD PER KIND. There used to be a copy of the cleanup here, and it counted orphans by the
            // joint list of THE CURRENT assembly: delete a joint in the root, and the connectors of every
            // subassembly's joints went with it while those joints stayed dead forever. That is how one document
            // ended up with five joints on two connectors.
            match item {
                MateItem::Joint => self.project.delete_joint(id),
                MateItem::Constraint => self.project.delete_group(id),
                MateItem::Relation => self.project.delete_relation(id),
            }
            changed = true;
        }
        if changed {
            self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
        }
    }

}
