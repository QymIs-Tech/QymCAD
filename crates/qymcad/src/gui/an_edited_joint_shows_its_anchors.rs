//! A JOINT BEING EDITED SHOWS WHERE ITS ANCHORS ARE.
//!
//! The highlight only lit up under what was being picked RIGHT NOW: the anchor fixed during the pick,
//! and whatever sits under the cursor. A joint opened for editing lit nothing at all — a person looks
//! at it, changes the axis and the offsets, and cannot see what it holds on to. Worse, the whole
//! highlight pass returned at the very start when no tool was in hand — and while editing, usually
//! none is.
//!
//! Reported behaviour: the highlight of the selected faces and of the selected axes disappeared.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::JointKind;
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// How many shapes the highlight pass draws, with no cursor over a part.
    fn shapes(app: &mut App) -> usize {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        app.refresh_edges();
        let mut count = 0;
        for _ in 0..2 {
            let out = ctx.run(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                egui::CentralPanel::default().show(c, |ui| {
                    let painter = ui.painter().clone();
                    app.draw_pick_highlights_for_test(&painter, viewport());
                });
            });
            count = out.shapes.len();
        }
        count
    }

    /// An assembly with a joint on two faces, with that joint opened for editing.
    fn a_joint_being_edited(app: &mut App) -> Id {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        app.mode_3d = true;
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
        let ctx = app.current_ctx_id_for_test();
        let aim: Vec<[f64; 3]> = mine
            .iter()
            .map(|b| {
                let wt = app.project.body_display_transform(*b, ctx);
                let f = app.project.regen_faces.get(b).and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z))).expect("the top face");
                qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z])
            })
            .collect();
        let mut hand = Hand::new(app);
        hand.look_at([30.0, 10.0, 5.0], 7.0).mate(JointKind::Slider).click(aim[0]).click(aim[1]);
        app.rebuild_if_dirty();
        app.project.joints.last().map(|j| j.id).expect("the joint was created")
    }

    #[test]
    fn its_anchors_are_lit_even_with_no_tool_in_hand() {
        let mut app = App::default();
        let jid = a_joint_being_edited(&mut app);

        // NO TOOL IN HAND — as it is when a person is simply editing an existing joint.
        app.drop_assembly_tools();
        app.joint.edit = None;
        let quiet = shapes(&mut app);

        app.joint.edit = Some(jid);
        let lit = shapes(&mut app);

        // GUARD AGAINST A VACUOUS CHECK: without editing, the highlight stays silent (the frame only
        // draws the panel background), otherwise there is nothing to compare against and a passing
        // test means nothing.
        assert!(quiet <= 1, "GUARD: without editing the highlight draws {quiet} shapes, so the difference proves nothing");
        assert!(
            lit > quiet,
            "the joint is open for editing and its anchors are not lit ({quiet} shapes before, {lit} after) — a person cannot see what it holds on to"
        );
    }
}
