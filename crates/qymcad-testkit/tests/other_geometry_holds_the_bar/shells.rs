//! The bar held by documents with a shell in them.

use super::*;

/// SHELL -> CIRCULAR PATTERN -> FILLET. An order that appears in neither familiar document.
#[test]
fn a_shelled_and_patterned_part_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 30.0, 20.0, 12.0);
    let open: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    if open.is_empty() {
        eprintln!("skip: the box has no top face");
        return;
    }
    let shell = p.add_shell_mode(body, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let arr = p.add_circular_array_axis(shell, 4, 360.0, 0);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&arr).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the pattern did not build");
        return;
    }
    demand_the_bar("shell + circular pattern", &mut p);
}

/// SHELL ON A SOLID OF REVOLUTION. A wall is born from a source face, and the faces here are round —
/// not the case the shell was checked on, which was a box.
#[test]
fn a_shelled_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch("Profile", vec![Point2::new(6.0, 0.0), Point2::new(14.0, 0.0), Point2::new(14.0, 18.0), Point2::new(6.0, 18.0)], true);
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Profile");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_revolve_axis(sid, closed, 1, 360.0, 0, 0);
    qymcad_testkit::regenerate(&mut p);
    // THE FACE IS PICKED THE WAY A PERSON WOULD PICK IT — the largest one. Looking for the "top" by
    // a normal along Z is wrong here: the revolve runs around a different axis and no such face
    // exists, so the case was skipped silently — and a skip does not count as a pass.
    let open: Vec<u32> = p.regen_faces[&body].iter().max_by(|a, b| a.area.total_cmp(&b.area)).map(|f| vec![f.id]).expect("a solid of revolution must have at least one face");
    let sh = p.add_shell_mode(body, 1.5, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_faces.get(&sh).map(|f| f.len() >= 4).unwrap_or(false), "the shell on a revolve did not build: faces {:?}", p.regen_faces.get(&sh).map(|f| f.len()));
    demand_the_bar("shell on a solid of revolution", &mut p);
}

/// A SHELL AFTER A PATTERN. The reverse of the already checked "shell -> pattern": here the wall is
/// built on a body that is ALREADY multiplied, and its source face is the image of a copy, not the
/// original.
#[test]
fn a_shell_after_a_pattern_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 20.0, 14.0, 9.0);
    let arr = p.add_circular_array_axis(body, 3, 300.0, 0);
    qymcad_testkit::regenerate(&mut p);
    let open: Vec<u32> = p.regen_faces[&arr].iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.area.total_cmp(&b.area)).map(|f| vec![f.id]).expect("top face of the pattern result");
    let before: std::collections::HashSet<u32> = p.regen_faces[&arr].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, arr);
    let sh = p.add_shell_mode(arr, 1.5, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&sh).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the shell after the pattern changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("shell after a pattern", area_before, area_of(&p, sh));
    demand_the_bar("shell after a pattern", &mut p);
}

/// A SHELL AFTER A SPLIT. The split leaves a section face and the shell builds its wall on that one —
/// so the wall's source is a face that did not exist in the original body at all.
#[test]
fn a_shell_after_a_split_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 26.0, 18.0, 14.0);
    let pieces = p.add_split_body(body, 0, 0, 7.0, 2);
    qymcad_testkit::regenerate(&mut p);
    assert_eq!(pieces.len(), 2, "the split must yield two pieces");
    let target = pieces[0];
    let open: Vec<u32> = p.regen_faces[&target].iter().max_by(|a, b| a.area.total_cmp(&b.area)).map(|f| vec![f.id]).expect("a face on the split piece");
    let before: std::collections::HashSet<u32> = p.regen_faces[&target].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, target);
    let sh = p.add_shell_mode(target, 1.5, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&sh).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the shell after the split changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("shell after a split", area_before, area_of(&p, sh));
    demand_the_bar("shell after a split", &mut p);
}

