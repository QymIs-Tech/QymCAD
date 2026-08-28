//! DRAGGING IN THE TREE: ORDER AND GROUPING.
//!
//! Two requests:
//!
//! * dragging parts and subassemblies within an assembly in the list, so that the order in the tree of
//!   the right-hand panel can simply be changed for convenience (pure cosmetics);
//! * if selected parts or subassemblies are dropped onto another part or assembly, a new subassembly is
//!   created and everything selected goes into it along with the one that was dropped on.
//!
//! Both gestures are one movement of the mouse, so they are told apart by THE PLACE of the drop: at the
//! edges of a row it reorders, in the middle it gathers things inside. The middle band is wider than the
//! edges: it is easier to hit, and grouping makes more sense than an exact reordering.
#[cfg(test)]
mod tests {
    use super::super::{tree_drop_intent, App, Sel, TreeDrop};
    use qymcad_core::model::{Id, Project};

    fn asm_with_three(p: &mut Project) -> (Id, [Id; 3]) {
        let asm = p.add_assembly("Node");
        p.set_active_component(Some(asm));
        let a = p.add_part("A");
        let b = p.add_part("B");
        let c = p.add_part("C");
        (asm, [a, b, c])
    }

    fn order_in(p: &Project, parent: Id) -> Vec<String> {
        p.components.iter().filter(|c| c.parent == Some(parent)).map(|c| c.name.clone()).collect()
    }

    /// THE GESTURE IS RESOLVED BY THE PLACE OF THE DROP. The edges reorder, the middle puts things
    /// inside.
    #[test]
    fn the_drop_zone_decides_the_gesture() {
        let r = egui::Rect::from_min_size(egui::pos2(0.0, 100.0), egui::vec2(200.0, 20.0));
        assert_eq!(tree_drop_intent(r, 101.0), TreeDrop::Before, "at the top edge, stand before");
        assert_eq!(tree_drop_intent(r, 110.0), TreeDrop::Onto, "in the middle, gather into a subassembly");
        assert_eq!(tree_drop_intent(r, 119.0), TreeDrop::After, "at the bottom edge, stand after");
    }

