//! A mesh recognised into a body: every face on its surface, bounded by exact edges, sewn and closed. The meshes come
//! from our kernel, so the body they were made from is known: the recognised one holds its volume and its faces.

use qymcad_kernel::recognise::recognise as solid;
use qymcad_kernel::{last_kernel_refusal, Shape};
use qymcad_meshfit::{boundaries, curves, prepare, regions, weld_tolerance, Curve, Tolerance};
use qymcad_kernel::specimen::Specimen as _;

/// The body recognised on the tessellation of `shape`: its volume, how many faces were built, how many it has.
fn recognised(name: &str, shape: &Shape, deflection: f64) -> (f64, usize, usize, bool) {
    let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(shape, deflection).into_iter().next().expect("a body");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let b = boundaries(&p, &found);
    let c = curves(&b, &found, &tol);
    let made = solid(&p, &found, &b, &c, &tol).unwrap_or_else(|| panic!("{name}: no body - {:?}", last_kernel_refusal()));
    let (body, built) = (made.shape, made.areas.iter().flatten().count());
    let faces = qymcad_kernel::specimen::mesh(&body, deflection).into_iter().next().map(|qymcad_core::geom::Built { faces: f, .. }| f.len()).unwrap_or(0);
    (body.volume(), built, faces, body.is_sheet())
}

#[test]
fn a_cube_mesh_becomes_the_cube() {
    let cube = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 100.0, 0.0, 100.0, 100.0, 0.0, 100.0], 100.0).expect("a cube");
    let (volume, built, faces, sheet) = recognised("cube", &cube, 0.1);
    assert_eq!((built, faces), (6, 6), "faces built and faces of the body");
    assert!(!sheet, "the cube came back a sheet, not a solid");
    assert!((volume - 1.0e6).abs() < 1.0, "the cube holds {volume}, not 1000000");
}

#[test]
fn a_washer_mesh_becomes_the_washer() {
    let washer = qymcad_kernel::specimen::revolve(&[0.0, 5.0, 4.0, 5.0, 4.0, 10.0, 0.0, 10.0], 0, 360.0).expect("a washer");
    let want = washer.volume();
    let (volume, built, faces, sheet) = recognised("washer", &washer, 0.01);
    assert_eq!((built, faces), (4, 4), "faces built and faces of the body");
    // EACH FACE TOOK ITS OWN RING, not the disc with the hole added: the flat rings took 392.7 mm^2 for 235.6 while the
    // point they were checked against stood in their hole
    let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(&washer, 0.01).into_iter().next().expect("a body");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let b = boundaries(&p, &found);
    let made = solid(&p, &found, &b, &curves(&b, &found, &tol), &tol).expect("the washer");
    for (r, took) in made.areas.iter().enumerate() {
        let own: f64 = found[r]
            .tris
            .iter()
            .map(|&t| {
                let [a, b, c] = p.mesh.triangle(t as usize);
                let (u, w) = ([b.x - a.x, b.y - a.y, b.z - a.z], [c.x - a.x, c.y - a.y, c.z - a.z]);
                0.5 * ((u[1] * w[2] - u[2] * w[1]).powi(2) + (u[2] * w[0] - u[0] * w[2]).powi(2) + (u[0] * w[1] - u[1] * w[0]).powi(2)).sqrt()
            })
            .sum();
        assert!(took.is_some_and(|t| (t / own - 1.0).abs() < 0.1), "the face of region {r} took {took:?} of a region of {own:.1} mm^2");
    }
    assert!(!sheet, "the washer came back a sheet, not a solid");
    assert!((volume - want).abs() < 0.005 * want, "the washer holds {volume}, not {want}");
}

#[test]
fn a_rounded_bar_mesh_becomes_the_bar() {
    let bar = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 20.0, 0.0, 20.0], 10.0).expect("a bar").rounded(2.0).expect("filleted");
    let want = bar.volume();
    for deflection in [0.1, 0.02] {
        let (volume, built, faces, sheet) = recognised(&format!("rounded bar at {deflection}"), &bar, deflection);
        assert_eq!((built, faces), (26, 26), "faces built and faces of the rounded bar at {deflection}");
        assert!(!sheet, "the rounded bar at {deflection} came back a sheet, not a solid");
        assert!((volume - want).abs() < 0.005 * want, "the rounded bar at {deflection} holds {volume}, not {want}");
    }
}

/// A torus is one face closed on itself: nothing to sew it to, and it has to come back a solid all the same.
#[test]
fn a_torus_mesh_becomes_the_torus() {
    use qymcad_core::geom::{encode_loops, Point2, ProfEdge};
    // a circle of 5 about (0, 20) in the XY plane, y the distance from the axis, turned about X
    let (a, b, center) = (Point2::new(5.0, 20.0), Point2::new(-5.0, 20.0), Point2::new(0.0, 20.0));
    let ring = [ProfEdge::Arc { a, b, center, ccw: true }, ProfEdge::Arc { a: b, b: a, center, ccw: true }];
    let torus = qymcad_kernel::specimen::revolve_profile(&encode_loops(&[&ring]), 0, 360.0).expect("a torus");
    let want = torus.volume();
    let (volume, built, faces, sheet) = recognised("torus", &torus, 0.01);
    assert_eq!((built, faces), (1, 1), "faces built and faces of the torus");
    assert!(!sheet, "the torus came back a sheet, not a solid");
    assert!((volume - want).abs() < 0.005 * want, "the torus holds {volume}, not {want}");
}

