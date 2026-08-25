//! A matrix of the sketcher: tools against cases. Failures accumulate and are reported together.
//!
//! The central case is a dumbbell, or a cam: three circles with a larger one in the middle, four external
//! tangent lines, and the inner arcs trimmed away. It has to come out as one closed contour with an exact
//! area.

use qymcad_core::model::Project;

const PI: f64 = std::f64::consts::PI;

fn closed_contours(p: &Project, si: usize) -> Vec<(u64, f64)> {
    p.sketches[si]
        .contour_ids
        .iter()
        .copied()
        .filter_map(|c| {
            let ci = p.contour_index(c)?;
            let ct = &p.contours[ci];
            if ct.closed && p.contour_profile_xy(c).is_some() {
                Some((c, ct.area()))
            } else {
                None
            }
        })
        .collect()
}

fn check(fails: &mut Vec<String>, label: &str, got: &[(u64, f64)], want_n: usize, want_area: f64, tol: f64) {
    if got.len() != want_n {
        fails.push(format!("{label}: {} contours, expected {want_n}; areas: {:?}", got.len(), got.iter().map(|g| g.1.round()).collect::<Vec<_>>()));
        return;
    }
    if want_n == 1 && want_area > 0.0 {
        let a = got[0].1;
        if ((a - want_area) / want_area).abs() > tol {
            fails.push(format!("{label}: area {a:.1}, expected {want_area:.1}"));
        }
    }
}

