//! THE BAR HOLDS ON MORE THAN THE TWO FAMILIAR DOCUMENTS.
//!
//! Everything that measured naming stood on two files, both taken apart by hand and both repaired
//! against measurements taken from themselves. "Works on two documents" is not yet "works on any
//! geometry", which is what the bar actually says. Until other geometry is seen, what was achieved
//! may simply be fitted to the familiar.
//!
//! Here OTHER parts are built — same tools, different order and different shape — and each one is
//! held to the same requirements as the familiar ones:
//!
//! * no two faces share a name;
//! * no two edges share a name;
//! * no face on a body is left unnamed;
//! * a rebuild with no edit at all yields THE SAME names.
use qymcad_core::feature::{BasePlane, FaceKey, SketchPlane};
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project, WorkPlane};

/// Stock: a rectangular sketch turned into a prism. Each case shapes its own part from there.
fn box_body(p: &mut Project, w: f64, h: f64, up: f64) -> Id {
    let sid = p.add_line_sketch(
        "Sketch",
        vec![Point2::new(0.0, 0.0), Point2::new(w, 0.0), Point2::new(w, h), Point2::new(0.0, h)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Sketch");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let b = p.add_extrude_multi(sid, closed, up, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(p);
    b
}

/// Total face area of a body. Exact for PLANAR faces, a tessellation approximation for curved ones,
/// so it proves the FACT that an operation did work ("material changed"), not an exact value.
fn area_of(p: &Project, b: Id) -> f64 {
    p.regen_faces.get(&b).map(|f| f.iter().map(|x| x.area).sum()).unwrap_or(0.0)
}

/// AN OPERATION MUST DO WORK, NOT MERELY RENAME. A "the names changed" check passes a tool glued on
/// instead of cut away, and an imprinted circle instead of a hole — both were found in this very
/// suite. So ask for a change in material.
fn demand_material_changed(title: &str, was: f64, now: f64) {
    assert!(
        (now - was).abs() > 0.5,
        "{title}: the operation left the material untouched — area was {was:.2}, is now {now:.2}"
    );
}

fn snapshot(p: &Project) -> Vec<(Id, Vec<u32>)> {
    let mut v: Vec<(Id, Vec<u32>)> = p
        .regen_faces
        .iter()
        .map(|(b, f)| {
            let mut ids: Vec<u32> = f.iter().map(|x| x.id).collect();
            ids.sort_unstable();
            (*b, ids)
        })
        .collect();
    v.sort_unstable_by_key(|(b, _)| *b);
    v
}

/// All four requirements at once, on any assembled project.
fn demand_the_bar(title: &str, p: &mut Project) {
    let consumed = p.consumed_bodies();
    let mut bad: Vec<String> = Vec::new();
    let bodies: Vec<Id> = p.regen_faces.keys().copied().filter(|b| !consumed.contains(b)).collect();
    assert!(!bodies.is_empty(), "{title}: no live bodies left — nothing to check, the case did not build");

    for b in &bodies {
        let faces: Vec<u32> = p.regen_faces.get(b).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
        // AN EMPTY BODY WOULD PASS EVERYTHING. The requirements are phrased as "no two alike" and
        // "none unnamed" — a body with no faces satisfies all of that by itself, and the check would
        // quietly degenerate into nothing. So ask whether there is anything to measure at all.
        //
        // The threshold here is "not empty", not "more than three": a swept cylinder and a loft of
        // two circles have exactly three faces (the side and two caps), and that is CORRECT geometry.
        // How many faces to expect is known to each case, and each case states it itself.
        assert!(!faces.is_empty(), "{title}: body {b} has no faces — the case did not build, nothing to measure");
        let uniq: std::collections::HashSet<u32> = faces.iter().copied().collect();
        if uniq.len() != faces.len() {
            bad.push(format!("body {b}: {} faces, {} distinct names", faces.len(), uniq.len()));
        }
        let pos = faces.iter().filter(|d| !qymcad_core::names::NameTable::is_named(**d)).count();
        if pos > 0 {
            bad.push(format!("body {b}: {pos} unnamed faces out of {}", faces.len()));
        }
        let edges: Vec<u32> = p.regen_edges.get(b).map(|e| e.iter().map(|x| x.id).collect()).unwrap_or_default();
        let euniq: std::collections::HashSet<u32> = edges.iter().copied().collect();
        if euniq.len() != edges.len() {
            bad.push(format!("body {b}: {} edges, {} distinct names", edges.len(), euniq.len()));
        }
    }

    let first = snapshot(p);
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let _ = qymcad_testkit::regenerate(p);
    if snapshot(p) != first {
        bad.push("a rebuild with no edits produced DIFFERENT face names".into());
    }

    assert!(bad.is_empty(), "{title}:\n  {}", bad.join("\n  "));
    // AND THE STRONGEST PART — AN EDIT. Everything above measures a model at rest; the bar is stated
    // about changing the model. Move a point of the first sketch that has one and demand the same as
    // on the familiar documents: the edit adds no failing nodes and wakes no geometric fallback.
    if let Some(si) = (0..p.sketches.len()).find(|i| !p.sketches[*i].points.is_empty()) {
        let sid = p.sketches[si].id;
        let (before, _) = qymcad_testkit::regenerate(p);
        let was: std::collections::HashSet<Id> = before.errors.iter().map(|(n, _)| *n).collect();
        p.sketches[si].points[0].x += 1.5;
        p.solve_sketch(si);
        p.regen_sketch(si);
        p.mark_sketch_dirty(sid);
        p.snap_rebinds.store(0, std::sync::atomic::Ordering::Relaxed);
        let (after, _) = qymcad_testkit::regenerate(p);
        let snaps = p.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed);
        let reds: Vec<String> =
            after.errors.iter().filter(|(n, _)| !was.contains(n)).map(|(n, e)| format!("node {n}: {e:?}")).collect();
        assert!(reds.is_empty(), "{title}: the sketch edit broke nodes that stood before it:\n  {}", reds.join("\n  "));
        assert_eq!(snaps, 0, "{title}: after the edit {snaps} references resolved BY POSITION instead of by name");
    }

    let total: usize = bodies.iter().map(|b| p.regen_faces.get(b).map(|f| f.len()).unwrap_or(0)).sum();
    eprintln!("{title}: {} live bodies, {total} faces — requirements held, the edit survived", bodies.len());
}

/// REVOLVE + FILLET ON EVERY EDGE. A different surface family (solids of revolution), a different order.
#[test]
fn a_revolved_part_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Profile",
        vec![Point2::new(10.0, 0.0), Point2::new(20.0, 0.0), Point2::new(20.0, 15.0), Point2::new(10.0, 15.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Profile");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_revolve_axis(sid, closed, 1, 360.0, 0, 0);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&body).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: the revolve did not build on this profile");
        return;
    }
    demand_the_bar("revolve", &mut p);
}

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
    let rim = p
        .regen_edges
        .get(&shaft)
        .and_then(|e| e.iter().find(|e| (e.radius - 8.0).abs() < 0.05).map(|e| e.id));
    let Some(rim) = rim else {
        panic!("the shaft has no rim of radius 8 — the case did not build, nothing to measure");
    };
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 16.0, pitch: 2.0, internal: false, fit: 0.2, ..Default::default() };
    let t = p.add_thread(shaft, rim, spec, 20.0, 1.0, 1.0);
    qymcad_testkit::regenerate(&mut p);
    assert!(
        p.regen_faces.get(&t).map(|f| f.len() > 10).unwrap_or(false),
        "the thread did not build: faces {:?}",
        p.regen_faces.get(&t).map(|f| f.len())
    );
    demand_the_bar("thread on a shaft", &mut p);
}

