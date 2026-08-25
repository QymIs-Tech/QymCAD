mod common;
#[test]
fn eskiz15_no_backside_step() {
    let mut p = common::testbug();
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    // the only expected rebuild error is the chamfer, which is a separate defect; the cuts must not fail. The
    // chamfer error is told apart by the operation code rather than by the word in the text
    let errs: Vec<_> = report
        .errors
        .iter()
        .filter(|(_, e)| !matches!(e, qymcad_core::errors::CoreError::OpFailed(qymcad_core::errors::Op::Chamfer | qymcad_core::errors::Op::ChamferAsym) | qymcad_core::errors::CoreError::ChamferTooBig { .. }))
        .collect();
    assert!(errs.is_empty(), "no new rebuild errors besides the chamfer: {errs:?}");
    // after the entry overshoot was fixed the volume is higher: the false step of about 0.1 mm against the
    // normal is no longer cut away
    let v = shapes.get(&(293u64 as qymcad_core::model::Id)).expect("293").volume();
    eprintln!("the volume of the body is {v:.4}; it was about 3594.96 with the defect and is about 3598.07 now)");
    assert!(v > 3597.0, "the false entry step is no longer cut away: {v:.2}");
    // the cuts still go through, about 39 mm³ removed per contour rather than a mere lid
    let v290 = shapes.get(&(290u64 as qymcad_core::model::Id)).unwrap().volume();
    let v292 = shapes.get(&(292u64 as qymcad_core::model::Id)).unwrap().volume();
    assert!(v290 - v292 > 30.0, "the cut really cuts rather than leaving a lid: {:.1}", v290 - v292);
}
