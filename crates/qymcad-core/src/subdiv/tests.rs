//! Subdivision is checked by properties rather than against reference numbers.
//!
//! Comparing against a recorded array of coordinates is pointless: such a test goes red on any edit and
//! explains nothing. What is checked instead are the things that have to hold for any implementation of
//! Catmull-Clark, and each of them catches its own class of error.
use super::*;

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Face and vertex counts, exactly by the formula rather than approximately.
///
/// An n-gon yields n quadrilaterals, and the vertex count becomes the old one plus one per face and one per
/// edge. An error in the connectivity — a lost edge, a face counted twice — shows up here first and is
/// explained by a number.
#[test]
fn one_step_multiplies_the_cage_exactly_as_the_rule_says() {
    for cage in [Cage::cube(10.0), Cage::grid(3, 2, 30.0, 20.0)] {
        let corners: usize = cage.faces.iter().map(|f| f.len()).sum();
        let edges = {
            let mut e = std::collections::HashSet::new();
            for f in &cage.faces {
                for (k, &a) in f.iter().enumerate() {
                    e.insert(edge_key(a, f[(k + 1) % f.len()]));
                }
            }
            e.len()
        };
        let s = cage.subdivide();
        assert_eq!(s.faces.len(), corners, "every corner of an original face has to yield one quadrilateral");
        assert!(s.faces.iter().all(|f| f.len() == 4), "after one step the cage has to be entirely quadrilateral");
        assert_eq!(s.verts.len(), cage.verts.len() + cage.faces.len() + edges, "vertices: the old ones plus one per face plus one per edge");
    }
}

/// A closed cage stays closed.
///
/// A hole in the result would mean the new faces are stitched wrongly, and that is the kind of error a picture
/// does not show until it is held up to the light.
#[test]
fn a_closed_cage_stays_closed() {
    let mut c = Cage::cube(10.0);
    assert!(c.is_closed(), "the cube is closed");
    for step in 1..=3 {
        c = c.subdivide();
        assert!(c.is_closed(), "after step {step} the cage came apart at the seams");
    }
}

/// An open cage does not shrink.
///
/// The main trap in the boundary rules: without them the border pulls inwards on every step and the patch
/// visibly shrinks. The bounding box is what gets checked, and it has to stay as it was, because the corners of
/// the border stay put and the border itself behaves like a curve.
#[test]
fn an_open_patch_keeps_its_size_instead_of_shrinking() {
    let g = Cage::grid(2, 2, 40.0, 30.0);
    let (lo0, hi0) = g.bounds();
    let s = g.subdivided(3);
    let (lo, hi) = s.bounds();
    for k in 0..2 {
        assert!((lo[k] - lo0[k]).abs() < 1e-9, "the border moved along axis {k}: {} against {}", lo[k], lo0[k]);
        assert!((hi[k] - hi0[k]).abs() < 1e-9, "the border moved along axis {k}: {} against {}", hi[k], hi0[k]);
    }
}

/// What is flat stays flat.
///
/// A planar mesh has no right to bulge: subdivision smooths but does not invent curvature.
#[test]
fn a_flat_grid_never_bulges() {
    let s = Cage::grid(3, 3, 30.0, 30.0).subdivided(3);
    let worst = s.verts.iter().map(|v| v[2].abs()).fold(0.0, f64::max);
    assert!(worst < 1e-12, "the planar mesh bulged by {worst}");
    let limit = s.limit_points();
    let worst = limit.iter().map(|v| v[2].abs()).fold(0.0, f64::max);
    assert!(worst < 1e-12, "the limit points of a planar mesh left the plane by {worst}");
}

/// A cube corner settles at exactly half, which is derived on paper rather than fitted.
///
/// At valence 3 one step gives `5h/9` and the limit is exactly `0.5h`. An earlier version of the mask took the
/// original neighbours instead of the subdivided ones and produced `0.75h`; the discrepancy was 2.17 mm, and
/// what caught it was a comparison against deep subdivision rather than the eye.
///
/// The test asserts an exact number rather than a range: where the answer is known analytically, a tolerance is
/// a way of not noticing an error.
#[test]
fn a_cube_corner_lands_exactly_halfway() {
    let h = 5.0;
    let limit = Cage::cube(2.0 * h).limit_points();
    for (vi, p) in limit.iter().enumerate() {
        for k in 0..3 {
            assert!((p[k].abs() - 0.5 * h).abs() < 1e-12, "corner {vi} along axis {k}: {} instead of {}", p[k].abs(), 0.5 * h);
        }
    }
}

