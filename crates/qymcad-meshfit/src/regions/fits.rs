//! THE FITS OF SURFACES: planes, cylinders, spheres, cones, tori, and their refinement.

use super::*;

pub(crate) fn distance(s: &Surface, v: &Vector3<f64>) -> f64 {
    match s {
        Surface::Plane { point, normal } => (v - Vector3::from(*point)).dot(&Vector3::from(*normal)).abs(),
        Surface::Cylinder { point, axis, radius } => {
            let d = v - Vector3::from(*point);
            let a = Vector3::from(*axis);
            ((d - a * d.dot(&a)).norm() - radius).abs()
        }
        Surface::Sphere { center, radius } => ((v - Vector3::from(*center)).norm() - radius).abs(),
        Surface::Cone { apex, axis, half_angle } => {
            let d = v - Vector3::from(*apex);
            let a = Vector3::from(*axis);
            let z = d.dot(&a);
            ((d - a * z).norm() * half_angle.cos() - z * half_angle.sin()).abs()
        }
        Surface::Torus { center, axis, major, minor } => {
            let d = v - Vector3::from(*center);
            let a = Vector3::from(*axis);
            let z = d.dot(&a);
            (((d - a * z).norm() - major).hypot(z) - minor).abs()
        }
        // nothing lies on a free form by being near it: it is whatever its own triangles are
        Surface::Free { .. } | Surface::Spline(_) => f64::INFINITY,
        Surface::Helix { .. } | Surface::RoundHelix { .. } => helix_at(s, v).map_or(f64::INFINITY, |h| h.off.abs()),
        Surface::Coil { .. } => coil_at(s, v).map_or(f64::INFINITY, |c| c.off.abs()),
    }
}

/// The unit normal of `s` at the point nearest `v`, or `None` on a cylinder's axis or a sphere's centre.
pub(crate) fn normal_at(s: &Surface, v: &Vector3<f64>) -> Option<Vector3<f64>> {
    let n = match s {
        Surface::Plane { normal, .. } => Vector3::from(*normal),
        Surface::Cylinder { point, axis, .. } => {
            let d = v - Vector3::from(*point);
            let a = Vector3::from(*axis);
            d - a * d.dot(&a)
        }
        Surface::Sphere { center, .. } => v - Vector3::from(*center),
        Surface::Cone { apex, axis, half_angle } => {
            let d = v - Vector3::from(*apex);
            let a = Vector3::from(*axis);
            let out = (d - a * d.dot(&a)).try_normalize(1e-12)?;
            out * half_angle.cos() - a * half_angle.sin()
        }
        Surface::Torus { center, axis, major, .. } => {
            let d = v - Vector3::from(*center);
            let a = Vector3::from(*axis);
            let out = (d - a * d.dot(&a)).try_normalize(1e-12)?;
            d - out * *major
        }
        Surface::Free { .. } | Surface::Spline(_) => return None,
        Surface::Helix { .. } | Surface::RoundHelix { .. } => helix_at(s, v)?.normal,
        Surface::Coil { .. } => coil_at(s, v)?.out,
    };
    n.try_normalize(1e-12)
}

pub(crate) fn mean(pts: &[Vector3<f64>]) -> Vector3<f64> {
    pts.iter().fold(Vector3::zeros(), |a, p| a + p) / pts.len().max(1) as f64
}

/// The eigenvectors of a symmetric matrix, smallest eigenvalue first, with the eigenvalues.
pub(crate) fn eigen_sorted(m: Matrix3<f64>) -> ([f64; 3], [Vector3<f64>; 3]) {
    let e = SymmetricEigen::new(m);
    let mut idx = [0, 1, 2];
    idx.sort_by(|&a, &b| e.eigenvalues[a].total_cmp(&e.eigenvalues[b]));
    (idx.map(|i| e.eigenvalues[i]), idx.map(|i| e.eigenvectors.column(i).into_owned()))
}

pub(crate) fn fit_plane(pts: &[Vector3<f64>]) -> Option<Surface> {
    if pts.len() < 3 {
        return None;
    }
    let c = mean(pts);
    let m = pts.iter().fold(Matrix3::zeros(), |a, p| a + (p - c) * (p - c).transpose());
    let (_, v) = eigen_sorted(m);
    Some(Surface::Plane { point: c.into(), normal: v[0].normalize().into() })
}

