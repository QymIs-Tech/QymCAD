//! A BOOLEAN THAT GIVES NO BODY SAYS SO before anything is laid.
//!
//! Reported behaviour: intersecting two halves of a block that only touch laid a red node "the result is an
//! empty body" instead of refusing: OCCT answers such an intersection with a valid, empty compound.
use crate::{BoolVerdict, Shape};

/// A 10 x 10 x 10 block with its lower corner at x = `x`.
fn block(x: f64) -> Shape {
    Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0)
        .expect("the block")
        .transformed(&[1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0])
        .expect("the block moved into place")
}

#[test]
fn touching_bodies_have_nothing_in_common_and_overlapping_ones_do() {
    let mut fails = Vec::new();
    let a = block(0.0);
    for (x, op, want) in [
        (10.0, 2, Err(BoolVerdict::NothingInCommon)), // touching along a face
        (20.0, 2, Err(BoolVerdict::NothingInCommon)), // apart
        (20.0, 0, Err(BoolVerdict::CutRemovedNothing)),
        (5.0, 2, Ok(500.0)),
        (5.0, 0, Ok(500.0)),
        (10.0, 1, Ok(2000.0)),
    ] {
        let got = a.boolean_checked(&block(x), op).map(|s| (s.volume() * 1000.0).round() / 1000.0);
        if got != want {
            fails.push(format!("B at x = {x}, op {op}: {got:?}, want {want:?}"));
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}
