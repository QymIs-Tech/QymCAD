//! THE MODEL TREE: its rows, its gestures, and the two trees drawn from it.
//!
//! Split out of `panels.rs`, which had grown to 6 129 lines holding four unrelated subjects, where finding
//! the row of a feature meant scrolling past the settings window.

use super::*;

impl App {
    /// A DROP IN THE TREE: reorder, or gather into a subassembly.
    ///
    /// The model logic lives in the core (`reorder_component_before`, `group_components_into_assembly`);
    /// here there is only the gesture and whatever has to be fixed in the interface afterwards.
    ///
    /// THE MAIN THING AFTER ANY REORDER IS TO RECOMPUTE THE SELECTION. The tree remembers the selected
    /// component BY INDEX (`Sel::Component(ci)`), and both operations change the order in `components`:
    /// without recomputing it, the selection silently moves to a neighbour and a person edits the wrong
    /// part. A silent substitution is the worst kind of trouble - it cannot be seen.
    ///
    /// Returns true if the model changed.
    pub(super) fn tree_apply_drop(&mut self, dragged: Id, target: Id, how: super::TreeDrop) -> bool {
        if dragged == target {
            return false;
        }
        // WHAT EXACTLY IS BEING DRAGGED: if the row is part of the selection, the WHOLE selection moves; otherwise only that row.
        let mut moving: Vec<Id> = if self.tree_sel.multi.contains(&dragged) { self.tree_sel.multi.clone() } else { vec![dragged] };
        moving.retain(|&m| m != target);
        if moving.is_empty() {
            return false;
        }
        let keep: Option<Id> = match self.sel {
            Sel::Component(ci) => self.project.components.get(ci).map(|c| c.id),
            _ => None,
        };
        // ONE DROP MAKES ONE UNDO STEP. The edit used to go straight into `self.project`, past `App::edit`,
        // which states plainly that everything changing the document must go through it - and Ctrl+Z did not
        // see the reorder at all. If nothing changed there is no step either: `Edit` closes itself and leaves
        // no empty trace in the history.
        let ed = self.edit(crate::i18n::tr("tree-drop-step"));
        let this = &mut *ed.app;
        let changed = match how {
            // DROPPED ONTO A SUBASSEMBLY: the things go INSIDE it. There is nothing to create a new one from -
            // an existing assembly was chosen, and the expectation is that the things end up in it. Breeding
            // another one beside it is doing something other than what was asked.
            //
            // DROPPED ONTO A PART: then, and only then, a new subassembly. Nothing goes inside a part, and two
            // things can only be joined by a common parent.
            super::TreeDrop::Onto if this.project.component_kind(target) == Some(qymcad_core::feature::ComponentKind::Assembly) => {
                let mut any = false;
                for m in &moving {
                    any |= this.project.reparent_component(*m, target);
                }
                any
            }
            super::TreeDrop::Onto => this
                .project
                .group_components_into_assembly(&moving, target, crate::i18n::tr("tree-group-name"))
                .is_some(),
            super::TreeDrop::Before | super::TreeDrop::After => {
                // "after the target" means "before the next sibling"; the last one has no next, and then it is
                // the end of the list.
                let before = match how {
                    super::TreeDrop::Before => Some(target),
                    _ => {
                        let par = this.project.components.iter().find(|c| c.id == target).and_then(|c| c.parent);
                        let sibs: Vec<Id> = this.project.components.iter().filter(|c| c.parent == par).map(|c| c.id).collect();
                        sibs.iter().position(|&x| x == target).and_then(|i| sibs.get(i + 1).copied())
                    }
                };
                let mut any = false;
                for m in &moving {
                    any |= this.project.reorder_component_before(*m, before);
                }
                any
            }
        };
        drop(ed);
        if changed {
            // THE SELECTION GOES BY Id, NOT BY INDEX.
            if let Some(id) = keep {
                self.sel = match self.project.component_index(id) {
                    Some(ci) => Sel::Component(ci),
                    None => Sel::None,
                };
            }
            self.resync_after_topology_change();
        }
        changed
    }

    pub(super) fn tree_select_component(&mut self, ci: usize, cid: Id, ctrl: bool, shift: bool) {
        if shift {
            let par = self.project.components.get(ci).and_then(|c| c.parent);
            let sibs: Vec<Id> = self.project.components.iter().filter(|c| c.parent == par).map(|c| c.id).collect();
            match (self.tree_sel.anchor.and_then(|a| sibs.iter().position(|&x| x == a)), sibs.iter().position(|&x| x == cid)) {
                (Some(ia), Some(ib)) => {
                    let (lo, hi) = if ia <= ib { (ia, ib) } else { (ib, ia) };
                    self.tree_sel.multi = sibs[lo..=hi].to_vec();
                }
                _ => {
                    self.tree_sel.multi = vec![cid];
                    self.tree_sel.anchor = Some(cid);
                }
            }
        } else if ctrl {
            if let Some(p) = self.tree_sel.multi.iter().position(|&x| x == cid) {
                self.tree_sel.multi.remove(p);
            } else {
                self.tree_sel.multi.push(cid);
            }
            self.tree_sel.anchor = Some(cid);
        } else {
            self.tree_sel.multi = vec![cid];
            self.tree_sel.anchor = Some(cid);
        }
        self.sel = Sel::Component(ci);
    }


    /// Copy or cut the selected node into the TREE clipboard: a sketch (outside editing), a part or a subassembly.
    pub(super) fn tree_clipboard_copy(&mut self, cut: bool) {
        // With several components selected, the whole set goes into the bulk clipboard (the root excepted).
        if self.is_multi() {
            let root = self.project.root;
            let ids: Vec<Id> = self.tree_sel.multi.iter().copied().filter(|&id| id != root).collect();
            if ids.is_empty() {
                self.status = crate::i18n::tr("tree-root-not-copyable");
                return;
            }
            let n = ids.len();
            self.clip.tree = None;
            self.clip.tree_multi = Some((ids, cut));
            self.clip.os_ping = true; // a marker into the OS clipboard, so that Ctrl+V (Event::Paste) starts working
            self.status = crate::i18n::tr2("clip-components", "n", &n.to_string(), "how", &if cut { crate::i18n::tr("action-cut") } else { crate::i18n::tr("action-copy") });
            return;
        }
        self.clip.tree_multi = None;
        match self.sel {
            Sel::Sketch(si) => {
                if let Some(s) = self.project.sketches.get(si) {
                    let sid = s.id;
                    self.clip.tree = Some(TreeClip::Sketch { sid, cut });
                    self.clip.os_ping = true; // a marker into the OS clipboard, so that Ctrl+V (Event::Paste) starts working
                    self.status = crate::i18n::tr1("clip-sketch", "how", &if cut { crate::i18n::tr("action-cut") } else { crate::i18n::tr("action-copy") });
                }
            }
            Sel::Component(ci) => {
                if let Some(c) = self.project.components.get(ci) {
                    let id = c.id;
                    if id == self.project.root {
                        self.status = crate::i18n::tr("tree-root-not-copyable");
                        return;
                    }
                    self.clip.tree = Some(TreeClip::Component { id, cut });
                    self.clip.os_ping = true; // a marker into the OS clipboard, so that Ctrl+V (Event::Paste) starts working
                    let what = if self.project.component_kind(id) == Some(qymcad_core::feature::ComponentKind::Assembly) { crate::i18n::tr("node-subassembly") } else { crate::i18n::tr("node-part") };
                    self.status = crate::i18n::tr2("clip-node", "what", &what, "how", &if cut { crate::i18n::tr("action-cut") } else { crate::i18n::tr("action-copy") });
                }
            }
            _ => self.status = crate::i18n::tr("tree-copy-pick-first"),
        }
    }


    /// Paste from the tree clipboard into a target component (the selected one, otherwise the active context).
    /// A copy is a deep clone (new Ids); a cut re-parents the node (keeping its Id and its associativity).
    pub(super) fn tree_clipboard_paste(&mut self) {
        // The bulk component clipboard takes priority.
        if let Some((ids, cut)) = self.clip.tree_multi.clone() {
            self.paste_components_multi(&ids, cut);
            return;
        }
        let Some(clip) = self.clip.tree else {
            self.status = crate::i18n::tr("tree-clipboard-empty");
            return;
        };
        use qymcad_core::feature::ComponentKind;
        let root = self.project.root;
        // The target depends on what is in the clipboard.
        let target = match clip {
            // A sketch can be pasted into ANY component (a part, a subassembly or the root): the selected
            // component, otherwise the active context (having entered a Part, that Part).
            TreeClip::Sketch { .. } => match self.sel {
                Sel::Component(ci) => self.project.components.get(ci).map(|c| c.id).unwrap_or_else(|| self.project.active_ctx()),
                _ => self.project.active_ctx(),
            },
            // A component can be pasted ONLY into an assembly (a part inside a part is forbidden): the selected
            // subassembly; if a Part is selected, its parent assembly; otherwise the active context.
            TreeClip::Component { .. } => match self.sel {
                Sel::Component(ci) => match self.project.components.get(ci) {
                    Some(c) if self.project.component_kind(c.id) == Some(ComponentKind::Assembly) => c.id,
                    Some(c) => c.parent.unwrap_or(root),
                    None => self.project.active_ctx(),
                },
                _ => self.project.active_ctx(),
            },
        };
        match clip {
            TreeClip::Sketch { sid, cut } => {
                if cut {
                    if self.project.move_sketch_node(sid, target) {
                        self.clip.tree = None;
                        self.status = crate::i18n::tr("status-sketch-moved");
                    } else {
                        self.status = crate::i18n::tr("status-sketch-move-failed");
                    }
                } else if self.project.clone_sketch_node(sid, target).is_some() {
                    self.status = crate::i18n::tr("status-sketch-copied");
                } else {
                    self.status = crate::i18n::tr("status-sketch-copy-failed");
                }
                self.invalidate();
            }
            TreeClip::Component { id, cut } => {
                let into_part = self.project.component_is_part(target);
                if cut {
                    if self.project.reparent_component(id, target) {
                        self.clip.tree = None;
                        self.status = crate::i18n::tr("status-component-moved");
                    } else if into_part {
                        self.status = crate::i18n::tr("status-paste-needs-assembly");
                    } else {
                        self.status = crate::i18n::tr("status-move-impossible");
                    }
                    self.invalidate();
                } else if let Some(cl) = self.project.clone_component(id, target) {
                    self.mark_dirty_for_rebuild(); // the document is marked; the scheduler builds the clone's bodies through the kernel
                    if let Some(ci) = self.project.components.iter().position(|c| c.id == cl) {
                        self.sel = Sel::Component(ci);
                    }
                    self.status = crate::i18n::tr("status-component-copied");
                } else if into_part {
                    self.status = crate::i18n::tr("status-paste-needs-assembly");
                    self.invalidate();
                } else {
                    self.status = crate::i18n::tr("status-component-copy-failed");
                    self.invalidate();
                }
            }
        }
    }


