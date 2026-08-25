//! TAKING ONE TOOL RELEASES THE PREVIOUS ONE.
//!
//! Two tools at once means ambiguity under the cursor: the click goes to whichever handler stands
//! higher in the code, while the person is certain they are working with the one picked last. The
//! error is quiet, and people blame themselves for it.
//!
//! EVERY pair is checked against the single list (`gui/assembly_tools.rs`): take the first, take the
//! second, and exactly one must be left in hand. The sweep is exhaustive, so a new tool lands here by
//! itself without editing this check.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::Id;

    fn two_parts(app: &mut App) -> Vec<Id> {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        app.mode_3d = true;
        app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect()
    }

    /// The tools and their doors, the same ones a person uses.
    const DOORS: [(&str, fn(&mut App)); 7] = [
        ("mate", |a: &mut App| a.arm_joint_pick_for_test()),
        ("anchor", |a: &mut App| a.start_conn_pick()),
        ("group", |a: &mut App| a.start_group_pick()),
        ("width", |a: &mut App| a.start_width_pick()),
        ("tangency", |a: &mut App| a.start_tangent_pick()),
        ("relation", |a: &mut App| a.start_relation_pick()),
        ("ground", |a: &mut App| a.start_ground_pick()),
    ];

    #[test]
    fn taking_a_tool_releases_the_previous_one() {
        let mut both: Vec<String> = Vec::new();
        for (first_name, first) in DOORS {
            for (second_name, second) in DOORS {
                if first_name == second_name {
                    continue; // the same door is a toggle rather than a change of tool
                }
                let mut app = App::default();
                let mine = two_parts(&mut app);
                assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
                app.workbench = super::super::Workbench::Assembly;

                first(&mut app);
                // GUARD AGAINST A VACUOUS CHECK: the first tool really was taken, otherwise there is no change to check.
                assert!(!app.armed_assembly_tools().is_empty(), "GUARD: \"{first_name}\" was not taken, so there is nothing to check the change on");
                second(&mut app);

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
