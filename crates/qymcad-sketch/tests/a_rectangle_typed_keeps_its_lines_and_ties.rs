//! A RECTANGLE TYPED TO ITS SIZE KEEPS ITS LINES AND WHAT HOLDS ITS CORNERS: the width typed into its box moves its
//! corners, and the lines and the points are the same ones after it - the corner drawn on the end of another line stays
//! that end. Laid anew for every figure typed, the rectangle took new ids each time, and the corner left the line.

use qymcad_core::geom::Point2;
use qymcad_core::model::Id;
use qymcad_ui_state::Bench;

/// The box of the sizes, driven through a real frame.
fn frame(b: &mut Bench, ctx: &egui::Context, rect: egui::Rect, events: Vec<egui::Event>) {
    let raw = egui::RawInput { screen_rect: Some(rect), events, ..Default::default() };
    let _ = ctx.run_ui(raw, |ui| qymcad_ui_state::tool_in_hand(ui, rect, qymcad_sketch::in_hand(&mut b.sketch_ctx())));
}

#[test]
fn typing_the_width_moves_the_corners_of_the_same_rectangle() {
    let mut b = Bench::default();
    b.project.new_document();
    let si = b.project.new_sketch("S");
    // a line ending where the rectangle is drawn from
    let line = b.project.add_line_entity(si, -20.0, 20.0, 10.0, 20.0, qymcad_core::feature::Purpose::Real);
    let (a, corner) = (Point2::new(10.0, 20.0), Point2::new(50.0, 80.0));
    let ids = b.project.add_rect_entity(si, a.x, a.y, corner.x, corner.y, qymcad_core::feature::Purpose::Real);
    let at = |b: &Bench, id: Id| b.project.sketches[si].points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
    let line_end = b.project.sketches[si]
        .entity(line)
        .map(qymcad_core::model::entity_points)
        .unwrap_or_default()
        .into_iter()
        .find(|&p| at(&b, p).is_some_and(|q| q.dist(a) < 1e-9))
        .expect("GUARD: the end of the line at the corner");
    let rect = b.project.sketches[si].rects().into_iter().find(|r| r.sides.contains(&ids[0])).expect("GUARD: the rectangle");
    let first = rect.corners.into_iter().find(|&c| at(&b, c).is_some_and(|q| q.dist(a) < 1e-9)).expect("GUARD: the first corner of the rectangle");
    // the rectangle drawn on the end of the line takes that end for its corner: the two are one point
    assert_eq!(first, line_end, "GUARD: the rectangle drawn on the end of the line is not held by it");
    b.project.regen_sketch(si);
    b.view = qymcad_ui_state::View2d { center: egui::Vec2::ZERO, scale: 4.0, initialized: true, fit: 4.0 };
    b.place.set(qymcad_ui_state::PlacingShape::Rect { a, b: corner, ids: ids.clone() });
    b.place.focus = true;
    b.armed = qymcad_ui_state::Armed::Draw(qymcad_ui_state::SketchShape::Rect);
    b.sel = qymcad_ui_state::Sel::Sketch(si);
    b.sketch_ses.editing = Some(b.project.sketches[si].id);
    let entities_before: Vec<Id> = b.project.sketches[si].entities.iter().map(|e| e.id).collect();
    let ctx = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
    for _ in 0..3 {
        frame(&mut b, &ctx, rect, Vec::new());
    }
    frame(&mut b, &ctx, rect, vec![egui::Event::Text("25".into())]);
    for _ in 0..3 {
        frame(&mut b, &ctx, rect, Vec::new());
    }
    let entities_after: Vec<Id> = b.project.sketches[si].entities.iter().map(|e| e.id).collect();
    assert_eq!(entities_after, entities_before, "typing the width laid the rectangle anew: its lines are other ones");
    let corners = b.project.rect_corners(si, ids[0]).expect("the rectangle is still the one of its first side");
    assert!((corners[0].dist(corners[1]) - 25.0).abs() < 1e-6, "the width is {} and not 25", corners[0].dist(corners[1]));
    // THE LOOP THE LINES ARE DRAWN BY GOES WITH THE CORNERS at once. Reported behaviour: the points of a rectangle went
    // with the width typed and its lines stood where they were, drawn anew only some time after
    let closed = b.project.sketches[si]
        .contour_ids
        .iter()
        .filter_map(|&cid| b.project.contour_index(cid))
        .filter(|&ci| b.project.contours[ci].closed)
        .map(|ci| {
            let xs = b.project.contours[ci].points.iter().map(|p| p.x);
            let (lo, hi) = xs.fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
            hi - lo
        })
        .fold(0.0_f64, f64::max);
    assert!((closed - 25.0).abs() < 1e-6, "the closed loop of the rectangle is {closed} wide after 25 was typed: its lines stand where they were");
    let rect = b.project.sketches[si].rects().into_iter().find(|r| r.sides.contains(&ids[0])).expect("the rectangle");
    assert!(rect.corners.contains(&line_end), "the corner shared with the end of the line is gone: the corners are {:?}", rect.corners);
}
