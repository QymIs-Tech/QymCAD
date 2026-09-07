//! A CLICK ON THE VIEWPORT EITHER DOES SOMETHING OR SAYS WHY NOT.
//!
//! The worst thing a CAD can do is silently nothing: a person cannot tell "the program did not
//! understand me" from "I clicked the wrong place", and starts clicking at random. The workbench
//! suffered from this for a long time — joints without anchors were silently dropped from the
//! computation, the highlight stayed silent in half the tools, an anchor on a moving part was
//! silently created and tore the assembly apart.
//!
//! The guard checks them all at once and BY FACT: a tool is in hand, a person clicked ON A PART, and
//! after that either the document changed, or the pick state changed, or the program SAID something
//! new. None of the three means the click vanished and the person never learned of it.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    fn two_parts(app: &mut App) -> Vec<Id> {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        let root = app.project.root;
        app.enter_component(root);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        app.viewing.cam.scale = 6.0;
        app.viewing.cam.target = [30.0, 10.0, 5.0];
        app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect()
    }

    /// A screen point OVER A PART: the centre of its top face.
    fn over_the_part(app: &App, body: Id) -> egui::Pos2 {
        let wt = app.project.body_display_transform(body, qymcad_ui_state::current_ctx_id(&app.active_path, &app.project));
        let f = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)))
            .expect("the body has faces");
        let w = qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z]);
        let basis = app.viewing.cam.basis();
        qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(w).0
    }

    /// Everything by which a click can be seen not to have vanished.
    fn traces(app: &App) -> (usize, usize, usize, usize, bool, bool, bool, bool) {
        (
            app.project.connectors.len(),
            app.project.joints.len(),
            app.project.mate_constraints.len(),
            app.project.components.iter().filter(|c| c.grounded).count(),
            app.side.joint.pick_first.is_some(),
            app.side.joint.group_pick.as_ref().is_some_and(|v| !v.is_empty()),
            app.side.joint.width_pick.as_ref().is_some_and(|v| !v.is_empty()),
            app.side.joint.tangent_pick.as_ref().is_some_and(|v| !v.is_empty()),
        )
    }

    #[test]
    fn a_click_on_a_part_never_vanishes_without_a_word() {
        // EVERY TOOL THERE IS, from the shared door table (`assembly_tools::doors`). It used to be a
        // hand-written subset here - and each of the four checks of this kind had its OWN subset, of seven,
        // seven, five and six, with no reason written for what was left out.
        let mut silent: Vec<String> = Vec::new();
        // ONLY THE TOOLS THAT ASK FOR GEOMETRY, by `AssemblyTool::wants_geometry`. A click on a part
        // while the relation tool is armed does nothing on purpose: that tool wants the mates in the list,
        // and it says so in the status line the moment it is taken.
        for t in super::super::assembly_tools::AssemblyTool::ALL.into_iter().filter(|t| t.wants_geometry()) {
            let name = t.help_mode();
            let mut app = App::default();
            let mine = two_parts(&mut app);
            assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
            app.workbench = super::super::Workbench::Assembly;
            crate::gui::assembly_tools::doors::arm(&mut app, t);
            let at = over_the_part(&app, mine[0]);
            let said_before = app.status.clone();
            let was = traces(&app);

            // THE CLICK GOES THE SAME WAY A PERSON'S DOES: through the real viewport click handling.
            let basis = app.viewing.cam.basis();
            app.viewport_3d_click_at(at, viewport(), &basis);
            qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());

            let now = traces(&app);
            let did = now != was;
            let said = app.status != said_before && !app.status.trim().is_empty();
            if !did && !said {
                silent.push(format!("\"{name}\": a part was clicked, nothing changed and not a word was said (it was \"{said_before}\")"));
            }
        }
        assert!(
            silent.is_empty(),
            "the click vanished and the person never learned of it:\n{}",
            silent.join("\n")
        );
    }
}
