//! THE MATE WINDOW STANDS STILL WHILE ITS MECHANISM MOVES.
//!
//! The window of a mate stands by the mate on screen, at the midpoint of its two anchors. Reported behaviour:
//! the window travelled up and down in step with a run of the mate. Running the travel of a cylindrical mate
//! carries anchor B along the axis, the midpoint goes with it, and the window went with the midpoint: 41 px over
//! one second of the run here, 21-25 px at four other camera positions.
//!
//! The window follows the view, not the motion it causes itself.
#[cfg(test)]
pub(in crate::gui) mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::JointKind;
    use qymcad_core::model::Id;

    /// A top corner of a body: `Far` at the largest x and y, `Near` at the smallest.
    #[derive(Clone, Copy)]
    enum Corner {
        Far,
        Near,
    }

    /// Where `which` top corner of `body` stands in the world.
    fn corner(app: &App, body: Id, which: Corner) -> [f64; 3] {
        let wt = app.project.body_display_transform(body, qymcad_ui_state::current_ctx_id(&app.active_path, &app.project));
        let mi = app.project.mesh_index(body).expect("the body has a mesh");
        let bb = app.project.bodies[mi].mesh.bounds().expect("the mesh has bounds");
        let p = match which {
            Corner::Far => [bb.max.x, bb.max.y, bb.max.z],
            Corner::Near => [bb.min.x, bb.min.y, bb.max.z],
        };
        qymcad_core::feature::apply12(&wt, p)
    }

    /// Where the glyph of mate `jid` stands on screen, the next frame drawn.
    pub(in crate::gui) fn glyph(hand: &mut Hand, jid: Id) -> Option<egui::Pos2> {
        hand.frame(Vec::new());
        let rect = hand.app.viewing.view_rect;
        qymcad_assembly::joint_glyphs(&mut hand.app.joint_ctx(), rect).into_iter().find(|(id, _, _)| *id == jid).map(|(_, at, _)| at)
    }

    /// TWO BOXES, ONE FIXED, A CYLINDRICAL MATE CLICKED CORNER TO CORNER, AND ITS WINDOW OPENED by a double click on
    /// the glyph. Answers the mate and where its glyph stood when the window was opened.
    pub(in crate::gui) fn a_cylindrical_mate_with_its_window_open(hand: &mut Hand) -> (Id, egui::Pos2) {
        let before: Vec<Id> = hand.app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(hand.app, 0.0);
        super::super::joint_flow::tests::add_part_at(hand.app, 60.0);
        let root = hand.app.project.root;
        hand.app.enter_component(root);
        qymcad_ui_state::rebuild_if_dirty(&mut hand.app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut hand.app.part_ctx());
        let mine: Vec<Id> = hand.app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
        let fixed = hand.app.project.body_owner(mine[0]).expect("the owner of the first body");
        hand.app.project.set_grounded(fixed, true);
        let (pa, pb) = (corner(hand.app, mine[0], Corner::Far), corner(hand.app, mine[1], Corner::Near));
        hand.look_at([30.0, 10.0, 5.0], 6.0).mate(JointKind::Cylindrical).anchor(2).click(pa).click(pb).key(egui::Key::Enter); // Enter applies the tool
        let jid = hand.app.project.joints.last().map(|j| j.id).expect("two clicks must create a mate");
        let at = glyph(hand, jid).expect("the glyph of the mate is on screen");
        hand.double_click_screen(at);
        assert_eq!(hand.app.side.joint.edit, Some(jid), "setup: a double click on the glyph opens the window of the mate");
        (jid, at)
    }

    /// How far apart the farthest two places of a list lie, in points.
    pub(in crate::gui) fn spread(places: &[egui::Pos2]) -> f32 {
        places.iter().flat_map(|a| places.iter().map(move |b| a.distance(*b))).fold(0.0, f32::max)
    }

    /// RUNNING THE TRAVEL LEAVES THE WINDOW WHERE IT STOOD.
    #[test]
    fn running_the_travel_leaves_the_mate_window_where_it_stood() {
        let mut app = App::default();
        let mut hand = Hand::new(&mut app);
        let (jid, at) = a_cylindrical_mate_with_its_window_open(&mut hand);
        assert!(hand.press_word(&crate::i18n::tr("j-anim-offset"), at), "the window of a cylindrical mate has a button to run its travel");
        let stop = crate::i18n::tr("j-anim-stop");
        let mut window = Vec::new();
        let mut glyphs = Vec::new();
        for _ in 0..60 {
            window.push(hand.written_at(&stop).expect("the run goes on: its stop button is on screen").min);
            glyphs.extend(glyph(&mut hand, jid));
        }
        // GUARD: the mate itself moved on screen, otherwise the window had nothing to follow
        assert!(spread(&glyphs) > 10.0, "GUARD: the glyph of the mate must travel with the run, and it travelled {:.1} px", spread(&glyphs));
        assert!(spread(&window) < 0.5, "the window of the mate must stand still while the mate runs, and it travelled {:.1} px", spread(&window));
    }

    /// THE WINDOW OF A RUNNING MATE GOES WITH THE VIEW: the view moved by the mouse while the mate runs carries the
    /// window as it carries a place of the scene that stands still. Reported behaviour: in a run of a cylindrical mate
    /// the window stood on the screen while the view moved, and the mate went from under it.
    #[test]
    fn the_window_of_a_running_mate_goes_with_the_view() {
        let mut app = App::default();
        let mut hand = Hand::new(&mut app);
        let (_, at) = a_cylindrical_mate_with_its_window_open(&mut hand);
        assert!(hand.press_word(&crate::i18n::tr("j-anim-offset"), at), "the window of a cylindrical mate has a button to run its travel");
        let still = [30.0, 10.0, 5.0];
        let window = egui::Id::new("joint_edit_popup");
        hand.frame(Vec::new());
        assert!(hand.app.joint_anim.is_some(), "the travel of the mate runs");
        let (window_was, still_was) = (hand.area_rect(window).expect("the window of the mate is drawn").min, hand.on_screen3d(still));
        let from = hand.app.viewing.view_rect.left_center() + egui::vec2(60.0, 0.0);
        let shift = super::super::hand::ViewDrag { button: egui::PointerButton::Secondary, modifiers: egui::Modifiers::SHIFT, from, to: from + egui::vec2(120.0, 40.0) };
        hand.drag_view(shift);
        assert!(hand.app.joint_anim.is_some(), "the travel of the mate runs on while the view moves");
        let (window_now, still_now) = (hand.area_rect(window).expect("the window of the mate is drawn").min, hand.on_screen3d(still));
        let (went, moved) = (still_now - still_was, window_now - window_was);
        assert!(went.length() > 20.0, "GUARD: the view must move, and a place of the scene went {went:?}");
        assert!((moved - went).length() < 6.0, "the window of a running mate stood apart from the view: the scene went {went:?}, the window {moved:?}");
    }
}
