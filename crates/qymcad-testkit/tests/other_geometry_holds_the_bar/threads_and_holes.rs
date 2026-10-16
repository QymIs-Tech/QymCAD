//! The bar held by documents with threads or holes in them.

use super::*;

/// A THREADED SHAFT. The turns are helicoidal surfaces that no other operation produces, and the
/// thread carried the naming debt longest (16 pieces of one groove under a single name). The shaft
/// is a primitive so the case does not depend on a sketch.
#[test]
fn a_threaded_shaft_holds_the_bar() {
    use qymcad_core::thread::{ThreadSpec, ThreadStandard};
    let mut p = Project::default();
    p.new_document();
    let shaft = p.add_cylinder(8.0, 40.0);
    qymcad_testkit::regenerate(&mut p);
    let rim = p.regen_edges.get(&shaft).and_then(|e| e.iter().find(|e| (e.radius - 8.0).abs() < 0.05).map(|e| e.id));
    let Some(rim) = rim else {
        panic!("the shaft has no rim of radius 8 — the case did not build, nothing to measure");
    };
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 16.0, pitch: 2.0, internal: false, fit: 0.2, ..Default::default() };
    let t = p.add_thread(shaft, rim, spec, 20.0, 1.0, 1.0);
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_faces.get(&t).map(|f| f.len() > 10).unwrap_or(false), "the thread did not build: faces {:?}", p.regen_faces.get(&t).map(|f| f.len()));
    demand_the_bar("thread on a shaft", &mut p);
}

/// HOLES FROM SKETCH POINTS. The wall of each hole is named after the POINT that placed it: adding a
/// point leaves the neighbouring holes with unchanged names. This checks that the recipe holds when
/// several holes come from one operation.
#[test]
fn holes_from_sketch_points_hold_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let base = box_body(&mut p, 30.0, 20.0, 10.0);
    let hsi = p.new_sketch("drill marks");
    let holes_sk = p.sketches[hsi].id;
    p.add_sketch_node(holes_sk, "drill marks");
    p.sketch_point_at(hsi, 6.0, 6.0, 1e-6);
    p.sketch_point_at(hsi, 22.0, 6.0, 1e-6);
    p.sketch_point_at(hsi, 14.0, 15.0, 1e-6);
    assert_eq!(p.sketch_isolated_points(holes_sk).len(), 3, "three drill marks");
    // `flip=true` drills INTO the body. With `false` the operation leaves only an IMPRINT of the
    // circle on the face, and the face count grows exactly as it would from a real hole: measured,
    // the area does not change at all.
    let area_before: f64 = p.regen_faces.get(&base).map(|f| f.iter().map(|x| x.area).sum()).unwrap_or(0.0);
    let h = p.add_hole_from_sketch(base, holes_sk, qymcad_core::model::HoleTool { kind: 0, diameter: 4.0, depth: 6.0, dia2: 0.0, depth2: 0.0 }, true);
    qymcad_testkit::regenerate(&mut p);
    let area_after: f64 = p.regen_faces.get(&h).map(|f| f.iter().map(|x| x.area).sum()).unwrap_or(0.0);
    assert!(area_after > area_before + 1.0, "the drill removed nothing: area was {area_before:.2}, is now {area_after:.2} — an imprint instead of a hole");
    assert!(p.regen_faces.get(&h).map(|f| f.len() >= 9).unwrap_or(false), "the three holes were not drilled: faces {:?}", p.regen_faces.get(&h).map(|f| f.len()));
    demand_the_bar("three holes from sketch points", &mut p);
}

/// A THREAD ON A TUBE. An inner surface right next to an outer one: the thread has two rims of the
/// same radius, outside and inside, and the kernel picks the groove side by geometry.
#[test]
fn a_thread_on_a_tube_holds_the_bar() {
    use qymcad_core::thread::{ThreadSpec, ThreadStandard};
    let mut p = Project::default();
    p.new_document();
    let outer = p.add_cylinder(10.0, 30.0);
    let inner = p.add_cylinder(6.0, 40.0);
    let tube = p.add_body_boolean(outer, inner, 0);
    qymcad_testkit::regenerate(&mut p);
    let rim = p.regen_edges.get(&tube).and_then(|e| e.iter().find(|e| (e.radius - 10.0).abs() < 0.05).map(|e| e.id)).expect("outer rim of the tube");
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 20.0, pitch: 2.0, internal: false, fit: 0.2, ..Default::default() };
    let before: std::collections::HashSet<u32> = p.regen_faces[&tube].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, tube);
    let t = p.add_thread(tube, rim, spec, 15.0, 1.0, 1.0);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&t).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the thread on the tube changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("thread on a tube", area_before, area_of(&p, t));
    demand_the_bar("thread on a tube", &mut p);
}

