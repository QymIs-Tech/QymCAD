//! A CIRCLE AND AN ARC TAKE A SIZE ONLY WHEN IT IS TYPED: drawn, each opens the field of its size - a diameter, a
//! radius - and lays it as a dimension only when a value is typed and the field closed with Enter. Closed untouched, or
//! with Esc, the shape is left free, and a free shape shows no number of its own. A double click on a free circle opens
//! the same field. A size laid can be deleted, and nothing
//! like it stays. An arc with a radius opens its field by a double click on the radius and on the arc alike.
//!
//! Reported behaviour: "on circles a radius is always put ... if no value was typed and nothing was edited, do not put
//! it, and let it be deleted - it is nailed down. Sometimes a mess comes of it." And: "the arc tool does not open the
//! editing when the size of the radius is clicked ... make it as everyone has it".
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use crate::gui::import_door::tests::running;
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::Constraint;

    /// HOW THE FIELD OF A SHAPE JUST DRAWN IS CLOSED.
    #[derive(Clone, Copy, Debug)]
    enum Close {
        /// Enter, nothing typed
        Untouched,
        /// Esc, nothing typed
        Escaped,
        /// a value typed, then Enter
        Typed(&'static str),
        /// a value typed, then Esc
        TypedThenEscaped(&'static str),
    }

    fn close(hand: &mut Hand, how: Close) {
        match how {
            Close::Untouched => {
                hand.key(egui::Key::Enter);
            }
            Close::Escaped => {
                hand.key(egui::Key::Escape);
            }
            Close::Typed(v) => {
                hand.type_text(v).key(egui::Key::Enter);
            }
            Close::TypedThenEscaped(v) => {
                hand.type_text(v).key(egui::Key::Escape);
            }
        }
    }

    /// The sizes of the rims of sketch 0: (a diameter, its value) for each `Diameter`.
    fn sizes(hand: &Hand) -> Vec<(bool, f64)> {
        hand.app.project.sketches[0]
            .constraints
            .iter()
            .filter_map(|c| match c {
                Constraint::Diameter { d, diam, .. } => Some((*diam, *d)),
                _ => None,
            })
            .collect()
    }

    /// Words on screen that read as the size of a rim: "Ø..." or "R...".
    fn rim_labels(hand: &mut Hand) -> Vec<String> {
        hand.frame(Vec::new());
        let label = |t: &str| t.strip_prefix('Ø').or_else(|| t.strip_prefix('R')).is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()));
        hand.words_drawn().iter().map(|(t, _)| t.clone()).filter(|t| label(t)).collect()
    }

    fn a_sketch() -> (crate::gui::App, egui::Context) {
        let (mut app, ctx) = running();
        let si = app.create_sketch_on(SketchPlane::default());
        app.enter_sketch_edit(si);
        (app, ctx)
    }

    /// A circle by its centre and a point of its rim, 20 mm across, and an arc by its centre, start and end, 10 mm in
    /// radius.
    fn draw(hand: &mut Hand, tool: u8) {
        match tool {
            3 => {
                hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(0.0, 0.0).click2d(10.0, 0.0);
            }
            _ => {
                hand.sk_tool(qymcad_ui_state::SketchShape::Arc).click2d(40.0, 0.0).click2d(50.0, 0.0).click2d(40.0, 10.0);
            }
        }
    }

    #[test]
    fn a_size_is_laid_only_when_typed() {
        let mut failures = Vec::new();
        for (tool, name, diam) in [(3u8, "circle", true), (4u8, "arc", false)] {
            for how in [Close::Untouched, Close::Escaped, Close::Typed("50"), Close::TypedThenEscaped("50")] {
                let (mut app, _ctx) = a_sketch();
                let mut hand = Hand::new(&mut app);
                draw(&mut hand, tool);
                if hand.app.tools.place.round().is_none() {
                    failures.push(format!("the {name} drawn opened no field of its size"));
                    continue;
                }
                close(&mut hand, how);
                hand.key(egui::Key::Escape).key(egui::Key::Escape);
                let want: Vec<(bool, f64)> = if matches!(how, Close::Typed(_)) { vec![(diam, 50.0)] } else { Vec::new() };
                let got = sizes(&hand);
                if got.len() != want.len() || got.iter().zip(&want).any(|(g, w)| g.0 != w.0 || (g.1 - w.1).abs() > 1e-6) {
                    failures.push(format!("the {name} closed {how:?}: sizes laid {got:?}, wanted {want:?}"));
                }
                if want.is_empty() {
                    let labels = rim_labels(&mut hand);
                    if !labels.is_empty() {
                        failures.push(format!("the {name} closed {how:?} is free and shows {labels:?}"));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "the size of a circle or an arc:\n{}", failures.join("\n"));
    }

    #[test]
    fn a_double_click_on_a_free_circle_lays_a_size_only_when_typed() {
        let mut failures = Vec::new();
        for how in [Close::Untouched, Close::Escaped, Close::Typed("30"), Close::TypedThenEscaped("30")] {
            let (mut app, _ctx) = a_sketch();
            let mut hand = Hand::new(&mut app);
            draw(&mut hand, 3);
            close(&mut hand, Close::Escaped);
            hand.key(egui::Key::Escape).key(egui::Key::Escape);
            hand.sk_select();
            assert!(sizes(&hand).is_empty(), "GUARD: the circle is free");
            hand.double_click2d(0.0, 10.0);
            if hand.app.tools.place.round().is_none() && hand.app.tools.inline.dim().is_none() {
                failures.push(format!("{how:?}: a double click on the circle opened no field"));
                continue;
            }
            close(&mut hand, how);
            let want: Vec<(bool, f64)> = if matches!(how, Close::Typed(_)) { vec![(true, 30.0)] } else { Vec::new() };
            if sizes(&hand) != want {
                failures.push(format!("a double click on a free circle closed {how:?}: sizes {:?}, wanted {want:?}", sizes(&hand)));
            }
        }
        assert!(failures.is_empty(), "a double click on a free circle:\n{}", failures.join("\n"));
    }

    #[test]
    fn a_diameter_deleted_leaves_nothing_like_it() {
        let (mut app, _ctx) = a_sketch();
        let mut hand = Hand::new(&mut app);
        draw(&mut hand, 3);
        close(&mut hand, Close::Typed("50"));
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_select();
        assert_eq!(sizes(&hand).len(), 1, "GUARD: the diameter typed is laid");
        let labels = rim_labels(&mut hand);
        assert_eq!(labels.len(), 1, "GUARD: the diameter is written once: {labels:?}");
        // the label picked, as a person picks it, and Delete
        let at = hand.written_at(&labels[0]).expect("the label of the diameter").center();
        hand.click_screen(at);
        hand.key(egui::Key::Delete);
        assert!(sizes(&hand).is_empty(), "the diameter picked and deleted is still there: {:?}", sizes(&hand));
        let left = rim_labels(&mut hand);
        assert!(left.is_empty(), "the diameter deleted, the circle still shows {left:?}");
        // and it is not laid again by itself: a frame, a move over the circle
        hand.mouse2d(10.0, 0.0);
        assert!(sizes(&hand).is_empty(), "the diameter came back by itself");
    }

    #[test]
    fn an_arc_opens_its_radius_from_the_radius_and_from_the_arc() {
        let mut failures = Vec::new();
        for (by, value) in [("its radius", "14"), ("the arc", "16")] {
            let (mut app, _ctx) = a_sketch();
            let mut hand = Hand::new(&mut app);
            draw(&mut hand, 4);
            close(&mut hand, Close::Typed("12"));
            hand.key(egui::Key::Escape).key(egui::Key::Escape);
            hand.sk_select();
            assert_eq!(sizes(&hand), vec![(false, 12.0)], "GUARD: the radius typed is laid");
            match by {
                "its radius" => {
                    let labels = rim_labels(&mut hand);
                    if labels.len() != 1 || !hand.double_click_word(&labels[0]) {
                        failures.push(format!("no radius to double-click: {labels:?}"));
                        continue;
                    }
                }
                _ => {
                    // the middle of the arc, a quarter from (52, 0) round to (40, 12) about (40, 0) once 12 is laid
                    let half = std::f64::consts::FRAC_PI_4;
                    hand.double_click2d(40.0 + 12.0 * half.cos(), 12.0 * half.sin());
                }
            }
            let open = hand.app.tools.place.round().is_some() || hand.app.tools.inline.dim().is_some();
            if !open {
                failures.push(format!("a double click on {by} opened no field; status {:?}, selected {:?}", hand.app.status, hand.app.tools.sel_sk.items));
                continue;
            }
            hand.type_text(value).key(egui::Key::Enter);
            let want: f64 = value.parse().expect("a number");
            let got = sizes(&hand);
            if got != vec![(false, want)] {
                failures.push(format!("a double click on {by} and {value} typed: sizes {got:?}"));
            }
            let radius = hand.app.project.sketches[0].entities.iter().find_map(|e| match e.kind.as_arc() {
                Some(qymcad_core::model::ArcParts { center, a, .. }) => {
                    let sk = &hand.app.project.sketches[0];
                    let at = |id| sk.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
                    at(center).zip(at(a)).map(|(c, p)| (p.0 - c.0).hypot(p.1 - c.1))
                }
                _ => None,
            });
            if radius.is_none_or(|r| (r - want).abs() > 1e-6) {
                failures.push(format!("a double click on {by} and {value} typed: the arc stands at {radius:?}"));
            }
        }
        assert!(failures.is_empty(), "the radius of an arc:\n{}", failures.join("\n"));
    }

    /// The radius of the arc about `centre_at`, the one nearest it.
    fn radius_near(hand: &Hand, centre_at: (f64, f64)) -> Option<f64> {
        let sk = &hand.app.project.sketches[0];
        let at = |id| sk.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
        sk.entities
            .iter()
            .filter_map(|e| match e.kind.as_arc() {
                Some(qymcad_core::model::ArcParts { center, a, .. }) => at(center).zip(at(a)),
                _ => None,
            })
            .min_by(|x, y| (x.0 .0 - centre_at.0).hypot(x.0 .1 - centre_at.1).total_cmp(&(y.0 .0 - centre_at.0).hypot(y.0 .1 - centre_at.1)))
            .map(|(c, p)| (p.0 - c.0).hypot(p.1 - c.1))
    }

    /// AN ARC OF A SHAPE: what is drawn, the centre of the arc looked at and the middle of the arc for a radius `r`.
    struct ShapeArc {
        name: &'static str,
        draw: fn(&mut Hand),
        centre: fn(f64) -> (f64, f64),
        middle: fn(f64) -> (f64, f64),
        radius: f64,
    }

    /// The corner (30, 0) -> (0, 0) -> (0, 30) rounded by 5 with the fillet tool, by hand.
    fn a_fillet(hand: &mut Hand) {
        hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(30.0, 0.0).click2d(0.0, 0.0).double_click2d(0.0, 30.0);
        hand.sk_select();
        assert!(hand.press_hint(&qymcad_i18n::tr("tb-fillet-sketch-hint")), "no sketch fillet button");
        hand.click2d(0.0, 0.0).type_text("5").key(egui::Key::Enter);
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
    }

    /// A slot from (0, 0) to (30, 0), 5 in radius.
    fn a_slot(hand: &mut Hand) {
        hand.sk_tool(qymcad_ui_state::SketchShape::Slot).click2d(0.0, 0.0).click2d(30.0, 0.0).click2d(0.0, 5.0);
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
    }

    #[test]
    fn the_arc_of_a_fillet_and_of_a_slot_opens_from_the_arc_and_from_its_radius() {
        let shapes = [
            ShapeArc { name: "the fillet", draw: a_fillet, centre: |r| (r, r), middle: |r| (r - r * std::f64::consts::FRAC_1_SQRT_2, r - r * std::f64::consts::FRAC_1_SQRT_2), radius: 5.0 },
            ShapeArc { name: "the slot", draw: a_slot, centre: |_| (0.0, 0.0), middle: |r| (-r, 0.0), radius: 5.0 },
        ];
        let mut failures = Vec::new();
        for shape in shapes {
            let (mut app, _ctx) = a_sketch();
            let mut hand = Hand::new(&mut app);
            (shape.draw)(&mut hand);
            hand.sk_select();
            let mut r = shape.radius;
            match radius_near(&hand, (shape.centre)(r)) {
                Some(got) if (got - r).abs() < 1e-6 => {}
                got => {
                    failures.push(format!("{}: GUARD - the arc stands at {got:?}, not {r}", shape.name));
                    continue;
                }
            }
            // by the arc
            let (x, y) = (shape.middle)(r);
            hand.double_click2d(x, y);
            if hand.app.tools.place.round().is_none() && hand.app.tools.inline.dim().is_none() {
                failures.push(format!("{}: a double click on the arc opened no field; selected {:?}", shape.name, hand.app.tools.sel_sk.items));
                continue;
            }
            hand.type_text("7").key(egui::Key::Enter);
            r = 7.0;
            let got = radius_near(&hand, (shape.centre)(r));
            if got.is_none_or(|g| (g - r).abs() > 1e-6) {
                failures.push(format!("{}: 7 typed by the arc, the arc stands at {got:?}", shape.name));
                continue;
            }
            // by its radius, laid now
            let labels = rim_labels(&mut hand);
            let label = labels.iter().find(|t| t.as_str() == "R7.0").cloned();
            if !label.is_some_and(|l| hand.double_click_word(&l)) {
                failures.push(format!("{}: no radius R7.0 to double-click: {labels:?}", shape.name));
                continue;
            }
            if hand.app.tools.place.round().is_none() && hand.app.tools.inline.dim().is_none() {
                failures.push(format!("{}: a double click on its radius opened no field", shape.name));
                continue;
            }
            hand.type_text("9").key(egui::Key::Enter);
            r = 9.0;
            let got = radius_near(&hand, (shape.centre)(r));
            if got.is_none_or(|g| (g - r).abs() > 1e-6) {
                failures.push(format!("{}: 9 typed by its radius, the arc stands at {got:?}", shape.name));
            }
        }
        assert!(failures.is_empty(), "the arc of a shape:\n{}", failures.join("\n"));
    }

    #[test]
    fn circles_tied_without_typing_are_neither_redundant_nor_in_conflict() {
        let (mut app, _ctx) = a_sketch();
        let mut hand = Hand::new(&mut app);
        for x in [0.0, 30.0, 60.0] {
            hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(x, 0.0).click2d(x + 10.0, 0.0);
            close(&mut hand, Close::Escaped);
            hand.key(egui::Key::Escape).key(egui::Key::Escape);
        }
        hand.sk_select();
        assert!(sizes(&hand).is_empty(), "GUARD: no size laid on circles drawn without typing: {:?}", sizes(&hand));
        let circles: Vec<u64> = hand.app.project.sketches[0].entities.iter().filter(|e| e.kind.as_circle().is_some()).map(|e| e.id).collect();
        // the first and the second equal, the second and the third equal: tied as a person ties them, by hand
        for pair in circles.windows(2) {
            let picked: Vec<qymcad_ui_state::SketchItem> = pair.iter().map(|&c| qymcad_ui_state::SketchItem::curve(c)).collect();
            assert!(hand.select2d(&picked), "the two circles could not be picked");
            hand.constraint(qymcad_ui_state::ConstraintKind::Equal);
        }
        let (checks, conflicts) = (hand.app.project.sketch_checks(0), hand.app.project.sketch_conflicts(0));
        assert!(checks.redundant.is_empty() && conflicts.is_empty(), "circles tied without typing: redundant {:?}, in conflict {conflicts:?}", checks.redundant);
    }
}