/// A DENT IN A FLAT FACE IS NOT A SPHERE THE SIZE OF A HOUSE. A cube of 100 drawn with a 10 x 10 grid on every face, the
/// middle corner of its top pushed 0.02 mm in - past the tolerance, the way noise leaves a mesh: the triangles round it
/// make a region of their own, and whatever surface fits them, the body is still the cube, 1 000 000 mm^3 less the dent.
#[test]
fn a_dent_in_a_flat_face_leaves_the_body_whole() {
    let mesh = gridded_cube(10, 100.0, 0.02);
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let b = boundaries(&p, &found);
    let c = curves(&b, &found, &tol);
    let made = solid(&p, &found, &b, &c, &tol).unwrap_or_else(|| panic!("no body - {:?}", last_kernel_refusal()));
    let (body, areas) = (made.shape, made.areas);
    let want = p.mesh.volume();
    assert!(
        !body.is_sheet(),
        "the dented cube came back a sheet: {} of {} faces built, regions {:?}",
        areas.iter().flatten().count(),
        found.len(),
        found.iter().map(|r| (r.tris.len(), r.surface.clone())).collect::<Vec<_>>()
    );
    assert!((body.volume() - want).abs() < 10.0, "the dented cube holds {}, not {want}: regions {:?}", body.volume(), found.iter().map(|r| (r.tris.len(), r.surface.clone())).collect::<Vec<_>>());
}

/// A SHORT ARC KNOWN BY ITS ENDS ALONE STAYS SHORT, whichever way its circle's axis points. A quarter of a cylinder of
/// 10, 10 high, its two arcs left with nothing but their corners, as a sliver between a plane and a sphere of a gear is:
/// with no point between the ends to show the way round, the arc used to go the long way - measured on `cube_gears`, an
/// edge of 0.017 mm became most of a circle of 32.9 and its face took 3408 mm^2 for a triangle of 4e-6.
#[test]
fn a_short_arc_known_by_its_ends_alone_stays_short() {
    use qymcad_core::geom::{encode_loops, Point2, ProfEdge};
    let (o, x, y) = (Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(0.0, 10.0));
    let slice = [ProfEdge::Line { a: o, b: x }, ProfEdge::Arc { a: x, b: y, center: o, ccw: true }, ProfEdge::Line { a: y, b: o }];
    let quarter = qymcad_kernel::specimen::extrude_profile(&encode_loops(&[&slice]), 10.0).expect("a quarter of a cylinder");
    let want = quarter.volume();
    let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(&quarter, 0.01).into_iter().next().expect("a body");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let b = boundaries(&p, &found);
    let traced = curves(&b, &found, &tol);
    for turned in [false, true] {
        let (mut short, mut c) = (b.clone(), traced.clone());
        let mut arcs = 0;
        for (e, ce) in c.iter_mut().enumerate() {
            if let (Curve::Circle { axis, .. }, Some(_)) = (ce, short.edges[e].ends) {
                let pts = &short.edges[e].points;
                short.edges[e].points = vec![pts[0], pts[pts.len() - 1]];
                if turned {
                    *axis = [-axis[0], -axis[1], -axis[2]];
                }
                arcs += 1;
            }
        }
        assert_eq!(arcs, 2, "arcs of the quarter cylinder: curves {c:?}");
        let body = solid(&p, &found, &short, &c, &tol).unwrap_or_else(|| panic!("no body - {:?}", last_kernel_refusal())).shape;
        assert!(!body.is_sheet(), "the quarter cylinder, its axes turned: {turned}, came back a sheet");
        assert!((body.volume() - want).abs() < 0.005 * want, "the quarter cylinder, its axes turned: {turned}, holds {}, not {want}", body.volume());
    }
}

/// A REGION LEFT AS MESH IS SEWN IN AS ITS TRIANGLES, and the body stays whole. The cube's mesh from the kernel with one
/// face's region given no surface - as recognition leaves a region no surface fits, and as a face that took a wrong piece
/// of its surface is taken back: its two triangles become flat faces beside the five recognised, and the body is the
/// cube, not a shell with a hole in it.
#[test]
fn a_region_left_as_mesh_is_sewn_in_as_its_triangles() {
    let cube = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 100.0, 0.0, 100.0, 100.0, 0.0, 100.0], 100.0).expect("a cube");
    let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(&cube, 0.1).into_iter().next().expect("a body");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let mut found = regions(&p, &tol);
    assert_eq!(found.len(), 6, "faces of the cube");
    found[0].surface = None;
    let b = boundaries(&p, &found);
    let c = curves(&b, &found, &tol);
    let made = solid(&p, &found, &b, &c, &tol).unwrap_or_else(|| panic!("no body - {:?}", last_kernel_refusal()));
    let (body, areas) = (made.shape, made.areas);
    assert_eq!(areas.iter().flatten().count(), 5, "recognised faces built");
    assert!(!body.is_sheet(), "the cube with a face left as mesh came back a sheet");
    assert!((body.volume() - 1.0e6).abs() < 1.0, "the cube with a face left as mesh holds {}", body.volume());
}

