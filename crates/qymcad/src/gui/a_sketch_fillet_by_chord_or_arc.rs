//! A SKETCH FILLET IS GIVEN IN THE WINDOW BY ITS RADIUS, ITS CHORD OR ITS ARC LENGTH: the way is pressed on the bar
//! before the field, the corner clicked, the value typed in the field at the corner, Enter applies. The captions of
//! the field on the bar and at the corner name the way.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::EntityKind;

    /// The radius, the chord and the arc length of the fillet arc, `None` while there is none.
    struct Measured {
        radius: f64,
        chord: f64,
        arc: f64,
    }

    fn measure(app: &App, si: usize) -> Option<Measured> {
        let s = &app.project.sketches[si];
        let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let (c, a, b) = s.entities.iter().find_map(|e| match e.kind {
            EntityKind::Arc { center, a, b, .. } => Some((at(center)?, at(a)?, at(b)?)),
            _ => None,
        })?;
        let radius = (a.0 - c.0).hypot(a.1 - c.1);
        let chord = (b.0 - a.0).hypot(b.1 - a.1);
        Some(Measured { radius, chord, arc: radius * 2.0 * (chord / (2.0 * radius)).clamp(-1.0, 1.0).asin() })
    }

    /// The corner (30, 0) -> (0, 0) -> (0, 30) drawn as a chain, the fillet tool taken, `way` pressed on the bar. The
    /// window is left in hand for the caller.
    fn fillet_in_hand(app: &mut App, way: &str) -> usize {
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(app).sk_tool(1).click2d(30.0, 0.0).click2d(0.0, 0.0).double_click2d(0.0, 30.0);
        let mut hand = Hand::new(app);
        hand.sk_tool(0);
        assert!(hand.press_hint(&qymcad_i18n::tr("tb-fillet-sketch-hint")), "no sketch fillet button");
        // the mode, the leftmost of the words: the caption of the field after the modes says the same word, and a caption
        // in a wrapping row is laid out from the start of the row - its middle stood on the next mode, Chord
        assert!(hand.press_word(&qymcad_i18n::tr(way), egui::pos2(0.0, 0.0)), "no {way} on the bar");
        si
    }

    #[test]
    fn a_fillet_is_typed_as_a_radius_a_chord_or_an_arc_length() {
        let mut sins = Vec::new();
        let pi = std::f64::consts::PI;
        // on a square corner: a radius of 5; a chord of 5 = a radius of 5 / sqrt 2; an arc of 5 = a radius of 10 / pi
        for (way, r) in [("opt-radius", 5.0), ("opt-fillet-chord", 5.0 / 2f64.sqrt()), ("opt-fillet-arc-length", 10.0 / pi)] {
            let mut app = App::default();
            let si = fillet_in_hand(&mut app, way);
            let mut hand = Hand::new(&mut app);
            // the caption of the field on the bar, then at the corner, names the way
            if !hand.shows(&qymcad_i18n::tr(way)) {
                sins.push(format!("{way}: no caption of the way on the bar"));
            }
            hand.click2d(0.0, 0.0);
            let at_corner = hand.written_at(&qymcad_i18n::tr(way)).map(|_| ());
            hand.type_text("5").key(egui::Key::Enter);
            if way != "opt-radius" && at_corner.is_none() {
                sins.push(format!("{way}: the field at the corner is not captioned with the way"));
            }
            match measure(&app, si) {
                Some(m) if (m.radius - r).abs() < 1e-5 => {}
                m => sins.push(format!("{way} 5: radius / chord / arc {:?}, not a radius of {r:.4}; status: {}", m.map(|m| (m.radius, m.chord, m.arc)), app.status)),
            }
        }
        // a chord as long as the lines is refused in words, and the corner stays sharp
        let mut app = App::default();
        let si = fillet_in_hand(&mut app, "opt-fillet-chord");
        Hand::new(&mut app).click2d(0.0, 0.0).type_text("45").key(egui::Key::Enter);
        if measure(&app, si).is_some() || !app.status.words.contains(&qymcad_i18n::tr("sk-fillet-too-big")) {
            sins.push(format!("a chord of 45 on lines of 30: rounded {:?}, the status line says {:?}", measure(&app, si).map(|m| m.radius), app.status));
        }
        assert!(sins.is_empty(), "{}", sins.join("\n"));
    }
}
