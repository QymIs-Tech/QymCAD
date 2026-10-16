//! A STAIRCASE OF LIMITING BLENDS KEEPS ITS NAMES. Three steps 10 high, 50 deep: the first chamfered 10 x 10, the second
//! rounded R10, the third chamfered 10 x 30 - each eats its faces whole, is refused by OCCT at its size and builds a
//! hair smaller, leaving a strip the door takes out - then R2 on the line of the first chamfer at the near end, and R2
//! on the arc of the R10 there. Reported behaviour, on such a part made in the window: a preview lit the part almost
//! all over, the last rounding rebuilt for minutes into a body of 3.6e6 mm^3 reaching 590 mm out of 4.9e4, and with the
//! depth of the part edited the roundings above lost their edges. The strips were taken out with the names carried by
//! where things lay, to 1e-6, and the ends of the part - their corner cut by each blend - were renamed by every blend.
use qymcad_core::feature::{ChamferMode, Reach};
use qymcad_core::geom::Point2;
use qymcad_core::model::{ChamferShape, Project};

/// The depth of the staircase, as built and as edited.
const DEPTH: f64 = 50.0;
const DEEPER: f64 = 60.0;

/// The id of the edge of `body` whose middle stands nearest `at`, within 0.2 mm.
fn edge_near(p: &Project, body: u64, at: [f64; 3]) -> Option<u32> {
    let d = |m: [f64; 3]| (0..3).map(|i| (m[i] - at[i]).powi(2)).sum::<f64>().sqrt();
    p.regen_edges.get(&body)?.iter().filter(|e| d(e.mid) < 0.2).min_by(|a, b| d(a.mid).total_cmp(&d(b.mid))).map(|e| e.id)
}

/// The names of the two ends of `body`: the faces facing -Z and +Z.
fn ends(p: &Project, body: u64) -> [Option<u32>; 2] {
    let end = |nz: f64| p.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| (f.normal[2] - nz).abs() < 1e-6).map(|f| f.id));
    [end(-1.0), end(1.0)]
}

/// The staircase and its blends, each laid on the edge standing where the report has it; the bodies in order.
fn the_staircase(p: &mut Project) -> Result<Vec<u64>, String> {
    p.new_document();
    let pts = [(0.0, 0.0), (50.0, 0.0), (50.0, 30.0), (20.0, 30.0), (20.0, 20.0), (10.0, 20.0), (10.0, 10.0), (0.0, 10.0)].iter().map(|&(x, y)| Point2::new(x, y)).collect();
    let sketch = p.add_line_sketch("Steps", pts, true);
    let si = p.sketch_index(sketch).ok_or("the sketch")?;
    p.regen_sketch(si);
    p.add_sketch_node(sketch, "Steps");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let mut bodies = vec![p.add_extrude_multi(sketch, closed, DEPTH, Reach::Forward, 0.0, vec![])];
    let _ = qymcad_testkit::regenerate(p);
    let two_legs = |d2: f64| ChamferShape { mode: ChamferMode::TwoDist, d2, flip: false, ref_face: 0 };
    let steps: [(&str, [f64; 3]); 5] = [
        ("the first step chamfered 10 x 10", [0.0, 10.0, DEPTH / 2.0]),
        ("the second step rounded R10", [10.0, 20.0, DEPTH / 2.0]),
        ("the third step chamfered 10 x 30", [20.0, 30.0, DEPTH / 2.0]),
        ("the line of the first chamfer rounded R2 at the near end", [5.0, 5.0, DEPTH]),
        ("the arc of the R10 rounded R2 at the near end", [12.93, 17.07, DEPTH]),
    ];
    for (k, (what, at)) in steps.iter().enumerate() {
        let src = *bodies.last().ok_or("a body")?;
        let e = edge_near(p, src, *at).ok_or_else(|| format!("setup: {what} - no edge stands at {at:?}"))?;
        let node = match k {
            0 => p.add_chamfer_ex(src, 10.0, two_legs(10.0), vec![e]),
            1 => p.add_fillet(src, 10.0, vec![e]),
            2 => [false, true]
                .into_iter()
                .map(|flip| {
                    let mut q = p.clone();
                    let n = q.add_chamfer_ex(src, 10.0, ChamferShape { flip, ..two_legs(30.0) }, vec![e]);
                    let _ = qymcad_testkit::regenerate(&mut q);
                    (flip, n, q.regen_errors.contains_key(&n))
                })
                .find(|(_, _, red)| !red)
                .map(|(flip, _, _)| p.add_chamfer_ex(src, 10.0, ChamferShape { flip, ..two_legs(30.0) }, vec![e]))
                .ok_or("setup: the third step takes no chamfer 10 x 30 either way")?,
            _ => p.add_fillet(src, 2.0, vec![e]),
        };
        let _ = qymcad_testkit::regenerate(p);
        if k < 4 && p.regen_errors.contains_key(&node) {
            return Err(format!("setup: {what} refused - {:?}", p.regen_errors.get(&node)));
        }
        bodies.push(node);
    }
    Ok(bodies)
}