/// A SEAM BESIDE A REGION LEFT AS MESH STILL CLOSES, however far the corner beside it was placed. The cube's mesh with
/// its top left as mesh and one side given a sphere of 10 000 mm for a surface - a fit as loose as a crumb's on the gears
/// of `cube_gears`: that side stays a face, but the corners it shares with the top are placed on the sphere, a quarter of
/// a millimetre off the mesh corners, while the top's triangles keep them. Fifteen times the sewing's tolerance: the
/// seam closes only if the edge beside the mesh ends where the mesh does.
#[test]
fn a_seam_beside_a_region_left_as_mesh_closes() {
    use qymcad_meshfit::Surface;
    let cube = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 100.0, 0.0, 100.0, 100.0, 0.0, 100.0], 100.0).expect("a cube");
    let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(&cube, 0.1).into_iter().next().expect("a body");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let mut found = regions(&p, &tol);
    let facing = |r: usize, want: [f64; 3]| matches!(found[r].surface, Some(Surface::Plane { normal, .. }) if normal.iter().zip(want).map(|(a, b)| a * b).sum::<f64>() > 0.9);
    let top = (0..found.len()).find(|&r| facing(r, [0.0, 0.0, 1.0])).expect("the top of the cube");
    let side = (0..found.len()).find(|&r| facing(r, [1.0, 0.0, 0.0])).expect("the side of the cube");
    found[top].surface = None;
    found[side].surface = Some(Surface::Sphere { center: [100.0 - 10_000.0, 50.0, 50.0], radius: 10_000.0 });
    let b = boundaries(&p, &found);
    let c = curves(&b, &found, &tol);
    let made = solid(&p, &found, &b, &c, &tol).unwrap_or_else(|| panic!("no body - {:?}", last_kernel_refusal()));
    assert_eq!(made.free_edges, 0, "sides left free: {:?}", made.free_at);
    assert!(!made.shape.is_sheet(), "the cube came back a sheet");
    assert!((made.shape.volume() - 1.0e6).abs() < 1.0e4, "the cube holds {}", made.shape.volume());
}

/// A cube of `side` drawn with a `grid` x `grid` net on every face, the middle corner of its top pushed `dent` in.
fn gridded_cube(grid: usize, side: f64, dent: f64) -> qymcad_core::geom::Mesh {
    use qymcad_core::geom::{Mesh, Point3};
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
                    if a == 2 && high && i == grid / 2 && j == grid / 2 {
                        q[2] -= dent;
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
    Mesh { verts, tris, ..Default::default() }
}

/// A mesh of `shape` whose every corner is moved along its radius by up to `amp`, the same way every time: the noise of
/// a scan or of a coarse export, larger than the tolerance a mesh from a CAD gets. `radial` names what the radius is
/// taken from - all three axes for a sphere, the two across Z for a cylinder. Returns the mesh and the volume it holds.
fn noisy(shape: &Shape, amp: f64, radial: [f64; 3]) -> (qymcad_core::geom::Mesh, f64) {
    let qymcad_core::geom::Built { mut mesh, .. } = qymcad_kernel::specimen::mesh(shape, 0.05).into_iter().next().expect("a mesh");
    for v in &mut mesh.verts {
        // THE NOISE OF A CORNER COMES FROM WHERE IT STANDS, not from its number: the kernel hands a corner shared by two
        // faces twice, and moved by two numbers the copies part - the mesh itself torn along the rim of a cylinder
        let mut seed = [v.x, v.y, v.z].iter().fold(12345u64, |h, c| (h ^ ((c * 1e6).round() as i64 as u64)).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407));
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let k = ((seed >> 33) as f64 / (1u64 << 31) as f64 - 0.5) * 2.0 * amp;
        let r = (v.x * v.x * radial[0] + v.y * v.y * radial[1] + v.z * v.z * radial[2]).sqrt().max(1e-9);
        v.x += v.x * radial[0] / r * k;
        v.y += v.y * radial[1] / r * k;
        v.z += v.z * radial[2] / r * k;
    }
    let held = mesh
        .tris
        .iter()
        .map(|t| {
            let (a, b, c) = (mesh.verts[t[0] as usize], mesh.verts[t[1] as usize], mesh.verts[t[2] as usize]);
            (a.x * (b.y * c.z - b.z * c.y) - a.y * (b.x * c.z - b.z * c.x) + a.z * (b.x * c.y - b.y * c.x)) / 6.0
        })
        .sum();
    (mesh, held)
}

