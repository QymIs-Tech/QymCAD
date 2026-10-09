//! A POINT PUT ON A LINE IS TIED TO IT, the start of a new line as its end: at the middle of the line a midpoint, along
//! it a point on the line. The pointer sticks there, and what it sticks to holds the point.
//!
//! Reported behaviour: "nothing snaps to projected geometry" - found on a line drawn by hand as well: the start of a line
//! put on the middle of another, or along it, was held by nothing.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use crate::gui::import_door::tests::running;
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::{Constraint, EntityKind};

    #[test]
    fn the_start_and_the_end_of_a_line_are_tied_to_the_line_they_land_on() {
        let mut failures = Vec::new();
        // where the new line starts and ends on the line (0, 0) - (40, 0), and what should hold each
        for (name, start, end, want) in [
            ("a start at the middle", (20.0, 0.0), (29.0, 13.0), "Midpoint"),
            ("a start along", (10.0, 0.0), (19.0, 23.0), "PointOnLine"),
            ("an end at the middle", (11.0, 17.0), (20.0, 0.0), "Midpoint"),
        ] {
            let (mut app, _ctx) = running();
            let si = app.create_sketch_on(SketchPlane::default());
            app.enter_sketch_edit(si);
            let mut hand = Hand::new(&mut app);
            hand.sk_tool(1).click2d(0.0, 0.0).click2d(40.0, 0.0).key(egui::Key::Escape).key(egui::Key::Escape);
            hand.sk_tool(1).click2d(start.0, start.1).click2d(end.0, end.1).key(egui::Key::Escape).key(egui::Key::Escape);
            let s = &hand.app.project.sketches[si];
            let lines: Vec<(u64, u64)> = s.entities.iter().filter_map(|e| if let EntityKind::Line { a, b } = e.kind { Some((a, b)) } else { None }).collect();
            let (base, new) = (lines[0], lines[1]);
            let on = if name.starts_with("an end") { new.1 } else { new.0 };
            let held = s.constraints.iter().find_map(|c| match *c {
                Constraint::Midpoint { p, a, b } if p == on && (a, b) == base => Some("Midpoint"),
                Constraint::PointOnLine { p, a, b } if p == on && (a, b) == base => Some("PointOnLine"),
                _ => None,
            });
            if held != Some(want) {
                failures.push(format!("{name}: held by {held:?}, wanted {want}"));
            }
        }
        assert!(failures.is_empty(), "a point put on a line:\n{}", failures.join("\n"));
    }
}
