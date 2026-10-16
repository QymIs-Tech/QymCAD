//! CYLINDRICAL FACES of a mesh: inside or outside, their axis, radius, side and span.

use super::*;

/// Is a cylindrical face a hole or a shaft? This decides which thread to build: on a hole the material lies outside
/// the surface, on a shaft inside it, and the groove profiles are mirror images of each other.
///
/// The answer is computed per triangle of the face: whether its own normal looks away from the axis or towards it,
/// weighted by area. The averaged normal of the whole face cannot work here in principle — on a full cylinder the
/// normals around the circle cancel to zero and the centroid lands on the axis, so the sign comes out at random.
/// That is exactly how a shaft of 30 mm diameter was taken for a hole: the thread ran outwards from the shaft and
/// removed 2 cm³ instead of 12.
pub fn cyl_face_is_internal(mesh: &Mesh, tris: &[u32], axis_pt: [f64; 3], axis_dir: [f64; 3]) -> bool {
    let al = (axis_dir[0] * axis_dir[0] + axis_dir[1] * axis_dir[1] + axis_dir[2] * axis_dir[2]).sqrt();
    if al < 1e-12 {
        return false;
    }
    let ax = [axis_dir[0] / al, axis_dir[1] / al, axis_dir[2] / al];
    let mut score = 0.0_f64;
    for &ti in tris {
        if ti as usize >= mesh.tris.len() {
            continue;
        }
        let (n, area, c) = mesh.tri_normal_area(ti as usize);
        if !area.is_finite() || area <= 0.0 {
            continue;
        }
        let d = [c.x - axis_pt[0], c.y - axis_pt[1], c.z - axis_pt[2]];
        let t = d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2];
        let radial = [d[0] - ax[0] * t, d[1] - ax[1] * t, d[2] - ax[2] * t]; // From the axis to the triangle.
        let rl = (radial[0] * radial[0] + radial[1] * radial[1] + radial[2] * radial[2]).sqrt();
        if rl < 1e-9 {
            continue; // A triangle on the axis itself says nothing about the radial direction.
        }
        let dot = (radial[0] * n[0] + radial[1] * n[1] + radial[2] * n[2]) / rl;
        score += dot * area;
    }
    score < 0.0 // Normals looking towards the axis mean the material is outside, so it is a hole.
}

/// Which way a thread runs from the selected rim: along the selected face, not towards whichever side holds more
/// vertices of the body. Returns the axis turned towards the side the cylindrical face itself lies on.
///
/// Measuring over the whole mesh does not work: with a chamfer on the end face the rim of the thread sits at the
/// base of that chamfer, and on a part whose bulk is above that rim (a boss on a plate, a flange) most vertices end
/// up on the chamfer side, so the thread ran into thin air towards the end face instead of into the material. The
/// face, by contrast, always lies where the thread belongs.
pub fn axis_along_face(mesh: &Mesh, tris: &[u32], rim: [f64; 3], axis: [f64; 3]) -> [f64; 3] {
    let al = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if al < 1e-12 {
        return axis;
    }
    let ax = [axis[0] / al, axis[1] / al, axis[2] / al];
    let mut acc = 0.0;
    for &ti in tris {
        if ti as usize >= mesh.tris.len() {
            continue;
        }
        let (_, area, c) = mesh.tri_normal_area(ti as usize);
        if !area.is_finite() || area <= 0.0 {
            continue;
        }
        let d = [c.x - rim[0], c.y - rim[1], c.z - rim[2]];
        acc += (d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2]) * area;
    }
    if acc < 0.0 {
        [-ax[0], -ax[1], -ax[2]]
    } else {
        ax
    }
}

/// Which way a thread runs from a rim when no face is selected, which is all a timeline rebuild has, since it knows
/// only the edge: take the triangles lying on the cylinder of that radius and see which way they extend.
///
/// Measuring over the whole mesh does not work for the reason given on [`axis_along_face`]: with a chamfer on the
/// end face the thread ran into thin air. This gives the same answer, derived from the radius instead of the
/// face.
pub fn axis_along_cylinder(mesh: &Mesh, rim: [f64; 3], axis: [f64; 3], radius: f64) -> Option<[f64; 3]> {
    let al = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if al < 1e-12 || radius <= 1e-9 {
        return None;
    }
    let ax = [axis[0] / al, axis[1] / al, axis[2] / al];
    let band = (0.05 * radius).max(1e-3);
    let (mut acc, mut hits) = (0.0, 0usize);
    for ti in 0..mesh.tris.len() {
        let (_, area, c) = mesh.tri_normal_area(ti);
        if !area.is_finite() || area <= 0.0 {
            continue;
        }
        let d = [c.x - rim[0], c.y - rim[1], c.z - rim[2]];
        let t = d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2];
        let radial = [d[0] - ax[0] * t, d[1] - ax[1] * t, d[2] - ax[2] * t];
        let r = (radial[0] * radial[0] + radial[1] * radial[1] + radial[2] * radial[2]).sqrt();
        if (r - radius).abs() > band {
            continue;
        }
        acc += t * area;
        hits += 1;
    }
    if hits < 3 || acc.abs() < 1e-9 {
        return None;
    }
    Some(if acc < 0.0 { [-ax[0], -ax[1], -ax[2]] } else { ax })
}

