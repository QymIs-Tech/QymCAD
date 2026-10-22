//! A RECTANGLE DRAGGED BY A CORNER STRETCHES FROM THE CORNER ACROSS, in real frames of the window: the corner across
//! stays and the dragged corner goes where the pointer took it, so both the width and the height change; drawn from its
//! centre, the centre stays under a dragged corner, and a dragged centre carries the whole rectangle.
//!
//! Reported behaviour (issue #56): on a rectangle drawn from a corner, dragging a corner next to that corner moved it
//! too; with that corner held, a neighbour dragged changed one size only.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// A rectangle (0, 0) - (40, 30) drawn by `mode` - its two clicks, the size window closed with Enter - and the select
    /// tool taken.
    fn a_rectangle(mode: &str, clicks: [(f64, f64); 2]) -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect);
        assert!(hand.press_word(&qymcad_i18n::tr(mode), egui::pos2(0.0, 0.0)), "no {mode} on the bar");
        hand.click2d(clicks[0].0, clicks[0].1).click2d(clicks[1].0, clicks[1].1).key(egui::Key::Enter);
        Hand::new(&mut app).sk_select();
        assert_eq!(app.project.sketches[si].rects().len(), 1, "the rectangle was not drawn as one shape");
        (app, si)
    }

    /// Where the point of the sketch nearest to `at` stands now, `None` when there is none within 1 mm.
    fn point_near(app: &App, si: usize, id: u64) -> (f64, f64) {
        let q = app.project.sketches[si].points.iter().find(|q| q.id == id).expect("the point is there");
        (q.x, q.y)
    }

    fn id_at(app: &App, si: usize, at: (f64, f64)) -> u64 {
        app.project.sketches[si].points.iter().filter(|q| (q.x - at.0).hypot(q.y - at.1) < 1e-6).map(|q| q.id).next().expect("a corner there")
    }

    fn stayed(app: &App, si: usize, id: u64, at: (f64, f64)) -> Option<String> {
        let now = point_near(app, si, id);
        ((now.0 - at.0).hypot(now.1 - at.1) > 1e-3).then(|| format!("went from {at:?} to {now:?}"))
    }

    #[test]
    fn a_dragged_corner_stretches_the_rectangle_from_the_corner_across() {
        let mut sins = Vec::new();
        // drawn from the top left corner (10, 40), as reported, clear of the origin a corner would be glued to; every
        // corner dragged outwards, the one drawn from too
        let corners = [(10.0, 10.0), (50.0, 10.0), (50.0, 40.0), (10.0, 40.0)];
        for k in 0..4 {
            let (from, across) = (corners[k], corners[(k + 2) % 4]);
            let to = (from.0 + if from.0 > 30.0 { 8.0 } else { -8.0 }, from.1 + if from.1 > 25.0 { 6.0 } else { -6.0 });
            let (mut app, si) = a_rectangle("opt-rect-2corners", [(10.0, 40.0), (50.0, 10.0)]);
            let (dragged, held) = (id_at(&app, si, from), id_at(&app, si, across));
            Hand::new(&mut app).drag2d(from, to);
            if let Some(why) = stayed(&app, si, held, across) {
                sins.push(format!("the corner {from:?} dragged: the corner across {why}"));
            }
            let now = point_near(&app, si, dragged);
            if (now.0 - to.0).hypot(now.1 - to.1) > 0.5 {
                sins.push(format!("the corner {from:?} dragged to {to:?} stands at {now:?}: not both sizes changed"));
            }
        }
        // drawn from its centre (20, 15): a corner dragged - the centre stays
        let (mut app, si) = a_rectangle("opt-rect-centre", [(20.0, 15.0), (40.0, 30.0)]);
        let centre = app.project.sketches[si].rects()[0].centre;
        Hand::new(&mut app).drag2d((40.0, 30.0), (46.0, 36.0));
        if let Some(why) = stayed(&app, si, centre, (20.0, 15.0)) {
            sins.push(format!("drawn from the centre, a corner dragged: the centre {why}"));
        }
        assert!(sins.is_empty(), "{}", sins.join("\n"));
    }

    /// A RECTANGLE DRAGGED BY ITS CENTRE GOES WITH IT AS A WHOLE, as a circle goes with its centre, in either way it
    /// was drawn. Reported (issue #56): the centre could not be dragged, the rectangle stayed where it was.
    #[test]
    fn a_rectangle_dragged_by_its_centre_goes_as_a_whole() {
        let mut sins = Vec::new();
        for (mode, clicks) in [("opt-rect-2corners", [(10.0, 10.0), (50.0, 40.0)]), ("opt-rect-centre", [(30.0, 25.0), (50.0, 40.0)])] {
            let (mut app, si) = a_rectangle(mode, clicks);
            let before: Vec<(f64, f64)> = app.project.sketches[si].rects()[0].corners.iter().map(|c| point_near(&app, si, *c)).collect();
            Hand::new(&mut app).drag2d((30.0, 25.0), (42.0, 31.0));
            let after: Vec<(f64, f64)> = app.project.sketches[si].rects()[0].corners.iter().map(|c| point_near(&app, si, *c)).collect();
            if before.iter().zip(&after).any(|(b, a)| (a.0 - b.0 - 12.0).hypot(a.1 - b.1 - 6.0) > 0.5) {
                sins.push(format!("{mode}: the centre dragged by (12, 6) moved the corners {before:?} -> {after:?}"));
            }
        }
        assert!(sins.is_empty(), "{}", sins.join("\n"));
    }

    /// A WIDTH AND A HEIGHT TYPED IN THE SIZE WINDOW ARE LAID AS DIMENSIONS, as the diameter of a circle is; the window
    /// closed untouched leaves the rectangle free.
    #[test]
    fn a_typed_width_and_height_are_laid_as_dimensions() {
        let dims = |app: &App, si: usize| -> Vec<f64> {
            let mut d: Vec<f64> = app.project.sketches[si].constraints.iter().filter_map(|c| if let qymcad_core::model::Constraint::Distance { d, .. } = c { Some(*d) } else { None }).collect();
            d.sort_by(f64::total_cmp);
            d
        };
        let mut sins = Vec::new();
        // typed: 60 by 35
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect);
        hand.click2d(10.0, 10.0).click2d(50.0, 40.0).type_text("60").key(egui::Key::Tab).type_text("35").key(egui::Key::Enter);
        let got = dims(&app, si);
        if got.len() != 2 || (got[0] - 35.0).abs() > 1e-6 || (got[1] - 60.0).abs() > 1e-6 {
            sins.push(format!("60 by 35 typed: the dimensions are {got:?}"));
        }
        // untouched: no dimension
        let (app, si) = a_rectangle("opt-rect-2corners", [(10.0, 10.0), (50.0, 40.0)]);
        if !dims(&app, si).is_empty() {
            sins.push(format!("nothing typed: the dimensions are {:?}", dims(&app, si)));
        }
        assert!(sins.is_empty(), "{}", sins.join("\n"));
    }

    /// A DOUBLE CLICK ON A SIDE REOPENS THE RECTANGLE WITH ITS WIDTH AND HEIGHT, as a double click on a circle reopens its
    /// diameter: the dimensions are laid, the values typed are theirs, and the rectangle grows from the corner it was
    /// drawn from.
    #[test]
    fn a_double_click_on_a_side_reopens_the_width_and_height() {
        let (mut app, si) = a_rectangle("opt-rect-2corners", [(10.0, 10.0), (50.0, 40.0)]);
        Hand::new(&mut app).double_click2d(30.0, 10.0).type_text("60").key(egui::Key::Tab).type_text("35").key(egui::Key::Enter);
        let s = &app.project.sketches[si];
        let mut dims: Vec<f64> = s.constraints.iter().filter_map(|c| if let qymcad_core::model::Constraint::Distance { d, .. } = c { Some(*d) } else { None }).collect();
        dims.sort_by(f64::total_cmp);
        let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a corner");
        let [a, _, c, _] = s.rects()[0].corners.map(at);
        assert!(
            dims.len() == 2
                && (dims[0] - 35.0).abs() < 1e-6
                && (dims[1] - 60.0).abs() < 1e-6
                && (a.0 - 10.0).abs() < 1e-6
                && (a.1 - 10.0).abs() < 1e-6
                && (c.0 - 70.0).abs() < 1e-6
                && (c.1 - 45.0).abs() < 1e-6,
            "a double click on a side, 60 and 35 typed: the dimensions are {dims:?}, the corners {a:?} and {c:?}; status: {}",
            app.status
        );
    }

    /// THE CENTRE OF A RECTANGLE TAKES CONSTRAINTS like any point: clicked, the origin clicked with Shift, Coincident -
    /// the rectangle stands about the origin; on another, its centre clicked with a circle's centre, Vertical - the
    /// centre stands straight above it.
    #[test]
    fn the_centre_of_a_rectangle_takes_constraints() {
        let mut sins = Vec::new();
        let (mut app, si) = a_rectangle("opt-rect-2corners", [(10.0, 10.0), (50.0, 40.0)]);
        Hand::new(&mut app).click2d(30.0, 25.0).shift_click2d(0.0, 0.0).constraint(qymcad_ui_state::ConstraintKind::Coincident);
        let c = point_near(&app, si, app.project.sketches[si].rects()[0].centre);
        if c.0.hypot(c.1) > 1e-6 {
            sins.push(format!("the centre made coincident with the origin stands at {c:?}; status: {}", app.status));
        }
        let (mut app, si) = a_rectangle("opt-rect-2corners", [(10.0, 10.0), (50.0, 40.0)]);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(70.0, 60.0).click2d(75.0, 60.0).key(egui::Key::Enter);
        Hand::new(&mut app).sk_select().click2d(30.0, 25.0).shift_click2d(70.0, 60.0).constraint(qymcad_ui_state::ConstraintKind::Vertical);
        let c = point_near(&app, si, app.project.sketches[si].rects()[0].centre);
        let o = app.project.sketches[si]
            .entities
            .iter()
            .find_map(|e| if let Some(qymcad_core::model::CircleParts { center, .. }) = e.kind.as_circle() { Some(point_near(&app, si, center)) } else { None })
            .expect("the circle");
        if (c.0 - o.0).abs() > 1e-6 {
            sins.push(format!("the centre made vertical with a circle's centre {o:?} stands at {c:?}; status: {}", app.status));
        }
        assert!(sins.is_empty(), "{}", sins.join("\n"));
    }

    /// ROTATE TAKES THE WHOLE RECTANGLE: a click on one side, the centre, 30 deg - all four sides turn and the
    /// rectangle stays square. Reported (issue #56): only the side clicked was taken.
    #[test]
    fn rotate_takes_the_whole_rectangle_by_one_side() {
        let (mut app, si) = a_rectangle("opt-rect-2corners", [(0.0, 0.0), (40.0, 30.0)]);
        Hand::new(&mut app).sk_rotate((20.0, 0.0), (20.0, 15.0), 30.0);
        let s = &app.project.sketches[si];
        let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a corner");
        let [a, b, _, d] = s.rects()[0].corners.map(at);
        let turn = (b.1 - a.1).atan2(b.0 - a.0).to_degrees();
        let square = (b.0 - a.0) * (d.0 - a.0) + (b.1 - a.1) * (d.1 - a.1);
        assert!((turn - 30.0).abs() < 1e-6 && square.abs() < 1e-6, "the rectangle turned to {turn:.3} deg, off square by {square:.2e}; status: {}", app.status);
    }
}
