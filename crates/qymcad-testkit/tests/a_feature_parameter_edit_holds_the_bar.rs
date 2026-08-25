//! EDITING A FEATURE PARAMETER IS AN EDIT TOO.
//!
//! The whole bar suite edits the same thing: it moves a sketch point. But a person in a CAD system
//! changes other things too — the thickness of a shell, the radius of a fillet, the angle of a
//! pattern — and such an edit takes a DIFFERENT path: the value arrives as an expression through
//! `feat_dims` rather than through a sketch recompute. Until that path is checked, "a reference
//! survives an edit" is said about only half of the edits.
//!
//! The requirement is the same as on the familiar documents: not one NEW failing node, not one
//! unnamed face, not one duplicate — and EVERY name that existed before the edit exists after it. The
//! face count may change: as the copies of a pattern move apart they legitimately open up new faces.
//! What is required is not an equal count but that nothing old was lost.
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project};

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

fn node_of(p: &Project, body: Id) -> Id {
    p.timeline.iter().find(|n| n.kind.bodies().contains(&body)).map(|n| n.id).expect("the body has a node")
}

fn live_names(p: &Project) -> Vec<(Id, Vec<u32>)> {
    let consumed = p.consumed_bodies();
    let mut v: Vec<(Id, Vec<u32>)> = p
        .regen_faces
        .iter()
        .filter(|(b, _)| !consumed.contains(b))
        .map(|(b, f)| {
            let mut ids: Vec<u32> = f.iter().map(|x| x.id).collect();
            ids.sort_unstable();
            (*b, ids)
        })
        .collect();
    v.sort_unstable_by_key(|(b, _)| *b);
    v
}

/// Edit the `key` parameter of the feature that produced `body`, and all the requirements at once.
fn demand_after_param_edit(title: &str, p: &mut Project, body: Id, key: &str, value: &str) {
    let (before, _) = qymcad_testkit::regenerate(p);
    let was_red: std::collections::HashSet<Id> = before.errors.iter().map(|(n, _)| *n).collect();
    let was = live_names(p);
    assert!(!was.is_empty(), "{title}: there are no live bodies — there is nothing to check the edit on");

    // AN EMPTY EDIT WOULD PASS EVERYTHING. Get the dimension key wrong and the value lands nowhere,
    // the body does not stir, and every requirement below is satisfied by itself. So ask first
    // whether the geometry RESPONDED at all: the total area of the live bodies must become different.
    let area_of = |p: &Project| -> f64 {
        let consumed = p.consumed_bodies();
        p.regen_faces.iter().filter(|(b, _)| !consumed.contains(b)).flat_map(|(_, f)| f.iter().map(|x| x.area)).sum()
    };
    let area_was = area_of(p);

    let node = node_of(p, body);
    p.set_feat_dim(node, key, value.to_string());
    let (after, _) = qymcad_testkit::regenerate(p);
    let area_now = area_of(p);
    assert!(
        (area_now - area_was).abs() > 1e-6,
        "{title}: the edit \"{key} = {value}\" changed NOTHING (the area stayed {area_was:.3}) — the dimension key is wrong and the case checks nothing"
    );

    let reds: Vec<String> =
        after.errors.iter().filter(|(n, _)| !was_red.contains(n)).map(|(n, e)| format!("node {n}: {e:?}")).collect();
    assert!(reds.is_empty(), "{title}: the edit \"{key} = {value}\" broke nodes that stood before it:\n  {}", reds.join("\n  "));

    let now = live_names(p);
    let mut bad: Vec<String> = Vec::new();
    for (b, w) in &was {
        let Some((_, n)) = now.iter().find(|(x, _)| x == b) else {
            bad.push(format!("body {b} disappeared after the edit"));
            continue;
        };
        let lost: Vec<String> = w.iter().filter(|x| !n.contains(x)).map(|x| p.names.describe(*x)).collect();
        if !lost.is_empty() {
            bad.push(format!("body {b}: names lost ({}): {}", lost.len(), lost.join(", ")));
        }
        let unnamed = n.iter().filter(|x| !qymcad_core::names::NameTable::is_named(**x)).count();
        if unnamed > 0 {
            bad.push(format!("body {b}: {unnamed} unnamed faces out of {}", n.len()));
        }
        let uniq: std::collections::HashSet<u32> = n.iter().copied().collect();
        if uniq.len() != n.len() {
            bad.push(format!("body {b}: {} faces, {} distinct names", n.len(), uniq.len()));
        }
    }
    assert!(bad.is_empty(), "{title}: the edit \"{key} = {value}\":\n  {}", bad.join("\n  "));
}