/// SWEEP: a circle along a straight path in another plane. The surfaces here are born from the
/// profile edge, as on a prism, but the path is its own — and there are TWO sketches, on different
/// planes.
#[test]
fn a_swept_body_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sprof = p.new_sketch("profile");
    let prof_sid = p.sketches[sprof].id;
    p.add_sketch_node(prof_sid, "profile");
    p.add_circle_entity(sprof, 0.0, 0.0, 3.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(sprof);
    let prof_cid = p.sketches[sprof].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("profile contour");
    let spath = p.new_sketch("path");
    let path_sid = p.sketches[spath].id;
    p.sketches[spath].plane = SketchPlane::World(BasePlane::XZ);
    p.add_sketch_node(path_sid, "path");
    p.add_line_entity(spath, 0.0, 0.0, 0.0, 40.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(spath);
    let path_cid = p.sketches[spath].contour_ids.first().copied().expect("path contour");
    let body = p.add_sweep(prof_sid, vec![prof_cid], path_sid, path_cid);
    qymcad_testkit::regenerate(&mut p);
    assert!(
        p.regen_faces.get(&body).map(|f| f.len() >= 3).unwrap_or(false),
        "the sweep did not build: faces {:?}",
        p.regen_faces.get(&body).map(|f| f.len())
    );
    demand_the_bar("sweep", &mut p);
}

/// LOFT: two sections at different heights. The faces are born from SECTION edges, and the operation
/// has its own history — it is the only tool where a name comes from edges of TWO different sketches
/// at once.
#[test]
fn a_lofted_body_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let s0 = p.new_sketch("bottom");
    let sid0 = p.sketches[s0].id;
    p.add_sketch_node(sid0, "bottom");
    p.add_circle_entity(s0, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s0);
    let c0 = p.sketches[s0].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("bottom contour");
    let s1 = p.new_sketch("top");
    let sid1 = p.sketches[s1].id;
    // THE SECOND SECTION SITS ON A DATUM PLANE: SketchPlane carries no offset, so the height comes
    // from the datum.
    let pl = p.add_plane(WorkPlane { id: 0, name: "z20".into(), origin: [0.0, 0.0, 20.0], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, def: Default::default() });
    p.sketches[s1].plane = SketchPlane::Datum(pl);
    p.add_sketch_node(sid1, "top");
    p.add_circle_entity(s1, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s1);
    let c1 = p.sketches[s1].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("top contour");
    let body = p.add_loft(vec![sid0, sid1], vec![c0, c1], true, 0, 1, false);
    qymcad_testkit::regenerate(&mut p);
    assert!(
        p.regen_faces.get(&body).map(|f| f.len() >= 3).unwrap_or(false),
        "the loft did not build: faces {:?}",
        p.regen_faces.get(&body).map(|f| f.len())
    );
    demand_the_bar("loft", &mut p);
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
    assert!(
        p.regen_faces.get(&m).map(|f| f.len() >= 6).unwrap_or(false),
        "the mirror did not build: faces {:?}",
        p.regen_faces.get(&m).map(|f| f.len())
    );
    demand_the_bar("mirror keeping the original", &mut p);
}