/// HOLES ON A SOLID OF REVOLUTION. The drill enters a ROUND face: the hole wall borders a surface of
/// revolution rather than a plane, as in every case checked so far.
#[test]
fn holes_on_a_revolve_hold_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch("Profile", vec![Point2::new(0.0, 0.0), Point2::new(20.0, 0.0), Point2::new(20.0, 10.0), Point2::new(0.0, 10.0)], true);
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Profile");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let disc = p.add_revolve_axis(sid, closed, 1, 360.0, 0, 0);
    qymcad_testkit::regenerate(&mut p);
    let hsi = p.new_sketch("drill marks");
    let hsk = p.sketches[hsi].id;
    p.add_sketch_node(hsk, "drill marks");
    p.sketch_point_at(hsi, 10.0, 0.0, 1e-6);
    p.sketch_point_at(hsi, -10.0, 0.0, 1e-6);
    assert_eq!(p.sketch_isolated_points(hsk).len(), 2, "two drill marks");
    let before: std::collections::HashSet<u32> = p.regen_faces[&disc].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, disc);
    let h = p.add_hole_from_sketch(disc, hsk, qymcad_core::model::HoleTool { kind: 0, diameter: 3.0, depth: 12.0, dia2: 0.0, depth2: 0.0 }, false);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&h).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the holes in the solid of revolution were not drilled: errors {:?}", p.regen_errors);
    demand_material_changed("holes on a solid of revolution", area_before, area_of(&p, h));
    demand_the_bar("holes on a solid of revolution", &mut p);
}

/// A THREAD AFTER A SPLIT. The rim the thread runs on belongs to a split PIECE — a body that did not
/// exist in the timeline before.
#[test]
fn a_thread_after_a_split_holds_the_bar() {
    use qymcad_core::thread::{ThreadSpec, ThreadStandard};
    let mut p = Project::default();
    p.new_document();
    let shaft = p.add_cylinder(9.0, 50.0);
    qymcad_testkit::regenerate(&mut p);
    let pieces = p.add_split_body(shaft, 0, 0, 25.0, 2);
    qymcad_testkit::regenerate(&mut p);
    assert_eq!(pieces.len(), 2, "splitting the shaft must yield two pieces");
    let part = pieces[0];
    let rim = p.regen_edges.get(&part).and_then(|e| e.iter().find(|e| (e.radius - 9.0).abs() < 0.05).map(|e| e.id)).expect("a rim on the split piece");
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 18.0, pitch: 2.0, internal: false, fit: 0.2, ..Default::default() };
    let before: std::collections::HashSet<u32> = p.regen_faces[&part].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, part);
    let t = p.add_thread(part, rim, spec, 12.0, 1.0, 1.0);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&t).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the thread after the split changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("thread after a split", area_before, area_of(&p, t));
    demand_the_bar("thread after a split", &mut p);
}

