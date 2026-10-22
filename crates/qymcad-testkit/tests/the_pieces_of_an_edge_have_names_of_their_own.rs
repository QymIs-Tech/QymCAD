//! THE PIECES OF AN EDGE HAVE NAMES OF THEIR OWN. Faces divided by a plane cut the edge between them in two, and the
//! history gave the name of the edge to both pieces: one address for two edges, and a rounding picked on the upper
//! piece took the lower after any rebuild (the matrix of references through an edit above: all seven edits after "the
//! faces divided at z = 5"). Each piece is named by the pair of faces it runs between.
//!
//! A block 40 x 30 x 10, its faces divided at z = 5: the front right upright edge in two, the lower between the lower
//! pieces of the front and the right faces, the upper between their upper pieces. Live OCCT, headless.
use qymcad_core::feature::Reach;
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

#[test]
fn the_two_pieces_of_an_edge_cut_by_a_split_of_its_faces_are_named_by_their_own_faces() {
    let mut p = Project::default();
    p.new_document();
    let pts = [(0.0, 0.0), (40.0, 0.0), (40.0, 30.0), (0.0, 30.0)].iter().map(|&(x, y)| Point2::new(x, y)).collect();
    let sketch = p.add_line_sketch("Base", pts, true);
    let si = p.sketch_index(sketch).expect("the sketch");
    p.regen_sketch(si);
    p.add_sketch_node(sketch, "Base");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let block = p.add_extrude_multi(sketch, closed, 10.0, Reach::Forward, 0.0, vec![]);
    let body = p.add_split_face(block, 0, 0, 5.0);
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the block is not built: {:?}", report.errors);
    let upright: Vec<&qymcad_core::geom::MeshEdge> = p.regen_edges[&body].iter().filter(|e| (e.mid[0] - 40.0).abs() < 1e-6 && e.mid[1].abs() < 1e-6).collect();
    assert_eq!(upright.len(), 2, "setup: the front right upright edge is not in two");
    let upper = upright.iter().find(|e| e.mid[2] > 5.0).expect("the upper piece");
    let lower = upright.iter().find(|e| e.mid[2] < 5.0).expect("the lower piece");
    assert_ne!(upper.id, lower.id, "the two pieces of the edge wear one name: {}", p.names.describe(upper.id));
    // the faces of the name of the upper piece stand above z = 5
    let faces = p.names.edge(upper.id).map(|n| n.faces).expect("the upper piece is named by a pair of faces");
    let above = |f: u32| p.regen_faces[&body].iter().any(|x| x.id == f && x.centroid.z > 5.0);
    assert!(faces.iter().all(|f| above(*f)), "the upper piece is named after faces not beside it: {}", p.names.describe(upper.id));
}