/// The axis is the direction every normal of a cylinder is square to: the least eigenvector of the normals'
/// scatter. Normals that all point one way (a flat patch) give no axis. The circle is then fitted to the corners
/// dropped onto the plane across the axis (the algebraic fit: linear, and exact for points on a circle).
///
/// THEN THE AXIS IS REFINED ON THE CORNERS. A triangle's normal is square to the axis only when the triangle stands
/// square on the wall; a long strip whose top corner sits at another angle than its foot - the wall under a curve two
/// surfaces meet along - tilts its normal, and over a strip 27 mm tall that tilt moves the corners dropped across the
/// axis as far as the sag of the arc itself. Measured on a cylinder of 10 under a sphere set 0.1 mm off its axis: the
/// corners lay on the wall exactly, and the wall still came in 70 cylinders of 12.1 to 150 over 105 regions.
pub(crate) fn fit_cylinder(pts: &[Vector3<f64>], normals: &[Vector3<f64>], accept: f64) -> Option<Surface> {
    if pts.len() < 3 || normals.len() < 2 {
        return None;
    }
    let m = normals.iter().fold(Matrix3::zeros(), |a, n| a + n * n.transpose());
    let (val, vec) = eigen_sorted(m);
    if val[1] < 1e-9 * val[2] {
        return None; // the normals span no plane: a flat patch
    }
    let first = circle_about(pts, &vec[0].normalize())?;
    // five numbers: the axis tilted along two directions across it, its foot moved along the same two, the radius
    if pts.len() < 6 {
        return Some(first);
    }
    let Surface::Cylinder { point, axis, radius } = first else { return Some(first) };
    let (c0, base) = (Vector3::from(point), Vector3::from(axis));
    let (u, w) = across(&base);
    let turn = Turn { base, u, w };
    let miss = |q: &[f64; 5], v: &Vector3<f64>| cylinder_miss(&turn, &c0, q, v);
    let mut q = [0.0, 0.0, 0.0, 0.0, radius];
    if rms(&q, pts, &miss) <= accept || !refined(&mut q, pts, &miss, accept) || q[4].is_nan() || q[4] <= 0.0 {
        return Some(first);
    }
    let a = (base + u * q[0] + w * q[1]).normalize();
    let c = c0 + u * q[2] + w * q[3];
    Some(Surface::Cylinder { point: c.into(), axis: a.into(), radius: q[4] })
}

/// How far `v` lies off the cylinder `q` - the axis `turn.base` tilted by `q[0]` and `q[1]` along `turn.u` and `turn.w`,
/// its foot `c0` moved by `q[2]` and `q[3]` along them, radius `q[4]` - with the derivatives by those five numbers.
pub(crate) fn cylinder_miss(turn: &Turn, c0: &Vector3<f64>, q: &[f64; 5], v: &Vector3<f64>) -> (f64, [f64; 5]) {
    let (u, w) = (turn.u, turn.w);
    let big = turn.base + u * q[0] + w * q[1];
    let len = big.norm();
    let a = big / len;
    let d = v - (c0 + u * q[2] + w * q[3]);
    let along = d.dot(&a);
    let e = d - a * along;
    let dist = e.norm();
    if dist < 1e-12 {
        return (-q[4], [0.0, 0.0, 0.0, 0.0, -1.0]);
    }
    let h = e / dist;
    // the axis turns by q[0] and q[1] as da = (u - a (a.u)) / len; the distance moves against the along-axis part of that
    let da0 = (u - a * a.dot(&u)) / len;
    let da1 = (w - a * a.dot(&w)) / len;
    let dd0 = -(h.dot(&da0) * along + h.dot(&a) * d.dot(&da0));
    let dd1 = -(h.dot(&da1) * along + h.dot(&a) * d.dot(&da1));
    let dc0 = -(h.dot(&u) - h.dot(&a) * a.dot(&u));
    let dc1 = -(h.dot(&w) - h.dot(&a) * a.dot(&w));
    (dist - q[4], [dd0, dd1, dc0, dc1, -1.0])
}

