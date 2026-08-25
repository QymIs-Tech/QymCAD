//! The FFI to the kernel: a box tessellates and a real STEP file reads.

use qymcad_core::feature::{LoftBody, LoftWalls};
use qymcad_kernel::{box_mesh, extrude, import_step};

/// The profiles for these tests are built by the production encoder (`geom::encode_*`) rather than by a copy of
/// the format: otherwise extending the record of an edge breaks the tests silently, the kernel reading the
/// profile at an offset.
mod enc {
    use qymcad_core::geom::{encode_loop, encode_loops, Point2, ProfEdge};
    pub fn line(ax: f64, ay: f64, bx: f64, by: f64) -> ProfEdge {
        ProfEdge::Line { a: Point2::new(ax, ay), b: Point2::new(bx, by) }
    }
    pub fn circle(cx: f64, cy: f64, r: f64) -> ProfEdge {
        ProfEdge::Circle { center: Point2::new(cx, cy), r }
    }
    /// A whole profile: the first loop is the outer one and the rest are holes.
    pub fn prof(loops: &[&[ProfEdge]]) -> Vec<f64> { encode_loops(loops) }
    /// A single loop without the contour count, as loft sections take it.
    pub fn one_loop(edges: &[ProfEdge]) -> Vec<f64> { encode_loop(edges) }
}

#[test]
fn box_tessellates() {
    let m = box_mesh(10.0, 20.0, 5.0, 0.5);
    assert!(m.verts.len() >= 8, "a box has vertices");
    assert!(m.tris.len() >= 12, "a box is at least 12 triangles, found {}", m.tris.len());
    // the indices stay in range
    let nv = m.verts.len() as u32;
    assert!(m.tris.iter().all(|t| t.iter().all(|&i| i < nv)));
}

#[test]
fn imports_sample_step() {
    // the path from the workspace root, `CARGO_MANIFEST_DIR` being the crate directory
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/Table_CNC_WORK.stp");
    if !std::path::Path::new(path).exists() {
        eprintln!("skipped: no {path}");
        return;
    }
    let bodies = import_step(path, 0.5).expect("the STEP file imports");
    assert!(!bodies.is_empty(), "there is at least one body");
    let tris: usize = bodies.iter().map(|(m, _)| m.tris.len()).sum();
    let faces: usize = bodies.iter().map(|(_, f)| f.len()).sum();
    assert!(tris > 100, "a real part has many triangles, found {tris}");
    assert!(faces > 0, "the faces of the B-rep topology are extracted");
    eprintln!("STEP: {} bodies, {tris} triangles, {faces} B-rep faces", bodies.len());
}

#[test]
fn extrude_square_makes_box() {
    // a 10×10 square extruded by 5, giving a cuboid
    let xy = [0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0];
    let bodies = extrude(&xy, 5.0, 0.5).expect("the extrusion succeeded");
    assert_eq!(bodies.len(), 1, "one body");
    let (m, faces) = &bodies[0];
    assert!(m.tris.len() >= 12, "a cuboid has at least 12 triangles, found {}", m.tris.len());
    let b = m.bounds().expect("the bounding box");
    assert!((b.max.z - 5.0).abs() < 1e-6, "the height is 5, max.z={}", b.max.z);
    assert!(faces.len() >= 6, "a block has at least six B-rep faces, found {}", faces.len());
}

