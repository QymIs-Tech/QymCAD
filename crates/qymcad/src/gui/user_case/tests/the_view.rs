use super::*;

/// THE VIEW AFTER A FAR LOOK: the whole assembly framed by the home button of the cube, the wheel down to a hole, a turn -
/// the hole stays in the frame. Reported behaviour: zoomed in from a far view and turned, the view "collapsed - everything
/// flew off into space".
pub(super) fn framed_wheeled_to_a_hole_and_turned(app: &mut App, problems: &mut Vec<String>) {
    let hole = app.project.timeline.iter().find_map(|n| match n.kind {
        qymcad_core::feature::FeatureKind::Hole { point, sketch: 0, .. } => {
            Some(qymcad_core::feature::apply12(&n.parent.map(|c| app.project.world_transform(c)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY), point))
        }
        _ => None,
    });
    let Some(hole) = hole else {
        problems.push("[the view] setup: there is no hole to zoom to".into());
        return;
    };
    let mut hand = Hand::new(app);
    hand.frame(Vec::new());
    let rect = hand.app.viewing.view_rect;
    let home = crate::gui::viewcube::home_rect(&hand.app.cube_ctx(), rect).center();
    hand.click_screen(home);
    std::thread::sleep(std::time::Duration::from_millis(300)); // the turn to the home view takes 220 ms of the clock
    hand.frame(Vec::new()).frame(Vec::new());
    let on_screen = |hand: &Hand| {
        let basis = hand.app.viewing.cam.basis();
        qymcad_ui_state::Screen { cam: &hand.app.viewing.cam, set: &hand.app.set, rect, basis: &basis }.at(hole).0
    };
    let over = on_screen(&hand);
    for _ in 0..40 {
        if rect.width() / hand.app.viewing.cam.scale <= 100.0 {
            break;
        }
        hand.wheel3d(over, 1);
    }
    let from = on_screen(&hand);
    hand.drag_view(crate::gui::hand::ViewDrag { button: egui::PointerButton::Secondary, modifiers: egui::Modifiers::NONE, from, to: from + egui::vec2(200.0, 0.0) });
    let after = on_screen(&hand);
    let across = rect.width() / hand.app.viewing.cam.scale;
    if !rect.contains(after) || across > 100.0 {
        problems.push(format!("[the view] framed, wheeled to the hole and turned: the hole stands at {after:?} of {rect:?}, {across:.0} mm across"));
    }
    check_all(app, "the view: framed, wheeled to a hole, turned", problems);
}
