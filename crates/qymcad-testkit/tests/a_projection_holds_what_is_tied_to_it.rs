//! A PROJECTION HOLDS WHAT IS TIED TO IT through a rebuild of the body it came from: the body changes size, and the
//! geometry tied to the projection by dimensions follows it; the outline of the face changes its structure, and the
//! curves of the edges still there keep their ids, so the dimensions and the ties on them stay.
//!
//! Reported behaviour: "if other geometry is tied to the projection by dimensions or constraints, the projection may
//! fall apart when the model it came from is rebuilt".
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, EntityKind, FilletBy, FilletSize, Id, ProjSource, Project};
use qymcad_kernel::OcctKernel;
use std::collections::HashMap;

/// A live kernel with its cache of shapes: a projection asks the kernel for the edges of the body.
struct Live {
    p: Project,
    shapes: HashMap<Id, qymcad_kernel::Shape>,
}

impl Live {
    fn rebuild(&mut self) {
        let (_, sh) = qymcad_testkit::regenerate_with_shapes(&mut self.p, std::mem::take(&mut self.shapes));
        self.shapes = sh;
    }

    fn project_into(&mut self, si: usize, body: Id, src: ProjSource) -> Id {
        let k = OcctKernel { shapes: std::cell::RefCell::new(std::mem::take(&mut self.shapes)), ..Default::default() };
        let id = self.p.add_sketch_projection(si, body, src, &k);
        self.shapes = k.shapes.into_inner();
        id
    }
}

fn top_face_id(p: &Project, body: u64) -> u32 {
    p.regen_faces.get(&body).and_then(|fs| fs.iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z))).map(|f| f.id).expect("a top face")
}

fn at(p: &Project, si: usize, id: Id) -> (f64, f64) {
    p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("the point")
}

/// A point of the sketch of its own at `x`, `y`.
fn a_point(p: &mut Project, si: usize, x: f64, y: f64) -> Id {
    p.sketch_point_at(si, x, y, 1e-9)
}

fn distance(a: Id, b: Id, d: f64, axis: u8) -> Constraint {
    Constraint::Distance { a, b, d, off: 3.0, expr: String::new(), driven: false, axis, at: None }
}

#[test]
fn geometry_tied_to_a_projection_follows_the_body() {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let part = p.add_part("part");
    p.set_active_component(Some(part));
    let body = p.add_box(20.0, 20.0, 20.0);
    let si = p.new_sketch("Sketch");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Sketch");
    let mut live = Live { p, shapes: HashMap::new() };
    live.rebuild();
    let face = top_face_id(&live.p, body);
    live.project_into(si, body, ProjSource::Face(face));
    // the corner of the outline farthest along X and Y, and a point of the sketch 10 right and 5 up of it, held there
    let corner = *live.p.sketches[si].projections[0]
        .points
        .iter()
        .max_by(|a, b| {
            let (pa, pb) = (at(&live.p, si, **a), at(&live.p, si, **b));
            (pa.0 + pa.1).total_cmp(&(pb.0 + pb.1))
        })
        .expect("a corner");
    let c = at(&live.p, si, corner);
    let free = a_point(&mut live.p, si, c.0 + 10.0, c.1 + 5.0);
    live.p.sketches[si].constraints.push(distance(corner, free, 10.0, 1));
    live.p.sketches[si].constraints.push(distance(corner, free, 5.0, 2));
    live.p.solve_sketch(si);

    // the box made 30 along X above the sketch
    if let Some(n) = live.p.timeline.iter_mut().find(|n| n.kind.bodies().contains(&body)) {
        if let qymcad_core::feature::FeatureKind::Box3 { dx, .. } = &mut n.kind {
            *dx = 30.0;
        }
        n.dirty = true;
    }
    live.rebuild();

    let (c, f) = (at(&live.p, si, corner), at(&live.p, si, free));
    assert!((f.0 - c.0 - 10.0).abs() < 1e-6 && (f.1 - c.1 - 5.0).abs() < 1e-6, "the point held 10 and 5 off the corner of the projection stands at {f:?}, the corner at {c:?}");
}