/// The volume decreases towards the limit and stays meaningful.
///
/// An exact number cannot be named here without a derivation on paper, so properties are checked instead: the
/// volume falls monotonically, meaning the scheme smooths rather than oscillates, and stays within the bounds
/// beyond which the shape stops being a rounded cube — neither deflated into a ball nor left as a cube.
#[test]
fn the_volume_falls_towards_a_limit_and_stays_sane() {
    let side = 10.0;
    let v0 = side * side * side;
    let mut c = Cage::cube(side);
    let mut prev = c.volume();
    for _ in 0..5 {
        c = c.subdivide();
        let v = c.volume();
        assert!(v < prev + 1e-9, "the volume has to decrease towards the limit, but it grew: {prev} -> {v}");
        prev = v;
    }
    let ratio = prev / v0;
    assert!((0.25..0.45).contains(&ratio), "the rounded cube stopped making sense: {ratio:.3} of the volume is left");
}

/// The steps converge rather than wander.
///
/// Each successive step has to move the shape less than the previous one. A scheme that oscillates gives itself
/// away here even when every individual step looks plausible.
#[test]
fn each_step_moves_the_shape_less_than_the_one_before() {
    let mut c = Cage::cube(10.0);
    let mut prev_delta = f64::MAX;
    for step in 1..=4 {
        let before = c.volume();
        c = c.subdivide();
        let delta = (before - c.volume()).abs();
        assert!(delta < prev_delta, "step {step} moved the shape more than the previous one: {delta} against {prev_delta}");
        prev_delta = delta;
    }
}

/// A limit point really is the limit.
///
/// The limit is computed directly from the mask and compared against where the vertex arrives after many steps.
/// Agreement means the mask is right; a discrepancy means either it or the subdivision itself is wrong, and
/// then the anchors of the cage are off by a constant that no number of steps reduces.
#[test]
fn the_limit_mask_agrees_with_where_subdivision_actually_goes() {
    // The cages differ, and not for the sake of coverage. A cube is symmetric enough that a wrong mask gives
    // the right answer on it by coincidence, which is where the first error hid. A skewed cage catches that
    // one, and a tetrahedron catches the second: a triangle has no opposite corner, and without a subdivision
    // step the mask would measure the wrong thing.
    let mut skewed = Cage::cube(10.0);
    skewed.verts[6] = [9.0, 4.0, 7.5];
    skewed.verts[3] = [-6.0, 6.5, -4.0];
    let tetra = Cage {
        verts: vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [5.0, 9.0, 0.0], [5.0, 3.0, 8.0]],
        faces: vec![vec![0, 2, 1], vec![0, 1, 3], vec![1, 2, 3], vec![2, 0, 3]],
    };

    for (what, cage) in [("cube", Cage::cube(10.0)), ("skewed cage", skewed), ("tetrahedron", tetra)] {
        let limit = cage.limit_points();
        // the original vertices keep their indices: subdivision puts them first
        let mut c = cage.clone();
        for _ in 0..8 {
            c = c.subdivide();
        }
        for vi in 0..cage.verts.len() {
            let d = dist(limit[vi], c.verts[vi]);
            assert!(d < 1e-3, "{what}: the limit mask disagrees with subdivision at vertex {vi} by {d:.6} mm");
        }
    }
}

/// The limit does not depend on how many times the cage has already been subdivided.
///
/// A property of the notion of a limit itself, and the same property explains why the step inside the mask is
/// harmless for quadrilateral cages: it exists to define the opposite corner, not to produce the answer.
#[test]
fn the_limit_is_the_same_whichever_cage_you_ask() {
    let mut cage = Cage::cube(10.0);
    cage.verts[6] = [9.0, 4.0, 7.5];
    let a = cage.limit_points();
    let b = cage.subdivide().limit_points();
    for vi in 0..cage.verts.len() {
        let d = dist(a[vi], b[vi]);
        assert!(d < 1e-9, "the limit of vertex {vi} changed by {d:.3e} under an extra step, so it is not a limit");
    }
}

/// Symmetry does not break.
///
/// A cube is symmetric about all three planes. If the masks confuse the order of the neighbours or lose a face,
/// the symmetry drifts apart — and to the eye that merely looks slightly off.
#[test]
fn a_symmetric_cage_stays_symmetric() {
    let c = Cage::cube(10.0).subdivided(3);
    let has = |p: [f64; 3]| c.verts.iter().any(|q| dist(*q, p) < 1e-9);
    for v in &c.verts {
        for mirror in [[-v[0], v[1], v[2]], [v[0], -v[1], v[2]], [v[0], v[1], -v[2]]] {
            assert!(has(mirror), "there is no mirror vertex for {v:?}, so the symmetry has drifted apart");
        }
    }
}