#[test]
fn step_write_roundtrips_via_step_read() {
    // a STEP export: two bodies with their own world transforms, written to a file and read back
    use qymcad_kernel::{import_step, step_solids, write_step, Shape};
    let ident = qymcad_core::feature::PLACE_IDENTITY;
    // cuboid A at the origin, cuboid B moved 30 along X by the world transform of the assembly
    let a = Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("A");
    let b = Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("B");
    let mut shift = ident;
    shift[3] = 30.0; // the translation along X
    let path = std::env::temp_dir().join("qym_export_test.step");
    let p = path.to_string_lossy();
    write_step(&[(&a, ident), (&b, shift)], &p).expect("the STEP write succeeded");

    // read back as separate solids: there have to be two bodies
    let solids = step_solids(&p).expect("the STEP shapes read back");
    assert_eq!(solids.len(), 2, "two bodies in the STEP file, found {}", solids.len());
    // and as meshes: the transform survived, so the combined extent along X is about [0, 40]
    let bodies = import_step(&p, 0.5).expect("the STEP file as meshes");
    let (mut xmin, mut xmax) = (f64::MAX, f64::MIN);
    for (m, _) in &bodies {
        let bb = m.bounds().unwrap();
        xmin = xmin.min(bb.min.x);
        xmax = xmax.max(bb.max.x);
    }
    assert!(xmin.abs() < 1e-3, "the first body sits at the origin: xmin={xmin}");
    assert!((xmax - 40.0).abs() < 1e-3, "the second body is moved by 30, so xmax is about 40: {xmax}");
}

#[test]
fn step_write_empty_is_error() {
    assert!(qymcad_kernel::write_step(&[], "/tmp/qym_never.step").is_err(), "an empty set is an error");
}

#[test]
fn revolve_makes_solid() {
    // a rectangle over x in [0, 10] and y in [2, 5], revolved about X through 360°, giving a tube
    let xy = [0.0, 2.0, 10.0, 2.0, 10.0, 5.0, 0.0, 5.0];
    let bodies = qymcad_kernel::revolve(&xy, 0, 360.0, 0.5).expect("the revolve succeeded");
    assert_eq!(bodies.len(), 1);
    let (m, _faces) = &bodies[0];
    assert!(m.tris.len() > 50, "a body of revolution, {} triangles", m.tris.len());
    let b = m.bounds().unwrap();
    assert!((b.max.x - 10.0).abs() < 1e-3, "the length along X is 10");
}

#[test]
fn shell_center_builds_and_grows_bbox() {
    // A shell centred on the surface: a wall of thickness t is centred on the face, so the bounding box grows
    // by about t/2 on each side, where an ordinary inward shell leaves it unchanged. The top face of a 10 mm
    // cube is left open.
    use qymcad_kernel::Shape;
    let sq = |h: f64| {
        let d = enc::prof(&[&[enc::line(0.0, 0.0, 10.0, 0.0), enc::line(10.0, 0.0, 10.0, 10.0), enc::line(10.0, 10.0, 0.0, 10.0), enc::line(0.0, 10.0, 0.0, 0.0)]]);
        Shape::extrude_profile(&d, h).expect("the cube")
    };
    let faces = sq(10.0).tessellate(0.5).remove(0).1;
    // the top face, whose normal is +Z
    let top = faces.iter().find(|f| f.normal[2] > 0.9).map(|f| f.id).expect("the top face");
    let shape = sq(10.0).shell_center(2.0, &[top]).expect("the centred shell built");
    let b = shape.tessellate(0.3).remove(0).0.bounds().expect("bbox");
    // t = 2 centred gives +1 on each side, so X goes from [0, 10] to about [-1, 11]
    assert!(b.max.x - b.min.x > 11.5, "the bounding box grew by about t, the centred wall reaching outward: {:?}", (b.min.x, b.max.x));
    assert!(b.min.x < -0.5, "the wall passed outside the original face: min.x={}", b.min.x);
}

#[test]
fn hole_stepped_counterbore_countersink_cut() {
    // a stepped hole: a counterbore and a countersink cut real B-rep, so the face count grows
    use qymcad_kernel::Shape;
    let box20 = |h: f64| {
        let d = enc::prof(&[&[enc::line(0.0, 0.0, 10.0, 0.0), enc::line(10.0, 0.0, 10.0, 10.0), enc::line(10.0, 10.0, 0.0, 10.0), enc::line(0.0, 10.0, 0.0, 0.0)]]);
        Shape::extrude_profile(&d, h).expect("the cube")
    };
    let base = box20(20.0).tessellate(0.4)[0].1.len(); // six faces
    // the frame sits at the centre of the top face with Z pointing outward, and the tool goes down into the
    // body
    let pl = [1.0, 0.0, 0.0, 5.0, 0.0, 1.0, 0.0, 5.0, 0.0, 0.0, 1.0, 20.0];
    let cb = box20(20.0).hole_stepped(1, pl, 4.0, 15.0, 8.0, 5.0, &[]).expect("the counterbore");
    assert!(cb.tessellate(0.4)[0].1.len() > base, "the counterbore added faces");
    let cs = box20(20.0).hole_stepped(2, pl, 4.0, 15.0, 8.0, 3.0, &[]).expect("the countersink");
    assert!(cs.tessellate(0.4)[0].1.len() > base, "the countersink added faces");
    assert!(box20(20.0).hole_stepped(0, pl, 4.0, 15.0, 0.0, 0.0, &[]).is_some(), "a plain hole cuts");
}

