//! THE MODEL TREE: its rows, its gestures, and the two trees drawn from it.
//!
//! Split out of `panels.rs`, which had grown to 6 129 lines holding four unrelated subjects, where finding
//! the row of a feature meant scrolling past the settings window.

use qymcad_ui_state::Notice;
use super::*;

impl App {
    /// The tree panel, through its own doorway: the panel is a free function over `TreeCtx`, and what it
    /// asks the application for is carried out AFTER the drawing.
    pub(crate) fn tree_panel(&mut self, ui: &mut egui::Ui) {
        let mut asks = Vec::new();
        crate::gui::panels_tree::tree_panel(&mut self.tree_ctx(&mut asks), ui);
        self.do_tree_asks(asks);
    }

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
        let mut moving: Vec<Id> = if self.chosen.tree_sel.multi.contains(&dragged) { self.chosen.tree_sel.multi.clone() } else { vec![dragged] };
        moving.retain(|&m| m != target);
        if moving.is_empty() {
            return false;
        }
        let keep: Option<Id> = match self.chosen.sel {
            Sel::Component(ci) => self.project.components.get(ci).map(|c| c.id),
            _ => None,
        };
        // ONE DROP MAKES ONE UNDO STEP. The edit used to go straight into `self.project`, past `App::edit`,
        // which states plainly that everything changing the document must go through it - and Ctrl+Z did not
        // see the reorder at all. If nothing changed there is no step either: `Edit` closes itself and leaves
        // no empty trace in the history.
        let mut ed = self.edit(crate::i18n::tr("tree-drop-step"));

