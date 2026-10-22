//! A ROUNDED RECTANGLE WITH ITS SIZES TURNS WHOLE, through the window: a rectangle typed 45 x 35, its four corners
//! rounded R3 as one set, then Rotate by one side and 30 deg - the sides turn by 30 deg, each arc stays a quarter of a
//! circle of radius 3 inside its corner, and the sketch still solves.
//!
//! Reported behaviour: after the turn the rectangle stood on the axes as before, and every arc had flipped outward into
//! almost a whole circle R3 outside its corner, the sides running past the points of touching.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// How far each arc of the sketch turns, in degrees, with its radius.
    fn arcs(app: &App, si: usize) -> Vec<(f64, f64)> {
        let s = &app.project.sketches[si];
        let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a point of the arc");
        s.entities
            .iter()
            .filter_map(|e| match e.kind.as_arc() {
                Some(qymcad_core::model::ArcParts { center, a, b, ccw }) => {
                    let (c, pa, pb) = (at(center), at(a), at(b));
                    let (a0, a1) = ((pa.1 - c.1).atan2(pa.0 - c.0), (pb.1 - c.1).atan2(pb.0 - c.0));
                    let sweep = if ccw { (a1 - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - a1).rem_euclid(std::f64::consts::TAU) };
                    Some((sweep.to_degrees(), (pa.0 - c.0).hypot(pa.1 - c.1)))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_rounded_rectangle_with_its_sizes_turns_by_one_side() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(40.0, 30.0).type_text("45").key(egui::Key::Tab).type_text("35").key(egui::Key::Enter);
        Hand::new(&mut app).sk_select();
        let sides = app.project.sketches[si].rects()[0].sides;
        let named: Vec<qymcad_ui_state::SketchItem> = sides.iter().map(|&id| qymcad_ui_state::SketchItem::curve(id)).collect();
        assert!(Hand::new(&mut app).sk_corner_set("tb-fillet-sketch-hint", &named, 3.0), "no place to click on the four sides");
        let before = arcs(&app, si);
        assert!(before.len() == 4 && before.iter().all(|(sweep, r)| (sweep - 90.0).abs() < 1e-6 && (r - 3.0).abs() < 1e-6), "the four corners are not rounded R3: {before:?}");

        let side = sides[0];
        let spot = Hand::new(&mut app).spot2d(qymcad_ui_state::SketchItem::curve(side)).expect("a place on the bottom side");
        Hand::new(&mut app).sk_rotate(spot, (22.5, 17.5), 30.0);

        let mut sins = Vec::new();
        let s = &app.project.sketches[si];
        let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let turn = s.entities.iter().find(|e| e.id == side).and_then(|e| match e.kind.as_line() {
            Some(qymcad_core::model::LineParts { a, b }) => {
                let (pa, pb) = (at(a)?, at(b)?);
                Some((pb.1 - pa.1).atan2(pb.0 - pa.0).to_degrees())
            }
            _ => None,
        });
        if turn.is_none_or(|t| (t.rem_euclid(180.0) - 30.0).abs() > 1e-3) {
            sins.push(format!("the bottom side turned to {turn:?} deg, not 30; status: {}", app.status));
        }
        let after = arcs(&app, si);
        if after.len() != 4 || after.iter().any(|(sweep, r)| (sweep - 90.0).abs() > 1e-3 || (r - 3.0).abs() > 1e-3) {
            sins.push(format!("the arcs after the turn are not quarters of R3: {after:?}"));
        }
        let worst = app.project.sketch_residuals(si).into_iter().fold(0.0_f64, f64::max);
        if worst > 1e-6 {
            sins.push(format!("the sketch does not solve after the turn: worst residual {worst:.2e}"));
        }
        assert!(sins.is_empty(), "{}", sins.join("\n"));
    }
}
