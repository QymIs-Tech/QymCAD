//! THE MIRROR PICKS AS EVERY TOOL DOES, and a mirrored arc is its mirror image.
//!
//! Geometry chosen, then the tool: Shift + click or Shift + box adds geometry to reflect, and a plain click on a line, a
//! point or an axis reflects about it. The tool first: the first click chooses what to reflect, and then the same. After a
//! reflection the tool stays in hand.
//!
//! Reported behaviour: "mirroring an arc draws the wrong part of the arc on the copy; then the tool works wrong, not as
//! every tool: with geometry chosen and the tool taken, Shift + LMB or Shift + box adds geometry to mirror, a plain click
//! on something mirrors about it (a point, a line and so on); with the tool first, the first click chooses the geometry".
#[cfg(test)]
mod tests {
    use super::super::hand::{Drag2d, Hand};
    use super::super::{App, Sel};
    use qymcad_core::feature::{Purpose, SketchPlane, Winding};
    use qymcad_core::geom::Point2;
    use qymcad_core::model::EntityKind;
    use qymcad_ui_state::EditTool;

    /// The middles of the arcs of the sketch, along the way each runs.
    fn arc_middles(app: &App) -> Vec<(f64, f64)> {
        let s = &app.project.sketches[0];
        let at = |q: u64| s.points.iter().find(|x| x.id == q).map(|x| (x.x, x.y)).expect("the point");
        s.entities
            .iter()
            .filter_map(|e| match e.kind {
                EntityKind::Arc { center, a, b, ccw } => Some((at(center), at(a), at(b), ccw)),
                _ => None,
            })
            .map(|(c, pa, pb, ccw)| {
                let (a0, a1) = ((pa.1 - c.1).atan2(pa.0 - c.0), (pb.1 - c.1).atan2(pb.0 - c.0));
                let tau = std::f64::consts::TAU;
                let sweep = if ccw { (a1 - a0).rem_euclid(tau) } else { -(a0 - a1).rem_euclid(tau) };
                let (r, m) = ((pa.0 - c.0).hypot(pa.1 - c.1), a0 + sweep / 2.0);
                (c.0 + r * m.cos(), c.1 + r * m.sin())
            })
            .collect()
    }

    fn stands(app: &App, x: f64, y: f64) -> bool {
        app.project.sketches[0].points.iter().any(|q| (q.x - x).abs() < 1e-6 && (q.y - y).abs() < 1e-6)
    }

    fn has(v: &[(f64, f64)], x: f64, y: f64) -> bool {
        v.iter().any(|p| (p.0 - x).abs() < 1e-6 && (p.1 - y).abs() < 1e-6)
    }

    #[test]
    fn the_mirror_adds_with_shift_reflects_about_a_line_or_a_point_and_stays() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        // a quarter arc about (-30, 0) from (-20, 0) to (-30, 10), its middle at (-30 + d, d); a short line below it; the
        // upright x = 0 from (0, -20) to (0, 20)
        app.project.add_arc_entity(si, Point2::new(-30.0, 0.0), Point2::new(-20.0, 0.0), Point2::new(-30.0, 10.0), Winding::Ccw, Purpose::Real);
        app.project.add_line_entity(si, -40.0, -15.0, -25.0, -15.0, Purpose::Real);
        app.project.add_line_entity(si, 0.0, -20.0, 0.0, 20.0, Purpose::Real);
        app.project.regen_sketch(si);
        let d = 10.0 * std::f64::consts::FRAC_1_SQRT_2;
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(0);
        let mut failures = Vec::new();

        // geometry chosen, then the tool: Shift + click adds the short line, a plain click on the upright reflects about it
        hand.click2d(-30.0 + d, d);
        assert!(hand.press_hint(&crate::i18n::tr("tb-mirror-sketch-hint")), "no sketch mirror button");
        hand.shift_click2d(-32.0, -15.0).click2d(0.0, 10.0);
        if !has(&arc_middles(hand.app), 30.0 - d, d) || has(&arc_middles(hand.app), 30.0 + d, -d) || !stands(hand.app, 25.0, -15.0) || !stands(hand.app, 40.0, -15.0) {
            failures.push(format!(
                "chosen first, Shift + the line, a click on x = 0: the arc's middles {:?}, the line not copied to (25..40, -15); status: {}",
                arc_middles(hand.app),
                hand.app.status
            ));
        }
        if hand.app.tools.sel_sk.modify != Some(EditTool::Mirror) || hand.app.tools.armed.modify() != Some(EditTool::Mirror) {
            failures.push(format!("after a reflection the tool is not in hand: modify {:?}, armed {:?}", hand.app.tools.sel_sk.modify, hand.app.tools.armed.modify()));
        }

        // the tool in hand, nothing chosen: the first click chooses the arc, Shift + a box adds the short line, a plain
        // click on the top end of the upright reflects about that point
        hand.click2d(-30.0 + d, d);
        hand.drag2d_held(Drag2d { from: (-44.0, -11.0), to: (-21.0, -19.0), button: egui::PointerButton::Primary, modifiers: egui::Modifiers::SHIFT });
        let held = hand.app.tools.sel_sk.mirror_of.len();
        hand.click2d(0.0, 20.0);
        if held != 2 || !has(&arc_middles(hand.app), 30.0 - d, 40.0 - d) || !stands(hand.app, 40.0, 55.0) || !stands(hand.app, 25.0, 55.0) {
            failures.push(format!(
                "the tool first, the arc clicked, Shift + a box over the line ({held} held), a click on the point (0, 20): the arc's middles {:?}, no line at (25..40, 55); status: {}",
                arc_middles(hand.app),
                hand.app.status
            ));
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
