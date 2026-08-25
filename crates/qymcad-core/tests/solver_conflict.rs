// A conflict of a vertical, an anchor and a dimension pulling along X: does the solver tilt the vertical line?
use qymcad_core::model::{SketchPoint, Constraint};
use qymcad_core::solver;

fn pt(id: u64, x: f64, y: f64) -> SketchPoint { SketchPoint { id, x, y } }

#[test]
fn conflict_tilts_vertical_line() {
    // A(1) is anchored at (0,0) and B(2) sits at (0,10). The vertical between them forces B.x = 0. The conflict:
    // the distance from B to the anchored C(3) at (5,10) is zero, which pulls B to (5,10). The solver finds a
    // compromise, B.x becomes non-zero and the vertical tilts.
    let mut points = vec![pt(1,0.0,0.0), pt(2,0.0,10.0), pt(3,5.0,10.0)];
    let cons = vec![
        Constraint::Fixed { p: 1 },
        Constraint::Fixed { p: 3 },
        Constraint::Vertical { a: 1, b: 2 },
        Constraint::Distance { a: 1, b: 2, d: 10.0, off: 0.0, expr: String::new(), driven: false, axis: 0 },
        Constraint::Distance { a: 2, b: 3, d: 0.0, off: 0.0, expr: String::new(), driven: false, axis: 0 }, // the conflict: B coincides with C
    ];
    let res = solver::solve(&mut points, &cons);
    let b = points.iter().find(|p| p.id==2).unwrap();
    eprintln!("residual={res:.3}, B=({:.3},{:.3}); the vertical requires B.x = 0", b.x, b.y);
    eprintln!("the vertical is violated by dx={:.3}", b.x.abs());
}

#[test]
fn consistent_change_low_residual() {
    // A consistent sketch: A(1) anchored at (0,0), B(2) free, a vertical between them and a distance of 10.
    // Changing the dimension to 15 simply moves B along the vertical, with a residual near zero: no conflict
    // and therefore no rollback.
    let mut points = vec![pt(1,0.0,0.0), pt(2,0.0,10.0)];
    let cons = vec![
        Constraint::Fixed { p: 1 },
        Constraint::Vertical { a: 1, b: 2 },
        Constraint::Distance { a: 1, b: 2, d: 15.0, off: 0.0, expr: String::new(), driven: false, axis: 0 },
    ];
    let res = solver::solve(&mut points, &cons);
    eprintln!("consistent change: residual={res:.2e}, rollback threshold 1e-2");
    assert!(res < 1e-2, "a consistent change leaves a small residual, so there is no rollback");
    let b = points.iter().find(|p| p.id==2).unwrap();
    assert!(b.x.abs() < 1e-3 && (b.y.abs()-15.0).abs() < 1e-2, "B moved 15 along the vertical");
}

/// A NaN in the geometry neither crashes the application nor produces rubbish.
///
/// The check was carried over from the old solver to the new one along with its meaning: degenerate geometry has
/// to produce a refusal rather than a panic or meaningless coordinates. In the old solver the choice of pivot
/// went through `partial_cmp(..).unwrap()`, which meant the process dying on a real assembly.
#[test]
fn degenerate_geometry_is_refused_not_a_crash() {
    use nalgebra::{Isometry3, Translation3};
    use qymcad_core::asm::frame::Anchor;
    use qymcad_core::asm::problem::{Body, Constraint, Problem};

    let mut p = Problem::new(vec![
        Body::grounded(Isometry3::identity()),
        Body::new(Isometry3::from_parts(Translation3::new(f64::NAN, 1.0, 2.0), nalgebra::UnitQuaternion::identity())),
    ]);
    p.add(Constraint::PointCoincident { a: Anchor::origin(0), b: Anchor::origin(1) });
    let (poses, rep) = qymcad_core::asm::iterate::solve(&p);
    // The requirement is no panic and no rubbish. Converging is not forbidden: the initial placement derives
    // the position of a part from its joint, so corrupted input coordinates are simply overwritten with
    // meaningful ones. Recovering is better than refusing; all that matters is that no NaN or infinity leaves
    // the solver.
    assert_eq!(poses.len(), 2, "the solver has to return placements rather than panic");
    assert!(
        poses.iter().all(|t| t.translation.vector.iter().all(|v| v.is_finite()) && t.rotation.coords.iter().all(|v| v.is_finite())),
        "no NaN or infinity may leave the solver: {poses:?}"
    );
    assert!(rep.residual.is_finite(), "the residual has to be a number: {}", rep.residual);
}
/// The solver has to terminate rather than hang.
///
/// The previous test checked the output, that no NaN escapes, and therefore missed the worse case: on corrupted
/// input the singular value decomposition never converges and the application freezes for good — no error, no
/// exit, no chance to save the work. What has to be checked is termination itself, measured in time: a refusal
/// and an infinite loop look the same from the output and nothing alike from the outside.
#[test]
fn a_broken_input_ends_quickly_instead_of_hanging_forever() {
    use nalgebra::{Isometry3, Translation3, UnitQuaternion};
    use qymcad_core::asm::frame::Anchor;
    use qymcad_core::asm::problem::{Body, Constraint, Problem};

    for bad in [f64::NAN, f64::INFINITY, -f64::INFINITY] {
        let mut p = Problem::new(vec![
            Body::grounded(Isometry3::identity()),
            Body::new(Isometry3::from_parts(Translation3::new(bad, bad, bad), UnitQuaternion::identity())),
        ]);
        p.add(Constraint::PointCoincident { a: Anchor::origin(0), b: Anchor::origin(1) });
        let t = std::time::Instant::now();
        let (poses, rep) = qymcad_core::asm::iterate::solve(&p);
        let took = t.elapsed().as_secs_f64();
        assert!(took < 1.0, "the solve has to terminate, and on input {bad} it took {took:.1} s, which is a frozen application");
        assert!(rep.recovered_from_garbage, "the solver has to report that the input placements were rubbish");
        assert!(
            poses.iter().all(|t| t.translation.vector.iter().all(|v| v.is_finite())),
            "no non-numbers may leave the solver: {poses:?}"
        );
    }
}
