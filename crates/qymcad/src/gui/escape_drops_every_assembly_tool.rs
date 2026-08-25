//! ESC DROPS ANY ASSEMBLY TOOL AND LEAVES NO LEFTOVERS.
//!
//! There is a guard on Esc in the program (`behaviour_sweep`), but the list of "what stayed switched
//! on" is named there ITEM BY ITEM and knows only two of the nine states of the assembly workbench.
//! Exactly that illness — listing the modes by hand — has already produced a whole class of troubles:
//! first the highlight stayed silent while pointing at an axis, then four more blind tools were
//! found.
//!
//! Here the list is complete, and it is checked BY FACT: every tool is taken up by its own door, Esc
//! is pressed — and not one state of the selection has the right to remain. A tool that will not be
//! released is worse than one that will not start: a person is certain they left it, and the next
//! click goes somewhere else.
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

    /// WHAT STAYED SWITCHED ON IS ASKED OF THE SINGLE LIST (`gui/assembly_tools.rs`).
    ///
    /// There used to be a list of its own here, and that was the whole trouble: lists in different
    /// places drift apart. Now the check and the program read ONE, and a new tool gets here by
    /// itself.
    fn leftovers(app: &App) -> Vec<&'static str> {
        use super::super::assembly_tools::AssemblyTool as T;
        // THE NAMES LIVE HERE AND NOT IN THE CODE: they are for whoever reads a failed run, not for
        // the person at the screen, and they have no business in the language catalogue.
        app.armed_assembly_tools()
            .into_iter()
            .map(|t| match t {
                T::Mate => "mate",
                T::Anchor => "connector",
                T::Group => "group",
                T::Width => "width",
                T::Tangent => "tangent",
                T::Relation => "relation",
                T::Ground => "ground",
                T::Axis => "pointing at an axis",
                T::Repick => "re-picking an anchor",
            })
            .collect()
    }

    #[test]
    fn escape_leaves_no_assembly_tool_running() {
        let tools: [(&str, fn(&mut App)); 7] = [
            ("mate", |a: &mut App| a.arm_joint_pick_for_test()),
            ("connector", |a: &mut App| a.start_conn_pick()),
            ("group", |a: &mut App| a.start_group_pick()),
            ("width", |a: &mut App| a.start_width_pick()),
            ("tangent", |a: &mut App| a.start_tangent_pick()),
            ("relation", |a: &mut App| a.start_relation_pick()),
            ("ground", |a: &mut App| a.start_ground_pick()),
        ];
        let mut stuck: Vec<String> = Vec::new();
        for (name, arm) in tools {
            let mut app = App::default();
            let mine = two_parts(&mut app);
            assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
            app.workbench = super::super::Workbench::Assembly;
            arm(&mut app);
            // TRAP GUARD: the tool really was taken up, otherwise there is nothing to put down and
            // a green result is empty.
            assert!(!leftovers(&app).is_empty(), "GUARD: the \"{name}\" tool was not taken up — Esc has nothing to put down");

            app.on_escape();
            let left = leftovers(&app);
            if !left.is_empty() {
                stuck.push(format!("\"{name}\": left over after Esc — {}", left.join(", ")));
            }
        }
        assert!(
            stuck.is_empty(),
            "a tool is not released by Esc, and the next click will go somewhere else:\n{}",
            stuck.join("\n")
        );
    }
}