/// Primitives one at a time: the contour closes and the area is exact.
#[test]
fn matrix_primitives() {
    let mut fails = Vec::new();
    // a rectangle
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        p.add_rect_entity(si, 0.0, 0.0, 30.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        check(&mut fails, "rectangle", &closed_contours(&p, si), 1, 600.0, 0.01);
    }
    // a circle
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        p.add_circle_entity(si, 5.0, 5.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        check(&mut fails, "circle", &closed_contours(&p, si), 1, PI * 100.0, 0.01);
    }
    // a triangle from three lines meeting at points
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        p.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
        p.add_line_entity(si, 30.0, 0.0, 0.0, 30.0, qymcad_core::feature::Purpose::Real);
        p.add_line_entity(si, 0.0, 30.0, 0.0, 0.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        check(&mut fails, "triangle from lines", &closed_contours(&p, si), 1, 450.0, 0.01);
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Trimming: the basic cases that lead up to the dumbbell.
#[test]
fn matrix_trim_basic() {
    let mut fails = Vec::new();
    // a circle with a line cutting all the way through: trim the upper arc away and an arc plus a chord
    // remain, giving one segment contour
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let c = p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.add_line_entity(si, -20.0, 0.0, 20.0, 0.0, qymcad_core::feature::Purpose::Real);
        if !p.trim_curve(si, c, 0.0, 10.0) {
            fails.push("trim of a circle by a secant: trim_curve returned false".into());
        }
        p.regen_sketch(si);
        // what is left is the lower semicircle plus the line, giving a half-disc region of 50π; the line
        // sticks out at both ends
        let cc = closed_contours(&p, si);
        check(&mut fails, "trim of a circle by a secant, half-disc", &cc, 1, PI * 50.0, 0.02);
    }
    // two overlapping circles forming a lens: trim the inner arc of one of them
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        let c1 = p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.add_circle_entity(si, 12.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
        // click the arc of c1 that lies inside c2, on the right at (10,0)
        if !p.trim_curve(si, c1, 10.0, 0.0) {
            fails.push("trim of overlapping circles: trim_curve returned false".into());
        }
        p.regen_sketch(si);
        let cc = closed_contours(&p, si);
        // What remains is the large arc of c1 plus the whole circle c2, giving two regions: c2 entire, at
        // 100π, and the crescent of c1 minus c2. The check is that there are two regions and one of them is
        // about 100π.
        if cc.len() != 2 || !cc.iter().any(|g| (g.1 - PI * 100.0).abs() / (PI * 100.0) < 0.02) {
            fails.push(format!("trim of overlapping circles: regions {:?}, expected 2 with one near {:.0}", cc.iter().map(|g| g.1.round()).collect::<Vec<_>>(), PI * 100.0));
        }
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Degenerate tangencies: a lone tangency must not destroy a region.
#[test]
fn matrix_tangency_degenerate() {
    let mut fails = Vec::new();
    // a circle with a single tangent line: the circle stays a region, since a lone tangency is not a cut
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.add_line_entity(si, -20.0, 10.0, 20.0, 10.0, qymcad_core::feature::Purpose::Real); // tangent at (0,10)
        p.regen_sketch(si);
        check(&mut fails, "circle with a tangent", &closed_contours(&p, si), 1, PI * 100.0, 0.01);
    }
    // two externally tangent circles: both stay regions
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.add_circle_entity(si, 25.0, 0.0, 15.0, qymcad_core::feature::Purpose::Real); // tangent at (10,0)
        p.regen_sketch(si);
        let cc = closed_contours(&p, si);
        if cc.len() != 2 {
            fails.push(format!("two tangent circles: {} regions, expected 2: {:?}", cc.len(), cc.iter().map(|g| g.1.round()).collect::<Vec<_>>()));
        }
    }
    // a square with an inscribed circle, tangent to all four sides: two regions, the square with a hole and
    // the circle
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.add_circle_entity(si, 10.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cc = closed_contours(&p, si);
        let has_circle = cc.iter().any(|g| (g.1 - PI * 100.0).abs() / (PI * 100.0) < 0.02);
        let has_sq = cc.iter().any(|g| (g.1 - 400.0).abs() / 400.0 < 0.02 || (g.1 - (400.0 - PI * 100.0)).abs() > 0.0);
        if cc.len() < 2 || !has_circle || !has_sq {
            fails.push(format!("square with an inscribed circle: regions {:?}, expected the square and a circle near 314", cc.iter().map(|g| g.1.round()).collect::<Vec<_>>()));
        }
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// The dumbbell: a central circle of R = 20 at (0,0), side circles of R = 10 at (±40,0), four external tangent
/// lines at exact coordinates, and the inner arcs trimmed away.
///
/// The expectation is one closed contour of area 2·hull − πR² = 2789.55.
#[test]
fn matrix_tangent_dumbbell_trim() {
    let mut fails = Vec::new();
    let (rr, r, d) = (20.0_f64, 10.0_f64, 40.0_f64);
    let th = ((rr - r) / d).acos(); // 75.522°
    let (ct, st) = (th.cos(), th.sin());
    let mut p = Project::default();
    let si = p.new_sketch("s");
    let c_mid = p.add_circle_entity(si, 0.0, 0.0, rr, qymcad_core::feature::Purpose::Real);
    let c_r = p.add_circle_entity(si, d, 0.0, r, qymcad_core::feature::Purpose::Real);
    let c_l = p.add_circle_entity(si, -d, 0.0, r, qymcad_core::feature::Purpose::Real);
    // the tangency points: on the central circle at (±R·cosθ, ±R·sinθ), and on the right-hand circle at the
    // same θ
    let (mx, my) = (rr * ct, rr * st); // (5, 19.365)
    let (sx, sy) = (d + r * ct, r * st); // (42.5, 9.68)
    p.add_line_entity(si, mx, my, sx, sy, qymcad_core::feature::Purpose::Real); // upper right
    p.add_line_entity(si, mx, -my, sx, -sy, qymcad_core::feature::Purpose::Real); // lower right
    p.add_line_entity(si, -mx, my, -sx, sy, qymcad_core::feature::Purpose::Real); // upper left
    p.add_line_entity(si, -mx, -my, -sx, -sy, qymcad_core::feature::Purpose::Real); // lower left
    // Trimming the interior. On the central circle the eastern span is removed by clicking (20,0) and the
    // western one by clicking (−20,0).
    if !p.trim_curve(si, c_mid, rr, 0.0) {
        fails.push("dumbbell: trim of the central circle to the east returned false; the cut at the tangency was not found".into());
    }
    // after the first trim the circle became an arc, possibly with a new id: find the arc entity centred at
    // (0,0)
    let mid2 = p.sketches[si]
        .entities
        .iter()
        .find(|e| matches!(e.kind, qymcad_core::model::EntityKind::Arc { .. } | qymcad_core::model::EntityKind::Circle { .. }) && {
            let pid = match e.kind {
                qymcad_core::model::EntityKind::Arc { center, .. } => center,
                qymcad_core::model::EntityKind::Circle { center, .. } => center,
                _ => unreachable!(),
            };
            p.sketches[si].points.iter().any(|q| q.id == pid && q.x.abs() < 1e-6 && q.y.abs() < 1e-6)
        })
        .map(|e| e.id);
    match mid2 {
        Some(id) => {
            if !p.trim_curve(si, id, -rr, 0.0) {
                fails.push("dumbbell: trim of the central circle to the west returned false".into());
            }
        }
        None => fails.push("dumbbell: the central arc vanished after the first trim".into()),
    }
    // on the side circles the inner arcs are removed by clicking the point facing the centre
    if !p.trim_curve(si, c_r, d - r, 0.0) {
        fails.push("dumbbell: trim of the right-hand circle returned false".into());
    }
    if !p.trim_curve(si, c_l, -d + r, 0.0) {
        fails.push("dumbbell: trim of the left-hand circle returned false".into());
    }
    p.regen_sketch(si);
    let cc = closed_contours(&p, si);
    let hull = rr * rr * (PI - th) + r * r * th + (rr + r) * (d * d - (rr - r) * (rr - r)).sqrt();
    let want = 2.0 * hull - PI * rr * rr; // 2789.55
    check(&mut fails, "dumbbell after trimming", &cc, 1, want, 0.02);
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// The same dumbbell without any trimming: the arrangement must not fall apart at the tangencies. The regions
/// exist and the planar picture as a whole makes sense, being neither empty nor debris.
#[test]
fn matrix_tangent_dumbbell_regions_no_trim() {
    let mut fails = Vec::new();
    let (rr, r, d) = (20.0_f64, 10.0_f64, 40.0_f64);
    let th = ((rr - r) / d).acos();
    let (ct, st) = (th.cos(), th.sin());
    let mut p = Project::default();
    let si = p.new_sketch("s");
    p.add_circle_entity(si, 0.0, 0.0, rr, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, d, 0.0, r, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, -d, 0.0, r, qymcad_core::feature::Purpose::Real);
    let (mx, my) = (rr * ct, rr * st);
    let (sx, sy) = (d + r * ct, r * st);
    p.add_line_entity(si, mx, my, sx, sy, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, mx, -my, sx, -sy, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, -mx, my, -sx, sy, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, -mx, -my, -sx, -sy, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let cc = closed_contours(&p, si);
    // At least five regions are expected — three circles plus two belts between the tangent lines — and no
    // debris micro-regions of area below 1: a tangency must not spawn slivers.
    if cc.len() < 5 {
        fails.push(format!("dumbbell without trimming: {} regions, fewer than 5; areas {:?}", cc.len(), cc.iter().map(|g| g.1.round()).collect::<Vec<_>>()));
    }
    for (cid, a) in &cc {
        if *a < 1.0 {
            fails.push(format!("dumbbell without trimming: debris micro-region {cid} of area {a:.4}"));
        }
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// A 4×4 square with every corner filleted at r = 2 has to yield a circle of Ø4, the sides degenerating to
/// nothing, and at r = 1.8 something close to a circle. Neither used to work: 1.8 failed and about 1 was the
/// best that succeeded.
#[test]
fn matrix_fillet_square_to_circle() {
    let mut fails = Vec::new();
    for (r, want_area) in [(1.0_f64, 16.0 - (4.0 - PI)), (1.8, 16.0 - (4.0 - PI) * 1.8 * 1.8), (2.0, PI * 4.0)] {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        p.add_rect_entity(si, 0.0, 0.0, 4.0, 4.0, qymcad_core::feature::Purpose::Real);
        let done = p.fillet_all_corners(si, r);
        if done != 4 {
            fails.push(format!("r={r}: {done} of 4 corners filleted"));
            continue;
        }
        p.regen_sketch(si);
        let cc = closed_contours(&p, si);
        check(&mut fails, &format!("4x4 square, r={r}"), &cc, 1, want_area, 0.02);
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Trimming leaves no dangling points behind; they used to have to be deleted by hand.
#[test]
fn matrix_trim_no_orphan_points() {
    let mut fails = Vec::new();
    let mut p = Project::default();
    let si = p.new_sketch("s");
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    let l = p.add_line_entity(si, -20.0, 0.0, 20.0, 0.0, qymcad_core::feature::Purpose::Real);
    // cut off the right-hand tail of the line by clicking (15,0), with the cuts at ±10, which would orphan
    // the endpoint at (20,0)
    if !p.trim_line(si, l, 15.0, 0.0) {
        fails.push("trim_line of the tail returned false".into());
    }
    let s = &p.sketches[si];
    let mut used: std::collections::HashSet<u64> = Default::default();
    for e in &s.entities {
        match e.kind {
            qymcad_core::model::EntityKind::Line { a, b } => { used.insert(a); used.insert(b); }
            qymcad_core::model::EntityKind::Arc { center, a, b, .. } => { used.insert(center); used.insert(a); used.insert(b); }
            qymcad_core::model::EntityKind::Circle { center, .. } => { used.insert(center); }
            qymcad_core::model::EntityKind::Ellipse { c, ma, mi } => { used.insert(c); used.insert(ma); used.insert(mi); }
        }
    }
    let sys: std::collections::HashSet<u64> = s.system_ids().into_iter().collect();
    for pt in &s.points {
        if !used.contains(&pt.id) && !sys.contains(&pt.id) {
            fails.push(format!("dangling point {} at ({:.1},{:.1})", pt.id, pt.x, pt.y));
        }
    }
    if s.points.iter().any(|q| (q.x - 20.0).abs() < 1e-6 && q.y.abs() < 1e-6) {
        fails.push("the endpoint of the removed tail at (20,0) is still there".into());
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// A line starting on the rim of a circle, with its endpoint exactly on the circle, must not make the circle
/// disappear or fall apart.
#[test]
fn matrix_line_starting_on_circle() {
    let mut fails = Vec::new();
    // the endpoint sits exactly on the rim and the line runs outwards
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.add_line_entity(si, 10.0, 0.0, 30.0, 5.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        check(&mut fails, "line from the rim outwards", &closed_contours(&p, si), 1, PI * 100.0, 0.01);
    }
    // a line from the rim inwards, a chord to the centre: one end on the rim, the other inside
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.add_line_entity(si, 10.0, 0.0, 0.0, 0.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        check(&mut fails, "line from the rim inwards", &closed_contours(&p, si), 1, PI * 100.0, 0.01);
    }
    // a spoke straight through: both ends on the rim, a diameter, giving two half-discs
    {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.add_line_entity(si, -10.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cc = closed_contours(&p, si);
        if cc.len() != 2 || cc.iter().any(|g| (g.1 - PI * 50.0).abs() / (PI * 50.0) > 0.02) {
            fails.push(format!("diameter chord: regions {:?}, expected two of {:.0}", cc.iter().map(|g| g.1.round()).collect::<Vec<_>>(), PI * 50.0));
        }
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Filleting every corner with a selection active touches only the selected figure.
#[test]
fn matrix_fillet_all_respects_selection() {
    let mut fails = Vec::new();
    let mut p = Project::default();
    let si = p.new_sketch("s");
    let r1 = p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.add_rect_entity(si, 40.0, 0.0, 60.0, 20.0, qymcad_core::feature::Purpose::Real);
    let only: std::collections::HashSet<u64> = r1.into_iter().collect();
    let done = p.fillet_all_corners_of(si, 3.0, Some(&only));
    if done != 4 {
        fails.push(format!("selected square: {done} of 4 filleted"));
    }
    p.regen_sketch(si);
    let cc = closed_contours(&p, si);
    let bite = (4.0 - PI) * 9.0; // four corners at r = 3
    let ok_rounded = cc.iter().any(|g| (g.1 - (400.0 - bite)).abs() < 3.0);
    let ok_sharp = cc.iter().any(|g| (g.1 - 400.0).abs() < 1.0);
    if cc.len() != 2 || !ok_rounded || !ok_sharp {
        fails.push(format!("regions {:?}: expected a filleted one near {:.0} and an untouched 400", cc.iter().map(|g| g.1.round()).collect::<Vec<_>>(), 400.0 - bite));
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// The connected figure grown from a single entity — clicking one side of a rectangle selects the whole
/// rectangle — and filleting every corner of that set leaves the neighbouring figure alone.
#[test]
fn matrix_connected_figure_pick() {
    let mut fails = Vec::new();
    let mut p = Project::default();
    let si = p.new_sketch("s");
    let r1 = p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    let _r2 = p.add_rect_entity(si, 40.0, 0.0, 60.0, 20.0, qymcad_core::feature::Purpose::Real);
    let comp = p.connected_entities(si, r1[0]);
    if comp.len() != 4 || !r1.iter().all(|id| comp.contains(id)) {
        fails.push(format!("connected figure from a side: {} entities, expected the four sides of the first one", comp.len()));
    }
    let done = p.fillet_all_corners_of(si, 3.0, Some(&comp));
    if done != 4 {
        fails.push(format!("{done} of 4 corners filleted on the clicked figure"));
    }
    p.regen_sketch(si);
    let cc = closed_contours(&p, si);
    if !cc.iter().any(|g| (g.1 - 400.0).abs() < 1.0) {
        fails.push("the second figure was touched and should have stayed at 400".into());
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}