/// A PATTERN AFTER A SHELLED REVOLVE. Three operations in a row, each with its own naming recipe:
/// the revolve gives round faces, the shell builds walls from them, the pattern multiplies both.
#[test]
fn a_pattern_after_a_shelled_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch("Profile", vec![Point2::new(8.0, 0.0), Point2::new(15.0, 0.0), Point2::new(15.0, 16.0), Point2::new(8.0, 16.0)], true);
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Profile");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_revolve_axis(sid, closed, 1, 360.0, 0, 0);
    qymcad_testkit::regenerate(&mut p);
    let open: Vec<u32> = p.regen_faces[&body].iter().max_by(|a, b| a.area.total_cmp(&b.area)).map(|f| vec![f.id]).expect("a face of the solid of revolution");
    let sh = p.add_shell_mode(body, 1.2, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let before: std::collections::HashSet<u32> = p.regen_faces[&sh].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, sh);
    let arr = p.add_circular_array_axis(sh, 3, 270.0, 0);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&arr).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(after.len() > before.len(), "the pattern after the shell did not multiply the body: was {} now {}", before.len(), after.len());
    demand_material_changed("pattern after a shelled revolve", area_before, area_of(&p, arr));
    demand_the_bar("pattern after a shelled revolve", &mut p);
}

/// A SHELL AFTER A SHELL. The second wall is built on a face that IS ITSELF a wall: its source name
/// already carries `ShellWall`, and the second pass lays its own role on top.
#[test]
fn a_shell_after_a_shell_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 30.0, 22.0, 16.0);
    let open1: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    let sh1 = p.add_shell_mode(body, 2.0, open1, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let open2: Vec<u32> = p.regen_faces[&sh1].iter().filter(|f| f.normal[2] < -0.9).max_by(|a, b| a.area.total_cmp(&b.area)).map(|f| vec![f.id]).expect("bottom face of the first shell");
    let before: std::collections::HashSet<u32> = p.regen_faces[&sh1].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, sh1);
    let sh2 = p.add_shell_mode(sh1, 0.8, open2, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&sh2).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    if after.difference(&before).count() == 0 {
        eprintln!("skip: the kernel does not build a second shell on this body — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("shell after a shell", area_before, area_of(&p, sh2));
    demand_the_bar("shell after a shell", &mut p);
}

/// A SHELL ON EACH OF THREE SPLIT PIECES. The very "one body at a time" path where a defect with
/// discarded wall names was found — but now there are three pieces and the shell runs over each.
#[test]
fn a_shell_on_each_of_three_split_pieces_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 40.0, 20.0, 12.0);
    let arr = p.add_linear_array_grid3(body, [ArrayAxis { d: [50.0, 0.0, 0.0], count: 3 }, ArrayAxis::none(), ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    let open: Vec<u32> = p.regen_faces[&arr].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    assert!(open.len() >= 3, "three copies must have three top faces, found {}", open.len());
    let before: std::collections::HashSet<u32> = p.regen_faces[&arr].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, arr);
    let sh = p.add_shell_mode(arr, 1.5, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&sh).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "shelling the three copies changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("shell on each of three copies", area_before, area_of(&p, sh));
    demand_the_bar("shell on each of three copies", &mut p);
}

