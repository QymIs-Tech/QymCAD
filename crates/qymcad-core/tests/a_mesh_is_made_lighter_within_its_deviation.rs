//! A MESH IS MADE LIGHTER WITHIN A DEVIATION: a fine mesh of a ball and of a cube, as a CAD exports them, comes back
//! with a fraction of its triangles, its corners still on the surface within the deviation, closed, and holding its
//! volume. Reported behaviour: a body made from a 178 000-triangle mesh as it is hung the window on every operation.
use qymcad_core::geom::{Mesh, Point3};
use qymcad_core::mesh_simplify::simplify;

/// A ball of `r` in `rings` x `segs` quads, every triangle with its own corners, as a triangle file holds them.
fn ball(r: f64, rings: usize, segs: usize) -> Mesh {
    let at = |i: usize, j: usize| {
        if i == 0 || i == rings {
            return Point3::new(0.0, 0.0, if i == 0 { r } else { -r });
        }
        let (t, a) = (std::f64::consts::PI * i as f64 / rings as f64, std::f64::consts::TAU * (j % segs) as f64 / segs as f64);
        Point3::new(r * t.sin() * a.cos(), r * t.sin() * a.sin(), r * t.cos())
    };
    let (mut verts, mut tris) = (Vec::new(), Vec::new());
    let mut tri = |a: Point3, b: Point3, c: Point3| {
        let k = verts.len() as u32;
        verts.extend([a, b, c]);
        tris.push([k, k + 1, k + 2]);
    };
    for i in 0..rings {
        for j in 0..segs {
            let (p00, p01, p10, p11) = (at(i, j), at(i, j + 1), at(i + 1, j), at(i + 1, j + 1));
            if i > 0 {
                tri(p00, p10, p01);
            }
            if i + 1 < rings {
                tri(p01, p10, p11);
            }
        }
    }
    Mesh { verts, tris, ..Default::default() }
}

/// A cube of `side` with a `grid` x `grid` net on every face.
fn cube(side: f64, grid: usize) -> Mesh {
    let (mut verts, mut tris) = (Vec::new(), Vec::new());
    for a in 0..3 {
        for high in [false, true] {
            let (u, v) = ((a + 1) % 3, (a + 2) % 3);
            let base = verts.len() as u32;
            for i in 0..=grid {
                for j in 0..=grid {
                    let mut q = [0.0; 3];
                    q[a] = if high { side } else { 0.0 };
                    q[u] = side * i as f64 / grid as f64;
                    q[v] = side * j as f64 / grid as f64;
                    verts.push(Point3::new(q[0], q[1], q[2]));
                }
            }
            let id = |i: usize, j: usize| base + (i * (grid + 1) + j) as u32;
            for i in 0..grid {
                for j in 0..grid {
                    for t in [[id(i, j), id(i + 1, j), id(i + 1, j + 1)], [id(i, j), id(i + 1, j + 1), id(i, j + 1)]] {
                        tris.push(if high { t } else { [t[0], t[2], t[1]] });
                    }
                }
            }
        }
    }
    Mesh { verts, tris, ..Default::default() }
}

fn volume(m: &Mesh) -> f64 {
    m.tris
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|v| m.verts[v as usize]);
            (a.x * (b.y * c.z - b.z * c.y) - a.y * (b.x * c.z - b.z * c.x) + a.z * (b.x * c.y - b.y * c.x)) / 6.0
        })
        .sum::<f64>()
        .abs()
}

/// Every side is a side of exactly two triangles.
fn closed(m: &Mesh) -> bool {
    let mut count: std::collections::HashMap<(u32, u32), u32> = Default::default();
    for t in &m.tris {
        for k in 0..3 {
            let (a, b) = (t[k].min(t[(k + 1) % 3]), t[k].max(t[(k + 1) % 3]));
            *count.entry((a, b)).or_default() += 1;
        }
    }
    count.values().all(|&c| c == 2)
}

#[test]
fn a_fine_ball_comes_back_lighter_and_on_its_surface() {
    let (r, tol) = (10.0, 0.01);
    let fine = ball(r, 120, 240);
    assert!(closed(&simplify(&fine, 0.0)), "the ball to begin with is open");
    let light = simplify(&fine, tol);
    let off = light.verts.iter().map(|p| ((p.x * p.x + p.y * p.y + p.z * p.z).sqrt() - r).abs()).fold(0.0, f64::max);
    let (v0, v1) = (volume(&fine), volume(&light));
    assert!(light.tris.len() * 3 <= fine.tris.len(), "{} triangles of {} left", light.tris.len(), fine.tris.len());
    assert!(off <= tol, "a corner stands {off} off the ball, over {tol}");
    assert!(closed(&light), "the ball came back open");
    assert!((v1 - v0).abs() < 0.005 * v0, "the ball holds {v1} against {v0}");
}

#[test]
fn a_fine_cube_comes_back_as_few_triangles() {
    let fine = cube(100.0, 40);
    let light = simplify(&fine, 0.001);
    assert!(light.tris.len() <= 100, "{} triangles of {} left on a cube", light.tris.len(), fine.tris.len());
    assert!(closed(&light), "the cube came back open");
    assert!((volume(&light) - 1.0e6).abs() < 1.0, "the cube holds {}", volume(&light));
}

#[test]
fn no_deviation_leaves_the_mesh_as_it_is() {
    let fine = cube(10.0, 4);
    let same = simplify(&fine, 0.0);
    assert_eq!(same.tris.len(), fine.tris.len());
}

/// A SEAM WHOSE CORNERS DIFFER IN THEIR LAST DIGITS STAYS CLOSED: the cube's top drawn with its corners a nanometre off
/// the sides', as an exporter leaves them. Welded bit for bit, a smooth handle kept 200 sides with no second triangle,
/// they opened as the banks collapsed apart, and the body made of it was full of holes.
#[test]
fn a_seam_off_by_the_last_digits_stays_closed() {
    let mut fine = cube(100.0, 40);
    for v in fine.verts.iter_mut().filter(|v| v.z == 100.0) {
        v.x += 1e-9;
        v.y -= 1e-9;
    }
    let light = simplify(&fine, 0.001);
    assert!(closed(&light), "the cube with its top a nanometre off came back open");
    assert!((volume(&light) - 1.0e6).abs() < 1.0, "the cube holds {}", volume(&light));
}
