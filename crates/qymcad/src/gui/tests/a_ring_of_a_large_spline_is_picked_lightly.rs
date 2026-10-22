//! A RING OF A LARGE SPLINE IS PICKED LIGHTLY, on the path of the report: a closed spline of 40 nodes some 500 mm
//! across drawn by hand, offset 5 inwards by the offset tool, the sketch left, Extrude taken, the pointer led over 20
//! places of the ring between the two curves and a click on one of them - the hover keeps up with the pointer, and the
//! click takes the ring.
//!
//! Reported behaviour: over such a ring the program hung at the choice of a closed region, and the region taken was not
//! the one under the pointer. Measured on a debug build: 20 places hovered in 45 s, a click 3.8 s, every frame laying the
//! region under the pointer out in triangles afresh; 0.55 s and 26 ms since.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::time_budget::{cost_of, over, Doer};
use std::time::Duration;

/// The nodes of the spline: 40 places round a circle of 200, waved by 40 five times and by 10 nine times.
fn nodes() -> Vec<(f64, f64)> {
    (0..40)
        .map(|k| {
            let t = std::f64::consts::TAU * k as f64 / 40.0;
            let r = 200.0 + 40.0 * (5.0 * t).sin() + 10.0 * (9.0 * t).cos();
            (r * t.cos(), r * t.sin())
        })
        .collect()
}

#[test]
fn the_ring_between_a_large_spline_and_its_offset_is_hovered_and_taken_lightly() {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    let nodes = nodes();
    hand.sk_tool(qymcad_ui_state::SketchShape::Spline);
    for (x, y) in &nodes {
        hand.click2d(*x, *y);
    }
    hand.click2d(nodes[0].0, nodes[0].1);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let s = &hand.app.project.sketches[si];
    let spline = s.entities.iter().find(|e| e.kind.as_spline().is_some_and(|sp| sp.ends == qymcad_core::feature::Ends::Closed)).map(|e| e.id).expect("setup: the spline was not closed");
    let on_it = s.entity(spline).and_then(|e| Curve::of(s, e)).map(|c| c.at(0.01)).expect("the spline");
    hand.sk_select().click2d(on_it.x, on_it.y);
    assert!(hand.press_hint(&crate::i18n::tr("tb-offset-hint")), "no sketch offset button");
    let laid = cost_of(Doer::Thread, || {
        hand.click2d(0.0, 0.0);
        hand.type_text("5").key(egui::Key::Enter);
    });
    eprintln!("the offset laid: {}", laid.cost);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    // 20 places of the ring: 2.5 off the spline, on the side of its copy
    let s = &hand.app.project.sketches[si];
    let source = s.entity(spline).and_then(|e| Curve::of(s, e)).expect("the spline");
    let copies: Vec<Curve> = s.entities.iter().filter(|e| e.id != spline).filter_map(|e| Curve::of(s, e)).collect();
    assert!(!copies.is_empty(), "setup: the spline was not offset; the status line says {:?}", hand.app.status);
    let near_copy = |q: Point2| copies.iter().map(|c| c.nearest(q).off).fold(f64::MAX, f64::min);
    let places: Vec<Point2> = (0..20)
        .map(|k| {
            let t = k as f64 / 20.0;
            let (q, d) = (source.at(t), source.tangent(t));
            let n = Point2::new(-d.y, d.x) * 2.5;
            if near_copy(q + n) < near_copy(q - n) {
                q + n
            } else {
                q - n
            }
        })
        .collect();
    let outer = hand.app.project.sketches[si]
        .contour_ids
        .iter()
        .copied()
        .filter_map(|id| hand.app.project.contour_index(id).map(|i| (id, hand.app.project.contours[i].area().abs())))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id);
    hand.key(egui::Key::Escape).ctrl(egui::Key::Enter);
    assert!(matches!(hand.app.chosen.sel, Sel::Sketch(s) if s == si), "setup: the sketch left is not selected");
    hand.tool(qymcad_ui_state::PartTool::Extrude);
    hand.look2d((places[0].x, places[0].y));
    let mut failures = Vec::new();
    let hovered = cost_of(Doer::Thread, || {
        for q in &places {
            hand.hover2d(q.x, q.y);
        }
    });
    eprintln!("20 places of the ring hovered: {}", hovered.cost);
    // measured on a debug build: 0.55 s on the report's sketch; 45 s before
    let budget = if cfg!(debug_assertions) { Duration::from_millis(3_000) } else { Duration::from_millis(600) };
    if hovered.cost.work > budget {
        failures.push(over("20 places of the ring hovered", hovered.cost, budget));
    }
    let clicked = cost_of(Doer::Thread, || {
        hand.click2d(places[0].x, places[0].y);
    });
    eprintln!("a click on the ring: {}", clicked.cost);
    let budget = if cfg!(debug_assertions) { Duration::from_millis(500) } else { Duration::from_millis(100) };
    if clicked.cost.work > budget {
        failures.push(over("a click on the ring", clicked.cost, budget));
    }
    let taken = hand.app.tools.gsel.profiles.clone();
    if outer.is_none_or(|o| taken != std::collections::HashSet::from([o])) {
        failures.push(format!("the click on the ring took the regions {taken:?}, not the ring of the loop {outer:?}; the status line says {:?}", hand.app.status));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