#[test]
fn revolve_around_offset_axis_makes_ring() {
    // a profile revolved about an arbitrary axis rather than only X or Y through the origin; the square spans
    // x in [2, 6] and y in [0, 4]
    use qymcad_kernel::Shape;
    let d = enc::prof(&[&[enc::line(2.0, 0.0, 6.0, 0.0), enc::line(6.0, 0.0, 6.0, 4.0), enc::line(6.0, 4.0, 2.0, 4.0), enc::line(2.0, 4.0, 2.0, 0.0)]]);
    // about the world Y through the origin: a valid body
    let a = Shape::revolve_profile_axis(&d, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0], 360.0).expect("the revolve about Y");
    assert!(a.tessellate(0.3)[0].0.tris.len() > 20, "the ring is built");
    // about a parallel axis offset to x = -2: also a valid body, a ring of larger radius
    let b = Shape::revolve_profile_axis(&d, [-2.0, 0.0, 0.0], [0.0, 1.0, 0.0], 360.0).expect("the revolve about the offset axis");
    assert!(b.tessellate(0.3)[0].0.tris.len() > 20, "an offset axis gives a body");
}

#[test]
fn sweep_square_along_z_makes_prism() {
    // A 4×4 square in the XY plane at z = 0 swept along a straight path 20 up the world Z, giving a prism. The
    // profile is perpendicular to the path at the start, so the result is a solid body.
    use qymcad_kernel::Shape;
    // the profile: a square from -2 to 2 centred on the origin, of four exact segments
    let prof = enc::prof(&[&[enc::line(-2.0, -2.0, 2.0, -2.0), enc::line(2.0, -2.0, 2.0, 2.0), enc::line(2.0, 2.0, -2.0, 2.0), enc::line(-2.0, 2.0, -2.0, -2.0)]]);
    // the path: one contour of a single segment from (0,0) to (0,20) in the local plane of the path
    let path = enc::prof(&[&[enc::line(0.0, 0.0, 0.0, 20.0)]]);
    let ident = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]; // the profile in the world XY
    // the placement of the path sends the local Y to the world +Z, so the segment runs up along Z
    let path_tf = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let s = Shape::sweep_profile(&prof, &ident, &path, &path_tf).expect("the sweep built");
    let (m, _f) = s.tessellate(0.3).remove(0);
    let b = m.bounds().expect("bbox");
    assert!((b.max.x - b.min.x - 4.0).abs() < 1e-2, "the section along X is 4: {:?}", (b.min.x, b.max.x));
    assert!((b.max.y - b.min.y - 4.0).abs() < 1e-2, "the section along Y is 4: {:?}", (b.min.y, b.max.y));
    assert!((b.max.z - b.min.z - 20.0).abs() < 1e-2, "the length of the sweep along Z is 20: {:?}", (b.min.z, b.max.z));
}