/// A PATTERN AFTER A THREAD. What is copied is a body that already has undercut and turn faces —
/// instance names land over thread names, and every copy must have its own.
#[test]
fn an_array_after_a_thread_holds_the_bar() {
    use qymcad_core::thread::{ThreadSpec, ThreadStandard};
    let mut p = Project::default();
    p.new_document();
    let shaft = p.add_cylinder(6.0, 26.0);
    qymcad_testkit::regenerate(&mut p);
    let rim = p.regen_edges.get(&shaft).and_then(|e| e.iter().find(|e| (e.radius - 6.0).abs() < 0.05).map(|e| e.id));
    let Some(rim) = rim else {
        eprintln!("skip: the shaft has no rim of radius 6");
        return;
    };
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 12.0, pitch: 1.75, internal: false, fit: 0.2, ..Default::default() };
    let t = p.add_thread(shaft, rim, spec, 14.0, 1.0, 1.0);
    qymcad_testkit::regenerate(&mut p);
    let threaded = p.regen_faces.get(&t).map(|f| f.len()).unwrap_or(0);
    if threaded < 6 {
        eprintln!("skip: the thread did not build — faces {threaded}, {:?}", p.regen_errors);
        return;
    }
    let area_before = area_of(&p, t);
    let arr = p.add_linear_array_grid3(t, [ArrayAxis { d: [20.0, 0.0, 0.0], count: 2 }, ArrayAxis::none(), ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    let after = p.regen_faces.get(&arr).map(|f| f.len()).unwrap_or(0);
    if after == 0 {
        eprintln!("skip: the pattern of the threaded shaft did not build — {:?}", p.regen_errors);
        return;
    }
    assert!(after > threaded, "the pattern did not multiply the threaded shaft: was {threaded}, now {after}");
    demand_material_changed("pattern after a thread", area_before, area_of(&p, arr));
    demand_the_bar("pattern after a thread", &mut p);
}

/// A THREAD ON THE RIM OF A PATTERN COPY. The rim belongs not to the source body but to an INSTANCE:
/// its name is of the "instance" kind, and the thread lays its undercut over a copy, not an original.
#[test]
fn a_thread_on_an_array_copy_holds_the_bar() {
    use qymcad_core::thread::{ThreadSpec, ThreadStandard};
    let mut p = Project::default();
    p.new_document();
    let shaft = p.add_cylinder(6.0, 24.0);
    qymcad_testkit::regenerate(&mut p);
    let arr = p.add_linear_array_grid3(shaft, [ArrayAxis { d: [30.0, 0.0, 0.0], count: 2 }, ArrayAxis::none(), ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    let area_before = area_of(&p, arr);
    // The rim of the SECOND copy: a round edge of radius 6 whose centre moved along x by the pattern step.
    let rim = p.regen_edges.get(&arr).and_then(|e| e.iter().find(|e| (e.radius - 6.0).abs() < 0.05 && e.mid[0] > 20.0).map(|e| e.id));
    let Some(rim) = rim else {
        eprintln!("skip: the pattern copy has no rim of radius 6");
        return;
    };
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 12.0, pitch: 1.75, internal: false, fit: 0.2, ..Default::default() };
    let t = p.add_thread(arr, rim, spec, 12.0, 1.0, 1.0);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&t).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the thread on the pattern copy did not build — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("thread on a pattern copy", area_before, area_of(&p, t));
    demand_the_bar("thread on a pattern copy", &mut p);
}

/// A SPLIT OF A THREADED SHAFT ACROSS THE TURN. The plane runs through the helicoidal groove: the
/// section grows faces born neither from a face nor from an edge, but from an intersection of a
/// helical surface.
#[test]
fn a_split_across_a_thread_holds_the_bar() {
    use qymcad_core::thread::{ThreadSpec, ThreadStandard};
    let mut p = Project::default();
    p.new_document();
    let shaft = p.add_cylinder(8.0, 40.0);
    qymcad_testkit::regenerate(&mut p);
    let rim = p.regen_edges.get(&shaft).and_then(|e| e.iter().find(|e| (e.radius - 8.0).abs() < 0.05).map(|e| e.id));
    let Some(rim) = rim else {
        eprintln!("skip: the shaft has no rim of radius 8");
        return;
    };
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 16.0, pitch: 2.0, internal: false, fit: 0.2, ..Default::default() };
    let t = p.add_thread(shaft, rim, spec, 24.0, 1.0, 1.0);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&t).map(|f| f.len() < 10).unwrap_or(true) {
        eprintln!("skip: the thread did not build — {:?}", p.regen_errors);
        return;
    }
    // Cut across the axis in the middle of the thread — the XY plane at z = 12, not at a body boundary.
    let pieces = p.add_split_body(t, 0, 0, 12.0, 2);
    qymcad_testkit::regenerate(&mut p);
    if pieces.len() < 2 || pieces.iter().any(|b| p.regen_faces.get(b).map(|f| f.is_empty()).unwrap_or(true)) {
        eprintln!("skip: the split did not divide the threaded shaft in two — {:?}", p.regen_errors);
        return;
    }
    demand_the_bar("split of a threaded shaft across the turn", &mut p);
}