#[test]
fn a_projection_keeps_the_ties_on_its_curves_when_its_face_changes_structure() {
    // a plate 60 x 40 extruded 14 from a sketch of four lines, its top face projected into a second sketch
    let mut p = Project::default();
    p.new_document();
    let sid1 = p.add_line_sketch("Sketch 1", vec![Point2::new(0.0, 0.0), Point2::new(60.0, 0.0), Point2::new(60.0, 40.0), Point2::new(0.0, 40.0)], true);
    let si1 = p.sketch_index(sid1).expect("the sketch");
    p.regen_sketch(si1);
    if let Some(o) = p.sketch_owner(sid1) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid1, "Sketch 1");
    let closed: Vec<u64> = p.sketches[si1].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_extrude_multi(sid1, closed, 14.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let si = p.new_sketch("Sketch 2");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Sketch 2");
    let mut live = Live { p, shapes: HashMap::new() };
    live.rebuild();
    let face = top_face_id(&live.p, body);
    live.project_into(si, body, ProjSource::Face(face));
    let before = live.p.sketches[si].projections[0].entities.len();
    assert_eq!(before, 4, "GUARD: the outline of the plate is four sides");

    // the bottom side of the outline, a point on it held 15 from its start
    let bottom = live.p.sketches[si].projections[0]
        .entities
        .iter()
        .copied()
        .find(|e| match live.p.sketches[si].entities.iter().find(|x| x.id == *e).map(|x| x.kind) {
            Some(EntityKind::Line { a, b }) => at(&live.p, si, a).1.abs() < 1e-6 && at(&live.p, si, b).1.abs() < 1e-6,
            _ => false,
        })
        .expect("the bottom side");
    let Some(EntityKind::Line { a: ba, b: bb }) = live.p.sketches[si].entities.iter().find(|x| x.id == bottom).map(|x| x.kind) else { panic!("the bottom side is a line") };
    let start = if at(&live.p, si, ba).0 < at(&live.p, si, bb).0 { ba } else { bb };
    let free = a_point(&mut live.p, si, 15.0, 0.0);
    live.p.sketches[si].constraints.push(Constraint::PointOnLine { p: free, a: ba, b: bb });
    live.p.sketches[si].constraints.push(distance(start, free, 15.0, 1));
    live.p.solve_sketch(si);
    let ties = live.p.sketches[si].constraints.len();

    // the far corner (60, 40) of the first sketch rounded: the face has five edges now, its bottom side as it was
    let corner = live.p.sketches[si1].points.iter().find(|q| (q.x - 60.0).abs() < 1e-9 && (q.y - 40.0).abs() < 1e-9).map(|q| q.id).expect("the far corner");
    assert!(live.p.fillet_at_vertex_by(si1, corner, FilletSize { by: FilletBy::Radius, value: 5.0 }), "GUARD: the far corner rounded");
    if let Some(n) = live.p.timeline.iter_mut().find(|n| n.kind.bodies().contains(&body)) {
        n.dirty = true;
    }
    live.rebuild();

    let s = &live.p.sketches[si];
    let mut failures = Vec::new();
    if s.projections.len() != 1 || s.projections[0].entities.len() <= before {
        failures.push(format!("the projection did not take the new outline: {:?}", s.projections.iter().map(|x| x.entities.len()).collect::<Vec<_>>()));
    }
    if !s.projections.first().is_some_and(|x| x.entities.contains(&bottom)) {
        failures.push("the bottom side is no longer the same curve of the projection".into());
    }
    if s.constraints.len() != ties {
        failures.push(format!("the ties on the projection went: {} of {ties} constraints left", s.constraints.len()));
    }
    let f = at(&live.p, si, free);
    if (f.0 - 15.0).abs() > 1e-6 || f.1.abs() > 1e-6 {
        failures.push(format!("the point held on the bottom side 15 from its start stands at {f:?}"));
    }
    let (_, redundant) = live.p.sketch_dof(si);
    if redundant != 0 || !live.p.sketch_conflicts(si).is_empty() {
        failures.push(format!("the sketch does not solve: {redundant} redundant, conflicts {:?}", live.p.sketch_conflicts(si)));
    }
    assert!(failures.is_empty(), "the projection through a change of its face:\n{}", failures.join("\n"));
}

