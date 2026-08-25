//! A large selection survives the file.
//!
//! Selecting 52 edges for a fillet and saving left the project unable to open: the recursion limit was exceeded.
//! The selection was written as a ladder of `Union(Union(...))` whose depth grew with the number of edges, and
//! the reader hit its limit. The file saved silently, so access to the work was lost without anything having
//! been done wrong.
//!
//! What is checked here is what matters: however much is picked with the mouse, the document reads back.
use qymcad_core::refs::{Query, Ref};

/// The depth of a query does not depend on how much was selected.
#[test]
fn a_pick_list_stays_flat_however_much_is_picked() {
    let depth = |r: &Ref| -> usize {
        fn go(q: &Query) -> usize {
            match q {
                Query::Union(a, b) | Query::Minus(a, b) | Query::Filter(a, b) | Query::Between(a, b) => 1 + go(a).max(go(b)),
                Query::Adjacent(a) => 1 + go(a),
                Query::TangentChain { seed, .. } => 1 + go(seed),
                _ => 1,
            }
        }
        go(&r.query)
    };
    let few: Vec<u32> = (1..=3).collect();
    let many: Vec<u32> = (1..=200).collect();
    assert_eq!(depth(&Ref::picks(&few)), depth(&Ref::picks(&many)), "the depth grew with the size of the selection, so a large selection would make the document unreadable again");
    assert_eq!(Ref::picks(&many).query.picked_descs().len(), 200, "the selection has to survive in full");
}

/// And it survives a save and reopen.
#[test]
fn a_big_pick_list_survives_a_round_trip_through_the_document() {
    let many: Vec<u32> = (1..=200).collect();
    let r = Ref::picks(&many);
    let text = ron::ser::to_string(&r).expect("writing");
    let back: Ref = ron::from_str(&text).expect("a document with a large selection has to read back");
    assert_eq!(back.query.picked_descs(), many, "the selection drifted after a write and read");
}