/// A NOISY MESH BECOMES A BODY OF ITS OWN VOLUME. A cylinder of 10 by 20 and a sphere of 10, every corner moved along its
/// radius by up to 0.001 or 0.02 mm - more than the tolerance a CAD mesh of this size gets: the regions fit surfaces
/// only in patches. The body closed, but the cylinder moved by 0.001 held 7.5 % more than its mesh, and by 0.02, 2.3 %
/// more - a face taking a larger piece of its surface than its region covers, the fault the gears of `cube_gears` show
/// at 16 to 21 %. The guard is the volume within 0.5 %; a true surface holds more than the chords of its mesh, so
/// the body may lie between the mesh less 0.5 % and the exact body of the kernel plus 0.5 %.
#[test]
fn a_noisy_mesh_becomes_a_body_of_its_own_volume() {
    let cylinder = qymcad_kernel::specimen::cylinder(10.0, 20.0).expect("a cylinder");
    let sphere = qymcad_kernel::specimen::sphere(10.0).expect("a sphere");
    let mut wrong = Vec::new();
    for (name, shape, amp, radial) in [("cylinder", &cylinder, 0.001, [1.0, 1.0, 0.0]), ("cylinder", &cylinder, 0.02, [1.0, 1.0, 0.0]), ("sphere", &sphere, 0.02, [1.0, 1.0, 1.0])] {
        let (mesh, held) = noisy(shape, amp, radial);
        let p = prepare(&mesh, weld_tolerance(&mesh));
        let tol = Tolerance::for_mesh(&p);
        let found = regions(&p, &tol);
        let b = boundaries(&p, &found);
        let c = curves(&b, &found, &tol);
        let Some(made) = solid(&p, &found, &b, &c, &tol) else {
            wrong.push(format!("{name} moved by {amp}: no body - {:?}", last_kernel_refusal()));
            continue;
        };
        let v = made.shape.volume();
        if made.free_edges > 0 || made.shape.is_sheet() || v < held * 0.995 || v > shape.volume() * 1.005 {
            wrong.push(format!("{name} moved by {amp}: {} sides unmet, volume {v:.2} against the mesh's {held:.2} and the exact {:.2}", made.free_edges, shape.volume()));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
#[ignore = "a measure"]
fn noisy_meshes_are_measured() {
    for (name, shape) in [("sphere", qymcad_kernel::specimen::sphere(10.0).expect("a sphere")), ("cylinder", qymcad_kernel::specimen::cylinder(10.0, 20.0).expect("a cylinder"))] {
        for amp in [0.0, 0.001, 0.005, 0.02, 0.05] {
            let (mesh, held) = noisy(&shape, amp, if name == "sphere" { [1.0, 1.0, 1.0] } else { [1.0, 1.0, 0.0] });
            let p = prepare(&mesh, weld_tolerance(&mesh));
            let tol = Tolerance::for_mesh(&p);
            let found = regions(&p, &tol);
            let b = boundaries(&p, &found);
            let c = curves(&b, &found, &tol);
            match solid(&p, &found, &b, &c, &tol) {
                Some(m) => println!(
                    "NOISY {name} {amp}: {} regions, {} tris, free {}, sheet {}, volume {:.2} against {held:.2}",
                    found.len(),
                    mesh.tris.len(),
                    m.free_edges,
                    m.shape.is_sheet(),
                    m.shape.volume()
                ),
                None => println!("NOISY {name} {amp}: {} regions - no body", found.len()),
            }
        }
    }
}

/// A PLANE ACROSS A CYLINDER AT A SLANT MEETS IT IN AN ELLIPSE, and the edge is that ellipse, not a curve through the
/// mesh's points. A cylinder of 10 by 30 cut by our kernel along a plane turned 30 deg about X through (0, 0, 15): the
/// top is an ellipse of half-axes 10 / cos 30 = 11.547 and 10. The recognised body holds the kernel's volume.
#[test]
fn a_slanted_cut_of_a_cylinder_is_an_ellipse() {
    let post = qymcad_kernel::specimen::cylinder(10.0, 30.0).expect("a cylinder");
    let (s, c) = (30f64.to_radians().sin(), 30f64.to_radians().cos());
    let block = qymcad_kernel::specimen::extrude(&[-50.0, -50.0, 50.0, -50.0, 50.0, 50.0, -50.0, 50.0], 100.0).expect("a block");
    let up = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 15.0];
    let turn = [1.0, 0.0, 0.0, 0.0, 0.0, c, -s, s * 15.0, 0.0, s, c, 15.0 - c * 15.0];
    let knife = qymcad_kernel::specimen::placed(&block, &up).and_then(|b| qymcad_kernel::specimen::placed(&b, &turn)).expect("the knife");
    let cut = post.combined(&knife, 0).expect("the slanted post");
    let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(&cut, 0.01).into_iter().next().expect("a mesh");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let b = boundaries(&p, &found);
    let traced = curves(&b, &found, &tol);
    let ellipse = traced.iter().find_map(|k| match k {
        Curve::Ellipse { major, minor, .. } => Some((*major, *minor)),
        _ => None,
    });
    assert!(
        ellipse.is_some_and(|(a, m)| (a - 10.0 / c).abs() < 0.01 && (m - 10.0).abs() < 0.01),
        "the slanted top is not an ellipse of 11.547 and 10: {ellipse:?}; curves {:?}",
        traced
            .iter()
            .map(|k| match k {
                Curve::Line { .. } => "line",
                Curve::Circle { .. } => "circle",
                Curve::Points(_) => "points",
                _ => "other",
            })
            .collect::<Vec<_>>()
    );
    let made = solid(&p, &found, &b, &traced, &tol).expect("a body");
    assert!(!made.shape.is_sheet() && (made.shape.volume() - cut.volume()).abs() < 1e-3 * cut.volume(), "the body holds {} against the kernel's {}", made.shape.volume(), cut.volume());
}

/// A NOISY MESH AT A TOLERANCE ABOVE ITS NOISE FALLS INTO ITS OWN FACES. The cylinder of 10 by 20 with its corners moved
/// along the radius by up to 0.02 mm: at the tolerance a CAD mesh gets, its wall is patches; at a hundred times that -
/// the number a person gives the recognition for a scan - it is two caps and one wall of 10, and the body holds the
/// mesh's volume.
#[test]
fn a_noisy_mesh_at_a_wider_tolerance_falls_into_its_faces() {
    let cylinder = qymcad_kernel::specimen::cylinder(10.0, 20.0).expect("a cylinder");
    let (mesh, held) = noisy(&cylinder, 0.02, [1.0, 1.0, 0.0]);
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let mut tol = Tolerance::for_mesh(&p);
    tol.distance *= 100.0;
    let found = regions(&p, &tol);
    let walls: Vec<f64> = found
        .iter()
        .filter_map(|r| match r.surface {
            Some(qymcad_meshfit::Surface::Cylinder { radius, .. }) => Some(radius),
            _ => None,
        })
        .collect();
    assert!(
        found.len() == 3 && walls.len() == 1 && (walls[0] - 10.0).abs() < 0.02,
        "at a tolerance of {:.4} the noisy cylinder is {} regions, walls {walls:?}: {:?}",
        tol.distance,
        found.len(),
        found.iter().map(|r| (r.tris.len(), r.surface.clone())).collect::<Vec<_>>()
    );
    let b = boundaries(&p, &found);
    let made = solid(&p, &found, &b, &curves(&b, &found, &tol), &tol).expect("a body");
    assert!(!made.shape.is_sheet() && (made.shape.volume() - held).abs() < 0.01 * held, "the body holds {} against the mesh's {held}", made.shape.volume());
}

/// A NARROW CONICAL BAND IS A CONE. A washer of 5 to 10 and 4 thick with a chamfer of 0.5 at 45 deg on its outer rim: the
/// chamfer is a band 0.71 wide round a radius of 9.75 - the flanges of the owner's screw are such bands, and they came in
/// as spheres. It is one cone of half-angle 45 deg, and the body is the washer's five faces and its volume.
#[test]
fn a_narrow_conical_band_is_a_cone() {
    let washer = qymcad_kernel::specimen::revolve(&[0.0, 5.0, 4.0, 5.0, 4.0, 9.5, 3.5, 10.0, 0.0, 10.0], 0, 360.0).expect("a chamfered washer");
    let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(&washer, 0.01).into_iter().next().expect("a mesh");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let cones: Vec<f64> = found
        .iter()
        .filter_map(|r| match r.surface {
            Some(qymcad_meshfit::Surface::Cone { half_angle, .. }) => Some(half_angle.to_degrees()),
            _ => None,
        })
        .collect();
    assert!(
        found.len() == 5 && cones.len() == 1 && (cones[0] - 45.0).abs() < 0.1,
        "the chamfered washer is {} regions, cones {cones:?}: {:?}",
        found.len(),
        found.iter().map(|r| (r.tris.len(), r.surface.clone())).collect::<Vec<_>>()
    );
    let b = boundaries(&p, &found);
    let made = solid(&p, &found, &b, &curves(&b, &found, &tol), &tol).expect("a body");
    assert!(!made.shape.is_sheet() && (made.shape.volume() - washer.volume()).abs() < 1e-3 * washer.volume(), "the body holds {} against the kernel's {}", made.shape.volume(), washer.volume());
}

/// A TRIANGLE TOO SMALL TO CARRY A NORMAL JOINS THE SURFACE ITS CORNERS LIE ON. The cube of 10, its top a fan round a
/// triangle of sides 0.017 in the middle with one corner raised by 0.008 - under the tolerance of 0.017 a hundred times
/// wider gives, yet enough to turn that triangle's normal 28 deg: it stood alone, a region of one triangle, as 1 677 of
/// them stood on the owner's screw at that tolerance. At that size the tolerance itself turns a normal further than the
/// angle allows; its corners on the top, it is the top.
#[test]
fn a_triangle_too_small_for_its_normal_joins_its_surface() {
    let v = |x: f64, y: f64, z: f64| qymcad_core::geom::Point3::new(x, y, z);
    let mut verts = vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(10.0, 10.0, 0.0), v(0.0, 10.0, 0.0), v(0.0, 0.0, 10.0), v(10.0, 0.0, 10.0), v(10.0, 10.0, 10.0), v(0.0, 10.0, 10.0)];
    let mut tris: Vec<[u32; 3]> = vec![[0, 2, 1], [0, 3, 2], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]];
    let s = 0.0173 / 3f64.sqrt();
    verts.push(v(5.0, 5.0 - s, 10.008)); // 8: p, raised
    verts.push(v(5.0 + s * 0.866, 5.0 + s * 0.5, 10.0)); // 9: q
    verts.push(v(5.0 - s * 0.866, 5.0 + s * 0.5, 10.0)); // 10: r
    let (a, b, c, d, p, q, r) = (4, 5, 6, 7, 8, 9, 10);
    tris.extend([[a, b, p], [b, q, p], [b, c, q], [c, r, q], [c, d, r], [d, p, r], [d, a, p], [p, q, r]]);
    let mesh = qymcad_core::geom::Mesh { verts, tris, ..Default::default() };
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let mut tol = Tolerance::for_mesh(&p);
    tol.distance *= 100.0;
    let found = regions(&p, &tol);
    assert_eq!(found.len(), 6, "the cube is not its six faces at a tolerance of {:.4}: {:?}", tol.distance, found.iter().map(|r| (r.tris.len(), r.surface.clone())).collect::<Vec<_>>());
}

