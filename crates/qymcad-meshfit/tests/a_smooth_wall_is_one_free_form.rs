//! A SMOOTH WALL IS ONE FREE FORM: a bicubic B-spline surface fitted to the points of the whole wall, every point within
//! the tolerance, with a modest net of poles - not a mosaic of primitives lying on it strip by strip (a smooth handle
//! came back as 1 887 strips of cylinders). A wall that turns past a right angle does not lie flat and is refused: it has
//! to be cut first.
use qymcad_meshfit::bspline::fit;

/// Points on `z = f(x, y)` over a square of half-side `h`, `n` x `n` of them, with the normals of the surface.
fn sampled(h: f64, n: usize, f: impl Fn(f64, f64) -> f64) -> (Vec<[f64; 3]>, Vec<[f64; 3]>) {
    let (mut pts, mut nrm) = (Vec::new(), Vec::new());
    let e = 1e-5;
    for i in 0..n {
        for j in 0..n {
            let (x, y) = (-h + 2.0 * h * i as f64 / (n - 1) as f64, -h + 2.0 * h * j as f64 / (n - 1) as f64);
            let (fx, fy) = ((f(x + e, y) - f(x - e, y)) / (2.0 * e), (f(x, y + e) - f(x, y - e)) / (2.0 * e));
            let l = (fx * fx + fy * fy + 1.0).sqrt();
            pts.push([x, y, f(x, y)]);
            nrm.push([-fx / l, -fy / l, 1.0 / l]);
        }
    }
    (pts, nrm)
}

#[test]
fn a_smooth_wall_is_fitted_by_one_surface_within_the_tolerance() {
    let tol = 1e-3;
    for (name, (pts, nrm)) in [("a saddle", sampled(20.0, 60, |x, y| 0.01 * (x * x - y * y))), ("a wave", sampled(30.0, 70, |x, y| 2.0 * (x / 8.0).sin() * (y / 10.0).cos()))] {
        let fitted = fit(&pts, &nrm, tol).unwrap_or_else(|| panic!("{name}: no free form holds its points within {tol}"));
        assert!(fitted.worst <= tol, "{name}: a point stands {} off the surface", fitted.worst);
        let poles = fitted.surface.poles.len();
        assert!(poles <= 441, "{name}: {poles} poles - not one smooth wall but a mosaic of spans");
    }
}

#[test]
fn a_wall_turning_past_a_right_angle_is_refused() {
    // a half ball: its rim faces sideways, square to its top
    let (mut pts, mut nrm) = (Vec::new(), Vec::new());
    for i in 0..=20 {
        for j in 0..40 {
            let (t, a) = (std::f64::consts::FRAC_PI_2 * i as f64 / 20.0, std::f64::consts::TAU * j as f64 / 40.0);
            let n = [t.sin() * a.cos(), t.sin() * a.sin(), t.cos()];
            pts.push([10.0 * n[0], 10.0 * n[1], 10.0 * n[2]]);
            nrm.push(n);
        }
    }
    assert!(fit(&pts, &nrm, 1e-3).is_none(), "a half ball was laid flat on one plane - its rim folds over itself there");
}