#[test]
fn the_ends_keep_their_names_through_every_limiting_blend() {
    let mut p = Project::default();
    let bodies = the_staircase(&mut p).unwrap_or_else(|e| panic!("{e}"));
    let mut fails: Vec<String> = Vec::new();
    let first = ends(&p, bodies[0]);
    // the last rounding may refuse: its body is not asked about
    for pair in bodies[..bodies.len() - 1].windows(2) {
        let (src, b) = (pair[0], pair[1]);
        if ends(&p, b) != first {
            fails.push(format!("node {b}: the ends are {:?}, they were {:?} on the extrusion", ends(&p, b).map(|e| e.map(|x| p.names.describe(x))), first.map(|e| e.map(|x| p.names.describe(x)))));
        }
        // what a preview lights: the faces of the node its source does not have
        let had: std::collections::HashSet<u32> = p.regen_faces.get(&src).map(|fs| fs.iter().map(|f| f.id).collect()).unwrap_or_default();
        let fresh = p.regen_faces.get(&b).map(|fs| fs.iter().filter(|f| !had.contains(&f.id)).count()).unwrap_or(0);
        if fresh > 3 {
            fails.push(format!("node {b}: {fresh} faces are new to it - a preview lights all of them"));
        }
    }
    assert!(fails.is_empty(), "\nNAMES ON A STAIRCASE OF LIMITING BLENDS ({}):\n{}", fails.len(), fails.join("\n"));
}

#[test]
fn the_last_rounding_lands_inside_its_source_or_says_why_at_once() {
    let mut p = Project::default();
    let bodies = the_staircase(&mut p).unwrap_or_else(|e| panic!("{e}"));
    let (src, last) = (bodies[bodies.len() - 2], bodies[bodies.len() - 1]);
    let t = std::time::Instant::now();
    let _ = qymcad_testkit::regenerate(&mut p);
    let took = t.elapsed().as_secs_f64();
    if !p.regen_errors.contains_key(&last) {
        let bounds = |b: u64| p.mesh_index(b).and_then(|i| p.bodies[i].mesh.bounds()).map(|x| [x.min.x, x.min.y, x.min.z, x.max.x, x.max.y, x.max.z]).expect("a mesh");
        let (a, b) = (bounds(src), bounds(last));
        let out = (0..3).map(|i| (a[i] - b[i]).max(b[i + 3] - a[i + 3])).fold(0.0f64, f64::max);
        assert!(out <= 2.0 + 1e-3, "the last rounding of R2 landed a body {out:.1} mm past its source: {b:?} from {a:?}");
    }
    assert!(took < 10.0, "the staircase rebuilt in {took:.1} s");
}

#[test]
fn a_deeper_staircase_keeps_every_reference() {
    let mut p = Project::default();
    let bodies = the_staircase(&mut p).unwrap_or_else(|e| panic!("{e}"));
    for n in &mut p.timeline {
        if let qymcad_core::feature::FeatureKind::Extrude { height, .. } = &mut n.kind {
            *height = DEEPER;
        }
    }
    let _ = qymcad_testkit::regenerate(&mut p);
    let lost: Vec<String> = bodies
        .iter()
        .filter_map(|b| p.regen_errors.get(b).map(|e| (b, e)))
        .filter(|(_, e)| matches!(e, qymcad_core::errors::CoreError::EdgesNotFound { .. } | qymcad_core::errors::CoreError::SourceBodyNotBuilt))
        .map(|(b, e)| format!("node {b}: {e:?}"))
        .collect();
    assert!(lost.is_empty(), "made {DEEPER} deep, the blends lost their edges:\n{}", lost.join("\n"));
}