/// A SMOOTH FREE-FORM WALL IS ONE FACE, not a mosaic of primitives. A loft of our kernel, smooth, through squares of 10, 6
/// and 10 at heights 0, 15 and 30: its four walls are B-spline surfaces, pinched in at the middle - no plane, cylinder,
/// cone, sphere or torus is any of them. The body is the loft's six faces and its volume.
#[test]
fn a_free_form_wall_is_one_face() {
    use qymcad_core::geom::{encode_loop, Point2, ProfEdge};
    let square = |h: f64| {
        let line = |ax: f64, ay: f64, bx: f64, by: f64| ProfEdge::Line { a: Point2::new(ax, ay), b: Point2::new(bx, by) };
        encode_loop(&[line(-h, -h, h, -h), line(h, -h, h, h), line(h, h, -h, h), line(-h, h, -h, -h)])
    };
    let (mut data, mut offsets, mut places) = (Vec::new(), vec![0usize], Vec::new());
    for (h, z) in [(5.0, 0.0), (3.0, 15.0), (5.0, 30.0)] {
        data.extend_from_slice(&square(h));
        offsets.push(data.len());
        places.extend_from_slice(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, z]);
    }
    let loft = qymcad_kernel::specimen::loft(&data, &offsets, &places, qymcad_core::feature::LoftWalls::Smooth, qymcad_core::feature::LoftBody::Solid).expect("the loft");
    let qymcad_core::geom::Built { mesh, faces: kernel_faces } = qymcad_kernel::specimen::mesh(&loft, 0.01).into_iter().next().expect("a mesh");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let b = boundaries(&p, &found);
    let made = solid(&p, &found, &b, &curves(&b, &found, &tol), &tol).expect("a body");
    let faces = qymcad_kernel::specimen::mesh(&made.shape, 0.01).into_iter().next().map(|qymcad_core::geom::Built { faces: f, .. }| f.len()).unwrap_or(0);
    assert!(
        faces == kernel_faces.len() && !made.shape.is_sheet() && (made.shape.volume() - loft.volume()).abs() < 0.005 * loft.volume(),
        "the loft of {} faces came back as {faces} faces from {} regions ({} left as mesh), {}, holding {} against {}",
        kernel_faces.len(),
        found.len(),
        found.iter().filter(|r| r.surface.is_none()).count(),
        if made.shape.is_sheet() { "a shell" } else { "a solid" },
        made.shape.volume(),
        loft.volume()
    );
}

