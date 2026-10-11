use egui::{Pos2, Rect, Vec2};
use qymcad_core::feature::Purpose;
use qymcad_core::model::Project;
use qymcad_ui_state::{PickCtx, Settings, View2d, slot_contour_under_2d};

#[test]
fn clicking_near_inner_contour_boundary_selects_inner_contour() {
    let mut project = Project::default();
    project.new_document();
    let si = project.new_sketch("S");
    let sid = project.sketches[si].id;

    // Outer rectangle: 40x40 mm centered at (0, 0)
    project.add_line_entity(si, -20.0, -20.0, 20.0, -20.0, Purpose::Real);
    project.add_line_entity(si, 20.0, -20.0, 20.0, 20.0, Purpose::Real);
    project.add_line_entity(si, 20.0, 20.0, -20.0, 20.0, Purpose::Real);
    project.add_line_entity(si, -20.0, 20.0, -20.0, -20.0, Purpose::Real);

    // Inner rectangle: 10x10 mm centered at (0, 0)
    project.add_line_entity(si, -5.0, -5.0, 5.0, -5.0, Purpose::Real);
    project.add_line_entity(si, 5.0, -5.0, 5.0, 5.0, Purpose::Real);
    project.add_line_entity(si, 5.0, 5.0, -5.0, 5.0, Purpose::Real);
    project.add_line_entity(si, -5.0, 5.0, -5.0, -5.0, Purpose::Real);

    let cands = project.sweep_profile_contours(sid);
    assert_eq!(cands.len(), 2, "setup: two closed contours are found");

    // The inner contour has smaller area
    let ci0 = project.contour_index(cands[0]).unwrap();
    let ci1 = project.contour_index(cands[1]).unwrap();
    let (outer_cid, inner_cid) = if project.contours[ci0].area() > project.contours[ci1].area() { (cands[0], cands[1]) } else { (cands[1], cands[0]) };

    let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
    let view = View2d {
        center: Vec2::ZERO,
        scale: 10.0, // 1 mm = 10 px
        initialized: true,
        ..Default::default()
    };
    let set = Settings::default();
    let pick = PickCtx { project: &project, set: &set, view: &view };

    // Center is (400, 300).
    // Inner rectangle top edge is at y = 5 mm -> screen Y = 300 - 5 * 10 = 250 px.
    // Click at screen Y = 248 px (2 px above inner edge, inside outer rect):
    let click_pos = Pos2::new(400.0, 248.0);
    let picked = slot_contour_under_2d(&pick, rect, click_pos, &cands);

    assert_eq!(picked, Some(inner_cid), "clicking 2px from the inner contour edge must pick the inner contour, not the outer one (outer was {outer_cid})");
}

#[test]
fn clicking_near_inner_contour_boundary_in_3d_selects_inner_contour() {
    let mut project = Project::default();
    project.new_document();
    let si = project.new_sketch("S");

    // Outer rectangle: 40x40 mm centered at (0, 0)
    project.add_line_entity(si, -20.0, -20.0, 20.0, -20.0, Purpose::Real);
    project.add_line_entity(si, 20.0, -20.0, 20.0, 20.0, Purpose::Real);
    project.add_line_entity(si, 20.0, 20.0, -20.0, 20.0, Purpose::Real);
    project.add_line_entity(si, -20.0, 20.0, -20.0, -20.0, Purpose::Real);

    // Inner rectangle: 10x10 mm centered at (0, 0)
    project.add_line_entity(si, -5.0, -5.0, 5.0, -5.0, Purpose::Real);
    project.add_line_entity(si, 5.0, -5.0, 5.0, 5.0, Purpose::Real);
    project.add_line_entity(si, 5.0, 5.0, -5.0, 5.0, Purpose::Real);
    project.add_line_entity(si, -5.0, 5.0, -5.0, -5.0, Purpose::Real);

    let sid = project.sketches[si].id;
    let cands = project.sweep_profile_contours(sid);
    let ci0 = project.contour_index(cands[0]).unwrap();
    let ci1 = project.contour_index(cands[1]).unwrap();
    let (outer_cid, inner_cid) = if project.contours[ci0].area() > project.contours[ci1].area() { (cands[0], cands[1]) } else { (cands[1], cands[0]) };

    let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
    let cam = qymcad_ui_state::Cam3 {
        target: [0.0, 0.0, 0.0],
        yaw: 0.0,
        pitch: -std::f64::consts::FRAC_PI_2,
        scale: 10.0, // 1 mm = 10 px
        init: true,
        ..Default::default()
    };
    let set = Settings::default();
    let basis = cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &cam, set: &set, rect, basis: &basis };

    // Center is (400, 300).
    // Sketch plane default is XY. Inner top edge is at y = 5 -> screen Y = 250 px.
    let click_pos = Pos2::new(400.0, 248.0);
    let picked = qymcad_ui_state::contour_under_3d(&project, &scr, click_pos, si);

    assert_eq!(picked, Some(inner_cid), "in 3D, clicking 2px from inner contour edge must pick inner contour, not outer ({outer_cid})");
}
