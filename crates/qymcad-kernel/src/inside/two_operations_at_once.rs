//! TWO OPERATIONS AT ONCE ON BODIES SHARING AN ELEMENT GIVE WHAT EACH GIVES ALONE. The rebuild works the nodes of one
//! wave on threads of their own; an algorithm of OCCT that writes into what it is handed - a boolean raising the
//! tolerance of a vertex of its argument - writes into every body holding that vertex. Every entry that hands a body
//! to such an algorithm works on a copy of its own (`QYM_OWN_COPY`), so two of them at once meet nowhere.
use crate::Shape;

/// How many times the two are built together.
const ROUNDS: usize = 50;

/// The block 10 x 10 x 10 at the origin, and the same block at (10, 10, 10): united, two solids meeting in one vertex.
struct Corner {
    first: Shape,
    second: Shape,
}

fn corner() -> Corner {
    let square = [0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0];
    let a = Shape::extrude(&square, 10.0).expect("a block");
    let b = Shape::extrude(&square, 10.0).expect("a block").transformed(&[1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 10.0, 0.0, 0.0, 1.0, 10.0]).expect("the block placed");
    let union = Shape::fuse_many(&[&a, &b]).expect("the union");
    Corner { first: union.solid_at(0).expect("the first solid"), second: union.solid_at(1).expect("the second solid") }
}

/// The body cut by a pin of radius 3 whose surface runs through the shared corner, and the body itself after, both as
/// BRep. Its surface through the vertex makes the boolean raise the tolerance of that vertex: cut in place, both bodies
/// holding it changed (a pin centred on the corner changed neither).
struct Cut {
    result: Option<Vec<u8>>,
    body_after: Option<Vec<u8>>,
}

fn cut_at_the_corner(body: Shape) -> Cut {
    let pin = Shape::cylinder(3.0, 30.0).expect("a pin").transformed(&[1.0, 0.0, 0.0, 13.0, 0.0, 1.0, 0.0, 10.0, 0.0, 0.0, 1.0, -5.0]).expect("the pin placed");
    let result = body.boolean(&pin, 1).and_then(|r| r.to_brep_bytes());
    Cut { result, body_after: body.to_brep_bytes() }
}

#[test]
fn two_cuts_at_once_on_bodies_sharing_a_vertex_give_what_each_gives_alone() {
    let setup = corner();
    assert!(setup.first.shares_geometry(&setup.second), "setup: the two solids do not share their corner");
    let alone_first = cut_at_the_corner(corner().first).result;
    let alone_second = cut_at_the_corner(corner().second).result;
    assert!(alone_first.is_some() && alone_second.is_some(), "setup: a cut alone gave nothing");
    let mut unlike: Vec<String> = Vec::new();
    for round in 1..=ROUNDS {
        let Corner { first, second } = corner();
        let (first_was, second_was) = (first.to_brep_bytes(), second.to_brep_bytes());
        let one = std::thread::spawn(move || cut_at_the_corner(first));
        let two = std::thread::spawn(move || cut_at_the_corner(second));
        let (one, two) = (one.join().expect("the first cut"), two.join().expect("the second cut"));
        let mut wrong: Vec<&str> = Vec::new();
        if one.result != alone_first || two.result != alone_second {
            wrong.push("a cut differs from the cut alone");
        }
        if one.body_after != first_was || two.body_after != second_was {
            wrong.push("a body cut changed under its cut");
        }
        if !wrong.is_empty() {
            unlike.push(format!("round {round}: {}", wrong.join("; ")));
        }
    }
    assert!(unlike.is_empty(), "{} of {ROUNDS} rounds:\n  {}", unlike.len(), unlike.join("\n  "));
}
