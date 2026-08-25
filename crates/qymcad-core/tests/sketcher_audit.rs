//! An audit of the sketcher: reproduction tests that pin down the defects found in it.
//!
//! Every test states the expected, correct behaviour, so a failure here means the defect is real.
use qymcad_core::model::{constraint_point_ids, Constraint, EntityKind, Project};

fn new_sketch() -> (Project, usize) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("t", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    (p, si)
}

fn centers(p: &Project, si: usize) -> Vec<u64> {
    p.sketches[si].entities.iter().filter_map(|e| match e.kind {
        EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => Some(center),
        _ => None,
    }).collect()
}

fn circle_r(p: &Project, si: usize) -> Option<f64> {
    p.sketches[si].entities.iter().find_map(|e| match e.kind { EntityKind::Circle { r, .. } => Some(r), _ => None })
}

// An arc placed concentric with a circle from a single point shares the centre node, and the two radii then
// collapse into one. Unlike `add_circle_entity`, `add_arc_entity` had no guard against reusing a radius
// centre.
#[test]
fn arc_concentric_with_circle_keeps_distinct_centers() {
    let (mut p, si) = new_sketch();
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_arc_entity(si, 0.0, 0.0, 5.0, 0.0, 0.0, 5.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real); // centre at the same point (0,0)
    let c = centers(&p, si);
    eprintln!("centres of circle and arc: {c:?}");
    p.solve_sketch(si);
    eprintln!("circle r after the solve: {:?}, expecting 10", circle_r(&p, si));
    assert_ne!(c[0], c[1], "an arc and a circle have to own distinct centre nodes, or their radii collapse");
}

// The same class of defect: a polygon or a slot placed concentric with a circle from one point. With a shared
// centre node the radius variable of the circle collapses onto the circumscribed radius of the polygon or onto
// the radius of the slot end.
#[test]
fn polygon_concentric_with_circle_keeps_circle_radius() {
    let (mut p, si) = new_sketch();
    let ec = p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_polygon_entity(si, 0.0, 0.0, 5.0, 0.0, 6, qymcad_core::feature::Purpose::Real); // circumscribed circle of the polygon, r = 5 at (0,0)
    p.solve_sketch(si);
    let r = p.sketches[si].entities.iter().find_map(|e| match e.kind {
        EntityKind::Circle { center, r } if e.id == ec => Some((center, r)),
        _ => None,
    });
    eprintln!("circle (centre, r): {r:?}, expecting r = 10");
    assert!(matches!(r, Some((_, rr)) if (rr - 10.0).abs() < 1e-3), "the circle radius collapsed because it shares a centre with the polygon");
}

#[test]
fn slot_end_on_circle_center_keeps_circle_radius() {
    let (mut p, si) = new_sketch();
    let ec = p.add_circle_entity(si, 0.0, 0.0, 8.0, qymcad_core::feature::Purpose::Real);
    p.add_slot_entity(si, 0.0, 0.0, 30.0, 0.0, 4.0, qymcad_core::feature::Purpose::Real); // the slot end, an arc of r = 4, at the circle centre
    p.solve_sketch(si);
    let r = p.sketches[si].entities.iter().find_map(|e| match e.kind {
        EntityKind::Circle { r, .. } if e.id == ec => Some(r),
        _ => None,
    });
    eprintln!("circle r after the slot: {r:?}, expecting 8");
    assert!(matches!(r, Some(rr) if (rr - 8.0).abs() < 1e-3), "the circle radius collapsed because it shares a centre with the slot end");
}

