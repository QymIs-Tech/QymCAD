//! THE WINDOWS AND DIALOGUES: the parts library, the properties of a component and of a datum, the
//! parameters, the machine, the settings, the tools, the modal confirmations.

use super::*;

impl App {
    /// The parts library window: a category tree on the left, a grid of products on the right, insertion into the project.
    pub(super) fn parts_library_window(&mut self, ctx: &egui::Context) {
        if !self.win.parts_library {
            return;
        }
        // The tree is taken out of self for a moment, so that it can be read while the selection and the search are mutated.
        let tree = self.parts.tree.take().unwrap_or_else(crate::parts_library::LibraryTree::load);
        let mut open = self.win.parts_library;
        let mut to_insert: Option<crate::parts_library::PartSource> = None;
        let mut rescan = false;
        egui::Window::new(format!("{} {}", ph::PACKAGE, crate::i18n::tr("win-parts-library")))
            .open(&mut open)
            .default_size([640.0, 440.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button(format!("{} {}", ph::ARROWS_CLOCKWISE, crate::i18n::tr("win-refresh")))
                        .on_hover_text(&crate::i18n::tr("pl-rescan"))
                        .clicked()
                    {
                        rescan = true;
                    }
                    ui.separator();
                    ui.label(ph::MAGNIFYING_GLASS.to_string());
                    ui.add(
                        egui::TextEdit::singleline(&mut self.parts.search)
                            .hint_text(&crate::i18n::tr("pl-search"))
                            .desired_width(200.0),
                    );
                    if !self.parts.search.is_empty() && ui.small_button(ph::X).clicked() {
                        self.parts.search.clear();
                    }
                });
                ui.separator();
                let query = self.parts.search.trim().to_lowercase();
                egui::SidePanel::left("parts_lib_tree")
                    .resizable(true)
                    .default_width(230.0)
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("parts_lib_tree_scroll").show(ui, |ui| {
                            let mut path = Vec::new();
                            Self::parts_tree_node(ui, &tree.embedded, true, &mut path, &mut self.parts.sel);
                            Self::parts_tree_node(ui, &tree.user, false, &mut path, &mut self.parts.sel);
                        });
                    });
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    // The set on the right: while searching, every match in the catalogue; otherwise the direct products of the chosen category.
                    let mut entries: Vec<&crate::parts_library::PartEntry> = Vec::new();
                    if query.is_empty() {
                        match &self.parts.sel {
                            Some((true, p)) => {
                                if let Some(n) = Self::cat_at(&tree.embedded, p) {
                                    entries.extend(n.parts.iter());
                                }
                            }
                            Some((false, p)) => {
                                if let Some(n) = Self::cat_at(&tree.user, p) {
                                    entries.extend(n.parts.iter());
                                }
                            }
                            None => {
                                ui.weak(&crate::i18n::tr("pl-pick-category"));
                            }
                        }
                    } else {
                        Self::collect_matching(&tree.embedded, &query, &mut entries);
                        Self::collect_matching(&tree.user, &query, &mut entries);
                        ui.weak(crate::i18n::tr1("pl-found-n", "n", &entries.len().to_string()));
                        ui.add_space(2.0);
                    }
                    if (query.is_empty() && self.parts.sel.is_some() || !query.is_empty()) && entries.is_empty() {
                        ui.weak(&crate::i18n::tr("pl-empty"));
                    }
                    // the thumbnails (lazily loaded and cached) are prepared IN ADVANCE, so that no &mut self is held inside the drawing loop
                    let thumbs: Vec<Option<egui::TextureHandle>> = entries.iter().map(|e| self.parts_thumb_texture(ui.ctx(), &e.source)).collect();
                    egui::ScrollArea::vertical().id_salt("parts_lib_grid").show(ui, |ui| {
                        for (e, thumb) in entries.iter().zip(thumbs.iter()) {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    // a preview thumbnail of the body (or a placeholder icon when the product has no thumb.png)
                                    match thumb {
                                        Some(t) => {
                                            ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(t.id(), egui::vec2(52.0, 52.0))).rounding(3.0));
                                        }
                                        None => {
                                            ui.add_sized([52.0, 52.0], egui::Label::new(egui::RichText::new(ph::CUBE).size(24.0).weak()));
                                        }
                                    }
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(&e.name).strong());
                                        if let Some(m) = &e.manifest {
                                            if !m.description.is_empty() {
                                                ui.label(egui::RichText::new(&m.description).weak().small());
                                            }
                                            if !m.tags.is_empty() {
                                                ui.label(egui::RichText::new(m.tags.join(" · ")).weak().small());
                                            }
                                        }
                                    });
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button(format!("{} {}", ph::ARROW_SQUARE_IN, crate::i18n::tr("win-insert"))).clicked() {
                                            to_insert = Some(e.source.clone());
                                        }
                                    });
                                });
                            });
                        }
                    });
                });
            });
        // The tree is put back (or rebuilt when Refresh was pressed).
        if rescan {
            self.parts.tree = Some(crate::parts_library::LibraryTree::load());
            // the thumbnails are re-read (they may have changed on disk) and the old textures are retired (never dropped mid-frame)
            self.tex_graveyard.extend(self.parts.thumbs.drain().filter_map(|(_, v)| v));
            self.parts.sel = None;
        } else {
            self.parts.tree = Some(tree);
        }
        if let Some(src) = to_insert {
            self.insert_part_from(src);
        }
        self.win.parts_library = open;
    }

    /// The properties of a component or a part: the name, whether it is active, its contents, deletion.
    pub(super) fn component_props(&mut self, ui: &mut egui::Ui, ci: usize) {
        use qymcad_core::feature::FeatureKind;
        let cid = self.project.components[ci].id;
        let lin = self.lineage_of(Some(cid));
        // The ROOT's name is only displayed: it is a catalogue key rather than the document's text (`migrate_root`).
        let slot = if cid == self.project.root { NameSlot::Fixed(self.project.components[ci].name.clone()) } else { NameSlot::Editable(self.project.components[ci].name.clone()) };
        if let Some(n) = props_header(ui, ph::CUBE_TRANSPARENT, "props-component", slot, &lin) {
            self.project.components[ci].name = n;
        }
        let active = self.current_ctx_id() == cid;
        if active {
            ui.label(egui::RichText::new(&crate::i18n::tr("comp-active-context")).color(self.scheme.pal.hint()));
            if self.active_path.len() > 1 && ui.button(&crate::i18n::tr("comp-go-up")).clicked() {
                self.exit_context();
            }
        } else {
            ui.label(egui::RichText::new(&crate::i18n::tr("comp-not-active")).weak());
            if ui.button(format!("{} {}", ph::CUBE, crate::i18n::tr("props-enter-component"))).clicked() {
                self.set_context_to(cid);
            }
        }
        // the contents
        let (mut sk, mut ft) = (0, 0);
        for n in &self.project.timeline {
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
        if cid != self.project.root {
            ui.separator();
            ui.label(egui::RichText::new(&crate::i18n::tr("comp-placement")).strong());
            let mut t = self.project.component_transform(cid);
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
                self.project.set_component_transform(cid, t);
                self.after_placement_change();
            }
            let mut rot = self.opts.rot_deg;
            let mut rot_axis: Option<u8> = None;
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("comp-rotation"));
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
            self.opts.rot_deg = rot;
            if let Some(ax) = rot_axis {
                self.project.rotate_component(cid, ax, rot);
                self.after_placement_change();
            }
            let mut grounded = self.project.is_grounded(cid);
            let mut reset = false;
            let mut g_changed = false;
            ui.horizontal(|ui| {
                if ui.button(&crate::i18n::tr("comp-reset-placement")).clicked() {
                    reset = true;
                }
                g_changed = ui.checkbox(&mut grounded, &crate::i18n::tr("comp-grounded")).on_hover_text(&crate::i18n::tr("comp-grounded-hint")).changed();
            });
            if reset {
                self.project.set_component_transform(cid, qymcad_core::feature::PLACE_IDENTITY);
                self.after_placement_change();
            }
            if g_changed {
                self.project.set_grounded(cid, grounded);
            }
        }

        // This component's external (top-down) references: the list + breaking them
        let ext: Vec<(Id, Id)> = self.project.external_refs.iter().filter(|r| r.from_component == cid).filter_map(|r| r.source_body().map(|b| (r.id, b))).collect();
        if !ext.is_empty() {
            ui.separator();
            ui.label(egui::RichText::new(&crate::i18n::tr("comp-external-refs")).strong());
            let mut del: Option<Id> = None;
            for (rid, body) in &ext {
                let on = self.project.body_owner(*body).and_then(|o| self.project.components.iter().find(|c| c.id == o)).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::tr1("comp-on-face-of", "name", &on)).small());
                    if ui.small_button(ph::X).on_hover_text(&crate::i18n::tr("comp-break-ref-hint")).clicked() {
                        del = Some(*rid);
                    }
                });
            }
            if let Some(rid) = del {
                // not a raw deletion (which would leave a sketch on another part's face without authorisation, an
                // isolation error), but an honest break: the sketch planes are frozen into a copy in place
                let frozen = self.project.break_external_ref(rid);
                self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
                self.status = crate::i18n::tr1("comp-ref-broken", "n", &frozen.to_string());
            }
        }

        ui.separator();
        // Deleting a part from an assembly removes it WHOLE (with everything inside it), with a confirmation. Its
        // builds are NOT spilled into the root: what is wanted is the part gone, not its nodes scattered.
        if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("props-delete-component"))).clicked() {
            self.deferred.delete = Some(Sel::Component(ci));
        }
    }

    /// The properties of a datum AXIS: the name + the kind (manual or through two points) + an editor + delete.
    pub(super) fn datum_axis_props(&mut self, ui: &mut egui::Ui, i: usize) {
        use qymcad_core::model::AxisDef;
        let mut changed = false;
        let aid = self.project.datum_axes[i].id;
        let lin = self.lineage_of(Some(aid));
        if let Some(n) = props_header(ui, ph::LINE_SEGMENT, "props-datum-axis", NameSlot::Editable(self.project.datum_axes[i].name.clone()), &lin) {
            self.project.datum_axes[i].name = n;
        }
        // THE KIND: manual, or parametric through two datum points
        ui.horizontal(|ui| {
            ui.label(&crate::i18n::tr("pp-kind"));
            let is_2pt = matches!(self.project.datum_axes[i].def, AxisDef::TwoPoints { .. });
            if ui.selectable_label(!is_2pt, &crate::i18n::tr("pp-manual")).clicked() && is_2pt {
                // switching to manual mode FIXES the current coordinates as the definition
                let (o, d) = (self.project.datum_axes[i].origin(), self.project.datum_axes[i].dir());
                self.project.datum_axes[i].set_manual(o, d);
                changed = true;
            }
            if ui.selectable_label(is_2pt, &crate::i18n::tr("dax-two-points")).clicked() && !is_2pt {
                let pts: Vec<Id> = self.project.datum_points.iter().map(|p| p.id).collect();
                self.project.datum_axes[i].def = AxisDef::TwoPoints { a: pts.first().copied().unwrap_or(0), b: pts.get(1).copied().unwrap_or(0) };
                changed = true;
            }
        });
        if let AxisDef::TwoPoints { mut a, mut b } = self.project.datum_axes[i].def {
            let pts: Vec<(Id, String)> = self.project.datum_points.iter().map(|p| (p.id, crate::i18n::name(&p.name))).collect();
            if pts.len() < 2 {
                ui.label(egui::RichText::new(&crate::i18n::tr("dax-need-points")).color(self.scheme.pal.hint_action()).small());
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
            self.project.datum_axes[i].def = AxisDef::TwoPoints { a, b };
            let d = &self.project.datum_axes[i];
            ui.label(egui::RichText::new(crate::i18n::tr2("dax-origin-dir", "o", &format!("[{}, {}, {}]", crate::i18n::num(d.origin()[0],1), crate::i18n::num(d.origin()[1],1), crate::i18n::num(d.origin()[2],1)), "d", &format!("[{}, {}, {}]", crate::i18n::num(d.dir()[0],2), crate::i18n::num(d.dir()[1],2), crate::i18n::num(d.dir()[2],2)))).weak().small());
        } else {
            // a COPY is edited and the result put back through set_manual - the coordinates do not live beside a
            // parametric definition, they ARE the definition of a manual axis.
            let (mut o, mut dv) = (self.project.datum_axes[i].origin(), self.project.datum_axes[i].dir());
            ui.label(&crate::i18n::tr("pp-origin-mm"));
            egui::Grid::new(("dao", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
                changed |= drag(ui, "X", &mut o[0], 1.0, -100000.0..=100000.0);
                changed |= drag(ui, "Y", &mut o[1], 1.0, -100000.0..=100000.0);
                changed |= drag(ui, "Z", &mut o[2], 1.0, -100000.0..=100000.0);
            });
            ui.separator();
            ui.label(&crate::i18n::tr("dax-direction"));
            egui::Grid::new(("dad", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
                changed |= drag(ui, "Dx", &mut dv[0], 0.05, -1.0..=1.0);
                changed |= drag(ui, "Dy", &mut dv[1], 0.05, -1.0..=1.0);
                changed |= drag(ui, "Dz", &mut dv[2], 0.05, -1.0..=1.0);
            });
            let nl = (dv[0].powi(2) + dv[1].powi(2) + dv[2].powi(2)).sqrt();
            if nl > 1e-6 {
                for k in 0..3 {
                    dv[k] /= nl;
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
                self.project.datum_axes[i].set_manual(o, dv);
            }
        }
        if changed {
            self.datum.regen_pending = true; // the axis moved, so the consumers are rebuilt (debounced on release)
        }
        ui.separator();
        if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("props-delete-axis"))).clicked() {
            self.ask_delete(Sel::DatumAxis(i));
        }
    }

    /// THE ROWS OF THE PARAMETER TABLE. They were lifted out of the window into a METHOD OF THEIR OWN, because a
    /// field's width has to be checked in a REAL frame rather than against a number some function computed. The
    /// first attempt at making the fields elastic was checked by a test on the function and by a test that the
    /// call appeared in the source - both green, while the fields in the window stayed narrow. Now the test draws
    /// THIS method into a ui of a known width and measures how much the frame grew.
    ///
    /// Returns (whether anything was edited, what to delete).
    pub(super) fn params_rows_ui(&mut self, ui: &mut egui::Ui) -> super::ParamRowsOut {
        let mut out = super::ParamRowsOut::default();
        let (dirty, remove) = (&mut out.dirty, &mut out.remove);
        // THE FIELD WIDTHS ARE ELASTIC. They used to be a hard 90 and 120 points: the window stretched and the
        // fields did not, so a long variable name COULD NOT BE TYPED - the text crawled under the edge and one had
        // to type blind. The width comes from the ui being drawn into and is handed to EVERY field.
        let (w_name, w_expr) = super::param_field_widths(ui.available_width());
        let mut name_w = 0.0_f32;
        // THE LIST SCROLLS. Asked for: with some 150 variables and drivers, is there vertical scrolling? There
        // was not: the table simply grew, and at a hundred and fifty parameters the window ran off the edge of the
        // screen along with the Add button. The height is now capped and the contents scroll; when there is less
        // room than the cap, whatever there is gets used.
        let max_h = ui.available_height().clamp(120.0, super::PARAM_ROWS_MAX_H);
        // WHAT WAS ASKED FOR. The edits are gathered and applied AFTER the drawing: during it the document is lent
        // to the driver list (which reads the whole project), and changing the model mid-frame is the very thing
        // being moved away from.
        enum Act {
            Rename(usize, String),
            SetExpr(usize, String),
            /// A driving dimension's value (or a feature's parameter) is edited straight from the table.
            SetDriver(usize, f64),
            DropDriver(usize),
            /// Go to whatever was named: the dimension's sketch, or the feature.
            GoTo(usize),
        }
        let mut acts: Vec<Act> = Vec::new();
        // WHERE THE PATHS WERE DRAWN, for the tests: they move the mouse over the REAL coordinates.
        let mut paths: Vec<(usize, egui::Rect)> = Vec::new();
        // WHO HOLDS A NAME IS SAID IN WORDS, UNDER THE TABLE. What was asked for: either forbid identical names,
        // or make it clear which sketch, body or assembly each of them comes from.
        let mut refusal: Option<String> = None;
        let names: Vec<String> = self.project.parameters.iter().map(|p| p.name.clone()).collect();
        let exprs: Vec<String> = self.project.parameters.iter().map(|p| p.expr.clone()).collect();
        // A SEARCH ABOVE THE TABLE. At a hundred and fifty names the list cannot be read by eye. It searches both
        // the name and the path: a person remembers either what they called it or where it sits.
        ui.horizontal(|ui| {
            ui.label(ph::MAGNIFYING_GLASS);
            ui.add(egui::TextEdit::singleline(&mut self.par_search).desired_width(160.0).hint_text(&crate::i18n::tr("par-search")));
        });
        let q = self.par_search.trim().to_lowercase();
        let hit = |name: &str, path: &str| q.is_empty() || name.to_lowercase().contains(&q) || path.to_lowercase().contains(&q);
        let scrolled = egui::ScrollArea::vertical().max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
        egui::Grid::new("params_grid").num_columns(4).spacing([8.0, 4.0]).striped(true).show(ui, |ui| {
            ui.label(egui::RichText::new(&crate::i18n::tr("pp-name")).strong());
            ui.label(egui::RichText::new(&crate::i18n::tr("par-expression")).strong());
            ui.label(egui::RichText::new(&crate::i18n::tr("par-value")).strong());
            ui.label("");
            ui.end_row();
            for i in 0..names.len() {
                if !hit(&names[i], "") {
                    continue;
                }
                // THE NAME. It is edited in A BUFFER and goes into the document on Enter - it used to be written
                // into the model on every letter, and every formula referring to it broke on the very first one.
                let own = names[i].clone();
                let taken = |nm: &str| {
                    !nm.eq_ignore_ascii_case(&own) && self.project.name_owner(nm).is_some()
                };
                let ok = |nm: &str| qymcad_core::drivers::check_ident(nm).is_ok() && !taken(nm);
                let id = egui::Id::new(("par_name", i));
                let r = super::expr_field::name_field(ui, &self.project, id, &names[i], w_name, "w", &ok);
                name_w = name_w.max(r.resp.rect.width());
                if r.committed && r.text.trim() != names[i] {
                    acts.push(Act::Rename(i, r.text.trim().to_string()));
                }
                // THE EXPLANATION STAYS FOR AS LONG AS THE NAME IS BAD rather than flashing for one frame on
                // refusal. Measured: the first edition showed the line only in the frame where Enter was pressed -
                // that is, never. It is computed from the CURRENT text.
                let nm = r.text.trim().to_string();
                if !nm.is_empty() && !ok(&nm) {
                    refusal = Some(match self.project.name_owner(&nm) {
                        Some(o) if !nm.eq_ignore_ascii_case(&own) => {
                            // A GLOBAL PARAMETER HAS NO PATH, and giving it a "where this dimension sits" would be
                            // a lie: it is not a dimension and it sits nowhere.
                            let where_ = if o.path.is_empty() { crate::i18n::tr("par-owner-project") } else { o.path.clone() };
                            crate::i18n::tr2("par-name-taken", "name", &nm, "where", &where_)
                        }
                        _ => crate::i18n::tr1("par-name-bad", "name", &nm),
                    });
                }

                let id = egui::Id::new(("par_expr", i));
                let r = super::expr_field::expr_field(ui, &self.project, id, &exprs[i], w_expr, &crate::i18n::tr("par-example"));
                if r.committed && r.text != exprs[i] {
                    acts.push(Act::SetExpr(i, r.text.clone()));
                }

                // THE VALUE IS COMPUTED FROM WHAT IS IN THE FIELD RIGHT NOW rather than from what was recorded: the
                // answer is visible while typing, and it costs the document nothing.
                match self.project.eval_expr(&r.text) {
                    Ok(v) => {
                        ui.label(format!("{v:.3}"));
                    }
                    Err(e) => {
                        // THE REASON IN WORDS RATHER THAN AN ICON (see the history in expr_errors.rs).
                        //
                        // IN THE CELL ONLY A MARK: the value column is the narrowest of the four, and the reason
                        // does not fit it. Written into the cell as a plain label it ran off the edge of the
                        // window and was cut mid-word ("...a number or a na"); wrapped, it broke every second
                        // word onto its own line. The words themselves go below the table, across its whole
                        // width, where they can be read at a glance - see `params_window`.
                        out.errors.push((nm.clone(), crate::i18n::expr_error_text(&e)));
                        ui.label(egui::RichText::new("!").color(self.scheme.pal.error_mild()).small())
                            .on_hover_text(&crate::i18n::expr_error_text(&e));
                    }
                }
                if ui.button(ph::TRASH).clicked() {
                    *remove = Some(i);
                }
                ui.end_row();
            }
            // THE DRIVERS LIVE IN THE SAME TABLE, AND THEIR VALUES ARE EDITABLE THERE.
            //
            // They used to hang in a separate look-but-do-not-touch list: a name, a path, a number. This table is
            // the one place where the project's WHOLE set of numbers is both visible and editable.
            let drv: Vec<(usize, String, String, Option<f64>, bool)> = self
                .project
                .named_dims
                .iter()
                .enumerate()
                .map(|(k, n)| {
                    let dup = self.project.named_dims.iter().filter(|m| m.name.eq_ignore_ascii_case(&n.name)).count() > 1;
                    (k, n.name.clone(), self.project.driver_path(&n.target), self.project.named_dim_value(n), dup)
                })
                .filter(|(_, nm, path, _, _)| hit(nm, path))
                .collect();
            for (k, nm, path, val, dup) in drv {
                // THE NAME AND THE PATH. The path answers "which of the identically named ones"; for namesakes it
                // also carries the warning colour - a bare name in a formula is ambiguous for them.
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&nm).strong());
                    // A PATH IS A ROAD, NOT A CAPTION. A click leads to that very sketch or feature: the question
                    // "which sketch is this dimension from" gets answered by arriving there, rather than by hunting
                    // through the tree by hand.
                    let mut t = egui::RichText::new(&path).small();
                    t = if dup { t.color(ui.visuals().warn_fg_color) } else { t.weak() };
                    let r = ui
                        .add(egui::Label::new(t).sense(egui::Sense::click()))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(&crate::i18n::tr(if dup { "par-driver-ambiguous" } else { "par-driver-goto" }));
                    paths.push((k, r.rect));
                    if r.clicked() {
                        acts.push(Act::GoTo(k));
                    }
                });
                match val {
                    Some(v) => {
                        let id = egui::Id::new(("drv_val", k));
                        let shown = qymcad_core::expr::fmt_num(v);
                        let r = super::expr_field::expr_field(ui, &self.project, id, &shown, w_expr, &crate::i18n::tr("par-example"));
                        if r.committed && r.text.trim() != shown {
                            if let Ok(nv) = self.project.eval_expr(&r.text) {
                                acts.push(Act::SetDriver(k, nv));
                            }
                        }
                        ui.label(format!("{v:.3}"));
                    }
                    None => {
                        // THE DIMENSION IS GONE (its sketch or its constraint was deleted) - that is said out loud
                        // rather than shown as a blank: the driver is still in the formulas and has no value.
                        ui.label(egui::RichText::new(crate::i18n::tr1("par-dim-missing", "name", &nm)).weak());
                        ui.label("");
                    }
                }
                if ui.button(ph::TRASH).on_hover_text(&crate::i18n::tr("par-drop-driver")).clicked() {
                    acts.push(Act::DropDriver(k));
                }
                ui.end_row();
            }

            // A FEATURE'S UNNAMED NUMBERS DO NOT GET IN HERE.
            //
            // They were all shown at first, following the convention where a parameter table also holds the
            // model's parameters. What was asked for: remove from the global parameters the empty entries where no
            // driver name was given. The parameter list is what a person HAS NAMED; a hundred foreign rows with
            // empty names turn it into a dump. There is a place to name a feature's number: the feature's
            // properties in the right panel.
        });
        });
        self.tree.drv_path_rects = paths;
        if let Some(msg) = &refusal {
            ui.label(egui::RichText::new(msg).color(self.scheme.pal.warning()).small());
        }
        for a in acts {
            match a {
                Act::Rename(i, nm) => {
                    let old = self.project.parameters[i].name.clone();
                    // ONE OPERATION, ONE UNDO STEP, AND THE REFERENCES FOLLOW THE NAME.
                    let ed = self.edit(crate::i18n::tr("par-rename-step"));
                    let done = ed.app.project.rename_driver(&old, &nm);
                    drop(ed);
                    if done.is_err() {
                        self.status = crate::i18n::tr1("par-name-bad", "name", &nm);
                    }
                }
                Act::SetExpr(i, e) => {
                    let ed = self.edit(crate::i18n::tr("par-edit-step"));
                    ed.app.project.parameters[i].expr = e;
                    drop(ed);
                    *dirty = true;
                }
                Act::SetDriver(k, v) => {
                    let Some(target) = self.project.named_dims.get(k).map(|n| n.target.clone()) else { continue };
                    let ed = self.edit(crate::i18n::tr("par-edit-step"));
                    let done = ed.app.project.set_dim_target_value(&target, v);
                    // THE SKETCH IS RESOLVED EXACTLY ONCE - here, on commit - AND THE BODIES STANDING ON IT ARE
                    // MARKED FOR REBUILD.
                    //
                    // Without that second step came exactly the trouble that was reported: a driver was changed
                    // from 90 to 300, the green circle grew, and the part behind it never rebuilt. The general
                    // parameter pass (`apply_param_edit`) touches only the sketches that contain EXPRESSIONS - and
                    // a driving dimension is an ordinary NUMBER, so its turn never came. A silent answer: 300 in
                    // the table, 90 on the screen.
                    if let qymcad_core::model::DimTarget::Sketch { sketch, .. } = &target {
                        if let Some(si) = ed.app.project.sketch_index(*sketch) {
                            ed.app.project.solve_sketch(si);
                        }
                        ed.app.project.mark_sketch_dirty(*sketch);
                    }
                    drop(ed);
                    if done {
                        *dirty = true;
                    }
                }
                Act::GoTo(k) => {
                    // NAVIGATING IS NOT EDITING THE DOCUMENT: there must be no undo step here.
                    let Some(target) = self.project.named_dims.get(k).map(|n| n.target.clone()) else { continue };
                    match target {
                        qymcad_core::model::DimTarget::Sketch { sketch, .. } => {
                            if let Some(owner) = self.project.sketch_owner(sketch) {
                                self.enter_component(owner);
                            }
                            if let Some(si) = self.project.sketch_index(sketch) {
                                self.sel = Sel::Sketch(si);
                            }
                        }
                        qymcad_core::model::DimTarget::Feature { node, .. } => {
                            if let Some(owner) = self.project.timeline.iter().find(|n| n.id == node).and_then(|n| n.parent) {
                                self.enter_component(owner);
                            }
                            if let Some(ti) = self.project.timeline.iter().position(|n| n.id == node) {
                                self.sel = Sel::Feature(ti);
                            }
                        }
                    }
                }
                Act::DropDriver(k) => {
                    if k < self.project.named_dims.len() {
                        let ed = self.edit(crate::i18n::tr("par-edit-step"));
                        ed.app.project.named_dims.remove(k);
                        drop(ed);
                        *dirty = true;
                    }
                }
            }
        }
        out.name_w = name_w;
        out.height = scrolled.inner_rect.height();
        out.content_h = scrolled.content_size.y;
        out
    }

    pub(super) fn params_window(&mut self, ctx: &egui::Context) {
        use qymcad_core::model::Param;
        if !self.win.params {
            return;
        }
        let mut open = true;
        let mut dirty = false;
        let mut remove: Option<usize> = None;
        egui::Window::new(format!("{} {}", ph::FUNCTION, crate::i18n::tr("win-params"))).open(&mut open).resizable(true).default_width(360.0).show(ctx, |ui| {
            ui.label(egui::RichText::new(&crate::i18n::tr("par-hint")).weak().small());
            ui.separator();
            // THE FIELD WIDTHS ARE ELASTIC. They used to be a hard 90 and 120 points: the window stretched and the
            // fields did not, so a long variable name COULD NOT BE TYPED - the text crawled under the edge and one
            // had to type blind. The fields now share the window's width, and the hard numbers became a lower bound.
            let rows = self.params_rows_ui(ui);
            dirty |= rows.dirty;
            remove = rows.remove;
            // THE REASONS, BELOW THE TABLE AND ACROSS ITS WHOLE WIDTH. A cause that has to be hunted for by
            // hovering is a cause nobody reads, and the narrowest column of the table is no place for a sentence.
            for (name, msg) in &rows.errors {
                ui.add(egui::Label::new(egui::RichText::new(format!("{name}: {msg}")).color(self.scheme.pal.error_mild()).small()).wrap());
            }
            ui.separator();
            if ui.button(format!("{} {}", ph::PLUS, crate::i18n::tr("win-add-param"))).clicked() {
                self.project.parameters.push(Param { name: String::new(), expr: String::new(), value: 0.0 });
                dirty = true;
            }
            // THE DRIVERS LIVE IN THE TABLE ITSELF (see `params_rows_ui`) rather than in a separate list below.
            //
            // That separate list was look-but-do-not-touch: a name, a path, a number, and nothing more. This table
            // is the one place where the project's WHOLE set of numbers is visible and editable; two different
            // lists with different rules have no business here.
        });
        if let Some(i) = remove {
            self.project.parameters.remove(i);
            dirty = true;
        }
        if dirty {
            self.apply_param_edit();
        }
        if !open {
            self.win.params = false;
        }
    }

    /// A GLOBAL PARAMETER EDIT HAS BEEN APPLIED. A method of its own, because this path has to be TESTABLE: the
    /// parameter window draws itself, and a test must pull the same handle rather than a similar one of its own.
    pub(super) fn apply_param_edit(&mut self) {
        self.project.eval_parameters();
        // THE REBUILD GRAPH: editing a parameter touches only the sketches that actually mention it, not every one
        // of them. The features with expressions over that name are marked by `mark_changed_params_dirty` (by
        // comparing against a snapshot of the values), so there is nothing to duplicate here.
        for si in 0..self.project.sketches.len() {
            let uses = self.project.sketches[si].constraints.iter().any(|c| c.expr().is_some());
            if uses {
                self.project.solve_sketch(si);
                let sid = self.project.sketches[si].id;
                self.project.mark_sketch_dirty(sid);
            }
        }
        self.regenerate_all(); // the bodies rebuild associatively from the new parameters
    }

    pub(super) fn machine_props(&mut self, ui: &mut egui::Ui) {
        ui.heading(format!("{} {}", ph::WRENCH, crate::i18n::tr("menu-machine")));

        // the library of machine profiles
        let mut apply: Option<usize> = None;
        let mut remove: Option<usize> = None;
        ui.horizontal(|ui| {
            ui.label(&crate::i18n::tr("cam-machine-profile"));
            egui::ComboBox::from_id_salt("machlib")
                .selected_text(self.project.machine.name.clone())
                .show_ui(ui, |ui| {
                    for (idx, mm) in self.cam_job.machines.iter().enumerate() {
                        if ui.selectable_label(false, mm.name.clone()).clicked() {
                            apply = Some(idx);
                        }
                    }
                });
        });
        ui.horizontal(|ui| {
            if ui.button(format!("{} {}", ph::FLOPPY_DISK, crate::i18n::tr("cam-save-profile"))).clicked() {
                let name = self.project.machine.name.clone();
                if let Some(slot) = self.cam_job.machines.iter_mut().find(|x| x.name == name) {
                    *slot = self.project.machine.clone();
                } else {
                    self.cam_job.machines.push(self.project.machine.clone());
                }
            }
            let cur = self.project.machine.name.clone();
            if self.cam_job.machines.len() > 1 && ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("cam-delete-profile"))).clicked() {
                if let Some(idx) = self.cam_job.machines.iter().position(|x| x.name == cur) {
                    remove = Some(idx);
                }
            }
        });
        if let Some(idx) = apply {
            self.project.machine = self.cam_job.machines[idx].clone();
            self.invalidate();
        }
        if let Some(idx) = remove {
            self.cam_job.machines.remove(idx);
        }
        ui.separator();

        let m = &mut self.project.machine;
        ui.horizontal(|ui| {
            ui.label(&crate::i18n::tr("cam-name"));
            crate::gui::name_edit(ui, &mut m.name);
        });
        ui.horizontal(|ui| {
            ui.label(&crate::i18n::tr("cam-controller"));
            egui::ComboBox::from_id_salt("postkind")
                .selected_text(m.post.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut m.post, PostKind::Mach3, "Mach3");
                    ui.selectable_value(&mut m.post, PostKind::Grbl, "GRBL");
                    ui.selectable_value(&mut m.post, PostKind::LinuxCnc, "LinuxCNC");
                });
        });
        ui.separator();
        egui::Grid::new("machineg").num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            drag(ui, &crate::i18n::tr("cam-table-x"), &mut m.work_max[0], 10.0, 50.0..=5000.0);
            drag(ui, &crate::i18n::tr("cam-table-y"), &mut m.work_max[1], 10.0, 50.0..=5000.0);
            ui.label(&crate::i18n::tr("cam-depth-z"));
            let mut depth = -m.work_min[2];
            if ui.add(egui::DragValue::new(&mut depth).speed(5.0).range(1.0..=1000.0)).changed() {
                m.work_min[2] = -depth;
            }
            ui.end_row();
            drag(ui, &crate::i18n::tr("cam-rapid"), &mut m.max_rapid, 100.0, 100.0..=60000.0);
            drag(ui, &crate::i18n::tr("cam-max-feed"), &mut m.max_feed, 100.0, 100.0..=60000.0);
            drag(ui, &crate::i18n::tr("cam-max-rpm"), &mut m.max_rpm, 500.0, 1000.0..=120000.0);
        });
        ui.separator();
        egui::CollapsingHeader::new(&crate::i18n::tr("cam-post-processor")).id_salt("postcfg").show(ui, |ui| {
            let c = &mut self.project.machine.post_cfg;
            ui.checkbox(&mut c.comments, &crate::i18n::tr("cam-comments"));
            ui.checkbox(&mut c.header, &crate::i18n::tr("cam-header"));
            ui.checkbox(&mut c.line_numbers, &crate::i18n::tr("cam-line-numbers"));
            ui.checkbox(&mut c.tlo, &crate::i18n::tr("cam-tlo"));
            ui.checkbox(&mut c.translate_cycles, &crate::i18n::tr("cam-expand-cycles"));
            egui::Grid::new("postprec").num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
                ui.label(&crate::i18n::tr("cam-axis-precision"));
                ui.add(egui::DragValue::new(&mut c.axis_precision).range(1..=5));
                ui.end_row();
                ui.label(&crate::i18n::tr("cam-feed-precision"));
                ui.add(egui::DragValue::new(&mut c.feed_precision).range(1..=5));
                ui.end_row();
            });
        });

        ui.add_space(6.0);
        ui.label(egui::RichText::new(&crate::i18n::tr("cam-machine-persist")).weak().small());
    }

    /// THE SETTINGS WINDOW: the sections on the left, the search at the top, a per-section reset at the bottom.
    ///
    /// It used to be one flat scroll where the sections were bold labels. Such a list does not scale: the settings
    /// will treble, and a bedsheet can be neither searched nor reset in parts. The sections, their rows and their
    /// resets are declared IN ONE place (`settings_sections.rs`), otherwise a new setting would reach the window
    /// and not be found by the search - silently.
    ///
    /// THE "UNITS: MILLIMETRES" LABEL WAS REMOVED. It stood among the settings and looked like one, yet it
    /// switched nothing: inches were left to a separate piece of work later. An interface pretending to do what it
    /// cannot is worse than a missing item.
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.win.settings {
            return;
        }
        let mut open = self.win.settings;
        egui::Window::new(format!("{} {}", ph::GEAR, crate::i18n::tr("win-settings"))).open(&mut open).default_size([620.0, 460.0]).show(ctx, |ui| {
            let mut q = std::mem::take(&mut self.scheme.search);
            // the field's width does NOT come from the window's width - otherwise the window swells as text is typed (see the tree)
            ui.horizontal(|ui| {
                ui.label(ph::MAGNIFYING_GLASS);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut q)
                            .id(egui::Id::new("settings_search_field"))
                            .hint_text(&crate::i18n::tr("settings-search"))
                            .desired_width(f32::INFINITY),
                    );
                });
            });
            self.scheme.search = q;
            let query = self.scheme.search.clone();
            let searching = !query.trim().is_empty();
            ui.separator();

            let visible = self.settings_sections_visible();
            if visible.is_empty() {
                ui.label(egui::RichText::new(&crate::i18n::tr("settings-search-empty")).weak());
                return; // closing the window is handled by `open` OUTSIDE the closure

            }
            if !visible.contains(&self.scheme.section) {
                self.scheme.section = visible[0];
            }

            if searching {
                // THE SEARCH RUNS ACROSS THE SECTIONS: a person searches for a setting, not for a section, and is
                // under no obligation to know where it was put.
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for sec in visible {
                        ui.label(egui::RichText::new(crate::i18n::tr(sec.key())).strong());
                        self.settings_section_body(ui, ctx, sec, &query);
                        ui.separator();
                    }
                });
            } else {
                let cur = self.scheme.section;
                egui::SidePanel::left("settings_sections").resizable(false).exact_width(168.0).show_inside(ui, |ui| {
                    for sec in &visible {
                        if ui.selectable_label(cur == *sec, crate::i18n::tr(sec.key())).clicked() {
                            self.scheme.section = *sec;
                            self.scheme.note.clear();
                        }
                    }
                    ui.separator();
                    // WHERE THE CONFIG LIVES - otherwise support turns into guesswork
                    if let Some(dir) = crate::gui::settings_dir() {
                        ui.label(egui::RichText::new(crate::i18n::tr1("settings-config-path", "path", &dir.display().to_string())).small().weak());
                        // OPEN THE FOLDER in the system file manager. No separate crate is added for one button:
                        // this is a single OS command, and it reads plainly as one.
                        if ui.small_button(format!("{}  {}", ph::FOLDER_OPEN, crate::i18n::tr("settings-open-folder"))).clicked() {
                            let (bin, args) = crate::gui::reveal_command(ui.ctx().os(), &dir);
                            if let Err(e) = std::process::Command::new(bin).args(&args).spawn() {
                                self.scheme.note = crate::i18n::tr1("settings-open-folder-failed", "error", &e.to_string());
                            }
                        }
                    }
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(egui::RichText::new(crate::i18n::tr(cur.key())).strong());
                    ui.separator();
                    self.settings_section_body(ui, ctx, cur, "");
                    ui.separator();
                    if ui.button(format!("{}  {}", ph::ARROW_COUNTER_CLOCKWISE, crate::i18n::tr("settings-reset-section"))).clicked() {
                        cur.reset(&mut self.set);
                        // the language and the scheme are not merely values: they have to be APPLIED, otherwise a
                        // reset shows only after a restart
                        self.apply_language();
                        self.apply_theme(ctx);
                        self.scheme.note = crate::i18n::tr1("settings-reset-done", "name", &crate::i18n::tr(cur.key()));
                    }
                    if !self.scheme.note.is_empty() {
                        ui.label(egui::RichText::new(&self.scheme.note).small().color(self.scheme.pal.hint()));
                    }
                });
            }
        });
        self.win.settings = open;
    }

    /// WHICH SECTIONS ARE SHOWN AT ALL: CAM only when its module is enabled, and while searching, only those where
    /// something was found. ONE function for the window and for the tests alike: separate them, and a test starts
    /// checking something other than what a person sees.
    pub(super) fn settings_sections_visible(&self) -> Vec<super::settings_sections::SettingsSection> {
        use super::settings_sections::SettingsSection as Sec;
        let q = self.scheme.search.clone();
        let searching = !q.trim().is_empty();
        let cam_on = self.set.cam_tab_enabled;
        Sec::all().iter().copied().filter(|s| (!s.is_cam() || cam_on) && (!searching || s.has_match(&q))).collect()
    }

    /// A SECTION'S CONTENTS. An empty `query` shows everything; otherwise only the matching rows.
    ///
    /// Every row goes through `row`, and its key must appear in its section's `row_keys`: a guard checks that in
    /// both directions, which makes "present in the window but not searchable" inexpressible.
    fn settings_section_body(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, sec: super::settings_sections::SettingsSection, query: &str) {
        use super::settings_sections::SettingsSection as Sec;
        let show = |k: &str| Sec::row_matches(k, query);
        match sec {
            Sec::General => {
                // THE INTERFACE LANGUAGE. The list is built FROM THE `i18n/` CATALOGUE - drop a folder in and the
                // language appears; its name is shown in that language itself, so that it is recognised by whoever
                // does not read the current one.
                if show("settings-language") {
                    ui.label(&crate::i18n::tr("settings-language"));
                    ui.horizontal_wrapped(|ui| {
                        let cur = crate::i18n::language();
                        for (code, name) in crate::i18n::available() {
                            if ui.selectable_label(cur == code, &name).clicked() {
                                self.set.language = code.clone();
                                crate::i18n::set_language(&code);
                            }
                        }
                    });
                }
                // THE HELP'S LANGUAGE IS SEPARATE FROM THE INTERFACE'S. CAD terminology is English, and someone
                // working in a translated interface may well want to read `sweep` and `loft` as they are written in
                // the manuals. The first choice is "as in the interface": that is both the default and the way back,
                // with no guessing which code was the native one.
                if show("settings-help-lang") {
                    ui.label(&crate::i18n::tr("settings-help-lang"));
                    ui.horizontal_wrapped(|ui| {
                        if ui.selectable_label(self.set.help_lang.is_empty(), crate::i18n::tr("settings-help-lang-follow")).clicked() {
                            self.set.help_lang.clear();
                            crate::help::set_lang("");
                        }
                        for code in crate::help::languages() {
                            let name = crate::i18n::available().into_iter().find(|(c, _)| *c == code).map(|(_, n)| n).unwrap_or_else(|| code.clone());
                            if ui.selectable_label(self.set.help_lang == code, &name).clicked() {
                                self.set.help_lang = code.clone();
                                crate::help::set_lang(&code);
                            }
                        }
                    });
                }
                if show("settings-help-open") {
                    ui.horizontal(|ui| {
                        ui.label(&crate::i18n::tr("settings-help-open"));
                        ui.selectable_value(&mut self.set.help_external, false, crate::i18n::tr("settings-help-open-window"));
                        ui.selectable_value(&mut self.set.help_external, true, crate::i18n::tr("settings-help-open-browser"));
                    });
                    ui.label(egui::RichText::new(&crate::i18n::tr("settings-help-open-hint")).weak().small());
                }
                if show("settings-autosave") {
                    ui.horizontal(|ui| {
                        ui.label(&crate::i18n::tr("settings-autosave"));
                        ui.add(egui::DragValue::new(&mut self.set.autosave_secs).range(0..=3600).suffix(crate::i18n::tr("unit-seconds")));
                    });
                    ui.label(egui::RichText::new(&crate::i18n::tr("settings-autosave-hint")).weak().small());
                }
                if show("settings-undo-cap") {
                    ui.horizontal(|ui| {
                        ui.label(&crate::i18n::tr("settings-undo-cap"));
                        ui.add(egui::DragValue::new(&mut self.set.undo_cap).range(1..=500));
                    });
                    ui.label(egui::RichText::new(&crate::i18n::tr("settings-undo-cap-hint")).weak().small());
                }
                if show("settings-profile") {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(&crate::i18n::tr("settings-profile")).strong());
                    ui.horizontal(|ui| {
                        if ui.button(format!("{}  {}", ph::EXPORT, crate::i18n::tr("settings-profile-export"))).clicked() {
                            if let Some(p) = rfd::FileDialog::new().set_file_name("qym-cad-settings.ron").add_filter("qym-cad settings", &["ron"]).save_file() {
                                let path = p.to_string_lossy().into_owned();
                                self.scheme.note = match self.export_settings_to(&path) {
                                    Ok(()) => crate::i18n::tr1("settings-profile-saved", "path", &path),
                                    Err(e) => format!("{} {}", ph::WARNING, crate::i18n::tr1("settings-profile-failed", "error", &e)),
                                };
                            }
                        }
                        if ui.button(format!("{}  {}", ph::FOLDER_OPEN, crate::i18n::tr("settings-profile-import"))).clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("qym-cad settings", &["ron"]).pick_file() {
                                let path = p.to_string_lossy().into_owned();
                                self.scheme.note = match self.import_settings_from(&path, ctx) {
                                    Ok(()) => crate::i18n::tr("settings-profile-loaded"),
                                    // A BROKEN FILE DOES NOT TOUCH THE CURRENT SETTINGS: it is said that it did not
                                    // work, and what was there stays.
                                    Err(e) => format!("{} {}", ph::WARNING, crate::i18n::tr1("settings-profile-failed", "error", &e)),
                                };
                            }
                        }
                    });
                    ui.label(egui::RichText::new(&crate::i18n::tr("settings-profile-hint")).weak().small());
                }
                if show("settings-recent-limit") {
                    ui.horizontal(|ui| {
                        ui.label(&crate::i18n::tr("settings-recent-limit"));
                        let before = self.set.recent_limit;
                        ui.add(egui::DragValue::new(&mut self.set.recent_limit).range(1..=50));
                        if self.set.recent_limit < before {
                            // the list was shortened, so the excess is trimmed AT ONCE rather than at some next
                            // opening of the file: a setting must take effect where it is made
                            let n = self.set.recent_limit.max(1);
                            self.set.recent.truncate(n);
                        }
                    });
                }
            }
            Sec::Appearance => {
                // THE INTERFACE SCALE is applied LIVE - choosing a size blind, with the window closed, is
                // impossible. The step is coarse (10%): the intermediate values cannot be told apart by eye, and a
                // fine step turns the choice into fiddling.
                if show("settings-ui-scale") {
                    ui.horizontal(|ui| {
                        ui.label(&crate::i18n::tr("settings-ui-scale"));
                        let before = self.set.ui_scale;
                        ui.add(egui::DragValue::new(&mut self.set.ui_scale).speed(0.05).range(0.5..=3.0).fixed_decimals(2));
                        if (self.set.ui_scale - before).abs() > 1e-6 {
                            self.apply_ui_scale(ctx);
                        }
                        if ui.small_button(crate::i18n::tr("settings-ui-scale-reset")).clicked() {
                            self.set.ui_scale = 1.0;
                            self.apply_ui_scale(ctx);
                        }
                    });
                }
                if show("settings-scheme") {
                    ui.label(&crate::i18n::tr("settings-scheme"));
                    self.scheme_section(ui, ctx);
                }
            }
            Sec::Viewport => {
                // THE ORDER GOES FROM THE MAIN THING TO THE PARTICULAR, not however it happened to accumulate.
                //
                // It used to be: antialiasing, ghosts, field of view, projection, shading, the cube, precision.
                // First came what is touched once in a lifetime, while the projection - switched daily - hid in
                // the middle. The blocks were MOVED WHOLE: they carry side effects inside (clearing the raster
                // cache, re-uploading the vertex buffer), and rewriting them would have lost something silently.
                if show("settings-engine") {
                    if self.gpu_ok {
                        // the 3D viewport's engine. The GPU (wgpu, a depth buffer) is faster and free of
                        // visibility artefacts; the CPU raster is the compatible fallback.
                        ui.horizontal(|ui| {
                            ui.label(&crate::i18n::tr("settings-engine"));
                            let prev = self.set.gpu_viewport;
                            ui.selectable_value(&mut self.set.gpu_viewport, true, &crate::i18n::tr("settings-engine-gpu"));
                            ui.selectable_value(&mut self.set.gpu_viewport, false, &crate::i18n::tr("settings-engine-cpu"));
                            if prev != self.set.gpu_viewport {
                                *self.cache.view.borrow_mut() = None; // force a redraw when it is switched
                                self.cache.gpu_scene_key.set(u64::MAX); // force the GPU buffer to be re-uploaded
                            }
                        });
                    } else {
                        ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr("settings-gpu-unavailable"))).weak().small());
                    }
                }
                // The projection: one formula in project3, so it works on BOTH paths (GPU and CPU raster) and the
                // overlays (edges, dimensions, gizmos) stay glued to the bodies.
                if show("settings-projection") {
                    ui.horizontal(|ui| {
                        ui.label(&crate::i18n::tr("settings-projection"));
                        let prev = self.set.cam_perspective;
                        ui.selectable_value(&mut self.set.cam_perspective, false, &crate::i18n::tr("settings-projection-ortho"));
                        ui.selectable_value(&mut self.set.cam_perspective, true, &crate::i18n::tr("settings-projection-persp"));
                        if prev != self.set.cam_perspective {
                            *self.cache.view.borrow_mut() = None; // the CPU raster: the projection changed
                        }
                    });
                }
                // Shading: smooth (Gouraud, from the smoothed normals) or flat (from the face).
                if show("settings-shading") {
                    ui.horizontal(|ui| {
                        ui.label(&crate::i18n::tr("settings-shading"));
                        let prev = self.set.smooth_shading;
                        ui.selectable_value(&mut self.set.smooth_shading, true, &crate::i18n::tr("settings-shading-smooth"));
                        ui.selectable_value(&mut self.set.smooth_shading, false, &crate::i18n::tr("settings-shading-flat"));
                        if prev != self.set.smooth_shading {
                            *self.cache.view.borrow_mut() = None;
                            self.cache.gpu_scene_key.set(u64::MAX); // the GPU: re-upload the vertex buffer
                        }
                    });
                }
                // THE NAVIGATION CUBE'S SIZE: on a 4K screen the old one is unreadable, on a small screen a large one gets in the way
                if show("settings-viewcube") {
                    ui.label(&crate::i18n::tr("settings-viewcube"));
                    ui.horizontal(|ui| {
                        for (v, key) in [(0u8, "settings-viewcube-small"), (1, "settings-viewcube-medium"), (2, "settings-viewcube-large")] {
                            if ui.selectable_label(self.set.viewcube_size == v, crate::i18n::tr(key)).clicked() {
                                self.set.viewcube_size = v;
                            }
                        }
                    });
                }
                // POINTING PRECISION: it scales the grab radii of every role at once (see `grab.rs`). On a 4K or a
                // touch screen the pixel radii are small; with a mouse, large ones make it hard to aim in tight
                // geometry - that is a person's choice, not ours.
                if show("settings-pick-precision") {
                    ui.label(&crate::i18n::tr("settings-pick-precision"));
                    ui.horizontal(|ui| {
                        for (v, key) in [(0u8, "settings-pick-fine"), (1, "settings-pick-normal"), (2, "settings-pick-coarse")] {
                            if ui.selectable_label(self.set.pick_precision == v, crate::i18n::tr(key)).clicked() {
                                self.set.pick_precision = v;
                            }
                        }
                    });
                }
                if show("settings-ghost-alpha") {
                    ui.horizontal(|ui| {
                        ui.label(&crate::i18n::tr("settings-ghost-alpha"));
                        if ui.add(egui::Slider::new(&mut self.set.ghost_alpha, 20..=255)).changed() {
                            self.invalidate(); // the ghost is drawn from the raster cache, so the edit would not be seen otherwise
                        }
                    });
                }
                if show("settings-fov") {
                    // AN INAPPLICABLE SETTING GOES GREY AND SAYS WHY.
                    //
                    // A field of view exists only in perspective. Under an orthographic projection the slider
                    // looked alive, moved and changed nothing: a setting that pretends to work is worse than a
                    // missing one - a person turns it and concludes the program is broken. It is greyed out and the
                    // reason written beside it, not only in a tooltip: a tooltip has to be summoned first.
                    let persp = self.set.cam_perspective;
                    ui.add_enabled_ui(persp, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(&crate::i18n::tr("settings-fov"));
                            if ui.add(egui::Slider::new(&mut self.set.persp_fov_deg, 10.0..=90.0).suffix("°")).changed() {
                                self.invalidate();
                            }
                        });
                    })
                    .response
                    .on_disabled_hover_text(&crate::i18n::tr("settings-fov-needs-persp"));
                    if !persp {
                        ui.label(egui::RichText::new(&crate::i18n::tr("settings-fov-needs-persp")).weak().small());
                    }
                }
                if show("settings-msaa") {
                    // ANTIALIASING BELONGS TO THE GPU VIEWPORT ONLY: the software raster draws its own way, and
                    // the wgpu sample count does not affect it at all. On the CPU path the row looked alive.
                    let gpu = self.gpu_ok && self.set.gpu_viewport;
                    ui.add_enabled_ui(gpu, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(&crate::i18n::tr("settings-msaa"));
                        // THE LIST COMES FROM THE DEVICE, not from us. The specification guarantees only 1 and 4;
                        // 8x was once offered without asking, and on a real graphics card the program CRASHED ON
                        // START - the setting made it unlaunchable. What the hardware cannot do is not shown.
                        for n in crate::viewport_gpu::supported_msaa() {
                            let label = if n == 1 { crate::i18n::tr("settings-msaa-off") } else { format!("{n}×") };
                            if ui.selectable_label(self.set.msaa == n, label).clicked() {
                                self.set.msaa = n;
                            }
                        }
                    });
                    })
                    .response
                    .on_disabled_hover_text(&crate::i18n::tr("settings-msaa-needs-gpu"));
                    // THE SETTING'S PRICE IS SAID OUT LOUD. The sample count is baked into the wgpu pipelines at
                    // start-up; silently not applying it would be a lie, so it is stated plainly. And on the CPU
                    // raster what is said is not "restart" but that the setting has nothing to do with it.
                    ui.label(egui::RichText::new(if gpu { crate::i18n::tr("settings-msaa-restart") } else { crate::i18n::tr("settings-msaa-needs-gpu") }).weak().small());
                }
            }
            Sec::Sketch => {
                if show("settings-snap-on") {
                    ui.checkbox(&mut self.set.snap.on, &crate::i18n::tr("settings-snap-on"));
                }
                if show("settings-grid-step") {
                    ui.add_enabled(
                        self.set.snap.on,
                        egui::DragValue::new(&mut self.set.snap.grid).speed(0.5).range(0.1..=100.0).prefix(&crate::i18n::tr("settings-grid-step")).suffix(&crate::i18n::tr("unit-mm-suffix")),
                    );
                }
                if show("settings-rot-step") {
                    ui.add_enabled(self.set.snap.on, egui::DragValue::new(&mut self.set.snap.rot_deg).speed(1.0).range(0.5..=90.0).prefix(&crate::i18n::tr("settings-rot-step")).suffix("°"));
                }
                if show("settings-auto-constrain") {
                    ui.checkbox(&mut self.set.auto_constrain, &crate::i18n::tr("settings-auto-constrain")).on_hover_text(&crate::i18n::tr("settings-auto-constrain-hint"));
                }
            }
            Sec::Part => {
                egui::Grid::new("settings_def").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                    if show("settings-default-extrude") {
                        ui.label(&crate::i18n::tr("settings-default-extrude"));
                        ui.add(egui::DragValue::new(&mut self.set.defaults.extrude_h).speed(0.5).range(0.1..=5000.0).suffix(&crate::i18n::tr("unit-mm-suffix")));
                        ui.end_row();
                    }
                    if show("settings-default-offset") {
                        ui.label(&crate::i18n::tr("settings-default-offset"));
                        ui.add(egui::DragValue::new(&mut self.set.defaults.offset_2d).speed(0.2).range(0.1..=500.0).suffix(&crate::i18n::tr("unit-mm-suffix")));
                        ui.end_row();
                    }
                });
            }
            Sec::Assembly => {
                // The shared contours toggle is about the ASSEMBLY only: inside a Part every sketch has a
                // visibility checkbox of its own, and a shared one would duplicate it.
                if show("settings-show-contours") {
                    ui.checkbox(&mut self.set.show_contours, &crate::i18n::tr("settings-show-contours"));
                }
                if show("settings-show-joints") {
                    ui.checkbox(&mut self.set.show_joints, &crate::i18n::tr("settings-show-joints"));
                }
                if show("settings-show-interference") {
                    ui.checkbox(&mut self.set.show_interference, &crate::i18n::tr("settings-show-interference"));
                }
            }
            Sec::Cam => {
                // EVERYTHING ABOUT MACHINING LIVES HERE AND NOWHERE ELSE. The rapid moves used to sit under
                // Display beside the sketch contours: covered by the checkbox test, but in the wrong section.
                if show("cam-tab-checkbox") {
                    let was_cam = self.set.cam_tab_enabled;
                    ui.checkbox(&mut self.set.cam_tab_enabled, format!("{} {}", ph::GEAR, crate::i18n::tr("cam-tab-checkbox")))
                        .on_hover_text(&crate::i18n::tr("settings-cam-tab-hint"));
                    ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr("settings-cam-wip"))).color(self.scheme.pal.warning()).small());
                    if was_cam && !self.set.cam_tab_enabled && self.cam_mode {
                        // the tab was switched off from within CAM itself, so CAD is entered at once
                        self.cam_mode = false;
                        self.sync_workbench();
                    }
                }
                if show("settings-rapids") {
                    ui.checkbox(&mut self.set.show_rapids, &crate::i18n::tr("settings-rapids"));
                }
            }
        }
    }

    /// THE COLOUR SCHEME SECTION: choosing one, making a copy of one's own, the colour editor.
    ///
    /// The edits go in LIVE, with no Apply button: choosing a shade blind - close the window, look, come back - is
    /// impossible. Writing to the file is a separate button: until it is written, this is a draft in memory and a
    /// restart brings back what was there.
    ///
    /// THE BUILT-IN SCHEMES ARE NOT EDITED. The dark one must stay exactly as it is: someone said they liked the
    /// look, and "put it back the way it was" has to be backed by something. Editing a built-in one makes a copy by
    /// itself, as material editors in grown-up CAD packages do.
    pub(super) fn scheme_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            // THE SCHEMES COME FROM THE LIST: there will be more than two of one's own, and "dark/light" cannot express that
            let rows: Vec<(String, String, bool)> = self.scheme.all.iter().map(|p| (p.id.clone(), p.title(), p.light)).collect();
            for (id, title, light) in rows {
                let own = !crate::palette::store::is_builtin(&id);
                let mark = if own { ph::PENCIL_SIMPLE } else if light { ph::SUN } else { ph::MOON };
                if ui.selectable_label(self.set.scheme == id, format!("{mark} {title}")).clicked() {
                    self.set.scheme = id.clone();
                    self.apply_theme(ctx);
                }
            }
        });

        let own = !crate::palette::store::is_builtin(&self.set.scheme);
        ui.horizontal(|ui| {
            let label = crate::i18n::tr(if own { "scheme-edit" } else { "scheme-duplicate" });
            if ui.button(format!("{}  {label}", ph::PALETTE)).clicked() {
                if own {
                    self.scheme.edit.open = !self.scheme.edit.open;
                } else {
                    self.duplicate_scheme(ctx);
                }
            }
            if own && ui.button(format!("{}  {}", ph::TRASH, crate::i18n::tr("scheme-delete"))).clicked() {
                let id = self.set.scheme.clone();
                let title = self.scheme.pal.title();
                self.scheme.edit.note = match crate::palette::store::delete(&id) {
                    Ok(()) => crate::i18n::tr1("scheme-deleted", "name", &title),
                    Err(e) => crate::i18n::tr1("scheme-delete-failed", "error", &e),
                };
                self.set.scheme = crate::palette::dark().id;
                self.scheme.edit.open = false;
                self.reload_schemes();
                self.apply_theme(ctx);
            }
            if own && ui.button(format!("{}  {}", ph::FLOPPY_DISK, crate::i18n::tr("scheme-save"))).clicked() {
                let pal = self.scheme.pal.clone();
                self.scheme.edit.note = match crate::palette::store::save(&pal) {
                    Ok(p) => crate::i18n::tr1("scheme-saved", "path", &p.display().to_string()),
                    Err(e) => crate::i18n::tr1("scheme-save-failed", "error", &e),
                };
                self.reload_schemes();
            }
        });
        if !self.scheme.edit.note.is_empty() {
            ui.label(egui::RichText::new(&self.scheme.edit.note).small().color(self.scheme.pal.hint()));
        }
        if own && self.scheme.edit.open {
            self.scheme_editor(ui, ctx);
        }
    }

    /// Make a scheme of one's own as a copy of the current one and switch to it at once.
    fn duplicate_scheme(&mut self, ctx: &egui::Context) {
        let existing: Vec<String> = self.scheme.all.iter().map(|p| p.id.clone()).collect();
        let mut copy = self.scheme.pal.clone();
        // THE LABEL IS TAKEN BEFORE THE IDENTIFIER CHANGES: a built-in scheme has no name of its own, and `title()`
        // goes to the language catalogue for it - after the id is replaced it would find something else there.
        let was = copy.title();
        copy.id = crate::palette::store::unique_copy_id(&copy.id, &existing);
        // THE LABEL IS IN WORDS, NOT A MACHINE KEY. It used to read like "Light (light-1)": the identifier is of no
        // use to a person at all, and it exists precisely so that it need not be seen.
        copy.name = crate::i18n::tr1("scheme-copy-of", "name", &was);
        let mut n = 2;
        while self.scheme.all.iter().any(|p| p.title() == copy.name) {
            copy.name = crate::i18n::tr2("scheme-copy-of-n", "name", &was, "n", &n.to_string());
            n += 1;
        }
        self.scheme.edit.note = match crate::palette::store::save(&copy) {
            Ok(p) => crate::i18n::tr2("scheme-created", "name", &copy.name, "path", &p.display().to_string()),
            Err(e) => crate::i18n::tr1("scheme-create-failed", "error", &e),
        };
        self.set.scheme = copy.id.clone();
        self.scheme.edit.rename = copy.name.clone();
        self.scheme.edit.open = true;
        self.reload_schemes();
        self.apply_theme(ctx);
    }

    /// THE EDITOR ITSELF: the name, light or dark, the shading fractions and every colour by section.
    fn scheme_editor(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("scheme-name"));
            if self.scheme.edit.rename.is_empty() {
                self.scheme.edit.rename = self.scheme.pal.title();
            }
            ui.add(egui::TextEdit::singleline(&mut self.scheme.edit.rename).desired_width(180.0));
            if ui.button(crate::i18n::tr("scheme-rename")).clicked() {
                // THE LABEL IS RENAMED while the identifier stays: it lives in the settings and in the file name,
                // and changing it along with the label would mean losing the chosen scheme.
                let new = self.scheme.edit.rename.trim().to_string();
                if new.is_empty() {
                    self.scheme.edit.note = crate::i18n::tr("scheme-name-taken");
                } else {
                    self.scheme.pal.name = new;
                    let pal = self.scheme.pal.clone();
                    // A RENAME SAYS WHERE THE FILE MOVED TO. The result used to be swallowed silently (`let _ =`),
                    // and the "written" message came only from Save - with the former, by then wrong, file name.
                    self.scheme.edit.note = match crate::palette::store::save(&pal) {
                        Ok(p) => crate::i18n::tr1("scheme-saved", "path", &p.display().to_string()),
                        Err(e) => crate::i18n::tr1("scheme-save-failed", "error", &e),
                    };
                    self.reload_schemes();
                }
            }
        });
        // light or dark: egui's own appearance depends on it, not only the canvas colours
        let mut light = self.scheme.pal.light;
        if ui.checkbox(&mut light, crate::i18n::tr("scheme-is-light")).changed() {
            self.scheme.pal.light = light;
            self.sync_visuals(ctx);
            self.repaint_after_scheme_edit();
        }
        // WHETHER TO PAINT THE INTERFACE ITSELF. Off: the panels and buttons take egui's factory look, exactly as
        // it was before the Interface section existed; on: the twelve colours from it are in force.
        let mut ui_on = self.scheme.pal.ui_on;
        if ui.checkbox(&mut ui_on, crate::i18n::tr("scheme-ui-on")).on_hover_text(crate::i18n::tr("scheme-ui-on-hint")).changed() {
            self.scheme.pal.ui_on = ui_on;
            self.sync_visuals(ctx);
            self.repaint_after_scheme_edit();
        }

        egui::CollapsingHeader::new(crate::i18n::tr("scheme-shading")).show(ui, |ui| {
            let mut changed = false;
            for (key, v) in [
                ("shade-body", &mut self.scheme.pal.shade_floor_body),
                ("shade-mesh", &mut self.scheme.pal.shade_floor_mesh),
                ("shade-viewcube", &mut self.scheme.pal.shade_floor_viewcube),
                ("body-lighten", &mut self.scheme.pal.body_lighten),
                ("body-saturate", &mut self.scheme.pal.body_saturate),
            ] {
                let label = crate::i18n::tr(&format!("scheme-{key}"));
                let hint = crate::i18n::tr(&format!("scheme-{key}-hint"));
                changed |= ui.add(egui::Slider::new(v, 0.0..=1.0).text(label)).on_hover_text(hint).changed();
            }
            if changed {
                self.repaint_after_scheme_edit();
            }
        });

        for (section, rows) in crate::palette::groups() {
            let title = crate::i18n::tr(&format!("scheme-group-{section}"));
            egui::CollapsingHeader::new(title).show(ui, |ui| {
                egui::Grid::new(section).num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
                    for key in rows {
                        let label = crate::i18n::tr(&format!("scheme-color-{key}"));
                        let mut rgb = self.scheme.pal.entries().iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or([0, 0, 0]);
                        if ui.color_edit_button_srgb(&mut rgb).changed() {
                            // THROUGH `set` BY NAME: a typo in a key is rejected rather than writing a colour into
                            // the wrong place - the same device as in a user scheme's file.
                            if self.scheme.pal.set(key, rgb) {
                                self.repaint_after_scheme_edit();
                            }
                        }
                        ui.label(label);
                        ui.end_row();
                    }
                });
            });
        }
    }

    /// Editing a colour is NOT editing the document: the picture is repainted while the project stays clean.
    fn repaint_after_scheme_edit(&mut self) {
        *self.cache.view.borrow_mut() = None; // the CPU raster is painted by the scheme, so it is recomputed
        self.cache.gpu_scene_key.set(u64::MAX); // the GPU: re-upload the vertex buffer
    }

    pub(super) fn tools_window(&mut self, ctx: &egui::Context) {
        if !self.win.tools || !self.set.cam_tab_enabled {
            return;
        }
        let mut open = self.win.tools;
        egui::Window::new(format!("{} {}", ph::SCREWDRIVER, crate::i18n::tr("cam-tool-library"))).open(&mut open).default_size([460.0, 320.0]).show(ctx, |ui| {
            if ui.button(format!("{} {}", ph::PLUS, crate::i18n::tr("cam-add"))).clicked() {
                let next = self.project.tools.iter().map(|t| t.number).max().unwrap_or(0) + 1;
                self.project.tools.push(default_tool(next));
            }
            ui.separator();
            let mut remove: Option<usize> = None;
            egui::Grid::new("toolswin").num_columns(6).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label("№");
                ui.label(&crate::i18n::tr("cam-name"));
                ui.label(&crate::i18n::tr("cam-tool-type"));
                ui.label("Ø");
                ui.label(&crate::i18n::tr("cam-flutes"));
                ui.label("");
                ui.end_row();
                for i in 0..self.project.tools.len() {
                    let t = &mut self.project.tools[i];
                    ui.add(egui::DragValue::new(&mut t.number).range(1..=999));
                    ui.text_edit_singleline(&mut t.name);
                    tool_type_combo(ui, 1000 + i, &mut t.kind);
                    ui.add(egui::DragValue::new(&mut t.diameter).speed(0.1).range(0.1..=50.0));
                    ui.add(egui::DragValue::new(&mut t.flutes).range(1..=12));
                    if ui.small_button(ph::TRASH).clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if let Some(i) = remove {
                if self.project.tools.len() > 1 {
                    self.project.tools.remove(i);
                }
            }

            // --- The global library (shared across projects, in the OS config directory) ---
            ui.add_space(10.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-global-library")).strong());
                if ui.button(format!("{} {}", ph::FLOPPY_DISK, crate::i18n::tr("cam-save-project-tools"))).on_hover_text(&crate::i18n::tr("cam-save-project-tools-hint")).clicked() {
                    for tl in &self.project.tools {
                        if !self.cam_job.tools.tools.iter().any(|x| x.name == tl.name && (x.diameter - tl.diameter).abs() < 1e-6) {
                            self.cam_job.tools.tools.push(tl.clone());
                        }
                    }
                    match crate::library::save_tool_library(&self.cam_job.tools) {
                        Ok(()) => self.status = crate::i18n::tr("cam-tools-added"),
                        Err(e) => self.status = crate::i18n::tr1("cam-library-error", "error", &e.to_string()),
                    }
                }
            });
            let mut import: Option<usize> = None;
            let mut lib_remove: Option<usize> = None;
            egui::Grid::new("libgrid").num_columns(5).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
                for li in 0..self.cam_job.tools.tools.len() {
                    let lt = &self.cam_job.tools.tools[li];
                    ui.label(&lt.name);
                    ui.label(crate::i18n::tr(tool_type_label(lt.kind)));
                    ui.label(format!("Ø{}", lt.diameter));
                    if ui.small_button(&crate::i18n::tr("cam-to-project")).clicked() {
                        import = Some(li);
                    }
                    if ui.small_button(ph::TRASH).clicked() {
                        lib_remove = Some(li);
                    }
                    ui.end_row();
                }
            });
            if let Some(li) = import {
                let next = self.project.tools.iter().map(|t| t.number).max().unwrap_or(0) + 1;
                let mut tl = self.cam_job.tools.tools[li].clone();
                tl.number = next;
                self.project.tools.push(tl);
                self.status = crate::i18n::tr1("cam-tool-imported", "t", &next.to_string());
            }
            if let Some(li) = lib_remove {
                self.cam_job.tools.tools.remove(li);
                let _ = crate::library::save_tool_library(&self.cam_job.tools);
            }
        });
        self.win.tools = open;
    }

    /// An operation's geometry: pick a part or a contour in the tree or in 3D/2D, then Add;
    /// a list where every entry can be removed.
    pub(super) fn op_geometry_editor(&mut self, ui: &mut egui::Ui, i: usize) {
        let is3d = mesh_of_kind(self.project.operations[i].kind).is_some();

        // --- The parts (for the 3D operations) ---
        if is3d {
            ui.label(egui::RichText::new(&crate::i18n::tr("cam-op-parts")).strong());
            ui.horizontal(|ui| {
                let picking = self.op_pick == Some(OpPick::Body);
                let btn = egui::Button::new(format!("{} {}", ph::PLUS, crate::i18n::tr("cam-add-part"))).selected(picking);
                if ui.add(btn).on_hover_text(&crate::i18n::tr("cam-add-part-hint")).clicked() {
                    self.op_pick = if picking { None } else { Some(OpPick::Body) };
                }
            });
            if self.op_pick == Some(OpPick::Body) {
                ui.label(egui::RichText::new(&crate::i18n::tr("cam-pick-part-hint")).color(self.scheme.pal.hint_action()).small());
            }
            let bodies = self.project.operations[i].bodies.clone();
            if bodies.is_empty() {
                let def = mesh_of_kind(self.project.operations[i].kind).unwrap_or(0);
                let nm = self.project.mesh_index(def).map(|k| crate::i18n::name(&self.project.mesh_name(k))).unwrap_or_else(|| crate::i18n::tr("cam-not-set"));
                ui.label(egui::RichText::new(crate::i18n::tr1("cam-default-is", "name", &nm)).weak().small());
            } else {
                let mut rm: Option<Id> = None;
                for id in &bodies {
                    let nm = self.project.mesh_index(*id).map(|k| crate::i18n::name(&self.project.mesh_name(k))).unwrap_or_else(|| format!("#{id}?"));
                    ui.horizontal(|ui| {
                        ui.label(format!("{} {}", ph::CUBE, nm));
                        if ui.small_button(ph::X).clicked() {
                            rm = Some(*id);
                        }
                    });
                }
                if let Some(id) = rm {
                    self.project.operations[i].bodies.retain(|x| *x != id);
                    self.invalidate();
                }
            }
            ui.separator();
        }

        // --- The contours (for every operation; for 3D, a boundary or a projection) ---
        ui.label(egui::RichText::new(if is3d { crate::i18n::tr("cam-contours-area") } else { crate::i18n::tr("cam-op-contours") }).strong());
        ui.horizontal(|ui| {
            let picking = self.op_pick == Some(OpPick::Contour);
            let btn = egui::Button::new(format!("{} {}", ph::PLUS, crate::i18n::tr("cam-add-contour"))).selected(picking);
            if ui.add(btn).on_hover_text(&crate::i18n::tr("cam-add-contour-hint")).clicked() {
                self.op_pick = if picking { None } else { Some(OpPick::Contour) };
            }
            if !self.project.operations[i].selection.is_empty() && ui.button(&crate::i18n::tr("cam-clear-all")).on_hover_text(&crate::i18n::tr("cam-clear-all-hint")).clicked() {
                self.project.operations[i].selection.clear();
                self.invalidate();
            }
        });
        if self.op_pick == Some(OpPick::Contour) {
            ui.label(egui::RichText::new(&crate::i18n::tr("cam-pick-contour-hint")).color(self.scheme.pal.hint_action()).small());
        }
        let selc = self.project.operations[i].selection.clone();
        if selc.is_empty() {
            ui.label(egui::RichText::new(&crate::i18n::tr("cam-empty-means-all")).weak().small());
        } else {
            let mut rm: Option<Id> = None;
            for id in &selc {
                let lbl = self.project.contour_index(*id).map(|k| crate::i18n::tr1("cam-contour-n", "n", &(k + 1).to_string())).unwrap_or_else(|| format!("#{id}?"));
                ui.horizontal(|ui| {
                    ui.label(format!("{} {}", ph::POLYGON, lbl));
                    if ui.small_button(ph::X).clicked() {
                        rm = Some(*id);
                    }
                });
            }
            if let Some(id) = rm {
                self.project.operations[i].selection.retain(|x| *x != id);
                self.invalidate();
            }
        }
        ui.separator();
    }

    pub(super) fn properties_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("props").resizable(true).default_width(290.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(4.0);
                // the CAM properties (the machine, the stock, the tool, the setup, the operation) belong to the CAM workbench only
                let cam = self.workbench.is_cam();
                match self.sel {
                    Sel::Machine if cam => self.machine_props(ui),
                    Sel::Stock if cam => self.stock_props(ui),
                    Sel::Mesh(i) if i < self.project.bodies.len() => self.mesh_props(ui, i),
                    Sel::Face(mi, fi) if self.project.bodies.get(mi).map_or(false, |b| fi < b.faces.len()) => self.face_props(ui, mi, fi),
                    Sel::Contour(i) if i < self.project.contours.len() => self.contour_props(ui, i),
                    Sel::Sketch(i) if i < self.project.sketches.len() => self.sketch_props(ui, i),
                    Sel::Plane(i) if i < self.project.planes.len() => self.plane_props(ui, i),
                    Sel::DatumPoint(i) if i < self.project.datum_points.len() => self.datum_point_props(ui, i),
                    Sel::DatumAxis(i) if i < self.project.datum_axes.len() => self.datum_axis_props(ui, i),
                    Sel::Component(i) if i < self.project.components.len() => self.component_props(ui, i),
                    Sel::Feature(i) if i < self.project.timeline.len() => self.feature_props(ui, i),
                    Sel::Tool(i) if cam && i < self.project.tools.len() => self.tool_props(ui, i),
                    Sel::Setup(i) if cam && i < self.project.setups.len() => self.setup_props(ui, i),
                    Sel::Op(i) if cam && i < self.project.operations.len() => self.op_editor(ui, i),
                    _ => {
                        ui.heading(&crate::i18n::tr("props-title"));
                        ui.label(egui::RichText::new(&crate::i18n::tr("props-pick-in-tree")).weak());
                        if !cam {
                            ui.separator();
                            ui.label(egui::RichText::new(&crate::i18n::tr("props-new-sketch-on")).strong());
                            use qymcad_core::feature::{BasePlane, SketchPlane};
                            ui.horizontal(|ui| {
                                if ui.button(&crate::i18n::tr("plane-xy-table")).clicked() {
                                    self.create_sketch_on(SketchPlane::World(BasePlane::XY));
                                }
                                if ui.button(&crate::i18n::tr("plane-xz-front")).clicked() {
                                    self.create_sketch_on(SketchPlane::World(BasePlane::XZ));
                                }
                                if ui.button(&crate::i18n::tr("plane-yz-side")).clicked() {
                                    self.create_sketch_on(SketchPlane::World(BasePlane::YZ));
                                }
                            });
                        }
                    }
                }
                // the mates are always available in an Assembly context
                if matches!(self.workbench, Workbench::Assembly) {
                    ui.separator();
                    self.joints_panel(ui);
                }
            });
        });
    }

    /// THE DOCUMENT PROPERTIES WINDOW. The document used to be a nameless heap of geometry.
    ///
    /// The fields are free text on purpose: one person's version is `1.2` and another's is `rev. B`, and an imposed
    /// format would only be worked around in a comment. The program does not interpret them - it stores and shows them.
    ///
    /// The edits go STRAIGHT INTO THE DOCUMENT, as everything else does: the project becomes dirty and the "not
    /// saved?" question is asked on exit. That is right in substance - the properties travel WITH THE FILE.
    pub(super) fn save_template_dialog(&mut self, ctx: &egui::Context) {
        if !self.win.save_template {
            return;
        }
        let mut done = false;
        let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        egui::Window::new(format!("{} {}", ph::PACKAGE, crate::i18n::tr("file-save-as-template")))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(&crate::i18n::tr("tpl-name"));
                let r = ui.text_edit_singleline(&mut self.win.tpl_name);
                r.request_focus();
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    done = true;
                }
                ui.label(egui::RichText::new(&crate::i18n::tr("tpl-hint")).weak().small());
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(!self.win.tpl_name.trim().is_empty(), egui::Button::new(&crate::i18n::tr("confirm-yes"))).clicked() {
                        done = true;
                    }
                    if ui.button(&crate::i18n::tr("confirm-no")).clicked() {
                        cancel = true;
                    }
                });
            });
        if done && !self.win.tpl_name.trim().is_empty() {
            let name = self.win.tpl_name.trim().to_string();
            self.save_as_template(&name);
            self.win.save_template = false;
        } else if cancel {
            self.win.save_template = false;
        }
    }

    pub(super) fn doc_props_window(&mut self, ctx: &egui::Context) {
        if !self.win.doc_props {
            return;
        }
        let mut open = self.win.doc_props;
        egui::Window::new(format!("{} {}", ph::FILE_TEXT, crate::i18n::tr("doc-props-title")))
            .open(&mut open)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                egui::Grid::new("doc_props").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label(&crate::i18n::tr("doc-props-name"));
                    ui.text_edit_singleline(&mut self.project.meta.title);
                    ui.end_row();
                    ui.label(&crate::i18n::tr("doc-props-author"));
                    ui.text_edit_singleline(&mut self.project.meta.author);
                    ui.end_row();
                    ui.label(&crate::i18n::tr("doc-props-version"));
                    ui.text_edit_singleline(&mut self.project.meta.version);
                    ui.end_row();
                });
                ui.label(&crate::i18n::tr("doc-props-comment"));
                ui.add(egui::TextEdit::multiline(&mut self.project.meta.comment).desired_rows(4).desired_width(f32::INFINITY));
                ui.separator();
                // THE GEOMETRY TOLERANCE LIVES HERE, NOT IN THE PROGRAM'S SETTINGS. Both the look on screen and the
                // contents of an STL depend on it: were it a program setting, one file would export differently for
                // two people, and both would be sure the program was lying.
                ui.label(egui::RichText::new(&crate::i18n::tr("doc-props-quality")).strong());
                ui.horizontal_wrapped(|ui| {
                    for q in qymcad_core::model::GeomQuality::all() {
                        if ui.selectable_label(self.project.geom_quality == q, crate::i18n::tr(q.label_key())).clicked() && self.project.geom_quality != q {
                            self.begin_edit(&crate::i18n::tr("doc-props-quality")); // THE EDIT BOUNDARY: this changes the document and can be undone
                            self.project.geom_quality = q;
                            // the geometry must be recomputed: the tolerance changes THE MESH, not just a number
                            for n in self.project.timeline.iter_mut() {
                                if n.kind.body().is_some() {
                                    n.dirty = true;
                                }
                            }
                            self.commit_edit();
                            self.mark_dirty_for_rebuild();
                        }
                    }
                });
                ui.label(egui::RichText::new(&crate::i18n::tr("doc-props-quality-hint")).weak().small());
                ui.separator();
                // THE FACTS THE PROGRAM KNOWS BY ITSELF - they are read, not edited
                let created = self.project.meta.created.clone();
                ui.label(
                    egui::RichText::new(if created.is_empty() {
                        crate::i18n::tr("doc-props-not-saved-yet")
                    } else {
                        crate::i18n::tr1("doc-props-created", "when", &created)
                    })
                    .small()
                    .weak(),
                );
                let counts = crate::i18n::tr2("doc-props-counts", "parts", &self.project.components.len().to_string(), "bodies", &self.project.bodies.len().to_string());
                ui.label(egui::RichText::new(counts).small().weak());
                if let Some(p) = self.project_path.clone() {
                    ui.label(egui::RichText::new(crate::i18n::tr1("doc-props-path", "path", &p)).small().weak());
                }
                // WHICH BUILD WROTE THIS FILE. Shown, not editable: it is the program's word about itself,
                // and the person's own version lives in the field above. Empty for a document that has
                // never been saved, and then there is nothing to say.
                if !self.project.meta.saved_by.is_empty() {
                    let by = self.project.meta.saved_by.clone();
                    ui.label(egui::RichText::new(crate::i18n::tr1("doc-props-saved-by", "build", &by)).small().weak());
                }
            });
        self.win.doc_props = open;
    }

    /// "THE LAST RUN ENDED IN AN ERROR" - shown once, when a report from an earlier run is found.
    ///
    /// Without this the reports pile up in a directory nobody has heard of. The window says where the
    /// file is and hands the path over, because the next thing asked of the person is to attach it.
    pub(super) fn crash_notice(&mut self, ctx: &egui::Context) {
        let Some(path) = self.crash_report.clone() else { return };
        let mut open = true;
        let mut dismiss = false;
        egui::Window::new(format!("{} {}", ph::WARNING, crate::i18n::tr("crash-title")))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(&crate::i18n::tr("crash-what"));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(crate::crash::without_home(&path.to_string_lossy())).monospace().small());
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(format!("{} {}", ph::COPY, crate::i18n::tr("crash-copy-path"))).clicked() {
                        ui.output_mut(|o| o.copied_text = path.to_string_lossy().into_owned());
                    }
                    if ui.button(&crate::i18n::tr("close")).clicked() {
                        dismiss = true;
                    }
                });
            });
        if dismiss || !open {
            // Renamed rather than deleted: the person may still want to attach it to a report.
            crate::crash::mark_seen(&path);
            self.crash_report = None;
        }
    }

    /// The About window (Help -> About).
    pub(super) fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.win.about {
            return;
        }
        let mut open = true;
        egui::Window::new(format!("{} {}", ph::INFO, crate::i18n::tr("win-about")))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("QymCAD").size(22.0).strong());
                ui.label(egui::RichText::new(&crate::i18n::tr("about-tagline")).italics());
                // WHICH BUILD IS THIS - the first question asked of any complaint. With a build a day the
                // version number alone names a whole week of binaries, so the commit stands beside it, and
                // the button hands the whole line over ready to paste: nobody retypes a hash by eye.
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(&crate::i18n::tr("about-build"));
                    ui.label(egui::RichText::new(crate::build_info::line()).monospace());
                    if ui
                        .small_button(ph::COPY)
                        .on_hover_text(&crate::i18n::tr("about-copy-hint"))
                        .clicked()
                    {
                        ui.output_mut(|o| o.copied_text = crate::diagnostics::block());
                    }
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(&crate::i18n::tr("about-what"));
                ui.add_space(10.0);
                ui.colored_label(
                    self.scheme.pal.warning(),
                    format!("{} {}", ph::WARNING, crate::i18n::tr("about-develop-warning")),
                );
                ui.add_space(6.0);
                // THE CAM TAB IS IN THE SETTINGS AND DOES NOT WORK. Whoever finds the checkbox is owed the
                // reason here rather than after an hour of trying: it is groundwork, not a finished tool.
                ui.label(egui::RichText::new(&crate::i18n::tr("about-cam")).weak().small());
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(&crate::i18n::tr("about-site"));
                    ui.hyperlink_to("cad.qymis.tech", "https://cad.qymis.tech");
                });
                ui.horizontal(|ui| {
                    ui.label(&crate::i18n::tr("about-author"));
                    ui.label(egui::RichText::new(&crate::i18n::tr("about-author-name")).strong());
                });
                // THE PERSON MUST SEE ON WHAT TERMS THEY GOT THE PROGRAM. A copyleft licence is worth nothing
                // to whoever does not know they hold it: the rights to study, change and pass the program on
                // come with it, and the only place a desktop program can say so is here.
                ui.horizontal(|ui| {
                    ui.label(&crate::i18n::tr("about-license"));
                    ui.hyperlink_to("AGPL-3.0-or-later", "https://www.gnu.org/licenses/agpl-3.0.html");
                });
                ui.label(egui::RichText::new(&crate::i18n::tr("about-no-warranty")).weak().small());
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if ui.button(&crate::i18n::tr("close")).clicked() {
                        self.win.about = false;
                    }
                });
            });
        if !open {
            self.win.about = false;
        }
    }

    /// The modal "save the changes?" dialogue - drawn for as long as a navigation is pending.
    pub(super) fn nav_dialog(&mut self, ctx: &egui::Context) {
        // THE FLOOR IS CHECKED BEFORE THE NAVIGATION, not after it. The write finishing is what sets
        // `pending_nav`, and performing it at once takes the whole frame away - card and all - so a card put
        // up a moment earlier would blink out exactly as it did before. The navigation waits the few
        // milliseconds it takes for what is on screen to be readable.
        if let Some(shown) = self.waiting.save_shown {
            if std::time::Instant::now().duration_since(shown) < super::SAVE_WAIT_MIN {
                self.draw_splash(ctx, &crate::i18n::tr("io-saving"));
                ctx.request_repaint();
                return;
            }
            self.waiting.save_since = None;
            self.waiting.save_shown = None;
        }
        // A NAVIGATION THAT WAITED FOR THE WRITE IS PERFORMED HERE: this is where `ctx` exists, which the
        // background task's handler does not have.
        if let Some(nav) = self.pending_nav.take() {
            self.do_nav(nav, ctx);
            return;
        }
        // WHILE THE WRITE IS RUNNING THERE IS A WAITING CARD, NOT SILENCE. The "save?" question has been answered
        // and the navigation is being waited for; the window is alive and says what is going on.
        if self.deferred.nav_after_save {
            let now = std::time::Instant::now();
            if self.saving_now() {
                // THE CARD DOES NOT BLINK. A small document is written faster than an eye can catch, and a
                // card flashing for one frame reads as a glitch rather than an answer: nothing is shown
                // before `SAVE_WAIT_GRACE`.
                let since = *self.waiting.save_since.get_or_insert(now);
                if now.duration_since(since) >= super::SAVE_WAIT_GRACE {
                    self.waiting.save_shown.get_or_insert(now);
                    self.draw_splash(ctx, &crate::i18n::tr("io-saving"));
                }
                ctx.request_repaint();
                return;
            }
            // The write is over and no answer arrived (it was never started), so nobody is held up. The
            // floor above has already had its say by this point.
            self.waiting.save_since = None;
            self.waiting.save_shown = None;
            self.deferred.nav_after_save = false;
        }
        if self.deferred.nav.is_none() {
            return;
        }
        let mut choice: Option<u8> = None; // 0 save, 1 do not save, 2 cancel
        egui::Window::new(format!("{}  {}", ph::WARNING, crate::i18n::tr("win-unsaved")))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(&crate::i18n::tr("nav-unsaved-text"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(format!("{}  {}", ph::FLOPPY_DISK, crate::i18n::tr("io-save"))).clicked() {
                        choice = Some(0);
                    }
                    if ui.button(&crate::i18n::tr("nav-dont-save")).clicked() {
                        choice = Some(1);
                    }
                    if ui.button(&crate::i18n::tr("nav-cancel")).clicked() {
                        choice = Some(2);
                    }
                });
            });
        match choice {
            Some(0) => {
                self.save_project();
                // THE WRITE IS WAITED FOR WITHOUT FREEZING THE WINDOW.
                //
                // There used to be a blocking `wait_bg()` here: no frame was drawn at all while the file went to
                // disk. To a person that is indistinguishable from a hung program. Now the navigation simply waits
                // its turn, and a waiting card is drawn for the duration of the write.
                //
                // The request to save may never have reached a write (Save As was cancelled) - then there is no
                // background task and the navigation is cancelled at once, as before.
                if self.saving_now() {
                    self.deferred.nav_after_save = true;
                } else if !self.is_dirty() {
                    if let Some(nav) = self.deferred.nav.take() {
                        self.do_nav(nav, ctx);
                    }
                } else {
                    self.deferred.nav = None;
                }
            }
            Some(1) => {
                if let Some(nav) = self.deferred.nav.take() {
                    self.do_nav(nav, ctx);
                }
            }
            Some(_) => self.deferred.nav = None, // Cancel: stay here and drop the navigation
            None => {}
        }
    }

    /// The modal dialogue for choosing the STL quality (the deflection) before an export.
    pub(super) fn stl_quality_dialog(&mut self, ctx: &egui::Context) {
        let Some(target) = self.stl_export else { return };
        let mut choice: Option<Option<f64>> = None; // None = close; Some(Some(defl)) = export; Some(None) = cancel
        egui::Window::new(&crate::i18n::tr("stl-title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(&crate::i18n::tr("stl-detail"));
                ui.add_space(4.0);
                // (the label, the deflection in mm)
                let presets: [(&str, f64); 4] = [
                    (&crate::i18n::tr("stl-draft"), 0.2),
                    (&crate::i18n::tr("stl-standard"), 0.05),
                    (&crate::i18n::tr("stl-high"), 0.02),
                    (&crate::i18n::tr("stl-max"), 0.005),
                ];
                for (lbl, defl) in presets {
                    if ui.add_sized([260.0, 24.0], egui::Button::new(lbl)).clicked() {
                        choice = Some(Some(defl));
                    }
                }
                ui.add_space(6.0);
                if ui.button(&crate::i18n::tr("nav-cancel")).clicked() {
                    choice = Some(None);
                }
            });
        match choice {
            Some(Some(defl)) => {
                self.stl_export = None;
                self.export_stl(target, defl);
            }
            Some(None) => self.stl_export = None,
            None => {}
        }
    }

    /// The modal confirmation popup for deleting a tree node. Enter or Yes deletes, Esc or No cancels.
    pub(super) fn confirm_delete_popup(&mut self, ctx: &egui::Context) {
        let Some(sel) = self.deferred.delete else { return };
        let what = self.sel_delete_label(sel);
        let cascade = self.delete_cascade_names(sel);
        let mut do_del = ctx.input(|i| i.key_pressed(egui::Key::Enter)); // Yes is the default
        let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        egui::Window::new(format!("{} {}", ph::TRASH, crate::i18n::tr("win-delete-q")))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(crate::i18n::tr1("confirm-delete-what", "what", &what));
                // WHAT WILL GO WITH IT, BY NAME. A general line saying "along with its dependants" is true but does
                // not answer "what am I about to lose"; a list does, and it comes from the same core query as the
                // lineage in the properties card.
                if cascade.is_empty() {
                    ui.label(egui::RichText::new(&crate::i18n::tr("confirm-cascade")).weak().small());
                } else {
                    ui.label(egui::RichText::new(crate::i18n::tr1("confirm-cascade-n", "n", &cascade.len().to_string())).weak().small());
                    for n in cascade.iter().take(8) {
                        ui.label(egui::RichText::new(format!("  · {n}")).weak().small());
                    }
                    if cascade.len() > 8 {
                        ui.label(egui::RichText::new(crate::i18n::tr1("confirm-cascade-more", "n", &(cascade.len() - 8).to_string())).weak().small());
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(&crate::i18n::tr("confirm-yes")).min_size(egui::vec2(64.0, 0.0))).clicked() {
                        do_del = true;
                    }
                    if ui.add(egui::Button::new(&crate::i18n::tr("confirm-no")).min_size(egui::vec2(64.0, 0.0))).clicked() {
                        cancel = true;
                    }
                    ui.label(egui::RichText::new(&crate::i18n::tr("confirm-keys")).weak().small());
                });
            });
        if do_del {
            self.deferred.delete = None;
            self.execute_delete(sel);
        } else if cancel {
            self.deferred.delete = None;
        }
    }
}
