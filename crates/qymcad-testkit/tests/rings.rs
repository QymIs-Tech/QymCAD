use qymcad_core::model::Project;
// A real case: THREE concentric circles (R=30,20,10) in a part sketch.
// The arranged regions: the outer ring 30/20, the middle one 20/10, the inner disc 10.
// Checked: extruding a ring, cutting with it, and combinations — the volumes must be exact.
fn mk() -> (Project, Vec<u64>) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("Sketch");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Sketch");
    p.add_circle_entity(si, 0.0, 0.0, 30.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 0.0, 0.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    (p, closed)
}
fn area(p: &Project, cid: u64) -> f64 {
    let ci = p.contour_index(cid).unwrap();
    p.contours[ci].area()
}
#[test]
fn ring_regions_exist() {
    let (p, closed) = mk();
    eprintln!("contours: {}", closed.len());
    for &c in &closed { eprintln!("  contour {c}: area={:.0}", area(&p, c)); }
    // expect 3 regions: the ring 30/20 (pi*500 = about 1571), the ring 20/10 (pi*300 = about 942), the disc 10 (pi*100 = about 314)
    assert_eq!(closed.len(), 3, "3 circles give 3 regions (2 rings and a disc)");
}
#[test]
fn extrude_each_region_correct_volume() {
    let (p0, closed) = mk();
    let pi = std::f64::consts::PI;
    let expect = [pi*500.0*10.0, pi*300.0*10.0, pi*100.0*10.0]; // ring areas times h, in any order
    let mut got: Vec<f64> = Vec::new();
    for &c in &closed {
        let mut p = p0.clone();
        let sid = p.sketches[0].id;
        let e = p.add_extrude_multi(sid, vec![c], 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let body = p.finish_base_body(e, 1);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        for (id,er) in &report.errors { eprintln!("ERROR {id}: {er}"); }
        let v = shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
        eprintln!("contour {c}: V={v:.0}");
        got.push(v);
    }
    got.sort_by(|a,b| b.partial_cmp(a).unwrap());
    let mut exp = expect.to_vec(); exp.sort_by(|a,b| b.partial_cmp(a).unwrap());
    for (g,e) in got.iter().zip(exp.iter()) {
        assert!((g-e).abs()/e < 0.02, "region volume: got {g:.0}, expected {e:.0}");
    }
}
#[test]
fn extrude_disk_then_cut_middle_ring() {
    let (mut p, closed) = mk();
    let sid = p.sketches[0].id;
    let pi = std::f64::consts::PI;
    // find them by area: the disc (314), the middle ring (942), the outer one (1571)
    let mut by_area: Vec<(f64,u64)> = closed.iter().map(|&c| (area(&p,c), c)).collect();
    by_area.sort_by(|a,b| a.0.partial_cmp(&b.0).unwrap());
    let (disk, mid, outer) = (by_area[0].1, by_area[1].1, by_area[2].1);
    // extrude EVERYTHING (the disc plus both rings, that is the full R30 circle) by 10
    let e = p.add_extrude_multi(sid, vec![disk, mid, outer], 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let body = p.finish_base_body(e, 1);
    let (r1, s1) = qymcad_testkit::regenerate(&mut p);
    for (id,er) in &r1.errors { eprintln!("ERROR in the base {id}: {er}"); }
    let v1 = s1.get(&body).map(|s| s.volume()).unwrap_or(0.0);
    eprintln!("base (3 regions at once): V={v1:.0}, expected {:.0}", pi*900.0*10.0);
    assert!((v1 - pi*900.0*10.0).abs()/(pi*900.0*10.0) < 0.02, "a full R30 cylinder from 3 regions");
    // CUT with the middle ring all the way through, leaving a groove: the R10 disc and the 30/20 ring remain
    let cut = p.add_combine_multi_op(body, sid, vec![mid], 12.0, 0, qymcad_core::feature::Extent { through: true, ..Default::default() }, 0.0, vec![]);
    let (r2, s2) = qymcad_testkit::regenerate(&mut p);
    for (id,er) in &r2.errors { eprintln!("ERROR in the cut {id}: {er}"); }
    let v2 = s2.get(&cut).map(|s| s.volume()).unwrap_or(0.0);
    let exp2 = pi*(500.0+100.0)*10.0;
    eprintln!("after cutting with the ring: V={v2:.0}, expected {exp2:.0}");
    assert!((v2-exp2).abs()/exp2 < 0.02, "cut with the middle ring: V={v2:.0} != {exp2:.0}");
}