/// A FITTED WALL MAKES A FACE OF ITS OWN AREA: the saddle fitted by one surface and bounded by its four sides, laid on
/// the surface by the rule the wall was fitted with, takes the area the saddle has over its square - not the far side of
/// the surface where it runs on past the wall (measured on a smooth handle: faces of 2 299 mm^2 on walls of 99).
#[test]
fn a_fitted_wall_makes_a_face_of_its_own_area() {
    use qymcad_kernel::{BoundedFace, EdgeCurve, FaceEdge, FaceSurface, Shape};
    let (h, n) = (20.0, 41);
    let f = |x: f64, y: f64| 0.01 * (x * x - y * y);
    let (pts, nrm) = sampled(h, n, f);
    let wall = fit(&pts, &nrm, 1e-3).expect("the saddle is fitted");
    // the area of the saddle over the square, summed over a fine grid of its own
    let m = 400;
    let at = |i: usize, j: usize| {
        let (x, y) = (-h + 2.0 * h * i as f64 / m as f64, -h + 2.0 * h * j as f64 / m as f64);
        [x, y, f(x, y)]
    };
    let tri = |a: [f64; 3], b: [f64; 3], c: [f64; 3]| {
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let w = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt() / 2.0
    };
    let mut want = 0.0;
    for i in 0..m {
        for j in 0..m {
            want += tri(at(i, j), at(i + 1, j), at(i + 1, j + 1)) + tri(at(i, j), at(i + 1, j + 1), at(i, j + 1));
        }
    }
    // the four sides, walked anticlockwise seen from above (the saddle faces up), each a polyline of the saddle's points
    let side = |from: (f64, f64), to: (f64, f64)| -> Vec<[f64; 3]> {
        (0..=40)
            .map(|k| {
                let t = k as f64 / 40.0;
                let (x, y) = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
                [x, y, f(x, y)]
            })
            .collect()
    };
    let c = [(-h, -h), (h, -h), (h, h), (-h, h)];
    let corners: Vec<[f64; 3]> = c.iter().map(|&(x, y)| [x, y, f(x, y)]).collect();
    let edges: Vec<FaceEdge> = (0..4).map(|k| FaceEdge { curve: EdgeCurve::Polyline, ends: Some([k, (k + 1) % 4]), points: side(c[k], c[(k + 1) % 4]) }).collect();
    let face = BoundedFace {
        surface: FaceSurface::Spline { nu: wall.surface.nu, nv: wall.surface.nv, frame: wall.surface.frame },
        outward: true,
        inside: [0.0, 0.0, 0.0],
        loops: vec![vec![(0, true), (1, true), (2, true), (3, true)]],
        points: wall.surface.poles.clone(),
    };
    let took = Shape::face_areas(&corners, &edges, &[face], 1e-3).expect("the face is measured")[0];
    assert!(took.is_some_and(|a| (a - want).abs() < 1e-3 * want), "the fitted wall took {took:?} mm^2 of a saddle of {want}");
}

/// A side of the test body: its net of points by row and column, and the way it looks out.
struct Side<'a> {
    at: Box<dyn Fn(usize, usize) -> [f64; 3] + 'a>,
    out: [f64; 3],
}

fn side<'a>(at: Box<dyn Fn(usize, usize) -> [f64; 3] + 'a>, out: [f64; 3]) -> Side<'a> {
    Side { at, out }
}

/// A box of 40 x 40 standing on z = 0, its top the vault of an ellipse of half-axes 24 across and 20 up, run along y -
/// a smooth wall on which no plane, cylinder, cone, sphere or torus lies, its bend drifting slowly from its crown to its
/// eaves, as the walls of a handle do - every face drawn with a `grid` x `grid` net. Returns the mesh and the volume it
/// holds.
fn vaulted_box(grid: usize) -> (qymcad_core::geom::Mesh, f64) {
    use qymcad_core::geom::{Mesh, Point3};
    let h = 20.0;
    let top = |x: f64, _y: f64| 5.0 + 20.0 * (1.0 - (x / 24.0).powi(2)).sqrt();
    let s = |i: usize| -h + 2.0 * h * i as f64 / grid as f64;
    let f = |j: usize| j as f64 / grid as f64;
    // each face as a net of points, and which way it looks out
    let faces: Vec<Side> = vec![
        side(Box::new(move |i, j| [s(i), s(j), top(s(i), s(j))]), [0.0, 0.0, 1.0]),
        side(Box::new(move |i, j| [s(i), s(j), 0.0]), [0.0, 0.0, -1.0]),
        side(Box::new(move |i, j| [s(i), -h, top(s(i), -h) * f(j)]), [0.0, -1.0, 0.0]),
        side(Box::new(move |i, j| [s(i), h, top(s(i), h) * f(j)]), [0.0, 1.0, 0.0]),
        side(Box::new(move |i, j| [-h, s(i), top(-h, s(i)) * f(j)]), [-1.0, 0.0, 0.0]),
        side(Box::new(move |i, j| [h, s(i), top(h, s(i)) * f(j)]), [1.0, 0.0, 0.0]),
    ];
    let (mut verts, mut tris) = (Vec::new(), Vec::new());
    for Side { at, out } in &faces {
        let base = verts.len() as u32;
        for i in 0..=grid {
            for j in 0..=grid {
                let q = at(i, j);
                verts.push(Point3::new(q[0], q[1], q[2]));
            }
        }
        // the net's own turn, against the way the face looks out
        let (a, b, c) = (at(0, 0), at(1, 0), at(0, 1));
        let n = [
            (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1]),
            (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]),
            (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]),
        ];
        let flip = n[0] * out[0] + n[1] * out[1] + n[2] * out[2] < 0.0;
        let id = |i: usize, j: usize| base + (i * (grid + 1) + j) as u32;
        for i in 0..grid {
            for j in 0..grid {
                for t in [[id(i, j), id(i + 1, j), id(i + 1, j + 1)], [id(i, j), id(i + 1, j + 1), id(i, j + 1)]] {
                    tris.push(if flip { [t[0], t[2], t[1]] } else { t });
                }
            }
        }
    }
    let mesh = Mesh { verts, tris, ..Default::default() };
    let volume = mesh
        .tris
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|v| mesh.verts[v as usize]);
            (a.x * (b.y * c.z - b.z * c.y) - a.y * (b.x * c.z - b.z * c.x) + a.z * (b.x * c.y - b.y * c.x)) / 6.0
        })
        .sum();
    (mesh, volume)
}

