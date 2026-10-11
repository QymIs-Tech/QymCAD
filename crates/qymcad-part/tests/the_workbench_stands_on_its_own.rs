//! THE PART WORKBENCH, CHECKED WITHOUT THE APPLICATION.
//!
//! Until now every check of this crate was a check of the application: the workbench takes a context, a
//! context is a bundle of borrows, and only `App` owned the records the borrows point at. So verifying
//! three lines of arithmetic meant building a window and running frames, and the crate could not be
//! verified on its own at all - which is the one thing splitting it out was supposed to buy.
//!
//! `Bench` owns the same records and ties the same context. Nothing here is a mock: the document,
//! the command, the selection are the real ones.

use qymcad_ui_state::{Armed, Bench};

/// OPENING A COMMAND AND CANCELLING IT LEAVES NOTHING IN HAND.
///
/// The rule the whole `Armed` rework is about, checked at the level it lives on rather than through a
/// window: the command is one field, and cancelling puts it back to nothing.
#[test]
fn cancelling_a_command_leaves_the_hand_empty() {
    let mut b = Bench::default();
    b.cmd.open(&mut b.armed, 1, false); // extrude
    assert_eq!(b.armed, Armed::Command(1), "opening a command must put it in hand");

    qymcad_ui_state::cancel_feat_cmd(&mut b.part_ctx());
    assert_eq!(b.armed, Armed::None, "cancelling must leave nothing in hand");
}

/// THE COMMAND'S NAME COMES FROM WHAT IS IN HAND, not from a field of its own.
///
/// Checked by swapping ONLY the hand: `feat` is untouched between the two readings, so a name that
/// changes can have come from nowhere else. Asking instead what an EMPTY hand is called would prove
/// nothing - `feat_cmd_name` has one caller, `apply_feat_cmd`, which names the undo step of a command
/// already in hand, so an empty hand never reaches it and its `f-operation` fallback stands for an
/// unknown KIND rather than for no command at all.
#[test]
fn the_command_names_itself_from_the_hand() {
    let mut b = Bench::default();

    b.cmd.open(&mut b.armed, 1, false); // extrude
    let extrude = qymcad_part::feat_cmd_name(&b.armed, b.feat);

    b.cmd.open(&mut b.armed, 4, false); // fillet
    let fillet = qymcad_part::feat_cmd_name(&b.armed, b.feat);

    assert!(!extrude.is_empty() && !fillet.is_empty(), "a command in hand must have a name to show");
    assert_ne!(extrude, fillet, "the name must follow the hand: same `feat`, different command, same name means it does not");
}

/// A CLICK IN 3D UNDER LOFT PICKS THE CONTOUR OF THE SECTION UNDER THE CURSOR.
#[test]
fn clicking_loft_contour_in_3d_updates_section_contour() {
    use egui::{Pos2, Rect, Vec2};
    use qymcad_core::feature::Purpose;

    let mut b = Bench::default();
    b.project.new_document();
    let s1 = b.project.new_sketch("S1");
    let s2 = b.project.new_sketch("S2");
    let s1_id = b.project.sketches[s1].id;
    let s2_id = b.project.sketches[s2].id;

    // Sketch 1: outer rect (-20..20) and inner rect (-5..5)
    b.project.add_line_entity(s1, -20.0, -20.0, 20.0, -20.0, Purpose::Real);
    b.project.add_line_entity(s1, 20.0, -20.0, 20.0, 20.0, Purpose::Real);
    b.project.add_line_entity(s1, 20.0, 20.0, -20.0, 20.0, Purpose::Real);
    b.project.add_line_entity(s1, -20.0, 20.0, -20.0, -20.0, Purpose::Real);

    b.project.add_line_entity(s1, -5.0, -5.0, 5.0, -5.0, Purpose::Real);
    b.project.add_line_entity(s1, 5.0, -5.0, 5.0, 5.0, Purpose::Real);
    b.project.add_line_entity(s1, 5.0, 5.0, -5.0, 5.0, Purpose::Real);
    b.project.add_line_entity(s1, -5.0, 5.0, -5.0, -5.0, Purpose::Real);

    // Sketch 2: just outer rect
    b.project.add_line_entity(s2, -30.0, -30.0, 30.0, -30.0, Purpose::Real);
    b.project.add_line_entity(s2, 30.0, -30.0, 30.0, 30.0, Purpose::Real);
    b.project.add_line_entity(s2, 30.0, 30.0, -30.0, 30.0, Purpose::Real);
    b.project.add_line_entity(s2, -30.0, 30.0, -30.0, -30.0, Purpose::Real);

    let s1_cands = b.project.sweep_profile_contours(s1_id);
    let s1_ci0 = b.project.contour_index(s1_cands[0]).unwrap();
    let s1_ci1 = b.project.contour_index(s1_cands[1]).unwrap();
    let (outer_cid, inner_cid) = if b.project.contours[s1_ci0].area() > b.project.contours[s1_ci1].area() { (s1_cands[0], s1_cands[1]) } else { (s1_cands[1], s1_cands[0]) };

    // Arm Loft with 2 sections
    b.cmd.open(&mut b.armed, 9, true);
    b.loft.sids = vec![s1_id, s2_id];
    b.loft.cids = vec![outer_cid, 0];

    b.cam.target = [0.0, 0.0, 0.0];
    b.cam.yaw = 0.0;
    b.cam.pitch = -std::f64::consts::FRAC_PI_2;
    b.cam.scale = 10.0;
    b.cam.init = true;

    let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
    // Click 2px from the inner rectangle edge of S1
    let click_pos = Pos2::new(400.0, 248.0);
    qymcad_part::loft_contour_click_3d(&mut b.part_ctx(), rect, click_pos);

    assert_eq!(b.loft.cids[0], inner_cid, "clicking near inner contour in 3D updates section 1 to inner contour");
}
