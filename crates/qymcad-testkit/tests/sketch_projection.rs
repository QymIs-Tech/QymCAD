//! PROJECTING BODY GEOMETRY INTO A SKETCH — associative, not a one-off copy of points.
//!
//! Before this, body edges were only drawn as a backdrop in the sketcher: they could be snapped to,
//! but not taken as entities. What is checked here is exactly what separates a projection from a
//! copy-paste: it follows the part, it keeps its ids (so user constraints stay alive), and it does
//! not disappear silently when the source is gone.
use qymcad_kernel::OcctKernel;
use qymcad_core::model::{Id, ProjSource, Project};
use std::collections::HashMap;

/// A live kernel with a shape cache: `add_sketch_projection` asks the kernel for edge geometry, so
/// the tests must hold the same cache the application does, not an empty one.
struct Live {
    p: Project,
    shapes: HashMap<Id, qymcad_kernel::Shape>,
}

impl Live {
    /// A full rebuild that keeps the kernel cache.
    fn rebuild(&mut self) {
        let (_, sh) = qymcad_testkit::regenerate_with_shapes(&mut self.p, std::mem::take(&mut self.shapes));
        self.shapes = sh;
    }

    /// Project a source into a sketch with the same kernel that built the bodies.
    fn project_into(&mut self, si: usize, body: Id, src: ProjSource) -> Id {
        let k = OcctKernel { shapes: std::cell::RefCell::new(std::mem::take(&mut self.shapes)), ..Default::default() };
        let id = self.p.add_sketch_projection(si, body, src, &k);
        self.shapes = k.shapes.into_inner();
        id
    }
}

/// A part with a 20x20x20 box and an empty sketch on world XY.
fn part_and_sketch() -> (Live, u64, usize) {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let part = p.add_part("part");
    p.set_active_component(Some(part));
    let body = p.add_box(20.0, 20.0, 20.0);
    let si = p.new_sketch("Sketch");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Sketch"); // as in the application: the sketch has a timeline node (that is where projections resolve)
    let mut live = Live { p, shapes: HashMap::new() };
    live.rebuild();
    (live, body, si)
}

/// The persistent id of the TOP face of a body.
fn top_face_id(p: &Project, body: u64) -> u32 {
    p.regen_faces
        .get(&body)
        .and_then(|fs| fs.iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap()))
        .map(|f| f.id)
        .expect("there is a top face")
}

/// Driven entities of the projections in a sketch.
fn proj_entities(p: &Project, si: usize) -> Vec<u64> {
    p.sketches[si].projections.iter().flat_map(|x| x.entities.clone()).collect()
}

/// The outline of a box face projects as FOUR straight lines — a straight edge stays straight rather
/// than becoming a polyline.
#[test]
fn a_face_outline_projects_as_four_straight_lines() {
    let (mut live, body, si) = part_and_sketch();
    let face = top_face_id(&live.p, body);
    let pid = live.project_into(si, body, ProjSource::Face(face));
    assert_ne!(pid, 0, "the projection must be created");

    let ents = proj_entities(&live.p, si);
    assert_eq!(ents.len(), 4, "a square face has four edges, and it came out {}", ents.len());
    for e in &ents {
        let kind = live.p.sketches[si].entities.iter().find(|x| x.id == *e).map(|x| x.kind).expect("the entity is there");
        assert!(matches!(kind, qymcad_core::model::EntityKind::Line { .. }), "an edge of a box must land as a LINE, and it landed as {kind:?}");
    }
    // the geometry agrees: a 20x20 square in sketch coordinates
    let pts: Vec<[f64; 2]> = live.p.sketches[si].projections[0].points.iter().filter_map(|id| live.p.sketches[si].points.iter().find(|q| q.id == *id)).map(|q| [q.x, q.y]).collect();
    let (xs, ys): (Vec<f64>, Vec<f64>) = (pts.iter().map(|q| q[0]).collect(), pts.iter().map(|q| q[1]).collect());
    let w = xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min);
    let h = ys.iter().cloned().fold(f64::MIN, f64::max) - ys.iter().cloned().fold(f64::MAX, f64::min);
    assert!((w - 20.0).abs() < 1e-6 && (h - 20.0).abs() < 1e-6, "a 20x20 square, and it came out {w}x{h}");
}

