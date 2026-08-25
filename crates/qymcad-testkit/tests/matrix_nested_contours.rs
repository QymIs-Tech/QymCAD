//! MATRIX OF NESTED CONTOURS. Reported behaviour: three nested rectangles could not be extruded with
//! a hole in the middle. The contract: EVERY selected contour is its own REGION (itself minus its
//! DIRECT children), and the regions merge. Checked at FOUR levels of nesting against real OCCT
//! volumes — depth must change nothing. Failures accumulate.
use qymcad_core::model::Project;

const H: f64 = 2.0;

/// Four concentric squares: A 100x100, B 80x80, C 60x60, D 40x40 (in decreasing area).
fn scene() -> (Project, u64, [u64; 4]) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("nested");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "nested");
    for half in [50.0, 40.0, 30.0, 20.0] {
        p.add_rect_entity(si, -half, -half, half, half, qymcad_core::feature::Purpose::Real);
    }
    p.regen_sketch(si);
    let mut closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    closed.sort_by(|a, b| {
        let ar = |c: &u64| p.contour_index(*c).map(|i| p.contours[i].area()).unwrap_or(0.0);
        ar(b).partial_cmp(&ar(a)).unwrap() // decreasing: A, B, C, D
    });
    assert_eq!(closed.len(), 4, "four nested contours");
    (p, sid, [closed[0], closed[1], closed[2], closed[3]])
}

#[test]
fn matrix_nested_selection_any_depth() {
    let (p0, sid, [a, b, c, d]) = scene();
    let (sa, sb, sc, sd) = (10000.0, 6400.0, 3600.0, 1600.0);
    // (what is selected, expected area of material, label)
    let cases: Vec<(Vec<u64>, f64, &str)> = vec![
        (vec![a], sa - sb, "outer only -> the ring A minus B"),
        (vec![a, b], sa - sc, "outer plus middle -> a plate with a hole at C (THE REPORTED CASE)"),
        (vec![a, b, c], sa - sd, "three levels -> a plate with a hole at the smallest"),
        (vec![a, b, c, d], sa, "all four -> a solid plate"),
        (vec![a, c], (sa - sb) + (sc - sd), "skipping a level -> two independent rings"),
        (vec![d], sd, "the smallest only -> a solid square"),
        (vec![b, c], (sb - sc) + (sc - sd), "two inner levels -> the ring B minus D"),
    ];
    let mut fails: Vec<String> = Vec::new();
    for (sel, area, label) in cases {
        let mut p = p0.clone();
        let e = p.add_extrude_multi(sid, sel.clone(), H, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let body = p.finish_base_body(e, 1);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        for (id, er) in &report.errors {
            fails.push(format!("{label}: ERROR {id}: {er}"));
        }
        let v = shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
        let exp = area * H;
        if exp <= 0.0 || ((v - exp) / exp).abs() > 0.01 {
            fails.push(format!("{label}: V={v:.1}, expected {exp:.1}"));
        }
    }
    assert!(fails.is_empty(), "\nNESTING MATRIX FAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Older files (a `fill` mark on the outer contour instead of a selection of its own) must repair
/// THEMSELVES: `fill` is read as "this contour is selected too". Exactly the reported case, where the
/// node is saved as profiles=[outer], fill=[middle] and used to give a solid slab.
#[test]
fn legacy_fill_is_read_as_selection() {
    let (mut p, sid, [a, b, _c, _d]) = scene();
    let e = p.add_extrude_multi(sid, vec![a], H, qymcad_core::feature::Reach::Forward, 0.0, vec![b]); // the old format
    let body = p.finish_base_body(e, 1);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "regen errors: {:?}", report.errors);
    let v = shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
    let exp = (10000.0 - 3600.0) * H; // a plate with a hole at C, NOT a solid slab of 10000*H
    assert!((v - exp).abs() / exp < 0.01, "legacy fill: V={v:.1}, expected {exp:.1} (a solid slab would be {:.1})", 10000.0 * H);
}
