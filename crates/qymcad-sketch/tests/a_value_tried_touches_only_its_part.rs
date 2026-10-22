//! A VALUE TRIED IN THE BOX OF A DIMENSION TOUCHES ONLY THE PART OF THE SKETCH IT SIZES: in a sketch of 10 000 lines - 2 500
//! rectangles apart - the width of one rectangle typed figure by figure costs the work of that rectangle, not of the
//! sketch. Every figure tried took back the whole document from a copy of it and solved the whole sketch: 79-104 ms a
//! figure in a test build, against 1.2 ms for the part alone.

use qymcad_core::time_budget::{cost_of, Doer};
use qymcad_ui_state::Bench;

/// The box of the dimension, driven through a real frame.
fn frame(b: &mut Bench, ctx: &egui::Context, rect: egui::Rect, events: Vec<egui::Event>) {
    let raw = egui::RawInput { screen_rect: Some(rect), events, ..Default::default() };
    let _ = ctx.run_ui(raw, |ui| qymcad_ui_state::tool_in_hand(ui, rect, qymcad_sketch::in_hand(&mut b.sketch_ctx())));
}

/// The sketch of the rectangles, and the place of the width of the first one among its constraints.
struct Drawn {
    si: usize,
    width: usize,
}

/// A sketch of `n` rectangles 10 by 6 apart on a grid, the first one held by its width and height.
fn rectangles(b: &mut Bench, n: usize) -> Drawn {
    b.project.new_document();
    let si = b.project.new_sketch("S");
    let side = (n as f64).sqrt().ceil() as usize;
    let mut first = Vec::new();
    b.project.batch(|p| {
        for k in 0..n {
            let (x, y) = ((k % side) as f64 * 20.0, (k / side) as f64 * 20.0);
            let ids = p.add_rect_entity(si, x, y, x + 10.0, y + 6.0, qymcad_core::feature::Purpose::Real);
            if k == 0 {
                first = ids;
            }
        }
    });
    assert!(b.project.dimension_rect(si, first[0]), "GUARD: the first rectangle takes its width and height");
    b.project.regen_sketch(si);
    let width = b.project.sketches[si].constraints.len() - 2;
    Drawn { si, width }
}

#[test]
fn typing_a_width_among_ten_thousand_lines_costs_the_work_of_one_rectangle() {
    let mut b = Bench::default();
    let Drawn { si, width } = rectangles(&mut b, 2500);
    assert!(b.project.sketches[si].entities.len() >= 10_000, "GUARD: the sketch holds {} lines", b.project.sketches[si].entities.len());
    b.view = qymcad_ui_state::View2d { center: egui::Vec2::ZERO, scale: 4.0, initialized: true, fit: 4.0 };
    b.sel = qymcad_ui_state::Sel::Sketch(si);
    b.sketch_ses.editing = Some(b.project.sketches[si].id);
    qymcad_sketch::open_dim_box(&mut b.sketch_ctx(), width);
    let ctx = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
    for _ in 0..3 {
        frame(&mut b, &ctx, rect, Vec::new());
    }
    let idle = cost_of(Doer::Thread, || frame(&mut b, &ctx, rect, Vec::new())).cost.work;
    let mut figures = Vec::new();
    for t in ["1", "2", "5"] {
        figures.push(cost_of(Doer::Thread, || frame(&mut b, &ctx, rect, vec![egui::Event::Text(t.into())])).cost.work);
    }
    let corners = b.project.rect_corners(si, b.project.sketches[si].rects()[0].sides[0]).expect("the first rectangle");
    eprintln!("A VALUE TRIED: a frame at rest {idle:?}, the figures typed {figures:?}");
    assert!((corners[0].dist(corners[1]) - 125.0).abs() < 1e-6, "GUARD: 125 typed, the width is {}", corners[0].dist(corners[1]));
    // THE FIRST FIGURE FINDS THE PART and keeps the document for Esc once; every figure after it is the work of the part.
    // Measured in a test build: 79-104 ms a figure taking the document back and solving the sketch whole, 1.2 ms the
    // part alone
    for (k, f) in figures.iter().enumerate().skip(1) {
        assert!(qymcad_core::time_budget::within(*f, std::time::Duration::from_millis(15)), "figure {} typed among 10 000 lines took {f:?}, budget 15 ms in a test build", k + 1);
    }
}

#[test]
fn typing_a_width_of_a_rectangle_opened_again_costs_the_work_of_one_rectangle() {
    let mut b = Bench::default();
    let Drawn { si, .. } = rectangles(&mut b, 2500);
    b.view = qymcad_ui_state::View2d { center: egui::Vec2::ZERO, scale: 4.0, initialized: true, fit: 4.0 };
    b.sel = qymcad_ui_state::Sel::Sketch(si);
    b.sketch_ses.editing = Some(b.project.sketches[si].id);
    let side = b.project.sketches[si].rects()[0].sides[0];
    // the first rectangle opened again by a double click on its side
    b.place.reopen(qymcad_ui_state::PlacingShape::RectSide(side));
    b.place.focus = true;
    let ctx = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
    for _ in 0..3 {
        frame(&mut b, &ctx, rect, Vec::new());
    }
    let mut figures = Vec::new();
    for t in ["1", "2", "5"] {
        figures.push(cost_of(Doer::Thread, || frame(&mut b, &ctx, rect, vec![egui::Event::Text(t.into())])).cost.work);
    }
    eprintln!("A SIZE OF A SHAPE TRIED: the figures typed {figures:?}");
    // THE FIRST FIGURE FINDS THE PART AND LAYS THE DIMENSIONS ON IT: measured in a test build, 100-115 ms laid over a copy
    // of the document taken back and solving the whole sketch for each, 30 ms solved with the part
    assert!(qymcad_core::time_budget::within(figures[0], std::time::Duration::from_millis(60)), "the first figure typed among 10 000 lines took {:?}, budget 60 ms in a test build", figures[0]);
    let corners = b.project.rect_corners(si, side).expect("the first rectangle");
    assert!((corners[0].dist(corners[1]) - 125.0).abs() < 1e-6, "GUARD: 125 typed, the width is {}", corners[0].dist(corners[1]));
    for (k, f) in figures.iter().enumerate().skip(1) {
        assert!(qymcad_core::time_budget::within(*f, std::time::Duration::from_millis(15)), "figure {} typed among 10 000 lines took {f:?}, budget 15 ms in a test build", k + 1);
    }
}