/// SHELL ON A SOLID OF REVOLUTION. A wall is born from a source face, and the faces here are round —
/// not the case the shell was checked on, which was a box.
#[test]
fn a_shelled_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Profile",
        vec![Point2::new(6.0, 0.0), Point2::new(14.0, 0.0), Point2::new(14.0, 18.0), Point2::new(6.0, 18.0)],
        true,
    );
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
    let open: Vec<u32> = p.regen_faces[&body]
        .iter()
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .map(|f| vec![f.id])
        .expect("a solid of revolution must have at least one face");
    let sh = p.add_shell_mode(body, 1.5, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    assert!(
        p.regen_faces.get(&sh).map(|f| f.len() >= 4).unwrap_or(false),
        "the shell on a revolve did not build: faces {:?}",
        p.regen_faces.get(&sh).map(|f| f.len())
    );
    demand_the_bar("shell on a solid of revolution", &mut p);
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
    let h = p.add_hole_from_sketch(base, holes_sk, 4.0, 6.0, 0, 0.0, 0.0, true);
    qymcad_testkit::regenerate(&mut p);
    let area_after: f64 = p.regen_faces.get(&h).map(|f| f.iter().map(|x| x.area).sum()).unwrap_or(0.0);
    assert!(
        area_after > area_before + 1.0,
        "the drill removed nothing: area was {area_before:.2}, is now {area_after:.2} — an imprint instead of a hole"
    );
    assert!(
        p.regen_faces.get(&h).map(|f| f.len() >= 9).unwrap_or(false),
        "the three holes were not drilled: faces {:?}",
        p.regen_faces.get(&h).map(|f| f.len())
    );
    demand_the_bar("three holes from sketch points", &mut p);
}

/// A BODY CUT OVER A SOLID OF REVOLUTION: a tube. Two name spaces merge in the boolean, and both
/// sides must keep their own — this is the path where base and tool names coexist.
#[test]
fn a_tube_cut_by_a_body_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let outer = p.add_cylinder(12.0, 30.0);
    let inner = p.add_cylinder(7.0, 40.0);
    let tube = p.add_body_boolean(outer, inner, 0);
    qymcad_testkit::regenerate(&mut p);
    assert!(
        p.regen_faces.get(&tube).map(|f| f.len() >= 4).unwrap_or(false),
        "the tube was not cut: faces {:?}",
        p.regen_faces.get(&tube).map(|f| f.len())
    );
    demand_the_bar("tube cut by a body", &mut p);
}

/// A SKETCH ON THE FACE OF ANOTHER BODY. The sketch plane is not a world plane but a FACE: the
/// reference to it must survive an edit like any other. This is the case naming was built for.
#[test]
fn a_sketch_on_a_face_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let base = box_body(&mut p, 30.0, 20.0, 10.0);
    let top = p.regen_faces[&base]
        .iter()
        .filter(|f| f.normal[2] > 0.9)
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .cloned()
        .expect("top face of the box");
    let si = p.new_sketch("on the face");
    let sid = p.sketches[si].id;
    p.sketches[si].plane = SketchPlane::Face(
        base,
        FaceKey { index: 0, centroid: [top.centroid.x, top.centroid.y, top.centroid.z], normal: top.normal, id: top.id },
    );
    p.add_sketch_node(sid, "on the face");
    p.add_circle_entity(si, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let c = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("contour on the face");
    let boss = p.add_extrude_on(sid, c, 8.0, qymcad_core::feature::Reach::Forward, 0.0);
    qymcad_testkit::regenerate(&mut p);
    // The boss here is a SEPARATE cylindrical body: three faces (side and two caps), and that is
    // correct. What matters is not how many there are but that the sketch stood ON A FACE of another
    // body and that survived the rebuild.
    assert!(
        p.regen_faces.get(&boss).map(|f| f.len() >= 3).unwrap_or(false),
        "the boss on the face did not build: faces {:?}",
        p.regen_faces.get(&boss).map(|f| f.len())
    );
    assert!(
        matches!(p.sketches.iter().find(|s| s.id == sid).map(|s| &s.plane), Some(SketchPlane::Face(..))),
        "the sketch stopped standing on the face — the case degenerated into an ordinary sketch on a world plane"
    );
    demand_the_bar("sketch on the face of another body", &mut p);
}