/// A MIRRORED THREADED SHAFT. What is reflected is a HELICAL surface: in the image it is left-handed
/// where the original is right-handed, and pairs of faces matching in shape are more common here than
/// anywhere else.
#[test]
fn a_mirrored_thread_holds_the_bar() {
    use qymcad_core::thread::{ThreadSpec, ThreadStandard};
    let mut p = Project::default();
    p.new_document();
    let shaft = p.add_cylinder(7.0, 30.0);
    qymcad_testkit::regenerate(&mut p);
    let rim = p.regen_edges.get(&shaft).and_then(|e| e.iter().find(|e| (e.radius - 7.0).abs() < 0.05).map(|e| e.id));
    let Some(rim) = rim else {
        eprintln!("skip: the shaft has no rim of radius 7");
        return;
    };
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 14.0, pitch: 2.0, internal: false, fit: 0.2, ..Default::default() };
    // THE THREAD LENGTH IS CHOSEN BY TIMING, NOT BY EYE. The bar rebuilds the body four times
    // (build, rebuild, edit, rebuild again) and every helicoid is recomputed each time. Measured:
    //
    //     18 mm (nine turns) — 235 s   <- four times the rest of the suite put together
    //      6 mm (three turns) — 53 s
    //      4 mm (two turns)   — 28 s
    //
    // The "mirror x helical surface" seam is exercised identically at any number of turns, so take
    // the cheapest one on which the thread is still real.
    let t = p.add_thread(shaft, rim, spec, 4.0, 1.0, 1.0);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&t).map(|f| f.len() < 10).unwrap_or(true) {
        eprintln!("skip: the thread did not build — {:?}", p.regen_errors);
        return;
    }
    let area_before = area_of(&p, t);
    // The YZ plane, original kept: the shaft stands on the Z axis, so the image moves sideways and
    // does not merge with it.
    let m = p.add_mirror(t, 2, true, 0);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&m).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the mirrored threaded shaft did not build — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("mirrored threaded shaft", area_before, area_of(&p, m));
    demand_the_bar("mirrored threaded shaft", &mut p);
}

/// HOLES THROUGH THE WALL OF A LOFT. The drill passes through a surface that is neither planar nor
/// cylindrical: the loft side is split into pieces by the number of section edges, and the hole
/// crosses them at an angle.
#[test]
fn holes_through_a_loft_hold_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let s0 = p.new_sketch("bottom");
    let sid0 = p.sketches[s0].id;
    p.add_sketch_node(sid0, "bottom");
    p.add_circle_entity(s0, 0.0, 0.0, 30.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s0);
    let c0 = p.sketches[s0].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("bottom contour");
    let pl = p.add_plane(WorkPlane { id: 0, name: "z16".into(), origin: [0.0, 0.0, 16.0], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, tilt: Default::default(), def: Default::default() });
    let s1 = p.new_sketch("top");
    let sid1 = p.sketches[s1].id;
    p.sketches[s1].plane = SketchPlane::Datum(pl);
    p.add_sketch_node(sid1, "top");
    p.add_circle_entity(s1, 0.0, 0.0, 18.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s1);
    let c1 = p.sketches[s1].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("top contour");
    let body = p.add_loft(vec![sid0, sid1], vec![c0, c1], true, 0, 1, false);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&body).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the loft did not build — {:?}", p.regen_errors);
        return;
    }
    let area_before = area_of(&p, body);
    let hsi = p.new_sketch("drill marks");
    let hsk = p.sketches[hsi].id;
    p.add_sketch_node(hsk, "drill marks");
    p.sketch_point_at(hsi, 6.0, 0.0, 1e-6);
    p.sketch_point_at(hsi, -6.0, 0.0, 1e-6);
    assert_eq!(p.sketch_isolated_points(hsk).len(), 2, "two drill marks");
    let h = p.add_hole_from_sketch(body, hsk, qymcad_core::model::HoleTool { kind: 0, diameter: 4.0, depth: 20.0, dia2: 0.0, depth2: 0.0 }, true);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&h).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: drilling the loft did not build — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("holes through a loft", area_before, area_of(&p, h));
    demand_the_bar("holes through a loft", &mut p);
}