/// The cylinder about a given axis direction: the circle fitted to the corners dropped onto the plane across it.
pub(crate) fn circle_about(pts: &[Vector3<f64>], axis: &Vector3<f64>) -> Option<Surface> {
    if pts.len() < 3 {
        return None;
    }
    let axis = *axis;
    let u = axis.cross(&if axis.x.abs() < 0.9 { Vector3::x() } else { Vector3::y() }).normalize();
    let w = axis.cross(&u);
    let c = mean(pts);
    let flat: Vec<(f64, f64)> = pts.iter().map(|p| ((p - c).dot(&u), (p - c).dot(&w))).collect();
    let (mut ata, mut atb) = (Matrix3::zeros(), Vector3::zeros());
    for &(x, y) in &flat {
        let row = Vector3::new(x, y, 1.0);
        ata += row * row.transpose();
        atb += row * -(x * x + y * y);
    }
    let sol = ata.lu().solve(&atb)?;
    let (cx, cy) = (-sol[0] / 2.0, -sol[1] / 2.0);
    let r2 = cx * cx + cy * cy - sol[2];
    if r2 <= 0.0 || !r2.is_finite() {
        return None;
    }
    let point = c + u * cx + w * cy;
    Some(Surface::Cylinder { point: point.into(), axis: axis.into(), radius: r2.sqrt() })
}

/// The algebraic sphere through the corners: linear in the centre and in r^2 - |c|^2. Corners on one plane give no
/// sphere.
pub(crate) fn fit_sphere(pts: &[Vector3<f64>]) -> Option<Surface> {
    if pts.len() < 4 {
        return None;
    }
    let c = mean(pts);
    let (mut ata, mut atb) = (Matrix4::zeros(), Vector4::zeros());
    for p in pts {
        let d = p - c;
        let row = Vector4::new(d.x, d.y, d.z, 1.0);
        ata += row * row.transpose();
        atb += row * -d.norm_squared();
    }
    let sol = ata.lu().solve(&atb)?;
    let o = Vector3::new(-sol[0] / 2.0, -sol[1] / 2.0, -sol[2] / 2.0);
    let r2 = o.norm_squared() - sol[3];
    if r2 <= 0.0 || !r2.is_finite() {
        return None;
    }
    Some(Surface::Sphere { center: (c + o).into(), radius: r2.sqrt() })
}

/// The axis of a surface of revolution: the one line every normal's line meets - on a cylinder, a cone and a torus
/// alike. In Plucker coordinates a line (a, M) meets the line through p along n when a . (p x n) + M . n = 0, which
/// is linear in the six numbers, so the axis is the least eigenvector of a 6 x 6 matrix. It replaced a search along
/// the tube's centre points, which on a narrow band of a torus lie on too short an arc to fit a circle to.
/// Returns the unit direction and the point of the axis nearest the feet's middle; `None` when the normals do not
/// pin a line down.
pub(crate) fn axis_by_normal_lines(feet: &[Vector3<f64>], normals: &[Vector3<f64>]) -> Option<(Vector3<f64>, Vector3<f64>)> {
    if normals.len() < 5 {
        return None;
    }
    let o = mean(feet);
    let mut m = nalgebra::Matrix6::<f64>::zeros();
    for (p, n) in feet.iter().zip(normals) {
        let row = nalgebra::Vector6::from_iterator((p - o).cross(n).iter().chain(n.iter()).copied());
        m += row * row.transpose();
    }
    let e = SymmetricEigen::new(m);
    let k = (0..6).min_by(|&a, &b| e.eigenvalues[a].total_cmp(&e.eigenvalues[b]))?;
    let x = e.eigenvectors.column(k);
    let (a, big_m) = (Vector3::new(x[0], x[1], x[2]), Vector3::new(x[3], x[4], x[5]));
    let len = a.norm();
    if len < 1e-9 {
        return None;
    }
    let (a, big_m) = (a / len, big_m / len);
    Some((a, o + a.cross(&big_m)))
}

/// Two unit vectors square to `a` and to each other.
pub(crate) fn across(a: &Vector3<f64>) -> (Vector3<f64>, Vector3<f64>) {
    let u = a.cross(&if a.x.abs() < 0.9 { Vector3::x() } else { Vector3::y() }).normalize();
    (u, a.cross(&u))
}

/// Each corner as (height along the axis, distance from it).
pub(crate) fn profile(pts: &[Vector3<f64>], a: &Vector3<f64>, on: &Vector3<f64>) -> Vec<(f64, f64)> {
    pts.iter()
        .map(|p| {
            let d = p - on;
            let z = d.dot(a);
            (z, (d - a * z).norm())
        })
        .collect()
}

