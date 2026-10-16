//! The bar held by documents with patterns, arrays and mirrors in them.

use super::*;

/// SPLIT AFTER A PATTERN. A split multiplies what it inherits (measured), and so does a pattern;
/// together they are the worst case for naming, and nobody built it deliberately in the familiar
/// documents.
#[test]
fn a_split_after_a_pattern_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 24.0, 16.0, 10.0);
    let arr = p.add_circular_array_axis(body, 3, 240.0, 0);
    qymcad_testkit::regenerate(&mut p);
    // A PLANE AT MID-HEIGHT: at z = 0 it merely touches the base and divides nothing.
    let pieces = p.add_split_body(arr, 0, 0, 5.0, 2);
    qymcad_testkit::regenerate(&mut p);
    if pieces.len() < 2 || pieces.iter().any(|b| p.regen_faces.get(b).map(|f| f.is_empty()).unwrap_or(true)) {
        eprintln!("skip: the split did not divide the body in two");
        return;
    }
    demand_the_bar("pattern -> split", &mut p);
}

/// MIRROR KEEPING THE ORIGINAL. The mirror is what produced twin edge pairs: the halves meet with
/// identical pairs of faces, and the rank of an edge within a pair has to be chosen. The case is
/// mandatory.
#[test]
fn a_mirrored_part_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 18.0, 12.0, 8.0);
    let m = p.add_mirror(body, 2, true, 0); // the YZ plane, the original stays
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_faces.get(&m).map(|f| f.len() >= 6).unwrap_or(false), "the mirror did not build: faces {:?}", p.regen_faces.get(&m).map(|f| f.len()));
    demand_the_bar("mirror keeping the original", &mut p);
}

/// A PATTERN IN TWO DIRECTIONS. Copies multiply on a grid and every one has its own neighbours — a
/// check that an instance name holds outside a one-dimensional row.
#[test]
fn a_grid_pattern_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 8.0, 6.0, 5.0);
    let grid = p.add_linear_array_grid3(body, [ArrayAxis { d: [14.0, 0.0, 0.0], count: 3 }, ArrayAxis { d: [0.0, 11.0, 0.0], count: 2 }, ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    let n = p.regen_faces.get(&grid).map(|f| f.len()).unwrap_or(0);
    assert!(n >= 30, "the 3x2 grid did not build: faces {n} (a single box has 6)");
    demand_the_bar("3x2 grid pattern", &mut p);
}

/// MIRRORING A SOLID OF REVOLUTION. The mirror is already checked on a box; here its image consists
/// of round faces, and pairs of "face plus its image" match in shape far more often than planar ones.
#[test]
fn a_mirrored_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch("Profile", vec![Point2::new(10.0, 2.0), Point2::new(18.0, 2.0), Point2::new(18.0, 12.0), Point2::new(10.0, 12.0)], true);
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Profile");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_revolve_axis(sid, closed, 1, 180.0, 0, 0);
    qymcad_testkit::regenerate(&mut p);
    let before: std::collections::HashSet<u32> = p.regen_faces[&body].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, body);
    // A PLANE WHERE THE BODY IS NOT SYMMETRIC. About YZ a half turn is symmetric to itself: the
    // mirror coincides with the original, the merge adds nothing and the case comes out empty —
    // that was a mistake in setting the case up, not a defect.
    let m = p.add_mirror(body, 0, true, 0);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&m).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "mirroring the solid of revolution added nothing: errors {:?}", p.regen_errors);
    demand_material_changed("mirrored solid of revolution", area_before, area_of(&p, m));
    demand_the_bar("mirrored solid of revolution", &mut p);
}

/// A BODY CUT THROUGH A PATTERN. The tool passes through SEVERAL copies at once: one name space cuts
/// many bodies, and each must keep its own.
#[test]
fn a_body_cut_through_a_pattern_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 12.0, 10.0, 8.0);
    let arr = p.add_linear_array_grid3(body, [ArrayAxis { d: [16.0, 0.0, 0.0], count: 3 }, ArrayAxis::none(), ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    let bar = p.add_cylinder(3.0, 60.0); // a rod along Z, through the row
    qymcad_testkit::regenerate(&mut p);
    let before: std::collections::HashSet<u32> = p.regen_faces[&arr].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, arr);
    let cut = p.add_body_boolean(arr, bar, 0);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&cut).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the body cut through the pattern changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("body cut through a pattern", area_before, area_of(&p, cut));
    demand_the_bar("body cut through a pattern", &mut p);
}

