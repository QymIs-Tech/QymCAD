//! THE ARROW OF AN EXTRUDE IS DRAGGED WITH THE MOUSE, in whole frames.
//!
//! Reported behaviour: while the field of the value holds the caret, the arrow in the viewport does not work; after a
//! flip the arrow cannot be grabbed; with "two sides" no second arrow appears on the other side.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;

    /// A part with a 60 x 40 rectangle on XY, and the extrude in hand on it: the field of the height holds the caret,
    /// as it does when the tool opens.
    fn extrude_in_hand() -> App {
        let mut app = App::default();
        let part = app.project.add_part("Plate");
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(60.0, 40.0);
        app.finish_sketch_edit();
        app.chosen.sel = super::super::Sel::Sketch(si);
        let mut h = Hand::new(&mut app);
        h.look_at([30.0, 20.0, 10.0], 5.0).tool(qymcad_ui_state::PartTool::Extrude).set("height", 10.0);
        for _ in 0..3 {
            h.frame(Vec::new());
        }
        app
    }

    fn value(app: &App, key: &str) -> f64 {
        qymcad_ui_state::cmd_val(&app.tools.cmd, key)
    }

    /// Where point `p` of the scene is on the screen, through the canvas the last frame laid out.
    fn on_screen(app: &App, p: [f64; 3]) -> egui::Pos2 {
        let basis = app.viewing.cam.basis();
        qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at(p).0
    }

    /// SETUP: the arrow as the tool opens, nothing else touched, is dragged - otherwise the checks below are red for
    /// the hand, not for the program.
    #[test]
    fn the_arrow_is_dragged() {
        let mut app = extrude_in_hand();
        let mut h = Hand::new(&mut app);
        h.drag3d([30.0, 20.0, 10.0], [30.0, 20.0, 25.0]);
        let now = value(h.app, "height");
        assert!((now - 25.0).abs() < 2.0, "the arrow dragged from 10 to 25 left the height at {now}");
    }

    /// WITH THE CARET IN THE FIELD the arrow is dragged all the same, and the field follows it.
    #[test]
    fn the_arrow_is_dragged_while_the_field_holds_the_caret() {
        let mut app = extrude_in_hand();
        let mut h = Hand::new(&mut app);
        h.frame(Vec::new());
        let tip = on_screen(h.app, [30.0, 20.0, 10.0]);
        assert!(h.press_word("10", tip), "setup: the field of the height is not beside the arrow");
        h.frame(Vec::new());
        assert!(h.typing(), "setup: a click in the field of the height did not put the caret in it");
        // a person types a value of their own first: the caret at the end, the old digits out, 12 in
        h.key(egui::Key::End).key(egui::Key::Backspace).key(egui::Key::Backspace).type_text("12").frame(Vec::new());
        assert!((value(h.app, "height") - 12.0).abs() < 1e-9, "setup: 12 typed into the field did not take: {}", value(h.app, "height"));
        h.drag3d([30.0, 20.0, 12.0], [30.0, 20.0, 25.0]);
        let now = value(h.app, "height");
        assert!((now - 25.0).abs() < 2.0, "the arrow dragged from 12 to 25 after typing into the field left the height at {now}");
        let shown = h.app.tools.cmd.params.iter().find(|p| p.key == "height").map(|p| p.txt.clone()).unwrap_or_default();
        assert!((shown.trim().parse::<f64>().unwrap_or(f64::NAN) - now).abs() < 0.01, "the field shows {shown:?} while the arrow says {now}");
    }

    /// AFTER A FLIP the arrow stands on the other side, and is grabbed there.
    #[test]
    fn the_arrow_is_grabbed_after_a_flip() {
        let mut app = extrude_in_hand();
        let mut h = Hand::new(&mut app);
        let flip = crate::i18n::tr("cmd-flip-btn");
        assert!(h.press_word(&flip, egui::pos2(0.0, 0.0)), "setup: no flip in the bar");
        h.frame(Vec::new());
        assert!(h.app.feat.flip, "setup: the flip did not take");
        h.drag3d([30.0, 20.0, -10.0], [30.0, 20.0, -25.0]);
        let now = value(h.app, "height");
        assert!((now - 25.0).abs() < 2.0 && h.app.feat.flip, "the flipped arrow dragged from 10 to 25 left the height at {now} (flip {})", h.app.feat.flip);
    }

    /// TWO SIDES: the second side has an arrow of its own, pointing the other way, and it drives the second value.
    #[test]
    fn two_sides_have_two_arrows() {
        let mut app = extrude_in_hand();
        let mut h = Hand::new(&mut app);
        let two = crate::i18n::tr("cmd-two-sides");
        assert!(h.press_word(&two, egui::pos2(0.0, 0.0)), "setup: no two sides in the bar");
        h.frame(Vec::new());
        h.set("down", 8.0).frame(Vec::new());
        let height = value(h.app, "height");
        h.drag3d([30.0, 20.0, -8.0], [30.0, 20.0, -20.0]);
        let (down, now) = (value(h.app, "down"), value(h.app, "height"));
        assert!((down - 20.0).abs() < 2.0, "the second arrow dragged from 8 to 20 left the second side at {down}");
        assert!((now - height).abs() < 1e-9 && !h.app.feat.flip, "the second arrow moved the first side: the height went {height} -> {now}, flip {}", h.app.feat.flip);
    }

    /// THE ARROW AT A FACE (pushing a face, as thickening and splitting use it) is dragged after a value was typed into
    /// its field, the same as the arrow of an extrude.
    #[test]
    fn the_face_arrow_is_dragged_after_typing_into_its_field() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let body = app.project.mesh_id(0).expect("the block");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let mut h = Hand::new(&mut app);
        h.look_at([10.0, 10.0, 5.0], 6.0).frame(Vec::new());
        h.tool(qymcad_ui_state::PartTool::PushFace).click([10.0, 10.0, 10.0]);
        for _ in 0..3 {
            h.frame(Vec::new());
        }
        assert!(!h.app.tools.gsel.faces.is_empty(), "setup: the top face picked before the tool was not taken: {}", h.app.status);
        let shown = h.app.tools.cmd.params.iter().find(|p| p.key == "dist").map(|p| p.txt.clone()).unwrap_or_default();
        let (o, tip, _) = qymcad_ui_state::face_arrow_geometry(&h.app.painting()).expect("setup: the face has no arrow");
        let near = on_screen(h.app, tip);
        assert!(h.press_word(shown.trim(), near), "setup: the field of the distance ({shown:?}) is not beside the arrow");
        h.frame(Vec::new());
        assert!(h.typing(), "setup: a click in the field of the distance did not put the caret in it");
        h.key(egui::Key::End);
        for _ in 0..shown.trim().chars().count() {
            h.key(egui::Key::Backspace);
        }
        h.type_text("3").frame(Vec::new());
        assert!((value(h.app, "dist") - 3.0).abs() < 1e-9, "setup: 3 typed into the field did not take: {}", value(h.app, "dist"));
        h.drag3d([o[0], o[1], o[2] + 3.0], [o[0], o[1], o[2] + 10.0]);
        let now = value(h.app, "dist");
        assert!((now - 10.0).abs() < 2.0, "the arrow at the face dragged from 3 to 10 after typing into the field left the distance at {now}");
    }
}
