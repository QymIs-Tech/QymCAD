//! KEYBOARD INPUT: the commands of a frame, the Esc ladder, the tool hotkeys, the autosave.
//!
//! The keyboard is a layer of its own rather than "part of the frame": it has an order of interception
//! (whoever asked first took the press), and out of that order grows a whole class of reports like "Esc
//! closed the sketch straight away". While the handlers lay in the middle of `update`, the order was
//! neither visible nor checkable.

use super::*;

impl App {
    /// THE KEYBOARD COMMANDS OF A FRAME: Enter, U, Ctrl+Enter, Esc, Delete, Ctrl+A.
    ///
    /// Moved out of `update` unchanged. There they lay among the other phases of a frame (clearing
    /// textures, the splash screen, the datum debounce, intercepting the closing of the window, the
    /// autosave, the panels), and neither the fact that the keyboard is a phase of its own nor the order
    /// in which the keys intercept each other was visible. The Esc ladder lives here too — it belongs
    /// beside the rest of the keys rather than in a place of its own.
    pub(super) fn handle_key_commands(&mut self, ctx: &egui::Context) {
        // F1 IS HELP ABOUT WHAT IS BEING DONE. Not the title page: F1 is pressed at the very minute
        // somebody is stuck on a particular tool, and an extra click to reach the right article is
        // exactly the difference that makes people stop using the help.
        // THE COMMAND SEARCH: space when the keyboard is free, Ctrl+K always.
        //
        // Space is convenient and unoccupied, but inside a field it must TYPE ITSELF: `60 + 2` is a
        // lawful expression. So the search has two entrances, and the second works from inside a field —
        // otherwise it would be unreachable exactly when it is needed most.
        // THE STATE OF THE KEYBOARD IS ASKED FOR BEFORE `ctx.input`, NOT INSIDE IT.
        //
        // `wants_keyboard_input()` takes locks of its own; called INSIDE `ctx.input(...)`, which already
        // holds the input, it DEADLOCKS. Caught by a full test run: separately the tests passed, together
        // they stood dead — "hanging for more than 60 seconds" and not one failure. The other lines below
        // ask it from outside and are therefore alive.
        let typing_now = ctx.egui_wants_keyboard_input();
        let open_search = ctx.input(|i| (i.modifiers.command && i.key_pressed(egui::Key::K)) || (!typing_now && !i.modifiers.any() && i.key_pressed(egui::Key::Space)));
        if open_search {
            crate::gui::command_search::toggle_command_search(&mut self.win);
        }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            let a = self.help_for_context();
            self.open_help(a);
        }
        // Enter confirms an array if one is active and the focus is not in a text field
        if self.tools.armed.pat_op() != 0 && !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.confirm_pattern();
        }
        // Enter confirms A COMPONENT ARRAY (in an assembly)
        if self.side.carr.mode != 0 && !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            crate::gui::commands::apply_comp_array(&mut self.part_ctx());
        }
        // Returning to the choice of contours of an active sketch command goes by the "U" key through
        // `part_hotkey` (not "C": C is the circle in a sketch and the chamfer in a Part, and it clashed
        // while editing a feature).
        // Enter inside a Part command. For a sketch command in 2D (choosing a profile, multi-select
        // included): the first Enter CONFIRMS the choice and takes one into 3D to set the dimension (the
        // gizmo or the field), the second applies it.
        if self.tools.armed.commanding() && !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            let sketch_cmd = matches!(self.tools.armed.cmd_kind(), 1 | 3);
            if self.tools.picking.contour().is_some() {
                // in the half-sketcher of choosing the contour of a slot a CLICK on the contour is
                // awaited — Enter does NOT apply the feature
            } else if sketch_cmd && !self.viewing.mode_3d && !self.tools.gsel.profiles.is_empty() {
                self.viewing.mode_3d = true; // the choice is ready -> into 3D to enter the height or angle
                self.status = crate::i18n::tr("in-drag-or-type");
            } else {
                crate::gui::commands::apply_feat_cmd(&mut self.part_ctx());
            }
        }
        // Ctrl+Enter finishes the current context (leaving a sketch, a part or a subassembly one level
        // up) without the mouse. Only when no command or array is active — otherwise Enter applies
        // those.
        if self.tools.armed.cmd_kind() == 0
            && self.tools.armed.pat_op() == 0
            && !ctx.egui_wants_keyboard_input()
            && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter))
        {
            self.exit_context();
        }
        // ESC cancels a typing mode or a drawing, otherwise it clears the selection (the whole ladder
        // lives in `on_escape` so that it can be run by tests without a window: the class of reports
        // "ESC closes the sketch straight away").
        // A DRIVER LIST HAS FIRST REFUSAL ON ESCAPE. The answer is taken away every frame, whether or not
        // Escape was pressed, so that a field which has since gone cannot leave a stale "open" behind.
        let list_was_open = super::expr_field::take_list_open(ctx);
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            // ESC IN THREE STEPS, NOT TWO. The first one takes down the list of drivers, the second leaves
            // the field, the third cancels the command.
            //
            // Reported behaviour: typing in a parameter field brings up the list, and Escape cancels the
            // operation instead of closing the list. The field's own ladder was already right, but it never
            // got the key: this handler runs BEFORE anything is drawn, and by surrendering the focus it left
            // the field unable to consume Escape — after which the popup's own `key_pressed(Escape)` closed
            // the whole thing.
            //
            // So the key is simply left alone here: the field is still focused, consumes it and closes its
            // list, and everything downstream sees nothing.
            if list_was_open {
                // the list takes it
            } else if ctx.egui_wants_keyboard_input() {
                ctx.memory_mut(|m| {
                    if let Some(id) = m.focused() {
                        m.surrender_focus(id);
                    }
                });
            } else {
                self.on_escape();
            }
        }
        // F2 renames whatever is selected - the key every tree a person has used renames by
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::F2)) {
            qymcad_ui_state::rename_selected(&self.project, &mut self.side.rename, self.chosen.sel);
        }
        // Delete removes the selected entities of a sketch (while editing one)
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            // OUTSIDE the editing of a sketch: any structural node of the tree (a feature, a body, a
            // sketch, a datum, a mate) brings up a yes-or-no confirmation. DEL used to do nothing at all
            // for datums and sketches.
            if self.sketch_ses.editing.is_none() {
                if matches!(
                    self.chosen.sel,
                    Sel::Feature(_) | Sel::Mesh(_) | Sel::Sketch(_) | Sel::Plane(_) | Sel::DatumPoint(_) | Sel::DatumAxis(_) | Sel::Joint(_) | Sel::Component(_)
                ) {
                    self.deferred.delete = Some(self.chosen.sel);
                }
            } else if let Sel::Sketch(si) = self.chosen.sel {
                qymcad_sketch::delete_selected_in_sketch(&mut self.sketch_ctx(), si);
            }
        }
        // Ctrl+A selects all the geometry of the active sketch (entities and points)
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::A)) {
            if let Sel::Sketch(si) = self.chosen.sel {
                if qymcad_ui_state::edit_si(&self.project, &self.sketch_ses) == Some(si) {
                    qymcad_ui_state::select_all_sketch(&mut self.tools.annot, &mut self.tools.gsel, &self.project, &mut self.tools.sel_sk, &mut self.status, si);
                }
            }
        }
        // X switches the selected entities into or out of construction geometry
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| !i.modifiers.any() && i.key_pressed(egui::Key::X)) {
            if let Sel::Sketch(si) = self.chosen.sel {
                if qymcad_ui_state::edit_si(&self.project, &self.sketch_ses) == Some(si) {
                    let eids: Vec<Id> = self.tools.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
                    if !eids.is_empty() {
                        let now = self.project.toggle_construction(si, &eids);
                        self.project.solve_sketch(si);
                        qymcad_ui_state::invalidate(&mut self.regen);
                        self.status = if now { crate::i18n::tr("in-made-construction") } else { crate::i18n::tr("in-made-normal") };
                    }
                }
            }
        }
        // Ctrl+C / Ctrl+X / Ctrl+V — the clipboard.
        // While editing a sketch with entities selected: copying and pasting GEOMETRY.
        // Otherwise (a node of the tree is selected): copying and pasting SKETCHES, PARTS and
        // SUBASSEMBLIES.
        // egui translates Cmd+C/X/V into Event::Copy/Cut/Paste — THOSE are what get caught (otherwise
        // `key_pressed(C)` is empty), with the direct hotkey as a reserve.
        if !ctx.egui_wants_keyboard_input() {
            let (do_copy, do_cut, do_paste) = ctx.input(|i| {
                let cmd = i.modifiers.command;
                let mut c = cmd && i.key_pressed(egui::Key::C);
                let mut x = cmd && i.key_pressed(egui::Key::X);
                let mut v = cmd && i.key_pressed(egui::Key::V);
                for e in &i.events {
                    match e {
                        egui::Event::Copy => c = true,
                        egui::Event::Cut => x = true,
                        egui::Event::Paste(_) => v = true,
                        _ => {}
                    }
                }
                (c, x, v)
            });
            if do_cut {
                self.clipboard_copy(true);
            } else if do_copy {
                self.clipboard_copy(false);
            } else if do_paste {
                self.clipboard_paste();
            }
        }
        // After copying a node of the tree a marker is put into THE CLIPBOARD OF THE SYSTEM: egui emits
        // Event::Paste (that is, Ctrl+V) only when that clipboard is non-empty. Without this only the
        // Paste menu item worked while the Ctrl+V key stayed silent.
        if std::mem::take(&mut self.side.clip.os_ping) {
            ctx.output_mut(|o| o.commands.push(egui::OutputCommand::CopyText("qymcad-tree-clip".to_string())));
        }
        // Undo and redo: Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y (not taken away from text fields)
        if !ctx.egui_wants_keyboard_input() {
            let (do_undo, do_redo) = ctx.input(|i| {
                let cmd = i.modifiers.command;
                let z = i.key_pressed(egui::Key::Z);
                let y = i.key_pressed(egui::Key::Y);
                (cmd && z && !i.modifiers.shift, cmd && ((z && i.modifiers.shift) || y))
            });
            if do_undo {
                self.undo();
            } else if do_redo {
                self.redo();
            }
        }
    }

    /// THE ESC LADDER: THE INNERMOST state is cancelled, and only at the very bottom is the sketch left.
    /// The order matters — reports arrive exactly when some rung is missing and ESC "falls through" to
    /// closing the sketch (the measure tool and the selected elements were two such rungs).
    pub(super) fn on_escape(&mut self) {
        if self.tools.inline.dim().is_some() || self.tools.inline.circle().is_some() {
            // the popup for editing a dimension is open (a double click on a dimension or a circle) —
            // ESC cancels ONLY the popup rather than closing the whole sketch. Otherwise the chain
            // reached `finish_sketch_edit`.
            self.tools.inline.clear();
            self.tools.dim.buf.clear();
            self.tools.dim.focus = false;
        } else if self.params.arr.axis_pick {
            self.params.arr.axis_pick = false; // the axis-pick sub-mode is left first, the command is NOT cancelled
            self.status = crate::i18n::tr("in-axis-pick-cancelled");
        } else if self.params.rev.pick_axis || self.params.rev.pick_line {
            // the sub-mode of picking the axis of revolution is left, the command is NOT cancelled
            if self.params.rev.pick_line {
                return_view(&mut self.part_ctx()); // cancelling an action does not touch the camera
            }
            self.params.rev.pick_axis = false;
            self.params.rev.pick_line = false;
            self.status = crate::i18n::tr("in-axis-pick-cancelled");
        } else if self.side.carr.mode != 0 {
            self.side.carr = CompArrayCmd::default(); // a component array is cancelled without a trace
            self.tools.cmd.params.clear();
            self.status = crate::i18n::tr("in-comp-array-cancelled");
        } else if self.side.m3.on {
            // THE MEASURING TOOL: Esc first drops WHAT WAS CLICKED, and only when that is empty does it
            // leave the tool. That way a miss does not throw one out of measuring (things usually need
            // measuring several times in a row).
            if self.side.m3.picks.is_empty() {
                self.side.m3.on = false;
                self.status = crate::i18n::tr("in-measure-off");
            } else {
                self.side.m3.picks.clear();
                self.status = crate::i18n::tr("in-measure-hint");
            }
        } else if self.params.boolean.pick.is_some() {
            self.params.boolean.pick = None;
            self.status = crate::i18n::tr("in-bool-cancelled");
        } else if self.params.boolean.edit.is_some() {
            self.params.boolean.edit = None;
            self.status = crate::i18n::tr("in-bool-done");
        } else if self.tools.picking.contour().is_some() {
            // ONLY the choice of the contour of a slot is cancelled (returning to 3D); the sweep or
            // loft command is NOT closed
            self.tools.picking.clear();
            return_view(&mut self.part_ctx()); // cancelling an action does not touch the camera
            self.status = crate::i18n::tr("in-contour-cancelled");
        } else if self.tools.armed.commanding() {
            cancel_feat_cmd(&mut self.part_ctx());
        } else if self.side.joint.edit_repick.is_some() {
            self.side.joint.edit_repick = None; // the pick of a new anchor is cancelled first, the editing is NOT left
            self.status = crate::i18n::tr("in-anchor-swap-cancelled");
        } else if self.side.joint.edit.is_some() {
            qymcad_ui_state::exit_joint_edit(&mut self.side.joint, &mut self.status);
        } else if self.side.joint.ground_pick {
            self.side.joint.ground_pick = false;
            self.status = crate::i18n::tr("in-ground-off");
        } else if self.side.joint.pick_faces {
            self.side.joint.pick_faces = false;
            self.side.joint.pick_first = None;
            self.status = crate::i18n::tr("in-joint-faces-cancelled");
        }
        // THE REST OF THE ASSEMBLY TOOLS GO BY Esc AS WELL.
        //
        // Only two of the nine were named here: collecting a joint and grounding. A standalone anchor, a
        // group, a width, a tangency and a relation WERE NOT RELEASED by Esc — one presses, believes one
        // has left, and the next click goes somewhere else. Found by the guard
        // `escape_drops_every_assembly_tool`, and it is the same disease that already produced a class of
        // troubles with the highlight: the modes are enumerated by name and a new one is forgotten.
        else if !self.armed_assembly_tools().is_empty() {
            crate::gui::assembly_tools::drop_assembly_tools(&mut self.side.joint);
            self.status = crate::i18n::tr("in-assembly-tool-cancelled");
        } else if self.tools.pending_import.curves.is_some() {
            self.tools.pending_import.curves = None;
            self.status = crate::i18n::tr("in-import-cancelled");
        } else if let Some(k) = self.tools.picking.cancel_key() {
            self.tools.picking.clear(); // the modes Escape simply puts down; the words come from the mode itself
            self.status = crate::i18n::tr(k);
        } else if self.tools.picking.plane_face().is_some() {
            self.tools.picking.set_plane_face(None);
            self.status = crate::i18n::tr("in-plane-face-cancelled");
        } else if self.tools.sel_sk.constraint.is_some() || self.tools.sel_sk.modify.is_some() {
            self.tools.sel_sk.constraint = None;
            self.tools.sel_sk.modify = None;
            // AND THE EDIT MODE ITSELF. Only THE EXPECTED PICK was extinguished while `tool.modify`
            // stayed: the cancellation worked, yet the tool bar went on saying "Mirror" and the button in
            // the panel stayed pressed. Switched off yet looking switched on is the worst kind of
            // cancellation: one is sure the tool is active and cannot understand why a click does
            // nothing.
            self.tools.armed = qymcad_ui_state::Armed::None;
        } else if self.tools.armed.click_op() != 0 {
            self.tools.armed = qymcad_ui_state::Armed::None;
        } else if self.tools.armed.pat_op() != 0 {
            self.tools.armed = qymcad_ui_state::Armed::None;
            self.tools.pat.edit = None;
            self.tools.pat.center = None;
            self.status = crate::i18n::tr("in-array-cancelled");
        } else if self.tools.armed.move_op() != 0 {
            self.tools.armed = qymcad_ui_state::Armed::None;
            self.tools.tool.move_base = None;
            self.status = crate::i18n::tr("in-move-cancelled");
        } else if self.side.clip.geom_place {
            self.side.clip.geom_place = false;
            self.status = crate::i18n::tr("in-insert-cancelled");
        } else if self.side.clip.geom_pending.is_some() {
            self.side.clip.geom_pending = None;
            self.status = crate::i18n::tr("in-copy-cancelled");
        } else if self.tools.place.dim.is_some() {
            // a provisional length of a line (the second element is awaited) — cancel it; otherwise
            // leave the dimension where it is
            if self.tools.dim.first.is_some() {
                if let (Sel::Sketch(si), Some(ci)) = (self.chosen.sel, self.tools.place.dim) {
                    if ci < self.project.sketches[si].constraints.len() {
                        self.project.sketches[si].constraints.remove(ci);
                        self.project.solve_sketch(si);
                        qymcad_ui_state::invalidate(&mut self.regen);
                    }
                }
            }
            self.tools.place.dim = None;
            self.tools.dim.first = None;
        } else if self.tools.dim.first.is_some() {
            self.tools.dim.first = None; // cancel the first reference (the point)
        } else if !self.tools.tool.pts.is_empty() {
            self.tools.tool.pts.clear(); // break off the construction under way
        } else if let Some(msg) = qymcad_ui_state::release_armed_sketch_tool(&mut qymcad_ui_state::tools_of!(self)) {
            // ONE RUNG FOR EVERY SKETCH TOOL IN HAND, and the tool itself says what to clear. There used to
            // be a rung per family here and none for the editing tools, so mirror, offset, fillet, chamfer
            // and the arrays could not be put down at all - reported on the mirror, true of all of them.
            self.status = crate::i18n::tr(msg);
        } else if self.tools.pending_import.draw_pts.is_some() {
            self.tools.pending_import.draw_pts = None;
        } else if self.sketch_ses.editing.is_some() && (!self.tools.sel_sk.items.is_empty() || self.tools.annot.text.is_some() || self.tools.annot.note.is_some()) {
            // With elements SELECTED, ESC first clears the selection — as in any grown-up CAD. The
            // selection used to take no part in the ladder, and the very first ESC closed the sketch.
            self.tools.sel_sk.clear(); // the selection and whatever was waiting for it
            self.tools.annot.text = None;
            self.tools.annot.note = None;
            self.status = crate::i18n::tr("in-selection-cleared");
        } else {
            // THE LADDER ENDS HERE: Esc gives things back, it does not finish a context. Leaving the
            // sketch was the rung below; it is Ctrl+Enter now - see `the_sketch_is_left_by_ctrl_enter.rs`.
            self.chosen.sel = Sel::None;
        }
    }

    /// The tool hotkeys, as in grown-up CAD. They work without modifiers and when the focus is not in a
    /// text field. The context decides the layout: editing a sketch gives drawing, editing, dimensions
    /// and constraints; a Part gives features and primitives; an Assembly gives components and mates. The
    /// hotkeys are repeated in the tooltips of the buttons.
    pub(super) fn handle_tool_hotkeys(&mut self, ctx: &egui::Context) {
        // FOCUS IN A FIELD MUST NOT KILL EVERY KEY.
        //
        // An unconditional `return` stood here, and it extinguished ALL 23 tool keys in ALL commands the
        // moment the cursor landed in any input field. The most visible case: inside an extrusion `U`
        // ("re-choose the contour") could not be pressed until the focus was knocked off with the mouse.
        //
        // A bare letter in a field is not intercepted — it must type itself: expressions contain both `w`
        // and `len`. But ALT plus a letter does not type itself in a field, and that is given to the
        // command. The rule is one: with no focus, the bare letter; with focus, Alt.
        let typing = ctx.egui_wants_keyboard_input();
        use egui::Key;
        let key = ctx.input(|i| {
            let ok = if typing { i.modifiers.alt && !i.modifiers.command && !i.modifiers.ctrl } else { !i.modifiers.any() };
            if !ok {
                return None;
            }
            const KEYS: [Key; 23] = [
                Key::S, Key::L, Key::R, Key::C, Key::A, Key::P, Key::G, Key::D, Key::E, Key::O, Key::N, Key::T, Key::F, Key::M, Key::X,
                Key::K, Key::Q, Key::H, Key::U, Key::J, Key::I, Key::B, Key::Y,
            ];
            KEYS.into_iter().find(|&k| i.key_pressed(k))
        });
        let Some(key) = key else { return };
        if qymcad_ui_state::edit_si(&self.project, &self.sketch_ses).is_some() {
            self.sketch_hotkey(key);
        } else {
            match self.workbench {
                Workbench::Part => self.part_hotkey(key),
                Workbench::Assembly => self.assembly_hotkey(key),
                _ => {}
            }
        }
    }

    /// THE AUTOSAVE: every few minutes, while there are unsaved edits, a copy is written silently beside
    /// the project (`<name>.autosave.qcad`; an unnamed one goes into the temporary directory). An
    /// ordinary Save removes the autosave. `force` is for the tests and for quitting. A crash or a power
    /// cut no longer eats the work.
    pub(crate) fn maybe_autosave(&mut self, force: bool) {
        // THE PERIOD IS A SETTING rather than a constant: on a heavy assembly the write is noticeable,
        // and the price of the pause against the price of lost work is different for everybody. Zero means
        // no autosave; `force` (quitting, a test) works even then: that is no longer "once every N
        // minutes" but an explicit request to write.
        if !force {
            if self.set.autosave_secs == 0 {
                return;
            }
            if self.disk.edits.last_autosave.elapsed() < std::time::Duration::from_secs(self.set.autosave_secs) {
                return;
            }
        }
        self.disk.edits.last_autosave = std::time::Instant::now();
        let key = qymcad_ui_state::edit_key(&self.draw_ctx());
        if key == self.disk.edits.saved_key || key == self.disk.edits.autosave_key {
            return; // clean, or this state has been autosaved already
        }
        if self.regen.bg.iter().any(|b| b.kind == BgKind::Save) {
            return; // a write is already under way — no jostling, we try again next period
        }
        let path = qymcad_ui_state::autosave_path(&self.disk.project_path);
        self.disk.io.autosave_key = Some(key); // applied ONLY if the write really went through
        crate::gui::io_jobs::spawn_save(&mut self.disk.io, &mut self.live, &mut self.project, &mut self.regen, &mut self.status, path, true);
    }
}
