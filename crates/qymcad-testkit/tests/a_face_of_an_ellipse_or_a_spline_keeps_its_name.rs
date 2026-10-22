//! A FACE OF AN ELLIPSE OR A SPLINE KEEPS ITS NAME WHEN THE CURVE IS EDITED: an ellipse extruded, its half axis given
//! 24 instead of 20 by moving the end of the axis; a profile closed by a spline extruded, a middle node of the spline
//! moved up - the body changes, and every face and every edge it had stands under the name it had, none new and none
//! gone: the seam of the wall of the ellipse as the edges between the spans of the spline. A fillet or a sketch standing
//! on a face of the curve follows it by that name.
use qymcad_core::feature::{Purpose, Reach};
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project};

/// The closed profiles of sketch `si`, extruded 10 forward: the body.
fn extruded(p: &mut Project, si: usize) -> Id {
    let sid = p.sketches[si].id;
    p.regen_sketch(si);
    let c: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_extrude_multi(sid, c, 10.0, Reach::Forward, 0.0, vec![]);
    let (report, _) = qymcad_testkit::regenerate(p);
    assert!(report.errors.is_empty(), "setup: the extrusion did not build: {:?}", report.errors);
    body
}

/// A SCENE: the drawing, its sketch, the body, and the point of the curve the edit moves with where it goes.
struct Scene {
    p: Project,
    si: usize,
    body: Id,
    moved: Id,
    by: Point2,
}

/// An ellipse 20 by 10 about the origin, extruded; the end of its major axis moved 4 out.
fn ellipse() -> Scene {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("Ellipse");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Ellipse");
    p.add_ellipse_entity(si, Point2::new(0.0, 0.0), 20.0, 10.0, 0.0, Purpose::Real);
    let body = extruded(&mut p, si);
    let moved = p.sketches[si].entities.iter().find_map(|e| e.kind.as_ellipse()).map(|el| el.ma).expect("the ellipse");
    Scene { p, si, body, moved, by: Point2::new(4.0, 0.0) }
}

/// Three sides of a rectangle 40 x 20 closed by a spline bowed up through (20, 28), extruded; its middle node moved 3 up.
fn spline() -> Scene {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("Spline");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Spline");
    p.add_line_entity(si, 0.0, 20.0, 0.0, 0.0, Purpose::Real);
    p.add_line_entity(si, 0.0, 0.0, 40.0, 0.0, Purpose::Real);
    p.add_line_entity(si, 40.0, 0.0, 40.0, 20.0, Purpose::Real);
    let nodes = [[40.0, 20.0], [30.0, 26.0], [20.0, 28.0], [10.0, 26.0], [0.0, 20.0]].map(|q| Point2::new(q[0], q[1]));
    p.add_spline(si, nodes.to_vec(), qymcad_core::feature::Ends::Open, Purpose::Real);
    p.merge_close_points(si, 1e-9);
    let body = extruded(&mut p, si);
    let moved = p.sketches[si].entities.iter().find_map(|e| e.kind.as_spline()).map(|sp| sp.nodes[2]).expect("the spline");
    Scene { p, si, body, moved, by: Point2::new(0.0, 3.0) }
}

/// A CASE: what is edited, and how its scene is laid.
struct Case {
    name: &'static str,
    scene: fn() -> Scene,
}

#[test]
fn a_face_of_an_ellipse_or_a_spline_keeps_its_name_when_the_curve_is_edited() {
    let mut failures = Vec::new();
    for Case { name, scene } in [Case { name: "an ellipse", scene: ellipse }, Case { name: "a spline", scene: spline }] {
        let Scene { mut p, si, body, moved, by } = scene();
        let faces = |p: &Project| -> std::collections::BTreeSet<u32> { p.regen_faces.get(&body).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default() };
        let edges = |p: &Project| -> std::collections::BTreeSet<u32> { p.regen_edges.get(&body).map(|e| e.iter().map(|x| x.id).collect()).unwrap_or_default() };
        let before = faces(&p);
        let edges_before = edges(&p);
        let volume_before = qymcad_testkit::regenerate(&mut p).1.get(&body).map(|s| s.volume()).unwrap_or(0.0);
        if let Some(q) = p.sketches[si].points.iter_mut().find(|q| q.id == moved) {
            q.x += by.x;
            q.y += by.y;
        }
        p.solve_sketch(si);
        let sid = p.sketches[si].id;
        p.mark_sketch_dirty(sid);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        let volume = shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
        let after = faces(&p);
        if !report.errors.is_empty() || (volume - volume_before).abs() < 1.0 {
            failures.push(format!("{name}: the edit did not change the body (volume {volume_before:.1} -> {volume:.1}) or broke it: {:?}", report.errors));
        } else if before != after {
            let gone: Vec<String> = before.difference(&after).map(|d| p.names.describe(*d)).collect();
            let new: Vec<String> = after.difference(&before).map(|d| p.names.describe(*d)).collect();
            failures.push(format!("{name}: the faces did not keep their names - gone {gone:?}, new {new:?}"));
        }
        let edges_after = edges(&p);
        if edges_before.is_empty() {
            failures.push(format!("{name}: setup - the body has no edges"));
        } else if edges_before != edges_after {
            let gone: Vec<String> = edges_before.difference(&edges_after).map(|d| p.names.describe(*d)).collect();
            let new: Vec<String> = edges_after.difference(&edges_before).map(|d| p.names.describe(*d)).collect();
            failures.push(format!("{name}: the edges did not keep their names - gone {gone:?}, new {new:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
