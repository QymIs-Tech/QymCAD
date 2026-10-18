//! A WALL OF THE BASE AND A WALL OF A BOSS MERGED INTO ONE FACE KEEP THE NAME OF THE BASE. The boss is laid on a body
//! that is there; its walls are named by the operation that makes them, and where one lies in the plane of a wall of the
//! base the two merge into one face - which is the wall of the base grown, as it stands in the timeline before the boss.
//! Reported behaviour: on the case of the examples a dimension edited by -10% let the wall of a boss take the name of a
//! wall of the base it merged with, and every reference to that wall went with it.
use qymcad_core::feature::{Extent, Purpose, Reach};
use qymcad_core::geom::Point2;
use qymcad_core::model::{CombineSpan, Id, Project};
use qymcad_core::names::Role;

/// A rectangle from the origin to `corner` on its own sketch on XY, and the first closed contour of it.
fn rectangle(p: &mut Project, corner: Point2) -> Profile {
    let si = p.new_sketch("s");
    let sketch = p.sketches[si].id;
    p.add_sketch_node(sketch, "s");
    p.add_rect_entity(si, 0.0, 0.0, corner.x, corner.y, Purpose::Real);
    p.regen_sketch(si);
    let contour = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("a closed contour");
    Profile { sketch, contour }
}

/// A sketch and one of its contours.
struct Profile {
    sketch: Id,
    contour: Id,
}

#[test]
fn the_wall_of_a_block_merged_with_the_wall_of_a_boss_is_the_wall_of_the_block() {
    let mut p = Project::default();
    p.new_document();
    let base = rectangle(&mut p, Point2::new(10.0, 10.0));
    let block = p.add_extrude_multi(base.sketch, vec![base.contour], 6.0, Reach::Forward, 0.0, vec![]);
    let tool = rectangle(&mut p, Point2::new(10.0, 4.0));
    let boss = p.add_combine_multi_op(block, tool.sketch, vec![tool.contour], CombineSpan { height: 20.0, down: 0.0, extent: Extent::default(), fill: &[] }, 1);
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the block with its boss does not build: {:?}", report.errors);
    // the wall at x = 0: the block's from z 0 to 6 along y 0..10, the boss's from z 0 to 20 along y 0..4 - one face
    let walls: Vec<&qymcad_core::geom::MeshFace> = p.regen_faces[&boss].iter().filter(|f| f.normal[0] < -0.99).collect();
    assert_eq!(walls.len(), 1, "the two walls at x = 0 did not merge into one face: {}", walls.len());
    let name = p.names.get(walls[0].id);
    assert!(matches!(name, Some(n) if n.feature == block && n.role == Role::Wall), "the merged wall is named {}, not after the block {block} under it", p.names.describe(walls[0].id));
}
