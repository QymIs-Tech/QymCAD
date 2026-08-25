//! MATRIX: extrude/cut/intersect x extent x profile sets — against the REAL OCCT kernel.
//! Every combination is checked by exact volume. All failures accumulate and are reported AT ONCE.
//! Project rule: the matrix grows with every new tool and every fixed defect, so nothing repeats.

use qymcad_core::model::Project;

const PI: f64 = std::f64::consts::PI;

/// Profile scenarios: name, area of the region(s) when everything is selected, sketch constructor.
/// Returns (project, sid, closed contours sorted by area).
fn scene(which: &str) -> (Project, u64, Vec<u64>) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("Sketch");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Sketch");
    match which {
        // a single 30x30 square
        "rect" => {
            p.add_rect_entity(si, 0.0, 0.0, 30.0, 30.0, qymcad_core::feature::Purpose::Real);
        }
        // a 40x40 square with a d20 round island in the middle (contour inside a contour)
        "rect_hole" => {
            p.add_rect_entity(si, 0.0, 0.0, 40.0, 40.0, qymcad_core::feature::Purpose::Real);
            p.add_circle_entity(si, 20.0, 20.0, 10.0, qymcad_core::feature::Purpose::Real);
        }
        // three concentric circles R=30/20/10
        "rings" => {
            p.add_circle_entity(si, 0.0, 0.0, 30.0, qymcad_core::feature::Purpose::Real);
            p.add_circle_entity(si, 0.0, 0.0, 20.0, qymcad_core::feature::Purpose::Real);
            p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
        }
        // two DISJOINT squares (a multi-profile in one node)
        "two_rects" => {
            p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
            p.add_rect_entity(si, 40.0, 0.0, 60.0, 20.0, qymcad_core::feature::Purpose::Real);
        }
        _ => unreachable!(),
    }
    p.regen_sketch(si);
    let mut closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    closed.sort_by(|a, b| {
        let ar = |c: &u64| p.contour_index(*c).map(|i| p.contours[i].area()).unwrap_or(0.0);
        ar(a).partial_cmp(&ar(b)).unwrap()
    });
    (p, sid, closed)
}

/// Area when ALL regions of the scene are selected. Selecting every region, nested ones included,
/// means their union is a SOLID shape — the island is "filled" and there is no hole. Held by this
/// matrix.
fn scene_area(which: &str) -> f64 {
    match which {
        "rect" => 900.0,
        "rect_hole" => 1600.0, // outer + island selected -> a solid plate
        "rings" => PI * 900.0, // all regions = the full R30 disc
        "two_rects" => 800.0,
        _ => unreachable!(),
    }
}

/// Run one combination, recording a description on mismatch.
#[allow(clippy::too_many_arguments)]
fn run_case(fails: &mut Vec<String>, label: &str, v_got: f64, v_exp: f64, tol: f64) {
    if v_exp == 0.0 {
        if v_got.abs() > 1e-6 {
            fails.push(format!("{label}: expected NOTHING, got V={v_got:.1}"));
        }
        return;
    }
    if ((v_got - v_exp) / v_exp).abs() > tol {
        fails.push(format!("{label}: V={v_got:.1}, expected {v_exp:.1}"));
    }
}