/// The limit lies inside the original bounding box.
///
/// The convex hull property: the surface has no right to leave the cage. A violation means an error in the
/// weights, and that is exactly how a typo in a denominator shows itself.
#[test]
fn the_surface_never_escapes_its_cage() {
    for cage in [Cage::cube(10.0), Cage::grid(2, 3, 20.0, 30.0)] {
        let (lo, hi) = cage.bounds();
        for p in cage.limit_points().iter().chain(cage.subdivided(3).verts.iter()) {
            for k in 0..3 {
                assert!(p[k] >= lo[k] - 1e-9 && p[k] <= hi[k] + 1e-9, "point {p:?} left the bounding box of the cage along axis {k}");
            }
        }
    }
}

/// Triangles and pentagons work too.
///
/// The first step has to turn any cage into a quadrilateral one. Without that there is no promising that a cage
/// may be drawn in any shape at all — and it will be.
#[test]
fn a_cage_of_odd_polygons_becomes_quads_after_one_step() {
    // a tetrahedron: four triangles
    let cage = Cage {
        verts: vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [5.0, 9.0, 0.0], [5.0, 3.0, 8.0]],
        faces: vec![vec![0, 2, 1], vec![0, 1, 3], vec![1, 2, 3], vec![2, 0, 3]],
    };
    assert!(cage.is_closed(), "the tetrahedron is closed");
    let s = cage.subdivide();
    assert_eq!(s.faces.len(), 12, "four triangles give twelve quadrilaterals");
    assert!(s.faces.iter().all(|f| f.len() == 4));
    assert!(s.is_closed(), "and it stays closed");
}

/// The valence of the original vertices does not change.
///
/// This is what makes the whole NURBS undertaking hard: vertices whose valence is not four, the extraordinary
/// ones, stay extraordinary forever however far the cage is subdivided. The test pins down the fact that patch
/// extraction rests on: after the first step every new vertex is regular and only the original ones can be
/// extraordinary.
#[test]
fn extraordinary_vertices_stay_and_do_not_multiply() {
    let cage = Cage::cube(10.0); // all eight vertices of a cube have valence 3 and are extraordinary
    let odd0 = (0..cage.verts.len()).filter(|&v| cage.valence(v) != 4).count();
    assert_eq!(odd0, 8, "a cubic cage has eight extraordinary vertices");

    let mut c = cage.clone();
    for step in 1..=3 {
        c = c.subdivide();
        let odd = (0..c.verts.len()).filter(|&v| c.valence(v) != 4).count();
        assert_eq!(odd, odd0, "after step {step} there are {odd} extraordinary vertices instead of {odd0}; their number may only stay the same");
    }
}

/// A patch equals the limit rather than approximating it.
///
/// Over a regular face a bicubic patch has to be equal to the Catmull-Clark limit surface; that is a property
/// of the scheme, not luck. The corners of the patch are compared against the limit points of the cage: a
/// discrepancy means the conversion is wrong and there is nothing to build the rest on.
#[test]
fn a_patch_matches_the_limit_surface_exactly() {
    let cage = Cage::cube(10.0);
    let refined = cage.subdivided(3);
    let limit = refined.limit_points();
    let mut checked = 0;
    for fi in 0..refined.faces.len() {
        let Some(p) = refined.patch_of_face_for_test(fi) else { continue };
        let f = &refined.faces[fi];
        // the corners of the Bezier patch at (0,0), (0,1), (1,1) and (1,0) are the limit points of the four
        // corners of the face
        for (uv, vi) in [((0.0, 0.0), f[0]), ((0.0, 1.0), f[3]), ((1.0, 1.0), f[2]), ((1.0, 0.0), f[1])] {
            let d = dist(p.eval(uv.0, uv.1), limit[vi as usize]);
            assert!(d < 1e-9, "a corner of the patch of face {fi} disagrees with the limit by {d:.3e} mm");
        }
        checked += 1;
    }
    assert!(checked > 100, "there was hardly anything to check: {checked} regular faces");
}

