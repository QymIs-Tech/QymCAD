//! THE CORNER IS SHOWN BEFORE IT IS TAKEN, and nothing of the drawing is changed while it is shown.
//!
//! Reported: the fillet and the chamfer gave nothing to look at until Enter had already cut the corner. What a person
//! wants while typing the value is where the corner WILL go — the two ends the lines will be cut at and the arc or the
//! straight cut that will stand between them — with the lines themselves left as they are.
use qymcad_core::feature::Purpose;
use qymcad_core::model::Project;

/// An L of two lines meeting at (20, 0). Answers the project, the sketch, the corner and the two edges at it.
fn an_angle() -> (Project, usize, u64, (u64, u64)) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let left = p.add_line_entity(si, 0.0, 0.0, 20.0, 0.0, Purpose::Real);
    let up = p.add_line_entity(si, 20.0, 0.0, 20.0, 20.0, Purpose::Real);
    p.regen_sketch(si);
    let corner = p.sketches[si].points.iter().find(|q| (q.x - 20.0).abs() < 1e-6 && q.y.abs() < 1e-6).map(|q| q.id).expect("the corner");
    (p, si, corner, (left, up))
}

fn near(a: [f64; 2], b: (f64, f64)) -> bool {
    (a[0] - b.0).hypot(a[1] - b.1) < 1e-6
}

#[test]
fn the_chamfer_is_shown_as_the_cut_it_will_make() {
    let (p, si, corner, edges) = an_angle();
    let b = p
        .corner_blend(si, corner, edges, qymcad_core::model::CornerCut::Chamfer(qymcad_core::model::ChamferLegs { mode: qymcad_core::feature::ChamferMode::TwoDist, first: 5.0, second: 5.0 }))
        .expect("a leg of 5 fits this corner");
    assert_eq!(b.vertex, [20.0, 0.0], "the preview is not drawn at the corner");
    assert!(near(b.ends[0], (15.0, 0.0)) && near(b.ends[1], (20.0, 5.0)), "a leg of 5 does not meet the lines at (15, 0) and (20, 5): {:?}", b.ends);
    assert!(b.arc.is_none(), "a chamfer is a straight cut and has no arc");
    // and the drawing is exactly as it was: the corner is still sharp
    let lines = p.sketches[si].entities.iter().filter(|e| e.kind.as_line().is_some()).count();
    assert_eq!(lines, 2, "the preview changed the drawing: {lines} lines where there were two");
    assert!(p.sketches[si].points.iter().any(|q| q.id == corner), "the preview took the corner's point away");
}

#[test]
fn the_fillet_is_shown_as_the_arc_it_will_leave() {
    let (p, si, corner, edges) = an_angle();
    let b = p.corner_blend(si, corner, edges, qymcad_core::model::CornerCut::Fillet(qymcad_core::model::FilletSize::radius(5.0))).expect("a radius of 5 fits this corner");
    // a right angle: the arc touches each line 5 from the corner, its centre on the bisector
    assert!(near(b.ends[0], (15.0, 0.0)) && near(b.ends[1], (20.0, 5.0)), "the arc of radius 5 does not meet the lines at (15, 0) and (20, 5): {:?}", b.ends);
    let (c, r) = b.arc.expect("a fillet leaves an arc");
    assert!((r - 5.0).abs() < 1e-9, "the arc is not of the radius asked for: {r}");
    assert!(near(c, (15.0, 5.0)), "the centre of the arc of radius 5 at a right angle is not (15, 5): {c:?}");
    for e in b.ends {
        assert!((e[0] - c[0]).hypot(e[1] - c[1] - 0.0).abs() - 5.0 < 1e-6, "the end {e:?} does not lie on the arc: {c:?} r{r}");
    }
}

#[test]
fn a_value_the_corner_cannot_take_is_shown_as_nothing() {
    let (p, si, corner, edges) = an_angle();
    assert!(
        p.corner_blend(si, corner, edges, qymcad_core::model::CornerCut::Chamfer(qymcad_core::model::ChamferLegs { mode: qymcad_core::feature::ChamferMode::TwoDist, first: 25.0, second: 25.0 }))
            .is_none(),
        "a leg of 25 was drawn on lines of 20: a preview must not promise what cannot be built"
    );
    assert!(p.corner_blend(si, corner, edges, qymcad_core::model::CornerCut::Fillet(qymcad_core::model::FilletSize::radius(25.0))).is_none(), "a radius of 25 was drawn on lines of 20");
    assert!(
        p.corner_blend(si, corner, edges, qymcad_core::model::CornerCut::Chamfer(qymcad_core::model::ChamferLegs { mode: qymcad_core::feature::ChamferMode::TwoDist, first: 0.0, second: 0.0 }))
            .is_none(),
        "a leg of zero is nothing to draw"
    );
    assert!(
        p.corner_blend(si, corner, edges, qymcad_core::model::CornerCut::Chamfer(qymcad_core::model::ChamferLegs { mode: qymcad_core::feature::ChamferMode::TwoDist, first: 19.9, second: 19.9 }))
            .is_some(),
        "a leg of 19.9 fits a line of 20 and must be shown"
    );
}

#[test]
fn a_pair_that_is_not_a_corner_is_shown_as_nothing() {
    let (mut p, si, corner, (left, _up)) = an_angle();
    // a line that does not meet the corner at all: there is no corner, and nothing to show
    let far = p.add_line_entity(si, 60.0, 60.0, 80.0, 60.0, Purpose::Real);
    p.regen_sketch(si);
    assert_eq!(p.corner_of_pair(si, left, far), None, "setup: these two lines share no corner");
    assert!(
        p.corner_blend(si, corner, (left, far), qymcad_core::model::CornerCut::Chamfer(qymcad_core::model::ChamferLegs { mode: qymcad_core::feature::ChamferMode::TwoDist, first: 5.0, second: 5.0 }))
            .is_none(),
        "a preview was drawn between two lines that meet nowhere"
    );
}
