mod common;
#[test]
fn body_293_is_one_valid_solid() {
    let mut p = common::testbug();
    let (_r, shapes) = qymcad_testkit::regenerate(&mut p);
    let s = shapes.get(&(293u64 as qymcad_core::model::Id)).expect("293");
    assert_eq!(s.tessellate(0.5).len(), 1, "a part is one solid");
    assert!(s.is_valid(), "the body is valid");
}
