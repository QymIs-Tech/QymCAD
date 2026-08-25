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
        app.rebuild_if_dirty();
        app.refresh_edges();
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 6.0;
        app.cam.target = [30.0, 10.0, 5.0];
        app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect()
    }

    /// A screen point OVER A PART: the centre of its top face.
    fn over_the_part(app: &App, body: Id) -> egui::Pos2 {
        let wt = app.project.body_display_transform(body, app.current_ctx_id_for_test());
        let f = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)))
            .expect("the body has faces");
        let w = qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z]);
        let basis = app.cam.basis();
        app.project3(w, viewport(), &basis).0
    }

    /// Everything by which a click can be seen not to have vanished.
    fn traces(app: &App) -> (usize, usize, usize, usize, bool, bool, bool, bool) {
        (
            app.project.connectors.len(),
            app.project.joints.len(),
            app.project.mate_constraints.len(),
            app.project.components.iter().filter(|c| c.grounded).count(),
            app.joint.pick_first.is_some(),
            app.joint.group_pick.as_ref().is_some_and(|v| !v.is_empty()),
            app.joint.width_pick.as_ref().is_some_and(|v| !v.is_empty()),
            app.joint.tangent_pick.as_ref().is_some_and(|v| !v.is_empty()),
        )
    }

    #[test]
    fn a_click_on_a_part_never_vanishes_without_a_word() {
        let tools: [(&str, fn(&mut App)); 6] = [
            ("mate", |a: &mut App| a.arm_joint_pick_for_test()),
            ("anchor", |a: &mut App| a.start_conn_pick()),
            ("group", |a: &mut App| a.start_group_pick()),
            ("width", |a: &mut App| a.start_width_pick()),
            ("tangency", |a: &mut App| a.start_tangent_pick()),
            ("ground", |a: &mut App| a.start_ground_pick()),
        ];
        let mut silent: Vec<String> = Vec::new();
        for (name, arm) in tools {
            let mut app = App::default();
            let mine = two_parts(&mut app);
            assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
            app.workbench = super::super::Workbench::Assembly;
            arm(&mut app);
            let at = over_the_part(&app, mine[0]);
            let said_before = app.status.clone();
            let was = traces(&app);

            // THE CLICK GOES THE SAME WAY A PERSON'S DOES: through the real viewport click handling.
            let basis = app.cam.basis();
            app.viewport_3d_click_at(at, viewport(), &basis);
            app.rebuild_if_dirty();

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
