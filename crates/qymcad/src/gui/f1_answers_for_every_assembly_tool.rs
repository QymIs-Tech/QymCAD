//! F1 ANSWERS ABOUT THE TOOL IN HAND.
//!
//! Context help rested on a hand-written enumeration and knew ONE Assembly tool out of nine: the mate
//! pick. Holding an anchor, a group, a width, a tangency, a relation, ground, an axis pick or a
//! re-pick, a person pressing F1 got the table of contents — an answer of "go find it yourself",
//! exactly at the minute they were stuck.
//!
//! This is the same disease that already produced blind highlighting and stuck tools: a list living
//! apart from the single source. So the check walks `AssemblyTool::ALL` rather than a list of its own,
//! and a new tool lands here by itself.
#[cfg(test)]
mod tests {
    use super::super::assembly_tools::AssemblyTool;
    use super::super::App;
    use qymcad_core::model::Id;

    /// A JOINT WITH SOMETHING TO EDIT: the axis pick and the anchor re-pick exist only alongside one.
    fn a_joint(app: &mut App) -> (Id, Id) {
        let ([ja, _jb], _wheels) = super::super::a_relation_is_made_by_hand::tests::two_hinges(app);
        let ca = app.project.joints.iter().find(|j| j.id == ja).map(|j| j.a).expect("the joint has an anchor");
        (ja, ca)
    }

    /// Take a tool through its own door.
    ///
    /// The axis pick and the re-pick have no door at all: they are switched on by a button IN THE
    /// POPUP of an existing joint, and outside that popup they cannot be taken. So the same fields
    /// the popup sets are used here.
    fn arm(app: &mut App, t: AssemblyTool) {
        match t {
            AssemblyTool::Mate => app.arm_joint_pick_for_test(),
            AssemblyTool::Anchor => app.start_conn_pick(),
            AssemblyTool::Group => app.start_group_pick(),
            AssemblyTool::Width => app.start_width_pick(),
            AssemblyTool::Tangent => app.start_tangent_pick(),
            AssemblyTool::Relation => app.start_relation_pick(),
            AssemblyTool::Ground => app.start_ground_pick(),
            AssemblyTool::Axis => {
                let (_, ca) = a_joint(app);
                app.joint.axis_pick = Some(ca);
            }
            AssemblyTool::Repick => {
                let (jid, _) = a_joint(app);
                app.joint.edit_repick = Some((jid, false));
            }
        }
    }

    /// The help articles present on disk (both languages must have the same one).
    fn article_exists(path: &str) -> bool {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/help");
        ["ru", "en"].iter().all(|lang| root.join(lang).join(format!("{path}.md")).exists())
    }

    #[test]
    fn every_assembly_tool_has_its_own_article() {
        let mut sins: Vec<String> = Vec::new();
        for t in AssemblyTool::ALL {
            let mode = t.help_mode();
            let Some(article) = crate::help_map::assembly_article(mode) else {
                sins.push(format!("{mode}: no article in the F1 table, so help will answer with the table of contents"));
                continue;
            };
            if !article_exists(article) {
                sins.push(format!("{mode}: the table points at \"{article}\", and no language has such an article"));
            }
        }
        assert!(sins.is_empty(), "an Assembly tool with no help ({}):\n{}", sins.len(), sins.join("\n"));
    }

    #[test]
    fn f1_answers_about_the_tool_in_hand() {
        let index = crate::help_map::workbench_article("assembly");
        let mut sins: Vec<String> = Vec::new();
        for t in AssemblyTool::ALL {
            let mut app = App::default();
            app.workbench = super::super::Workbench::Assembly;
            arm(&mut app, t);
            // GUARD AGAINST A VACUOUS CHECK: the tool really was taken, otherwise a table-of-contents answer would be legitimate.
            if !app.armed_assembly_tools().contains(&t) {
                sins.push(format!("{:?}: the tool was not taken, so there is nothing to check", t));
                continue;
            }
            let got = app.help_for_context();
            if got == index {
                sins.push(format!("{:?}: F1 answered with the table of contents \"{got}\" instead of an article about the tool", t));
            }
        }
        assert!(sins.is_empty(), "F1 does not answer about the tool in hand ({}):\n{}", sins.len(), sins.join("\n"));
    }
}
