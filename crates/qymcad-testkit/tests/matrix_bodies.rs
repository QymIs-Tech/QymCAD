//! MATRIX: primitives x volumes; body booleans; mirror; linear/circular arrays; hole; move.
//! Runs against the real OCCT kernel; failures accumulate and are reported in one batch.

use qymcad_core::model::Project;

const PI: f64 = std::f64::consts::PI;

fn check(fails: &mut Vec<String>, label: &str, got: f64, exp: f64, tol: f64) {
    if exp <= 0.0 || ((got - exp) / exp).abs() > tol {
        fails.push(format!("{label}: V={got:.1}, expected {exp:.1}"));
    }
}

fn regen_v(p: &mut Project, body: u64, fails: &mut Vec<String>, label: &str) -> f64 {
    let (report, shapes) = qymcad_testkit::regenerate(p);
    for (id, e) in &report.errors {
        fails.push(format!("{label}: ERROR {id}: {e}"));
    }
    shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0)
}

/// Every 3D primitive: exact volume.
#[test]
fn matrix_primitives_3d() {
    let mut fails = Vec::new();
    let cases: Vec<(&str, Box<dyn Fn(&mut Project) -> u64>, f64)> = vec![
        ("box 10x20x30", Box::new(|p: &mut Project| p.add_box(10.0, 20.0, 30.0)), 6000.0),
        ("cylinder r10 h20", Box::new(|p: &mut Project| p.add_cylinder(10.0, 20.0)), PI * 100.0 * 20.0),
        ("sphere r10", Box::new(|p: &mut Project| p.add_sphere(10.0)), 4.0 / 3.0 * PI * 1000.0),
        ("cone 10->5 h12", Box::new(|p: &mut Project| p.add_cone(10.0, 5.0, 12.0)), PI * 12.0 / 3.0 * (100.0 + 50.0 + 25.0)),
        ("torus R20 r5", Box::new(|p: &mut Project| p.add_torus(20.0, 5.0)), 2.0 * PI * PI * 20.0 * 25.0),
        ("prism 6 sides r10 h15", Box::new(|p: &mut Project| p.add_prism(10.0, 6, 15.0)), 1.5 * 3.0_f64.sqrt() * 100.0 * 15.0),
    ];
    for (label, mk, exp) in cases {
        let mut p = Project::default();
        p.new_document();
        let body = mk(&mut p);
        let v = regen_v(&mut p, body, &mut fails, label);
        check(&mut fails, label, v, exp, 0.01);
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Booleans of two bodies: a 20^3 box and a box shifted +10 along X (overlap 10x20x20 = 4000).
#[test]
fn matrix_body_booleans() {
    let mut fails = Vec::new();
    for (label, op, exp) in [("union", 1u8, 12000.0), ("difference", 0u8, 4000.0), ("intersection", 2u8, 4000.0)] {
        let mut p = Project::default();
        p.new_document();
        let a = p.add_box(20.0, 20.0, 20.0);
        let b0 = p.add_box(20.0, 20.0, 20.0);
        let mut mat = qymcad_core::feature::PLACE_IDENTITY;
        mat[3] = 10.0; // shift +10 along X
        let b = p.add_move(b0, mat);
        let res = p.add_body_boolean(a, b, op);
        let label = format!("boolean {label}");
        let v = regen_v(&mut p, res, &mut fails, &label);
        check(&mut fails, &label, v, exp, 0.01);
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Mirror: a cylinder shifted +30 along X, mirrored across YZ. keep=true gives 2 volumes, false gives 1.
#[test]
fn matrix_mirror() {
    let mut fails = Vec::new();
    for (label, keep, exp) in [("with original", true, 2.0 * PI * 100.0 * 20.0), ("copy only", false, PI * 100.0 * 20.0)] {
        let mut p = Project::default();
        p.new_document();
        let c0 = p.add_cylinder(10.0, 20.0);
        let mut mat = qymcad_core::feature::PLACE_IDENTITY;
        mat[3] = 30.0;
        let c = p.add_move(c0, mat);
        let m = p.add_mirror(c, 2, keep, 0); // 2 = YZ (x -> -x)
        let label = format!("mirror {label}");
        let v = regen_v(&mut p, m, &mut fails, &label);
        check(&mut fails, &label, v, exp, 0.01);
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Arrays: linear 3x2 (disjoint copies) and circular 4x90 degrees.
#[test]
fn matrix_arrays() {
    let mut fails = Vec::new();
    {
        let mut p = Project::default();
        p.new_document();
        let c = p.add_cylinder(5.0, 10.0);
        let arr = p.add_linear_array_grid(c, 20.0, 0.0, 0.0, 3, 0.0, 25.0, 0.0, 2);
        let v = regen_v(&mut p, arr, &mut fails, "linear array 3x2");
        check(&mut fails, "linear array 3x2", v, 6.0 * PI * 25.0 * 10.0, 0.01);
    }
    {
        let mut p = Project::default();
        p.new_document();
        let c0 = p.add_cylinder(5.0, 10.0);
        let mut mat = qymcad_core::feature::PLACE_IDENTITY;
        mat[3] = 30.0;
        let c = p.add_move(c0, mat);
        let arr = p.add_circular_array(c, 4, 360.0);
        let v = regen_v(&mut p, arr, &mut fails, "circular array 4x360");
        check(&mut fails, "circular array 4x360", v, 4.0 * PI * 25.0 * 10.0, 0.01);
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Holes on a face: simple, counterbore, countersink — exact volume of the cut.
#[test]
fn matrix_holes() {
    let mut fails = Vec::new();
    // (name, kind, dia, depth, dia2, depth2, removed volume)
    let cyl = |d: f64, h: f64| PI * d * d / 4.0 * h;
    let cases = [
        ("simple d8x10", 0u8, 8.0, 10.0, 0.0, 0.0, cyl(8.0, 10.0)),
        ("counterbore d8x15 + d12x4", 1u8, 8.0, 15.0, 12.0, 4.0, cyl(8.0, 15.0) + cyl(12.0, 4.0) - cyl(8.0, 4.0)),
    ];
    for (label, kind, dia, depth, dia2, depth2, cut) in cases {
        let mut p = Project::default();
        p.new_document();
        let cube = p.add_box(20.0, 20.0, 20.0);
        let _ = qymcad_testkit::regenerate(&mut p);
        let top = p.regen_faces.get(&cube).and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.9)).cloned();
        match top {
            None => fails.push(format!("{label}: top face not found")),
            Some(f) => {
                let key = qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
                let h = p.add_hole_typed(cube, key, dia, depth, kind, dia2, depth2);
                let label = format!("hole {label}");
                let v = regen_v(&mut p, h, &mut fails, &label);
                check(&mut fails, &label, v, 8000.0 - cut, 0.01);
            }
        }
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}
