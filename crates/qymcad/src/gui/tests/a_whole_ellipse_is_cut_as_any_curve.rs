//! A WHOLE ELLIPSE IS CUT AS ANY CURVE: an ellipse 40 x 8 and a line across it through its centre make two regions,
//! and the extrusion picks one by a click and extrudes that half alone.
//!
//! Reported behaviour: neither a line nor a spline nor anything else crossing a whole ellipse cut it into closed
//! regions; the ellipse stayed one region of its own.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;

#[test]
fn a_half_of_an_ellipse_cut_by_a_line_is_extruded_alone() {
    let mut app = App::default();
    app.set.gpu_viewport = false;
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.project.add_ellipse_entity(si, Point2::new(0.0, 0.0), 40.0, 8.0, 0.0, Purpose::Real);
    app.project.add_line_entity(si, -50.0, 0.0, 50.0, 0.0, Purpose::Real);
    app.project.regen_sketch(si);
    app.finish_sketch_edit();
    let closed = qymcad_ui_state::sketch_closed_contours(&app.project, si);
    assert_eq!(closed.len(), 2, "the ellipse and the line across it make {} closed regions, not its two halves", closed.len());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    hand.tool(qymcad_ui_state::PartTool::Extrude);
    assert!(!hand.app.viewing.mode_3d, "the extrusion over two regions does not ask which one");
    hand.click2d(0.0, 4.0).key(egui::Key::Enter);
    hand.set("height", 10.0).enter();
    qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
    let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the extrusion made no body");
    let volume = app.project.mesh_index(body).map(|i| app.project.bodies[i].mesh.volume()).unwrap_or(0.0);
    let half = std::f64::consts::PI * 40.0 * 8.0 / 2.0 * 10.0;
    assert!((volume - half).abs() < 0.01 * half, "the body holds {volume:.1} mm^3, not the half of the ellipse, {half:.1}");
}
