//! THE BOX OF THE FILLET AND THE CHAMFER STANDS CLEAR OF THE CORNER it is about, through the window: the corner point,
//! the two ends the lines will be cut at and the middle of the arc or of the cut are all outside the box, whether the
//! corner was named at its point or by its two lines.
//!
//! Reported behaviour: the box stood on the corner itself and hid it, and the arc of the preview was drawn over the box,
//! across its field.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// A rectangle (0, 0) - (40, 30) drawn from a corner, the size window closed with Enter, the select tool taken.
    fn a_rectangle() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(40.0, 30.0).key(egui::Key::Enter);
        Hand::new(&mut app).sk_select();
        (app, si)
    }

    /// What of the corner the box covers, in words; empty when it stands clear.
    fn covered(hand: &Hand, si: usize, chamfer: bool) -> Vec<String> {
        let Some(bx) = hand.corner_box() else { return vec!["the box is not up".into()] };
        let standing = hand.app.tools.corner.set.standing();
        let Some(&(pid, pair)) = standing.first() else { return vec!["no corner named".into()] };
        let Some(b) = hand.app.project.corner_blend(
            si,
            pid,
            pair,
            if chamfer {
                qymcad_core::model::CornerCut::Chamfer(qymcad_core::model::ChamferLegs::equal(3.0))
            } else {
                qymcad_core::model::CornerCut::Fillet(qymcad_core::model::FilletSize::radius(3.0))
            },
        ) else {
            return vec!["no preview of the corner".into()];
        };
        let mid = match b.arc {
            // the middle of the arc: from the centre towards the corner, at the radius
            Some((c, r)) => {
                let (dx, dy) = (b.vertex[0] - c[0], b.vertex[1] - c[1]);
                let l = dx.hypot(dy);
                [c[0] + dx / l * r, c[1] + dy / l * r]
            }
            None => [(b.ends[0][0] + b.ends[1][0]) / 2.0, (b.ends[0][1] + b.ends[1][1]) / 2.0],
        };
        [("the corner", b.vertex), ("the first cut end", b.ends[0]), ("the second cut end", b.ends[1]), ("the middle of the cut", mid)]
            .into_iter()
            .filter_map(|(what, p)| {
                let s = hand.on_screen2d((p[0], p[1]));
                bx.expand(2.0).contains(s).then(|| format!("{what} at {s:?} is under the box {bx:?}"))
            })
            .collect()
    }

    #[test]
    fn the_box_stands_clear_of_a_corner_named_at_its_point_or_by_its_lines() {
        let mut sins = Vec::new();
        for (key, tool, chamfer) in [("tb-fillet-sketch-hint", "the fillet", false), ("tb-chamfer-sketch-hint", "the chamfer", true)] {
            // named at its point: the click lands on the corner
            let (mut app, si) = a_rectangle();
            let mut hand = Hand::new(&mut app);
            assert!(hand.press_hint(&qymcad_i18n::tr(key)), "no button of {tool}");
            hand.mouse2d(40.0, 30.0);
            hand.frame(Vec::new());
            sins.extend(covered(&hand, si, chamfer).into_iter().map(|s| format!("{tool}, at the point: {s}")));
            // named by its two lines, picked near the corner
            let (mut app, si) = a_rectangle();
            let mut hand = Hand::new(&mut app);
            assert!(hand.press_hint(&qymcad_i18n::tr(key)), "no button of {tool}");
            hand.mouse2d(34.0, 30.0);
            hand.shift_click2d(40.0, 24.0);
            hand.frame(Vec::new());
            sins.extend(covered(&hand, si, chamfer).into_iter().map(|s| format!("{tool}, by its lines: {s}")));
        }
        assert!(sins.is_empty(), "{}", sins.join("\n"));
    }
}
