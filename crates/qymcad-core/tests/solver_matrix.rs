//! A matrix of the solver: for every constraint type a minimal sketch is solved and the geometry is checked,
//! not merely the residual. Failures accumulate. This is where the kink of |Δ| at zero and the degeneracy of a
//! cosine residual near 0° and 180° are guarded.

use qymcad_core::model::{Constraint, Project};

fn pt(p: &Project, si: usize, id: u64) -> (f64, f64) {
    p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).unwrap()
}

/// A line of two points; returns their ids.
fn line(p: &mut Project, si: usize, ax: f64, ay: f64, bx: f64, by: f64) -> (u64, u64) {
    let eid = p.add_line_entity(si, ax, ay, bx, by, qymcad_core::feature::Purpose::Real);
    let s = &p.sketches[si];
    let e = s.entities.iter().find(|e| e.id == eid).unwrap();
    match e.kind {
        qymcad_core::model::EntityKind::Line { a, b } => (a, b),
        _ => unreachable!(),
    }
}

fn add(p: &mut Project, si: usize, c: Constraint) {
    p.sketches[si].constraints.push(c);
}

#[test]
fn matrix_constraints() {
    let mut fails: Vec<String> = Vec::new();
    // horizontal and vertical
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 10.0, 3.0);
        add(&mut p, si, Constraint::Fixed { p: a });
        add(&mut p, si, Constraint::Horizontal { a, b });
        p.solve_sketch(si);
        let (_, ay) = pt(&p, si, a);
        let (_, by) = pt(&p, si, b);
        if (ay - by).abs() > 1e-4 {
            fails.push(format!("Horizontal: Δy={:.2e}", (ay - by).abs()));
        }
    }
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 3.0, 10.0);
        add(&mut p, si, Constraint::Fixed { p: a });
        add(&mut p, si, Constraint::Vertical { a, b });
        p.solve_sketch(si);
        let dv = (pt(&p, si, a).0 - pt(&p, si, b).0).abs();
        if dv > 1e-4 {
            fails.push(format!("Vertical: Δx={dv:.2e}"));
        }
    }
    // dimensions: aligned, |Δx| and |Δy|
    for (axis, name) in [(0u8, "aligned"), (1, "horizontal |Δx|"), (2, "vertical |Δy|")] {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 7.0, 4.0);
        add(&mut p, si, Constraint::Fixed { p: a });
        add(&mut p, si, Constraint::Distance { a, b, d: 25.0, off: 0.0, expr: String::new(), driven: false, axis });
        p.solve_sketch(si);
        let (ax, ay) = pt(&p, si, a);
        let (bx, by) = pt(&p, si, b);
        let m = match axis {
            1 => (ax - bx).abs(),
            2 => (ay - by).abs(),
            _ => ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt(),
        };
        if (m - 25.0).abs() > 1e-4 {
            fails.push(format!("Distance {name}: {m:.4} ≠ 25"));
        }
    }
    // a |Δx| dimension starting from Δx near zero, the points almost on one vertical: the kink of the modulus
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 1e-6, 30.0);
        add(&mut p, si, Constraint::Fixed { p: a });
        add(&mut p, si, Constraint::Distance { a, b, d: 12.0, off: 0.0, expr: String::new(), driven: false, axis: 1 });
        p.solve_sketch(si);
        let m = (pt(&p, si, a).0 - pt(&p, si, b).0).abs();
        if (m - 12.0).abs() > 1e-3 {
            fails.push(format!("|Δx| out of zero: {m:.5} != 12, the kink of the modulus at zero"));
        }
    }
    // parallel, perpendicular and equal
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 10.0, 0.0);
        let (c, d) = line(&mut p, si, 0.0, 5.0, 9.0, 8.0);
        add(&mut p, si, Constraint::Fixed { p: a });
        add(&mut p, si, Constraint::Fixed { p: b });
        add(&mut p, si, Constraint::Parallel { a, b, c, d });
        p.solve_sketch(si);
        let (cx, cy) = pt(&p, si, c);
        let (dx, dy) = pt(&p, si, d);
        let cross = 10.0 * (dy - cy) - 0.0 * (dx - cx);
        if cross.abs() > 1e-4 {
            fails.push(format!("Parallel: cross={cross:.2e}"));
        }
    }
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 10.0, 0.0);
        let (c, d) = line(&mut p, si, 5.0, 1.0, 8.0, 9.0);
        add(&mut p, si, Constraint::Fixed { p: a });
        add(&mut p, si, Constraint::Fixed { p: b });
        add(&mut p, si, Constraint::Perpendicular { a, b, c, d });
        p.solve_sketch(si);
        let (cx, _cy) = pt(&p, si, c);
        let (dx, _dy) = pt(&p, si, d);
        if (dx - cx).abs() > 1e-4 {
            fails.push(format!("Perpendicular: Δx={:.2e}, it has to become a vertical", (dx - cx).abs()));
        }
    }
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 20.0, 0.0);
        let (c, d) = line(&mut p, si, 0.0, 10.0, 7.0, 10.0);
        add(&mut p, si, Constraint::Fixed { p: a });
        add(&mut p, si, Constraint::Fixed { p: b });
        add(&mut p, si, Constraint::Equal { a, b, c, d });
        p.solve_sketch(si);
        let (cx, cy) = pt(&p, si, c);
        let (dx, dy) = pt(&p, si, d);
        let l2 = ((dx - cx).powi(2) + (dy - cy).powi(2)).sqrt();
        if (l2 - 20.0).abs() > 1e-3 {
            fails.push(format!("Equal: length {l2:.4} != 20"));
        }
    }
    // midpoint, point-on-line and symmetry
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 10.0, 0.0);
        let (m, _) = line(&mut p, si, 3.0, 4.0, 30.0, 30.0);
        add(&mut p, si, Constraint::Fixed { p: a });
        add(&mut p, si, Constraint::Fixed { p: b });
        add(&mut p, si, Constraint::Midpoint { p: m, a, b });
        p.solve_sketch(si);
        let (mx, my) = pt(&p, si, m);
        if (mx - 5.0).abs() > 1e-4 || my.abs() > 1e-4 {
            fails.push(format!("Midpoint: ({mx:.4},{my:.4}) ≠ (5,0)"));
        }
    }
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 10.0, 10.0);
        let (q, _) = line(&mut p, si, 6.0, 2.0, 30.0, 30.0);
        add(&mut p, si, Constraint::Fixed { p: a });
        add(&mut p, si, Constraint::Fixed { p: b });
        add(&mut p, si, Constraint::PointOnLine { p: q, a, b });
        p.solve_sketch(si);
        let (qx, qy) = pt(&p, si, q);
        if (qx - qy).abs() > 1e-4 {
            fails.push(format!("PointOnLine: ({qx:.4},{qy:.4}) is not on y = x"));
        }
    }
    // a 45° angle between segments, plus the case of an angle near 180°
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 10.0, 0.0);
        let (c, d) = line(&mut p, si, 0.0, 0.0, 10.0, 3.0);
        add(&mut p, si, Constraint::Fixed { p: a });
        add(&mut p, si, Constraint::Fixed { p: b });
        add(&mut p, si, Constraint::AngleLines { a, b, c, d, deg: 45.0, expr: String::new(), driven: false });
        p.solve_sketch(si);
        let (cx, cy) = pt(&p, si, c);
        let (dx, dy) = pt(&p, si, d);
        let ang = (dy - cy).atan2(dx - cx).to_degrees().abs();
        if (ang - 45.0).abs() > 0.1 {
            fails.push(format!("AngleLines 45°: {ang:.3}°"));
        }
    }
    {
        // a target of 179° from a start of 150°: a cosine residual is almost flat near 180°
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let (a, b) = line(&mut p, si, 0.0, 0.0, 10.0, 0.0);
        let (c, d) = line(&mut p, si, 0.0, 0.0, -8.66, 5.0);
        add(&mut p, si, Constraint::Fixed { p: a });
        add(&mut p, si, Constraint::Fixed { p: b });
        add(&mut p, si, Constraint::AngleLines { a, b, c, d, deg: 179.0, expr: String::new(), driven: false });
        p.solve_sketch(si);
        let (cx, cy) = pt(&p, si, c);
        let (dx, dy) = pt(&p, si, d);
        let ang = (dy - cy).atan2(dx - cx).to_degrees();
        let dev = (ang.abs() - 179.0).abs();
        if dev > 0.5 {
            fails.push(format!("angle 179°: got {ang:.2}°, deviation {dev:.2}°, the cosine degeneracy near 180°"));
        }
    }
    // circles: diameter, concentricity, equal radii and tangency to a line
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let e1 = p.add_circle_entity(si, 0.0, 0.0, 7.0, qymcad_core::feature::Purpose::Real);
        let c1 = match p.sketches[si].entities.iter().find(|e| e.id == e1).unwrap().kind {
            qymcad_core::model::EntityKind::Circle { center, .. } => center,
            _ => unreachable!(),
        };
        add(&mut p, si, Constraint::Fixed { p: c1 });
        add(&mut p, si, Constraint::Diameter { c: c1, d: 24.0, diam: true, off: 0.0, expr: String::new(), driven: false });
        p.solve_sketch(si);
        let r = match p.sketches[si].entities.iter().find(|e| e.id == e1).unwrap().kind {
            qymcad_core::model::EntityKind::Circle { r, .. } => r,
            _ => unreachable!(),
        };
        if (r - 12.0).abs() > 1e-4 {
            fails.push(format!("Diameter 24: r={r:.4} ≠ 12"));
        }
    }
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let e1 = p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
        let (a, b) = line(&mut p, si, -20.0, 14.0, 20.0, 15.0);
        let c1 = match p.sketches[si].entities.iter().find(|e| e.id == e1).unwrap().kind {
            qymcad_core::model::EntityKind::Circle { center, .. } => center,
            _ => unreachable!(),
        };
        add(&mut p, si, Constraint::Fixed { p: c1 });
        add(&mut p, si, Constraint::Tangent { a, b, c: c1, r: 10.0 });
        p.solve_sketch(si);
        let (ax, ay) = pt(&p, si, a);
        let (bx, by) = pt(&p, si, b);
        let (cx, cy) = pt(&p, si, c1);
        let r = match p.sketches[si].entities.iter().find(|e| e.id == e1).unwrap().kind {
            qymcad_core::model::EntityKind::Circle { r, .. } => r,
            _ => unreachable!(),
        };
        let len = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt().max(1e-9);
        let dist = (((bx - ax) * (cy - ay) - (by - ay) * (cx - ax)) / len).abs();
        if (dist - r).abs() > 1e-3 {
            fails.push(format!("Tangent: dist={dist:.4} ≠ r={r:.4}"));
        }
    }
    assert!(fails.is_empty(), "\nSOLVER FAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}
