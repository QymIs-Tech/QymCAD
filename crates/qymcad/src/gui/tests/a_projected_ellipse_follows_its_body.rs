//! A PROJECTED ELLIPSE FOLLOWS ITS BODY, on the path of a person: an ellipse 10 by 5, its major half held by the parameter
//! a and its minor half by a dimension of 5, extruded by hand; on its top face the outline projected by the tool - one ellipse - and a line drawn by hand to a
//! place on it, its end held there. a made 14: the projected ellipse is 14 by 5, and the end of the line is on it still.
//!
//! Reported behaviour: the outline of the face came in as a polyline of 24 segments, and the end of the line stood on
//! one segment that a rebuild laid anew.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{Constraint, Param};

/// The base: the parameter a 10, an ellipse 10 by 5 about the origin, its major half held by a and its minor half by 5,
/// extruded 10 by hand.
/// Answers the body.
fn a_base_held_by_a(app: &mut App) -> u64 {
    app.project.parameters.push(Param { name: "a".into(), expr: "10".into(), value: 10.0 });
    let part = app.project.add_part("Part");
    app.enter_component(part);
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    let c = app.project.add_ellipse_entity(si, Point2::new(0.0, 0.0), 10.0, 5.0, 0.0, qymcad_core::feature::Purpose::Real);
    let el = app.project.sketches[si].entities.iter().find_map(|e| e.kind.as_ellipse()).expect("the ellipse");
    app.project.sketches[si].constraints.push(Constraint::Distance { a: c, b: el.ma, d: 10.0, off: 3.0, expr: "a".into(), driven: false, axis: 0, at: None });
    app.project.sketches[si].constraints.push(Constraint::Distance { a: c, b: el.mi, d: 5.0, off: 3.0, expr: String::new(), driven: false, axis: 0, at: None });
    app.project.eval_parameters();
    app.project.solve_sketch(si);
    app.finish_sketch_edit();
    app.chosen.sel = Sel::Sketch(si);
    Hand::new(app).look_at([0.0, 0.0, 5.0], 6.0).tool(qymcad_ui_state::PartTool::Extrude).set("height", 10.0).enter();
    app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the base")
}

/// The top face of `body` as a face to seat a sketch on.
fn top_of(app: &App, body: u64) -> qymcad_core::feature::FaceKey {
    let top = app.project.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).cloned().expect("the top");
    qymcad_core::feature::FaceKey { index: 0, centroid: [top.centroid.x, top.centroid.y, top.centroid.z], normal: top.normal, id: top.id }
}

/// The projected curves of sketch `si`.
fn projected(app: &App, si: usize) -> Vec<Curve> {
    let s = &app.project.sketches[si];
    let ids = s.projected_entities();
    s.entities.iter().filter(|e| ids.contains(&e.id)).filter_map(|e| Curve::of(s, e)).collect()
}

/// The semi-axes of an ellipse, the greater first.
fn semi_axes(c: &Curve) -> Option<[f64; 2]> {
    let Curve::Oval { major, minor, .. } = c else { return None };
    let (a, b) = (major.x.hypot(major.y), minor.x.hypot(minor.y));
    Some([a.max(b), a.min(b)])
}

#[test]
fn a_projected_ellipse_and_a_line_held_on_it_follow_the_parameter() {
    let mut app = App::default();
    let body = a_base_held_by_a(&mut app);
    let key = top_of(&app, body);
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::Face(body, key));
    let mut hand = Hand::new(&mut app);
    assert!(hand.press_hint(&crate::i18n::tr("tb-project-body-hint")) && hand.press_word(&crate::i18n::tr("opt-face-outline"), egui::pos2(0.0, 0.0)), "the projection tool and its Face outline");
    hand.click2d(0.0, 0.0).key(egui::Key::Escape);
    let axes: Vec<Option<[f64; 2]>> = projected(hand.app, si).iter().map(semi_axes).collect();
    assert!(matches!(axes[..], [Some([a, b])] if (a - 10.0).abs() < 1e-3 && (b - 5.0).abs() < 1e-3), "the outline of the top face came in as {axes:?}, not one ellipse 10 x 5");
    // a line from outside the face to the place of the ellipse at 60 deg round it
    let (x, y) = (10.0 * 60f64.to_radians().cos(), 5.0 * 60f64.to_radians().sin());
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(20.0, 15.0).click2d(x, y).key(egui::Key::Escape).key(egui::Key::Escape);
    let end = {
        let s = &hand.app.project.sketches[si];
        s.points.iter().filter(|q| (q.x - x).hypot(q.y - y) < 0.05).map(|q| q.id).find(|id| s.entities.iter().any(|e| e.kind.as_line().is_some_and(|l| l.b == *id)))
    };
    let end = end.expect("the end of the line drawn on the ellipse");
    hand.app.finish_sketch_edit();
    if let Some(p) = app.project.parameters.iter_mut().find(|p| p.name == "a") {
        p.expr = "14".into();
    }
    let mut hand = Hand::new(&mut app);
    hand.frame(Vec::new());
    let mut asks = Vec::new();
    crate::gui::panels_windows::apply_param_edit(&mut hand.app.win_ctx(&mut asks));
    hand.close_window();
    let curves = projected(&app, si);
    let axes: Vec<Option<[f64; 2]>> = curves.iter().map(semi_axes).collect();
    let s = &app.project.sketches[si];
    let at = s.points.iter().find(|q| q.id == end).map(|q| Point2::new(q.x, q.y)).expect("the end");
    let off = curves.first().map_or(f64::MAX, |c| c.nearest(at).off);
    assert!(
        matches!(axes[..], [Some([a, b])] if (a - 14.0).abs() < 1e-3 && (b - 5.0).abs() < 1e-3) && off < 1e-6,
        "a made 14: the projection is {axes:?}, not one ellipse 14 x 5; the end of the line stands {off:.2e} off it"
    );
}