#[test]
fn sweep_auto_transports_profile_to_path_start() {
    // Automatic orientation: the profile, a 4×4 square in the world XY with a +Z normal, is not aligned to the
    // path by hand. The path runs 30 along the world +X, its placement being the identity. The kernel puts the
    // profile perpendicular to the path at the start itself, giving a 30 by 4 by 4 block; the body has to be
    // valid and the bounding box right.
    use qymcad_kernel::Shape;
    let prof = enc::prof(&[&[enc::line(-2.0, -2.0, 2.0, -2.0), enc::line(2.0, -2.0, 2.0, 2.0), enc::line(2.0, 2.0, -2.0, 2.0), enc::line(-2.0, 2.0, -2.0, -2.0)]]);
    // the path: a segment from (0,0) to (30,0) in the local plane, placed by the identity, so it lies along X
    // in the world XY
    let path = enc::prof(&[&[enc::line(0.0, 0.0, 30.0, 0.0)]]);
    let ident = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let s = Shape::sweep_profile(&prof, &ident, &path, &ident).expect("the sweep with automatic orientation built");
    let (m, _f) = s.tessellate(0.3).remove(0);
    let b = m.bounds().expect("bbox");
    // along the path, X, the length is 30; across it the section is 4 by 4, the profile having turned
    // perpendicular to the path
    assert!((b.max.x - b.min.x - 30.0).abs() < 1e-1, "the length along the path, X, is 30: {:?}", (b.min.x, b.max.x));
    assert!((b.max.y - b.min.y - 4.0).abs() < 1e-1, "the section along Y is 4: {:?}", (b.min.y, b.max.y));
    assert!((b.max.z - b.min.z - 4.0).abs() < 1e-1, "the section along Z is 4: {:?}", (b.min.z, b.max.z));
}

#[test]
fn loft_two_squares_makes_frustum() {
    // a loft through two square sections — 10×10 at z = 0 and 4×4 at z = 20 — giving a truncated pyramid
    use qymcad_kernel::Shape;
    let sq = |h: f64| {
        // a square from -h to h as a loop block: a count of four, then four lines
        enc::one_loop(&[enc::line(-h, -h, h, -h), enc::line(h, -h, h, h), enc::line(h, h, -h, h), enc::line(-h, h, -h, -h)])
    };
    let bottom = sq(5.0); // 10×10
    let top = sq(2.0); // 4×4
    let mut data = Vec::new();
    let mut offsets = vec![0usize];
    data.extend_from_slice(&bottom);
    offsets.push(data.len());
    data.extend_from_slice(&top);
    offsets.push(data.len());
    // the placements: the identity for the bottom at z = 0, and a shift of 20 along z for the top
    let mut places = vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    places.extend_from_slice(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 20.0]);
    let s = Shape::loft_sections(&data, &offsets, &places, LoftWalls::Smooth, LoftBody::Solid).expect("the loft built");
    let (m, _f) = s.tessellate(0.3).remove(0);
    let b = m.bounds().expect("bbox");
    assert!(m.tris.len() > 8, "a truncated pyramid has faces: {}", m.tris.len());
    assert!((b.max.x - b.min.x - 10.0).abs() < 1e-1, "the bottom along X is 10: {:?}", (b.min.x, b.max.x));
    assert!((b.max.y - b.min.y - 10.0).abs() < 1e-1, "the bottom along Y is 10: {:?}", (b.min.y, b.max.y));
    assert!((b.max.z - b.min.z - 20.0).abs() < 1e-1, "the height is 20: {:?}", (b.min.z, b.max.z));
}

#[test]
fn boolean_cut_makes_hole() {
    // a 20×20 plate extruded by 5 with a round column subtracted, giving a body with a hole
    let base = [0.0, 0.0, 20.0, 0.0, 20.0, 20.0, 0.0, 20.0];
    // an octagon at the centre standing in for a drill
    let mut tool = Vec::new();
    for k in 0..8 {
        let a = (k as f64) * std::f64::consts::TAU / 8.0;
        tool.push(10.0 + 3.0 * a.cos());
        tool.push(10.0 + 3.0 * a.sin());
    }
    let res = qymcad_kernel::extrude_bool(&base, 5.0, &tool, 9.0, 0, 0.5).expect("the cut succeeded");
    assert_eq!(res.len(), 1);
    let (m, _f) = &res[0];
    assert!(m.tris.len() > 12, "a plate with a hole has more triangles than a whole one: {}", m.tris.len());
}

