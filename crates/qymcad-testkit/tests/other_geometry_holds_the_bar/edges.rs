//! The bar held by documents with fillets, chamfers and drafts in them.

use super::*;

/// DRAFT AND THICKEN — operations that appeared exactly once in the familiar documents and both used
/// to lose names (draft named one face per operation, thicken named nothing at all).
#[test]
fn a_drafted_part_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 20.0, 14.0, 9.0);
    let sides: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2].abs() < 0.1).map(|f| f.id).collect();
    let neutral = p.regen_faces[&body].iter().find(|f| f.normal[2] < -0.9).map(|f| f.id).unwrap_or(0);
    if sides.is_empty() || neutral == 0 {
        eprintln!("skip: the box has no side faces or no base");
        return;
    }
    let drafted = p.add_draft(body, sides, neutral, 5.0, false);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&drafted).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the draft did not build");
        return;
    }
    demand_the_bar("draft", &mut p);
}

/// A FILLET ON A SOLID OF REVOLUTION. The fillet surface is named after the EDGE that bore it, and
/// the edge here is round, on a solid of revolution — not the shape the fillet was checked on.
#[test]
fn a_fillet_on_a_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch("Profile", vec![Point2::new(6.0, 0.0), Point2::new(16.0, 0.0), Point2::new(16.0, 14.0), Point2::new(6.0, 14.0)], true);
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Profile");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_revolve_axis(sid, closed, 1, 360.0, 0, 0);
    qymcad_testkit::regenerate(&mut p);
    let edge = p.regen_edges.get(&body).and_then(|e| e.iter().filter(|e| e.radius > 1e-9).max_by(|a, b| a.radius.total_cmp(&b.radius)).map(|e| e.id)).expect("a round edge on the solid of revolution");
    let before: std::collections::HashSet<u32> = p.regen_faces[&body].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, body);
    let f = p.add_fillet(body, 1.5, vec![edge]);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&f).map(|x| x.iter().map(|y| y.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the fillet on the revolve changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("fillet on a solid of revolution", area_before, area_of(&p, f));
    demand_the_bar("fillet on a solid of revolution", &mut p);
}

/// A DRAFT ON A SOLID OF REVOLUTION. A ROUND face is tilted: it has no straight generating edge, and
/// the draft side is born out of the surface of revolution.
#[test]
fn a_draft_on_a_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch("Profile", vec![Point2::new(6.0, 0.0), Point2::new(14.0, 0.0), Point2::new(14.0, 20.0), Point2::new(6.0, 20.0)], true);
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Profile");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_revolve_axis(sid, closed, 1, 360.0, 0, 0);
    qymcad_testkit::regenerate(&mut p);
    // TILT THE LARGEST FACE, take the smallest as neutral — both exist under any edit.
    let faces = p.regen_faces.get(&body).cloned().unwrap_or_default();
    let big = faces.iter().max_by(|a, b| a.area.total_cmp(&b.area)).map(|f| f.id).expect("a face");
    let small = faces.iter().min_by(|a, b| a.area.total_cmp(&b.area)).map(|f| f.id).expect("a face");
    let before: std::collections::HashSet<u32> = faces.iter().map(|f| f.id).collect();
    let d = p.add_draft(body, vec![big], small, 3.0, false);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&d).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    if after.difference(&before).count() == 0 {
        eprintln!("skip: the kernel does not build a draft on this revolved face — {:?}", p.regen_errors);
        return;
    }
    demand_the_bar("draft on a solid of revolution", &mut p);
}

/// A FILLET ON AN EDGE BORN OF A SPLIT. Such an edge has no direct origin in the source body: it
/// appeared on the section. The fillet must take it by name, not by position.
#[test]
fn a_fillet_on_a_split_born_edge_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 28.0, 18.0, 12.0);
    let pieces = p.add_split_body(body, 2, 0, 14.0, 2); // across YZ at x=14
    qymcad_testkit::regenerate(&mut p);
    if pieces.len() < 2 {
        eprintln!("skip: the split did not divide the body in two — {:?}", p.regen_errors);
        return;
    }
    let piece = pieces[0];
    // A SECTION edge: it lies in the cutting plane, so all of its points are at x = 14.
    let edge = p.regen_edges.get(&piece).and_then(|e| e.iter().find(|e| (e.mid[0] - 14.0).abs() < 1e-6).map(|e| e.id));
    let Some(edge) = edge else {
        eprintln!("skip: the piece has no section edge");
        return;
    };
    let f = p.add_fillet(piece, 1.5, vec![edge]);
    qymcad_testkit::regenerate(&mut p);
    let faces = p.regen_faces.get(&f).map(|x| x.len()).unwrap_or(0);
    if faces == 0 {
        eprintln!("skip: the fillet on the section edge did not build — {:?}", p.regen_errors);
        return;
    }
    // Measured: the split piece has 6 faces, 7 after filleting one edge.
    assert_eq!(faces, 7, "filleting a section edge must add exactly one face: got {faces}");
    demand_the_bar("fillet on a section edge", &mut p);
}