#[test]
fn a_fillet_put_above_the_sketch_parts_the_corner_and_the_ties_keep_their_sides() {
    // a plate 60 x 40 extruded 14, its top face projected into a second sketch
    let mut p = Project::default();
    p.new_document();
    let sid1 = p.add_line_sketch("Sketch 1", vec![Point2::new(0.0, 0.0), Point2::new(60.0, 0.0), Point2::new(60.0, 40.0), Point2::new(0.0, 40.0)], true);
    let si1 = p.sketch_index(sid1).expect("the sketch");
    p.regen_sketch(si1);
    if let Some(o) = p.sketch_owner(sid1) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid1, "Sketch 1");
    let closed: Vec<u64> = p.sketches[si1].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_extrude_multi(sid1, closed, 14.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let si = p.new_sketch("Sketch 2");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Sketch 2");
    let mut live = Live { p, shapes: HashMap::new() };
    live.rebuild();
    let face = top_face_id(&live.p, body);
    live.project_into(si, body, ProjSource::Face(face));

    // the corner (60, 0) and the right side of the outline
    let pts = live.p.sketches[si].projections[0].points.clone();
    let corner = *pts.iter().find(|q| at(&live.p, si, **q) == (60.0, 0.0)).expect("the corner (60, 0)");
    let top = *pts.iter().find(|q| at(&live.p, si, **q) == (60.0, 40.0)).expect("the corner (60, 40)");
    // a line of the sketch held 5 off the corner, on its left (the distance to a line is signed by the side), and a
    // point held on the right side
    let (la, lb) = (a_point(&mut live.p, si, 55.0, 10.0), a_point(&mut live.p, si, 55.0, 30.0));
    let id = live.p.alloc_id();
    live.p.sketches[si].entities.push(qymcad_core::model::SketchEntity { id, kind: EntityKind::Line { a: la, b: lb }, construction: false });
    let on = a_point(&mut live.p, si, 60.0, 20.0);
    live.p.sketches[si].constraints.push(Constraint::Vertical { a: la, b: lb });
    live.p.sketches[si].constraints.push(Constraint::DistancePL { p: corner, a: la, b: lb, d: -5.0, off: 3.0, expr: String::new(), driven: false, at: None });
    live.p.sketches[si].constraints.push(Constraint::PointOnLine { p: on, a: corner, b: top });
    live.p.solve_sketch(si);
    assert!((at(&live.p, si, la).0 - 55.0).abs() < 1e-6, "GUARD: the line held 5 off the corner at X 55, it stands at {:?}", at(&live.p, si, la));

    // the upright edge at (60, 0) rounded 3, the fillet put above the sketch
    let edge = live.p.regen_edges[&body]
        .iter()
        .find(|e| (e.a[0] - 60.0).abs() < 1e-6 && e.a[1].abs() < 1e-6 && (e.b[0] - 60.0).abs() < 1e-6 && e.b[1].abs() < 1e-6)
        .map(|e| e.id)
        .expect("the upright edge at (60, 0)");
    live.p.add_fillet(body, 3.0, vec![edge]);
    let (from, to) =
        (live.p.timeline.len() - 1, live.p.timeline.iter().position(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Sketch { sketch } if sketch == sid)).expect("the node of sketch 2"));
    assert!(live.p.reorder_feature(from, to), "GUARD: the fillet put above the sketch");
    live.rebuild();

    let s = &live.p.sketches[si];
    let mut failures = Vec::new();
    let arcs = s.entities.iter().filter(|e| s.projections[0].entities.contains(&e.id) && matches!(e.kind, EntityKind::Arc { .. })).count();
    if arcs != 1 {
        failures.push(format!("the rounded corner is {arcs} arcs of the projection, not one"));
    }
    let (a, o) = (at(&live.p, si, la), at(&live.p, si, on));
    if (a.0 - 55.0).abs() > 1e-6 {
        failures.push(format!("the line held 5 off the corner stands at X {:.4}, not 55", a.0));
    }
    if (o.0 - 60.0).abs() > 1e-6 {
        failures.push(format!("the point held on the right side stands at {o:?}, off the side at X 60"));
    }
    let (_, redundant) = live.p.sketch_dof(si);
    if redundant != 0 {
        failures.push(format!("the sketch has {redundant} redundant"));
    }
    assert!(failures.is_empty(), "a fillet above the sketch:\n{}", failures.join("\n"));
}