#[test]
fn shape_boolean_cuts() {
    use qymcad_kernel::Shape;
    let base = Shape::extrude(&[0.0, 0.0, 20.0, 0.0, 20.0, 20.0, 0.0, 20.0], 5.0).expect("base");
    let mut tool = Vec::new();
    for k in 0..8 {
        let a = (k as f64) * std::f64::consts::TAU / 8.0;
        tool.push(10.0 + 3.0 * a.cos());
        tool.push(10.0 + 3.0 * a.sin());
    }
    let tool = Shape::extrude(&tool, 9.0).expect("tool");
    let cut = base.boolean(&tool, 0).expect("cut");
    let bodies = cut.tessellate(0.5);
    assert!(!bodies.is_empty() && bodies[0].0.tris.len() > 12, "a body with a hole");
}

#[test]
fn exact_circle_extrudes_to_true_cylinder() {
    // An exact profile, the circle being a real edge, gives a cylinder of exactly three faces — bottom, top and
    // one cylindrical — rather than a faceted prism of dozens.
    use qymcad_kernel::Shape;
    let r = 5.0;
    let data = enc::prof(&[&[enc::circle(0.0, 0.0, r)]]);
    let s = Shape::extrude_profile(&data, 10.0).expect("the extrusion of the exact profile");
    let bodies = s.tessellate(0.1);
    assert_eq!(bodies.len(), 1, "one body");
    let (m, faces) = &bodies[0];
    assert_eq!(faces.len(), 3, "an exact cylinder is three B-rep faces — bottom, top and side — not a faceted one: {}", faces.len());
    let b = m.bounds().expect("the bounding box");
    assert!((b.max.x - b.min.x - 2.0 * r).abs() < 0.2, "the diameter is about 10: {}", b.max.x - b.min.x);
    assert!((b.max.z - b.min.z - 10.0).abs() < 1e-6, "the height is 10");
}

#[test]
fn exact_square_extrudes_to_six_faced_box() {
    // four segments give a cuboid of exactly six faces, the topology of the lines being exact
    use qymcad_kernel::Shape;
    // four straight edges around a 10×10 square
    let data = enc::prof(&[&[enc::line(0.0, 0.0, 10.0, 0.0), enc::line(10.0, 0.0, 10.0, 10.0), enc::line(10.0, 10.0, 0.0, 10.0), enc::line(0.0, 10.0, 0.0, 0.0)]]);
    let s = Shape::extrude_profile(&data, 5.0).expect("the extrusion of the square");
    let bodies = s.tessellate(0.5);
    assert_eq!(bodies[0].1.len(), 6, "a cuboid is six faces: {}", bodies[0].1.len());
}

#[test]
fn exact_ring_extrudes_with_cylindrical_hole() {
    // an outer circle of R = 10 with a hole of R = 4 gives a tube of four faces: two annular ends and two
    // cylinders
    use qymcad_kernel::Shape;
    let data = enc::prof(&[&[enc::circle(0.0, 0.0, 10.0)], &[enc::circle(0.0, 0.0, 4.0)]]);
    let s = Shape::extrude_profile(&data, 8.0).expect("the tube");
    let bodies = s.tessellate(0.1);
    assert_eq!(bodies[0].1.len(), 4, "a tube is four exact faces — two annular ends and the outer and inner cylinders: {}", bodies[0].1.len());
}

#[test]
fn exact_primitives_have_true_brep_faces() {
    use qymcad_kernel::Shape;
    // a Ø10 by 20 cylinder is three faces: bottom, top and side
    let cyl = Shape::cylinder(5.0, 20.0).expect("the cylinder").tessellate(0.1);
    assert_eq!(cyl[0].1.len(), 3, "a cylinder is three faces: {}", cyl[0].1.len());
    // a sphere is one face
    let sph = Shape::sphere(7.0).expect("the sphere").tessellate(0.1);
    assert_eq!(sph[0].1.len(), 1, "a sphere is one face: {}", sph[0].1.len());
    // a truncated cone is three faces: bottom, top and side
    let cone = Shape::cone(6.0, 3.0, 10.0).expect("the cone").tessellate(0.1);
    assert_eq!(cone[0].1.len(), 3, "a truncated cone is three faces: {}", cone[0].1.len());
    // a torus is one face
    let tor = Shape::torus(10.0, 3.0).expect("the torus").tessellate(0.2);
    assert_eq!(tor[0].1.len(), 1, "a torus is one face: {}", tor[0].1.len());
}

