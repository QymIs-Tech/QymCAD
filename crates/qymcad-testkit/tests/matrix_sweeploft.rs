//! A matrix over sweeps and lofts: straight and conical, ruled and smooth, checked by exact volumes.

use qymcad_core::feature::{BasePlane, SketchPlane};
use qymcad_core::model::{Project, WorkPlane};

const PI: f64 = std::f64::consts::PI;

fn check(fails: &mut Vec<String>, label: &str, got: f64, exp: f64, tol: f64) {
    if exp <= 0.0 || ((got - exp) / exp).abs() > tol {
        fails.push(format!("{label}: V={got:.1}, expected {exp:.1}"));
    }
}

/// A sweep of a Ø6 circle in XY along a straight path of length 40 in XZ, running up Z, giving a cylinder of
/// π·9·40.
#[test]
fn matrix_sweep_straight() {
    let mut fails: Vec<String> = Vec::new();
    let mut p = Project::default();
    p.new_document();
    let sprof = p.new_sketch("profile");
    let prof_sid = p.sketches[sprof].id;
    p.add_sketch_node(prof_sid, "profile");
    p.add_circle_entity(sprof, 0.0, 0.0, 3.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(sprof);
    let prof_cid = p.sketches[sprof].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
    let spath = p.new_sketch("path");
    let path_sid = p.sketches[spath].id;
    p.sketches[spath].plane = SketchPlane::World(BasePlane::XZ);
    p.add_sketch_node(path_sid, "path");
    p.add_line_entity(spath, 0.0, 0.0, 0.0, 40.0, qymcad_core::feature::Purpose::Real); // along the +Y of the sketch, which points up
    p.regen_sketch(spath);
    let path_cid = p.sketches[spath].contour_ids.first().copied().unwrap_or(0);
    let body = p.add_sweep(prof_sid, vec![prof_cid], path_sid, path_cid);
    let last = p.finish_base_body(body, 1);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    for (id, e) in &report.errors {
        fails.push(format!("sweep: error {id}: {e}"));
    }
    let v = shapes.get(&last).map(|s| s.volume()).unwrap_or(0.0);
    check(&mut fails, "a sweep along a straight path of 40", v, PI * 9.0 * 40.0, 0.02);
    assert!(fails.is_empty(), "\nfailures ({}):\n{}", fails.len(), fails.join("\n"));
}

/// A loft between two square sections at z = 0 and z = 10. Two equal 20×20 sections give a prism of 4000; a
/// ruled taper from 20 to 10 gives 10/3·(400 + 100 + 200) = 2333.3.
#[test]
fn matrix_loft() {
    let mut fails: Vec<String> = Vec::new();
    for (label, w2, exp) in [("a prism, 20 to 20", 20.0, 4000.0), ("a taper, 20 to 10", 10.0, 10.0 / 3.0 * (400.0 + 100.0 + 200.0))] {
        let mut p = Project::default();
        p.new_document();
        let s1 = p.new_sketch("bottom");
        let sid1 = p.sketches[s1].id;
        p.add_sketch_node(sid1, "bottom");
        p.add_rect_entity(s1, -10.0, -10.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(s1);
        let c1 = p.sketches[s1].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        // the upper section on a datum plane at z = 10, centred on the same origin
        let pl = p.add_plane(WorkPlane { id: 0, name: "z10".into(), origin: [0.0, 0.0, 10.0], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, def: Default::default() });
        let s2 = p.new_sketch("top");
        let sid2 = p.sketches[s2].id;
        p.sketches[s2].plane = SketchPlane::Datum(pl);
        p.add_sketch_node(sid2, "top");
        let h = w2 / 2.0;
        p.add_rect_entity(s2, -h, -h, h, h, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(s2);
        let c2 = p.sketches[s2].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let body = p.add_loft(vec![sid1, sid2], vec![c1, c2], true, 0, 0, false);
        let last = p.finish_base_body(body, 1);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        for (id, e) in &report.errors {
            fails.push(format!("{label}: error {id}: {e}"));
        }
        let v = shapes.get(&last).map(|s| s.volume()).unwrap_or(0.0);
        check(&mut fails, label, v, exp, 0.02);
    }
    assert!(fails.is_empty(), "\nfailures ({}):\n{}", fails.len(), fails.join("\n"));
}