/// Neighbouring patches meet along their seam.
///
/// If adjacent patches diverge along a shared edge, the kernel stitches them with a gap or fails to stitch them
/// at all. The check samples points along the shared edge rather than only its corners: a divergence in the
/// middle of a seam is invisible from the corners.
#[test]
fn neighbouring_patches_meet_along_their_seam() {
    let refined = Cage::cube(10.0).subdivided(3);
    let mut seams = 0;
    for fi in 0..refined.faces.len() {
        let Some(p) = refined.patch_of_face_for_test(fi) else { continue };
        let f = refined.faces[fi].clone();
        for (k, &a) in f.iter().enumerate() {
            let b = f[(k + 1) % 4];
            let Some(nj) = refined.face_across_for_test(a, b, fi) else { continue };
            let Some(q) = refined.patch_of_face_for_test(nj) else { continue };
            // take the seam points of both patches and compare them as sets
            let edge_pts = |patch: &super::patch::BezierPatch, ea: u32, eb: u32| -> Vec<[f64; 3]> {
                let g = &refined.faces[if std::ptr::eq(patch, patch) { 0 } else { 0 }];
                let _ = g;
                let _ = (ea, eb);
                (0..=8).map(|i| i as f64 / 8.0).map(|t| patch.eval(t, 0.0)).collect()
            };
            let _ = edge_pts;
            // the direct way: on both patches find the side whose ends coincide with the ends of the shared
            // edge
            let side = |patch: &super::patch::BezierPatch| -> Option<Vec<[f64; 3]>> {
                let ends = [
                    ((0.0, 0.0), (1.0, 0.0)),
                    ((1.0, 0.0), (1.0, 1.0)),
                    ((1.0, 1.0), (0.0, 1.0)),
                    ((0.0, 1.0), (0.0, 0.0)),
                ];
                let (la, lb) = (refined.limit_points()[a as usize], refined.limit_points()[b as usize]);
                for (s, e) in ends {
                    let (ps, pe) = (patch.eval(s.0, s.1), patch.eval(e.0, e.1));
                    if (dist(ps, la) < 1e-9 && dist(pe, lb) < 1e-9) || (dist(ps, lb) < 1e-9 && dist(pe, la) < 1e-9) {
                        let fwd = dist(ps, la) < 1e-9;
                        return Some(
                            (0..=8)
                                .map(|i| {
                                    let t = if fwd { i as f64 / 8.0 } else { 1.0 - i as f64 / 8.0 };
                                    patch.eval(s.0 + (e.0 - s.0) * t, s.1 + (e.1 - s.1) * t)
                                })
                                .collect(),
                        );
                    }
                }
                None
            };
            if let (Some(sp), Some(sq)) = (side(&p), side(&q)) {
                for (x, y) in sp.iter().zip(sq.iter()) {
                    let d = dist(*x, *y);
                    assert!(d < 1e-9, "patches {fi} and {nj} diverge along their seam by {d:.3e} mm");
                }
                seams += 1;
            }
        }
    }
    assert!(seams > 50, "only {seams} seams were checked, so the test saw almost nothing");
}

/// The cost of the approximation is stated as a number.
///
/// Extraordinary vertices do not go away, and the faces around them have no exact patch. Every subdivision step
/// reduces their share fourfold. The test pins that down with a number, so that "almost everything is
/// converted" stops being a phrase.
#[test]
fn the_cost_of_extraordinary_points_is_counted_not_hidden() {
    let cage = Cage::cube(10.0); // eight extraordinary vertices, all of them corners

    // One step does not help a cube at all, and that is worth knowing: it leaves 24 faces, each touching one
    // of the eight corners, so there are no exact patches whatsoever. Hence the requirement of two or three
    // steps.
    let first = cage.to_bezier_patches(1);
    assert_eq!(first.patches.len(), 0, "after one step a cube can have no exact patches");

    let mut prev = 1.1;
    for refine in 1..=4 {
        let set = cage.to_bezier_patches(refine);
        assert_eq!(set.irregular, 8 * 3, "every extraordinary vertex has exactly three faces without an exact patch");
        assert!(set.irregular_share < prev, "the share of unconverted faces has to fall: {} against {}", set.irregular_share, prev);
        prev = set.irregular_share;
    }
    let set = cage.to_bezier_patches(4);
    assert!(set.irregular_share < 0.02, "after four steps {:.1}% remains unconverted", set.irregular_share * 100.0);
    assert!(set.patches.len() > 1000, "only {} patches came out", set.patches.len());
}

/// A torus is a cage without extraordinary vertices, and that has to be proven rather than declared.
///
/// It is where the conversion into patches is checked cleanly: if even one vertex turns out not to have valence
/// four, a face without an exact patch appears, and "the shell closed without holes" stops meaning anything.
#[test]
fn a_torus_cage_has_no_extraordinary_points_at_all() {
    let t = Cage::torus(16, 8, 30.0, 10.0);
    assert!(t.is_closed(), "the torus is closed");
    let odd = (0..t.verts.len()).filter(|&v| t.valence(v) != 4).count();
    assert_eq!(odd, 0, "a torus must have no extraordinary vertices at all, yet there are {odd}");
    let set = t.to_bezier_patches(1);
    assert_eq!(set.irregular, 0, "on a torus every face has an exact patch");
    assert_eq!(set.patches.len(), t.faces.len() * 4, "after one step there are four times as many faces, and as many patches");
}