    /// THE DATUM'S NAME AS THE ROW SHOWS IT - for the search. One place shared with the row itself: were it
    /// assembled separately, the search would stop finding what is displayed (this class of fault has been
    /// caught four times).
    pub(super) fn datum_row_name(&self, kind: &qymcad_core::feature::FeatureKind) -> String {
        use qymcad_core::feature::FeatureKind as FK;
        let by = |id: Id, list: &dyn Fn(&Self, Id) -> Option<String>| list(self, id).unwrap_or_default();
        match *kind {
            FK::Plane { plane } => by(plane, &|s: &Self, id| s.project.planes.iter().find(|p| p.id == id).map(|p| crate::i18n::name(&p.name))),
            FK::DatumPoint { point } => by(point, &|s: &Self, id| s.project.datum_points.iter().find(|p| p.id == id).map(|p| crate::i18n::name(&p.name))),
            FK::DatumAxis { axis } => by(axis, &|s: &Self, id| s.project.datum_axes.iter().find(|a| a.id == id).map(|a| crate::i18n::name(&a.name))),
            _ => String::new(),
        }
    }

    pub(super) fn tree_datum_row(&mut self, ui: &mut egui::Ui, _id: Id, kind: &qymcad_core::feature::FeatureKind) {
        use qymcad_core::feature::FeatureKind as FK;
        match *kind {
            FK::Plane { plane } => {
                if let Some(pi) = self.project.planes.iter().position(|p| p.id == plane) {
                    let nm = crate::i18n::name(&self.project.planes[pi].name);
                    ui.horizontal(|ui| {
                        self.datum_vis_checkbox(ui, plane);
                        if self.rename_node_active(ui, RenameNode::Plane(plane)) {
                            return; // inline renaming
                        }
                        let r = ui.selectable_label(self.sel == Sel::Plane(pi), format!("{} {nm}", ph::SELECTION_ALL)).on_hover_text(&crate::i18n::tr("tree-datum-hint"));
                        if r.clicked() {
                            self.sel = Sel::Plane(pi);
                        }
                        if r.double_clicked() {
                            self.start_feat_cmd_edit(plane); // reopen the plane command
                        }
                        let mut ren = false;
                        r.context_menu(|ui| {
                            if ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                                ren = true;
                                ui.close();
                            }
                        });
                        if ren {
                            self.start_rename_node(RenameNode::Plane(plane), nm.clone());
                        }
                    });
                }
            }
            FK::DatumPoint { point } => {
                if let Some(pi) = self.project.datum_points.iter().position(|d| d.id == point) {
                    let nm = crate::i18n::name(&self.project.datum_points[pi].name);
                    ui.horizontal(|ui| {
                        self.datum_vis_checkbox(ui, point);
                        if self.rename_node_active(ui, RenameNode::DatumPoint(point)) {
                            return; // inline renaming
                        }
                        let r = ui.selectable_label(self.sel == Sel::DatumPoint(pi), format!("{} {nm}", ph::DOT)).on_hover_text(&crate::i18n::tr("tree-datum-hint"));
                        if r.clicked() {
                            self.sel = Sel::DatumPoint(pi); // a datum point is selectable (in sync with 3D)
                        }
                        if r.double_clicked() {
                            self.start_feat_cmd_edit(point); // reopen the point command
                        }
                        let mut ren = false;
                        r.context_menu(|ui| {
                            if ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                                ren = true;
                                ui.close();
                            }
                        });
                        if ren {
                            self.start_rename_node(RenameNode::DatumPoint(point), nm.clone());
                        }
                    });
                }
            }
            FK::DatumAxis { axis } => {
                if let Some(ai) = self.project.datum_axes.iter().position(|d| d.id == axis) {
                    let nm = crate::i18n::name(&self.project.datum_axes[ai].name);
                    ui.horizontal(|ui| {
                        self.datum_vis_checkbox(ui, axis);
                        if self.rename_node_active(ui, RenameNode::DatumAxis(axis)) {
                            return; // inline renaming
                        }
                        let r = ui.selectable_label(self.sel == Sel::DatumAxis(ai), format!("{} {nm}", ph::LINE_SEGMENT)).on_hover_text(&crate::i18n::tr("tree-datum-hint"));
                        if r.clicked() {
                            self.sel = Sel::DatumAxis(ai); // a datum axis is selectable
                        }
                        if r.double_clicked() {
                            self.start_feat_cmd_edit(axis); // reopen the axis command
                        }
                        let mut ren = false;
                        r.context_menu(|ui| {
                            if ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                                ren = true;
                                ui.close();
                            }
                        });
                        if ren {
                            self.start_rename_node(RenameNode::DatumAxis(axis), nm.clone());
                        }
                    });
                }
            }
            _ => {}
        }
    }


    /// If tree node `node` is being renamed right now, draw an input field instead of the label and commit the
    /// name into ITS OWN storage on Enter or on a click elsewhere (Escape cancels). Returns true if the field is
    /// shown (in which case the label and the row menu are not drawn).
    pub(super) fn rename_node_active(&mut self, ui: &mut egui::Ui, node: RenameNode) -> bool {
        if self.rename.node != Some(node) {
            return false;
        }
        let resp = ui.add(egui::TextEdit::singleline(&mut self.rename.buf).desired_width(160.0));
        if std::mem::take(&mut self.rename.focus) {
            resp.request_focus();
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.rename.node = None; // cancelled
        } else if resp.lost_focus() {
            let nm = self.rename.buf.trim().to_string(); // Enter or a click elsewhere commits
            if !nm.is_empty() {
                self.begin_edit(&crate::i18n::tr("status-rename")); // THE EDIT BOUNDARY: the document is changed from a panel
                match node {
                    RenameNode::Component(id) => {
                        if let Some(c) = self.project.components.iter_mut().find(|c| c.id == id) {
                            c.name = nm;
                        }
                    }
                    RenameNode::Plane(id) => {
                        if let Some(p) = self.project.planes.iter_mut().find(|p| p.id == id) {
                            p.name = nm;
                        }
                    }
                    RenameNode::DatumPoint(id) => {
                        if let Some(d) = self.project.datum_points.iter_mut().find(|d| d.id == id) {
                            d.name = nm;
                        }
                    }
                    RenameNode::DatumAxis(id) => {
                        if let Some(d) = self.project.datum_axes.iter_mut().find(|d| d.id == id) {
                            d.name = nm;
                        }
                    }
                    RenameNode::Body(mi) => self.project.set_mesh_name(mi, nm),
                }
                self.commit_edit();
            }
            self.rename.node = None;
        }
        true
    }


    /// If node `id` is being renamed right now, draw an input field instead of the heading and commit the name
    /// on Enter or on a click elsewhere (Escape cancels). Returns true if the field is shown (in which case the
    /// heading is not drawn).
    pub(super) fn rename_row_active(&mut self, ui: &mut egui::Ui, id: Id) -> bool {
        if self.rename.target != Some(id) {
            return false;
        }
        let resp = ui.add(egui::TextEdit::singleline(&mut self.rename.buf).desired_width(170.0));
        if std::mem::take(&mut self.rename.focus) {
            resp.request_focus();
        }
        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if esc {
            self.rename.target = None; // cancelled without saving
        } else if resp.lost_focus() {
            let nm = self.rename.buf.trim().to_string(); // committed by Enter or by a click elsewhere
            if !nm.is_empty() {
                self.begin_edit(&crate::i18n::tr("status-rename-op")); // THE EDIT BOUNDARY
                if let Some(n) = self.project.timeline.iter_mut().find(|n| n.id == id) {
                    n.name = nm;
                }
                self.commit_edit();
            }
            self.rename.target = None;
        }
        true
    }


    /// A sketch row: the visibility checkbox + expansion into contours; editing and finishing; a click selects.
    pub(super) fn tree_sketch_row(&mut self, ui: &mut egui::Ui, sid: Id) {
        let Some(si) = self.project.sketch_index(sid) else { return };
        let editing = self.sketch_ses.editing == Some(sid);
        let name = crate::i18n::name(&self.project.sketches[si].name);
        let icon = if editing { ph::PENCIL_SIMPLE } else { ph::POLYGON };
        let raw = format!("{icon} {name}"); // the icon (a pencil while editing) already tells the mode apart
        // A PLAIN selectable row (with no expansion into contours). Editing starts on a double click; the right
        // button offers rename (inline) and delete. Only the visibility checkbox and the name.
        let mut act: Option<u8> = None; // 1 rename-start, 2 delete
        ui.horizontal(|ui| {
            let mut vis = !self.sketch_hidden.contains(&sid);
            if ui.add(egui::Checkbox::without_text(&mut vis)).on_hover_text(&crate::i18n::tr("tree-sketch-visible-hint")).changed() {
                if vis {
                    self.sketch_hidden.remove(&sid);
                } else {
                    self.sketch_hidden.insert(sid);
                }
            }
            // INLINE renaming of a sketch (as for features): a field instead of the label
            if self.rename.sketch == Some(sid) {
                let r = ui.add(egui::TextEdit::singleline(&mut self.rename.buf).desired_width(160.0));
                if std::mem::take(&mut self.rename.focus) {
                    r.request_focus();
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.rename.sketch = None; // cancelled
                } else if r.lost_focus() {
                    let nm = self.rename.buf.trim().to_string(); // Enter or a click elsewhere commits
                    if !nm.is_empty() {
                        self.project.sketches[si].name = nm;
                    }
                    self.rename.sketch = None;
                }
                return;
            }
            let resp = ui.selectable_label(self.sel == Sel::Sketch(si), self.sel_title(raw, self.sel == Sel::Sketch(si))).on_hover_text(&crate::i18n::tr("tree-sketch-hint"));
            if resp.double_clicked() {
                self.enter_sketch_edit(si);
            } else if resp.clicked() {
                self.sel = Sel::Sketch(si);
            }
            resp.context_menu(|ui| {
                if ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                    act = Some(1);
                    ui.close();
                }
                if ui.button(format!("{} {}", ph::ARROWS_OUT_CARDINAL, crate::i18n::tr("act-move-sketch"))).on_hover_text(&crate::i18n::tr("tree-sketch-move-hint")).clicked() {
                    act = Some(7);
                    ui.close();
                }
                if ui.button(format!("{} {}", ph::COPY, crate::i18n::tr("act-copy-ctrl-c"))).clicked() {
                    act = Some(3);
                    ui.close();
                }
                if ui.button(format!("{} {}", ph::SCISSORS, crate::i18n::tr("act-cut-ctrl-x"))).clicked() {
                    act = Some(4);
                    ui.close();
                }
                if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("act-delete-sketch"))).clicked() {
                    act = Some(2);
                    ui.close();
                }
                ui.separator();
                if ui.button(format!("{} {}", ph::EXPORT, crate::i18n::tr("act-export-svg"))).on_hover_text(&crate::i18n::tr("tree-export-svg-hint")).clicked() {
                    act = Some(5);
                    ui.close();
                }
                if ui.button(format!("{} {}", ph::EXPORT, crate::i18n::tr("act-export-dxf"))).on_hover_text(&crate::i18n::tr("tree-export-dxf-hint")).clicked() {
                    act = Some(6);
                    ui.close();
                }
            });
        });
        match act {
            Some(1) => {
                self.rename.sketch = Some(sid);
                self.rename.target = None; // do not clash with renaming a feature
                self.rename.buf = name;
                self.rename.focus = true;
            }
            Some(2) => self.ask_delete(Sel::Sketch(si)),
            Some(3) => {
                self.sel = Sel::Sketch(si);
                self.clipboard_copy(false);
            }
            Some(4) => {
                self.sel = Sel::Sketch(si);
                self.clipboard_copy(true);
            }
            Some(5) => self.export_sketch(si, false),
            Some(6) => self.export_sketch(si, true),
            Some(7) => self.start_replace_sketch_plane(si),
            _ => {}
        }
    }


    /// WHAT A FEATURE'S SET IS DEFINED BY - IN WORDS, not as a number.
    ///
    /// Reported behaviour: the right panel did not show this at all, leaving no way to tell what had been
    /// clicked together. A count like "edges: 4" describes a manual pick and lies about a description: tomorrow
    /// there will be five. So a description shows THE DESCRIPTION ITSELF - that is what the document records.
    pub(super) fn ref_summary(&self, r: &qymcad_core::refs::Ref) -> String {
        use qymcad_core::refs::Query;
        let n = r.query.picked_descs().len();
        match &r.query {
            Query::Id(0) => crate::i18n::tr("all-of-them"),
            q if q.is_pick_list() => {
                if n == 0 {
                    crate::i18n::tr("all-of-them")
                } else {
                    crate::i18n::tr1("count-edges", "n", &n.to_string())
                }
            }
            Query::Adjacent(_) => crate::i18n::tr("expand-face-edges"),
            Query::Between(_, _) => crate::i18n::tr("expand-between-done"),
            Query::TangentChain { .. } => crate::i18n::tr("expand-tangent-chain"),
            Query::Oriented { .. } => crate::i18n::tr("expand-parallel"),
            Query::Extreme { .. } => crate::i18n::tr("expand-topmost"),
            Query::Largest => crate::i18n::tr("expand-largest"),
            Query::OfFeature { .. } => crate::i18n::tr("expand-feature-faces"),
            _ => crate::i18n::tr("expand-described-short"),
        }
    }

    /// A FEATURE'S LABEL IN THE TREE - one label for everyone who shows it or searches by it.
    ///
    /// It was lifted out of the tree row for the sake of the search: a person searches by what they SEE, and if
    /// the search assembled the label its own way it would stop finding what is displayed. The same class of
    /// divergence already caught on the localisation keys and on the settings table: two places knowing one thing.
    pub(super) fn feature_row_label(&self, ti: usize) -> String {
        use qymcad_core::feature::FeatureKind;
        let Some(node) = self.project.timeline.get(ti) else { return String::new() };
        let kind = node.kind.clone();
        let mut lbl = match kind {
            FeatureKind::Extrude { height, .. } => format!("{} {}", ph::CUBE, crate::i18n::tr1("feat-extrude", "h", &crate::i18n::num(height, 1))),
            FeatureKind::Revolve { angle, .. } => format!("{} {}", ph::CUBE, crate::i18n::tr1("feat-revolve", "angle", &crate::i18n::num(angle, 0))),
            FeatureKind::Sweep { .. } => format!("{} {}", ph::CUBE, crate::i18n::tr("feat-sweep")),
            FeatureKind::Loft { ref sketches, src, op, surface, .. } => {
                // A SURFACE IS VISIBLE IN THE TREE: a node with caps and a node without them are different
                // things, and one shared row on both would force the feature open just to learn what was built.
                let suf = if surface {
                    crate::i18n::tr("feat-suffix-surface")
                } else if src == 0 {
                    String::new()
                } else {
                    [crate::i18n::tr("feat-suffix-cut"), crate::i18n::tr("feat-suffix-union"), crate::i18n::tr("feat-suffix-intersect")].get(op as usize).cloned().unwrap_or_default()
                };
                format!("{} {}{}", ph::STACK, crate::i18n::tr1("feat-loft", "n", &sketches.len().to_string()), suf)
            }
            FeatureKind::PushFace { dist, .. } => format!("{} {}", ph::ARROWS_OUT_LINE_VERTICAL, crate::i18n::tr1("feat-push-face", "d", &crate::i18n::num_signed(dist, 1))),
            FeatureKind::Trim { .. } => format!("{} {}", ph::SCISSORS, crate::i18n::tr("feat-trim")),
            FeatureKind::Stitch { ref parts, .. } => format!("{} {}", ph::INTERSECT_SQUARE, crate::i18n::tr1("feat-stitch", "n", &parts.len().to_string())),
            FeatureKind::Patch { ref edges, .. } => format!("{} {}", ph::BANDAIDS, crate::i18n::tr1("feat-patch", "n", &edges.query.picked_descs().len().to_string())),
            FeatureKind::SurfaceReplace { ref faces, .. } => format!("{} {}", ph::SWAP, crate::i18n::tr1("feat-surface-replace", "n", &faces.query.picked_descs().len().to_string())),
            FeatureKind::FaceCopy { ref faces, .. } => format!("{} {}", ph::COPY_SIMPLE, crate::i18n::tr1("feat-face-copy", "n", &faces.query.picked_descs().len().to_string())),
            FeatureKind::RemoveFace { ref faces, .. } => format!("{} {}", ph::ERASER, crate::i18n::tr1("feat-remove-face", "n", &faces.query.picked_descs().len().to_string())),
            FeatureKind::SplitFace { offset, .. } => {
                let off = if offset.abs() < 1e-9 { String::new() } else { format!(" {offset:+.1}") };
                format!("{} {}{off}", ph::GRID_FOUR, crate::i18n::tr("feat-split-face"))
            }
            FeatureKind::Thicken { thickness, .. } => format!("{} {}", ph::STACK_SIMPLE, crate::i18n::tr1("feat-thicken", "d", &crate::i18n::num_signed(thickness, 1))),
            FeatureKind::PartInstance { src_comp, .. } => {
                let name = self.project.components.iter().find(|c| c.id == src_comp).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
                // NOT AN ASSEMBLY ICON. This used to be `ph::STACK` - the very icon that marks ASSEMBLIES in the
                // tree and labels the "new subassembly" button. Reported behaviour: an Instance inside carrying an
                // assembly icon that cannot be entered. Fairly so: the icon promised a node one enters, while this
                // is a copied body with nothing to enter.
                format!("{} {}", ph::CUBE_TRANSPARENT, crate::i18n::tr1("feat-part-instance", "name", &name))
            }
            FeatureKind::SplitBody { ref bodies, offset, .. } => {
                let off = if offset.abs() < 1e-9 { String::new() } else { format!(" {offset:+.1}") };
                format!("{} {}{}", ph::SQUARE_SPLIT_HORIZONTAL, crate::i18n::tr1("feat-split-body", "n", &bodies.len().to_string()), off)
            }
            FeatureKind::Draft { ref faces, angle, .. } => format!("{} {}", ph::ANGLE, crate::i18n::trn("feat-draft", &[("angle", &crate::i18n::num(angle, 0)), ("n", &faces.query.picked_descs().len().to_string())])),
            FeatureKind::Box3 { dx, dy, dz, .. } => format!("{} {}", ph::CUBE, crate::i18n::trn("feat-box", &[("x", &crate::i18n::num(dx, 0)), ("y", &crate::i18n::num(dy, 0)), ("z", &crate::i18n::num(dz, 0))])),
            FeatureKind::Cylinder { r, h, .. } => format!("{} {}", ph::CYLINDER, crate::i18n::trn("feat-cylinder", &[("d", &crate::i18n::num(2.0 * r, 0)), ("h", &crate::i18n::num(h, 0))])),
            FeatureKind::Sphere { r, .. } => format!("{} {}", ph::CIRCLE, crate::i18n::tr1("feat-sphere", "d", &crate::i18n::num(2.0 * r, 0))),
            FeatureKind::Combine { op, height, .. } => {
                // the icon follows THE OPERATION: scissors only for a cut; a boss ADDS material (a cube);
                // an intersection has its own (everything used to be scissors, and a boss looked like a cut)
                let ic = [ph::SCISSORS, ph::CUBE, ph::INTERSECT][(op as usize).min(2)];
                format!("{} {} h={}", ic, [crate::i18n::tr("bool-cut"), crate::i18n::tr("bool-boss"), crate::i18n::tr("bool-intersect-short")][(op as usize).min(2)], crate::i18n::num(height, 1))
            }
            FeatureKind::Fillet { radius, ref edges, .. } => format!("{} {}", ph::CIRCLE, crate::i18n::trn("feat-fillet", &[("r", &crate::i18n::num(radius, 1)), ("which", &self.ref_summary(edges))])),
            FeatureKind::Chamfer { dist, ref edges, mode, d2, .. } => {
                use qymcad_core::feature::ChamferMode;
                let size = match mode {
                    ChamferMode::TwoDist => format!("{dist:.1}×{d2:.1}"),
                    ChamferMode::DistAngle => crate::i18n::trn("feat-chamfer-dist-angle", &[("d", &crate::i18n::num(dist, 1)), ("angle", &crate::i18n::num(d2, 0))]),
                    ChamferMode::Symmetric => format!("{dist:.1}"),
                };
                format!("{} {}", ph::TRIANGLE, crate::i18n::trn("feat-chamfer", &[("size", &size), ("which", &self.ref_summary(edges))]))
            }
            FeatureKind::Cone { r1, r2, h, .. } => format!("{} {}", ph::CUBE, crate::i18n::trn("feat-cone", &[("d1", &crate::i18n::num(2.0 * r1, 0)), ("d2", &crate::i18n::num(2.0 * r2, 0)), ("h", &crate::i18n::num(h, 0))])),
            FeatureKind::Torus { major, minor, .. } => format!("{} {}", ph::CIRCLE, crate::i18n::trn("feat-torus", &[("r", &crate::i18n::num(major, 0)), ("r2", &crate::i18n::num(minor, 0))])),
            FeatureKind::Prism { r, n, h, .. } => format!("{} {}", ph::HEXAGON, crate::i18n::trn("feat-prism", &[("n", &n.to_string()), ("d", &crate::i18n::num(2.0 * r, 0)), ("h", &crate::i18n::num(h, 0))])),
            FeatureKind::Shell { thickness, ref faces, .. } => format!("{} {}", ph::BOUNDING_BOX, crate::i18n::trn("feat-shell", &[("t", &crate::i18n::num(thickness, 1)), ("n", &faces.query.picked_descs().len().to_string())])),
            FeatureKind::LinearArray { count, count2, .. } => format!("{} {}", ph::DOTS_THREE_OUTLINE, crate::i18n::tr1("feat-linear-array", "n", &if count2 > 1 { format!("×{count}×{count2}") } else { format!("×{count}") })),
            FeatureKind::CircularArray { count, angle, .. } => format!("{} {}", ph::ARROWS_CLOCKWISE, crate::i18n::trn("feat-circular-array", &[("n", &count.to_string()), ("angle", &crate::i18n::num(angle, 0))])),
            FeatureKind::Mirror { plane, datum, .. } => format!("{} {}", ph::FLIP_HORIZONTAL, crate::i18n::tr1("feat-mirror", "plane", &if datum != 0 { crate::i18n::tr("ref-datum") } else { ["XY", "XZ", "YZ"][(plane as usize).min(2)].to_string() })),
            FeatureKind::Hole { diameter, depth, sketch, .. } => {
                if sketch != 0 {
                    let n = self.project.sketch_isolated_points(sketch).len();
                    format!("{} {}", ph::CIRCLE, crate::i18n::trn("feat-holes", &[("n", &n.to_string()), ("d", &crate::i18n::num(diameter, 1)), ("h", &crate::i18n::num(depth, 1))]))
                } else {
                    format!("{} {}", ph::CIRCLE, crate::i18n::trn("feat-hole", &[("d", &crate::i18n::num(diameter, 1)), ("h", &crate::i18n::num(depth, 1))]))
                }
            }
            FeatureKind::BodyBoolean { op, .. } => format!("{} {}", ph::INTERSECT, [crate::i18n::tr("feat-body-cut"), crate::i18n::tr("feat-body-union"), crate::i18n::tr("feat-body-intersect")][(op as usize).min(2)]),
            FeatureKind::Move { .. } => format!("{} {}", ph::ARROWS_OUT_CARDINAL, crate::i18n::tr("feat-move")),
            FeatureKind::MirrorPart { .. } => format!("{} {}", ph::FLIP_HORIZONTAL, crate::i18n::tr("feat-mirror-part")),
            FeatureKind::Thread { spec, length, .. } => {
                let g = spec.geometry();
                let name = match spec.standard {
                    qymcad_core::thread::ThreadStandard::MetricIso => format!("M{:.0}×{:.2}", g.major_d, g.pitch),
                    qymcad_core::thread::ThreadStandard::TrapezoidalTr => format!("Tr{:.0}×{:.1}", g.major_d, g.pitch),
                    qymcad_core::thread::ThreadStandard::RoundRd => format!("Rd{:.0}×{:.1}", g.major_d, g.pitch),
                    _ => format!("Ø{:.1}×{:.2}", g.major_d, g.pitch),
                };
                format!("{} {}{}", ph::SPIRAL, crate::i18n::trn("feat-thread", &[("name", &name), ("side", &if spec.internal { crate::i18n::tr("thread-internal") } else { crate::i18n::tr("thread-external") }), ("len", &crate::i18n::num(length, 0))]), if spec.starts > 1 { crate::i18n::tr1("count-starts", "n", &spec.starts.to_string()) } else { String::new() })
            }
            FeatureKind::Auger { spec, length, .. } => format!("{} {}", ph::SPIRAL, crate::i18n::trn("feat-auger", &[("d", &crate::i18n::num(spec.outer_d, 0)), ("pitch", &crate::i18n::num(spec.pitch, 0)), ("len", &crate::i18n::num(length, 0))])),
            // a kind of feature that has no row of its own here (sketches and datums have their own rows)
            _ => return String::new(),
        };
        // a RENAMED feature (its name differs from the default) shows its name; otherwise the automatic label with the sizes
        let custom_name = self.project.timeline[ti].name != Self::feat_default_name(&kind);
        if custom_name {
            lbl = format!("{} {}", Self::feat_icon(&kind), crate::i18n::name(&self.project.timeline[ti].name));
        }
        lbl
    }

    /// WHETHER A TIMELINE ROW MATCHES THE SEARCH. An empty query matches everything.
    ///
    /// Both THE LABEL AND THE NODE'S NAME are compared: a person searches for "extrude" as readily as for "lid" -
    /// the first is the automatic label from the feature's kind, the second is the name they gave it themselves.
    /// Case does not matter: nobody types a capital letter for the sake of a search.
    pub(super) fn tree_row_matches(&self, ti: usize) -> bool {
        self.tree_text_matches(&self.feature_row_label(ti)) || self.project.timeline.get(ti).is_some_and(|n| self.tree_text_matches(&crate::i18n::name(&n.name)))
    }

    /// WHETHER THE DISPLAYED TEXT MATCHES THE SEARCH. One matcher for ALL the sections of the tree.
    ///
    /// The first version filtered only the bodies-and-history section, and that was fairly called a stub: in an
    /// assembly the search found neither parts nor subassemblies - that is, exactly what one searches an assembly
    /// for. A search that looks in one section out of five is worse than none: it looks like it works.
    pub(super) fn tree_text_matches(&self, text: &str) -> bool {
        let q = self.tree.search.trim().to_lowercase();
        q.is_empty() || text.to_lowercase().contains(&q)
    }

    /// A feature row: it expands into the resulting body; a click selects the feature (for editing or deleting).
    pub(super) fn tree_feature_row(&mut self, ui: &mut egui::Ui, ti: usize) {
        // a guard against a STALE index: deleting a feature within this same pass of the loop (right button ->
        // Delete) shortens the timeline, so the following rows get an index past the end (immediate mode). On the
        // next frame feat_tis is recomputed; until then, simply skip.
        if ti >= self.project.timeline.len() {
            return;
        }
        let lbl = self.feature_row_label(ti);
        if lbl.is_empty() {
            return; // this kind of node has no row of its own in the body list
        }
        let nid = self.project.timeline[ti].id;
        let n = self.project.timeline.len();
        // suppressed either by the rollback bar OR individually - shown grey and italic
        let suppressed = self.project.rollback.is_some_and(|rb| ti >= rb) || self.project.timeline[ti].suppressed;
        let node_suppressed = self.project.timeline[ti].suppressed;
        let mut act: Option<u8> = None; // 1 edit,2 rollback,3 clear-rb,4 up,5 down,6 delete,7 suppress-toggle,8 rename
        // the neighbouring FEATURES of the same parent: Up/Down reorder among FEATURES (not among sketches or
        // datums), and go grey when the move would break the dependencies (a linear fillet-over-extrude chain).
        let parent = self.project.timeline[ti].parent;
        let prev_feat = (0..ti).rev().find(|&j| self.project.timeline[j].parent == parent && self.project.timeline[j].kind.body().is_some());
        let next_feat = (ti + 1..n).find(|&j| self.project.timeline[j].parent == parent && self.project.timeline[j].kind.body().is_some());
        let can_up = prev_feat.is_some_and(|pf| self.project.can_reorder_feature(ti, pf));
        let can_down = next_feat.is_some_and(|nf| self.project.can_reorder_feature(nf, ti));
        // the feature did NOT build (it fell back to a pass-through, a copy of the source), so it is marked in
        // the tree. There is now ONE row per operation span, so an error on ANY node of the span (a hidden one
        // included) must turn the row red - otherwise the feature silently fails to apply with no marker at all.
        // A KERNEL ERROR IS A CODE; the text in the reader's language is assembled here (see i18n::error_text)
        let node_err = self
            .project
            .feature_op_span(nid)
            .iter()
            .find_map(|id| self.project.regen_errors.get(id))
            .map(crate::i18n::error_text);
        ui.horizontal(|ui| {
            // A feature is suppressed either by THE ROLLBACK LINE (the tail of the list) or individually from the
            // right-button menu. The suppress checkbox was removed: it confused, because in a linear chain it hid
            // everything from the current one downwards. An individually suppressed feature carries a mark.
            // inline renaming: a field instead of the label
            if self.rename_row_active(ui, nid) {
                return;
            }
            let base = if node_suppressed { format!("{lbl}  {}", ph::PROHIBIT) } else { lbl };
            let base = if node_err.is_some() { format!("{} {base}", ph::WARNING) } else { base };
            // the node's reference was REBOUND by its fingerprint - that is visible rather than silent.
            let rebound = self.regen.rebinds.iter().find(|r| r.node == nid).map(|r| r.what.clone());
            let base = if rebound.is_some() { format!("{} {base}", ph::LINK_BREAK) } else { base };
            let mut txt = egui::RichText::new(base);
            if suppressed {
                txt = txt.weak().italics(); // rolled back or suppressed - it is visible that it does not build
            } else if node_err.is_some() {
                txt = txt.color(self.scheme.pal.error()); // it did not apply, so it goes red
            }
            let hover = match (&node_err, &rebound) {
                (Some(e), _) => format!("{} {}", ph::WARNING, crate::i18n::tr1("tree-feature-failed", "error", e)),
                (None, Some(w)) => format!("{} {}", ph::LINK_BREAK, crate::i18n::tr1("tree-feature-rebound", "what", w)),
                _ => crate::i18n::tr("tree-feature-hint"),
            };
            let resp = ui.selectable_label(self.sel == Sel::Feature(ti), txt).on_hover_text(hover);
            if resp.clicked() {
                self.sel = Sel::Feature(ti);
            }
            if resp.double_clicked() {
                act = Some(1);
            }
            resp.context_menu(|ui| {
                if ui.button(format!("{} {}", ph::PENCIL_SIMPLE, crate::i18n::tr("act-edit"))).clicked() {
                    act = Some(1);
                    ui.close();
                }
                if ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                    act = Some(8);
                    ui.close();
                }
                let supp_label = if node_suppressed { format!("{} {}", ph::CHECK, crate::i18n::tr("act-unsuppress")) } else { format!("{} {}", ph::PROHIBIT, crate::i18n::tr("act-suppress")) };
                if ui.button(supp_label).on_hover_text(&crate::i18n::tr("tree-suppress-hint")).clicked() {
                    act = Some(7);
                    ui.close();
                }
                ui.separator();
                if ui.button(format!("{} {}", ph::ARROW_LINE_UP, crate::i18n::tr("act-rollback-here"))).on_hover_text(&crate::i18n::tr("tree-rollback-hint")).clicked() {
                    act = Some(2);
                    ui.close();
                }
                if self.project.rollback.is_some() && ui.button(format!("{} {}", ph::ARROW_LINE_DOWN, crate::i18n::tr("act-clear-rollback"))).clicked() {
                    act = Some(3);
                    ui.close();
                }
                ui.separator();
                if ui.add_enabled(can_up, egui::Button::new(format!("{} {}", ph::ARROW_UP, crate::i18n::tr("act-move-up")))).on_hover_text(&crate::i18n::tr("tree-move-up-hint")).clicked() {
                    act = Some(4);
                    ui.close();
                }
                if ui.add_enabled(can_down, egui::Button::new(format!("{} {}", ph::ARROW_DOWN, crate::i18n::tr("act-move-down")))).on_hover_text(&crate::i18n::tr("tree-move-down-hint")).clicked() {
                    act = Some(5);
                    ui.close();
                }
                ui.separator();
                if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("act-delete-feature"))).clicked() {
                    act = Some(6);
                    ui.close();
                }
            });
        });
        if let Some(a) = act {
            self.tree_action(a, ti, nid, prev_feat, next_feat);
        }
    }

    /// AN ACTION ON A TREE NODE - the same thing a context-menu item does, but without the menu.
    ///
    /// The codes: 1 edit, 2 roll back to here, 3 clear the rollback, 4 up, 5 down, 6 delete, 7 suppress or
    /// enable, 8 rename. It was lifted out of the row's drawing so that a test can do exactly what a person
    /// does rather than poke at the model's fields: the feature history is half the work in a parametric CAD,
    /// and it cannot be checked with a fake.
    pub(super) fn tree_action(&mut self, act: u8, ti: usize, nid: Id, prev_feat: Option<usize>, next_feat: Option<usize>) {
        match Some(act) {
            Some(1) => self.start_feat_cmd_edit(nid),
            Some(2) => {
                self.begin_edit(&crate::i18n::tr("status-rollback"));
                self.project.set_rollback(Some(ti + 1));
                self.resync_after_topology_change();
                self.commit_edit();
            }
            Some(3) => {
                self.begin_edit(&crate::i18n::tr("status-rollback-clear"));
                self.project.set_rollback(None);
                self.resync_after_topology_change();
                self.commit_edit();
            }
            Some(4) => {
                if let Some(pf) = prev_feat {
                    self.move_feature(ti, pf);
                }
            }
            Some(5) => {
                if let Some(nf) = next_feat {
                    // "down" means raising the next feature above the current one, which lands the current one below it
                    self.move_feature(nf, ti);
                }
            }
            Some(6) => self.ask_delete(Sel::Feature(ti)),
            Some(7) => {
                // suppress or enable: a pass-through (a no-op modifier) triggers a rebuild, then the caches are synced
                let on = !self.project.timeline[ti].suppressed;
                self.begin_edit(if on { crate::i18n::tr("status-op-suppressed") } else { crate::i18n::tr("status-op-enabled") });
                self.project.set_feature_suppressed(ti, on);
                self.resync_after_topology_change();
                self.commit_edit();
            }
            Some(8) => self.start_rename(nid),
            _ => {}
        }
    }


    /// A body row (imported, with no source feature): visibility + selection. WITHOUT nesting the faces - faces
    /// are picked by a click in 3D rather than in the tree (the tree is a plain list).
    pub(super) fn tree_body_row(&mut self, ui: &mut egui::Ui, mi: usize) {
        let name = crate::i18n::name(&self.project.mesh_name(mi));
        ui.horizontal(|ui| {
            let mut vis = self.project.bodies.get(mi).is_none_or(|b| b.visible);
            if ui.add(egui::Checkbox::without_text(&mut vis)).changed() {
                self.project.bodies[mi].visible = vis;
                self.visibility_changed(); // otherwise the "what is shown" cache gives yesterday's answer
            }
            if self.rename_node_active(ui, RenameNode::Body(mi)) {
                return; // inline renaming of the body
            }
            let r = ui.selectable_label(self.sel == Sel::Mesh(mi), format!("{} {}", ph::CUBE, name)).on_hover_text(&crate::i18n::tr("tree-body-hint"));
            if r.clicked() {
                self.sel = Sel::Mesh(mi);
            }
            let mut ren = false;
            r.context_menu(|ui| {
                if ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                    ren = true;
                    ui.close();
                }
            });
            if ren {
                self.start_rename_node(RenameNode::Body(mi), name.clone());
            }
        });
    }



    pub(super) fn tree_panel(&mut self, ui: &mut egui::Ui) {
        let cam = self.workbench.is_cam();
        egui::Panel::left("tree").resizable(true).default_size(260.0)
.show(ui, |ui| {
            // the lower FIXED section: the tools and the operations (the CAM workbench only)
            if cam {
                egui::Panel::bottom("ops_tools").resizable(true).default_size(260.0).show(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("opsscroll").show(ui, |ui| {
                        self.tools_tree(ui);
                        ui.separator();
                        self.ops_tree(ui);
                    });
                });
            }
            // the upper section: the machine and the geometry (it scrolls)
            egui::ScrollArea::vertical().id_salt("geomscroll").show(ui, |ui| {
                ui.add_space(4.0);

                // the machine and the stock: the CAM workbench only
                if cam {
                    if ui.selectable_label(self.sel == Sel::Machine, format!("{} {}", ph::WRENCH, crate::i18n::tr1("tree-machine", "post", self.project.machine.post.label()))).clicked() {
                        self.sel = Sel::Machine;
                    }
                    let st = self.project.stock;
                    let stock_lbl = if st.auto { crate::i18n::tr("stock-auto") } else { format!("{:.0}×{:.0}×{:.0}", st.size[0], st.size[1], st.size[2]) };
                    if ui.selectable_label(self.sel == Sel::Stock, format!("{} {}", ph::BOUNDING_BOX, crate::i18n::tr1("tree-stock", "size", &stock_lbl))).clicked() {
                        self.sel = Sel::Stock;
                    }
                    ui.separator();
                }

                // ONE build tree (the history in order, grouped by component)
                self.build_tree(ui);

                // the embedded originals of the imports (dxf/svg/stl)
                if !self.project.sources.is_empty() {
                    ui.separator();
                    egui::CollapsingHeader::new(format!("{} {}", ph::FILE, crate::i18n::tr1("tree-import-sources", "n", &self.project.sources.len().to_string())))
                        .id_salt("sources")
                        .show(ui, |ui| {
                            let mut del: Option<usize> = None;
                            for si in 0..self.project.sources.len() {
                                let s = &self.project.sources[si];
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::tr2("tree-source-size", "name", &crate::i18n::name(&s.name), "kb", &crate::i18n::num(s.data.len() as f64 / 1024.0, 1)));
                                    if ui.small_button(ph::TRASH).clicked() {
                                        del = Some(si);
                                    }
                                });
                            }
                            if let Some(i) = del {
                                self.project.sources.remove(i);
                            }
                        });
                }
                ui.separator();
            });
        });
    }

    pub(super) fn build_tree(&mut self, ui: &mut egui::Ui) {
        // Nodes are created from the Create panel on the left; here there is only the tree itself, with no duplicated buttons.
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&crate::i18n::tr("tree-title")).strong());
        });
        // THE TREE SEARCH: on a part of fifty features the right one cannot be found by eye.
        // The search goes by THE SAME label a person sees in the row (`feature_row_label`) - otherwise it would
        // stop finding what is displayed.
        //
        // THE FIELD'S WIDTH MUST NOT COME FROM THE PANEL'S WIDTH. This used to be `available_width()`, that is,
        // the width of LAST frame's panel, and it made a feedback loop: the field demands that width, the content
        // is wider by an icon and a button, the panel grows, and on the next frame the field asks for more still.
        // What that looked like: type into the search, and the panel slides smoothly to the right, squeezing
        // everything out of the window.
        //
        // The layout goes RIGHT TO LEFT: the button takes its place first and the field gets WHAT IS LEFT of the
        // row. That is why `desired_width(INFINITY)` is safe here - it means "whatever remains", not "whatever
        // the panel had". A zero width must not be asked for: the field would collapse and there would be nowhere
        // to type - which is what the first attempt did, leaving a search field impossible to reach.
        ui.horizontal(|ui| {
            ui.label(ph::MAGNIFYING_GLASS);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !self.tree.search.is_empty() && ui.small_button(ph::X).on_hover_text(&crate::i18n::tr("tree-search-clear")).clicked() {
                    self.tree.search.clear();
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.tree.search)
                        .id(egui::Id::new("tree_search_field"))
                        .hint_text(&crate::i18n::tr("tree-search"))
                        .desired_width(f32::INFINITY),
                );
            });
        });
        // The mates appear only in an Assembly or a subassembly. ONLY the active assembly's joints are shown:
        // the joints of nested subassemblies are hidden so that they do not get in the way (filtered by
        // joint_home == the active context).
        if matches!(self.workbench, Workbench::Assembly) {
            // THE SHARED contours toggle belongs here only. Inside a Part a sketch's visibility is governed by
            // its own checkbox in the tree, and a second, shared one would be superfluous there.
            ui.checkbox(&mut self.set.show_contours, format!("{}  {}", ph::POLYGON, crate::i18n::tr("tree-contours-toggle")))
                .on_hover_text(&crate::i18n::tr("tree-contours-hint"));
            ui.checkbox(&mut self.set.show_joints, &crate::i18n::tr("tree-show-joints")).on_hover_text(&crate::i18n::tr("tree-joints-hint"));
            if ui
                .checkbox(&mut self.set.show_interference, &crate::i18n::tr("tree-interference"))
                .on_hover_text(&crate::i18n::tr("tree-interference-hint"))
                .changed()
            {
                self.interference.rev = u64::MAX; // force a recompute when it is toggled
            }
            if self.set.show_interference && !self.interference.pairs.is_empty() {
                ui.colored_label(
                    self.scheme.pal.error(),
                    format!("{} {}", ph::WARNING, crate::i18n::tr1("tree-interference-n", "n", &self.interference.pairs.len().to_string())),
                );
            }
        }
        ui.separator();
        // THE CONTEXTUAL tree: the folders of the current context. The breadcrumbs live in the top panel.
        use qymcad_core::feature::{ComponentKind, FeatureKind as FK};
        let ctx = self.current_ctx_id();
        let ctx_nodes: Vec<(Id, FK)> = self.project.timeline.iter().filter(|n| n.parent == Some(ctx)).map(|n| (n.id, n.kind.clone())).collect();

        // The origin: the reference base planes (each component has a frame of its own)
        egui::CollapsingHeader::new(format!("{} {}", ph::SELECTION_ALL, crate::i18n::tr("tree-origin-node"))).id_salt(("origin", ctx)).default_open(false).show(ui, |ui| {
            use qymcad_core::feature::{BasePlane, SketchPlane};
            // a component's base planes are selectable DATUMS: a click creates a sketch on that plane
            let mut new_on: Option<BasePlane> = None;
            for (nm, bp) in [(&crate::i18n::tr("plane-xy"), BasePlane::XY), (&crate::i18n::tr("plane-xz"), BasePlane::XZ), (&crate::i18n::tr("plane-yz"), BasePlane::YZ)] {
                if ui.selectable_label(false, format!("{} {nm}", ph::DOT)).on_hover_text(&crate::i18n::tr("tree-base-plane-hint")).clicked() {
                    new_on = Some(bp);
                }
            }
            ui.label(egui::RichText::new(format!("{} {}", ph::DOT, crate::i18n::tr("tree-origin"))).weak());
            if let Some(b) = new_on {
                self.create_sketch_on(SketchPlane::World(b));
            }
        });

        // The sketches
        let sketches: Vec<Id> = ctx_nodes
            .iter()
            .filter(|(_, k)| matches!(k, FK::Sketch { .. }))
            .map(|(id, _)| *id)
            // THE SEARCH RUNS THROUGH EVERYTHING: a section is filtered by the same text its row displays
            .filter(|id| self.project.sketch_index(*id).is_some_and(|si| self.tree_text_matches(&crate::i18n::name(&self.project.sketches[si].name))))
            .collect();
        if !sketches.is_empty() {
            egui::CollapsingHeader::new(format!("{} {}", ph::POLYGON, crate::i18n::tr("tree-sketches"))).id_salt(("sketches", ctx)).default_open(true).show(ui, |ui| {
                for sid in sketches {
                    self.tree_sketch_row(ui, sid);
                }
            });
        }

        // The datums (user planes, points and axes)
        let datums: Vec<(Id, FK)> = ctx_nodes
            .iter()
            .filter(|(_, k)| matches!(k, FK::Plane { .. } | FK::DatumPoint { .. } | FK::DatumAxis { .. }))
            .filter(|(_, k)| self.tree_text_matches(&self.datum_row_name(k)))
            .cloned()
            .collect();
        if !datums.is_empty() {
            egui::CollapsingHeader::new(format!("{} {}", ph::SELECTION_PLUS, crate::i18n::tr("tree-datums"))).id_salt(("datums", ctx)).default_open(true).show(ui, |ui| {
                for (id, k) in datums {
                    self.tree_datum_row(ui, id, &k);
                }
            });
        }

        // Bodies and history (the features that produce a body), in build order; plus the imported bodies in the root
        let feat_tis: Vec<usize> = (0..self.project.timeline.len()).filter(|&ti| self.project.timeline[ti].parent == Some(ctx) && self.project.timeline[ti].kind.body().is_some()).collect();
        let imported: Vec<usize> = if ctx == self.project.root {
            // ALL of a node's bodies, not the first one. Splitting a body yields several; only the first was
            // recognised, and the remaining pieces surfaced in the ROOT assembly as separate numbered rows - so
            // the tree showed bodies nobody had made.
            let produced: std::collections::HashSet<Id> = self.project.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
            (0..self.project.bodies.len())
                .filter(|&mi| self.project.mesh_id(mi).map_or(true, |b| !produced.contains(&b)))
                .filter(|&mi| self.tree_text_matches(&crate::i18n::name(&self.project.mesh_name(mi))))
                .collect()
        } else {
            Vec::new()
        };
        if !feat_tis.is_empty() || !imported.is_empty() {
            egui::CollapsingHeader::new(format!("{} {}", ph::CUBE, crate::i18n::tr("tree-bodies"))).id_salt(("bodies", ctx)).default_open(true).show(ui, |ui| {
                // The rollback bar: it is dragged up and down the feature list. Everything BELOW the line is
                // suppressed (it does not build), everything above it is active. active_k is the number of active
                // features (those above the line).
                let rb = self.project.rollback;
                let active_k = feat_tis.iter().filter(|&&ti| rb.map_or(true, |r| ti < r)).count();
                // ONE OPERATION IS ONE ROW: a multi-contour extrude or cut used to breed a node per contour
                // (plus BodyBoolean joins). An operation span is collapsed into one row (its first node); the
                // span's other nodes are not duplicated in the tree. It is edited in the shared half-sketcher
                // (by a double click).
                let mut shown: std::collections::HashSet<qymcad_core::model::Id> = std::collections::HashSet::new();
                // ITERATE BY Id, NOT BY INDEX IN THE TIMELINE. A tree row can delete a feature and swap it with
                // its neighbour - after which the indexes taken before the loop point somewhere else, and the last
                // one past the end of the timeline altogether. That surfaced as a crash: "len is 14 but the index
                // is 15". An Id survives both deletion and reordering; a node that has vanished is simply skipped -
                // there is nobody left to draw its row in this frame.
                let feat_ids: Vec<qymcad_core::model::Id> = feat_tis.iter().map(|&ti| self.project.timeline[ti].id).collect();
                let searching = !self.tree.search.trim().is_empty();
                for (idx, &id) in feat_ids.iter().enumerate() {
                    if idx == active_k && !searching {
                        // THE ROLLBACK BAR IS NOT DRAWN WHILE SEARCHING: half the rows are hidden, and a line
                        // meaning "everything below is suppressed" would land in the middle of a filtered list,
                        // where it separates nothing and only lies.
                        self.rollback_bar(ui, &feat_tis, active_k);
                    }
                    let Some(ti) = self.project.timeline_index(id) else { continue };
                    if shown.contains(&id) {
                        continue; // part of an operation already shown - the row is not duplicated
                    }
                    if !self.tree_row_matches(ti) {
                        continue;
                    }
                    for s in self.project.feature_op_span(id) {
                        shown.insert(s);
                    }
                    self.tree_feature_row(ui, ti);
                }
                if active_k >= feat_tis.len() && !feat_tis.is_empty() {
                    self.rollback_bar(ui, &feat_tis, active_k); // everything is active, so the line sits at the very bottom
                }
                for mi in imported {
                    self.tree_body_row(ui, mi); // the imported bodies (STL and the like, with no source feature)
                }
            });
        }

        // The child components: a double click enters them
        let children: Vec<(usize, Id, String)> = self
            .project
            .components
            .iter()
            .enumerate()
            .filter(|(_, c)| c.parent == Some(ctx))
            .map(|(ci, c)| (ci, c.id, crate::i18n::name(&c.name)))
            .filter(|(_, _, name)| self.tree_text_matches(name))
            .collect();
        // WHAT WAS DROPPED IS DECIDED AFTER THE WALK. `components` must not be changed in the middle of drawing
        // the list: the indexes would shift under the feet of that very loop.
        let mut drop_act: Option<(Id, Id, super::TreeDrop)> = None;
        let mut rows_geom: Vec<(Id, egui::Rect)> = Vec::new();
        if !children.is_empty() {
            egui::CollapsingHeader::new(format!("{} {}", ph::STACK, crate::i18n::tr("tree-components"))).id_salt(("comps", ctx)).default_open(true).show(ui, |ui| {
                for (ci, cid, name) in children {
                    let is_asm = self.project.component_kind(cid) == Some(ComponentKind::Assembly);
                    let icon = if is_asm { ph::STACK } else { ph::CUBE };
                    ui.horizontal(|ui| {
                        let mut vis = self.project.components.iter().find(|c| c.id == cid).map(|c| c.visible).unwrap_or(true);
                        if ui.add(egui::Checkbox::without_text(&mut vis)).on_hover_text(&crate::i18n::tr("tree-component-visible-hint")).changed() {
                            self.set_component_visible(cid, vis);
                        }
                        if self.rename_node_active(ui, RenameNode::Component(cid)) {
                            return; // inline renaming: a field instead of the label
                        }
                        let in_multi = self.is_multi() && self.tree_sel.multi.contains(&cid);
                        // THE ROW: A CLICK SELECTS, A DOUBLE CLICK ENTERS, PRESS AND DRAG MOVES IT.
                        //
                        // This used to be `dnd_drag_source`, and it took the press for itself: a click stopped
                        // selecting and a double click stopped entering the part. Reported behaviour: a selection
                        // could not be made by a click at all, the row was immediately grabbed for dragging, and
                        // no other program works that way. Quite so. The dragging is assembled by hand on top of
                        // an ordinary row: `click_and_drag` leaves the click and the double click in place, and
                        // dragging only begins once the cursor has moved with the button held.
                        //
                        // THE GRABBED ROW TRAVELS WITH THE CURSOR - that same row, not a text in a popup.
                        // Reported behaviour: while holding, there was no sign at all that an item had been picked
                        // up and was being moved; and a popup with a text instead of the row was rejected
                        // separately. What is wanted is THE ITEM ITSELF travelling under the cursor. It is drawn in
                        // a layer of its own and that layer is shifted by the distance the mouse has covered:
                        // exactly what the built-in `dnd_drag_source` does, only the click and the double click
                        // stay where they were.
                        let carried = self.tree.drag.is_some_and(|src| src == cid || (self.tree_sel.multi.contains(&src) && self.tree_sel.multi.contains(&cid)));
                        let row = |ui: &mut egui::Ui, app: &App| {
                            ui.selectable_label(app.sel == Sel::Component(ci) || in_multi, format!("{icon} {name}"))
                                .interact(egui::Sense::click_and_drag())
                                .on_hover_text(&crate::i18n::tr("tree-component-hint"))
                        };
                        let resp = if carried {
                            let layer = egui::LayerId::new(egui::Order::Tooltip, ui.id().with(("carry", cid)));
                            let inner = ui.scope_builder(egui::UiBuilder::new().layer_id(layer), |ui| row(ui, self));
                            // The shift follows the cursor, as the built-in `dnd_drag_source` does.
                            if let Some(at) = ui.ctx().pointer_interact_pos() {
                                let delta = at - inner.inner.rect.center();
                                ui.ctx().transform_layer_shapes(layer, egui::emath::TSTransform::from_translation(delta));
                            }
                            inner.inner
                        } else {
                            row(ui, self)
                        };
                        if resp.double_clicked() {
                            self.enter_component(cid);
                        } else if resp.clicked() {
                            let m = ui.input(|i| i.modifiers);
                            self.tree_select_component(ci, cid, m.ctrl || m.command, m.shift);
                        }
                        if resp.drag_started() {
                            self.tree.drag = Some(cid);
                        }
                        // A HIT IS COMPUTED FROM THE CURSOR'S COORDINATE, NOT FROM `hovered()`.
                        //
                        // While a row is being dragged, egui gives the hover to IT - the neighbouring rows never
                        // get `hovered()` at all. That is why none of it worked: neither the highlight nor the drop.
                        // The row rectangles are gathered here and the decision is taken after the walk.
                        rows_geom.push((cid, resp.rect));
                        let mut act: Option<u8> = None; // 1 copy, 2 cut, 3 paste, 4 export STEP, 5 export STL, 6 rename
                        let multi_n = if in_multi { self.tree_sel.multi.len() } else { 0 };
                        resp.context_menu(|ui| {
                            // EDITING A COMPONENT PATTERN starts here, as editing a feature in the timeline does.
                            // The edit function had been written and covered by a test but was NOT CONNECTED to the
                            // interface: the test called it directly and nobody else could reach it. It surfaced
                            // through the compiler's "never used" warning - exactly the case where such a warning
                            // must not be silenced.
                            if let Some(pid) = self.project.comp_pattern_of(cid).map(|p| p.id) {
                                if ui.button(format!("{} {}", ph::DOTS_THREE_OUTLINE, crate::i18n::tr("act-edit-array"))).clicked() {
                                    self.start_comp_array_edit(pid);
                                    ui.close();
                                }
                                ui.separator();
                            }
                            // THE ROOT IS NOT RENAMED: its name is a catalogue key rather than the document's text
                            // (see `Project::migrate_root`). Offering the edit and silently reverting it on the next
                            // load would be worse than not offering it at all.
                            if cid != self.project.root && ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                                act = Some(6);
                                ui.close();
                            }
                            ui.separator();
                            let (cl, xl) = if multi_n > 1 {
                                (format!("{} {}", ph::COPY, crate::i18n::tr1("act-copy-selected", "n", &multi_n.to_string())), format!("{} {}", ph::SCISSORS, crate::i18n::tr1("act-cut-selected", "n", &multi_n.to_string())))
                            } else {
                                (format!("{} {}", ph::COPY, crate::i18n::tr("act-copy-ctrl-c")), format!("{} {}", ph::SCISSORS, crate::i18n::tr("act-cut-ctrl-x")))
                            };
                            if ui.button(cl).clicked() {
                                act = Some(1);
                                ui.close();
                            }
                            if ui.button(xl).clicked() {
                                act = Some(2);
                                ui.close();
                            }
                            if (self.clip.tree.is_some() || self.clip.tree_multi.is_some()) && ui.button(format!("{} {}", ph::CLIPBOARD, crate::i18n::tr("act-paste-here"))).clicked() {
                                act = Some(3);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(format!("{} {}", ph::EXPORT, crate::i18n::tr("act-export-step"))).on_hover_text(&crate::i18n::tr("tree-export-step-hint")).clicked() {
                                act = Some(4);
                                ui.close();
                            }
                            if ui.button(format!("{} {}", ph::EXPORT, crate::i18n::tr("act-export-stl"))).on_hover_text(&crate::i18n::tr("tree-export-stl-hint")).clicked() {
                                act = Some(5);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(format!("{} {}", ph::PACKAGE, crate::i18n::tr("act-save-as-part"))).on_hover_text(&crate::i18n::tr("tree-save-as-part-hint")).clicked() {
                                act = Some(7);
                                ui.close();
                            }
                        });
                        match act {
                            Some(1) => {
                                self.sel = Sel::Component(ci);
                                self.clipboard_copy(false);
                            }
                            Some(2) => {
                                self.sel = Sel::Component(ci);
                                self.clipboard_copy(true);
                            }
                            Some(3) => {
                                self.sel = Sel::Component(ci);
                                self.clipboard_paste();
                            }
                            Some(4) => self.export_step(ExportTarget::Component(cid)),
                            Some(5) => self.stl_export = Some(ExportTarget::Component(cid)),
                            Some(6) => self.start_rename_node(RenameNode::Component(cid), name.clone()),
                            Some(7) => self.open_save_part_dialog(cid),
                            _ => {}
                        }
                    });
                }
            });
        }
        // THE TARGET IS FOUND BY THE CURSOR, A HINT IS DRAWN, AND ON RELEASE THE ROW IS DROPPED.
        //
        // AUTO-SCROLLING AT THE EDGE. The list is longer than the window, so the drop target may be beyond its
        // edge; a person brings the cursor there and expects the list to move on its own. The speed is constant:
        // "the closer to the edge, the faster" would only make it harder to hit here.
        if self.tree.drag.is_some() {
            if let Some(at) = ui.ctx().input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos())) {
                const EDGE: f32 = 24.0; // the band at the edge in which scrolling starts
                const SPEED: f32 = 8.0; // points per frame
                let clip = ui.clip_rect();
                if at.x >= clip.left() && at.x <= clip.right() {
                    // The sign: a positive delta moves THE CONTENT downwards, that is, reveals what is above.
                    // At the bottom edge the opposite is wanted.
                    let d = if at.y > clip.bottom() - EDGE {
                        -SPEED
                    } else if at.y < clip.top() + EDGE {
                        SPEED
                    } else {
                        0.0
                    };
                    if d != 0.0 {
                        ui.scroll_with_delta(egui::vec2(0.0, d));
                        ui.ctx().request_repaint(); // the scroll runs frame by frame rather than in one jerk
                    }
                }
            }
        }
        // ESCAPE CANCELS THE DRAG, as in any tree. Without it a grabbed row has nowhere to go but to be dropped
        // somewhere.
        if self.tree.drag.is_some() && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.tree.drag = None;
            self.status = crate::i18n::tr("tree-drag-cancelled");
        }
        if let Some(src) = self.tree.drag {
            if let Some(at) = ui.ctx().input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos())) {
                if let Some((cid, rect)) = rows_geom.iter().copied().find(|(cid, r)| *cid != src && at.y >= r.top() && at.y <= r.bottom()) {
                    let how = super::tree_drop_intent(rect, at.y);
                    // AN INVALID TARGET GETS NEITHER A LINE NOR A HIGHLIGHT. Otherwise a person aims at a place
                    // where nothing will happen and finds out only after releasing.
                    let moving: Vec<Id> = if self.tree_sel.multi.contains(&src) { self.tree_sel.multi.clone() } else { vec![src] };
                    if !self.project.tree_drop_allowed(&moving, cid, how == super::TreeDrop::Onto) {
                        self.tree.row_rects = rows_geom;
                        if ui.ctx().input(|i| i.pointer.any_released()) {
                            self.tree.drag = None;
                        }
                        return;
                    }
                    // BETWEEN ITEMS there is a full-width coloured insertion line; OVER AN ITEM the item itself
                    // is highlighted. The ordinary behaviour of a tree.
                    let full = egui::Rect::from_min_max(egui::pos2(ui.min_rect().left(), rect.top()), egui::pos2(ui.min_rect().right(), rect.bottom()));
                    let col = ui.visuals().selection.bg_fill;
                    let pnt = ui.painter();
                    match how {
                        super::TreeDrop::Before | super::TreeDrop::After => {
                            let y = if how == super::TreeDrop::Before { full.top() } else { full.bottom() };
                            pnt.line_segment([egui::pos2(full.left(), y), egui::pos2(full.right(), y)], egui::Stroke::new(3.0, col));
                        }
                        super::TreeDrop::Onto => {
                            pnt.rect_filled(full.expand(1.0), 3.0, col.linear_multiply(0.25));
                            pnt.rect_stroke(full.expand(1.0), 3.0, egui::Stroke::new(2.0, col), egui::StrokeKind::Middle);
                        }
                    }
                    if ui.input(|i| i.pointer.any_released()) {
                        drop_act = Some((src, cid, how));
                    }
                }
            }
        }
        // THE ROW RECTANGLES ARE FOR THE TESTS: a test moves the mouse over the REAL coordinates of the rows.
        self.tree.row_rects = rows_geom;
        // THE DROP IS APPLIED AFTER THE WALK - otherwise the reorder would shift the indexes under that same loop.
        if let Some((src, dst, how)) = drop_act {
            self.tree_apply_drop(src, dst, how);
        }
        // THE BUTTON WAS RELEASED, SO THE DRAG IS OVER, wherever that happened. Otherwise a grabbed row would
        // stay grabbed forever and the next click in the tree would act as a drop.
        if ui.input(|i| i.pointer.any_released()) {
            self.tree.drag = None;
        }
    }

    pub(super) fn ops_tree(&mut self, ui: &mut egui::Ui) {
        // --- The setups + the work coordinate systems ---
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&crate::i18n::tr("cam-setups")).strong());
            if ui.small_button(ph::PLUS).on_hover_text(&crate::i18n::tr("cam-add-setup")).clicked() {
                let n = self.project.setups.len();
                let wcs = qymcad_core::model::Wcs::ALL[n.min(5)];
                self.project.setups.push(qymcad_core::model::Setup { name: crate::i18n::tr1("cam-setup-n", "n", &(n + 1).to_string()), wcs });
                self.sel = Sel::Setup(n);
            }
        });
        for si in 0..self.project.setups.len() {
            let s = &self.project.setups[si];
            let label = format!("{} {} · {}", ph::STACK, s.name, s.wcs.label());
            if ui.selectable_label(self.sel == Sel::Setup(si), label).clicked() {
                self.sel = Sel::Setup(si);
            }
        }
        ui.separator();

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&crate::i18n::tr("cam-operations")).strong());
            ui.menu_button(ph::PLUS, |ui| {
                let tool = self.project.tools.first().map(|t| t.number).unwrap_or(1);
                let kinds: [(&str, OpKind); 13] = [
                    ("Contour", OpKind::Contour { side: SideMode::Auto, tabs: Tabs::default(), ramp: Ramp::default(), climb: true, finish: false }),
                    ("Pocket", OpKind::Pocket { dogbone: false }),
                    ("Adaptive 2D", OpKind::Adaptive2D),
                    ("Slot", OpKind::Slot),
                    ("Drill", OpKind::Drill { cycle: DrillKind::Peck, peck: Some(2.0), dwell: None }),
                    ("Bore", OpKind::Bore),
                    ("Face", OpKind::Face),
                    ("Engrave", OpKind::Engrave),
                    ("Rough 3D", OpKind::Rough3D { mesh: 0 }),
                    ("Waterline 3D", OpKind::Waterline3D { mesh: 0 }),
                    ("Surface 3D", OpKind::Surface3D { mesh: 0 }),
                    ("Flat 3D", OpKind::Flat3D { mesh: 0 }),
                    ("Project 3D", OpKind::Project3D { mesh: 0 }),
                ];
                for (name, kind) in kinds {
                    if ui.button(name).clicked() {
                        let mut op = OperationDef::new(name, tool, kind);
                        // the 3D operations refer to the SELECTED part (rather than the first one)
                        let midx = match self.sel {
                            Sel::Mesh(k) | Sel::Face(k, _) => k,
                            _ => 0,
                        };
                        let midx = if midx < self.project.bodies.len() { midx } else { 0 };
                        let mid = self.project.mesh_id(midx).unwrap_or_else(|| self.project.first_mesh_id());
                        match &mut op.kind {
                            OpKind::Surface3D { mesh } | OpKind::Rough3D { mesh } | OpKind::Waterline3D { mesh } | OpKind::Project3D { mesh } | OpKind::Flat3D { mesh } => *mesh = mid,
                            _ => {}
                        }
                        // the 3D operations: fit the heights to the selected part
                        if let Some(b) = self.project.bodies.get(midx).map(|b| &b.mesh).and_then(|m| m.bounds()) {
                            match kind {
                                OpKind::Surface3D { .. } | OpKind::Rough3D { .. } | OpKind::Waterline3D { .. } | OpKind::Flat3D { .. } => {
                                    op.heights.top = b.max.z;
                                    op.heights.bottom = b.min.z;
                                    op.heights.clearance = b.max.z + 5.0;
                                    op.heights.retract = b.max.z + 2.0;
                                }
                                OpKind::Project3D { .. } => {
                                    // shallow surface engraving: 1 mm by default
                                    op.heights.top = b.max.z;
                                    op.heights.bottom = b.max.z - 1.0;
                                    op.heights.clearance = b.max.z + 5.0;
                                    op.heights.retract = b.max.z + 2.0;
                                }
                                _ => {}
                            }
                        }
                        self.project.operations.push(op);
                        self.sel = Sel::Op(self.project.operations.len() - 1);
                        ui.close();
                    }
                }
            });
        });

        let mut remove: Option<usize> = None;
        let mut dup: Option<usize> = None;
        let mut move_up: Option<usize> = None;
        let mut move_down: Option<usize> = None;
        let n = self.project.operations.len();
        let multi_setup = self.project.setups.len() > 1;
        for i in 0..n {
            ui.horizontal(|ui| {
                let op = &mut self.project.operations[i];
                ui.checkbox(&mut op.enabled, "");
                let tag = if multi_setup {
                    self.project.setups.get(op.setup).map(|s| format!(" [{}]", s.wcs.label())).unwrap_or_default()
                } else {
                    String::new()
                };
                let label = format!("{} · {}{}", op.name, op.kind.label(), tag);
                if ui.selectable_label(self.sel == Sel::Op(i), label).clicked() {
                    self.sel = Sel::Op(i);
                }
                if i > 0 && ui.small_button(ph::CARET_UP).clicked() {
                    move_up = Some(i);
                }
                if i + 1 < n && ui.small_button(ph::CARET_DOWN).clicked() {
                    move_down = Some(i);
                }
                if ui.small_button(ph::COPY).on_hover_text(&crate::i18n::tr("cam-duplicate")).clicked() {
                    dup = Some(i);
                }
                if ui.small_button(ph::TRASH).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = move_up {
            self.project.operations.swap(i - 1, i);
            self.sel = Sel::Op(i - 1);
        }
        if let Some(i) = move_down {
            self.project.operations.swap(i, i + 1);
            self.sel = Sel::Op(i + 1);
        }
        if let Some(i) = dup {
            let mut copy = self.project.operations[i].clone();
            copy.name = crate::i18n::tr1("cam-copy-suffix", "name", &copy.name);
            self.project.operations.insert(i + 1, copy);
            self.sel = Sel::Op(i + 1);
        }
        if let Some(i) = remove {
            self.project.operations.remove(i);
            self.sel = Sel::None;
        }
    }
}
