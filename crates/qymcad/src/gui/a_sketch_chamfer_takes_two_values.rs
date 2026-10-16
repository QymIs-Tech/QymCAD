//! A SKETCH CHAMFER TAKES TWO VALUES IN THE WINDOW: the mode is pressed on the bar, the corner clicked nearer to one of
//! its lines, and the two values typed in the fields at the corner - Tab from the first to the second, Enter applies.
//! The first leg runs along the line the click was nearer to.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::EntityKind;

    /// The two legs of the chamfer cut, measured from the sharp corner it was cut from: along the line drawn
    /// horizontally, and along the one drawn upright; `None` while the corner is not cut.
    fn legs(app: &App, si: usize, corner: u64) -> Option<(f64, f64)> {
        let sk = &app.project.sketches[si];
        let at = |id: u64| sk.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
        let c = at(corner)?;
        let cut = sk.entities.iter().skip(2).find_map(|e| match e.kind {
            EntityKind::Line { a, b } => Some((at(a)?, at(b)?)),
            _ => None,
        })?;
        let leg = |p: (f64, f64)| (p.0 - c.0).hypot(p.1 - c.1);
        let along_x = |p: (f64, f64)| (p.0 - c.0).abs() > (p.1 - c.1).abs();
        let (x, y) = if along_x(cut.0) { (cut.0, cut.1) } else { (cut.1, cut.0) };
        Some((leg(x), leg(y)))
    }

    /// The chamfer tool taken, the mode pressed, the corner of (30, 0) -> (0, 0) -> (0, 30) clicked at `click`, `first`
    /// typed, Tab, `second` typed, Enter. The legs of the cut, and the words of the status line.
    fn chamfer(mode: &str, click: (f64, f64), first: &str, second: &str) -> (Option<(f64, f64)>, String) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(1).click2d(30.0, 0.0).click2d(0.0, 0.0).double_click2d(0.0, 30.0);
        // the corner: the point both lines end at
        let ends: Vec<u64> = app.project.sketches[si]
            .entities
            .iter()
            .flat_map(|e| match e.kind {
                EntityKind::Line { a, b } => vec![a, b],
                _ => Vec::new(),
            })
            .collect();
        let corner = *ends.iter().find(|&&p| ends.iter().filter(|&&q| q == p).count() == 2).expect("two lines meeting at a corner");
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(0); // the flat canvas at the scale the sketch checks draw at
        assert!(hand.press_hint(&qymcad_i18n::tr("tb-chamfer-sketch-hint")), "no sketch chamfer button");
        assert!(hand.press_word(&qymcad_i18n::tr(mode), egui::pos2(400.0, 0.0)), "no {mode} on the bar");
        hand.click2d(click.0, click.1).type_text(first).key(egui::Key::Tab).type_text(second).key(egui::Key::Enter);
        (legs(&app, si, corner), app.status.clone().words)
    }

    /// The captions of the chamfer's fields, on the bar and at the corner, say what each field holds in the mode chosen.
    #[test]
    fn the_fields_of_a_chamfer_are_named_after_its_mode() {
        let mut sins = Vec::new();
        for (mode, captions) in [("cmd-symmetric", ["opt-chamfer-size", ""]), ("cmd-two-distances", ["cmd-leg1", "cmd-leg2"]), ("cmd-leg-angle", ["cmd-chamfer-length", "cmd-angle-deg"])] {
            let mut app = App::default();
            let si = app.create_sketch_on(SketchPlane::default());
            app.chosen.sel = Sel::Sketch(si);
            Hand::new(&mut app).sk_tool(1).click2d(30.0, 0.0).click2d(0.0, 0.0).double_click2d(0.0, 30.0);
            let mut hand = Hand::new(&mut app);
            hand.sk_tool(0);
            assert!(hand.press_hint(&qymcad_i18n::tr("tb-chamfer-sketch-hint")), "no sketch chamfer button");
            assert!(hand.press_word(&qymcad_i18n::tr(mode), egui::pos2(400.0, 0.0)), "no {mode} on the bar");
            // THE FIELDS OF ONE CHAMFER STAND TOGETHER on the bar, the modes before them: with the modes between them the
            // two values of one cut were read as two settings
            // by the right ends: a label in a wrapping row is laid out from the start of the row, so its left end says nothing
            let first = hand.written_at(&qymcad_i18n::tr(captions[0])).map(|r| r.max.x);
            for word in ["cmd-symmetric", "cmd-two-distances", "cmd-leg-angle"] {
                let at = hand.written_at(&qymcad_i18n::tr(word)).map(|r| r.max.x);
                if at.zip(first).is_none_or(|(at, first)| at > first) {
                    sins.push(format!("{mode}: the mode {word} ends at {at:?}, not before the caption of the first field at {first:?}"));
                }
            }
            for place in ["the bar", "the corner"] {
                if place == "the corner" {
                    hand.click2d(0.4, 0.1);
                }
                for caption in captions.iter().filter(|c| !c.is_empty()) {
                    if !hand.shows(&qymcad_i18n::tr(caption)) {
                        sins.push(format!("{mode}: {place} does not show the caption {caption:?}"));
                    }
                }
            }
        }
        assert!(sins.is_empty(), "{}", sins.join("\n"));
    }

    fn about(got: Option<(f64, f64)>, want: (f64, f64)) -> bool {
        got.is_some_and(|(x, y)| (x - want.0).abs() < 1e-6 && (y - want.1).abs() < 1e-6)
    }

    #[test]
    fn two_legs_and_a_leg_and_an_angle_are_typed_at_the_corner() {
        let mut sins = Vec::new();
        // clicked a hair above the horizontal line: the first leg of 5 runs along it, the second of 3 up the other
        let (got, status) = chamfer("cmd-two-distances", (0.4, 0.1), "5", "3");
        if !about(got, (5.0, 3.0)) {
            sins.push(format!("two legs 5 and 3, clicked near the horizontal line: the legs are {got:?}, not 5 along and 3 up; status: {status}"));
        }
        // clicked beside the upright line: the first leg runs up it
        let (got, status) = chamfer("cmd-two-distances", (0.1, 0.4), "5", "3");
        if !about(got, (3.0, 5.0)) {
            sins.push(format!("two legs 5 and 3, clicked near the upright line: the legs are {got:?}, not 3 along and 5 up; status: {status}"));
        }
        // a leg of 5 at 30 deg from the horizontal line: on a square corner the other leg is 5 tan 30 deg = 2.887
        let (got, status) = chamfer("cmd-leg-angle", (0.4, 0.1), "5", "30");
        if !about(got, (5.0, 5.0 * 30f64.to_radians().tan())) {
            sins.push(format!("a leg of 5 at 30 deg: the legs are {got:?}, not 5 and 2.887; status: {status}"));
        }
        // an angle that leaves no triangle on a square corner is refused in words, and the corner stays
        let (got, status) = chamfer("cmd-leg-angle", (0.4, 0.1), "5", "95");
        if got.is_some() || !status.contains(&qymcad_i18n::tr("sk-fillet-too-big")) {
            sins.push(format!("an angle of 95 deg: the legs are {got:?}, the status line says {status:?}"));
        }
        assert!(sins.is_empty(), "{}", sins.join("\n"));
    }
}
