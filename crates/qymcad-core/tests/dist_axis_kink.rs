//! The residual of a horizontal or vertical dimension is |Δ| − d, which has a kink at zero.
//!
//! What is probed here: degenerate starts, at exactly zero and with the points fully coincident, preservation of
//! the side, and repeated solves as during a drag. Failures accumulate so every broken combination is visible at
//! once.
use qymcad_core::model::{Constraint, Project};

fn pt(p: &Project, si: usize, id: u64) -> (f64, f64) {
    p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).unwrap()
}

fn line(p: &mut Project, si: usize, ax: f64, ay: f64, bx: f64, by: f64) -> (u64, u64) {
    let eid = p.add_line_entity(si, ax, ay, bx, by, qymcad_core::feature::Purpose::Real);
    let e = p.sketches[si].entities.iter().find(|e| e.id == eid).unwrap();
    match e.kind {
        qymcad_core::model::EntityKind::Line { a, b } => (a, b),
        _ => unreachable!(),
    }
}

fn dist(d: f64, axis: u8, a: u64, b: u64) -> Constraint {
    Constraint::Distance { a, b, d, off: 0.0, expr: String::new(), driven: false, axis }
}

#[test]
fn axis_distance_from_degenerate_start() {
    let mut fails: Vec<String> = Vec::new();

    // 1. Δx is exactly zero, a vertical segment, with a horizontal dimension of 12
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 0.0, 30.0);
        p.sketches[si].constraints.push(Constraint::Fixed { p: a });
        p.sketches[si].constraints.push(dist(12.0, 1, a, b));
        p.solve_sketch(si);
        let m = (pt(&p, si, a).0 - pt(&p, si, b).0).abs();
        if (m - 12.0).abs() > 1e-3 {
            fails.push(format!("Δx exactly 0 gave |Δx| = {m:.5}, expecting 12"));
        }
    }
    // 2. Δy is exactly zero, a horizontal segment, with a vertical dimension of 7
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 30.0, 0.0);
        p.sketches[si].constraints.push(Constraint::Fixed { p: a });
        p.sketches[si].constraints.push(dist(7.0, 2, a, b));
        p.solve_sketch(si);
        let m = (pt(&p, si, a).1 - pt(&p, si, b).1).abs();
        if (m - 7.0).abs() > 1e-3 {
            fails.push(format!("Δy exactly 0 gave |Δy| = {m:.5}, expecting 7"));
        }
    }
    // 3. The points coincide completely, one end dragged onto the other, with a horizontal dimension of 10:
    //    both coordinates are degenerate and only x can be pulled apart.
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 5.0, 5.0, 15.0, 5.0);
        assert_ne!(a, b, "the ends of the segment are distinct nodes");
        if let Some(q) = p.sketches[si].points.iter_mut().find(|q| q.id == b) {
            q.x = 5.0; // the end landed exactly on the start
            q.y = 5.0;
        }
        p.sketches[si].constraints.push(Constraint::Fixed { p: a });
        p.sketches[si].constraints.push(dist(10.0, 1, a, b));
        p.solve_sketch(si);
        let m = (pt(&p, si, a).0 - pt(&p, si, b).0).abs();
        if (m - 10.0).abs() > 1e-3 {
            fails.push(format!("coincident points gave |Δx| = {m:.5}, expecting 10"));
        }
    }
    // 4. The side is preserved: B is to the left of A at Δx = −10, and a dimension of 20 has to leave B on the
    //    left at −20 rather than jump it to the right, which looks like the figure turning inside out.
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, -10.0, 5.0);
        p.sketches[si].constraints.push(Constraint::Fixed { p: a });
        p.sketches[si].constraints.push(dist(20.0, 1, a, b));
        p.solve_sketch(si);
        let dx = pt(&p, si, b).0 - pt(&p, si, a).0;
        if (dx + 20.0).abs() > 1e-3 {
            fails.push(format!("the side was not preserved: Δx = {dx:.5}, expecting −20"));
        }
    }
    // 5. A small dimension out of zero, where the solver step is smaller than the kink: Δx = 0 to 0.05
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 0.0, 3.0);
        p.sketches[si].constraints.push(Constraint::Fixed { p: a });
        p.sketches[si].constraints.push(dist(0.05, 1, a, b));
        p.solve_sketch(si);
        let m = (pt(&p, si, a).0 - pt(&p, si, b).0).abs();
        if (m - 0.05).abs() > 1e-4 {
            fails.push(format!("a small dimension out of zero gave |Δx| = {m:.6}, expecting 0.05"));
        }
    }
    // 6. Repeated solves, as during a drag where the sketch is solved every frame: the dimension must not
    //    drift.
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 0.0, 30.0);
        p.sketches[si].constraints.push(Constraint::Fixed { p: a });
        p.sketches[si].constraints.push(dist(12.0, 1, a, b));
        let mut seen: Vec<f64> = Vec::new();
        for _ in 0..5 {
            p.solve_sketch(si);
            seen.push(pt(&p, si, b).0 - pt(&p, si, a).0);
        }
        let flip = seen.windows(2).any(|w| w[0] * w[1] < 0.0);
        if flip || seen.iter().any(|v| (v.abs() - 12.0).abs() > 1e-3) {
            fails.push(format!("repeated solves are unstable: {seen:?}"));
        }
    }

    assert!(fails.is_empty(), "axis dimensions from a degenerate start:\n{}", fails.join("\n"));
}

/// A chain whose solution requires flipping the side of one dimension: A to B is 10, B to C is 10, and A to C
/// is 0, so C has to return onto A and the B-to-C dimension changes sides. The system is solvable and the solver
/// has to solve it: the side of an axis dimension must not become a hard constraint.
#[test]
fn axis_distance_chain_that_must_flip_a_side() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    let (a, b) = line(&mut p, si, 0.0, 0.0, 10.0, 0.0);
    let (_b2, c) = line(&mut p, si, 10.0, 0.0, 20.0, 0.0);
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    p.sketches[si].constraints.push(dist(10.0, 1, a, b));
    p.sketches[si].constraints.push(dist(10.0, 1, b, c));
    p.sketches[si].constraints.push(dist(0.0, 1, a, c));
    p.solve_sketch(si); // one solve suffices: the barrier of |Δ| at zero is crossed by the multi-start
    let (ax, bx, cx) = (pt(&p, si, a).0, pt(&p, si, b).0, pt(&p, si, c).0);
    let mut bad = Vec::new();
    if ((ax - bx).abs() - 10.0).abs() > 1e-3 {
        bad.push(format!("A to B = {:.4}, expecting 10", (ax - bx).abs()));
    }
    if ((bx - cx).abs() - 10.0).abs() > 1e-3 {
        bad.push(format!("B to C = {:.4}, expecting 10", (bx - cx).abs()));
    }
    if (ax - cx).abs() > 1e-3 {
        bad.push(format!("A to C = {:.4}, expecting 0", (ax - cx).abs()));
    }
    assert!(bad.is_empty(), "a chain requiring a side flip (A={ax:.3}, B={bx:.3}, C={cx:.3}):\n{}", bad.join("\n"));
}