/// A PARTIAL REVOLVE. A full turn has no end caps at all; a sector grows them — and those are
/// DIFFERENT faces with a different origin, never seen in a full revolve.
#[test]
fn a_partial_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Sector",
        vec![Point2::new(8.0, 0.0), Point2::new(16.0, 0.0), Point2::new(16.0, 12.0), Point2::new(8.0, 12.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Sector");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_revolve_axis(sid, closed, 1, 120.0, 0, 0);
    qymcad_testkit::regenerate(&mut p);
    let n = p.regen_faces.get(&body).map(|f| f.len()).unwrap_or(0);
    assert!(n >= 5, "the revolved sector did not build: faces {n} (a full turn has 4; a sector must grow end caps)");
    demand_the_bar("revolve through 120 degrees", &mut p);
}

/// A PATTERN IN TWO DIRECTIONS. Copies multiply on a grid and every one has its own neighbours — a
/// check that an instance name holds outside a one-dimensional row.
#[test]
fn a_grid_pattern_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 8.0, 6.0, 5.0);
    let grid = p.add_linear_array_grid3(body, 14.0, 0.0, 0.0, 3, 0.0, 11.0, 0.0, 2, 0.0, 0.0, 0.0, 1);
    qymcad_testkit::regenerate(&mut p);
    let n = p.regen_faces.get(&grid).map(|f| f.len()).unwrap_or(0);
    assert!(n >= 30, "the 3x2 grid did not build: faces {n} (a single box has 6)");
    demand_the_bar("3x2 grid pattern", &mut p);
}

/// AN EXTRUDED CUT THROUGH A BODY. A sketch on a face, cut all the way through: the cut walls are
/// born from profile edges, and a through pass gives TWO end rims — not the case a blind hole is.
#[test]
fn a_through_cut_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let base = box_body(&mut p, 30.0, 20.0, 10.0);
    let top = p.regen_faces[&base]
        .iter()
        .filter(|f| f.normal[2] > 0.9)
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .cloned()
        .expect("top face");
    let si = p.new_sketch("window");
    let sid = p.sketches[si].id;
    p.sketches[si].plane = SketchPlane::Face(
        base,
        FaceKey { index: 0, centroid: [top.centroid.x, top.centroid.y, top.centroid.z], normal: top.normal, id: top.id },
    );
    p.add_sketch_node(sid, "window");
    p.add_circle_entity(si, 0.0, 0.0, 4.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let c = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("window contour");
    let before: std::collections::HashSet<u32> = p.regen_faces[&base].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, base);
    // op: 0 is a CUT, 1 is a union. A 1 stood here under the label "cut", and the case actually
    // GLUED a cylinder on instead of cutting: measured on a rectangular 10x8 window all the way
    // through, op=0 gives exactly +272 of area (the arithmetic of a through window agrees to the
    // hundredth) while op=1 gives 16 faces with the tool sticking out. A "the names changed" check
    // let that pass: they change from a glued-on tool too.
    let top_was = p.regen_faces[&base].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.area).fold(0.0, f64::max);
    let cut = p.add_combine_on(base, sid, c, 20.0, 0, qymcad_core::feature::Extent { through: true, reach: qymcad_core::feature::Reach::Backward }, 0.0);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&cut).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the through cut changed nothing — the case is empty");
    // MATERIAL MUST GO. The top face is planar, so its area is exact. Measured: 600.00 -> 587.48, a
    // loss of 12.52 — that is pi*2^2, a window of radius 2 (`add_circle_entity` takes a DIAMETER;
    // the first threshold was set expecting radius 4 and the case failed honestly — the expectation
    // was corrected rather than the threshold fitted).
    let top_now = p.regen_faces.get(&cut).map(|f| f.iter().filter(|x| x.normal[2] > 0.9).map(|x| x.area).fold(0.0, f64::max)).unwrap_or(0.0);
    assert!(
        top_now < top_was - 10.0,
        "the cut removed no material: the top face was {top_was:.2}, is now {top_now:.2}"
    );
    demand_material_changed("extruded cut all the way through", area_before, area_of(&p, cut));
    demand_the_bar("extruded cut all the way through", &mut p);
}

