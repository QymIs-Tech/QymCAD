//! Where regions meet: the corners and the edges of a mesh's faces, before any curve is fitted to them. The meshes come
//! from our kernel, so every corner and edge is known before the mesh is read: a cube has 8 corners and 12 edges 100
//! long, a washer 4 round edges and no corner, a bar with every edge rounded 24 corners and 48 edges.

use qymcad_core::geom::{Mesh, Point3};
use qymcad_kernel::Shape;
use qymcad_meshfit::{boundaries, prepare, regions, weld_tolerance, Boundaries, Prepared, Region, Surface, Tolerance};

fn edges_of(shape: &Shape, deflection: f64) -> (Boundaries, Vec<Region>, Tolerance, Prepared) {
    let qymcad_core::geom::Built { mesh, .. } = shape.tessellate(deflection).into_iter().next().expect("a body");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let b = boundaries(&p, &found);
    (b, found, tol, p)
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The area each loop of face `f` encloses, signed by how it turns about the face's outward normal: positive
/// anticlockwise seen from outside.
fn turning(b: &Boundaries, p: &Prepared, found: &[Region], f: usize) -> Vec<f64> {
    let n = found[f].tris.iter().fold([0.0; 3], |a, &t| {
        let m = p.normals[t as usize];
        [a[0] + m[0], a[1] + m[1], a[2] + m[2]]
    });
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    b.loops[f]
        .iter()
        .map(|l| {
            let mut pts: Vec<[f64; 3]> = Vec::new();
            for &(e, forward) in l {
                let mut run = b.edges[e].points.clone();
                if !forward {
                    run.reverse();
                }
                pts.extend(run);
            }
            let mut s = [0.0; 3];
            for i in 0..pts.len() {
                let (a, c) = (pts[i], pts[(i + 1) % pts.len()]);
                s[0] += a[1] * c[2] - a[2] * c[1];
                s[1] += a[2] * c[0] - a[0] * c[2];
                s[2] += a[0] * c[1] - a[1] * c[0];
            }
            (s[0] * n[0] + s[1] * n[1] + s[2] * n[2]) / (2.0 * len)
        })
        .collect()
}

/// Every edge is walked twice, once each way, by the loops of the two faces it parts; every corner lies on the
/// surface of every face whose edge ends in it.
fn well_formed(name: &str, b: &Boundaries, found: &[Region], tol: &Tolerance) {
    let mut ways = vec![[0usize; 2]; b.edges.len()];
    for loops in &b.loops {
        for l in loops {
            for &(e, forward) in l {
                ways[e][forward as usize] += 1;
            }
        }
    }
    let off: Vec<(usize, [usize; 2])> = (0..b.edges.len()).filter(|&e| ways[e] != [1, 1]).map(|e| (e, ways[e])).collect();
    assert!(off.is_empty(), "{name}: edges not walked once each way (backward, forward): {off:?}");
    for e in &b.edges {
        let Some(ends) = e.ends else { continue };
        for v in ends {
            for &f in &e.faces {
                let Some(s) = found[f as usize].surface.as_ref() else { continue };
                let d = s.distance(b.vertices[v]);
                assert!(d < tol.distance, "{name}: corner {v} at {:?} lies {d} mm off the surface of face {f}", b.vertices[v]);
            }
        }
    }
}

#[test]
fn a_cube_has_eight_corners_and_twelve_edges() {
    let cube = Shape::extrude(&[0.0, 0.0, 100.0, 0.0, 100.0, 100.0, 0.0, 100.0], 100.0).expect("a cube");
    let (b, found, tol, p) = edges_of(&cube, 0.1);
    assert_eq!(found.len(), 6, "faces of the cube");
    for f in 0..found.len() {
        let areas = turning(&b, &p, &found, f);
        assert!(areas.iter().all(|a| (a - 10_000.0).abs() < 0.01), "face {f} encloses {areas:?}, not 10000 turning anticlockwise from outside");
    }
    assert_eq!(b.vertices.len(), 8, "corners {:?}", b.vertices);
    for v in &b.vertices {
        assert!(v.iter().all(|c| c.abs() < 1e-6 || (c - 100.0).abs() < 1e-6), "a corner at {v:?}, not a corner of the cube");
    }
    assert_eq!(b.edges.len(), 12, "edges of the cube");
    for e in &b.edges {
        let [a, z] = e.ends.expect("an edge of a cube ends in corners");
        let (pa, pz) = (b.vertices[a], b.vertices[z]);
        let len = dist(pa, pz);
        assert!((len - 100.0).abs() < 0.01, "an edge {len} long, not 100");
        for &q in &e.points {
            let off = (dist(pa, q) + dist(q, pz) - len).abs();
            assert!(off < 1e-6, "a point {q:?} of the edge {pa:?} - {pz:?} lies off its straight line");
        }
    }
    for (f, loops) in b.loops.iter().enumerate() {
        assert_eq!(loops.len(), 1, "face {f} bounded by {} loops", loops.len());
        assert_eq!(loops[0].len(), 4, "face {f} has {} edges", loops[0].len());
    }
    well_formed("cube", &b, &found, &tol);
}

#[test]
fn a_washer_has_four_round_edges_and_no_corner() {
    // the profile in the XY plane, y the radius, turned about X: flat sides at x = 0 and 4, walls of radius 5 and 10
    let washer = Shape::revolve(&[0.0, 5.0, 4.0, 5.0, 4.0, 10.0, 0.0, 10.0], 0, 360.0).expect("a washer");
    let (b, found, tol, p) = edges_of(&washer, 0.01);
    assert_eq!(found.len(), 4, "faces of the washer");
    // a flat side turns anticlockwise about its outer edge and clockwise about its hole
    for f in (0..found.len()).filter(|&f| matches!(found[f].surface, Some(Surface::Plane { .. }))) {
        let mut areas = turning(&b, &p, &found, f);
        areas.sort_by(f64::total_cmp);
        let (hole, outer) = (std::f64::consts::PI * 25.0, std::f64::consts::PI * 100.0);
        assert!(areas.len() == 2 && (areas[0] + hole).abs() < 0.01 * hole && (areas[1] - outer).abs() < 0.01 * outer, "the flat side {f} encloses {areas:?}, not -{hole:.1} and {outer:.1}");
    }
    assert!(b.vertices.is_empty(), "corners {:?} on a washer", b.vertices);
    assert_eq!(b.edges.len(), 4, "edges of the washer");
    for e in &b.edges {
        assert!(e.ends.is_none(), "a round edge of a washer with ends {:?}", e.ends);
        let x = e.points[0][0];
        let r = e.points[0][1].hypot(e.points[0][2]);
        assert!(x.abs() < 1e-6 || (x - 4.0).abs() < 1e-6, "a round edge at x = {x}");
        assert!((r - 5.0).abs() < 0.01 || (r - 10.0).abs() < 0.01, "a round edge of radius {r}");
        for q in &e.points {
            assert!((q[0] - x).abs() < 1e-6 && (q[1].hypot(q[2]) - r).abs() < 0.01, "a point {q:?} off the circle x = {x}, r = {r}");
        }
    }
    let mut rings: Vec<usize> = b.loops.iter().map(|l| l.len()).collect();
    rings.sort();
    assert_eq!(rings, vec![2, 2, 2, 2], "every face of a washer is a ring between two round edges");
    well_formed("washer", &b, &found, &tol);
}

#[test]
fn a_rounded_bar_has_its_corners_where_the_roundings_meet() {
    let bar = Shape::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 20.0, 0.0, 20.0], 10.0).expect("a bar").fillet_all(2.0).expect("filleted");
    for deflection in [0.1, 0.02] {
        let (b, found, tol, _) = edges_of(&bar, deflection);
        assert_eq!(found.len(), 26, "faces of the rounded bar at {deflection}");
        assert_eq!((b.vertices.len(), b.edges.len()), (24, 48), "corners and edges of the rounded bar at {deflection}");
        assert!(b.loops.iter().all(|l| l.len() == 1), "a face of the rounded bar at {deflection} with a hole: {:?}", b.loops.iter().map(|l| l.len()).collect::<Vec<_>>());
        well_formed(&format!("rounded bar at {deflection}"), &b, &found, &tol);
    }
}