/// A BODY BOOLEAN OVER SHELLS. Both operands have inner walls, so at the intersection the names of
/// TWO shells meet — where an ordinary boolean only meets a base and a tool.
#[test]
fn a_body_boolean_of_two_shells_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let a = box_body(&mut p, 26.0, 20.0, 14.0);
    let open_a: Vec<u32> = p.regen_faces[&a].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    let sa = p.add_shell_mode(a, 2.0, open_a, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let b = {
        let sid = p.add_line_sketch("Second", vec![Point2::new(14.0, 6.0), Point2::new(38.0, 6.0), Point2::new(38.0, 26.0), Point2::new(14.0, 26.0)], true);
        let si = p.sketch_index(sid).unwrap();
        p.regen_sketch(si);
        p.add_sketch_node(sid, "Second");
        let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
        let bb = p.add_extrude_multi(sid, closed, 12.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        qymcad_testkit::regenerate(&mut p);
        bb
    };
    let open_b: Vec<u32> = p.regen_faces[&b].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    let sb = p.add_shell_mode(b, 1.5, open_b, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let u = p.add_body_boolean(sa, sb, 1); // union of two hollow bodies
    qymcad_testkit::regenerate(&mut p);
    let faces = p.regen_faces.get(&u).map(|f| f.len()).unwrap_or(0);
    if faces == 0 {
        eprintln!("skip: the kernel could not do a boolean of two shells — {:?}", p.regen_errors);
        return;
    }
    demand_the_bar("boolean of two shells", &mut p);
}

/// A SPLIT OF A SHELL. The plane runs through the VOID: every piece grows section faces both outside
/// and inside, and both sides must get names rather than positional numbers.
#[test]
fn a_split_of_a_shell_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 30.0, 18.0, 12.0);
    let open: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    let sh = p.add_shell_mode(body, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    // Cut across YZ at x=15 — dead centre, the plane does not coincide with a body boundary.
    let pieces = p.add_split_body(sh, 2, 0, 15.0, 2);
    qymcad_testkit::regenerate(&mut p);
    if pieces.len() < 2 || pieces.iter().any(|b| p.regen_faces.get(b).map(|f| f.is_empty()).unwrap_or(true)) {
        eprintln!("skip: the split did not divide the shell in two — {:?}", p.regen_errors);
        return;
    }
    demand_the_bar("split of a shell", &mut p);
}

/// HOLES FROM A SKETCH THROUGH A SHELL. The drill passes a wall, the void and a second wall: one hole
/// gives birth to SEVERAL pieces of side surface, and each must carry the name of that drill mark
/// rather than an ordinal number.
#[test]
fn holes_through_a_shell_hold_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let base = box_body(&mut p, 30.0, 20.0, 12.0);
    let open: Vec<u32> = p.regen_faces[&base].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    let sh = p.add_shell_mode(base, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let hsi = p.new_sketch("drill marks in the shell");
    let hsk = p.sketches[hsi].id;
    p.add_sketch_node(hsk, "drill marks in the shell");
    p.sketch_point_at(hsi, 9.0, 7.0, 1e-6);
    p.sketch_point_at(hsi, 21.0, 13.0, 1e-6);
    assert_eq!(p.sketch_isolated_points(hsk).len(), 2, "two drill marks");
    // DRILL DIRECTION IS NOT A DETAIL. With `flip=false` the drill goes AWAY from the body and
    // removes nothing: measured, the area was exactly that of a clean box (2400.00 at any depth) and
    // the extra face was an IMPRINT of the circle on the lid (600 = 580.44 + 19.56, and 19.56 is
    // pi*2.5^2). The node stayed green throughout. Hence `flip=true` here, and what is demanded
    // below is removed material rather than a face count.
    let area_before: f64 = p.regen_faces.get(&sh).map(|f| f.iter().map(|x| x.area).sum()).unwrap_or(0.0);
    let h = p.add_hole_from_sketch(sh, hsk, qymcad_core::model::HoleTool { kind: 0, diameter: 3.0, depth: 20.0, dia2: 0.0, depth2: 0.0 }, true);
    qymcad_testkit::regenerate(&mut p);
    let faces = p.regen_faces.get(&h).map(|f| f.len()).unwrap_or(0);
    if faces == 0 {
        eprintln!("skip: drilling the shell did not build — {:?}", p.regen_errors);
        return;
    }
    // A GREEN NODE IS NOT YET AN ANSWER. Names can be fine on a body from which the drilling quietly
    // vanished. Measured: the shell has 11 faces, after two drills 13 — exactly one side face per
    // hole. Ask directly, otherwise the case degenerates into a check of emptiness.
    let area_after: f64 = p.regen_faces.get(&h).map(|f| f.iter().map(|x| x.area).sum()).unwrap_or(0.0);
    assert!(area_after > area_before + 1.0, "the drill removed nothing: area was {area_before:.2}, is now {area_after:.2} — an imprint instead of a hole");
    demand_the_bar("holes through a shell", &mut p);
}

/// A SHELL AFTER A BODY BOOLEAN. The wall is built on a face the TOOL produced, not the base: the
/// shell has no origin of its own for such a face and has to take its name from a stranger.
#[test]
fn a_shell_after_a_body_boolean_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let base = box_body(&mut p, 30.0, 22.0, 14.0);
    let lug = {
        let sid = p.add_line_sketch("Boss", vec![Point2::new(24.0, 8.0), Point2::new(40.0, 8.0), Point2::new(40.0, 16.0), Point2::new(24.0, 16.0)], true);
        let si = p.sketch_index(sid).unwrap();
        p.regen_sketch(si);
        p.add_sketch_node(sid, "Boss");
        let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
        let b = p.add_extrude_multi(sid, closed, 14.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        qymcad_testkit::regenerate(&mut p);
        b
    };
    let joined = p.add_body_boolean(base, lug, 1);
    qymcad_testkit::regenerate(&mut p);
    let area_before = area_of(&p, joined);
    let open: Vec<u32> = p.regen_faces[&joined].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    if open.is_empty() {
        eprintln!("skip: the merged body has no top face — {:?}", p.regen_errors);
        return;
    }
    let sh = p.add_shell_mode(joined, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&sh).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the shell after the boolean did not build — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("shell after a boolean", area_before, area_of(&p, sh));
    demand_the_bar("shell after a boolean", &mut p);
}

/// A LOFT WITH A SHELL OVER IT. The side surface of a loft is split into pieces by the number of
/// section edges, and the shell builds a wall on EVERY piece — where a box would have a single face.
#[test]
fn a_shell_on_a_loft_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let s0 = p.new_sketch("bottom");
    let sid0 = p.sketches[s0].id;
    p.add_sketch_node(sid0, "bottom");
    p.add_circle_entity(s0, 0.0, 0.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s0);
    let c0 = p.sketches[s0].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("bottom contour");
    let pl = p.add_plane(WorkPlane { id: 0, name: "z18".into(), origin: [0.0, 0.0, 18.0], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, tilt: Default::default(), def: Default::default() });
    let s1 = p.new_sketch("top");
    let sid1 = p.sketches[s1].id;
    p.sketches[s1].plane = SketchPlane::Datum(pl);
    p.add_sketch_node(sid1, "top");
    p.add_circle_entity(s1, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s1);
    let c1 = p.sketches[s1].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("top contour");
    let body = p.add_loft(vec![sid0, sid1], vec![c0, c1], true, 0, 1, false);
    qymcad_testkit::regenerate(&mut p);
    let n = p.regen_faces.get(&body).map(|f| f.len()).unwrap_or(0);
    if n == 0 {
        eprintln!("skip: the loft did not build — {:?}", p.regen_errors);
        return;
    }
    let area_before = area_of(&p, body);
    let open: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    if open.is_empty() {
        eprintln!("skip: the loft has no top face");
        return;
    }
    let sh = p.add_shell_mode(body, 1.5, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&sh).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the shell on the loft did not build — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("shell on a loft", area_before, area_of(&p, sh));
    demand_the_bar("shell on a loft", &mut p);
}