#[test]
fn persistent_face_ids_survive_boolean_and_param_change() {
    use qymcad_kernel::Shape;
    use std::collections::HashSet;
    let sq = |h: f64| {
        let d = enc::prof(&[&[enc::line(0.0, 0.0, 10.0, 0.0), enc::line(10.0, 0.0, 10.0, 10.0), enc::line(10.0, 10.0, 0.0, 10.0), enc::line(0.0, 10.0, 0.0, 0.0)]]);
        Shape::extrude_profile(&d, h).expect("the cube")
    };
    // the base face ids of the cuboid are non-zero, six of them
    let base_ids: HashSet<u32> = sq(10.0).tessellate(0.5).remove(0).1.iter().map(|f| f.id).collect();
    assert_eq!(base_ids.len(), 6, "six faces with unique ids: {base_ids:?}");
    assert!(!base_ids.contains(&0), "every id is non-zero, coming from the B-rep: {base_ids:?}");
    // stability under a change of parameter: a different height gives the same set of ids
    let base_ids2: HashSet<u32> = sq(20.0).tessellate(0.5).remove(0).1.iter().map(|f| f.id).collect();
    assert_eq!(base_ids, base_ids2, "the face ids are stable under a change of height");
    // cutting a hole: the base faces keep their ids and a new wall face appears
    let circ = [1.0, 1.0, 2.0, 2.0, 0.0, 0.0, 0.0, 5.0, 5.0, 0.0]; // a circle of R = 2 at the centre
    let tool = Shape::extrude_profile(&circ, 10.0).expect("the tool");
    let cut = sq(10.0).boolean(&tool, 0).expect("the cut");
    let cut_ids: HashSet<u32> = cut.tessellate(0.2).remove(0).1.iter().map(|f| f.id).collect();
    assert!(base_ids.iter().all(|id| cut_ids.contains(id)), "the faces of the cuboid kept their ids after the cut: {base_ids:?} before, {cut_ids:?} after");
    assert!(cut_ids.len() > base_ids.len(), "a new wall face of the hole appeared with a new id: {cut_ids:?}");
}

#[test]
fn persistent_edge_ids_and_fillet_by_id() {
    use qymcad_kernel::Shape;
    use std::collections::HashSet;
    let sq = |h: f64| {
        let d = enc::prof(&[&[enc::line(0.0, 0.0, 10.0, 0.0), enc::line(10.0, 0.0, 10.0, 10.0), enc::line(10.0, 10.0, 0.0, 10.0), enc::line(0.0, 10.0, 0.0, 0.0)]]);
        Shape::extrude_profile(&d, h).expect("the cube")
    };
    let ids = |h: f64| -> HashSet<u32> { sq(h).edges_with_ids().1.into_iter().collect() };
    let e10 = ids(10.0);
    assert_eq!(e10.len(), 12, "a cuboid has 12 edges with unique ids: {e10:?}");
    assert!(!e10.contains(&0), "every edge id is non-zero: {e10:?}");
    assert_eq!(e10, ids(20.0), "the edge ids are stable under a change of height");
    // filleting an edge by id gives a new face: the id survives the selection
    let some = *e10.iter().next().unwrap();
    let filleted = sq(10.0).fillet_edges(1.0, &[some]).expect("the fillet of the edge by id");
    assert!(filleted.tessellate(0.2)[0].1.len() > 6, "filleting the edge added faces");
    // a variable fillet from r1 to r2 on the same edge builds, the radius changing along it
    let var = sq(10.0).fillet_var(0.5, 3.0, &[some]).expect("the variable fillet from 0.5 to 3");
    assert!(var.tessellate(0.2)[0].1.len() > 6, "the variable fillet added faces");
    assert!(sq(10.0).fillet_var(1.0, 1.0, &[999999]).is_none(), "a non-existent edge gives None");
}