/// WHAT A THREAD COMES BACK AS: an M10 x 1.5 cut by our kernel from the end of a rod of 25 along 20, meshed and
/// recognised - its regions by kind, the body's faces, and its volume against the kernel's. Ignored: a measure.
#[test]
#[ignore = "a measure"]
fn a_thread_is_measured() {
    for defl in [0.05, 0.02] {
        let cut = thread_on_a_rod(25.0, 0.0, 20.0);
        let qymcad_core::geom::Built { mesh, faces: kfaces } = qymcad_kernel::specimen::mesh(&cut, defl).into_iter().next().expect("a mesh");
        let t = std::time::Instant::now();
        let p = prepare(&mesh, weld_tolerance(&mesh));
        let tol = Tolerance::for_mesh(&p);
        let found = regions(&p, &tol);
        let mut kinds = std::collections::BTreeMap::new();
        for r in &found {
            *kinds.entry(format!("{:?}", r.surface).split(|c: char| !c.is_alphanumeric()).nth(if r.surface.is_some() { 1 } else { 0 }).unwrap_or("?").to_string()).or_insert(0usize) += 1;
        }
        let b = boundaries(&p, &found);
        let made = solid(&p, &found, &b, &curves(&b, &found, &tol), &tol);
        let (faces, sheet, v) = made.as_ref().map_or((0, true, 0.0), |m| {
            (qymcad_kernel::specimen::mesh(&m.shape, defl).into_iter().next().map(|qymcad_core::geom::Built { faces: f, .. }| f.len()).unwrap_or(0), m.shape.is_sheet(), m.shape.volume())
        });
        println!(
            "THREAD defl {defl}: {} triangles, kernel faces {}, {} regions {kinds:?}; body {faces} faces, {}, volume {v:.3} against {:.3}, free {:?}, {:.1} s",
            mesh.tris.len(),
            kfaces.len(),
            found.len(),
            if sheet { "a shell" } else { "a solid" },
            cut.volume(),
            made.as_ref().map(|m| m.free_edges),
            t.elapsed().as_secs_f64()
        );
    }
}

