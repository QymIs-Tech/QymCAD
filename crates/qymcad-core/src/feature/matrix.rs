//! THE MATRICES OF A PLACEMENT: 3x4, composed, inverted, applied.

use super::*;

pub(crate) fn unit_scale() -> f64 {
    1.0
}

/// Identity rigid transform (3x4 row-major): the body is built in place.
pub const PLACE_IDENTITY: [f64; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// A 90 degree rotation about X, taking the local Y axis (the axis of revolution) onto world Z, so that
/// bodies of revolution such as a cone or a torus stand along Z, as a cylinder does.
pub const ROT_Y_TO_Z: [f64; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Composition of two affine 3x4 row-major matrices: the result applies `b` first and then `a`, that is
/// `a * b`. The linear part (columns 0 to 2) is the product of the rotations and column 3 is
/// `a * b_translation + a_translation`, which is how `world(child) = mat_mul12(world(parent),
/// child.transform)` works.
pub fn mat_mul12(a: &[f64; 12], b: &[f64; 12]) -> [f64; 12] {
    let mut c = [0.0; 12];
    for r in 0..3 {
        for col in 0..3 {
            c[r * 4 + col] = a[r * 4] * b[col] + a[r * 4 + 1] * b[4 + col] + a[r * 4 + 2] * b[8 + col];
        }
        c[r * 4 + 3] = a[r * 4] * b[3] + a[r * 4 + 1] * b[7] + a[r * 4 + 2] * b[11] + a[r * 4 + 3];
    }
    c
}

/// Apply a 3x4 transform to a point, including the translation.
pub fn apply12(m: &[f64; 12], p: [f64; 3]) -> [f64; 3] {
    [m[0] * p[0] + m[1] * p[1] + m[2] * p[2] + m[3], m[4] * p[0] + m[5] * p[1] + m[6] * p[2] + m[7], m[8] * p[0] + m[9] * p[1] + m[10] * p[2] + m[11]]
}

/// Apply the rotational part of a 3x4 transform to a direction, without the translation.
pub fn apply12_dir(m: &[f64; 12], d: [f64; 3]) -> [f64; 3] {
    [m[0] * d[0] + m[1] * d[1] + m[2] * d[2], m[4] * d[0] + m[5] * d[1] + m[6] * d[2], m[8] * d[0] + m[9] * d[1] + m[10] * d[2]]
}

/// Whether a 3x4 transform is the identity, in which case a body needs no transform into world space.
pub fn is_identity12(m: &[f64; 12]) -> bool {
    *m == PLACE_IDENTITY
}

/// Extent of an extrude or a cut along the sketch plane normal, as `(start, total)`: the profile is swept by
/// `total` (a non-negative length) from `start` along the normal. One rule shared by `Extrude` and `Combine`:
/// - [`Reach::BothWays`] gives [-h/2, +h/2], and the second side is not consulted: half the height each way
///   is already both sides, which is why the old pair of booleans had a state that meant nothing;
/// - two-sided (`down > 0`) gives [-down, +h], and [`Reach::Backward`] swaps up and down;
/// - one-sided gives [0, +h] forwards and [-h, 0] backwards.
pub fn extrude_extent(height: f64, down: f64, reach: Reach) -> (f64, f64) {
    let (h, d) = (height.abs(), down.abs());
    match reach {
        Reach::BothWays => (-h / 2.0, h),
        Reach::Backward if d > 1e-9 => (-h, h + d),
        Reach::Forward if d > 1e-9 => (-d, h + d),
        Reach::Backward => (-h, h),
        Reach::Forward => (0.0, h),
    }
}

/// Inverse of an affine 3x4 transform: the inverse of the 3x3 linear part plus a translation of -inv * t.
///
/// Needed to convert a world placement into the local space of the active context, which is what draws a part
/// at its own zero. A degenerate matrix (determinant near zero) yields the identity as a guard.
/// Composition of rigid 3x4 transforms: (a after b)(x) = a(b(x)).
pub fn compose12(a: &[f64; 12], b: &[f64; 12]) -> [f64; 12] {
    let mut r = [0.0; 12];
    for i in 0..3 {
        for j in 0..3 {
            r[i * 4 + j] = a[i * 4] * b[j] + a[i * 4 + 1] * b[4 + j] + a[i * 4 + 2] * b[8 + j];
        }
        r[i * 4 + 3] = a[i * 4] * b[3] + a[i * 4 + 1] * b[7] + a[i * 4 + 2] * b[11] + a[i * 4 + 3];
    }
    r
}

/// A 3x4 rotation about an axis (point `o`, direction `d`, angle `deg`, right-handed). Used to pre-rotate a
/// revolve for the symmetric and flipped cases without touching the kernel.
pub fn rot12_axis(o: [f64; 3], d: [f64; 3], deg: f64) -> [f64; 12] {
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-12);
    let (x, y, z) = (d[0] / l, d[1] / l, d[2] / l);
    let (s, c) = deg.to_radians().sin_cos();
    let ic = 1.0 - c;
    let m = [
        c + x * x * ic,
        x * y * ic - z * s,
        x * z * ic + y * s,
        y * x * ic + z * s,
        c + y * y * ic,
        y * z * ic - x * s,
        z * x * ic - y * s,
        z * y * ic + x * s,
        c + z * z * ic,
    ];
    // Translation: o - R * o.
    let ro = [m[0] * o[0] + m[1] * o[1] + m[2] * o[2], m[3] * o[0] + m[4] * o[1] + m[5] * o[2], m[6] * o[0] + m[7] * o[1] + m[8] * o[2]];
    [m[0], m[1], m[2], o[0] - ro[0], m[3], m[4], m[5], o[1] - ro[1], m[6], m[7], m[8], o[2] - ro[2]]
}

pub fn mat_inv12(m: &[f64; 12]) -> [f64; 12] {
    let (a0, a1, a2) = (m[0], m[1], m[2]);
    let (a3, a4, a5) = (m[4], m[5], m[6]);
    let (a6, a7, a8) = (m[8], m[9], m[10]);
    let det = a0 * (a4 * a8 - a5 * a7) - a1 * (a3 * a8 - a5 * a6) + a2 * (a3 * a7 - a4 * a6);
    if det.abs() < 1e-12 {
        return PLACE_IDENTITY;
    }
    let id = 1.0 / det;
    // inv(3×3) = adj/det
    let i = [
        (a4 * a8 - a5 * a7) * id,
        (a2 * a7 - a1 * a8) * id,
        (a1 * a5 - a2 * a4) * id,
        (a5 * a6 - a3 * a8) * id,
        (a0 * a8 - a2 * a6) * id,
        (a2 * a3 - a0 * a5) * id,
        (a3 * a7 - a4 * a6) * id,
        (a1 * a6 - a0 * a7) * id,
        (a0 * a4 - a1 * a3) * id,
    ];
    let (tx, ty, tz) = (m[3], m[7], m[11]);
    [
        i[0],
        i[1],
        i[2],
        -(i[0] * tx + i[1] * ty + i[2] * tz),
        i[3],
        i[4],
        i[5],
        -(i[3] * tx + i[4] * ty + i[5] * tz),
        i[6],
        i[7],
        i[8],
        -(i[6] * tx + i[7] * ty + i[8] * tz),
    ]
}