#[test]
fn merged_tessellation_keeps_all_disjoint_solids() {
    // A pattern or a mirror gives a compound of disjoint solids, and `tessellate_merged` has to return them
    // all; otherwise a body node would show only the first one, that is, a single copy.
    use qymcad_kernel::Shape;
    let sq = |ox: f64| {
        let (x0, x1) = (ox, ox + 10.0);
        let d = enc::prof(&[&[enc::line(x0, 0.0, x1, 0.0), enc::line(x1, 0.0, x1, 10.0), enc::line(x1, 10.0, x0, 10.0), enc::line(x0, 10.0, x0, 0.0)]]);
        Shape::extrude_profile(&d, 10.0).expect("the cube")
    };
    // the union of two disjoint cuboids, over x in [0, 10] and x in [30, 40]
    let two = sq(0.0).boolean(&sq(30.0), 1).expect("union");
    // one solid is six faces, so the merged tessellation has to give twelve, both cuboids
    let (_mesh, faces) = two.tessellate_merged(0.5).expect("merged");
    assert_eq!(faces.len(), 12, "both disjoint cuboids are in the merged mesh: {} faces", faces.len());
    // the naive first-solid version would have given six
    assert_eq!(two.tessellate(0.5).len(), 2, "the compound really holds two solids");
}

#[test]
fn mirror_about_arbitrary_plane() {
    // a mirror about an arbitrary plane, given as an origin and a normal: a datum or a face
    use qymcad_kernel::Shape;
    let sq = |h: f64| {
        let d = enc::prof(&[&[enc::line(0.0, 0.0, 10.0, 0.0), enc::line(10.0, 0.0, 10.0, 10.0), enc::line(10.0, 10.0, 0.0, 10.0), enc::line(0.0, 10.0, 0.0, 0.0)]]);
        Shape::extrude_profile(&d, h).expect("the cube")
    };
    // mirrored about the plane x = 20, whose normal is +X and origin [20, 0, 0]
    let m = sq(10.0).mirrored_plane([20.0, 0.0, 0.0], [1.0, 0.0, 0.0]).expect("the mirror about the arbitrary plane");
    assert_eq!(m.tessellate(0.5).remove(0).1.len(), 6, "the mirror of a cuboid has six faces");
}

#[test]
fn shell_by_face_id_and_direction() {
    // a shell by the persistent id of a face, stable across a rebuild, with a signed offset for the direction
    use qymcad_kernel::Shape;
    let sq = |h: f64| {
        let d = enc::prof(&[&[enc::line(0.0, 0.0, 10.0, 0.0), enc::line(10.0, 0.0, 10.0, 10.0), enc::line(10.0, 10.0, 0.0, 10.0), enc::line(0.0, 10.0, 0.0, 0.0)]]);
        Shape::extrude_profile(&d, h).expect("the cube")
    };
    let faces = sq(10.0).tessellate(0.5).remove(0).1;
    let valid_id = faces.iter().map(|f| f.id).max().expect("there are faces");
    assert_ne!(valid_id, 0, "the face has a non-zero persistent id");
    // inward, a negative offset, by a valid face id: a hollow body builds
    assert!(sq(10.0).shell(-2.0, &[valid_id], &[]).is_some(), "the inward shell by face id built");
    // a non-existent id finds no face, so nothing is removed and None comes back, rather than the wrong face
    assert!(sq(10.0).shell(-2.0, &[99999], &[]).is_none(), "a shell by a non-existent id gives None");
    // a zero offset gives None, as a guard
    assert!(sq(10.0).shell(0.0, &[valid_id], &[]).is_none(), "a zero thickness gives None");
}