/// Base: extrude ALL contours of the scene by h -> volume = area * h. Every scene x extent.
#[test]
fn matrix_base_extrude() {
    let mut fails: Vec<String> = Vec::new();
    for which in ["rect", "rect_hole", "rings", "two_rects"] {
        use qymcad_core::feature::Reach;
        for (ext, reach, down, h_eff) in [
            ("length", Reach::Forward, 0.0, 10.0),   // one side, 10
            ("sym", Reach::BothWays, 0.0, 10.0),     // both ways 10 (+-5) — the total volume is the same
            ("two", Reach::Forward, 4.0, 14.0),      // 10 up + 4 down
        ] {
            let (mut p, sid, closed) = scene(which);
            let e = p.add_extrude_multi(sid, closed.clone(), 10.0, reach, down, vec![]);
            let body = p.finish_base_body(e, 1);
            let (report, shapes) = qymcad_testkit::regenerate(&mut p);
            let label = format!("base {which}/{ext}");
            for (id, er) in &report.errors {
                fails.push(format!("{label}: REGEN ERROR {id}: {er}"));
            }
            let v = shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
            run_case(&mut fails, &label, v, scene_area(which) * h_eff, 0.02);
        }
    }
    assert!(fails.is_empty(), "\nMATRIX FAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Cut/boss/intersect over a base plate: every scene acts as a TOOL against a 100x100x20 plate.
#[test]
fn matrix_ops_on_plate() {
    let mut fails: Vec<String> = Vec::new();
    for which in ["rect", "rect_hole", "rings", "two_rects"] {
        // a 100x100x20 plate from its own sketch (the tool region always sits inside the plate)
        for (opname, op) in [("cut", 0u8), ("boss", 1u8), ("intersect", 2u8)] {
            let mut p = Project::default();
            p.new_document();
            let bsi = p.new_sketch("Base");
            let bsid = p.sketches[bsi].id;
            p.add_sketch_node(bsid, "Base");
            p.add_rect_entity(bsi, -20.0, -20.0, 80.0, 80.0, qymcad_core::feature::Purpose::Real);
            p.regen_sketch(bsi);
            let bcid = p.sketches[bsi].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
            let e = p.add_extrude_multi(bsid, vec![bcid], 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
            let plate = p.finish_base_body(e, 1);
            // the tool scene on the SAME plane
            let si = p.new_sketch("Tool");
            let sid = p.sketches[si].id;
            p.add_sketch_node(sid, "Tool");
            match which {
                "rect" => {
                    p.add_rect_entity(si, 0.0, 0.0, 30.0, 30.0, qymcad_core::feature::Purpose::Real);
                }
                "rect_hole" => {
                    p.add_rect_entity(si, 0.0, 0.0, 40.0, 40.0, qymcad_core::feature::Purpose::Real);
                    p.add_circle_entity(si, 20.0, 20.0, 10.0, qymcad_core::feature::Purpose::Real);
                }
                "rings" => {
                    p.add_circle_entity(si, 30.0, 30.0, 30.0, qymcad_core::feature::Purpose::Real);
                    p.add_circle_entity(si, 30.0, 30.0, 20.0, qymcad_core::feature::Purpose::Real);
                    p.add_circle_entity(si, 30.0, 30.0, 10.0, qymcad_core::feature::Purpose::Real);
                }
                "two_rects" => {
                    p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
                    p.add_rect_entity(si, 40.0, 0.0, 60.0, 20.0, qymcad_core::feature::Purpose::Real);
                }
                _ => unreachable!(),
            }
            p.regen_sketch(si);
            let mut closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
            closed.sort_by(|a, b| {
                let ar = |c: &u64| p.contour_index(*c).map(|i| p.contours[i].area()).unwrap_or(0.0);
                ar(a).partial_cmp(&ar(b)).unwrap()
            });
            // The sketch sits on world XY, so Z=0 is the BOTTOM of the plate and the plate occupies
            // Z in [0..20]; flip=false grows the tool UPWARDS, into the body. A one-sided boss would
            // therefore add no volume at all, so the boss case is symmetric +-25: it sticks out 5
            // above the plate and 25 below it, adding area*30 outside. The intersect case is the
            // scene column meeting the plate.
            let area = scene_area(which);
            let plate_v = 100.0 * 100.0 * 20.0;
            use qymcad_core::feature::Reach;
            let (h, reach, expect) = match op {
                0 => (5.0, Reach::Forward, plate_v - area * 5.0),  // a pocket 5 deep into the plate
                1 => (50.0, Reach::BothWays, plate_v + area * 30.0), // both ways +-25: 5 above + 25 below, outside
                _ => (50.0, Reach::BothWays, area * 20.0),           // intersection: scene column ∩ plate = area*20
            };
            let node = p.add_combine_multi_op(plate, sid, closed.clone(), h, op, qymcad_core::feature::Extent { reach, ..Default::default() }, 0.0, vec![]);
            let (report, shapes) = qymcad_testkit::regenerate(&mut p);
            let label = format!("{opname} {which}");
            for (id, er) in &report.errors {
                fails.push(format!("{label}: REGEN ERROR {id}: {er}"));
            }
            let v = shapes.get(&node).map(|s| s.volume()).unwrap_or(0.0);
            run_case(&mut fails, &label, v, expect, 0.02);
        }
    }
    assert!(fails.is_empty(), "\nMATRIX FAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Picking a SINGLE region out of nested ones: each rings/rect_hole contour on its own.
#[test]
fn matrix_single_region_pick() {
    let mut fails: Vec<String> = Vec::new();
    // rings: [disc R10, disc R20, disc R30] by area -> regions: disc 100pi, ring 300pi, ring 500pi
    let (p0, sid, closed) = scene("rings");
    let expects = [PI * 100.0, PI * 300.0, PI * 500.0];
    for (i, (&cid, &a)) in closed.iter().zip(expects.iter()).enumerate() {
        let mut p = p0.clone();
        let e = p.add_extrude_multi(sid, vec![cid], 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let body = p.finish_base_body(e, 1);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        let label = format!("rings region#{i}");
        for (id, er) in &report.errors {
            fails.push(format!("{label}: ERROR {id}: {er}"));
        }
        let v = shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
        run_case(&mut fails, &label, v, a * 10.0, 0.02);
    }
    // rect_hole: the outer contour alone -> a plate with a hole; the inner one alone -> a cylinder
    let (p0, sid, closed) = scene("rect_hole");
    let cases = [(closed[0], PI * 100.0 * 10.0, "round island"), (closed[1], (1600.0 - PI * 100.0) * 10.0, "outer with a hole")];
    for (cid, exp, name) in cases {
        let mut p = p0.clone();
        let e = p.add_extrude_multi(sid, vec![cid], 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let body = p.finish_base_body(e, 1);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        let label = format!("rect_hole {name}");
        for (id, er) in &report.errors {
            fails.push(format!("{label}: ERROR {id}: {er}"));
        }
        let v = shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
        run_case(&mut fails, &label, v, exp, 0.02);
    }
    // picking OUTER + ISLAND together (the island is "filled") -> a SOLID plate 1600*10
    {
        let mut p = p0.clone();
        let e = p.add_extrude_multi(sid, vec![closed[1]], 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![closed[0]]);
        let body = p.finish_base_body(e, 1);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        for (id, er) in &report.errors {
            fails.push(format!("rect_hole both(fill): ERROR {id}: {er}"));
        }
        let v = shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
        run_case(&mut fails, "rect_hole both(fill)", v, 16000.0, 0.02);
    }
    assert!(fails.is_empty(), "\nMATRIX FAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}