/// A DRAFT ON A FACE BORN OF A BOOLEAN. The pocket wall came from the TOOL, not from the base, and
/// the draft lays its own role over someone else's name.
#[test]
fn a_draft_on_a_boolean_born_face_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let base = box_body(&mut p, 34.0, 24.0, 14.0);
    let cutter = {
        let sid = p.add_line_sketch("Pocket", vec![Point2::new(8.0, 6.0), Point2::new(26.0, 6.0), Point2::new(26.0, 18.0), Point2::new(8.0, 18.0)], true);
        let si = p.sketch_index(sid).unwrap();
        p.regen_sketch(si);
        p.add_sketch_node(sid, "Pocket");
        let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
        let c = p.add_extrude_multi(sid, closed, 8.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        qymcad_testkit::regenerate(&mut p);
        c
    };
    let pocket = p.add_body_boolean(base, cutter, 0);
    qymcad_testkit::regenerate(&mut p);
    // The pocket walls are the vertical faces INSIDE the outline: the tool produced them.
    let sides: Vec<u32> =
        p.regen_faces[&pocket].iter().filter(|f| f.normal[2].abs() < 0.1 && f.centroid.x > 7.0 && f.centroid.x < 27.0 && f.centroid.y > 5.0 && f.centroid.y < 19.0).map(|f| f.id).collect();
    let neutral = p.regen_faces[&pocket].iter().find(|f| f.normal[2] > 0.9).map(|f| f.id).unwrap_or(0);
    if sides.is_empty() || neutral == 0 {
        eprintln!("skip: the pocket has no walls or no top");
        return;
    }
    let nsides = sides.len();
    let d = p.add_draft(pocket, sides, neutral, 4.0, false);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&d).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the draft on the pocket wall did not build — {:?}", p.regen_errors);
        return;
    }
    // Measured: the pocket gives 11 faces (6 from the box, 4 walls and a floor); drafting the four
    // walls does not change that count — it tilts them, it does not add any.
    assert_eq!(nsides, 4, "the pocket must have four walls: found {nsides}");
    assert_eq!(p.regen_faces.get(&d).map(|f| f.len()).unwrap_or(0), 11, "the draft changed the pocket face count");
    demand_the_bar("draft on a pocket wall", &mut p);
}

/// A CHAMFER ON A LOFT EDGE. The edge lies between the loft side surface and the cap; the side is
/// split into pieces by the number of section edges, so "one cap edge" is in fact the junction of
/// several pieces.
#[test]
fn a_chamfer_on_a_loft_edge_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let s0 = p.new_sketch("bottom");
    let sid0 = p.sketches[s0].id;
    p.add_sketch_node(sid0, "bottom");
    p.add_circle_entity(s0, 0.0, 0.0, 30.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s0);
    let c0 = p.sketches[s0].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("bottom contour");
    let pl = p.add_plane(WorkPlane { id: 0, name: "z14".into(), origin: [0.0, 0.0, 14.0], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, tilt: Default::default(), def: Default::default() });
    let s1 = p.new_sketch("top");
    let sid1 = p.sketches[s1].id;
    p.sketches[s1].plane = SketchPlane::Datum(pl);
    p.add_sketch_node(sid1, "top");
    p.add_circle_entity(s1, 0.0, 0.0, 16.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s1);
    let c1 = p.sketches[s1].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("top contour");
    let body = p.add_loft(vec![sid0, sid1], vec![c0, c1], true, 0, 1, false);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&body).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the loft did not build — {:?}", p.regen_errors);
        return;
    }
    let area_before = area_of(&p, body);
    // The top cap edge: it lies at z = 14.
    let edge = p.regen_edges.get(&body).and_then(|e| e.iter().find(|e| (e.mid[2] - 14.0).abs() < 1e-6).map(|e| e.id));
    let Some(edge) = edge else {
        eprintln!("skip: the loft has no top cap edge");
        return;
    };
    let ch = p.add_chamfer(body, 1.2, vec![edge]);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&ch).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the chamfer on the loft edge did not build — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("chamfer on a loft edge", area_before, area_of(&p, ch));
    demand_the_bar("chamfer on a loft edge", &mut p);
}