/// The size of what the corners span, for judging a first fit before refining it.
pub(crate) fn span(pts: &[Vector3<f64>]) -> f64 {
    let (lo, hi) = pts.iter().fold((Vector3::repeat(f64::MAX), Vector3::repeat(f64::MIN)), |(lo, hi), p| (lo.inf(p), hi.sup(p)));
    (hi - lo).norm()
}

/// The root mean square of `residual` over the corners.
pub(crate) fn rms<const N: usize>(q: &[f64; N], pts: &[Vector3<f64>], miss: &impl Fn(&[f64; N], &Vector3<f64>) -> (f64, [f64; N])) -> f64 {
    (pts.iter().map(|p| miss(q, p).0.powi(2)).sum::<f64>() / pts.len().max(1) as f64).sqrt()
}

/// A first fit off by more than this share of what it spans is no fit to refine: it is some other surface. The first
/// fit of a torus band is off by 5 - 8% of its span, so the share is wide; the early stop of the refine does the rest.
pub(crate) const NOT_EVEN_CLOSE: f64 = 0.1;

/// A cone: in the axis's frame the corners lie on a straight line of distance against height, which gives the
/// half-angle and the apex. Refined on the corners, which lie on the surface.
pub(crate) fn fit_cone(pts: &[Vector3<f64>], feet: &[Vector3<f64>], normals: &[Vector3<f64>], accept: f64) -> Option<Surface> {
    let (a, on) = axis_by_normal_lines(feet, normals)?;
    let zr = profile(pts, &a, &on);
    let n = zr.len() as f64;
    let (mz, mr) = (zr.iter().map(|x| x.0).sum::<f64>() / n, zr.iter().map(|x| x.1).sum::<f64>() / n);
    let var = zr.iter().map(|x| (x.0 - mz).powi(2)).sum::<f64>();
    if var < 1e-12 {
        return None; // no length along the axis to tell a slope from
    }
    let k = zr.iter().map(|x| (x.0 - mz) * (x.1 - mr)).sum::<f64>() / var;
    if k.abs() < 1e-3 {
        return None; // a cylinder
    }
    let apex = on + a * (mz - mr / k);
    let axis = if k > 0.0 { a } else { -a };
    let (u, w) = across(&axis);
    let mut q = [apex.x, apex.y, apex.z, 0.0, 0.0, k.abs().atan()];
    let turn = Turn { base: axis, u, w };
    let at = |q: &[f64; 6]| (Vector3::new(q[0], q[1], q[2]), turn.tilted(q[3], q[4]).0, q[5]);
    let miss = |q: &[f64; 6], v: &Vector3<f64>| cone_miss(&turn, q, v);
    if rms(&q, pts, &miss) > NOT_EVEN_CLOSE * span(pts) {
        return None;
    }
    if !refined(&mut q, pts, &miss, accept) {
        return None;
    }
    let (apex, axis, half) = at(&q);
    (half > 1e-4 && half < std::f64::consts::FRAC_PI_2 - 1e-4).then(|| Surface::Cone { apex: apex.into(), axis: axis.into(), half_angle: half })
}

/// A torus: in the axis's frame the corners lie on a circle of distance against height - the tube's section - which
/// gives both radii and the centre. Refined on the corners.
pub(crate) fn fit_torus(pts: &[Vector3<f64>], feet: &[Vector3<f64>], normals: &[Vector3<f64>], accept: f64) -> Option<Surface> {
    let (a, on) = axis_by_normal_lines(feet, normals)?;
    let zr = profile(pts, &a, &on);
    let n = zr.len() as f64;
    let (mz, mr) = (zr.iter().map(|x| x.0).sum::<f64>() / n, zr.iter().map(|x| x.1).sum::<f64>() / n);
    let (mut lhs, mut rhs) = (Matrix3::zeros(), Vector3::zeros());
    for &(z, r) in &zr {
        let (x, y) = (r - mr, z - mz);
        let row = Vector3::new(x, y, 1.0);
        lhs += row * row.transpose();
        rhs += row * -(x * x + y * y);
    }
    let sol = lhs.lu().solve(&rhs)?;
    let (cx, cy) = (-sol[0] / 2.0, -sol[1] / 2.0);
    let r2 = cx * cx + cy * cy - sol[2];
    let (major, z0) = (mr + cx, mz + cy);
    if r2.is_nan() || r2 <= 0.0 || major.is_nan() || major <= 0.0 {
        return None;
    }
    let centre = on + a * z0;
    let (u, w) = across(&a);
    let mut q = [centre.x, centre.y, centre.z, 0.0, 0.0, major, r2.sqrt()];
    let turn = Turn { base: a, u, w };
    let at = |q: &[f64; 7]| (Vector3::new(q[0], q[1], q[2]), turn.tilted(q[3], q[4]).0, q[5], q[6]);
    let miss = |q: &[f64; 7], v: &Vector3<f64>| torus_miss(&turn, q, v);
    if rms(&q, pts, &miss) > NOT_EVEN_CLOSE * span(pts) {
        return None;
    }
    if !refined(&mut q, pts, &miss, accept) {
        return None;
    }
    let (centre, axis, major, minor) = at(&q);
    (major > 0.0 && minor > 0.0 && major.is_finite() && minor.is_finite()).then(|| Surface::Torus { center: centre.into(), axis: axis.into(), major, minor })
}

