//! Selecting a region: the planar arrangement of a sketch gives minimal faces. The strip between two circles,
//! cut by a line, has to become a selectable closed region — a half ring.
use qymcad_core::model::Project;

fn new_sketch() -> (Project, usize) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("t", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    (p, si)
}

// Two concentric circles, r = 10 and r = 5, with a diameter line at y = 0. The arrangement has to give four
// faces: two half rings of about 117.8 and two inner half discs of about 39.3.
#[test]
fn regions_split_ring_by_diameter_line() {
    let (mut p, si) = new_sketch();
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, -20.0, 0.0, 20.0, 0.0, qymcad_core::feature::Purpose::Real); // a diameter through both circles
    let regions = p.sketch_regions(si);
    let areas: Vec<f64> = regions.iter().map(|c| c.area()).collect();
    eprintln!("regions: {} areas={:?}", regions.len(), areas);
    let half_ring = 0.5 * std::f64::consts::PI * (100.0 - 25.0); // ≈117.8
    let half_disk = 0.5 * std::f64::consts::PI * 25.0; // ≈39.3
    let rings = areas.iter().filter(|a| (**a - half_ring).abs() < 3.0).count();
    let disks = areas.iter().filter(|a| (**a - half_disk).abs() < 3.0).count();
    assert!(rings >= 2, "there have to be two half rings of about {half_ring:.1}, found {rings}: {areas:?}");
    assert!(disks >= 2, "there have to be two inner half discs of about {half_disk:.1}, found {disks}: {areas:?}");
    // every region carries exact edges, for extruding into a B-rep
    assert!(regions.iter().all(|c| !c.edges.is_empty()), "the regions carry exact edges");
}

// A lone circle without intersections is a region in itself and is not lost.
#[test]
fn regions_lone_circle_is_one_region() {
    let (mut p, si) = new_sketch();
    p.add_circle_entity(si, 0.0, 0.0, 7.0, qymcad_core::feature::Purpose::Real);
    let regions = p.sketch_regions(si);
    eprintln!("regions of a lone circle: {}", regions.len());
    assert_eq!(regions.len(), 1, "a lone circle is one region");
    assert!((regions[0].area() - std::f64::consts::PI * 49.0).abs() < 2.0, "the area of the circle is about πr²");
}

// After a regeneration the regions become the contours of the sketch, which is what profile selection sees. A
// ring plus a line gives four selectable closed profiles with an area.
#[test]
fn regions_become_selectable_contours() {
    let (mut p, si) = new_sketch();
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, -20.0, 0.0, 20.0, 0.0, qymcad_core::feature::Purpose::Real);
    // the contours of the sketch are the region faces: closed, with a profile ready to extrude
    let closed: Vec<f64> = p.sketches[si].contour_ids.iter().filter_map(|&cid| p.contour_profile_xy(cid).map(|_| cid)).filter_map(|cid| p.contour_index(cid)).map(|i| p.contours[i].area()).collect();
    eprintln!("selectable region contours: {} areas={closed:?}", closed.len());
    // Four closed regions are selectable. The contours may also hold an open line, a sweep path, which profile
    // selection filters out as unclosed.
    assert_eq!(closed.len(), 4, "four regions are available as closed profiles for extrusion: {closed:?}");
}

// A rectangle of four lines is one region face.
#[test]
fn regions_rectangle_is_one_face() {
    let (mut p, si) = new_sketch();
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
    let regions = p.sketch_regions(si);
    let areas: Vec<f64> = regions.iter().map(|c| c.area()).collect();
    eprintln!("regions of the rectangle: {areas:?}");
    assert_eq!(regions.len(), 1, "a rectangle is one face");
    assert!((areas[0] - 1200.0).abs() < 1.0, "the area of 40×30 is 1200: {areas:?}");
}
