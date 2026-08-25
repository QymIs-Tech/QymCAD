//! A CONNECTOR IS AN ELEMENT IN ITS OWN RIGHT, NOT A PART OF SOMEBODY'S JOINT.
//!
//! In grown-up CAD a mate connector stands in the tree on a par with a sketch: it is created IN
//! ADVANCE, any number of mates are placed on it, and it is edited and deleted separately. Here a
//! connector used to be born only inside a joint — that is, it could not be reused, and seeing it in
//! the list and fixing it was only possible through the joint that created it.
//!
//! Checked here is the whole path: create a connector on its own, see it in the list, place a joint on
//! it, try to delete it — and get a refusal in words.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::{AnchorRef, JointKind};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// The words drawn by the mates panel.
    fn panel_text(app: &mut App) -> Vec<String> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out = ctx.run(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                egui::CentralPanel::default().show(c, |ui| app.joints_panel_for_test(ui));
            });
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }
        texts
    }

    /// Two parts in the root; returns them.
    fn two_parts(app: &mut App) -> (Id, Id) {
        let before: Vec<Id> = app.project.components.iter().filter(|c| c.parent == Some(app.project.root)).map(|c| c.id).collect();
        for x in [0.0, 40.0] {
            super::super::joint_flow::tests::add_part_at(app, x);
        }
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        let mine: Vec<Id> = app
            .project
            .components
            .iter()
            .filter(|c| c.parent == Some(app.project.root) && !before.contains(&c.id))
            .map(|c| c.id)
            .collect();
        assert_eq!(mine.len(), 2, "setup: there should be two parts of our own, and there are {}", mine.len());
        (mine[0], mine[1])
    }

    /// A CONNECTOR MADE ON ITS OWN IS VISIBLE IN THE LIST AND OUTLIVES OTHER PEOPLE'S JOINTS.
    #[test]
    fn a_connector_made_on_its_own_shows_up_in_the_list() {
        let mut app = App::default();
        let (a, b) = two_parts(&mut app);
        let own = app.project.add_connector_standalone(a, AnchorRef::Origin);
        let name = app.project.connector(own).map(|c| crate::i18n::name(&c.name)).expect("the name of the connector");

        let texts = panel_text(&mut app);
        assert!(texts.iter().any(|t| t.contains(&name)), "the panel does not show the connector \"{name}\": drawn {texts:?}");

        // place and remove SOMEBODY ELSE'S joint — a standalone connector must survive
        let (ca, cb) = (app.project.add_connector(a, AnchorRef::Origin), app.project.add_connector(b, AnchorRef::Origin));
        let jid = app.project.add_joint(ca, cb, JointKind::Rigid);
        app.project.delete_joint(jid);
        assert!(app.project.connector(own).is_some(), "a standalone connector disappeared along with somebody else's joint");
        let texts = panel_text(&mut app);
        assert!(texts.iter().any(|t| t.contains(&name)), "the connector vanished from the list after somebody else's joint was deleted: {texts:?}");
    }

    /// CONNECTORS MADE FOR THE SAKE OF A JOINT DO NOT CLUTTER THE LIST.
    ///
    /// They are edited in the joint itself and need no row of their own: in an assembly with five
    /// joints the list would swell by ten rows nobody created.
    #[test]
    fn connectors_made_for_a_mate_do_not_clutter_the_list() {
        let mut app = App::default();
        let (a, b) = two_parts(&mut app);
        let (ca, cb) = (app.project.add_connector(a, AnchorRef::Origin), app.project.add_connector(b, AnchorRef::Origin));
        app.project.add_joint(ca, cb, JointKind::Rigid);
        let names: Vec<String> = [ca, cb].iter().filter_map(|c| app.project.connector(*c).map(|x| crate::i18n::name(&x.name))).collect();
        assert_eq!(names.len(), 2, "GUARD: there should be two names of joint connectors, and there are {}", names.len());

        let texts = panel_text(&mut app);
        for n in &names {
            assert!(!texts.iter().any(|t| t.contains(n)), "the joint connector \"{n}\" got into the connector list — it is cluttered with what nobody created: {texts:?}");
        }
    }

    /// A CONNECTOR A JOINT STANDS ON CANNOT BE DELETED, AND THE REFUSAL IS SAID IN WORDS.
    #[test]
    fn deleting_a_connector_a_mate_stands_on_is_refused_out_loud() {
        let mut app = App::default();
        let (a, b) = two_parts(&mut app);
        let (ca, cb) = (app.project.add_connector(a, AnchorRef::Origin), app.project.add_connector(b, AnchorRef::Origin));
        app.project.add_joint(ca, cb, JointKind::Rigid);

        app.delete_connector_asked_for_test(ca);
        assert!(app.project.connector(ca).is_some(), "a connector under a joint must survive");
        assert!(
            app.status.contains(&crate::i18n::tr1("j-conn-in-use", "n", "1")),
            "the refusal must be said in words, and the status line holds: {}",
            app.status
        );

        // and a free one is deleted
        let free = app.project.add_connector_standalone(a, AnchorRef::Origin);
        app.delete_connector_asked_for_test(free);
        assert!(app.project.connector(free).is_none(), "a free connector must be deleted");
    }

    /// THE CONNECTOR TOOL MAKES A STANDALONE ONE, AND NO JOINT APPEARS ALONG WITH IT.
    #[test]
    fn the_connector_tool_makes_a_standalone_one_and_no_mate() {
        let mut app = App::default();
        let (a, _) = two_parts(&mut app);
        let before = app.project.connectors.len();
        app.start_conn_pick_for_test();
        assert!(app.conn_pick_active_for_test(), "the connector tool was not taken up");
        app.joint_pick_origin_click_for_test(app.project.mesh_id(0).expect("the body of the first part"));
        let _ = a;

        assert_eq!(app.project.connectors.len(), before + 1, "the connector was not created");
        assert!(app.project.joints.is_empty(), "the connector tool created a JOINT, which nobody asked it for");
        let made = app.project.connectors.last().expect("the new connector");
        assert!(made.standalone, "a connector created on its own must be marked as standalone");
        assert!(!app.conn_pick_active_for_test(), "the tool must be released after the creation");
    }
}