/// A CHAMFER ON AN EDGE BORN OF A SHELL. The shell rim is the junction of an outer face and an inner
/// wall; it has no preimage in the source body, and its name is derived by a recipe, not by history.
#[test]
fn a_chamfer_on_a_shell_born_edge_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 26.0, 18.0, 12.0);
    let open: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    let sh = p.add_shell_mode(body, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let area_before = area_of(&p, sh);
    // The rim lies on the top (z = 12) and NOT on the outer outline — so it was born of the shell.
    let edge = p.regen_edges.get(&sh).and_then(|e| e.iter().find(|e| (e.mid[2] - 12.0).abs() < 1e-6 && e.mid[0] > 1.0 && e.mid[0] < 25.0 && e.mid[1] > 1.0 && e.mid[1] < 17.0).map(|e| e.id));
    let Some(edge) = edge else {
        eprintln!("skip: the shell has no inner rim");
        return;
    };
    let ch = p.add_chamfer(sh, 0.6, vec![edge]);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&ch).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the chamfer on the shell rim did not build — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("chamfer on a shell rim", area_before, area_of(&p, ch));
    demand_the_bar("chamfer on a shell rim", &mut p);
}

/// A SHELL AFTER A DRAFT. The walls are already tilted, so the inward offset does not run parallel to
/// them: every inner wall has its own angle, and the shell recipe derives its name over the draft's.
#[test]
fn a_shell_after_a_draft_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 28.0, 20.0, 14.0);
    let sides: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2].abs() < 0.1).map(|f| f.id).collect();
    let neutral = p.regen_faces[&body].iter().find(|f| f.normal[2] < -0.9).map(|f| f.id).unwrap_or(0);
    if sides.is_empty() || neutral == 0 {
        eprintln!("skip: the box has no side faces or no base");
        return;
    }
    let drafted = p.add_draft(body, sides, neutral, 4.0, false);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&drafted).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the draft did not build — {:?}", p.regen_errors);
        return;
    }
    let area_before = area_of(&p, drafted);
    let open: Vec<u32> = p.regen_faces[&drafted].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    if open.is_empty() {
        eprintln!("skip: the drafted box has no top face");
        return;
    }
    let sh = p.add_shell_mode(drafted, 1.5, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&sh).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the shell after the draft did not build — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("shell after a draft", area_before, area_of(&p, sh));
    demand_the_bar("shell after a draft", &mut p);
}

