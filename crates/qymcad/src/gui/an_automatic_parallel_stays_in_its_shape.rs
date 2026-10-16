//! AN AUTOMATIC PARALLEL STAYS IN ITS SHAPE, through the window with auto-constraints on: two triangles drawn one after
//! the other with Line share no constraint, however alike their sides; a parallelogram drawn with Line gets a Parallel
//! on each pair of its opposite sides.
//!
//! Reported behaviour (#94): a second triangle drawn beside the first got a Parallel to a side of it - any line of the
//! sketch within about 3.4 deg of the new one was taken, wherever it stood - and an edit of one moved the other.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::Constraint;

    /// A sketch with each chain of `chains` drawn by the line tool, a chain closed by its last click on its first.
    fn drawn(chains: &[&[(f64, f64)]]) -> (App, usize) {
        let mut app = App::default();
        assert!(app.set.auto_constrain, "setup: auto-constraints are on by default");
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(1);
        for chain in chains {
            for &(x, y) in *chain {
                hand.click2d(x, y);
            }
            hand.key(egui::Key::Escape);
        }
        (app, si)
    }

    /// The points drawn standing within `(lo, hi)` in x - not the origin and the axes, which hold what is put on them.
    fn points_between(app: &App, si: usize, lo: f64, hi: f64) -> Vec<u64> {
        let system = app.project.sketches[si].system_ids();
        app.project.sketches[si].points.iter().filter(|q| q.x > lo && q.x < hi && !system.contains(&q.id)).map(|q| q.id).collect()
    }

    #[test]
    fn two_triangles_drawn_apart_share_no_constraint() {
        let first: &[(f64, f64)] = &[(0.0, 0.0), (30.0, 10.0), (10.0, 30.0), (0.0, 0.0)];
        let second: &[(f64, f64)] = &[(60.0, 0.0), (90.0, 10.0), (70.0, 30.0), (60.0, 0.0)];
        let (app, si) = drawn(&[first, second]);
        let (a, b) = (points_between(&app, si, -1.0, 31.0), points_between(&app, si, 59.0, 91.0));
        let tied: Vec<String> = app.project.sketches[si]
            .constraints
            .iter()
            .filter(|c| {
                let ps = c.points();
                ps.iter().any(|p| a.contains(p)) && ps.iter().any(|p| b.contains(p))
            })
            .map(|c| format!("{c:?}"))
            .collect();
        assert!(tied.is_empty(), "the second triangle is tied to the first:\n{}", tied.join("\n"));
    }

    #[test]
    fn a_parallelogram_drawn_with_line_gets_its_parallels() {
        let (app, si) = drawn(&[&[(0.0, 0.0), (30.0, 10.0), (40.0, 30.0), (10.0, 20.0), (0.0, 0.0)]]);
        let parallels = app.project.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::Parallel { .. })).count();
        assert_eq!(parallels, 2, "a parallelogram drawn with Line: {:?}", app.project.sketches[si].constraints);
    }

    /// A CHAIN TURNING TWICE BY A SQUARE CORNER gets each corner squared and nothing more: its third line is parallel to
    /// its first through the two Perpendiculars already, and a Parallel laid on top of them is redundant.
    ///
    /// Reported behaviour: a U drawn with Line got a Perpendicular at each corner and a Parallel between its legs as
    /// well, every one of them drawn in the colour of a redundant constraint.
    #[test]
    fn a_u_drawn_with_line_holds_no_redundant_constraint() {
        // the corners nearly square, as clicks leave them: the third line runs 1 mm off parallel to the first
        let (app, si) = drawn(&[&[(30.0, 50.0), (0.0, 0.0), (20.0, -12.0), (51.0, 38.0)]]);
        let (_, redundant) = app.project.sketch_dof(si);
        assert_eq!(redundant, 0, "a U drawn with Line holds redundant constraints: {:?}", app.project.sketches[si].constraints);
    }
}