// A copy of geometry carries the internal constraints and dimensions with it — horizontals, verticals, edge
// dimensions — but neither the anchor nor the dimensions to the coordinate axes, which reference points outside
// the copy.
fn build_dimensioned_rect(p: &mut Project, si: usize) -> (Vec<u64>, u64, u64) {
    let eids = p.add_rect_entity(si, 0.0, 0.0, 10.0, 6.0, qymcad_core::feature::Purpose::Real); // with automatic horizontals and verticals
    let c00 = p.sketches[si].points.iter().find(|q| q.x == 0.0 && q.y == 0.0).unwrap().id;
    let c10 = p.sketches[si].points.iter().find(|q| q.x == 10.0 && q.y == 0.0).unwrap().id;
    // a dimension on the bottom edge
    p.sketches[si].constraints.push(Constraint::Distance { a: c00, b: c10, d: 10.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    // the anchor and the dimension to the X axis must not be copied
    p.sketches[si].constraints.push(Constraint::Fixed { p: c00 });
    let (o, xax) = p.ensure_axis(si, 0);
    p.sketches[si].constraints.push(Constraint::DistancePL { p: c00, a: o, b: xax, d: 0.0, off: 0.0, expr: String::new(), driven: false });
    (eids, c00, c10)
}

fn assert_copied_constraints(p: &Project, si: usize, from_idx: usize, new_pts: &std::collections::HashSet<u64>) {
    let mut hv = 0;
    let mut dist = 0;
    for c in &p.sketches[si].constraints[from_idx..] {
        let all_new = constraint_point_ids(c).iter().all(|id| new_pts.contains(id));
        match c {
            Constraint::Fixed { .. } => panic!("the anchor was copied and must not be: it pins a position"),
            Constraint::DistancePL { .. } => panic!("a dimension to an axis was copied and must not be"),
            Constraint::Horizontal { .. } | Constraint::Vertical { .. } => {
                assert!(all_new, "a horizontal or vertical of the copy has to reference the new points");
                hv += 1;
            }
            Constraint::Distance { .. } => {
                assert!(all_new, "an edge dimension of the copy has to reference the new points");
                dist += 1;
            }
            _ => {}
        }
    }
    assert!(hv >= 1, "horizontals and verticals have to be copied");
    assert!(dist >= 1, "an edge dimension has to be copied");
}

#[test]
fn copy_entities_carries_internal_constraints_only() {
    let (mut p, si) = new_sketch();
    let (eids, _c00, _c10) = build_dimensioned_rect(&mut p, si);
    let old_pts: std::collections::HashSet<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let from = p.sketches[si].constraints.len();
    let new_eids = p.copy_entities(si, &eids, 20.0, 0.0);
    assert!(!new_eids.is_empty());
    let new_pts: std::collections::HashSet<u64> = p.sketches[si].points.iter().map(|q| q.id).filter(|id| !old_pts.contains(id)).collect();
    assert_copied_constraints(&p, si, from, &new_pts);
}

#[test]
fn clipboard_paste_carries_internal_constraints_only() {
    let (mut p, si) = new_sketch();
    let (eids, _c00, _c10) = build_dimensioned_rect(&mut p, si);
    let clip = p.copy_sketch_geometry(si, &eids, 0.0, 0.0);
    // the snapshot holds internal constraints only, without the anchor or dimensions to an axis
    assert!(clip.constraints.iter().any(|c| matches!(c, Constraint::Horizontal { .. } | Constraint::Vertical { .. })), "horizontals and verticals are in the clipboard");
    assert!(clip.constraints.iter().any(|c| matches!(c, Constraint::Distance { .. })), "the edge dimension is in the clipboard");
    assert!(!clip.constraints.iter().any(|c| matches!(c, Constraint::Fixed { .. } | Constraint::DistancePL { .. })), "the anchor and the dimension to an axis are not in the clipboard");

    let old_pts: std::collections::HashSet<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let from = p.sketches[si].constraints.len();
    let new_eids = p.paste_sketch_geometry(si, &clip, 20.0, 20.0);
    assert!(!new_eids.is_empty());
    let new_pts: std::collections::HashSet<u64> = p.sketches[si].points.iter().map(|q| q.id).filter(|id| !old_pts.contains(id)).collect();
    assert_copied_constraints(&p, si, from, &new_pts);
}

// Trimming a circle into an arc keeps the centre point in the data; if the centre appears to vanish, that is
// a matter of the interface rather than of the model.
#[test]
fn trim_circle_to_arc_keeps_center_point() {
    let (mut p, si) = new_sketch();
    let ec = p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    let center_before = p.sketches[si].entities.iter().find_map(|e| match e.kind {
        EntityKind::Circle { center, .. } if e.id == ec => Some(center),
        _ => None,
    }).unwrap();
    // a cutting line crossing the circle at two points, then the lower arc is trimmed away
    p.add_line_entity(si, -20.0, 5.0, 20.0, 5.0, qymcad_core::feature::Purpose::Real);
    let ok = p.trim_curve(si, ec, 0.0, -10.0);
    eprintln!("trim ok={ok}");
    let has_center = p.sketches[si].points.iter().any(|q| q.id == center_before);
    let arc_center = p.sketches[si].entities.iter().find_map(|e| match e.kind { EntityKind::Arc { center, .. } => Some(center), _ => None });
    eprintln!("centre present={has_center}, arc centre={arc_center:?}, original={center_before}");
    assert!(has_center, "the circle centre has to survive being trimmed into an arc");
    assert_eq!(arc_center, Some(center_before), "the arc references the original centre");
}

// `merge_close_points` did not protect the system points. At the default zoom the tolerance reaches 2.0, and
// the origin at (0,0) together with the axis points at (1,0) and (0,1) fall within it, so the axes get eaten.
#[test]
fn merge_close_points_preserves_axis_points() {
    let (mut p, si) = new_sketch();
    let _ = p.ensure_axis(si, 0);
    let _ = p.ensure_axis(si, 1);
    let sys = p.sketches[si].system_ids();
    eprintln!("system_ids: {sys:?}");
    p.merge_close_points(si, 2.0); // as the line tool does it, with the tolerance clamped to 2.0
    let alive: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    for id in &sys {
        assert!(alive.contains(id), "the system point {id}, an origin or an axis, was merged away by merge_close_points");
    }
}

// `auto_driven` did not handle `ArcLength`. `dim_redundant` does see it, but the automatic switch to a driven
// dimension never fired, so a redundant arc length stayed driving and the sketch looked falsely
// over-constrained.
#[test]
fn redundant_arclength_auto_drives() {
    let (mut p, si) = new_sketch();
    p.add_arc_entity(si, 0.0, 0.0, 10.0, 0.0, 0.0, 10.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real);
    let (arc_eid, center, a, b) = p.sketches[si].entities.iter().find_map(|e| match e.kind {
        EntityKind::Arc { center, a, b, .. } => Some((e.id, center, a, b)),
        _ => None,
    }).unwrap();
    for pid in [center, a, b] {
        p.sketches[si].constraints.push(Constraint::Fixed { p: pid });
    }
    let ci = p.ensure_arc_length(si, arc_eid).expect("arc length dim");
    p.solve_sketch(si);
    let redundant = p.dim_redundant(si, ci);
    let drove = p.auto_driven(si, ci);
    eprintln!("ArcLength: dim_redundant={redundant} auto_driven={drove}");
    assert!(redundant, "an arc length on an anchored arc is redundant");
    assert!(drove, "a redundant arc length has to become a driven dimension");
}

// `dim_redundant` left `Diameter` out of its guard and therefore always answered false for a diameter, so a
// redundant one could not be turned into a driven dimension automatically — `auto_driven` does handle diameters
// but calls `dim_redundant` first.
#[test]
fn redundant_diameter_is_detected() {
    let (mut p, si) = new_sketch();
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 40.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    let c = centers(&p, si);
    p.sketches[si].constraints.push(Constraint::EqualRadius { c1: c[0], c2: c[1] });
    let _ci1 = p.ensure_diameter(si, c[0], true).unwrap();
    let ci2 = p.ensure_diameter(si, c[1], true).unwrap();
    p.solve_sketch(si);
    let redundant = p.dim_redundant(si, ci2);
    let rank_set = p.sketch_redundant_constraints(si);
    eprintln!("diameter ci2: dim_redundant={redundant}, in the rank-based redundant set={}", rank_set.contains(&ci2));
    assert!(redundant, "a diameter made redundant through EqualRadius has to be recognised by dim_redundant");
}

// `sketch_conflicts` did not handle `EdgeDistance`, so an edge-to-edge dimension whose value contradicts the
// geometry was never flagged as a conflict.
#[test]
fn conflicting_edge_distance_is_flagged() {
    let (mut p, si) = new_sketch();
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 40.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    let c = centers(&p, si);
    // anchor the centres and the radii so that the geometry cannot satisfy the dimension
    p.sketches[si].constraints.push(Constraint::Fixed { p: c[0] });
    p.sketches[si].constraints.push(Constraint::Fixed { p: c[1] });
    p.ensure_diameter(si, c[0], true);
    p.ensure_diameter(si, c[1], true);
    // an edge-to-edge dimension with a deliberately impossible value
    p.sketches[si].constraints.push(Constraint::EdgeDistance { c1: c[0], c2: c[1], d: 999.0, m1: -1, m2: -1, off: 0.0, expr: String::new(), driven: false });
    let ci = p.sketches[si].constraints.len() - 1;
    p.solve_sketch(si);
    let conflicts = p.sketch_conflicts(si);
    eprintln!("conflicts at d = 999: {conflicts:?}, expecting it to contain {ci}");
    assert!(conflicts.contains(&ci), "an EdgeDistance that contradicts the geometry has to be flagged as a conflict");
}

// Associativity of contours: `regen_sketch` used to reuse a contour id by position. Circles come first in the
// tessellation, so adding a circle to a rectangle shifted the order of the loops and the old id of the
// rectangle slid onto the circle, leaving an extrusion attached to the wrong contour. An id now stays with its
// loop through a geometric signature of centroid and area.
fn contour_area(p: &Project, cid: u64) -> Option<f64> {
    let ci = p.contour_index(cid)?;
    let pts = &p.contours[ci].points;
    let mut a = 0.0;
    for i in 0..pts.len() {
        let (u, v) = (pts[i], pts[(i + 1) % pts.len()]);
        a += u.x * v.y - v.x * u.y;
    }
    Some((0.5 * a).abs())
}

#[test]
fn contour_id_stays_on_same_loop_after_adding_another() {
    let (mut p, si) = new_sketch();
    p.add_rect_entity(si, 100.0, 100.0, 140.0, 120.0, qymcad_core::feature::Purpose::Real); // a 40×20 rectangle of area 800
    let rid = p.sketches[si].contour_ids[0];
    let a_before = contour_area(&p, rid);
    // add a circle of area ~78.5: circles come first in the tessellation, so a positional reuse would move
    // `rid` onto the circle
    p.add_circle_entity(si, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
    let a_after = contour_area(&p, rid);
    eprintln!("area of contour rid before and after adding the circle: {a_before:?} -> {a_after:?}, expecting ~800 rather than ~78.5");
    assert!(matches!(a_after, Some(a) if (a - 800.0).abs() < 1.0), "the contour id of the rectangle still has to point at the rectangle, area ~800, and not at the circle");
}

// Tessellation of loops: at a branch point, a vertex of degree greater than two, the traversal took an
// arbitrary first unused edge, so loops meeting at a shared corner were assembled haphazardly and could merge
// or refuse to extrude. A branch now takes a consistent turn, the first edge clockwise, which traces the
// minimal face.
#[test]
fn two_squares_sharing_corner_are_two_closed_loops() {
    let (mut p, si) = new_sketch();
    p.add_rect_entity(si, 0.0, 0.0, -10.0, -10.0, qymcad_core::feature::Purpose::Real); // square A, with a corner at (0,0)
    p.add_rect_entity(si, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real); // square B, sharing that corner, giving a degree-4 vertex
    let closed: Vec<f64> = p.sketches[si]
        .contour_ids
        .iter()
        .filter_map(|&cid| {
            let ci = p.contour_index(cid)?;
            if p.contours[ci].closed { contour_area(&p, cid) } else { None }
        })
        .collect();
    eprintln!("closed areas: {closed:?}, expecting two of about 100");
    let hundreds = closed.iter().filter(|a| (**a - 100.0).abs() < 1.0).count();
    assert_eq!(hundreds, 2, "two squares sharing a corner give two closed loops of 100 each, not one merged or torn loop");
}

// Robustness of the solver: with a degenerate or near-degenerate start, where points have almost coincided —
// a common situation when a point is dragged onto its neighbour — the normal equations are poorly conditioned.
// A singular step used to `break` and leave the sketch under-solved; the damping is now raised and the solve
// continues. The invariant: the solution stays finite, with no NaN or infinity, and converges to valid
// geometry.
#[test]
fn near_degenerate_start_still_converges() {
    use qymcad_core::model::SketchPoint;
    let (mut p, si) = new_sketch();
    let mk = |p: &mut Project, x: f64, y: f64| {
        let id = p.alloc_id();
        p.sketches[si].points.push(SketchPoint { id, x, y });
        id
    };
    // three nearly coincident points, dimensioned into a 3-4-5 triangle by their edges
    let a = mk(&mut p, 0.0, 0.0);
    let b = mk(&mut p, 1e-3, 0.0);
    let c = mk(&mut p, 0.0, 1e-3);
    let s = &mut p.sketches[si];
    s.constraints.push(Constraint::Fixed { p: a });
    let dist = |a, b, d| Constraint::Distance { a, b, d, off: 0.0, expr: String::new(), driven: false, axis: 0 };
    s.constraints.push(dist(a, b, 3.0));
    s.constraints.push(dist(b, c, 4.0));
    s.constraints.push(dist(c, a, 5.0));
    let resid = p.solve_sketch(si);
    let all_finite = p.sketches[si].points.iter().all(|q| q.x.is_finite() && q.y.is_finite());
    eprintln!("residual={resid}, finite={all_finite}");
    assert!(all_finite, "the coordinates stay finite, with no NaN or infinity, from a near-degenerate start");
    assert!(resid.is_finite() && resid < 0.5, "the 3-4-5 triangle is solved through from a near-degenerate start: residual {resid}");
}

// Trimming did not anchor the cut points: after a trim the ends of the arc were free in angle and drifted on
// the next solve, which showed up as dimensions and radii wandering and the sketch twisting. The expectation is
// that the cut points stay attached to the intersecting line and remain on it even after the radius changes.
#[test]
fn trim_anchors_cut_points_to_crossing_lines() {
    let (mut p, si) = new_sketch();
    let ec = p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    let center = centers(&p, si)[0];
    // two verticals at x = ±4 crossing the circle, with their ends anchored
    let _l1 = p.add_line_entity(si, -4.0, -20.0, -4.0, 20.0, qymcad_core::feature::Purpose::Real);
    let _l2 = p.add_line_entity(si, 4.0, -20.0, 4.0, 20.0, qymcad_core::feature::Purpose::Real);
    for &(x, y) in &[(-4.0, -20.0), (-4.0, 20.0), (4.0, -20.0), (4.0, 20.0)] {
        let id = p.sketches[si].points.iter().find(|q| (q.x - x).abs() < 1e-6 && (q.y - y).abs() < 1e-6).unwrap().id;
        p.sketches[si].constraints.push(Constraint::Fixed { p: id });
    }
    p.sketches[si].constraints.push(Constraint::Fixed { p: center });
    let di = p.ensure_diameter(si, center, false).unwrap(); // a radius dimension, d = 10
    p.solve_sketch(si);
    assert!(p.trim_curve(si, ec, 0.0, 10.0), "trim away the upper sector");
    // change the radius from 10 to 14: the attached arc ends have to travel to the new intersections at x = ±4
    if let Constraint::Diameter { d, .. } = &mut p.sketches[si].constraints[di] {
        *d = 14.0;
    }
    p.solve_sketch(si);
    let ends: Vec<(f64, f64)> = p.sketches[si].entities.iter().filter_map(|e| match e.kind {
        EntityKind::Arc { a, b, .. } => Some((a, b)),
        _ => None,
    }).flat_map(|(a, b)| [a, b]).filter_map(|id| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y))).collect();
    eprintln!("arc ends after the radius change to 14: {ends:?}, expecting |x| ≈ 4");
    for (x, _) in &ends {
        assert!((x.abs() - 4.0).abs() < 0.2, "an arc end drifted off the vertical: x={x:.2}, expecting ±4, so the cut point is not attached");
    }
}

// Trimming a pie: a circle crossed by several lines. Clicking one sector has to remove that sector alone,
// leaving the remainder of the circle as a single arc rather than scattering it into an arc per intersection.
#[test]
fn trim_one_sector_leaves_rest_as_single_arc() {
    let (mut p, si) = new_sketch();
    let ec = p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    // three lines through the centre, like spokes, giving six intersections with the circle at 0°, 45°, 90°,
    // 180°, 225° and 270°
    p.add_line_entity(si, -15.0, 0.0, 15.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 0.0, -15.0, 0.0, 15.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, -15.0, -15.0, 15.0, 15.0, qymcad_core::feature::Purpose::Real);
    // click the arc between 0° and 45°, at about 20°, to remove that sector only
    assert!(p.trim_curve(si, ec, 10.0 * 0.94, 10.0 * 0.34), "remove a single sector");
    let arcs = p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).count();
    eprintln!("arcs after removing one sector: {arcs}, expecting 1");
    assert_eq!(arcs, 1, "the remainder of the circle is one arc rather than many ({arcs})");
}

