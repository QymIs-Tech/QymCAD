//! Will the kernel accept our surface?
//!
//! This exploration answers one question: a subdivision cage converts into Bezier patches — will they close into
//! a shell without holes, and will that shell become a solid? The geometry itself is already checked in
//! `qymcad-core`, where a patch matches the limit and neighbours meet along their seam; what is checked here is
//! the kernel.
//!
//! The reference is a torus rather than a cube. A cube has eight extraordinary corners with no exact patches
//! around them, so holes appear by construction and "the shell did not close" proves nothing on such a scene. A
//! torus has no extraordinary vertices at all, so any hole is a fault of ours.
use qymcad_kernel::Shape;
use qymcad_core::subdiv::Cage;

/// The shell closes without a single hole: the main answer of the exploration.
#[test]
fn the_kernel_sews_our_patches_into_a_watertight_shell() {
    let cage = Cage::torus(16, 8, 30.0, 10.0);
    let set = cage.to_bezier_patches(2);
    assert_eq!(set.irregular, 0, "a torus must have no unconverted faces");

    let nets: Vec<[[[f64; 3]; 4]; 4]> = set.patches.iter().map(|p| p.cps).collect();
    let (shape, free) = Shape::from_bezier_patches(&nets, 1e-6, true).expect("the kernel did not accept the patches at all");
    assert_eq!(free, 0, "the shell did not close: {free} unstitched edges over {} patches", nets.len());
    assert!(shape.is_valid(), "the kernel stitched it and considers the result invalid");
}

/// And it really is a solid, with the right volume.
///
/// A closed shell is half the answer: a solid still has to come out of it, and its volume has to agree with the
/// analytic volume of a torus, `2π²Rr²`, allowing for the limit surface of the cage being slightly thinner than
/// an ideal torus. The check catches patches turned inside out, where the volume of such a solid goes negative
/// or to zero.
#[test]
fn the_shell_becomes_a_solid_of_the_right_size() {
    let (r_major, r_minor) = (30.0, 10.0);
    let cage = Cage::torus(24, 12, r_major, r_minor);
    let nets: Vec<[[[f64; 3]; 4]; 4]> = cage.to_bezier_patches(2).patches.iter().map(|p| p.cps).collect();
    let (shape, free) = Shape::from_bezier_patches(&nets, 1e-6, true).expect("the kernel did not accept the patches");
    assert_eq!(free, 0, "a shell with holes will not become a solid");

    let ideal = 2.0 * std::f64::consts::PI * std::f64::consts::PI * r_major * r_minor * r_minor;
    let v = shape.volume();
    assert!(v > 0.0, "a volume of {v} means the solid is inside out or empty");
    let ratio = v / ideal;
    assert!((0.85..1.05).contains(&ratio), "the volume disagrees with the analytic torus: {v:.0} against {ideal:.0} ({ratio:.3})");
}