    /// THE MIDDLE IS WIDER THAN THE EDGES — otherwise it cannot be hit, and it is the commonest gesture.
    #[test]
    fn the_middle_band_is_the_wide_one() {
        let r = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 20.0));
        let onto = (0..=20).filter(|&i| tree_drop_intent(r, i as f32) == TreeDrop::Onto).count();
        assert!(onto >= 10, "the inside band is too narrow: {onto} points out of 21");
    }

    /// REORDERING CHANGES THE ORDER IN THE TREE.
    #[test]
    fn dropping_at_the_edge_reorders() {
        let mut app = App::default();
        app.project.new_document();
        let (asm, [a, _b, c]) = asm_with_three(&mut app.project);

        assert!(app.tree_apply_drop(c, a, TreeDrop::Before), "the reordering did not happen");
        assert_eq!(order_in(&app.project, asm), ["C", "A", "B"], "C did not stand before A");
    }

    /// A DROP IN THE MIDDLE GATHERS A SUBASSEMBLY OUT OF THE TARGET AND EVERYTHING SELECTED.
    #[test]
    fn dropping_onto_a_row_groups_the_selection_with_the_target() {
        let mut app = App::default();
        app.project.new_document();
        let (asm, [a, b, c]) = asm_with_three(&mut app.project);
        app.tree_sel.multi = vec![a, b];

        assert!(app.tree_apply_drop(a, c, TreeDrop::Onto), "the grouping did not happen");
        let top = order_in(&app.project, asm);
        assert_eq!(top.len(), 1, "one subassembly must be left in the place of the target: {top:?}");
        let new_asm = app.project.components.iter().find(|x| x.parent == Some(asm)).map(|x| x.id).expect("the subassembly");
        let inside = order_in(&app.project, new_asm);
        assert_eq!(inside.len(), 3, "what was selected and the target must land inside: {inside:?}");
    }

    /// THE MAIN THING AFTER A REORDER: THE SELECTION STAYED ON THE SAME PART.
    ///
    /// The tree remembers the chosen component BY INDEX, and a reorder changes the order. Without
    /// recomputing by Id the selection silently moves to a neighbour — the wrong part gets edited and
    /// nobody finds out.
    #[test]
    fn the_selection_follows_the_part_not_the_row_number() {
        let mut app = App::default();
        app.project.new_document();
        let (_asm, [a, b, c]) = asm_with_three(&mut app.project);
        // B is selected
        let bi = app.project.component_index(b).expect("the index of B");
        app.sel = Sel::Component(bi);

        assert!(app.tree_apply_drop(c, a, TreeDrop::Before), "the reordering did not happen");

        let now = match app.sel {
            Sel::Component(ci) => app.project.components.get(ci).map(|x| x.id),
            _ => None,
        };
        assert_eq!(now, Some(b), "the selection moved to another part after the reorder");
    }

    /// AND AFTER GROUPING TOO.
    #[test]
    fn the_selection_survives_grouping() {
        let mut app = App::default();
        app.project.new_document();
        let (_asm, [a, b, c]) = asm_with_three(&mut app.project);
        let bi = app.project.component_index(b).expect("the index of B");
        app.sel = Sel::Component(bi);
        app.tree_sel.multi = vec![a, b];

        assert!(app.tree_apply_drop(a, c, TreeDrop::Onto), "the grouping did not happen");

        let now = match app.sel {
            Sel::Component(ci) => app.project.components.get(ci).map(|x| x.id),
            _ => None,
        };
        assert_eq!(now, Some(b), "the selection moved to another component after the grouping");
    }

    /// DRAG A ROW OUT OF A SELECTION AND THE WHOLE SELECTION TRAVELS. Otherwise three parts are selected,
    /// one is dragged and only it moves: the selection exists in the interface and means nothing.
    #[test]
    fn dragging_a_selected_row_moves_the_whole_selection() {
        let mut app = App::default();
        app.project.new_document();
        let (asm, [a, b, c]) = asm_with_three(&mut app.project);
        app.tree_sel.multi = vec![a, b];

        assert!(app.tree_apply_drop(a, c, TreeDrop::Before), "the reordering did not happen");
        assert_eq!(order_in(&app.project, asm), ["A", "B", "C"], "the selection did not travel whole: {:?}", order_in(&app.project, asm));
    }

    /// A DROP ONTO ITSELF DOES NOTHING — and creates no subassembly out of one.
    #[test]
    fn dropping_onto_itself_does_nothing() {
        let mut app = App::default();
        app.project.new_document();
        let (asm, [a, _b, _c]) = asm_with_three(&mut app.project);
        let before = app.project.components.len();

        assert!(!app.tree_apply_drop(a, a, TreeDrop::Onto));
        assert_eq!(app.project.components.len(), before, "a spare subassembly appeared");
        assert_eq!(order_in(&app.project, asm), ["A", "B", "C"], "the order should not have changed");
    }

    /// THE CLICK AND THE DOUBLE CLICK STAYED IN PLACE, WITH DRAGGING ON TOP OF THEM.
    ///
    /// The first edition wrapped the row in `dnd_drag_source` — and it took the press for itself: the
    /// click stopped selecting and the double click stopped entering a part. Reported behaviour: it is
    /// impossible to set the selection by a click, the row is taken for dragging at once, and no other
    /// program works that way. So the check demands BOTH: that `dnd_drag_source` is gone, and that the
    /// click and the double click are handled.
    #[test]
    fn the_row_keeps_click_and_double_click_and_drags_on_hold() {
        let src = crate::gui::panels_source::PANELS;
        // THE SEARCH WINDOW RUNS FROM THE HEADING OF THE COMPONENT LIST TO THE END OF THE FUNCTION, not
        // "plus N bytes". Counting in bytes has already failed twice: first the slice landed in the middle
        // of a multibyte letter, then the window came out shorter than the code and "lost" a double-click
        // handler nobody had touched.
        let a = src.find("tree-components").expect("the component tree is in place");
        let end = src[a..].find("\n    pub(super) fn ops_tree").map(|i| a + i).unwrap_or(src.len());
        let body = &src[a..end];
        // THE CALL is searched for, not the word: a comment nearby explains why it was removed, and the
        // check used to trip over the mention.
        assert!(!body.contains(".dnd_drag_source("), "the row is wrapped in dnd_drag_source again — it eats the click");
        assert!(body.contains("resp.double_clicked()"), "the double click on a row is lost — a part cannot be entered");
        assert!(body.contains("resp.clicked()"), "the single click on a row is lost — nothing can be selected");
        assert!(body.contains("click_and_drag"), "the row does not respond to dragging");
        assert!(body.contains("drag_started"), "the drag does not begin on a held button");
        assert!(body.contains("tree_drop_intent"), "the gesture is not resolved by the place of the drop");
        // AND WHAT WILL HAPPEN IS VISIBLE: an insertion line between items and a highlight of the item
        // itself when it is hit. Without those one drags blind, and that was said outright.
        assert!(body.contains("line_segment"), "there is no insertion line between the items");
        assert!(body.contains("rect_filled") || body.contains("rect_stroke"), "there is no highlight of an item when it is hit");
    }

    /// A DROP ONTO A SUBASSEMBLY PUTS THINGS INSIDE IT rather than breeding another one.
    ///
    /// As requested: if a part, or a list of parts, or a subassembly is dropped not onto a part but onto
    /// a subassembly, there is no need to create another subassembly — everything should go into the one
    /// being dropped on.
    #[test]
    fn dropping_onto_an_assembly_puts_things_inside_it() {
        let mut app = App::default();
        app.project.new_document();
        let (top, [a, b, _c]) = asm_with_three(&mut app.project);
        app.project.set_active_component(Some(top));
        let target = app.project.add_assembly("Socket");
        let before = app.project.components.len();
        app.tree_sel.multi = vec![a, b];

        assert!(app.tree_apply_drop(a, target, TreeDrop::Onto), "the drop onto a subassembly did not work");
        assert_eq!(app.project.components.len(), before, "a spare subassembly was created instead of moving things inside");
        let inside = order_in(&app.project, target);
        assert_eq!(inside.len(), 2, "the wrong things landed inside the subassembly: {inside:?}");
        assert!(inside.contains(&"A".to_string()) && inside.contains(&"B".to_string()), "the wrong ones are inside: {inside:?}");
    }

    /// AND ONTO A PART it is still a new subassembly: nothing can be put inside a part, and two things can
    /// only be joined by a common parent.
    #[test]
    fn dropping_onto_a_part_still_makes_a_new_subassembly() {
        let mut app = App::default();
        app.project.new_document();
        let (_asm, [a, b, c]) = asm_with_three(&mut app.project);
        let before = app.project.components.len();
        app.tree_sel.multi = vec![a, b];

        assert!(app.tree_apply_drop(a, c, TreeDrop::Onto), "the drop onto a part did not work");
        assert_eq!(app.project.components.len(), before + 1, "no new subassembly was created");
    }

    /// A REAL GESTURE WITH THE MOUSE, not a call to the handler.
    ///
    /// Every check above calls `tree_apply_drop` directly — none of them sees whether the mouse reaches
    /// it. And it did not: the hit was computed through `resp.hovered()`, but while a row is being dragged
    /// egui gives the hover to IT, and the neighbouring rows never get it. There was neither a highlight
    /// nor a drop at all, and the report said exactly that: nothing works.
    ///
    /// Here the tree is drawn for real, the coordinates of the rows are taken from the frame, and the
    /// mouse is led along them: press on a row, move to the middle of another, release.
    fn drag_row_onto(app: &mut App, from: Id, to: Id, part: f32) -> bool {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let draw = |app: &mut App, ui: &mut egui::Ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                app.build_tree(ui);
            });
        };
        // frame 1 — find out where the rows are
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |c| draw(app, c));
        let rect_of = |app: &App, id: Id| app.tree.row_rects.iter().find(|(c, _)| *c == id).map(|(_, r)| *r);
        let (Some(a), Some(b)) = (rect_of(app, from), rect_of(app, to)) else {
            return false;
        };
        let start = a.center();
        let end = egui::pos2(b.center().x, b.top() + b.height() * part);
        let ev = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        // pressed
        let mut i2 = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        i2.events.push(egui::Event::PointerMoved(start));
        i2.events.push(ev(start, true));
        let _ = ctx.run_ui(i2, |c| draw(app, c));
        // dragged
        let mut i3 = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        i3.events.push(egui::Event::PointerMoved(end));
        let _ = ctx.run_ui(i3, |c| draw(app, c));
        // released
        let mut i4 = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        i4.events.push(egui::Event::PointerMoved(end));
        i4.events.push(ev(end, false));
        let _ = ctx.run_ui(i4, |c| draw(app, c));
        true
    }

    #[test]
    fn a_real_mouse_drag_onto_a_part_groups_them() {
        let mut app = App::default();
        app.project.new_document();
        let (asm, [a, _b, c]) = asm_with_three(&mut app.project);
        // THE TREE SHOWS THE CHILDREN OF THE ACTIVE PATH OF THE APPLICATION rather than of the active
        // component of the project: entering must go the same way a double click does, otherwise there
        // will simply be no rows in the frame.
        app.enter_component(asm);
        let before = app.project.components.len();

        assert!(drag_row_onto(&mut app, a, c, 0.5), "the rows of the tree were not found in the frame");
        assert_eq!(
            app.project.components.len(),
            before + 1,
            "the mouse dropped onto a part and no subassembly was created — the gesture never reached the handler"
        );
    }

    #[test]
    fn a_real_mouse_drag_to_the_edge_reorders() {
        let mut app = App::default();
        app.project.new_document();
        let (asm, [a, _b, c]) = asm_with_three(&mut app.project);
        app.enter_component(asm);

        assert!(drag_row_onto(&mut app, c, a, 0.05), "the rows of the tree were not found in the frame");
        assert_eq!(order_in(&app.project, asm), ["C", "A", "B"], "the mouse dragged to the top edge and the order did not change");
    }

    /// A CLICK ON A ROW SELECTS IT — CHECKED WITH A REAL MOUSE rather than over the source.
    ///
    /// The check "there is a `resp.clicked()` in the code" has already failed: an editing script CUT OUT
    /// the click and double-click handlers along with a neighbouring block, and selection in the tree
    /// stopped working. What caught it was not that check but a gesture like this one.
    #[test]
    fn a_real_click_selects_the_row() {
        let mut app = App::default();
        app.project.new_document();
        let (asm, [a, _b, _c]) = asm_with_three(&mut app.project);
        app.enter_component(asm);

        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let draw = |app: &mut App, ui: &mut egui::Ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                app.build_tree(ui);
            });
        };
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |c| draw(&mut app, c));
        let at = app.tree.row_rects.iter().find(|(c, _)| *c == a).map(|(_, r)| r.center()).expect("row A is in the frame");

        let ev = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let mut i2 = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        i2.events.push(egui::Event::PointerMoved(at));
        i2.events.push(ev(at, true));
        i2.events.push(ev(at, false));
        let _ = ctx.run_ui(i2, |c| draw(&mut app, c));

        let picked = match app.sel {
            Sel::Component(ci) => app.project.components.get(ci).map(|x| x.id),
            _ => None,
        };
        assert_eq!(picked, Some(a), "the click on the row did not select the part");
    }
    /// A DROP IS UNDONE BY ONE Ctrl+Z.
    ///
    /// The edits of the tree went straight into `self.project`, past `App::edit` — although it says there
    /// that everything changing the document must go through it. Ctrl+Z did not see a reorder at all: a
    /// gesture went wrong and there was no putting it back.
    #[test]
    fn a_drop_is_undone_by_one_ctrl_z() {
        let mut app = App::default();
        app.project.new_document();
        let (asm, [a, _b, c]) = asm_with_three(&mut app.project);
        app.enter_component(asm);
        let before = order_in(&app.project, asm);
        let undo_before = app.undo_len_for_test();

        assert!(drag_row_onto(&mut app, c, a, 0.05), "the rows of the tree were not found in the frame");
        assert_eq!(order_in(&app.project, asm), ["C", "A", "B"], "setup: the order changed");
        assert_eq!(app.undo_len_for_test(), undo_before + 1, "a move must be ONE step of undo");

        app.undo_for_test();
        assert_eq!(order_in(&app.project, asm), before, "Ctrl+Z did not put the order back as it was");
    }

    /// ESCAPE DURING A DRAG CANCELS IT.
    #[test]
    fn escape_cancels_the_drag() {
        let mut app = App::default();
        app.project.new_document();
        let (asm, [a, _b, c]) = asm_with_three(&mut app.project);
        app.enter_component(asm);
        let before = order_in(&app.project, asm);

        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let draw = |app: &mut App, ui: &mut egui::Ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                app.build_tree(ui);
            });
        };
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |x| draw(&mut app, x));
        let rect_of = |app: &App, id: Id| app.tree.row_rects.iter().find(|(x, _)| *x == id).map(|(_, r)| *r);
        let (start, end) = (rect_of(&app, c).expect("the row C").center(), rect_of(&app, a).expect("the row A"));
        let end = egui::pos2(end.center().x, end.top() + end.height() * 0.05);
        let ev = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };

        let mut i2 = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        i2.events.push(egui::Event::PointerMoved(start));
        i2.events.push(ev(start, true));
        let _ = ctx.run_ui(i2, |x| draw(&mut app, x));

        let mut i3 = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        i3.events.push(egui::Event::PointerMoved(end));
        let _ = ctx.run_ui(i3, |x| draw(&mut app, x));

        // ESCAPE, without releasing the button.
        let mut i4 = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        for pressed in [true, false] {
            i4.events.push(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed, repeat: false, modifiers: Default::default() });
        }
        let _ = ctx.run_ui(i4, |x| draw(&mut app, x));

        // And only now released — there is nothing left to drop.
        let mut i5 = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        i5.events.push(egui::Event::PointerMoved(end));
        i5.events.push(ev(end, false));
        let _ = ctx.run_ui(i5, |x| draw(&mut app, x));

        assert_eq!(order_in(&app.project, asm), before, "Escape did not cancel the drag — the tree got reordered all the same");
    }

    /// THE ROW BEING CARRIED IS VISIBLE UNDER THE CURSOR.
    ///
    /// Reported behaviour: while holding, there is no sign at all that an item has been taken and is
    /// being moved. The check goes by the frame: while a row is dragged, its text is drawn TWICE — in its
    /// own place and as a copy at the cursor — and the copy lies on top of everything else.
    #[test]
    fn the_carried_row_is_drawn_under_the_cursor() {
        let mut app = App::default();
        app.project.new_document();
        let (asm, [a, _b, c]) = asm_with_three(&mut app.project);
        app.enter_component(asm);

        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let draw = |app: &mut App, ui: &mut egui::Ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                app.build_tree(ui);
            });
        };
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |x| draw(&mut app, x));
        let rect_of = |app: &App, id: Id| app.tree.row_rects.iter().find(|(x, _)| *x == id).map(|(_, r)| *r);
        let (start, end) = (rect_of(&app, c).expect("the row C").center(), rect_of(&app, a).expect("the row A").center());
        let ev = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };

        let mut i2 = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        i2.events.push(egui::Event::PointerMoved(start));
        i2.events.push(ev(start, true));
        let _ = ctx.run_ui(i2, |x| draw(&mut app, x));

        let mut i3 = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        i3.events.push(egui::Event::PointerMoved(end));
        let out = ctx.run_ui(i3, |x| draw(&mut app, x));

        let mut texts = Vec::new();
        for cs in &out.shapes {
            super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
        }
        let seen = texts.iter().filter(|t| t.contains('C')).count();
        assert!(seen >= 2, "the row being carried is not visible under the cursor: the frame holds {texts:?}");
    }

    /// THE TREE NEITHER HIGHLIGHTS NOR ACCEPTS AN IMPERMISSIBLE TARGET.
    ///
    /// The rule is asked of the kernel (`Project::tree_drop_allowed`) — where the operations themselves
    /// live, otherwise the highlight and the action would diverge unnoticed. The checks of the rule itself
    /// live in the kernel (`tree_reorder_and_group.rs`).
    ///
    /// WHAT CAN BE CHECKED BY A GESTURE HERE: dropping a row onto itself. The cycle of putting an ancestor
    /// inside its own descendant cannot be assembled with a mouse in principle — the tree shows the
    /// children of the current context ONLY, and the ancestor is not in the frame. The first edition of
    /// this check tried and failed on "the rows were not found": that was a mistake in the scenario, not a
    /// breakage of the ban.
    #[test]
    fn a_row_dropped_onto_itself_changes_nothing() {
        let mut app = App::default();
        app.project.new_document();
        let (asm, [a, _b, _c]) = asm_with_three(&mut app.project);
        app.enter_component(asm);
        let before = order_in(&app.project, asm);
        let undo_before = app.undo_len_for_test();

        assert!(drag_row_onto(&mut app, a, a, 0.5), "the rows of the tree were not found in the frame");
        assert_eq!(order_in(&app.project, asm), before, "a drop onto itself changed something");
        assert_eq!(app.undo_len_for_test(), undo_before, "a drop into nowhere left a step of undo");
    }

    /// AT THE EDGE OF THE LIST IT SCROLLS BY ITSELF.
    ///
    /// The list is longer than the window, and the target of a move may lie beyond its edge. The cursor is
    /// brought there in the expectation that the list will travel by itself; otherwise moving a part ten
    /// rows down is impossible.
    ///
    /// The measurement goes BY THE FRAME: where the rows stood before and after several frames with the
    /// cursor at the bottom edge.
    #[test]
    fn the_list_scrolls_when_the_cursor_is_at_the_edge() {
        let mut app = App::default();
        app.project.new_document();
        let asm = app.project.add_assembly("Node");
        app.project.set_active_component(Some(asm));
        for i in 0..40 {
            app.project.add_part(&format!("Part {i}"));
        }
        app.enter_component(asm);

        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 300.0));
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        // THE TREE LIVES IN A SCROLL AREA — as in the right-hand panel of the program. Without one there
        // is nothing to scroll and the check would be empty.
        let draw = |app: &mut App, ui: &mut egui::Ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("geomscroll").max_height(240.0).show(ui, |ui| {
                    app.build_tree(ui);
                });
            });
        };
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |c| draw(&mut app, c));
        let first = app.tree.row_rects.first().copied().expect("the rows of the tree are in the frame");
        let ev = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };

        // The first row was taken and the cursor brought to the BOTTOM edge of the window.
        let start = first.1.center();
        let mut i2 = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        i2.events.push(egui::Event::PointerMoved(start));
        i2.events.push(ev(start, true));
        let _ = ctx.run_ui(i2, |c| draw(&mut app, c));

        let edge = egui::pos2(start.x, 236.0);
        let before = app.tree.row_rects.first().copied().expect("the rows are in place").1.top();
        // TIME MUST RUN. A measurement: scrolling was requested every frame (by 8 points), and the list
        // moved by only 4 — egui SMOOTHS scrolling over time, and in the check the clock stood still. In
        // the program the frames run for real, so the clock is moved here too.
        for k in 0..15 {
            let mut i3 = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(1.0 + k as f64 / 60.0),
                predicted_dt: 1.0 / 60.0,
                ..Default::default()
            };
            i3.events.push(egui::Event::PointerMoved(edge));
            let _ = ctx.run_ui(i3, |c| draw(&mut app, c));
        }
        let after = app.tree.row_rects.first().copied().expect("the rows are in place").1.top();

        assert!(
            after < before - 10.0,
            "the list did not travel under a cursor at the bottom edge: the first row was at {before:.0} and became {after:.0}"
        );
    }
}
