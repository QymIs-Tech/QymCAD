//! A STORED BODY WITH AN EDGE OCCT CANNOT SAMPLE: an offset curve whose basis starts degenerate throws when a point of
//! it is asked for. Asked for its edges, the body gives both - the bad one without a line, the straight one whole.
//! Without the guards the exception left the C bridge and Rust aborted: opening such a project file closed the whole
//! program the moment its edges were drawn. In a binary of its own, so an abort names this check.
//!
//! The body is `tests/data/unsampled_edge.qymb`, written by `tests/data/gen_unsampled_edge.cpp`.

#[test]
fn an_edge_that_cannot_be_sampled_loses_only_its_own_line() {
    let s = qymcad_kernel::Shape::from_brep_bytes(include_bytes!("data/unsampled_edge.qymb")).expect("the stored body reads");
    let (lines, ids) = s.edges_with_ids();
    assert_eq!(ids, vec![1, 2], "the two edges are not both there, by their names");
    assert!(lines[0].is_empty(), "the edge on the offset curve was drawn with {} points", lines[0].len());
    assert_eq!(lines[1].len(), 25, "the straight edge is not drawn whole");
}
