//! TAKING ONE DRAWING TOOL RELEASES THE PREVIOUS ONE.
//!
//! Two tools at once means ambiguity under the cursor: the click goes to whichever handler stands higher
//! in the code, while the person is certain they are working with the one picked last. The error is quiet,
//! and people blame themselves for it.
//!
//! THIS RULE WAS GUARDED FOR THE ASSEMBLY ONLY. `tools_are_mutually_exclusive` walks `AssemblyTool::ALL`
//! (nine tools), and the moment it was made exhaustive it found a real defect - the anchor re-pick was
//! never released. The DRAWING tools, shared by the Sketch and the Part, had no such check at all, and
//! they are the ones a person spends the day in.
//!
//! EVERY PAIR, from `DrawTool::ALL`, minus the sub-modes: a tool that is switched on from inside another
//! one cannot be "taken", so the sentence is not about it. That exclusion is `DrawTool::has_door`, a
//! predicate on the type - not a subset chosen here, which is how the Assembly check went blind.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_ui_state::DrawTool;

    /// Take a tool through its own door - the same handle a button or a key uses.
    ///
    /// The measuring tool and the primitive have no `_for_test` door, so the field the button sets is set
    /// here, exactly as `assembly_tools::doors` does for the two Assembly sub-modes.
    fn arm(app: &mut App, t: DrawTool) {
        match t {
            DrawTool::Draw => app.set_sk_tool(1),
            DrawTool::ClickOp => qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_of!(app), &mut app.viewing.mode_3d, 1),
            DrawTool::Modify => qymcad_ui_state::modify_button(
                qymcad_ui_state::editing_of!(app),
                &mut qymcad_ui_state::tools_of!(app),
                app.sk_pat,
                &app.tool_prefs,
                1,
            ),
            DrawTool::Move => qymcad_part::start_move_tool(&mut qymcad_ui_state::tools_of!(app), &mut app.status, 1),
            DrawTool::Pattern => qymcad_part::start_pattern(&mut qymcad_ui_state::tools_of!(app), &mut app.status, 1),
            DrawTool::Dimension => qymcad_ui_state::set_dim_tool(
                &mut qymcad_ui_state::tools_of!(app),
                &mut app.viewing.mode_3d,
                &app.project,
                app.chosen.sel,
                app.sketch_ses,
                &mut app.status,
                1,
            ),
            DrawTool::Command => app.start_feat_cmd(4),
            DrawTool::Measure => qymcad_ui_state::set_measure(&mut qymcad_ui_state::tools_of!(app), true),
            DrawTool::Place => app.start_prim_cmd(1),
            // The sub-modes: `has_door` says they are not taken on their own, and the sweep skips them.
            DrawTool::Pick | DrawTool::Import | DrawTool::Corner | DrawTool::Annot => {}
        }
    }

    fn in_hand(app: &mut App) -> Vec<DrawTool> {
        qymcad_ui_state::armed_draw_tools(&qymcad_ui_state::tools_of!(app))
    }


    /// THE RULER GOES THROUGH A DOOR, and the door takes the tool rather than losing it.
    ///
    /// It used to set `measure.on` itself and push `BarAsk::SketchSelectMode`. The frame performs a request
    /// AFTER the panel has drawn, that request calls `exit_draw_tools`, and `exit_draw_tools` clears the
    /// measuring tool along with the rest: on, then off, in the same frame. The button did nothing at all,
    /// and nobody had reported it - a button that does nothing looks like a button one has misunderstood.
    ///
    /// Both halves are checked: the door takes the tool, and the toolbar uses the door.
    #[test]
    fn the_ruler_button_actually_turns_the_tool_on() {
        let mut app = super::super::screen_keys::tests::plate();
        let si = app.project.sketches.first().map(|s| s.id).expect("the plate has a sketch");
        app.sketch_ses.editing = Some(si);

        qymcad_ui_state::set_measure(&mut qymcad_ui_state::tools_of!(app), true);
        assert!(app.tools.armed.measuring(), "the door did not take the measuring tool");

        let bar = include_str!("../../../qymcad-part/src/lib.rs");
        assert!(
            crate::gui::render_source::has(bar, "qymcad_ui_state::set_measure(&mut qymcad_ui_state::tools_in!(bc), on)"),
            "the ruler no longer goes through the door"
        );
        assert!(
            !crate::gui::render_source::has(bar, "bc.measure.on = on"),
            "the toolbar sets the flag itself again, and a deferred request will put it out"
        );
    }

    #[test]
    fn taking_a_drawing_tool_releases_the_previous_one() {
        let doors: Vec<DrawTool> = DrawTool::ALL.into_iter().filter(|t| t.has_door()).collect();
        assert!(doors.len() >= 8, "suspiciously few doors to sweep: {}", doors.len());
        let mut both: Vec<String> = Vec::new();
        for first in doors.iter().copied() {
            for second in doors.iter().copied() {
                if first == second {
                    continue; // the same door is a toggle rather than a change of tool
                }
                let mut app = super::super::screen_keys::tests::plate();
                let si = app.project.sketches.first().map(|s| s.id).expect("the plate has a sketch");
                app.sketch_ses.editing = Some(si); // the drawing tools belong to an open sketch

                arm(&mut app, first);
                // GUARD AGAINST A VACUOUS CHECK: the first tool really was taken, otherwise there is
                // nothing for the second one to release and a green result says nothing.
                assert!(!in_hand(&mut app).is_empty(), "GUARD: \"{first:?}\" was not taken, so there is nothing to check the change on");
                arm(&mut app, second);

                let armed = in_hand(&mut app);
                if armed.len() != 1 {
                    both.push(format!("{first:?} -> {second:?}: tools left in hand: {armed:?}"));
                }
            }
        }
        assert!(
            both.is_empty(),
            "two drawing tools at once: the click goes to the wrong one while the person is certain they work with the last taken:\n{}",
            both.join("\n")
        );
    }
}