/// SHELL THICKNESS. The wall dimension changes and the topology itself must stay the same.
/// Measured: 11 faces before and 11 after, all 11 names survived.
#[test]
fn a_shell_thickness_edit_keeps_every_name() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 30.0, 20.0, 12.0);
    let open: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    let sh = p.add_shell_mode(body, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    demand_after_param_edit("shell thickness", &mut p, sh, "thickness", "2.6");
}

/// FILLET RADIUS. The fillet face is rebuilt entirely and its name must stay the same — otherwise a
/// reference to it slides away on a plain change of a number. Measured: 7 faces before and after.
#[test]
fn a_fillet_radius_edit_keeps_every_name() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 24.0, 18.0, 10.0);
    let edge = p.regen_edges[&body].iter().map(|e| e.id).next().expect("the box has edges");
    let f = p.add_fillet(body, 1.5, vec![edge]);
    qymcad_testkit::regenerate(&mut p);
    demand_after_param_edit("fillet radius", &mut p, f, "radius", "2.4");
}

/// PATTERN ANGLE. The copies move apart and there are MORE faces — that is legitimate. What is not
/// legitimate is losing even one former name. Measured: 12 faces become 18, all 12 former names are
/// in place, the new ones are named from the recipe (instance/wall), and none are unnamed.
#[test]
fn a_circular_array_angle_edit_keeps_every_name() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 10.0, 8.0, 6.0);
    let arr = p.add_circular_array_axis(body, 3, 240.0, 0);
    qymcad_testkit::regenerate(&mut p);
    demand_after_param_edit("pattern angle", &mut p, arr, "angle", "300");
}

/// EXTRUDE HEIGHT UNDERNEATH A CHAIN. The EARLIEST feature is edited while the requirements are made
/// of the body at the end of the timeline: that is how it is checked that names survive a rebuild of
/// the whole chain rather than of their own operation alone.
#[test]
fn an_extrude_height_edit_under_a_chain_keeps_every_name() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 26.0, 16.0, 10.0);
    let open: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    let sh = p.add_shell_mode(body, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let arr = p.add_linear_array_grid3(sh, 30.0, 0.0, 0.0, 2, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.0, 1);
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_faces.get(&arr).map(|f| f.len() >= 11).unwrap_or(false), "the chain did not build: {:?}", p.regen_errors);
    demand_after_param_edit("extrude height under a chain", &mut p, body, "height", "13");
}

/// AN EDIT THAT CHANGES THE TOPOLOGY, AND A RETURN TO WHERE IT STARTED.
///
/// Hole depth is not merely a number: at 6 mm the hole is BLIND (it has a floor, 8 faces), at 20 mm
/// it goes THROUGH (no floor, 7 faces). Demanding "every former name is in place" is impossible here
/// — the floor disappears legitimately. The requirement is a different and stronger one: put the
/// parameter back and THE SAME names come back, down to the last one. If a name depended on
/// traversal order, the circle would not close.
#[test]
fn a_topology_changing_param_returns_the_same_names() {
    let mut p = Project::default();
    p.new_document();
    let base = box_body(&mut p, 30.0, 20.0, 12.0);
    let hsi = p.new_sketch("drill mark");
    let hsk = p.sketches[hsi].id;
    p.add_sketch_node(hsk, "drill mark");
    p.sketch_point_at(hsi, 15.0, 10.0, 1e-6);
    // `flip=true` drills INTO the body: with `false` the operation leaves only an imprint of the circle (measured).
    let h = p.add_hole_from_sketch(base, hsk, 5.0, 6.0, 0, 0.0, 0.0, true);
    qymcad_testkit::regenerate(&mut p);
    let blind = live_names(&p);
    let blind_faces = p.regen_faces.get(&h).map(|f| f.len()).unwrap_or(0);
    assert_eq!(blind_faces, 8, "a blind hole must have a floor: {blind_faces} faces");

    let node = node_of(&p, h);
    p.set_feat_dim(node, "depth", "20".into());
    let (r1, _) = qymcad_testkit::regenerate(&mut p);
    assert!(r1.errors.is_empty(), "the through hole broke nodes: {:?}", r1.errors);
    let through_faces = p.regen_faces.get(&h).map(|f| f.len()).unwrap_or(0);
    assert_eq!(through_faces, 7, "a through hole must lose its floor: {through_faces} faces");
    for (b, ids) in live_names(&p) {
        let un = ids.iter().filter(|x| !qymcad_core::names::NameTable::is_named(**x)).count();
        assert_eq!(un, 0, "body {b}: {un} unnamed faces after the topology change");
    }

    p.set_feat_dim(node, "depth", "6".into());
    let (r2, _) = qymcad_testkit::regenerate(&mut p);
    assert!(r2.errors.is_empty(), "restoring the depth broke nodes: {:?}", r2.errors);
    assert_eq!(
        live_names(&p),
        blind,
        "the depth was restored, so THE SAME names must come back; a divergence means a name depends on something other than the recipe"
    );
}