        let changed = match how {
            // DROPPED ONTO A SUBASSEMBLY: the things go INSIDE it. There is nothing to create a new one from -
            // an existing assembly was chosen, and the expectation is that the things end up in it. Breeding
            // another one beside it is doing something other than what was asked.
            //
            // DROPPED ONTO A PART: then, and only then, a new subassembly. Nothing goes inside a part, and two
            // things can only be joined by a common parent.
            super::TreeDrop::Onto if ed.project().component_kind(target) == Some(qymcad_core::feature::ComponentKind::Assembly) => {
                let mut any = false;
                for m in &moving {
                    any |= ed.project().reparent_component(*m, target);
                }
                any
            }
            super::TreeDrop::Onto => ed.project().group_components_into_assembly(&moving, target, crate::i18n::tr("tree-group-name")).is_some(),
            super::TreeDrop::Before | super::TreeDrop::After => {
                // "after the target" means "before the next sibling"; the last one has no next, and then it is
                // the end of the list.
                let before = match how {
                    super::TreeDrop::Before => Some(target),
                    _ => {
                        let par = ed.project().components.iter().find(|c| c.id == target).and_then(|c| c.parent);
                        let sibs: Vec<Id> = ed.project().components.iter().filter(|c| c.parent == par).map(|c| c.id).collect();
                        sibs.iter().position(|&x| x == target).and_then(|i| sibs.get(i + 1).copied())
                    }
                };
                let mut any = false;
                for m in &moving {
                    any |= ed.project().reorder_component_before(*m, before);
                }
                any
            }
        };
        drop(ed);
        if changed {
            // THE SELECTION GOES BY Id, NOT BY INDEX.
            if let Some(id) = keep {
                self.chosen.sel = match self.project.component_index(id) {
                    Some(ci) => Sel::Component(ci),
                    None => Sel::None,
                };
            }
            crate::gui::commands::resync_after_topology_change(&mut self.part_ctx());
        }
        changed
    }

    /// Copy or cut the selected node into the TREE clipboard: a sketch (outside editing), a part or a subassembly.
    pub(super) fn tree_clipboard_copy(&mut self, cut: bool) {
        tree_copy(&self.chosen, &self.project, &mut self.side.clip, &mut self.status, cut);
    }

    /// Paste from the tree clipboard into a target component (the selected one, otherwise the active context).
    /// A copy is a deep clone (new Ids); a cut re-parents the node (keeping its Id and its associativity).
    pub(super) fn tree_clipboard_paste(&mut self) {
        // The bulk component clipboard takes priority.
        if let Some((ids, cut)) = self.side.clip.tree_multi.clone() {
            self.paste_components_multi(&ids, cut);
            return;
        }
        let Some(clip) = self.side.clip.tree else {
            self.status = Notice::refusal(crate::i18n::tr("tree-clipboard-empty"));
            return;
        };
        if let TreeClip::Feature { nid } = clip {
            return qymcad_part::paste_feature(&mut self.part_ctx(), nid);
        } // the tool, opened with the copy's values
        use qymcad_core::feature::ComponentKind;
        let root = self.project.root;
        // The target depends on what is in the clipboard.
        let target = match clip {
            // A sketch can be pasted into ANY component (a part, a subassembly or the root): the selected
            // component, otherwise the active context (having entered a Part, that Part).
            TreeClip::Sketch { .. } => match self.chosen.sel {
                Sel::Component(ci) => self.project.components.get(ci).map(|c| c.id).unwrap_or_else(|| self.project.active_ctx()),
                _ => self.project.active_ctx(),
            },
            // A component can be pasted ONLY into an assembly (a part inside a part is forbidden): the selected
            // subassembly; if a Part is selected, its parent assembly; otherwise the active context.
            TreeClip::Feature { .. } => return,
            TreeClip::Component { .. } => match self.chosen.sel {
                Sel::Component(ci) => match self.project.components.get(ci) {
                    Some(c) if self.project.component_kind(c.id) == Some(ComponentKind::Assembly) => c.id,
                    Some(c) => c.parent.unwrap_or(root),
                    None => self.project.active_ctx(),
                },
                _ => self.project.active_ctx(),
            },
        };
        match clip {
            TreeClip::Feature { .. } => {}
            TreeClip::Sketch { sid, cut } => {
                if cut {
                    if self.project.move_sketch_node(sid, target) {
                        self.side.clip.tree = None;
                        self.status = Notice::info(crate::i18n::tr("status-sketch-moved"));
                    } else {
                        self.status = Notice::error(crate::i18n::tr("status-sketch-move-failed"));
                    }
                } else if self.project.clone_sketch_node(sid, target).is_some() {
                    self.status = Notice::info(crate::i18n::tr("status-sketch-copied"));
                } else {
                    self.status = Notice::error(crate::i18n::tr("status-sketch-copy-failed"));
                }
                qymcad_ui_state::invalidate(&mut self.regen);
            }
            TreeClip::Component { id, cut } => {
                let into_part = self.project.component_is_part(target);
                if cut {
                    if self.project.reparent_component(id, target) {
                        self.side.clip.tree = None;
                        self.status = Notice::info(crate::i18n::tr("status-component-moved"));
                    } else if into_part {
                        self.status = Notice::refusal(crate::i18n::tr("status-paste-needs-assembly"));
                    } else {
                        self.status = Notice::refusal(crate::i18n::tr("status-move-impossible"));
                    }
                    qymcad_ui_state::invalidate(&mut self.regen);
                } else if let Some(cl) = self.project.clone_component(id, target) {
                    qymcad_ui_state::mark_dirty_for_rebuild(&mut self.rebuild_ctx()); // the document is marked; the scheduler builds the clone's bodies through the kernel
                    if let Some(ci) = self.project.components.iter().position(|c| c.id == cl) {
                        self.chosen.sel = Sel::Component(ci);
                    }
                    self.status = Notice::info(crate::i18n::tr("status-component-copied"));
                } else if into_part {
                    self.status = Notice::refusal(crate::i18n::tr("status-paste-needs-assembly"));
                    qymcad_ui_state::invalidate(&mut self.regen);
                } else {
                    self.status = Notice::error(crate::i18n::tr("status-component-copy-failed"));
                    qymcad_ui_state::invalidate(&mut self.regen);
                }
            }
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
            Some(1) => crate::gui::commands::start_feat_cmd_edit(&mut self.part_ctx(), nid),
            Some(2) => {
                qymcad_ui_state::begin_edit(&mut self.disk.edits, &self.project, crate::i18n::tr("status-rollback"));
                self.project.set_rollback(Some(ti + 1));
                crate::gui::commands::resync_after_topology_change(&mut self.part_ctx());
                qymcad_ui_state::commit_edit(&mut self.rebuild_ctx());
            }
            Some(3) => {
                qymcad_ui_state::begin_edit(&mut self.disk.edits, &self.project, crate::i18n::tr("status-rollback-clear"));
                self.project.set_rollback(None);
                crate::gui::commands::resync_after_topology_change(&mut self.part_ctx());
                qymcad_ui_state::commit_edit(&mut self.rebuild_ctx());
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
            Some(6) => qymcad_ui_state::ask_delete(&mut self.deferred, Sel::Feature(ti)),
            Some(7) => {
                // suppress or enable: a pass-through (a no-op modifier) triggers a rebuild, then the caches are synced
                let on = !self.project.timeline[ti].suppressed;
                qymcad_ui_state::begin_edit(&mut self.disk.edits, &self.project, if on { crate::i18n::tr("status-op-suppressed") } else { crate::i18n::tr("status-op-enabled") });
                self.project.set_feature_suppressed(ti, on);
                crate::gui::commands::resync_after_topology_change(&mut self.part_ctx());
                qymcad_ui_state::commit_edit(&mut self.rebuild_ctx());
            }
            Some(8) => qymcad_ui_state::start_rename(&self.project, &mut self.side.rename, nid),
            _ => {}
        }
    }
}

mod rows;
pub(crate) use rows::*;
mod rename;
pub(crate) use rename::*;
mod build;
pub(crate) use build::*;
mod copy;
pub(crate) use copy::*;

mod a_moved_bar_rebuilds_only_below_it;