/// The frame an axis is turned in while a cone or a torus is refined: the axis the fit started from and two unit
/// vectors across it. The axis is `base + s u + t w`, made unit.
pub(crate) struct Turn {
    pub(crate) base: Vector3<f64>,
    pub(crate) u: Vector3<f64>,
    pub(crate) w: Vector3<f64>,
}

impl Turn {
    /// The axis at (s, t), and how it moves with s and with t.
    pub(crate) fn tilted(&self, s: f64, t: f64) -> (Vector3<f64>, Vector3<f64>, Vector3<f64>) {
        let m = self.base + self.u * s + self.w * t;
        let len = m.norm();
        let a = m / len;
        (a, (self.u - a * a.dot(&self.u)) / len, (self.w - a * a.dot(&self.w)) / len)
    }
}

/// How far `v` lies off the cone q = (apex, s, t, half-angle), signed, with its derivatives in q. In the axis's frame a
/// corner is at height z and distance rho; the miss is rho cos - z sin, and moving the apex moves the corner the other
/// way, turning the axis moves z by d and rho by -z along the corner's own direction across it.
pub(crate) fn cone_miss(turn: &Turn, q: &[f64; 6], v: &Vector3<f64>) -> (f64, [f64; 6]) {
    let (a, by_s, by_t) = turn.tilted(q[3], q[4]);
    let d = v - Vector3::new(q[0], q[1], q[2]);
    let z = d.dot(&a);
    let e = d - a * z;
    let rho = e.norm();
    let out = if rho > 1e-15 { e / rho } else { Vector3::zeros() };
    let (sin, cos) = q[5].sin_cos();
    let by_d = out * cos - a * sin;
    let by_axis = -(out * (z * cos)) - d * sin;
    (rho * cos - z * sin, [-by_d.x, -by_d.y, -by_d.z, by_axis.dot(&by_s), by_axis.dot(&by_t), -rho * sin - z * cos])
}

/// How far `v` lies off the torus q = (centre, s, t, major, minor), signed, with its derivatives in q: the distance
/// from the tube's centre circle less the tube's radius, moved by the centre against the surface's normal.
pub(crate) fn torus_miss(turn: &Turn, q: &[f64; 7], v: &Vector3<f64>) -> (f64, [f64; 7]) {
    let (a, by_s, by_t) = turn.tilted(q[3], q[4]);
    let d = v - Vector3::new(q[0], q[1], q[2]);
    let z = d.dot(&a);
    let e = d - a * z;
    let rho = e.norm();
    let out = if rho > 1e-15 { e / rho } else { Vector3::zeros() };
    let off = rho - q[5];
    let from_circle = off.hypot(z).max(1e-300);
    let by_d = out * (off / from_circle) + a * (z / from_circle);
    let by_axis = (d - out * off) * (z / from_circle);
    (from_circle - q[6], [-by_d.x, -by_d.y, -by_d.z, by_axis.dot(&by_s), by_axis.dot(&by_t), -off / from_circle, -1.0])
}

/// A first fit is refined on a few corners in one run of up to 23 steps, and taken - then refined in full - if that
/// brings it within this many tolerances. The run stops at the third step if the miss has not halved. Measured: a torus
/// from a print program's mesh went 0.33 -> 0.0096 in three steps and on to 4e-6 mm, a cone of our kernel to 1e-7 -
/// far inside one tolerance; on an organic mesh of 225 154 triangles a threshold of ten tolerances let 54 338 unions
/// through to the full refine, and 1 606 of 36 875 of them then lay on the surface - the rest took 10 s for nothing.
pub(crate) const SHORT_REFINE_TO: f64 = 1.0;