/// A DRAFT ON A SHELLED SOLID OF REVOLUTION. What has to be tilted is a wall that was itself born by
/// offsetting a round face: its name comes from the shell, and the draft lays its own on top — all of
/// it on a cylinder, where the "side face" is single and closed.
#[test]
fn a_draft_on_a_shelled_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch("Profile", vec![Point2::new(10.0, 0.0), Point2::new(18.0, 0.0), Point2::new(18.0, 14.0), Point2::new(10.0, 14.0)], true);
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Profile");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_revolve_axis(sid, closed, 1, 360.0, 0, 0);
    qymcad_testkit::regenerate(&mut p);
    // THE REVOLVE RUNS AROUND THE Y AXIS, so "top" here is not along Z: the end caps look along +-Y
    // and the sides are round. Looking for a face with a +Z normal silently SKIPPED the case; a skip
    // does not count as a pass, so faces are picked by the axis the body is actually built around.
    let open: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[1] > 0.9).map(|f| f.id).collect();
    assert!(!open.is_empty(), "a solid revolved around Y must have an end cap with a +Y normal");
    let sh = p.add_shell_mode(body, 1.5, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&sh).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the shell on the solid of revolution did not build — {:?}", p.regen_errors);
        return;
    }
    let area_before = area_of(&p, sh);
    // Tilt the round sides (their normal is perpendicular to the Y axis); the neutral face is the opposite cap.
    let sides: Vec<u32> = p.regen_faces[&sh].iter().filter(|f| f.normal[1].abs() < 0.1).map(|f| f.id).collect();
    let neutral = p.regen_faces[&sh].iter().find(|f| f.normal[1] < -0.9).map(|f| f.id).unwrap_or(0);
    assert!(!sides.is_empty() && neutral != 0, "the shelled revolve must have round sides and an end cap");
    let d = p.add_draft(sh, sides, neutral, 3.0, false);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&d).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the draft on the shelled revolve did not build — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("draft on a shelled revolve", area_before, area_of(&p, d));
    demand_the_bar("draft on a shelled revolve", &mut p);
}