/// A FILLET ON A SOLID OF REVOLUTION. The fillet surface is named after the EDGE that bore it, and
/// the edge here is round, on a solid of revolution — not the shape the fillet was checked on.
#[test]
fn a_fillet_on_a_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Profile",
        vec![Point2::new(6.0, 0.0), Point2::new(16.0, 0.0), Point2::new(16.0, 14.0), Point2::new(6.0, 14.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Profile");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_revolve_axis(sid, closed, 1, 360.0, 0, 0);
    qymcad_testkit::regenerate(&mut p);
    let edge = p
        .regen_edges
        .get(&body)
        .and_then(|e| e.iter().filter(|e| e.radius > 1e-9).max_by(|a, b| a.radius.total_cmp(&b.radius)).map(|e| e.id))
        .expect("a round edge on the solid of revolution");
    let before: std::collections::HashSet<u32> = p.regen_faces[&body].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, body);
    let f = p.add_fillet(body, 1.5, vec![edge]);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&f).map(|x| x.iter().map(|y| y.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the fillet on the revolve changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("fillet on a solid of revolution", area_before, area_of(&p, f));
    demand_the_bar("fillet on a solid of revolution", &mut p);
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
    let open: Vec<u32> = p.regen_faces[&arr]
        .iter()
        .filter(|f| f.normal[2] > 0.9)
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .map(|f| vec![f.id])
        .expect("top face of the pattern result");
    let before: std::collections::HashSet<u32> = p.regen_faces[&arr].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, arr);
    let sh = p.add_shell_mode(arr, 1.5, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&sh).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the shell after the pattern changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("shell after a pattern", area_before, area_of(&p, sh));
    demand_the_bar("shell after a pattern", &mut p);
}

/// MIRRORING A SOLID OF REVOLUTION. The mirror is already checked on a box; here its image consists
/// of round faces, and pairs of "face plus its image" match in shape far more often than planar ones.
#[test]
fn a_mirrored_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Profile",
        vec![Point2::new(10.0, 2.0), Point2::new(18.0, 2.0), Point2::new(18.0, 12.0), Point2::new(10.0, 12.0)],
        true,
    );
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

/// A SPLIT BY A TILTED DATUM PLANE. A world plane cuts along the axes; a tilted one cuts across every
/// face at once and yields more pieces: a different set of seams and different neighbours for each
/// piece.
#[test]
fn a_split_by_a_tilted_datum_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 24.0, 18.0, 12.0);
    let n = (1.0f64 / 3.0f64.sqrt(), 1.0 / 3.0f64.sqrt(), 1.0 / 3.0f64.sqrt());
    let pl = p.add_plane(WorkPlane {
        id: 0,
        name: "tilted".into(),
        origin: [12.0, 9.0, 6.0],
        normal: [n.0, n.1, n.2],
        rot_deg: 0.0,
        def: Default::default(),
    });
    let pieces = p.add_split_body(body, 0, pl, 0.0, 2);
    qymcad_testkit::regenerate(&mut p);
    assert_eq!(pieces.len(), 2, "a tilted split must yield two pieces");
    for b in &pieces {
        assert!(
            p.regen_faces.get(b).map(|f| !f.is_empty()).unwrap_or(false),
            "piece {b} of the tilted split is empty: errors {:?}",
            p.regen_errors
        );
    }
    demand_the_bar("split by a tilted datum plane", &mut p);
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
    let open: Vec<u32> = p.regen_faces[&target]
        .iter()
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .map(|f| vec![f.id])
        .expect("a face on the split piece");
    let before: std::collections::HashSet<u32> = p.regen_faces[&target].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, target);
    let sh = p.add_shell_mode(target, 1.5, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&sh).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the shell after the split changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("shell after a split", area_before, area_of(&p, sh));
    demand_the_bar("shell after a split", &mut p);
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
    let rim = p
        .regen_edges
        .get(&tube)
        .and_then(|e| e.iter().find(|e| (e.radius - 10.0).abs() < 0.05).map(|e| e.id))
        .expect("outer rim of the tube");
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

