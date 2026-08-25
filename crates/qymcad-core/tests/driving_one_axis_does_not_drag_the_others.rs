//! Driving one axis leaves the others where they stand.
//!
//! The axes of a machine sit in a stack: frame, beam along Y, carriage along X, head along Z. Driving the head
//! along Z has to do exactly that — the head goes up and the carriage stays where it was. A free degree of
//! freedom is free precisely because nothing touched it, so the solver has nothing to move it with either.
//!
//! The measurement this started from: a spindle went up by 60 mm and sideways by 0.183 mm. The analysis showed
//! that what moved sideways was not the spindle but the Z axis beneath it, along its own free X, with the head
//! simply travelling along. Three tenths of a per cent of the stroke, which on a machine is scrap.
//!
//! The cause turned out to be the measure used to reject a step along free directions: the threshold was taken
//! from the free part of the deviation while the result was measured as the full displacement, which includes
//! the requested travel itself. Measured on this very scene: "was 0.696, became 60.000" — the condition could
//! never hold for any drive, and the rejection always fired. One measure now serves both sides
//! (`free_deviation`).
use qymcad_core::feature::{AnchorRef, BasePlane, ComponentKind, JointKind};
use qymcad_core::model::{Id, Project};

fn at(p: &Project, c: Id) -> [f64; 3] {
    let w = p.world_transform(c);
    [w[3], w[7], w[11]]
}

fn part(p: &mut Project, name: &str, parent: Id, x: f64, y: f64, z: f64) -> Id {
    let c = p.add_component_kind(name, ComponentKind::Part);
    if let Some(i) = p.component_index(c) {
        p.components[i].parent = Some(parent);
        p.components[i].transform = [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z];
    }
    c
}

/// A slider held as built: travel along the normal of the named plane.
fn slide(p: &mut Project, base: Id, moving: Id, plane: BasePlane) -> Id {
    let ca = p.add_connector(base, AnchorRef::BasePlane(plane));
    let cb = p.add_connector(moving, AnchorRef::BasePlane(plane));
    let j = p.add_joint(ca, cb, JointKind::Slider);
    assert!(p.set_joint_as_built(j), "the joint has to accept being held as built");
    j
}

/// A machine of three axes. `nested` selects whether the axes sit inside one another, as on a real machine, or
/// all in the root.
fn a_machine_of_three_axes(nested: bool) -> (Project, Id, Id, Id) {
    let mut p = Project::default();
    let root = p.ensure_root();
    let frame = part(&mut p, "frame", root, 0.0, 0.0, 0.0);
    let beam = part(&mut p, "beam", root, 0.0, 100.0, 200.0);
    let carriage = part(&mut p, "carriage", if nested { beam } else { root }, 300.0, 100.0, 200.0);
    let head = part(&mut p, "head", if nested { carriage } else { root }, 300.0, 120.0, 150.0);
    p.set_grounded(frame, true);
    slide(&mut p, frame, beam, BasePlane::XZ); // the beam travels along Y
    slide(&mut p, beam, carriage, BasePlane::YZ); // the carriage along X
    let jz = slide(&mut p, carriage, head, BasePlane::XY); // the head along Z
    p.solve_joints();
    p.solve_joints();
    (p, carriage, head, jz)
}

#[test]
fn the_middle_axis_stays_put_while_the_top_one_travels() {
    for nested in [false, true] {
        let (mut p, carriage, head, jz) = a_machine_of_three_axes(nested);
        let (c0, h0) = (at(&p, carriage), at(&p, head));

        let j = p.joints.iter_mut().find(|x| x.id == jz).expect("the joint of the head");
        j.drive[1] = Some(60.0);
        p.solve_joints();

        let (c1, h1) = (at(&p, carriage), at(&p, head));
        let moved = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let (dc, dh) = (moved(c1, c0), moved(h1, h0));

        // A trap guard: the head really did travel, or the carriage standing still means nothing.
        assert!((dh[2] - 60.0).abs() < 1e-6, "the head did not travel the requested 60 mm (nested={nested}): {dh:?}");

        let sideways = (dc[0] * dc[0] + dc[1] * dc[1] + dc[2] * dc[2]).sqrt();
        assert!(
            sideways < 1e-6,
            "the head was driven along Z and the carriage crept {sideways:.4} mm (nested={nested}): {dc:?}; nothing touched that free degree of freedom"
        );
        // And the head itself travels cleanly along its own axis: the sideways drift is the scrap this check
        // exists for.
        let head_sideways = (dh[0] * dh[0] + dh[1] * dh[1]).sqrt();
        assert!(head_sideways < 1e-6, "the head drifted {head_sideways:.4} mm sideways over a 60 mm stroke (nested={nested}): {dh:?}");
    }
}