/// A thread of 10 cut by our kernel into a rod of radius 5 and `rod` long, from `from` along `length`: an M10 x 1.5.
fn thread_on_a_rod(rod: f64, from: f64, length: f64) -> Shape {
    thread_of(qymcad_core::thread::ThreadStandard::MetricIso, 1.5, rod, from, length)
}

/// A thread of 10 of `standard` and `pitch` cut by our kernel into a rod of radius 5 and `rod` long, from `from` along
/// `length`.
fn thread_of(standard: qymcad_core::thread::ThreadStandard, pitch: f64, rod: f64, from: f64, length: f64) -> Shape {
    use qymcad_core::thread::{encode_edges, ThreadSpec};
    let spec = ThreadSpec { standard, nominal_d: 10.0, pitch, ..Default::default() };
    let g = spec.geometry();
    qymcad_kernel::specimen::cylinder(g.stock_d * 0.5, rod)
        .expect("the rod")
        .grooved(qymcad_kernel::HelicalCut {
            axis: qymcad_core::feature::AxisLine { origin: [0.0, 0.0, from], dir: [0.0, 0.0, 1.0] },
            radius: g.stock_d * 0.5,
            profile: &encode_edges(&g.groove),
            length,
            lead: g.lead,
            starts: spec.starts,
            hand: qymcad_kernel::Hand::Right,
            kind: qymcad_kernel::Helix::Groove,
            lead_in: 0.0,
            lead_out: 0.0,
            gnames: &[],
            rnames: &[],
            crest_relief: 0.0,
        })
        .expect("the thread")
}

/// A THREAD COMES BACK A BODY OF A FEW SMOOTH FACES, not of thousands: an M10 x 1.5 cut by our kernel along 6 mm, from
/// the end of a rod of 10 and in the middle of a rod of 12. Its flanks and its root are straight pieces of the profile
/// screwed about the axis, and they came apart into tiles each within the tolerance of a small cylinder, plane or
/// sphere: 2 158 and 2 113 faces. As helices they are a face each; the crest - the rod's cylinder less the groove wound
/// round it - is cut in halves about the axis, whole it came back 283 and 316 faces; and the curves of the root's edges
/// hold within one step of their points, within four 372 and 368.
#[test]
fn a_thread_comes_back_in_a_few_smooth_faces() {
    for (rod, from) in [(10.0, 0.0), (12.0, 3.0)] {
        let cut = thread_on_a_rod(rod, from, 6.0);
        let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(&cut, 0.05).into_iter().next().expect("a mesh");
        let p = prepare(&mesh, weld_tolerance(&mesh));
        let tol = Tolerance::for_mesh(&p);
        let found = regions(&p, &tol);
        let b = boundaries(&p, &found);
        let made = solid(&p, &found, &b, &curves(&b, &found, &tol), &tol).expect("a body");
        let faces = qymcad_kernel::specimen::mesh(&made.shape, 0.05).into_iter().next().map(|qymcad_core::geom::Built { faces: f, .. }| f.len()).unwrap_or(0);
        let helices = found.iter().filter(|r| matches!(r.surface, Some(qymcad_meshfit::Surface::Helix { .. }))).count();
        assert!(
            !made.shape.is_sheet() && (made.shape.volume() - cut.volume()).abs() < 0.005 * cut.volume() && faces < 150,
            "the thread from {from} of a rod of {rod} came back {} of {faces} faces ({helices} helices, {} regions of {} triangles), holding {} against {}",
            if made.shape.is_sheet() { "a shell" } else { "a solid" },
            found.len(),
            mesh.tris.len(),
            made.shape.volume(),
            cut.volume()
        );
    }
}

