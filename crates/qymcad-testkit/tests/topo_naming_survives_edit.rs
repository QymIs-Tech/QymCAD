//! MEASUREMENT: do face and edge references survive an edit of a sketch higher up the timeline?
//!
//! Persistent ids are carried through the OCCT operation history (Modified/Generated) — that part
//! exists. But a BASE feature reseeds ids on every rebuild, in topology traversal order: if a sketch
//! edit changes the order or the number of faces, references further down the timeline slide onto
//! the wrong geometry. This is where that is checked in practice.
use qymcad_core::model::Project;

fn base_with_fillet(w: f64) -> (Project, u64, u64) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("base", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    p.add_rect_entity(si, 0.0, 0.0, w, 20.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let body = p.add_extrude_multi(sid, Vec::new(), 10.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let (_r, _s) = qymcad_testkit::regenerate(&mut p);
    (p, sid, body)
}

#[test]
fn face_ids_are_stable_across_a_sketch_edit() {
    let (mut p, sid, body) = base_with_fillet(30.0);
    let before: Vec<(u32, [f64; 3])> = p
        .regen_faces
        .get(&body)
        .map(|fs| fs.iter().map(|f| (f.id, f.normal)).collect())
        .unwrap_or_default();
    assert!(!before.is_empty(), "the base body has faces");
    eprintln!("before the edit: {} faces, ids {:?}", before.len(), before.iter().map(|(i, _)| *i).collect::<Vec<_>>());

    // the sketch edit: stretch the rectangle in width (same topology — 4 walls plus 2 caps)
    let si = p.sketch_index(sid).unwrap();
    for q in p.sketches[si].points.iter_mut() {
        if q.x > 15.0 {
            q.x = 45.0;
        }
    }
    p.regen_sketch(si);
    p.mark_sketch_dirty(sid);
    let (_r, _s) = qymcad_testkit::regenerate(&mut p);

    let after: Vec<(u32, [f64; 3])> = p.regen_faces.get(&body).map(|fs| fs.iter().map(|f| (f.id, f.normal)).collect()).unwrap_or_default();
    eprintln!("after the edit: {} faces, ids {:?}", after.len(), after.iter().map(|(i, _)| *i).collect::<Vec<_>>());

    // EVERY face must keep its id: same normal -> same id
    for (id, n) in &before {
        let same_dir: Vec<u32> = after.iter().filter(|(_, m)| (m[0] * n[0] + m[1] * n[1] + m[2] * n[2]) > 0.99).map(|(i, _)| *i).collect();
        assert!(same_dir.contains(id), "the face with normal {n:?} was id={id}, is now {same_dir:?} — the reference slid");
    }
}

/// THE MAIN CASE: the edit CHANGES THE TOPOLOGY of the profile (a notch appears in a wall, as in a
/// real part). Faces that stayed themselves must keep their ids: otherwise references further down
/// the timeline — a chamfer, a sketch on a face, a hole — slide onto a neighbouring face and the
/// part silently breaks.
#[test]
fn face_ids_survive_a_topology_change_in_the_sketch() {
    let (mut p, sid, body) = base_with_fillet(30.0);
    let si = p.sketch_index(sid).unwrap();
    let by_normal = |p: &Project| -> Vec<([f64; 3], u32)> {
        p.regen_faces.get(&body).map(|fs| fs.iter().map(|f| (f.normal, f.id)).collect()).unwrap_or_default()
    };
    let before = by_normal(&p);
    let n_before = before.len();
    eprintln!("before: {:?}", before.iter().map(|(n, i)| (format!("{:.0},{:.0},{:.0}", n[0], n[1], n[2]), *i)).collect::<Vec<_>>());

    // THE PROFILE BECOMES L-SHAPED: drop the rectangle, draw six segments. There will be 6 walls, so
    // the topology really is different — exactly the case where ordinal ids are expected to slide.
    let ents: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    p.delete_entities(si, &ents);
    let pts = [(0.0, 0.0), (30.0, 0.0), (30.0, 10.0), (12.0, 10.0), (12.0, 20.0), (0.0, 20.0)];
    for k in 0..pts.len() {
        let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
        p.add_line_entity(si, a.0, a.1, b.0, b.1, qymcad_core::feature::Purpose::Real);
    }
    p.regen_sketch(si);
    p.mark_sketch_dirty(sid);
    let (_r, _s) = qymcad_testkit::regenerate(&mut p);

    let after = by_normal(&p);
    eprintln!("after: {} faces (was {n_before}): {:?}", after.len(), after.iter().map(|(n, i)| (format!("{:.0},{:.0},{:.0}", n[0], n[1], n[2]), *i)).collect::<Vec<_>>());
    assert!(after.len() > n_before, "the profile topology really changed (there are more faces)");

    // THE CAPS (normals +-Z) are the most stable faces: they must keep their ids under any profile edit
    for (n, id) in before.iter().filter(|(n, _)| n[2].abs() > 0.99) {
        let now: Vec<u32> = after.iter().filter(|(m, _)| (m[2] - n[2]).abs() < 1e-6).map(|(_, i)| *i).collect();
        assert!(now.contains(id), "the cap with normal {n:?} was id={id}, is now {now:?} — the reference slid");
    }
}

/// THE NEXT LAYER: a wall name follows the SKETCH ENTITY, not the ordinal number of an edge.
///
/// Otherwise inserting an edge IN THE MIDDLE of a contour shifts the names of every wall after it —
/// which means a chamfer or a hole on a distant wall silently moves to its neighbour. The profile is
/// edited exactly that way here: one side of the rectangle is replaced by two segments with a step.
#[test]
fn wall_names_follow_sketch_entities_not_edge_order() {
    let (mut p, sid, body) = base_with_fillet(30.0);
    let si = p.sketch_index(sid).unwrap();
    let wall = |p: &Project, nx: f64, ny: f64| -> Option<u32> {
        p.regen_faces
            .get(&body)?
            .iter()
            .find(|f| (f.normal[0] - nx).abs() < 1e-6 && (f.normal[1] - ny).abs() < 1e-6)
            .map(|f| f.id)
    };
    let right_before = wall(&p, 1.0, 0.0).expect("right wall");
    let left_before = wall(&p, -1.0, 0.0).expect("left wall");
    eprintln!("before: right={right_before}, left={left_before}");

    // INSERTING IN THE MIDDLE: the bottom side (0,0)->(30,0) is replaced by a step of three segments
    let bottom = p.sketches[si]
        .entities
        .iter()
        .find(|e| match e.kind {
            qymcad_core::model::EntityKind::Line { a, b } => {
                let (pa, pb) = (p.sketches[si].points.iter().find(|q| q.id == a), p.sketches[si].points.iter().find(|q| q.id == b));
                matches!((pa, pb), (Some(x), Some(y)) if x.y.abs() < 1e-9 && y.y.abs() < 1e-9)
            }
            _ => false,
        })
        .map(|e| e.id)
        .expect("bottom side");
    p.delete_entities(si, &[bottom]);
    p.add_line_entity(si, 0.0, 0.0, 12.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 12.0, 0.0, 12.0, -4.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 12.0, -4.0, 30.0, -4.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 30.0, -4.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    p.mark_sketch_dirty(sid);
    let (_r, _s) = qymcad_testkit::regenerate(&mut p);

    let right_after = wall(&p, 1.0, 0.0).expect("right wall still there");
    let left_after = wall(&p, -1.0, 0.0).expect("left wall still there");
    eprintln!("after: right={right_after}, left={left_after}");
    assert_eq!(left_before, left_after, "the LEFT wall did not change — its name must stay the same");
    assert_eq!(right_before, right_after, "the RIGHT wall did not change — its name must stay the same");
}

/// A HOLE FROM A CIRCLE is the most common target of references (an edge fillet, a sketch on the hole
/// wall). A circular contour is assembled by a DIFFERENT path than loops of segments, and there the
/// edge origin was not set at first: the cylindrical wall was named positionally and slid whenever a
/// neighbour was edited.
#[test]
fn a_hole_wall_keeps_its_name_when_the_sketch_gains_entities() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("base", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 20.0, 15.0, 5.0, qymcad_core::feature::Purpose::Real); // a hole in the plate
    p.regen_sketch(si);
    let body = p.add_extrude_multi(sid, Vec::new(), 10.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let (_r, _s) = qymcad_testkit::regenerate(&mut p);
    // the hole wall is looked up BY NAME: the name is derived from the recipe (1000 + sketch entity
    // id), so it can be predicted in advance — that is the whole point of topological names.
    let circle_eid = p.sketches[si]
        .entities
        .iter()
        .find(|e| matches!(e.kind, qymcad_core::model::EntityKind::Circle { .. }))
        .map(|e| e.id)
        .expect("circle in the sketch");
    // the name is PREDICTABLE from the recipe: "wall of feature `body` from entity `circle_eid`"
    let expect = p.names.intern_face(qymcad_core::names::GeoName::new(body, qymcad_core::names::Role::Wall, circle_eid));
    let has = |p: &Project, id: u32| p.regen_faces.get(&body).is_some_and(|fs| fs.iter().any(|f| f.id == id));
    assert!(has(&p, expect), "the hole wall is named after the sketch entity (expected {expect}): {:?}", p.regen_faces.get(&body).map(|fs| fs.iter().map(|f| f.id).collect::<Vec<_>>()));

    // the edit: ANOTHER hole is added to the sketch — the name of the first must stay the same
    p.add_circle_entity(si, 8.0, 8.0, 3.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    p.mark_sketch_dirty(sid);
    let (_r, _s) = qymcad_testkit::regenerate(&mut p);
    assert!(has(&p, expect), "after adding the second hole the wall of the first changed its name: {:?}", p.regen_faces.get(&body).map(|fs| fs.iter().map(|f| f.id).collect::<Vec<_>>()));
}

/// A NAME IS A STRUCTURE, NOT A NUMBER: the descriptor says WHO made the face, in which role, and
/// from what.
#[test]
fn a_face_name_says_who_made_it_and_from_what() {
    let (p, sid, body) = base_with_fillet(30.0);
    let si = p.sketch_index(sid).unwrap();
    let faces = p.regen_faces.get(&body).expect("faces of the body");
    // the caps: the role is read straight off the name
    let roles: Vec<qymcad_core::names::Role> = faces.iter().filter_map(|f| p.names.get(f.id)).map(|n| n.role).collect();
    assert!(roles.contains(&qymcad_core::names::Role::CapStart), "the bottom is named by its role: {roles:?}");
    assert!(roles.contains(&qymcad_core::names::Role::CapEnd), "the top is named by its role: {roles:?}");
    // the walls: each remembers ITS OWN sketch line
    let ents: std::collections::HashSet<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    let walls: Vec<_> = faces.iter().filter_map(|f| p.names.get(f.id)).filter(|n| n.role == qymcad_core::names::Role::Wall).collect();
    assert_eq!(walls.len(), 4, "four walls of the rectangle: {walls:?}");
    for w in &walls {
        assert_eq!(w.feature, body, "the wall knows its feature");
        assert!(ents.contains(&w.src), "the wall knows its sketch line: {w:?}");
    }
    // and all of it is human-readable — for hints and diagnostics
    let any_wall = faces.iter().find(|f| p.names.get(f.id).is_some_and(|n| n.role == qymcad_core::names::Role::Wall)).unwrap();
    assert!(p.names.describe(any_wall.id).contains("wall"), "{}", p.names.describe(any_wall.id));
}

/// TWO FEATURES NEVER SHARE A NAME. The caps of EVERY body used to be called 1 and 2, so a boolean
/// had to renumber the tool, otherwise its cap merged with the cap of the base.
#[test]
fn two_features_never_share_a_name() {
    let mut p = Project::default();
    p.new_document();
    let mk = |p: &mut Project, name: &str, x: f64| -> u64 {
        let sid = p.add_sketch(name, vec![], None);
        p.add_sketch_node(sid, name);
        let si = p.sketch_index(sid).unwrap();
        p.add_rect_entity(si, x, 0.0, x + 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        p.add_extrude_multi(sid, Vec::new(), 10.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new())
    };
    let a = mk(&mut p, "A", 0.0);
    let b = mk(&mut p, "B", 50.0);
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let (_r, _s) = qymcad_testkit::regenerate(&mut p);
    let ids = |body: u64| -> std::collections::HashSet<u32> { p.regen_faces.get(&body).map(|fs| fs.iter().map(|f| f.id).collect()).unwrap_or_default() };
    let (fa, fb) = (ids(a), ids(b));
    assert!(!fa.is_empty() && !fb.is_empty(), "both bodies were built");
    assert!(fa.is_disjoint(&fb), "face names of two features do not overlap: {:?} vs {:?}", fa, fb);
}

/// A BOOLEAN MERGES TWO NAME SPACES. The pocket wall comes FROM THE TOOL and must keep the name from
/// the tool's recipe: editing the tool profile must not drag a reference off it.
#[test]
fn a_boolean_keeps_the_tool_names_instead_of_renumbering_them() {
    let mut p = Project::default();
    p.new_document();
    // the base: a 40x30 plate
    let base_sid = p.add_sketch("base", vec![], None);
    p.add_sketch_node(base_sid, "Sketch");
    let bsi = p.sketch_index(base_sid).unwrap();
    p.add_rect_entity(bsi, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(bsi);
    let base = p.add_extrude_multi(base_sid, Vec::new(), 10.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());
    let base_body = p.finish_base_body(base, 1);
    // the tool: a round pocket
    let tool_sid = p.add_sketch("tool", vec![], None);
    p.add_sketch_node(tool_sid, "Sketch 2");
    let tsi = p.sketch_index(tool_sid).unwrap();
    p.add_circle_entity(tsi, 20.0, 15.0, 5.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(tsi);
    let circle_eid = p.sketches[tsi].entities.iter().find(|e| matches!(e.kind, qymcad_core::model::EntityKind::Circle { .. })).map(|e| e.id).expect("circle");
    // the tool is a SEPARATE body (finish_base_body would glue it to the base at once), then it cuts
    let tool = p.add_extrude_multi(tool_sid, Vec::new(), 20.0, qymcad_core::feature::Reach::Forward, 5.0, Vec::new());
    let cut = p.add_body_boolean(base_body, tool, 0);
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let (_r, _s) = qymcad_testkit::regenerate(&mut p);

    let faces = p.regen_faces.get(&cut).expect("the cut body was built");
    let wall = p.names.intern_face(qymcad_core::names::GeoName::new(tool, qymcad_core::names::Role::Wall, circle_eid));
    assert!(
        faces.iter().any(|f| f.id == wall),
        "the pocket wall kept the name from the TOOL recipe ({wall}) instead of getting an ordinal number: {:?}",
        faces.iter().map(|f| f.id).collect::<Vec<_>>()
    );
    // and the base names are in place — both spaces coexist
    let base_cap = p.names.intern_face(qymcad_core::names::GeoName::new(base, qymcad_core::names::Role::CapStart, 0));
    assert!(faces.iter().any(|f| f.id == base_cap), "the base bottom kept its name too");
}

/// A REVOLVE IS NAMED FROM THE RECIPE TOO: a surface of revolution remembers the profile edge that
/// bore it. Every operation except extrude used to seed names by traversal order, which meant a
/// reference to a face of a revolved body lived exactly until the first profile edit.
#[test]
fn a_revolve_names_its_faces_from_the_recipe_too() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("base", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    p.add_rect_entity(si, 5.0, 0.0, 15.0, 8.0, qymcad_core::feature::Purpose::Real); // a rectangle set off from the X axis
    p.regen_sketch(si);
    let body = p.add_revolve(sid, 0, 270.0); // a partial turn -> there are end caps as well
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let (r, _s) = qymcad_testkit::regenerate(&mut p);
    assert!(r.errors.is_empty(), "the revolve was built: {:?}", r.errors);
    let faces = p.regen_faces.get(&body).expect("faces of the revolved body");
    let named: Vec<_> = faces.iter().filter_map(|f| p.names.get(f.id)).collect();
    assert!(!named.is_empty(), "the revolved body has structured names: {:?}", faces.iter().map(|f| f.id).collect::<Vec<_>>());
    let ents: std::collections::HashSet<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    let lateral: Vec<_> = named.iter().filter(|n| n.role == qymcad_core::names::Role::Revolved).collect();
    assert!(!lateral.is_empty(), "surfaces of revolution are named by their role: {named:?}");
    for l in &lateral {
        assert_eq!(l.feature, body, "the surface knows its feature");
        assert!(ents.contains(&l.src), "and its profile edge: {l:?}");
    }
}

/// A PRIMITIVE IS NAMED FROM ITS OWN GEOMETRY: caps and side are roles derived from the recipe
/// itself. The topology of a primitive does not change with its parameters, so positional numbers
/// did not slide here — but they were not unique either, and a primitive used as a boolean tool lost
/// its names entirely.
#[test]
fn a_primitive_gets_named_roles_not_traversal_numbers() {
    let mut p = Project::default();
    p.new_document();
    let body = p.add_cylinder(5.0, 20.0);
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let (r, _s) = qymcad_testkit::regenerate(&mut p);
    assert!(r.errors.is_empty(), "the cylinder was built: {:?}", r.errors);
    let faces = p.regen_faces.get(&body).expect("faces of the cylinder");
    let roles: Vec<_> = faces.iter().filter_map(|f| p.names.get(f.id)).map(|n| n.role).collect();
    use qymcad_core::names::Role;
    assert!(roles.contains(&Role::CapStart) && roles.contains(&Role::CapEnd), "the caps are named by roles: {roles:?}");
    assert!(roles.contains(&Role::Side), "the side surface is named by its role: {roles:?}");
    assert!(faces.iter().all(|f| p.names.get(f.id).is_some_and(|n| n.feature == body)), "every face knows its feature");
}

/// AN EDGE IS NAMED BY THE PAIR OF ITS FACES. Checked on a NEW part (it holds no references from the
/// old scheme, so the edge naming scheme is active): every edge of the body is named, and the name
/// says which faces it belongs to.
#[test]
fn an_edge_is_named_by_the_two_faces_that_meet_there() {
    let (p, _sid, body) = base_with_fillet(30.0);
    let edges = p.regen_edges.get(&body).expect("edges of the body");
    assert!(!edges.is_empty(), "there are edges");
    let named: Vec<_> = edges.iter().filter_map(|e| p.names.edge(e.id)).collect();
    assert_eq!(named.len(), edges.len(), "EVERY edge is named: {} of {}", named.len(), edges.len());
    let faces: std::collections::HashSet<u32> = p.regen_faces.get(&body).unwrap().iter().map(|f| f.id).collect();
    for e in &named {
        assert!(faces.contains(&e.faces[0]) && faces.contains(&e.faces[1]), "the edge name refers to REAL faces of the body: {e:?}");
        assert_ne!(e.faces[0], e.faces[1], "two DIFFERENT faces");
    }
    // and the name is human-readable
    let any = edges.iter().find(|e| p.names.edge(e.id).is_some()).unwrap();
    assert!(p.names.describe(any.id).starts_with("edge["), "{}", p.names.describe(any.id));
}

/// AN EDGE NAME SURVIVES A SKETCH EDIT: stretch the rectangle and the edges stay themselves.
#[test]
fn edge_names_survive_a_sketch_edit() {
    let (mut p, sid, body) = base_with_fillet(30.0);
    let before: std::collections::HashSet<u32> = p.regen_edges.get(&body).unwrap().iter().map(|e| e.id).collect();
    let si = p.sketch_index(sid).unwrap();
    for q in p.sketches[si].points.iter_mut() {
        if q.x > 15.0 {
            q.x = 45.0;
        }
    }
    p.regen_sketch(si);
    p.mark_sketch_dirty(sid);
    let (_r, _s) = qymcad_testkit::regenerate(&mut p);
    let after: std::collections::HashSet<u32> = p.regen_edges.get(&body).unwrap().iter().map(|e| e.id).collect();
    assert_eq!(before, after, "edge names did not slide from stretching the profile");
}
