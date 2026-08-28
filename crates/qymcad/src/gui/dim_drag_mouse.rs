//! A DIMENSION LEADER IS DRAGGED WITH THE MOUSE.
//!
//! The last of the mouse-driven paths: gizmo dragging, the rubber band, and the DIMENSION DRAG — for
//! those a snapshot is useless, a scenario with pointer events is what is needed.
//!
//! The cost of getting it wrong is mundane but nasty: dimensions crawl over each other and over the
//! geometry, a person drags the caption aside — and it does not take, or it jumps. Checking that by
//! state is pointless (a broken program can put `Dragging::Dim` into a field just as well), so here
//! there is a real canvas and a real press-move-release.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::Constraint;

    fn frame(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))),
            events,
            ..Default::default()
        }
    }

    fn press(at: egui::Pos2, down: bool) -> egui::Event {
        egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: down, modifiers: Default::default() }
    }

    /// A sketch under edit: a rectangle and a linear dimension along the bottom side.
    fn sketch_with_a_dimension() -> (App, egui::Context, usize) {
        let mut app = App::default();
        let part = app.project.add_component("Part");
        app.enter_component_for_test(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        let sid = app.project.sketches[si].id;
        app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 25.0, qymcad_core::feature::Purpose::Real);
        let pt = |app: &App, x: f64, y: f64| {
            app.project.sketches[si]
                .points
                .iter()
                .find(|p| (p.x - x).abs() < 1e-6 && (p.y - y).abs() < 1e-6)
                .map(|p| p.id)
                .expect("a point of the rectangle")
        };
        let (a, b) = (pt(&app, 0.0, 0.0), pt(&app, 40.0, 0.0));
        app.project.sketches[si].constraints.push(Constraint::Distance { a, b, d: 40.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
        app.project.solve_sketch(si);
        app.project.regen_sketch(si);

        // THE SKETCH CANVAS IS TWO-DIMENSIONAL, and the way into the edit is the one a person takes:
        // the sketch is selected and the session is open
        app.mode_3d = false;
        app.sel = super::super::Sel::Sketch(si);
        app.sketch_ses.editing = Some(sid);
        app.view.scale = 5.0;
        app.view.center = super::super::Vec2::new(20.0, 12.0);
        app.view.initialized = true;

        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport_for_test(c));
        (app, ctx, si)
    }

    fn label_offset(app: &App, si: usize) -> f64 {
        // THE DIMENSION IS LOOKED UP BY KIND, NOT BY POSITION: the rectangle also puts its own
        // constraints into the sketch, and "the first one by count" would not be a dimension at all.
        app.project.sketches[si]
            .constraints
            .iter()
            .find_map(|c| match c {
                Constraint::Distance { off, .. } => Some(*off),
                _ => None,
            })
            .expect("a linear dimension must be in the sketch")
    }

    /// THE CAPTION IS TAKEN WITH THE MOUSE AND FOLLOWS IT.
    #[test]
    fn the_dimension_label_follows_the_mouse() {
        let (mut app, ctx, si) = sketch_with_a_dimension();
        let before = label_offset(&app, si);
        let rect = app.view_rect_for_test();
        assert!(rect.is_positive(), "setup: the canvas did not lay out");

        // the caption stands by the middle of the bottom side — exactly where it is drawn
        let mid = app.to_screen_for_test(rect, qymcad_core::geom::Point2::new(20.0, 0.0));
        let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(mid)]), |c| app.viewport_for_test(c));
        let _ = ctx.run_ui(frame(vec![press(mid, true)]), |c| app.viewport_for_test(c));

        // DRAG OVER SEVERAL FRAMES: the grab is decided not by the press but when the motion is
        // recognised as a drag
        let mut grabbed = false;
        for k in 1..=4 {
            let p = mid + egui::vec2(0.0, 12.0 * k as f32);
            let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(p)]), |c| app.viewport_for_test(c));
            grabbed |= matches!(app.drag, super::super::Dragging::Dim(_));
        }
        let _ = ctx.run_ui(frame(vec![press(mid + egui::vec2(0.0, 48.0), false)]), |c| app.viewport_for_test(c));

        assert!(grabbed, "the dimension caption was not taken by the mouse: there is nothing to drag the leader aside with, and the dimensions keep crawling over each other");
        let after = label_offset(&app, si);
        assert!(
            (after - before).abs() > 1e-6,
            "the caption was taken but the offset did not change: it was {before}, it became {after} — the label did not follow the cursor"
        );
        assert!(after.is_finite(), "the offset stopped being a number: {after}");
        assert!(matches!(app.drag, super::super::Dragging::None), "after the release the grab must be let go");
    }

    /// A DRAG IN EMPTY SPACE IS A RUBBER BAND, NOT A DIMENSION.
    ///
    /// The first edition of this check expected a PAN here and went red. The program turned out to be
    /// right: in a sketch the left button in empty space pulls a band, which is the usual behaviour,
    /// while the view is moved with the middle one. What matters here is something else, and that is
    /// what is checked: whatever the empty space does, the dimension caption must not move because of
    /// it, otherwise a person would be dragging dimensions apart just by selecting.
    #[test]
    fn dragging_empty_space_starts_a_rubber_band_and_leaves_the_dimension_alone() {
        let (mut app, ctx, si) = sketch_with_a_dimension();
        let before = label_offset(&app, si);
        let center_before = app.view.center;
        let rect = app.view_rect_for_test();

        let far = egui::pos2(rect.min.x + 10.0, rect.min.y + 10.0);
        let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(far)]), |c| app.viewport_for_test(c));
        let _ = ctx.run_ui(frame(vec![press(far, true)]), |c| app.viewport_for_test(c));
        let mut box_started = false;
        for k in 1..=4 {
            let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(far + egui::vec2(15.0 * k as f32, 0.0))]), |c| app.viewport_for_test(c));
            box_started |= app.tree_sel.box_start.is_some();
        }
        let _ = ctx.run_ui(frame(vec![press(far + egui::vec2(60.0, 0.0), false)]), |c| app.viewport_for_test(c));

        assert!(box_started, "a drag in empty space did not start a rubber band — the left button in a sketch must select with a band");
        assert!((label_offset(&app, si) - before).abs() < 1e-9, "a drag in empty space moved the dimension caption");
        assert!(
            (app.view.center.x - center_before.x).abs() < 1e-9 && (app.view.center.y - center_before.y).abs() < 1e-9,
            "the rubber band moved the view along with it: the centre was {center_before:?}, it became {:?}",
            app.view.center
        );
    }
}