/// A SPRING OF ROUND WIRE COMES BACK A TUBE, not a mosaic: a wire of radius 1 wound three turns of radius 10 rising 4 a
/// turn, its circle swept square to the helix as a CAD program sweeps it, meshed every 5 deg along and 15 deg round,
/// its ends flat. The body is a few faces holding the exact volume, pi x 1^2 x the wire's length.
#[test]
fn a_spring_of_round_wire_comes_back_a_tube() {
    let (radius, pitch, wire, turns) = (10.0f64, 4.0f64, 1.0f64, 3.0f64);
    let rise = pitch / (2.0 * std::f64::consts::PI);
    let along = (turns * 72.0) as usize;
    let round = 24usize;
    let length = (radius * radius + rise * rise).sqrt();
    let at = |t: f64, phi: f64| {
        let (s, c) = t.sin_cos();
        let tangent = [-radius * s / length, radius * c / length, rise / length];
        let normal = [-c, -s, 0.0];
        let binormal = [tangent[1] * normal[2] - tangent[2] * normal[1], tangent[2] * normal[0] - tangent[0] * normal[2], tangent[0] * normal[1] - tangent[1] * normal[0]];
        let (sp, cp) = phi.sin_cos();
        qymcad_core::geom::Point3::new(
            radius * c + wire * (cp * normal[0] + sp * binormal[0]),
            radius * s + wire * (cp * normal[1] + sp * binormal[1]),
            rise * t + wire * (cp * normal[2] + sp * binormal[2]),
        )
    };
    let (mut verts, mut tris) = (Vec::new(), Vec::new());
    for i in 0..=along {
        let t = 2.0 * std::f64::consts::PI * turns * i as f64 / along as f64;
        for j in 0..round {
            verts.push(at(t, 2.0 * std::f64::consts::PI * j as f64 / round as f64));
        }
    }
    let id = |i: usize, j: usize| (i * round + j % round) as u32;
    for i in 0..along {
        for j in 0..round {
            tris.push([id(i, j), id(i, j + 1), id(i + 1, j + 1)]);
            tris.push([id(i, j), id(i + 1, j + 1), id(i + 1, j)]);
        }
    }
    for (i, inward) in [(0usize, true), (along, false)] {
        let t = 2.0 * std::f64::consts::PI * turns * i as f64 / along as f64;
        let middle = verts.len() as u32;
        verts.push(qymcad_core::geom::Point3::new(radius * t.cos(), radius * t.sin(), rise * t));
        for j in 0..round {
            tris.push(if inward { [middle, id(i, j + 1), id(i, j)] } else { [middle, id(i, j), id(i, j + 1)] });
        }
    }
    let mesh = qymcad_core::geom::Mesh { verts, tris, ..Default::default() };
    let exact = std::f64::consts::PI * wire * wire * 2.0 * std::f64::consts::PI * turns * length;
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let b = boundaries(&p, &found);
    let made = solid(&p, &found, &b, &curves(&b, &found, &tol), &tol).expect("a body");
    let faces = qymcad_kernel::specimen::mesh(&made.shape, 0.02).into_iter().next().map(|qymcad_core::geom::Built { faces: f, .. }| f.len()).unwrap_or(0);
    assert!(
        !made.shape.is_sheet() && (made.shape.volume() - exact).abs() < 0.005 * exact && faces < 10,
        "the spring came back {} of {faces} faces ({} regions of {} triangles), holding {} against {exact}",
        if made.shape.is_sheet() { "a shell" } else { "a solid" },
        found.len(),
        mesh.tris.len(),
        made.shape.volume()
    );
}

/// AN AUGER COMES BACK A BODY OF A FEW SMOOTH FACES: our kernel's flight 3 thick and 10 high, two turns rising 20 a
/// turn on a shaft of 10, its edges rounded by 0.8. The flight's sides are flat profiles screwed about the axis, its
/// rounded edges straight pieces of them, its tip a cylinder with the flight wound round it.
#[test]
fn an_auger_comes_back_in_a_few_smooth_faces() {
    use qymcad_core::thread::{encode_edges, AugerSpec};
    let a = AugerSpec { shaft_d: 10.0, outer_d: 30.0, pitch: 20.0, thickness: 3.0, edge_r: 0.8, ..Default::default() };
    let auger = qymcad_kernel::specimen::cylinder(a.shaft_d * 0.5, 50.0)
        .expect("the shaft")
        .grooved(qymcad_kernel::HelicalCut {
            axis: qymcad_core::feature::AxisLine { origin: [0.0, 0.0, 5.0], dir: [0.0, 0.0, 1.0] },
            radius: a.shaft_d * 0.5,
            profile: &encode_edges(&a.flight_profile()),
            length: 40.0,
            lead: a.lead(),
            starts: 1,
            hand: qymcad_kernel::Hand::Right,
            kind: qymcad_kernel::Helix::Rib,
            lead_in: 0.0,
            lead_out: 0.0,
            gnames: &[],
            rnames: &[],
            crest_relief: 0.0,
        })
        .expect("the auger");
    let qymcad_core::geom::Built { mesh, faces: kernel_faces } = qymcad_kernel::specimen::mesh(&auger, 0.05).into_iter().next().expect("a mesh");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let b = boundaries(&p, &found);
    let made = solid(&p, &found, &b, &curves(&b, &found, &tol), &tol).expect("a body");
    let faces = qymcad_kernel::specimen::mesh(&made.shape, 0.05).into_iter().next().map(|qymcad_core::geom::Built { faces: f, .. }| f.len()).unwrap_or(0);
    assert!(
        !made.shape.is_sheet() && (made.shape.volume() - auger.volume()).abs() < 0.005 * auger.volume() && faces < 2 * kernel_faces.len(),
        "the auger of {} faces came back {} of {faces} faces ({} regions of {} triangles), holding {} against {}",
        kernel_faces.len(),
        if made.shape.is_sheet() { "a shell" } else { "a solid" },
        found.len(),
        mesh.tris.len(),
        made.shape.volume(),
        auger.volume()
    );
}