/// THE PROJECTION FOLLOWS THE PART: change the size of the body and the outline in the sketch is
/// recomputed.
#[test]
fn the_projection_follows_the_body_it_came_from() {
    let (mut live, body, si) = part_and_sketch();
    let face = top_face_id(&live.p, body);
    live.project_into(si, body, ProjSource::Face(face));
    let before: Vec<[f64; 2]> = live.p.sketches[si].points.iter().map(|q| [q.x, q.y]).collect();

    // the box became 30 along X — the projection must notice that BY ITSELF, without a manual recompute
    if let Some(n) = live.p.timeline.iter_mut().find(|n| n.kind.bodies().contains(&body)) {
        if let qymcad_core::feature::FeatureKind::Box3 { dx, .. } = &mut n.kind {
            *dx = 30.0;
        }
        n.dirty = true;
    }
    live.rebuild();

    let pts: Vec<[f64; 2]> = live.p.sketches[si].projections[0].points.iter().filter_map(|id| live.p.sketches[si].points.iter().find(|q| q.id == *id)).map(|q| [q.x, q.y]).collect();
    let xs: Vec<f64> = pts.iter().map(|q| q[0]).collect();
    let w = xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min);
    assert!((w - 30.0).abs() < 1e-6, "the part became 30 mm — the projection must follow it, and its width is {w}");
    assert_ne!(before, live.p.sketches[si].points.iter().map(|q| [q.x, q.y]).collect::<Vec<_>>(), "the points must move");
}

/// THE IDS OF DRIVEN POINTS ARE STABLE across a rebuild — otherwise a user constraint on a corner of
/// the projection would fall off.
#[test]
fn projected_point_ids_survive_a_rebuild() {
    let (mut live, body, si) = part_and_sketch();
    let face = top_face_id(&live.p, body);
    live.project_into(si, body, ProjSource::Face(face));
    let ids_before = live.p.sketches[si].projections[0].points.clone();
    let ents_before = live.p.sketches[si].projections[0].entities.clone();

    if let Some(n) = live.p.timeline.iter_mut().find(|n| n.kind.bodies().contains(&body)) {
        if let qymcad_core::feature::FeatureKind::Box3 { dx, .. } = &mut n.kind {
            *dx = 26.0;
        }
        n.dirty = true;
    }
    live.rebuild();

    assert_eq!(live.p.sketches[si].projections[0].points, ids_before, "the structure is the same — the point ids must be kept");
    assert_eq!(live.p.sketches[si].projections[0].entities, ents_before, "the structure is the same — the entity ids must be kept");
}

/// DRIVEN GEOMETRY IS MARKED as such: it cannot be dragged by hand.
#[test]
fn projected_points_are_marked_immovable() {
    let (mut live, body, si) = part_and_sketch();
    let face = top_face_id(&live.p, body);
    live.project_into(si, body, ProjSource::Face(face));

    let driven = live.p.sketches[si].projected_points();
    assert_eq!(driven.len(), 4, "all four corners must count as driven");
    let immovable = live.p.sketches[si].immovable_points();
    assert!(driven.iter().all(|d| immovable.contains(d)), "driven points must land in \"cannot be moved\" alongside the system ones");
    // an ordinary point drawn by hand can move
    let free = live.p.add_line_entity(si, 100.0, 100.0, 150.0, 100.0, qymcad_core::feature::Purpose::Real);
    let _ = free;
    let user_pts: Vec<u64> = live.p.sketches[si].points.iter().filter(|q| q.x > 50.0).map(|q| q.id).collect();
    assert!(!user_pts.is_empty(), "setup: the hand-drawn points are there");
    assert!(user_pts.iter().all(|u| !immovable.contains(u)), "geometry drawn by hand can be moved");
}