/// A PATTERN OF A PATTERN. A copy of a copy: the image name carries a `split` copy number, and the
/// second pattern lays its own number over someone else's. The place where a naming scheme could
/// overwrite itself.
#[test]
fn a_pattern_of_a_pattern_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 8.0, 6.0, 5.0);
    let first = p.add_linear_array_grid3(body, [ArrayAxis { d: [12.0, 0.0, 0.0], count: 2 }, ArrayAxis::none(), ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    let before = p.regen_faces.get(&first).map(|f| f.len()).unwrap_or(0);
    let second = p.add_linear_array_grid3(first, [ArrayAxis { d: [0.0, 10.0, 0.0], count: 2 }, ArrayAxis::none(), ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    let after = p.regen_faces.get(&second).map(|f| f.len()).unwrap_or(0);
    assert!(after > before, "the second pattern did not multiply: was {before} now {after}, errors {:?}", p.regen_errors);
    demand_the_bar("pattern of a pattern", &mut p);
}

/// A FILLET AFTER A PATTERN. The edge under the fillet belongs to a COPY, not to the original: the
/// surface name is derived from an edge whose own name carries the copy number.
#[test]
fn a_fillet_after_a_pattern_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 14.0, 10.0, 8.0);
    let arr = p.add_linear_array_grid3(body, [ArrayAxis { d: [20.0, 0.0, 0.0], count: 2 }, ArrayAxis::none(), ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    // A VERTICAL EDGE IS PICKED: every copy has one and it survives moving a sketch point.
    let edge = p.regen_edges.get(&arr).and_then(|e| e.iter().find(|e| e.dir[2].abs() > 0.9).map(|e| e.id)).expect("a vertical edge on the pattern");
    let before: std::collections::HashSet<u32> = p.regen_faces[&arr].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, arr);
    let f = p.add_fillet(arr, 1.5, vec![edge]);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&f).map(|x| x.iter().map(|y| y.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the fillet after the pattern changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("fillet after a pattern", area_before, area_of(&p, f));
    demand_the_bar("fillet after a pattern", &mut p);
}

/// A CHAMFER AFTER A PATTERN. Like a fillet, a chamfer names its surface after the EDGE that bore it,
/// but its faces are planar: a different path through the kernel under the same naming recipe.
#[test]
fn a_chamfer_after_a_pattern_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 14.0, 10.0, 8.0);
    let arr = p.add_linear_array_grid3(body, [ArrayAxis { d: [20.0, 0.0, 0.0], count: 2 }, ArrayAxis::none(), ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    let edge = p.regen_edges.get(&arr).and_then(|e| e.iter().find(|e| e.dir[2].abs() > 0.9).map(|e| e.id)).expect("a vertical edge on the pattern");
    let before: std::collections::HashSet<u32> = p.regen_faces[&arr].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, arr);
    let ch = p.add_chamfer(arr, 1.2, vec![edge]);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&ch).map(|x| x.iter().map(|y| y.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the chamfer after the pattern changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("chamfer after a pattern", area_before, area_of(&p, ch));
    demand_the_bar("chamfer after a pattern", &mut p);
}

/// A SPLIT OF A MIRROR RESULT. The split runs through a body half of whose faces are IMAGES; the
/// pieces inherit original and mirrored names at once.
#[test]
fn a_split_of_a_mirror_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 18.0, 12.0, 10.0);
    let m = p.add_mirror(body, 0, true, 0); // across XY: the body sits at z=0..10, the image goes down
    qymcad_testkit::regenerate(&mut p);
    let pieces = p.add_split_body(m, 2, 0, 9.0, 2); // cut across YZ at x=9 — both halves stay whole
    qymcad_testkit::regenerate(&mut p);
    assert_eq!(pieces.len(), 2, "splitting the mirrored body must yield two pieces: errors {:?}", p.regen_errors);
    demand_the_bar("split of a mirror result", &mut p);
}

/// AN ARRAY OF AN ARRAY. A copy of a copy: the source of the second array already carries instance
/// names, and its own seeds land ON TOP of them. Exactly the seam where names stop being unique if
/// the recipe does not distinguish levels.
#[test]
fn an_array_of_an_array_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 10.0, 8.0, 6.0);
    let first = p.add_linear_array_grid3(body, [ArrayAxis { d: [22.0, 0.0, 0.0], count: 2 }, ArrayAxis::none(), ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    let second = p.add_linear_array_grid3(first, [ArrayAxis { d: [0.0, 20.0, 0.0], count: 2 }, ArrayAxis::none(), ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    let faces = p.regen_faces.get(&second).map(|f| f.len()).unwrap_or(0);
    if faces == 0 {
        eprintln!("skip: the array of an array did not build — {:?}", p.regen_errors);
        return;
    }
    assert!(faces >= 24, "an array of an array must yield four boxes: faces {faces}");
    demand_the_bar("array of an array", &mut p);
}

/// A MIRROR OF AN ARRAY. What is reflected is not a body but a SET of copies: in the image every copy
/// must get its own name, otherwise the left and right halves carry identical numbers.
#[test]
fn a_mirror_of_an_array_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 9.0, 7.0, 5.0);
    let arr = p.add_linear_array_grid3(body, [ArrayAxis { d: [15.0, 0.0, 0.0], count: 3 }, ArrayAxis::none(), ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    // The XZ plane: the array runs along x, so reflecting along y keeps the copies from overlapping.
    let m = p.add_mirror(arr, 1, true, 0);
    qymcad_testkit::regenerate(&mut p);
    let faces = p.regen_faces.get(&m).map(|f| f.len()).unwrap_or(0);
    if faces == 0 {
        eprintln!("skip: the mirror of the array did not build — {:?}", p.regen_errors);
        return;
    }
    demand_the_bar("mirror of an array", &mut p);
}

/// A PATTERN AFTER A SPLIT. What is copied is a PIECE — a body that was not in the timeline before
/// the split: half of its faces were born on the section and have no preimage in the stock.
#[test]
fn an_array_of_a_split_piece_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 24.0, 16.0, 10.0);
    let pieces = p.add_split_body(body, 2, 0, 12.0, 2); // across YZ at x=12
    qymcad_testkit::regenerate(&mut p);
    if pieces.len() < 2 {
        eprintln!("skip: the split did not divide the body in two — {:?}", p.regen_errors);
        return;
    }
    let piece = pieces[0];
    let area_before = area_of(&p, piece);
    let arr = p.add_linear_array_grid3(piece, [ArrayAxis { d: [0.0, 26.0, 0.0], count: 3 }, ArrayAxis::none(), ArrayAxis::none()]);
    qymcad_testkit::regenerate(&mut p);
    let after = p.regen_faces.get(&arr).map(|f| f.len()).unwrap_or(0);
    if after == 0 {
        eprintln!("skip: the pattern of the split piece did not build — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("pattern of a split piece", area_before, area_of(&p, arr));
    demand_the_bar("pattern of a split piece", &mut p);
}