/// A VAULT IS ONE REGION ON ONE SURFACE: the triangles of the box's top all go to one fitted wall, not to strips of the
/// cylinders that lie along it a little way each.
#[test]
fn a_vault_is_one_region_on_one_fitted_wall() {
    use qymcad_meshfit::{prepare, regions, weld_tolerance, Surface, Tolerance};
    let (mesh, _) = vaulted_box(60);
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let on_top: Vec<usize> = (0..found.len()).filter(|&r| found[r].tris.iter().any(|&t| p.normals[t as usize][2] > 0.5)).collect();
    let kinds: Vec<String> = on_top.iter().map(|&r| format!("{:?}", found[r].surface.as_ref().map(|s| format!("{s:?}").chars().take(8).collect::<String>()))).collect();
    assert!(on_top.len() == 1 && matches!(found[on_top[0]].surface, Some(Surface::Spline(_))), "the vault came back as {} regions: {kinds:?}", on_top.len());
}

/// The body recognised on `found`: sound, a solid, its faces and its volume.
fn body_of(p: &qymcad_meshfit::Prepared, found: &[qymcad_meshfit::Region], tol: &qymcad_meshfit::Tolerance) -> (bool, bool, usize, f64, usize) {
    use qymcad_meshfit::{boundaries, curves};
    let b = boundaries(p, found);
    let made = qymcad_kernel::recognise::recognise(p, found, &b, &curves(&b, found, tol), tol).unwrap_or_else(|| panic!("no body - {:?}", qymcad_kernel::last_kernel_refusal()));
    let faces = made.shape.face_kinds().map(|k| k.iter().sum::<u32>() as usize).unwrap_or(0);
    (made.shape.is_valid(), !made.shape.is_sheet(), faces, made.shape.volume(), made.free_edges)
}

/// A VAULTED BOX COMES BACK A SOUND SOLID OF ITS OWN VOLUME in its six faces: the vault one fitted wall.
#[test]
fn a_vaulted_box_becomes_a_sound_body_of_six_faces() {
    use qymcad_meshfit::{prepare, regions, weld_tolerance, Tolerance};
    let (mesh, want) = vaulted_box(60);
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let (sound, solid, faces, volume, free) = body_of(&p, &found, &tol);
    assert!(
        sound && solid && faces == 6 && free == 0 && (volume - want).abs() < 1e-3 * want,
        "the vaulted box: sound {sound}, solid {solid}, {faces} faces, {free} sides free, holding {volume} against {want}"
    );
}

/// A FITTED WALL BESIDE REGIONS LEFT AS MESH STILL MAKES A SOUND BODY: the vault stays a face while the four sides
/// go in as their triangles. The sides meet the wall along its own points, and the wall keeps the curves it was laid on
/// its surface with - sewing them anew by a search of the surface left loops crossing themselves on 12 walls of a smooth
/// handle, and the body was refused.
#[test]
fn a_fitted_wall_beside_the_mesh_makes_a_sound_body() {
    use qymcad_meshfit::{prepare, regions, weld_tolerance, Surface, Tolerance};
    let (mesh, want) = vaulted_box(60);
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let mut found = regions(&p, &tol);
    for r in &mut found {
        let side = r.tris.iter().all(|&t| p.normals[t as usize][2].abs() < 0.5);
        if side {
            r.surface = None;
        }
    }
    assert!(found.iter().any(|r| matches!(r.surface, Some(Surface::Spline(_)))), "the top is a fitted wall");
    let (sound, solid, faces, volume, free) = body_of(&p, &found, &tol);
    assert!(
        sound && solid && free == 0 && (volume - want).abs() < 1e-3 * want,
        "the vaulted box with its sides as mesh: sound {sound}, solid {solid}, {faces} faces, {free} sides free, holding {volume} against {want}"
    );
}