/// How far past the tolerance a single corner may lie after the short run for the full refine to be worth it.
pub(crate) const WORST_OFF: f64 = 1.0;

/// Refine `q` in one run on at most 60 corners - enough to tell a torus band of some sixty from an organic patch -
/// dropping it early if it is not closing in; whether it came within `accept`.
pub(crate) fn refined<const N: usize>(q: &mut [f64; N], pts: &[Vector3<f64>], miss: &impl Fn(&[f64; N], &Vector3<f64>) -> (f64, [f64; N]), accept: f64) -> bool {
    let first = rms(q, pts, miss);
    let now = refine(q, pts, miss, 23, 60, Some((3, first / 2.0)), accept);
    if now > accept {
        return false;
    }
    // THE WORST CORNER, NOT THE MEAN: two dozen corners of an organic patch take a torus of seven numbers within the
    // tolerance on the mean and still leave a corner out, so the union never lies on it. Measured: 25 114 such fits,
    // 1 576 of them lying on the surface; a fit of a CAD mesh is off by 1e-6 at its worst corner after the short run.
    if pts.iter().any(|p| miss(q, p).0.abs() > WORST_OFF * accept) {
        return false;
    }
    refine(q, pts, miss, 20, 1500, None, accept / (SHORT_REFINE_TO * 10.0));
    true
}

/// Least squares on the corners' distances to a surface given by `q`: Gauss-Newton with a Marquardt damping, the
/// derivatives by small steps. The corners of a CAD mesh lie on the surface, so the minimum is the exact surface;
/// a first guess from the triangles' middles is off by the tessellation's own sag. At most `most` corners are
/// used, evenly picked, for at most `steps` steps, stopping once the root mean square miss is within `enough`.
pub(crate) fn refine<const N: usize>(
    q: &mut [f64; N],
    pts: &[Vector3<f64>],
    miss: &impl Fn(&[f64; N], &Vector3<f64>) -> (f64, [f64; N]),
    steps: usize,
    most: usize,
    give_up: Option<(usize, f64)>,
    enough: f64,
) -> f64 {
    let step = pts.len().div_ceil(most).max(1);
    let pts: Vec<&Vector3<f64>> = pts.iter().step_by(step).collect();
    let cost = |q: &[f64; N]| pts.iter().map(|p| miss(q, p).0.powi(2)).sum::<f64>();
    let mut now = cost(q);
    let mut damping = 1e-3;
    for step_no in 0..steps {
        let off_by = (now / pts.len().max(1) as f64).sqrt();
        // STOPPED AS SOON AS IT IS CLOSE ENOUGH. It ran every step to the end, and on an organic mesh of 225 154
        // triangles, where a small smooth patch takes a torus of seven numbers easily, the refines took 20 s of 23.
        if off_by <= enough || give_up.is_some_and(|(at, above)| step_no == at && off_by > above) {
            break;
        }
        // the normal equations are summed corner by corner, from derivatives written out by hand: a matrix as tall as
        // the corners was made at every step, and each derivative by a small step cost a whole miss more
        let (mut lhs, mut rhs) = (DMatrix::<f64>::zeros(N, N), DVector::<f64>::zeros(N));
        for p in &pts {
            let (r, grad) = miss(q, p);
            for a in 0..N {
                rhs[a] -= grad[a] * r;
                for b in 0..N {
                    lhs[(a, b)] += grad[a] * grad[b];
                }
            }
        }
        for k in 0..N {
            lhs[(k, k)] *= 1.0 + damping;
        }
        let Some(dq) = lhs.lu().solve(&rhs) else { break };
        let mut next = *q;
        for k in 0..N {
            next[k] += dq[k];
        }
        let then = cost(&next);
        if then < now {
            let gained = now - then;
            *q = next;
            now = then;
            damping *= 0.3;
            // a step that gains less than a ten-thousandth has found the bottom; where that bottom is above the
            // tolerance - an organic patch is no torus - every further step was paid for nothing
            if gained <= 1e-4 * now {
                break;
            }
        } else {
            damping *= 10.0;
            if damping > 1e12 {
                break;
            }
        }
    }
    (now / pts.len().max(1) as f64).sqrt()
}
