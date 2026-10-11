//! Preparing a mesh for recognition: welding, adjacency, holes, orientation. The meshes here arrive the way a
//! triangle soup does from a file - every triangle with three corners of its own, moved by a few nanometres so
//! that no two copies of a point are equal bit for bit and only a tolerant weld joins them.

use qymcad_core::geom::{Mesh, Point3};
use qymcad_meshfit::{prepare, weld_tolerance, Prepared, NO_NEIGHBOUR};
use std::f64::consts::TAU;

type Tri = [Point3; 3];

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z)
}

fn sub(a: Point3, b: Point3) -> [f64; 3] {
    [a.x - b.x, a.y - b.y, a.z - b.z]
}

fn cross(u: [f64; 3], v: [f64; 3]) -> [f64; 3] {
    [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
}

fn dot(u: [f64; 3], v: [f64; 3]) -> f64 {
    u[0] * v[0] + u[1] * v[1] + u[2] * v[2]
}

fn centroid(t: &Tri) -> [f64; 3] {
    [(t[0].x + t[1].x + t[2].x) / 3.0, (t[0].y + t[1].y + t[2].y) / 3.0, (t[0].z + t[1].z + t[2].z) / 3.0]
}

/// Turns a triangle of a convex solid centred on the origin so that its normal points away from the centre.
fn outward(t: Tri) -> Tri {
    let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
    if dot(n, centroid(&t)) < 0.0 {
        [t[0], t[2], t[1]]
    } else {
        t
    }
}

fn quad(a: Point3, b: Point3, c: Point3, d: Point3) -> [Tri; 2] {
    [outward([a, b, c]), outward([a, c, d])]
}

/// Every triangle gets three corners of its own, moved by up to 6e-9 mm.
fn soup(tris: &[Tri]) -> Mesh {
    let mut verts = Vec::new();
    let mut out = Vec::new();
    for t in tris {
        let base = verts.len() as u32;
        for c in t {
            let j = ((verts.len() * 7919) % 13) as f64 - 6.0;
            verts.push(p(c.x + j * 1e-9, c.y - j * 1e-9, c.z + j * 0.5e-9));
        }
        out.push([base, base + 1, base + 2]);
    }
    Mesh { verts, tris: out, ..Default::default() }
}

/// A cube centred on the origin; the two triangles of the top face are 2 and 3.
fn cube(s: f64) -> Vec<Tri> {
    let h = s / 2.0;
    let c = |x: f64, y: f64, z: f64| p(x * h, y * h, z * h);
    let faces = [
        [c(-1., -1., -1.), c(-1., 1., -1.), c(1., 1., -1.), c(1., -1., -1.)],
        [c(-1., -1., 1.), c(1., -1., 1.), c(1., 1., 1.), c(-1., 1., 1.)],
        [c(-1., -1., -1.), c(1., -1., -1.), c(1., -1., 1.), c(-1., -1., 1.)],
        [c(-1., 1., -1.), c(-1., 1., 1.), c(1., 1., 1.), c(1., 1., -1.)],
        [c(-1., -1., -1.), c(-1., -1., 1.), c(-1., 1., 1.), c(-1., 1., -1.)],
        [c(1., -1., -1.), c(1., 1., -1.), c(1., 1., 1.), c(1., -1., 1.)],
    ];
    faces.iter().flat_map(|f| quad(f[0], f[1], f[2], f[3])).collect()
}

/// A cylinder along Z centred on the origin, `n` sides, both caps as fans around a centre point.
fn cylinder(r: f64, h: f64, n: usize) -> Vec<Tri> {
    let ring = |z: f64, i: usize| {
        let a = TAU * (i % n) as f64 / n as f64;
        p(r * a.cos(), r * a.sin(), z)
    };
    let (lo, hi) = (-h / 2.0, h / 2.0);
    let mut out = Vec::new();
    for i in 0..n {
        out.extend(quad(ring(lo, i), ring(lo, i + 1), ring(hi, i + 1), ring(hi, i)));
        out.push(outward([p(0., 0., lo), ring(lo, i), ring(lo, i + 1)]));
        out.push(outward([p(0., 0., hi), ring(hi, i), ring(hi, i + 1)]));
    }
    out
}

/// A UV sphere centred on the origin: `n` slices, `m` stacks, a pole at each end.
fn sphere(r: f64, n: usize, m: usize) -> Vec<Tri> {
    let at = |k: usize, i: usize| {
        if k == 0 {
            return p(0., 0., r);
        }
        if k == m {
            return p(0., 0., -r);
        }
        let t = TAU / 2.0 * k as f64 / m as f64;
        let a = TAU * (i % n) as f64 / n as f64;
        p(r * t.sin() * a.cos(), r * t.sin() * a.sin(), r * t.cos())
    };
    let mut out = Vec::new();
    for k in 0..m {
        for i in 0..n {
            let (a, b, c, d) = (at(k, i), at(k + 1, i), at(k + 1, i + 1), at(k, i + 1));
            if k == 0 {
                out.push(outward([a, b, c]));
            } else if k == m - 1 {
                out.push(outward([a, b, d]));
            } else {
                out.extend(quad(a, b, c, d));
            }
        }
    }
    out
}

/// The three solids with the number of distinct corners each really has: the cube 8, the cylinder two rings
/// of 24 plus two cap centres, the sphere 11 rings of 24 plus two poles.
fn solids() -> Vec<(&'static str, Vec<Tri>, usize)> {
    vec![("cube", cube(100.0), 8), ("cylinder", cylinder(20.0, 50.0, 24), 50), ("sphere", sphere(30.0, 24, 12), 266)]
}

fn prepared(tris: &[Tri]) -> Prepared {
    let mesh = soup(tris);
    prepare(&mesh, weld_tolerance(&mesh))
}

/// The triangles whose normal points towards the centre of a convex solid centred on the origin.
fn inward(p: &Prepared) -> usize {
    (0..p.mesh.tris.len())
        .filter(|&t| {
            let [a, b, c] = p.mesh.triangle(t);
            dot(p.normals[t], centroid(&[a, b, c])) <= 0.0
        })
        .count()
}

#[test]
fn welding_leaves_the_corners_the_solid_really_has() {
    for (name, tris, corners) in solids() {
        let p = prepared(&tris);
        assert_eq!(p.mesh.verts.len(), corners, "{name}: {} corners came in", tris.len() * 3);
        assert_eq!(p.mesh.tris.len(), tris.len(), "{name}: no triangle may be lost");
        assert_eq!(p.dropped, 0, "{name}");
    }
}

#[test]
fn a_closed_solid_has_a_neighbour_across_every_side_and_no_hole() {
    for (name, tris, _) in solids() {
        let p = prepared(&tris);
        assert!(p.holes.is_empty(), "{name}: holes {:?}", p.holes);
        assert_eq!(p.non_manifold, 0, "{name}");
        for (t, sides) in p.neighbours.iter().enumerate() {
            for &u in sides {
                assert_ne!(u, NO_NEIGHBOUR, "{name}: triangle {t} has an open side");
                assert!(p.neighbours[u as usize].contains(&(t as u32)), "{name}: {t} -> {u} is one-way");
            }
        }
    }
}

#[test]
fn every_normal_points_out_even_when_the_file_mixed_them() {
    for (name, tris, _) in solids() {
        let mut mixed = tris.clone();
        let mut turned = 0;
        for (i, t) in mixed.iter_mut().enumerate() {
            if i % 3 == 1 {
                t.swap(1, 2);
                turned += 1;
            }
        }
        let inside_out: Vec<Tri> = tris.iter().map(|t| [t[0], t[2], t[1]]).collect();
        for (case, input, flips) in [("as written", tris.clone(), 0), ("mixed", mixed, turned), ("inside out", inside_out, tris.len())] {
            let p = prepared(&input);
            assert_eq!(inward(&p), 0, "{name} {case}: normals point in");
            assert_eq!(p.flipped, flips, "{name} {case}: triangles turned");
        }
    }
}

#[test]
fn an_open_box_reports_its_one_hole_with_four_corners() {
    let mut tris = cube(100.0);
    tris.drain(2..4);
    let p = prepared(&tris);
    assert_eq!(p.holes.len(), 1, "holes {:?}", p.holes);
    assert_eq!(p.holes[0].len(), 4, "the hole {:?}", p.holes[0]);
    assert_eq!(inward(&p), 0, "normals of the open box point in");
}

#[test]
fn degenerate_triangles_are_dropped_without_opening_a_hole() {
    let mut tris = cube(100.0);
    let (a, b) = (tris[0][0], tris[0][1]);
    let mid = p((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0);
    tris.push([a, mid, b]); // no area: three points on one line
    tris.push([a, a, b]); // two corners in one place
    tris.push(tris[5]); // the same triangle twice
    let p = prepared(&tris);
    assert_eq!(p.dropped, 3);
    assert_eq!(p.mesh.tris.len(), 12);
    assert_eq!(p.origin, (0..12).collect::<Vec<u32>>(), "each triangle kept points back at its own in the file");
    assert_eq!(p.mesh.verts.len(), 8, "the corner of the dropped sliver stays behind");
    assert!(p.holes.is_empty(), "holes {:?}", p.holes);
    assert_eq!(p.non_manifold, 0);
}

/// Measured on a 3MF of printed gears: the file closes every T-junction with a triangle of no area, and each one
/// dropped opened a hole in a sound gear - 38 dropped, 37 holes. Kept as a sliver, it made no face in the kernel, which
/// dropped it in its turn: gears 14 and 15 came back shells. Here the bottom of the cube has a corner `m` in the middle
/// of its edge `a-b` while the side uses `a-b` whole, and the flat `a m b` closes the gap: it is mended - the side's
/// triangle across `a-b` split at `m` - and no triangle without an area is left, none lost, no hole opened.
#[test]
fn a_flat_triangle_that_closes_a_seam_is_mended() {
    let mut tris = cube(100.0);
    let bottom = tris.drain(0..2).collect::<Vec<_>>();
    let [a, b, c] = bottom[0];
    let d = bottom[1][2];
    let m = p((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0);
    tris.extend([outward([a, m, d]), outward([m, c, d]), outward([m, b, c])]);
    tris.push([a, b, m]);
    let p = prepared(&tris);
    assert_eq!(p.dropped, 0, "the flat triangle is part of a sound surface");
    assert!(p.holes.is_empty(), "holes {:?}", p.holes);
    assert_eq!(p.mesh.tris.len(), 14);
    assert!(p.slivers.is_empty(), "the flat triangle is still kept: {:?}", p.slivers);
    let flat = (0..p.mesh.tris.len())
        .filter(|&t| {
            let [a, b, c] = p.mesh.triangle(t);
            let n = cross(sub(b, a), sub(c, a));
            dot(n, n) < 1e-18
        })
        .count();
    assert_eq!(flat, 0, "a triangle without an area is left");
    let facing_in = (0..p.mesh.tris.len()).filter(|&t| !p.slivers.contains(&(t as u32))).filter(|&t| dot(p.normals[t], centroid(&p.mesh.triangle(t))) <= 0.0).count();
    assert_eq!(facing_in, 0, "every triangle with an area faces out");
}

#[test]
fn the_angle_across_every_side_is_known() {
    let angles = |p: &Prepared| -> Vec<f64> { (0..p.mesh.tris.len()).flat_map(|t| (0..3).filter_map(move |k| p.dihedral_deg(t, k))).collect() };
    // The corners are moved by up to 6e-9 mm, which tilts a flat pair by about 1e-6 deg.
    let near = |v: &[f64], want: f64| v.iter().filter(|a| (*a - want).abs() < 1e-4).count();

    let cube = angles(&prepared(&cube(100.0)));
    assert_eq!(cube.len(), 36, "every side of the cube has a neighbour");
    assert_eq!(near(&cube, 0.0), 12, "the six diagonals, seen from both sides");
    assert_eq!(near(&cube, 90.0), 24, "the twelve edges, seen from both sides");

    let side = angles(&prepared(&cylinder(20.0, 50.0, 24)));
    let (flat, turn, edge) = (near(&side, 0.0), near(&side, 15.0), near(&side, 90.0));
    assert_eq!(flat + turn + edge, side.len(), "only 0, 15 and 90 deg on a 24-sided cylinder: {side:?}");
    assert!(flat > 0 && turn > 0 && edge > 0, "flat {flat}, turn {turn}, edge {edge}");
}

#[test]
fn an_edge_shared_by_three_triangles_is_counted_not_guessed() {
    let (a, b) = (p(0., 0., 0.), p(10., 0., 0.));
    let tris = [[a, b, p(5., 5., 0.)], [b, a, p(5., -5., 0.)], [a, b, p(5., 0., 5.)]];
    let p = prepared(&tris);
    assert_eq!(p.non_manifold, 1);
    for t in 0..3 {
        assert!(p.neighbours[t].iter().all(|&u| u == NO_NEIGHBOUR), "triangle {t}: {:?}", p.neighbours[t]);
    }
}