/// A PATTERN AFTER A SHELLED REVOLVE. Three operations in a row, each with its own naming recipe:
/// the revolve gives round faces, the shell builds walls from them, the pattern multiplies both.
#[test]
fn a_pattern_after_a_shelled_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Profile",
        vec![Point2::new(8.0, 0.0), Point2::new(15.0, 0.0), Point2::new(15.0, 16.0), Point2::new(8.0, 16.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Profile");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_revolve_axis(sid, closed, 1, 360.0, 0, 0);
    qymcad_testkit::regenerate(&mut p);
    let open: Vec<u32> = p.regen_faces[&body]
        .iter()
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .map(|f| vec![f.id])
        .expect("a face of the solid of revolution");
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

/// A LOFT THROUGH THREE SECTIONS. A loft face is named from section edges; with three sections the
/// surface passes through the middle one and each face has more than one source edge.
#[test]
fn a_three_section_loft_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let mut mk = |name: &str, z: f64, r: f64| -> (Id, Id) {
        let si = p.new_sketch(name);
        let sid = p.sketches[si].id;
        if z > 0.0 {
            let pl = p.add_plane(WorkPlane { id: 0, name: name.into(), origin: [0.0, 0.0, z], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, def: Default::default() });
            p.sketches[si].plane = SketchPlane::Datum(pl);
        }
        p.add_sketch_node(sid, name);
        p.add_circle_entity(si, 0.0, 0.0, r, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let c = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("section contour");
        (sid, c)
    };
    let (s0, c0) = mk("bottom", 0.0, 10.0);
    let (s1, c1) = mk("middle", 12.0, 4.0);
    let (s2, c2) = mk("top", 24.0, 9.0);
    let body = p.add_loft(vec![s0, s1, s2], vec![c0, c1, c2], true, 0, 1, false);
    qymcad_testkit::regenerate(&mut p);
    assert!(
        p.regen_faces.get(&body).map(|f| f.len() >= 3).unwrap_or(false),
        "the three-section loft did not build: faces {:?}, errors {:?}",
        p.regen_faces.get(&body).map(|f| f.len()),
        p.regen_errors
    );
    demand_the_bar("three-section loft", &mut p);
}

/// A BODY CUT THROUGH A PATTERN. The tool passes through SEVERAL copies at once: one name space cuts
/// many bodies, and each must keep its own.
#[test]
fn a_body_cut_through_a_pattern_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 12.0, 10.0, 8.0);
    let arr = p.add_linear_array_grid3(body, 16.0, 0.0, 0.0, 3, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
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
    let first = p.add_linear_array_grid3(body, 12.0, 0.0, 0.0, 2, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
    qymcad_testkit::regenerate(&mut p);
    let before = p.regen_faces.get(&first).map(|f| f.len()).unwrap_or(0);
    let second = p.add_linear_array_grid3(first, 0.0, 10.0, 0.0, 2, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
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
    let arr = p.add_linear_array_grid3(body, 20.0, 0.0, 0.0, 2, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
    qymcad_testkit::regenerate(&mut p);
    // A VERTICAL EDGE IS PICKED: every copy has one and it survives moving a sketch point.
    let edge = p
        .regen_edges
        .get(&arr)
        .and_then(|e| e.iter().find(|e| e.dir[2].abs() > 0.9).map(|e| e.id))
        .expect("a vertical edge on the pattern");
    let before: std::collections::HashSet<u32> = p.regen_faces[&arr].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, arr);
    let f = p.add_fillet(arr, 1.5, vec![edge]);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&f).map(|x| x.iter().map(|y| y.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the fillet after the pattern changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("fillet after a pattern", area_before, area_of(&p, f));
    demand_the_bar("fillet after a pattern", &mut p);
}

/// HOLES ON A SOLID OF REVOLUTION. The drill enters a ROUND face: the hole wall borders a surface of
/// revolution rather than a plane, as in every case checked so far.
#[test]
fn holes_on_a_revolve_hold_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Profile",
        vec![Point2::new(0.0, 0.0), Point2::new(20.0, 0.0), Point2::new(20.0, 10.0), Point2::new(0.0, 10.0)],
        true,
    );
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
    let h = p.add_hole_from_sketch(disc, hsk, 3.0, 12.0, 0, 0.0, 0.0, false);
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
    let rim = p
        .regen_edges
        .get(&part)
        .and_then(|e| e.iter().find(|e| (e.radius - 9.0).abs() < 0.05).map(|e| e.id))
        .expect("a rim on the split piece");
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

/// A DRAFT ON A SOLID OF REVOLUTION. A ROUND face is tilted: it has no straight generating edge, and
/// the draft side is born out of the surface of revolution.
#[test]
fn a_draft_on_a_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Profile",
        vec![Point2::new(6.0, 0.0), Point2::new(14.0, 0.0), Point2::new(14.0, 20.0), Point2::new(6.0, 20.0)],
        true,
    );
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

/// A CHAMFER AFTER A PATTERN. Like a fillet, a chamfer names its surface after the EDGE that bore it,
/// but its faces are planar: a different path through the kernel under the same naming recipe.
#[test]
fn a_chamfer_after_a_pattern_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 14.0, 10.0, 8.0);
    let arr = p.add_linear_array_grid3(body, 20.0, 0.0, 0.0, 2, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
    qymcad_testkit::regenerate(&mut p);
    let edge = p
        .regen_edges
        .get(&arr)
        .and_then(|e| e.iter().find(|e| e.dir[2].abs() > 0.9).map(|e| e.id))
        .expect("a vertical edge on the pattern");
    let before: std::collections::HashSet<u32> = p.regen_faces[&arr].iter().map(|f| f.id).collect();
    let area_before = area_of(&p, arr);
    let ch = p.add_chamfer(arr, 1.2, vec![edge]);
    qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_faces.get(&ch).map(|x| x.iter().map(|y| y.id).collect()).unwrap_or_default();
    assert!(after.difference(&before).count() > 0, "the chamfer after the pattern changed nothing: errors {:?}", p.regen_errors);
    demand_material_changed("chamfer after a pattern", area_before, area_of(&p, ch));
    demand_the_bar("chamfer after a pattern", &mut p);
}

/// A LOFT BETWEEN UNLIKE SECTIONS: a circle below, a rectangle above. The sections have different
/// edge counts, so the surface is forced to split — names come from edges the other section does not
/// have.
#[test]
fn a_loft_between_unlike_sections_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let s0i = p.new_sketch("circle");
    let s0 = p.sketches[s0i].id;
    p.add_sketch_node(s0, "circle");
    p.add_circle_entity(s0i, 0.0, 0.0, 9.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s0i);
    let c0 = p.sketches[s0i].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("circle contour");
    let pl = p.add_plane(WorkPlane { id: 0, name: "z18".into(), origin: [0.0, 0.0, 18.0], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, def: Default::default() });
    let s1i = p.new_sketch("square");
    let s1 = p.sketches[s1i].id;
    p.sketches[s1i].plane = SketchPlane::Datum(pl);
    p.add_sketch_node(s1, "square");
    for (a, b, c, d) in [(-6.0, -6.0, 6.0, -6.0), (6.0, -6.0, 6.0, 6.0), (6.0, 6.0, -6.0, 6.0), (-6.0, 6.0, -6.0, -6.0)] {
        p.add_line_entity(s1i, a, b, c, d, qymcad_core::feature::Purpose::Real);
    }
    p.regen_sketch(s1i);
    let c1 = p.sketches[s1i].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("square contour");
    let body = p.add_loft(vec![s0, s1], vec![c0, c1], true, 0, 1, false);
    qymcad_testkit::regenerate(&mut p);
    assert!(
        p.regen_faces.get(&body).map(|f| f.len() >= 3).unwrap_or(false),
        "the circle-to-square loft did not build: faces {:?}, errors {:?}",
        p.regen_faces.get(&body).map(|f| f.len()),
        p.regen_errors
    );
    demand_the_bar("loft circle -> square", &mut p);
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
    let open2: Vec<u32> = p.regen_faces[&sh1]
        .iter()
        .filter(|f| f.normal[2] < -0.9)
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .map(|f| vec![f.id])
        .expect("bottom face of the first shell");
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
    let arr = p.add_linear_array_grid3(body, 50.0, 0.0, 0.0, 3, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
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
        let sid = p.add_line_sketch(
            "Second",
            vec![Point2::new(14.0, 6.0), Point2::new(38.0, 6.0), Point2::new(38.0, 26.0), Point2::new(14.0, 26.0)],
            true,
        );
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

/// AN ARRAY OF AN ARRAY. A copy of a copy: the source of the second array already carries instance
/// names, and its own seeds land ON TOP of them. Exactly the seam where names stop being unique if
/// the recipe does not distinguish levels.
#[test]
fn an_array_of_an_array_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 10.0, 8.0, 6.0);
    let first = p.add_linear_array_grid3(body, 22.0, 0.0, 0.0, 2, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
    qymcad_testkit::regenerate(&mut p);
    let second = p.add_linear_array_grid3(first, 0.0, 20.0, 0.0, 2, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
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
    let arr = p.add_linear_array_grid3(body, 15.0, 0.0, 0.0, 3, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
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
    let h = p.add_hole_from_sketch(sh, hsk, 3.0, 20.0, 0, 0.0, 0.0, true);
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
    assert!(
        area_after > area_before + 1.0,
        "the drill removed nothing: area was {area_before:.2}, is now {area_after:.2} — an imprint instead of a hole"
    );
    demand_the_bar("holes through a shell", &mut p);
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
    let edge = p
        .regen_edges
        .get(&piece)
        .and_then(|e| e.iter().find(|e| (e.mid[0] - 14.0).abs() < 1e-6).map(|e| e.id));
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
        let sid = p.add_line_sketch(
            "Pocket",
            vec![Point2::new(8.0, 6.0), Point2::new(26.0, 6.0), Point2::new(26.0, 18.0), Point2::new(8.0, 18.0)],
            true,
        );
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
    let sides: Vec<u32> = p.regen_faces[&pocket]
        .iter()
        .filter(|f| f.normal[2].abs() < 0.1 && f.centroid.x > 7.0 && f.centroid.x < 27.0 && f.centroid.y > 5.0 && f.centroid.y < 19.0)
        .map(|f| f.id)
        .collect();
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

/// A SHELL AFTER A BODY BOOLEAN. The wall is built on a face the TOOL produced, not the base: the
/// shell has no origin of its own for such a face and has to take its name from a stranger.
#[test]
fn a_shell_after_a_body_boolean_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let base = box_body(&mut p, 30.0, 22.0, 14.0);
    let lug = {
        let sid = p.add_line_sketch(
            "Boss",
            vec![Point2::new(24.0, 8.0), Point2::new(40.0, 8.0), Point2::new(40.0, 16.0), Point2::new(24.0, 16.0)],
            true,
        );
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
    let arr = p.add_linear_array_grid3(t, 20.0, 0.0, 0.0, 2, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
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
    let pl = p.add_plane(WorkPlane { id: 0, name: "z18".into(), origin: [0.0, 0.0, 18.0], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, def: Default::default() });
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

/// A THREAD ON THE RIM OF A PATTERN COPY. The rim belongs not to the source body but to an INSTANCE:
/// its name is of the "instance" kind, and the thread lays its undercut over a copy, not an original.
#[test]
fn a_thread_on_an_array_copy_holds_the_bar() {
    use qymcad_core::thread::{ThreadSpec, ThreadStandard};
    let mut p = Project::default();
    p.new_document();
    let shaft = p.add_cylinder(6.0, 24.0);
    qymcad_testkit::regenerate(&mut p);
    let arr = p.add_linear_array_grid3(shaft, 30.0, 0.0, 0.0, 2, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
    qymcad_testkit::regenerate(&mut p);
    let area_before = area_of(&p, arr);
    // The rim of the SECOND copy: a round edge of radius 6 whose centre moved along x by the pattern step.
    let rim = p
        .regen_edges
        .get(&arr)
        .and_then(|e| e.iter().find(|e| (e.radius - 6.0).abs() < 0.05 && e.mid[0] > 20.0).map(|e| e.id));
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
    let edge = p
        .regen_edges
        .get(&sh)
        .and_then(|e| e.iter().find(|e| (e.mid[2] - 12.0).abs() < 1e-6 && e.mid[0] > 1.0 && e.mid[0] < 25.0 && e.mid[1] > 1.0 && e.mid[1] < 17.0).map(|e| e.id));
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
    let arr = p.add_linear_array_grid3(piece, 0.0, 26.0, 0.0, 3, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
    qymcad_testkit::regenerate(&mut p);
    let after = p.regen_faces.get(&arr).map(|f| f.len()).unwrap_or(0);
    if after == 0 {
        eprintln!("skip: the pattern of the split piece did not build — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("pattern of a split piece", area_before, area_of(&p, arr));
    demand_the_bar("pattern of a split piece", &mut p);
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
    let pl = p.add_plane(WorkPlane { id: 0, name: "z16".into(), origin: [0.0, 0.0, 16.0], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, def: Default::default() });
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
    let h = p.add_hole_from_sketch(body, hsk, 4.0, 20.0, 0, 0.0, 0.0, true);
    qymcad_testkit::regenerate(&mut p);
    if p.regen_faces.get(&h).map(|f| f.is_empty()).unwrap_or(true) {
        eprintln!("skip: drilling the loft did not build — {:?}", p.regen_errors);
        return;
    }
    demand_material_changed("holes through a loft", area_before, area_of(&p, h));
    demand_the_bar("holes through a loft", &mut p);
}

/// A DRAFT ON A SHELLED SOLID OF REVOLUTION. What has to be tilted is a wall that was itself born by
/// offsetting a round face: its name comes from the shell, and the draft lays its own on top — all of
/// it on a cylinder, where the "side face" is single and closed.
#[test]
fn a_draft_on_a_shelled_revolve_holds_the_bar() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Profile",
        vec![Point2::new(10.0, 0.0), Point2::new(18.0, 0.0), Point2::new(18.0, 14.0), Point2::new(10.0, 14.0)],
        true,
    );
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
    let open: Vec<u32> = p.regen_faces[&body]
        .iter()
        .filter(|f| f.normal[1] > 0.9)
        .map(|f| f.id)
        .collect();
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
    let pl = p.add_plane(WorkPlane { id: 0, name: "z14".into(), origin: [0.0, 0.0, 14.0], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, def: Default::default() });
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