// Associativity after a trim, as with a fillet: a dimension placed before the trim survives it and keeps
// driving the geometry, and the geometry itself does not move when the trim happens.
#[test]
fn trim_keeps_dims_and_geometry_associative() {
    let (mut p, si) = new_sketch();
    let ec = p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    let center = centers(&p, si)[0];
    p.sketches[si].constraints.push(Constraint::Fixed { p: center });
    p.ensure_diameter(si, center, false); // a radius dimension of r = 10, placed before the trim
    p.add_line_entity(si, -4.0, -20.0, -4.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 4.0, -20.0, 4.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.solve_sketch(si);
    let arc_or_circle_r = |p: &Project| -> f64 {
        p.sketches[si].entities.iter().find_map(|e| match e.kind {
            EntityKind::Circle { r, .. } => Some(r),
            EntityKind::Arc { center: c, a, .. } => {
                let g = |id| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
                match (g(c), g(a)) {
                    (Some((cx, cy)), Some((ax, ay))) => Some(((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt()),
                    _ => None,
                }
            }
            _ => None,
        }).unwrap_or(0.0)
    };
    assert!(p.trim_curve(si, ec, 0.0, 10.0), "trim away the upper sector");
    p.solve_sketch(si);
    // the radius dimension survived with its value of 10
    let dim = p.sketches[si].constraints.iter().find_map(|c| match c {
        Constraint::Diameter { c: cc, d, .. } if *cc == center => Some(*d),
        _ => None,
    });
    eprintln!("radius dimension after the trim: {dim:?}, expecting 10");
    assert!(matches!(dim, Some(d) if (d - 10.0).abs() < 1e-6), "the radius dimension survived with a value of 10");
    // the geometry did not move: the radius is still 10
    assert!((arc_or_circle_r(&p) - 10.0).abs() < 0.05, "the radius did not move after the trim: {}", arc_or_circle_r(&p));
    // change the radius dimension and the geometry follows, so associativity is intact
    for c in &mut p.sketches[si].constraints {
        if let Constraint::Diameter { c: cc, d, .. } = c {
            if *cc == center {
                *d = 13.0;
            }
        }
    }
    p.solve_sketch(si);
    assert!((arc_or_circle_r(&p) - 13.0).abs() < 0.05, "the dimension drives the geometry after the trim: r={}", arc_or_circle_r(&p));
}

// A constraint that cannot be evaluated is rejected by the solver rather than accepted silently.
//
// `PointOnCircle { p, c }` needs the radius of the circle centred at `c`. When `c` is not a centre, as in a
// damaged file or under an outside caller, there is no radius and the residual degenerated into "distance minus
// the same distance" = 0: the constraint was accepted, held nothing, and the degrees of freedom did not drop,
// so the point looked constrained. The interface never creates such a constraint, but the safety of the solver
// must not rest on the discipline of the interface.
#[test]
fn point_on_circle_with_noncenter_is_noop() {
    let (mut p, si) = new_sketch();
    // a line, whose two endpoints are not circle centres
    p.add_line_entity(si, 0.0, 0.0, 20.0, 0.0, qymcad_core::feature::Purpose::Real);
    let pts: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let c = pts[0]; // an endpoint of the line, not a circle centre
    // a free point p
    let pid = p.sketch_point_at(si, 5.0, 5.0, 1e-9);
    let (dof_before, _) = p.sketch_dof(si);
    p.sketches[si].constraints.push(Constraint::PointOnCircle { p: pid, c });
    let (dof_after, _) = p.sketch_dof(si);
    eprintln!("degrees of freedom without and with PointOnCircle on a non-centre: {dof_before} -> {dof_after}");
    let bad = p.sketch_conflicts(si);
    assert!(bad.contains(&(p.sketches[si].constraints.len() - 1)), "a constraint that cannot be evaluated has to be visible rather than dropped quietly");
    assert_eq!(dof_after, dof_before, "a constraint against a non-centre must not pretend to constrain anything");
}
