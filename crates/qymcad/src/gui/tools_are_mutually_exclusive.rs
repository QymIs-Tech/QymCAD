//! TAKING ONE TOOL RELEASES THE PREVIOUS ONE.
//!
//! Two tools at once means ambiguity under the cursor: the click goes to whichever handler stands
//! higher in the code, while the person is certain they are working with the one picked last. The
//! error is quiet, and people blame themselves for it.
//!
//! EVERY pair is checked against the single list (`AssemblyTool::ALL`): take the first, take the second,
//! and exactly one must be left in hand.
//!
//! IT USED TO SAY THAT AND NOT DO IT. The doors were a list of SEVEN written by hand here, while the
//! enumeration has NINE: the axis pick and the anchor re-pick were missing, and the rule was unchecked for
//! them - they are taken from a button in the joint's own popup rather than from the toolbar, so nobody
//! noticed. The comment claimed the sweep was exhaustive, which is the worst kind of wrong: a promise that
//! stops anyone from looking. The doors now come from `assembly_tools::doors`, shared with the F1 check,
//! and the sweep really does walk `AssemblyTool::ALL`.
#[cfg(test)]
mod tests {
    use super::super::assembly_tools::doors::arm;
    use super::super::assembly_tools::AssemblyTool;
    use super::super::App;
    use qymcad_core::model::Id;

    fn two_parts(app: &mut App) -> Vec<Id> {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        let root = app.project.root;
        app.enter_component(root);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        app.viewing.mode_3d = true;
        app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect()
    }

    #[test]
    fn taking_a_tool_releases_the_previous_one() {
        let mut both: Vec<String> = Vec::new();
        for first in AssemblyTool::ALL {
            for second in AssemblyTool::ALL {
                if first == second {
                    continue; // the same door is a toggle rather than a change of tool
                }
                let (first_name, second_name) = (first.help_mode(), second.help_mode());
                let mut app = App::default();
                let mine = two_parts(&mut app);
                assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
                app.workbench = super::super::Workbench::Assembly;

                arm(&mut app, first);
                // GUARD AGAINST A VACUOUS CHECK: the first tool really was taken, otherwise there is no change to check.
                assert!(!app.armed_assembly_tools().is_empty(), "GUARD: \"{first_name}\" was not taken, so there is nothing to check the change on");
                arm(&mut app, second);

                let armed = app.armed_assembly_tools().len();
                if armed != 1 {
                    both.push(format!("\"{first_name}\" -> \"{second_name}\": tools left in hand: {armed}"));
                }
            }
        }
        assert!(
            both.is_empty(),
            "two tools at once: the click goes to the wrong one while the person is certain they work with the last taken:\n{}",
            both.join("\n")
        );
    }
}
