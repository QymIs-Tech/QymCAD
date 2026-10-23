//! A BLEND IS BOXED BY THE FACES IT MADE OR TRIMMED: the box a result of a blend is checked against the body it blends
//! with is the exact box of those faces alone - the faces the blend kept stand where they stood - against the exact box
//! of the body, kept with it once counted. The exact box of both bodies whole, 80 spline faces each, took 1.2 s of the
//! 1.5 s a fillet of 8 edges took on a propeller, and 60 ms of every edge tried alone when the edges would not take
//! together. A result standing far past its body is refused in words whatever its faces.
use crate::Shape;

/// A block 40 x 30 x 10 and its top front edge, the one along x at y 0, z 10.
struct BlockWithEdge {
    block: Shape,
    top_front: u32,
}

fn a_block_and_its_top_front_edge() -> BlockWithEdge {
    let block = Shape::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 30.0, 0.0, 30.0], 10.0).expect("the block");
    let edge = block.edges_info().into_iter().find(|e| e.circle.is_none() && e.poly.iter().all(|p| p[1].abs() < 1e-6 && (p[2] - 10.0).abs() < 1e-6)).expect("setup: the block has no top front edge");
    BlockWithEdge { block, top_front: edge.id }
}

#[test]
fn the_box_of_the_new_faces_of_a_fillet_is_the_box_of_its_surface_and_the_faces_it_trimmed() {
    let BlockWithEdge { block, top_front } = a_block_and_its_top_front_edge();
    let rounded = block.fillet_edges(3.0, &[top_front]).expect("R3 rounds an edge of a block 10 high");
    let b = rounded.bbox_exact_of_new_faces().expect("the fillet made no face of its own");
    // the surface of R3 along the edge and the two faces it trimmed, the top and the front: the whole top, y 0..30 at
    // z 10, and the whole front, z 0..10 at y 0, along x 0..40
    let want = [0.0, 0.0, 0.0, 40.0, 30.0, 10.0];
    let off = (0..6).map(|i| (b[i] - want[i]).abs()).fold(0.0, f64::max);
    assert!(off < 1e-3, "the box of the faces made and trimmed is {b:?}, expecting {want:?}");
    let poles = rounded.bbox_poles().expect("the box of the poles");
    assert!((0..6).all(|i| (poles[i] - want[i]).abs() < 1e-3), "the box of the poles of a block with a rounding is {poles:?}");
    // the block itself was born of its profile: every face of it is one it made
    let whole = block.bbox_exact_of_new_faces().expect("the block made no face");
    assert!((whole[4] - 30.0).abs() < 1e-6 && (whole[5] - 10.0).abs() < 1e-6, "the box of the faces the block made is {whole:?}, not the block");
}

/// A RESULT STANDING FAR PAST ITS BODY IS REFUSED, whatever faces it is made of: the block itself, stood 590 mm away as
/// if a blend had given it, is no blend of the block and is said so.
#[test]
fn a_result_standing_far_past_its_body_is_refused_in_words() {
    let BlockWithEdge { block, .. } = a_block_and_its_top_front_edge();
    let far = block.transformed(&[1.0, 0.0, 0.0, 590.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).expect("the block stood away");
    crate::clear_kernel_refusal();
    assert!(block.blended(Some(far), 3.0).is_none(), "a body standing 590 mm past the body it blends was taken");
    let why = crate::last_kernel_refusal().unwrap_or_default();
    assert!(why.contains("past the body"), "the refusal does not say the body stands past: {why:?}");
}