/// The radius of a cylindrical face from its triangles: the mean radius about the axis and the spread (max − min as
/// a fraction of the mean). On a true cylinder the spread is zero; on a chamfer or a cone the radius varies along
/// the axis, which is how the two are told apart. A 1 mm chamfer on a 20 mm diameter gives a spread of about 10%.
///
/// This is needed when choosing the target of a thread: with a chamfer on the end face, the edges of a cylindrical
/// face include rims of different radii, and the edge nearest the click gives the wrong radius, so the thread is
/// built on the wrong surface. Checking against the radius of the face itself picks the right rim.
pub fn cyl_face_radius(mesh: &Mesh, tris: &[u32], axis_pt: [f64; 3], axis_dir: [f64; 3]) -> Option<(f64, f64)> {
    let al = (axis_dir[0] * axis_dir[0] + axis_dir[1] * axis_dir[1] + axis_dir[2] * axis_dir[2]).sqrt();
    if al < 1e-12 {
        return None;
    }
    let ax = [axis_dir[0] / al, axis_dir[1] / al, axis_dir[2] / al];
    // Measured over vertices rather than triangle centres: a centre averages the radii of its vertices and smooths
    // the difference away — a cone from 12 to 8 showed a spread of only 6%, and a small chamfer vanished entirely.
    let (mut sum, mut n) = (0.0, 0usize);
    let (mut rmin, mut rmax) = (f64::MAX, 0.0_f64);
    for &ti in tris {
        if ti as usize >= mesh.tris.len() {
            continue;
        }
        for v in mesh.triangle(ti as usize) {
            let d = [v.x - axis_pt[0], v.y - axis_pt[1], v.z - axis_pt[2]];
            let t = d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2];
            let radial = [d[0] - ax[0] * t, d[1] - ax[1] * t, d[2] - ax[2] * t];
            let r = (radial[0] * radial[0] + radial[1] * radial[1] + radial[2] * radial[2]).sqrt();
            if !r.is_finite() {
                continue;
            }
            sum += r;
            n += 1;
            rmin = rmin.min(r);
            rmax = rmax.max(r);
        }
    }
    if n < 3 {
        return None;
    }
    let mean = sum / n as f64;
    if mean <= 1e-9 {
        return None;
    }
    Some((mean, (rmax - rmin) / mean))
}

/// The same question with no face selected: only the axis, the radius and a span along the axis are known, which is
/// how a thread is specified from a circular edge. Take the mesh triangles lying on the cylinder of radius `r`
/// within that span and ask them for the same side. `None` means there are no such triangles and nothing to judge
/// by, in which case the explicit choice stands.
pub fn cyl_side_from_mesh(mesh: &Mesh, axis_pt: [f64; 3], axis_dir: [f64; 3], r: f64, z0: f64, z1: f64) -> Option<bool> {
    let al = (axis_dir[0] * axis_dir[0] + axis_dir[1] * axis_dir[1] + axis_dir[2] * axis_dir[2]).sqrt();
    if al < 1e-12 || r <= 1e-9 {
        return None;
    }
    let ax = [axis_dir[0] / al, axis_dir[1] / al, axis_dir[2] / al];
    let (lo, hi) = (z0.min(z1), z0.max(z1));
    let band = (0.05 * r).max(1e-3); // The tolerance for "lies on the cylinder".
    let on_cyl: Vec<u32> = (0..mesh.tris.len() as u32)
        .filter(|&ti| {
            let (_, area, c) = mesh.tri_normal_area(ti as usize);
            if !area.is_finite() || area <= 0.0 {
                return false;
            }
            let d = [c.x - axis_pt[0], c.y - axis_pt[1], c.z - axis_pt[2]];
            let t = d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2];
            if t < lo - band || t > hi + band {
                return false;
            }
            let radial = [d[0] - ax[0] * t, d[1] - ax[1] * t, d[2] - ax[2] * t];
            ((radial[0] * radial[0] + radial[1] * radial[1] + radial[2] * radial[2]).sqrt() - r).abs() <= band
        })
        .collect();
    if on_cyl.len() < 3 {
        return None;
    }
    Some(cyl_face_is_internal(mesh, &on_cyl, axis_pt, ax))
}

/// HOW FAR THE CYLINDER OF RADIUS `r` RUNS along `axis_dir` from its rim at `axis_pt`, read off the mesh: the farthest
/// vertex of the triangles lying on it, ahead of the rim. `None` when the mesh shows no such cylinder.
pub fn cyl_span_from_mesh(mesh: &Mesh, axis_pt: [f64; 3], axis_dir: [f64; 3], r: f64) -> Option<f64> {
    let al = (axis_dir[0] * axis_dir[0] + axis_dir[1] * axis_dir[1] + axis_dir[2] * axis_dir[2]).sqrt();
    if al < 1e-12 || r <= 1e-9 {
        return None;
    }
    let ax = [axis_dir[0] / al, axis_dir[1] / al, axis_dir[2] / al];
    let band = (0.05 * r).max(1e-3); // the tolerance for "lies on the cylinder", as `cyl_side_from_mesh` has it
    let along = |p: &Point3| (p.x - axis_pt[0]) * ax[0] + (p.y - axis_pt[1]) * ax[1] + (p.z - axis_pt[2]) * ax[2];
    let on = |p: &Point3| {
        let t = along(p);
        let d = [p.x - axis_pt[0] - ax[0] * t, p.y - axis_pt[1] - ax[1] * t, p.z - axis_pt[2] - ax[2] * t];
        ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() - r).abs() <= band
    };
    mesh.tris
        .iter()
        .filter(|t| t.iter().all(|&v| on(&mesh.verts[v as usize])))
        .flat_map(|t| t.iter().map(|&v| along(&mesh.verts[v as usize])))
        .filter(|t| *t >= -band)
        .fold(None, |m: Option<f64>, t| Some(m.map_or(t, |m| m.max(t))))
}