/// A CIRCULAR EDGE PROJECTS AS A REAL CIRCLE, not as a 60-segment polyline from the tessellation.
///
/// Otherwise a diameter cannot be placed on the projection, and tracing along it gives a faceted
/// contour.
#[test]
fn a_circular_edge_projects_as_a_real_circle() {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let part = p.add_part("part");
    p.set_active_component(Some(part));
    let body = p.add_cylinder(8.0, 20.0);
    let si = p.new_sketch("Sketch");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Sketch"); // as in the application: the sketch has a timeline node (that is where projections resolve)
    let mut live = Live { p, shapes: HashMap::new() };
    live.rebuild();
    let face = top_face_id(&live.p, body);

    let pid = live.project_into(si, body, ProjSource::Face(face));
    assert_ne!(pid, 0, "projecting the cylinder cap must create a projection");
    let ents = proj_entities(&live.p, si);
    let circles: Vec<f64> = ents
        .iter()
        .filter_map(|e| live.p.sketches[si].entities.iter().find(|x| x.id == *e))
        .filter_map(|e| match e.kind {
            qymcad_core::model::EntityKind::Circle { r, .. } => Some(r),
            _ => None,
        })
        .collect();
    assert_eq!(circles.len(), 1, "the cap rim is ONE circle, and it came out {circles:?} ({} entities in total)", ents.len());
    assert!((circles[0] - 8.0).abs() < 1e-6, "the radius must be exact (8), and it came out {}", circles[0]);
}

/// THE SOURCE IS GONE — the projection is marked broken rather than silently disappearing or silently
/// staying "real".
#[test]
fn a_lost_source_marks_the_projection_broken() {
    let (mut live, body, si) = part_and_sketch();
    let face = top_face_id(&live.p, body);
    live.project_into(si, body, ProjSource::Face(face));
    assert!(!live.p.sketches[si].projections[0].lost, "setup: the projection is intact");
    let ents_before = live.p.sketches[si].projections[0].entities.len();

    // a reference to a face the body does not have
    live.p.sketches[si].projections[0].src = ProjSource::Face(999_999);
    live.rebuild();

    assert!(live.p.sketches[si].projections[0].lost, "a lost source must be flagged");
    assert_eq!(live.p.sketches[si].projections[0].entities.len(), ents_before, "the geometry stays: user constraints refer to it");
}

/// Picking the same source again does not breed a second copy on top of the first.
#[test]
fn projecting_the_same_source_twice_is_idempotent() {
    let (mut live, body, si) = part_and_sketch();
    let face = top_face_id(&live.p, body);
    let a = live.project_into(si, body, ProjSource::Face(face));
    let b = live.project_into(si, body, ProjSource::Face(face));
    assert_eq!(a, b, "the same source means the same projection");
    assert_eq!(live.p.sketches[si].projections.len(), 1, "there must be no second record");
    assert_eq!(proj_entities(&live.p, si).len(), 4, "and no second set of lines either");
}

/// Removing a projection takes its driven geometry with it — otherwise the sketch is left with debris.
#[test]
fn removing_a_projection_takes_its_geometry_with_it() {
    let (mut live, body, si) = part_and_sketch();
    let face = top_face_id(&live.p, body);
    let pid = live.project_into(si, body, ProjSource::Face(face));
    let pts_before = live.p.sketches[si].points.len();
    assert_eq!(proj_entities(&live.p, si).len(), 4, "setup");

    assert!(live.p.remove_sketch_projection(si, pid), "the projection must be removed");
    assert!(live.p.sketches[si].projections.is_empty(), "the record is gone");
    assert!(live.p.sketches[si].entities.is_empty(), "the driven entities are gone");
    assert!(live.p.sketches[si].points.len() < pts_before, "the driven points are gone");
}

/// THE POINT OF THE TOOL: the projected outline can be EXTRUDED.
///
/// If shared corners of edges became separate points, the contour would stay open — tracing over the
/// projection would work, but building from it would not, and the tool would be half decorative.
#[test]
fn the_projected_outline_is_a_closed_contour_you_can_extrude() {
    let (mut live, body, si) = part_and_sketch();
    let face = top_face_id(&live.p, body);
    live.project_into(si, body, ProjSource::Face(face));

    let cids = live.p.sketches[si].contour_ids.clone();
    assert!(!cids.is_empty(), "the projection must give a CLOSED contour, and there are no contours");

    // extrude along it — a body must come out with the expected volume
    let sid = live.p.sketches[si].id;
    let nb = live.p.add_extrude_multi(sid, cids.clone(), 5.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());
    live.rebuild();
    let v = live.p.bodies.iter().find(|b| b.id == nb).map(|b| b.mesh.volume()).unwrap_or(0.0);
    assert!((v - 20.0 * 20.0 * 5.0).abs() < 1.0, "a 20x20 face projection extruded by 5 mm must give 2000, and it came out {v}");
}
