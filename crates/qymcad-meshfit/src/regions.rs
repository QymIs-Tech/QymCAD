//! SPLITTING A PREPARED MESH INTO REGIONS, each lying on one surface: a plane, a cylinder, a cone, a sphere or a
//! torus.
//!
//! By the angle between triangles alone it cannot be done. Measured on a bar with every edge rounded (26 faces),
//! the angle across the border of two faces was 0.12 - 6.5 deg while the angle inside a rounding reached 14.3 deg:
//! the faces meet tangent, and any threshold that keeps a rounding whole glues the whole bar into one region. So a
//! region is grown together with its surface. A seed is given a plane, cylinders, cones and spheres in turn; each
//! grows over the neighbours whose corners lie on it; the one that covers the most area is kept. A rounding's first
//! strip lies 0.017 mm off the plane it leaves, far outside the tolerance. Then neighbours whose union lies on one
//! surface are joined - that is how a torus comes together from its bands - and what is left lying on a larger
//! neighbour's surface joins it.

use crate::{Prepared, NO_NEIGHBOUR};
use nalgebra::{DMatrix, DVector, Matrix3, Matrix4, SymmetricEigen, Vector3, Vector4};
use std::collections::{HashMap, HashSet, VecDeque};

#[cfg(test)]
mod tests {
    use super::*;

    /// THE DERIVATIVES WRITTEN OUT BY HAND AGREE WITH SMALL STEPS OF THE MISS ITSELF. A wrong one would not fail a fit
    /// outright - the refine would still crawl towards the surface - so only this says it is right.
    #[test]
    fn the_written_out_derivatives_agree_with_small_steps() {
        let base = Vector3::new(0.2, -0.3, 0.93).normalize();
        let (u, w) = across(&base);
        let turn = Turn { base, u, w };
        let v = Vector3::new(3.0, -1.5, 7.0);
        agree("cone", &[0.5, 0.2, -1.0, 0.05, -0.03, 0.4], |q| cone_miss(&turn, q, &v));
        agree("torus", &[0.3, -0.2, 0.5, 0.04, 0.02, 6.0, 1.5], |q| torus_miss(&turn, q, &v));
        let foot = Vector3::new(0.4, 0.1, -0.2);
        agree("cylinder", &[0.05, -0.03, 0.2, -0.1, 4.0], |q| cylinder_miss(&turn, &foot, q, &v));
    }

    /// A CYLINDER IS REFINED ONTO CORNERS THAT LIE ON IT, from a start a little off - as the corners of a wall's strips
    /// under a slanted cut stand: an arc of 60 deg of a cylinder of 10, at heights 0 and 20.
    #[test]
    fn a_cylinder_is_refined_onto_its_corners() {
        let pts: Vec<Vector3<f64>> = (0..26)
            .map(|k| {
                let a = (60.0 + 60.0 * k as f64 / 25.0f64).to_radians();
                Vector3::new(10.0 * a.cos(), 10.0 * a.sin(), if k % 2 == 0 { 0.0 } else { 20.0 + 0.02 * k as f64 })
            })
            .collect();
        let normals: Vec<Vector3<f64>> = pts.iter().map(|p| Vector3::new(p.x, p.y, 0.0).normalize() + Vector3::new(0.0, 0.0, 2e-4)).collect();
        let fitted = fit_cylinder(&pts, &normals, 0.00035).expect("a cylinder");
        let worst = pts.iter().map(|p| distance(&fitted, p)).fold(0.0, f64::max);
        assert!(worst < 1e-5, "the cylinder {fitted:?} leaves a corner {worst} off");
    }

    fn agree<const N: usize>(name: &str, q: &[f64; N], miss: impl Fn(&[f64; N]) -> (f64, [f64; N])) {
        let (_, written) = miss(q);
        for k in 0..N {
            let h = 1e-6;
            let (mut up, mut down) = (*q, *q);
            up[k] += h;
            down[k] -= h;
            let stepped = (miss(&up).0 - miss(&down).0) / (2.0 * h);
            assert!((stepped - written[k]).abs() < 1e-5 * (1.0 + stepped.abs()), "{name}: derivative {k} written {}, by steps {stepped}", written[k]);
        }
    }
}

mod surfaces;
pub use surfaces::*;
mod split;
pub use split::*;
mod turning;
pub(crate) use turning::*;
mod grower;
pub(crate) use grower::*;
mod fits;
pub(crate) use fits::*;