/// A CORNER THE MESH PUTS OFF ITS FACES STANDS WHERE THE FACES MEET. A cube of 100 drawn with a 10 x 10 grid on every
/// face and its eight corners pushed 0.001 mm out along the diagonal - within the tolerance, the way a text STL rounds
/// them: every corner found lies within 0.0001 of the true one, where the mesh put it 0.00058 off along every axis.
#[test]
fn a_corner_off_its_faces_is_placed_where_they_meet() {
    let (grid, side, push) = (10usize, 100.0, 1e-3);
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
                    if (i == 0 || i == grid) && (j == 0 || j == grid) {
                        for c in &mut q {
                            let out = if *c > side / 2.0 { 1.0 } else { -1.0 };
                            *c += out * push / 3f64.sqrt();
                        }
                    }
                    verts.push(Point3::new(q[0], q[1], q[2]));
                }
            }
            let at = |i: usize, j: usize| base + (i * (grid + 1) + j) as u32;
            for i in 0..grid {
                for j in 0..grid {
                    tris.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
                    tris.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
                }
            }
        }
    }
    let mesh = Mesh { verts, tris, ..Default::default() };
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    assert_eq!(found.len(), 6, "faces of the pushed cube");
    let b = boundaries(&p, &found);
    assert_eq!((b.vertices.len(), b.edges.len()), (8, 12), "corners and edges of the pushed cube");
    for v in &b.vertices {
        let off = v.iter().map(|c| c.abs().min((c - side).abs())).fold(0.0, f64::max);
        assert!(off < push / 10.0, "a corner at {v:?} stands {off} mm off the true one; the mesh put it {} out", push / 3f64.sqrt());
    }
    well_formed("pushed cube", &b, &found, &tol);
}
