//! A SHAPE GOES WITH ITS CENTRE, through the window: an arc, a slot and an ellipse, each drawn by its tool and dragged
//! by its centre with the select tool, have every point moved by the drag and their size and turn unchanged - as a
//! circle and a rectangle do. The centre of a fillet is held by the lines it touches and is not taken.
//!
//! Reported behaviour (#61): the centre of an arc and the centres of the ends of a slot could not be taken - nothing
//! moved; an ellipse dragged by its centre turned and changed shape, its axis ends left behind.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::{Constraint, EntityKind};

    /// The drag every point of the shape is to follow.
    const BY: (f64, f64) = (12.0, 6.0);

    /// A sketch with the shape of the tool `tool` drawn by `clicks`, in the mode of the word `mode`.
    fn drawn(tool: u8, mode: Option<&str>, clicks: &[(f64, f64)]) -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(tool);
        if let Some(word) = mode {
            assert!(hand.press_word(&crate::i18n::tr(word), egui::Pos2::ZERO), "the mode {word} on the bar");
        }
        for &(x, y) in clicks {
            hand.click2d(x, y);
        }
        hand.key(egui::Key::Escape);
        (app, si)
    }

    /// Every point of the sketch but the origin and the axes, by its id, where it stands.
    fn points(app: &App, si: usize) -> Vec<(u64, f64, f64)> {
        let s = &app.project.sketches[si];
        let system = s.system_ids();
        s.points.iter().filter(|q| !system.contains(&q.id)).map(|q| (q.id, q.x, q.y)).collect()
    }

    /// Drag the point `centre` of the sketch by `BY` with the select tool, and answer the points that did not go by it.
    fn left_behind(app: &mut App, si: usize, centre: u64) -> Vec<String> {
        let before = points(app, si);
        let at = before.iter().find(|q| q.0 == centre).map(|q| (q.1, q.2)).expect("the centre is a point of the sketch");
        Hand::new(app).sk_tool(0).drag2d(at, (at.0 + BY.0, at.1 + BY.1));
        let after = points(app, si);
        before
            .iter()
            .filter_map(|&(id, x, y)| {
                let (nx, ny) = after.iter().find(|q| q.0 == id).map(|q| (q.1, q.2))?;
                ((nx - x - BY.0).hypot(ny - y - BY.1) > 1e-3).then(|| format!("point {id}: ({x:.2}, {y:.2}) -> ({nx:.2}, {ny:.2})"))
            })
            .collect()
    }

    /// The centre of the first arc of the sketch, or of its ellipse.
    fn centre_of(app: &App, si: usize) -> u64 {
        app.project.sketches[si]
            .entities
            .iter()
            .find_map(|e| match e.kind {
                EntityKind::Arc { center, .. } => Some(center),
                EntityKind::Ellipse { c, .. } => Some(c),
                _ => None,
            })
            .expect("a shape with a centre")
    }

    #[test]
    fn an_arc_a_slot_and_an_ellipse_go_with_their_centre() {
        let mut sins = Vec::new();
        for (what, tool, mode, clicks) in [
            // away from the axes and its centre on a node of the grid: a drag led to within a few pixels of an axis or of
            // a node snaps onto it, and the drag would not be (12, 6); a point put on an axis is held there
            ("an arc by three points", 4, Some("opt-rect-3pt"), &[(10.0, 20.0), (30.0, 20.0), (20.0, 30.0)][..]),
            ("a slot", 7, None, &[(10.0, 10.0), (40.0, 10.0), (40.0, 15.0)][..]),
            ("an ellipse", 8, None, &[(10.0, 10.0), (30.0, 10.0), (10.0, 20.0)][..]),
        ] {
            let (mut app, si) = drawn(tool, mode, clicks);
            let centre = centre_of(&app, si);
            let behind = left_behind(&mut app, si, centre);
            if !behind.is_empty() {
                sins.push(format!("{what}: {}", behind.join("; ")));
            }
        }
        assert!(sins.is_empty(), "shapes dragged by their centre by {BY:?} did not go with it:\n{}", sins.join("\n"));
    }

    #[test]
    fn the_centre_of_a_fillet_is_not_taken() {
        let (mut app, si) = drawn(1, None, &[(0.0, 30.0), (0.0, 0.0), (40.0, 0.0)]);
        let s = &app.project.sketches[si];
        let lines: Vec<(u8, u64)> = s.entities.iter().filter(|e| matches!(e.kind, EntityKind::Line { .. })).map(|e| (1u8, e.id)).collect();
        assert!(Hand::new(&mut app).sk_corner_set("tb-fillet-sketch-hint", &lines, 5.0), "the corner rounded");
        let s = &app.project.sketches[si];
        assert!(s.constraints.iter().any(|c| matches!(c, Constraint::Tangent { .. })), "setup: the fillet touches its lines");
        let centre = centre_of(&app, si);
        let before = points(&app, si);
        let at = before.iter().find(|q| q.0 == centre).map(|q| (q.1, q.2)).expect("the centre of the fillet");
        Hand::new(&mut app).sk_tool(0).drag2d(at, (at.0 + BY.0, at.1 + BY.1));
        let after = points(&app, si);
        assert_eq!(before.len(), after.len());
        for (b, a) in before.iter().zip(&after) {
            assert!((a.1 - b.1).hypot(a.2 - b.2) < 1e-6, "the drag on the centre of a fillet moved point {}: {b:?} -> {a:?}", b.0);
        }
    }
}