#[test]
fn draft_tilts_side_face_about_neutral_plane() {
    // A draft: tilt a side face of a 10 mm cube by 10° relative to the neutral plane at z = 0, which holds the
    // bottom still, pulling along +Z. The top edge of the face moves by 10·tan(10°) ≈ 1.763, so the top and the
    // bottom differ along X.
    use qymcad_kernel::Shape;
    let sq = |h: f64| {
        let d = enc::prof(&[&[enc::line(0.0, 0.0, 10.0, 0.0), enc::line(10.0, 0.0, 10.0, 10.0), enc::line(10.0, 10.0, 0.0, 10.0), enc::line(0.0, 10.0, 0.0, 0.0)]]);
        Shape::extrude_profile(&d, h).expect("the cube")
    };
    // find the face whose normal is about +X, tilt that one and measure the span along X
    let faces = sq(10.0).tessellate(0.5).remove(0).1;
    let side = faces.iter().find(|f| f.id != 0 && f.normal[0] > 0.9).map(|f| f.id).expect("there is a +X face");
    let pull = [0.0, 0.0, 1.0];
    let np_o = [0.0, 0.0, 0.0];
    let np_n = [0.0, 0.0, 1.0];
    let s = sq(10.0).draft_faces(&[side], 10.0, pull, np_o, np_n, &[]).expect("the draft built");
    let (m, f2) = s.tessellate(0.3).remove(0);
    assert!(f2.len() >= 6, "the body stayed closed, with at least six faces: {}", f2.len());
    // the width of the section along X at the bottom, z near 0, and at the top, z near 10, has to differ by
    // about 10·tan(10°)
    let span_x = |zlo: f64, zhi: f64| -> f64 {
        let xs: Vec<f64> = m.verts.iter().filter(|p| p.z > zlo && p.z < zhi).map(|p| p.x).collect();
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for x in xs { lo = lo.min(x); hi = hi.max(x); }
        hi - lo
    };
    let bottom_w = span_x(-0.5, 0.5);
    let top_w = span_x(9.5, 10.5);
    let expect = 10.0 * (10.0_f64).to_radians().tan(); // ≈1.763
    assert!((bottom_w - 10.0).abs() < 0.2, "the bottom along X is unchanged, being the neutral plane: {bottom_w}");
    assert!(((bottom_w - top_w).abs() - expect).abs() < 0.3, "the top moved by 10·tan(10°) ≈ {expect:.3}: bottom={bottom_w}, top={top_w}");
    // a non-existent face gives None, and so does an empty list
    assert!(sq(10.0).draft_faces(&[99999], 10.0, pull, np_o, np_n, &[]).is_none(), "no such face gives None");
    assert!(sq(10.0).draft_faces(&[], 10.0, pull, np_o, np_n, &[]).is_none(), "an empty list of faces gives None");
}

#[test]
fn face_axis_of_cylinder_is_z() {
    // Clicking a cylindrical face gives its axis, which is how the axis of a circular pattern is picked. A
    // primitive cylinder runs along +Z from the origin, so its lateral face gives the Z axis through (0, 0).
    // Planar ends have no axis.
    use qymcad_kernel::Shape;
    let cyl = Shape::cylinder(5.0, 20.0).expect("the cylinder");
    let faces = cyl.tessellate(0.2).remove(0).1;
    let mut lateral = None;
    for f in &faces {
        if f.id != 0 {
            if let Some((o, d)) = cyl.face_axis(f.id) {
                lateral = Some((o, d));
            }
        }
    }
    let (o, d) = lateral.expect("the cylinder has a lateral face with an axis");
    assert!(d[2].abs() > 0.999 && d[0].abs() < 1e-6 && d[1].abs() < 1e-6, "the axis of the cylinder is parallel to Z: {d:?}");
    assert!(o[0].abs() < 1e-6 && o[1].abs() < 1e-6, "the axis passes through x = y = 0: {o:?}");
    // a planar end, whose normal is parallel to Z, has no axis
    if let Some(pid) = faces.iter().find(|f| f.id != 0 && f.normal[2].abs() > 0.9).map(|f| f.id) {
        assert!(cyl.face_axis(pid).is_none(), "a planar end has no axis");
    }
    // a non-existent id gives None
    assert!(cyl.face_axis(99999).is_none(), "no such face gives None");
}
