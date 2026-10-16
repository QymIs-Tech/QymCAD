//! THE LIMITING CASES, every tool against every way of landing on the boundary: a blend reaching exactly to an edge,
//! a cut flush with a face, bodies only touching. OCCT degenerates exactly there - a chamfer of 4 on an edge 4 from a
//! step was refused while 3.99999 built - and a person drawing to a dimension lands there on purpose.
//!
//! Live OCCT, headless; failures accumulate and are told together. The oracle of a cell: the node is built, the
//! volume is the one computed by hand, no face of the result is a crack, and a next blend beside it builds.
use qymcad_core::feature::{ChamferMode, Reach};
use qymcad_core::geom::Point2;
use qymcad_core::model::{ChamferShape, Project};
use qymcad_core::refs::Ref;

/// THE STEPPED BAR: an L section extruded 20 along z. The upper step is 4 wide (x 0..4, y 6..10), the lower one runs
/// on to x = 30. The edge at (0, 10) along z is the one blended: a leg of 4 on its top face reaches the edge of the
/// step at (4, 10) exactly.
const LENGTH: f64 = 20.0;
const STEP: f64 = 4.0;
const BAR_VOLUME: f64 = (30.0 * 6.0 + STEP * 4.0) * LENGTH;
/// The faces of the bar: six sides of the section and two ends.
const BAR_FACES: u32 = 8;

fn stepped_bar() -> (Project, u64) {
    let mut p = Project::default();
    p.new_document();
    let pts = [(0.0, 0.0), (30.0, 0.0), (30.0, 6.0), (STEP, 6.0), (STEP, 10.0), (0.0, 10.0)].iter().map(|&(x, y)| Point2::new(x, y)).collect();
    let sid = p.add_line_sketch("Bar", pts, true);
    let si = p.sketch_index(sid).expect("the sketch");
    p.regen_sketch(si);
    p.add_sketch_node(sid, "Bar");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let bar = p.add_extrude_multi(sid, closed, LENGTH, Reach::Forward, 0.0, vec![]);
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the bar did not build: {:?}", report.errors);
    (p, bar)
}

/// The edge of `body` running along z through (x, y).
fn edge_along_z(p: &Project, body: u64, x: f64, y: f64) -> Option<u32> {
    p.regen_edges.get(&body)?.iter().find(|e| (e.mid[0] - x).abs() < 1e-3 && (e.mid[1] - y).abs() < 1e-3 && (e.a[2] - e.b[2]).abs() > 1.0).map(|e| e.id)
}

/// The faces of the body, all of them: the tessellated list leaves out a strip too narrow to draw.
fn face_count(shapes: &std::collections::HashMap<u64, qymcad_kernel::Shape>, body: u64) -> u32 {
    shapes.get(&body).and_then(|s| s.face_kinds()).map(|k| k.iter().sum()).unwrap_or(0)
}

/// A WAY OF BLENDING the edge: what the person picks in the tool, and the material it takes off a sharp edge of
/// the size `size` (both legs on the faces at 90 deg).
struct Blend {
    name: &'static str,
    apply: fn(&mut Project, u64, u32, f64) -> u64,
    removed: fn(f64) -> f64,
}

fn chamfer_area(d: f64) -> f64 {
    d * d / 2.0
}

fn fillet_area(r: f64) -> f64 {
    r * r * (1.0 - std::f64::consts::PI / 4.0)
}

/// An asymmetric chamfer by `mode`: two legs of `d1`, or the leg `d1` at 45 deg - both take `d1` off either face.
fn chamfer_ex(p: &mut Project, src: u64, edge: u32, d1: f64, mode: ChamferMode) -> u64 {
    let d2 = if mode == ChamferMode::DistAngle { 45.0 } else { d1 };
    p.add_chamfer_ex(src, d1, ChamferShape { mode, d2, flip: false, ref_face: 0 }, vec![edge])
}

const BLENDS: [Blend; 5] = [
    Blend { name: "symmetric chamfer", apply: |p, src, e, d| p.add_chamfer(src, d, vec![e]), removed: chamfer_area },
    Blend { name: "chamfer by two legs", apply: |p, src, e, d| chamfer_ex(p, src, e, d, ChamferMode::TwoDist), removed: chamfer_area },
    Blend { name: "chamfer by a leg and an angle", apply: |p, src, e, d| chamfer_ex(p, src, e, d, ChamferMode::DistAngle), removed: chamfer_area },
    Blend { name: "fillet of one radius", apply: |p, src, e, r| p.add_fillet(src, r, vec![e]), removed: fillet_area },
    Blend {
        name: "fillet with radii at the vertices",
        apply: |p, src, e, r| {
            let ends: Vec<(u32, f64)> = p.vertex_pool(src).into_iter().filter(|c| c.centroid[0].abs() < 1e-3 && (c.centroid[1] - 10.0).abs() < 1e-3).map(|c| (c.desc, r)).collect();
            p.add_fillet_at_vertices(src, r, Ref::picks(&[e]), ends)
        },
        removed: fillet_area,
    },
];

/// HOW THE BLEND LANDS on the face it runs across: its size, and how many faces the clean result has.
struct Landing {
    name: &'static str,
    size: f64,
    faces: u32,
}

const LANDINGS: [Landing; 3] = [
    // the control: well inside the face, one blend face added
    Landing { name: "inside the face", size: 3.0, faces: BAR_FACES + 1 },
    // the top face of the step is eaten whole and the blend face takes its place
    Landing { name: "exactly to the edge", size: STEP, faces: BAR_FACES },
    // a strip of 1e-6 stays - a real face, narrower than nothing a person draws, but geometry all the same
    Landing { name: "1e-6 short of the edge", size: STEP - 1e-6, faces: BAR_FACES + 1 },
];

#[test]
fn a_blend_lands_on_the_edge_of_a_step() {
    let mut fails: Vec<String> = Vec::new();
    for blend in &BLENDS {
        for landing in &LANDINGS {
            let cell = format!("{} {} ({})", blend.name, landing.size, landing.name);
            let (mut p, bar) = stepped_bar();
            let Some(edge) = edge_along_z(&p, bar, 0.0, 10.0) else {
                fails.push(format!("{cell}: setup - no edge at (0, 10)"));
                continue;
            };
            let node = (blend.apply)(&mut p, bar, edge, landing.size);
            let (report, shapes) = qymcad_testkit::regenerate(&mut p);
            if let Some((_, e)) = report.errors.iter().find(|(id, _)| *id == node) {
                fails.push(format!("{cell}: refused - {e:?}"));
                continue;
            }
            let Some(shape) = shapes.get(&node) else {
                fails.push(format!("{cell}: no body"));
                continue;
            };
            let want = BAR_VOLUME - (blend.removed)(landing.size) * LENGTH;
            if (shape.volume() - want).abs() > 1e-3 * want {
                fails.push(format!("{cell}: volume {:.4}, expected {want:.4}", shape.volume()));
            }
            let cracks = shape.sliver_faces();
            if cracks > 0 {
                fails.push(format!("{cell}: {cracks} crack face(s) left"));
            }
            let faces = face_count(&shapes, node);
            if landing.size == STEP && faces != landing.faces {
                fails.push(format!("{cell}: {faces} faces, the clean result has {}", landing.faces));
            }
            // THE NEXT BLEND BESIDE: the edge where the step goes down at x = 4, now one side of the first blend
            let Some(next) = edge_along_z(&p, node, STEP, 6.0) else {
                fails.push(format!("{cell}: no edge at ({STEP}, 6) to blend next"));
                continue;
            };
            let after = p.add_fillet(node, 1.0, vec![next]);
            let (report, _) = qymcad_testkit::regenerate(&mut p);
            if let Some((_, e)) = report.errors.iter().find(|(id, _)| *id == after) {
                fails.push(format!("{cell}: the fillet R1 beside it refused - {e:?}"));
            }
        }
    }
    assert!(fails.is_empty(), "\nBLENDS AT THE EDGE OF A STEP ({}):\n{}", fails.len(), fails.join("\n"));
}

/// THE BLOCK the cuts and bosses work on: 40 x 30 x 10, centred on the origin in plan, from z = 0 up.
const BLOCK_VOLUME: f64 = 40.0 * 30.0 * 10.0;

/// A CUT OR A BOSS FROM THE BOTTOM OF THE BLOCK: a rectangle on XY (z = 0), taken up into the block (a cut) or down
/// out of it (a boss), and what the clean result is.
struct FromTheBottom {
    name: &'static str,
    /// The rectangle, two corners.
    from: Point2,
    to: Point2,
    /// How far the profile goes: up into the block for a cut, down from it for a boss.
    height: f64,
    what: Combine,
    volume: f64,
    faces: u32,
}

/// What the profile does to the block.
#[derive(Clone, Copy, PartialEq)]
enum Combine {
    /// Taken up into the block and out of it.
    Cut,
    /// Taken down out of the block and joined to it.
    Boss,
}

impl Combine {
    /// The operation code of the combine node.
    fn code(self) -> u8 {
        match self {
            Combine::Cut => 0,
            Combine::Boss => 1,
        }
    }
}

const fn p2(x: f64, y: f64) -> Point2 {
    Point2 { x, y }
}

const CUTS: [FromTheBottom; 6] = [
    // as deep as the block is thick: a hole right through, its far end flush with the top
    FromTheBottom { name: "a pocket exactly as deep as the block", from: p2(-5.0, -5.0), to: p2(5.0, 5.0), height: 10.0, what: Combine::Cut, volume: BLOCK_VOLUME - 1000.0, faces: 10 },
    // one side of the slot on the front face of the block: the front split in two, three walls of the slot
    FromTheBottom { name: "a slot through, flush with the front", from: p2(-5.0, -15.0), to: p2(5.0, -5.0), height: 10.0, what: Combine::Cut, volume: BLOCK_VOLUME - 1000.0, faces: 10 },
    // two sides on the faces of the corner: a step 5 high in the corner
    FromTheBottom { name: "a step flush in the corner", from: p2(10.0, 5.0), to: p2(20.0, 15.0), height: 5.0, what: Combine::Cut, volume: BLOCK_VOLUME - 500.0, faces: 9 },
    // a boss standing on the bottom face: it touches the block along a face and joins it
    FromTheBottom { name: "a boss touching the bottom", from: p2(-5.0, -5.0), to: p2(5.0, 5.0), height: 5.0, what: Combine::Boss, volume: BLOCK_VOLUME + 500.0, faces: 11 },
    // a boss whose front is the front of the block carried down: the two fronts one face
    FromTheBottom { name: "a boss flush with the front", from: p2(-5.0, -15.0), to: p2(5.0, -5.0), height: 5.0, what: Combine::Boss, volume: BLOCK_VOLUME + 500.0, faces: 10 },
    // a boss 1e-6 short of the front: its front a face of its own, 1e-6 behind that of the block
    FromTheBottom { name: "a boss 1e-6 short of the front", from: p2(-5.0, -15.0 + 1e-6), to: p2(5.0, -5.0), height: 5.0, what: Combine::Boss, volume: BLOCK_VOLUME + 500.0, faces: 11 },
];

#[test]
fn a_cut_or_a_boss_lands_flush() {
    let mut fails: Vec<String> = Vec::new();
    for cell in &CUTS {
        let mut p = Project::default();
        p.new_document();
        let block = p.add_box(40.0, 30.0, 10.0);
        let si = p.new_sketch("Tool");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "Tool");
        p.add_rect_entity(si, cell.from.x, cell.from.y, cell.to.x, cell.to.y, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let c: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
        let reach = if cell.what == Combine::Cut { Reach::Forward } else { Reach::Backward };
        let span = qymcad_core::model::CombineSpan { height: cell.height, down: 0.0, extent: qymcad_core::feature::Extent { reach, ..Default::default() }, fill: &[] };
        let node = p.add_combine_multi_op(block, sid, c, span, cell.what.code());
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        if let Some((_, e)) = report.errors.iter().find(|(id, _)| *id == node) {
            fails.push(format!("{}: refused - {e:?}", cell.name));
            continue;
        }
        let Some(shape) = shapes.get(&node) else {
            fails.push(format!("{}: no body", cell.name));
            continue;
        };
        if (shape.volume() - cell.volume).abs() > 1e-3 {
            fails.push(format!("{}: volume {:.6}, expected {:.6}", cell.name, shape.volume(), cell.volume));
        }
        if shape.solid_count() != 1 {
            fails.push(format!("{}: {} solids, a part is one", cell.name, shape.solid_count()));
        }
        let cracks = shape.sliver_faces();
        if cracks > 0 {
            fails.push(format!("{}: {cracks} crack face(s) left", cell.name));
        }
        let faces = face_count(&shapes, node);
        if faces != cell.faces {
            fails.push(format!("{}: {faces} faces, the clean result has {}", cell.name, cell.faces));
        }
        // THE NEXT OPERATION: a fillet of the upright edge at the corner the cut leaves alone
        let Some(edge) = edge_along_z(&p, node, -20.0, -15.0) else {
            fails.push(format!("{}: no edge at (-20, -15) to round next", cell.name));
            continue;
        };
        let after = p.add_fillet(node, 1.0, vec![edge]);
        let (report, _) = qymcad_testkit::regenerate(&mut p);
        if let Some((_, e)) = report.errors.iter().find(|(id, _)| *id == after) {
            fails.push(format!("{}: the fillet R1 after it refused - {e:?}", cell.name));
        }
    }
    assert!(fails.is_empty(), "\nCUTS AND BOSSES FLUSH ({}):\n{}", fails.len(), fails.join("\n"));
}

/// WHAT A CELL ON THE BLOCK GIVES when it is right.
enum Expect {
    /// One solid of this volume; and, where it is known, this many faces.
    Builds { volume: f64, faces: Option<u32> },
    /// Nothing to build, said in words: an error of the node other than a broken body.
    Refused,
    /// Nothing to build, said in these words.
    Says(qymcad_core::errors::CoreError),
}

/// AN OPERATION ON THE BLOCK 40 x 30 x 10 landing on its boundary: `apply` lays the node and gives its id.
struct OnTheBlock {
    name: &'static str,
    kind: &'static str,
    apply: fn(&mut Project, u64) -> u64,
    expect: Expect,
}

/// The key of the face of `body` whose normal is `normal`.
fn face_key(p: &Project, body: u64, normal: [f64; 3]) -> qymcad_core::feature::FaceKey {
    let f = p.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| (0..3).all(|i| (f.normal[i] - normal[i]).abs() < 1e-6))).cloned().expect("the face of that normal");
    qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
}

/// The id of the face of `body` whose normal is `normal`.
fn face_id(p: &Project, body: u64, normal: [f64; 3]) -> u32 {
    face_key(p, body, normal).id
}

const UP: [f64; 3] = [0.0, 0.0, 1.0];

/// A sketch on XY holding a rectangle, and its closed profiles.
fn rect_sketch(p: &mut Project, from: Point2, to: Point2) -> (u64, Vec<u64>) {
    let si = p.new_sketch("Rect");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Rect");
    p.add_rect_entity(si, from.x, from.y, to.x, to.y, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    (sid, p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect())
}

/// The round edge of `body` that stands highest: the rim a thread or an auger starts from.
fn top_rim(p: &mut Project, body: u64) -> u32 {
    let _ = qymcad_testkit::regenerate(p);
    let rims = p.regen_edges.get(&body).into_iter().flatten().filter(|e| e.radius > 1e-6);
    rims.max_by(|a, b| a.mid[2].total_cmp(&b.mid[2])).map(|e| e.id).expect("a round edge")
}

const ON_THE_BLOCK: [OnTheBlock; 38] = [
    // a hole 10 across exactly as deep as the block: through, no cap of the bottom left in it
    OnTheBlock {
        name: "a hole exactly as deep as the block",
        kind: "Hole",
        apply: |p, b| {
            let key = face_key(p, b, UP);
            p.add_hole(b, key, 10.0, 10.0)
        },
        expect: Expect::Builds { volume: BLOCK_VOLUME - std::f64::consts::PI * 25.0 * 10.0, faces: Some(7) },
    },
    // three copies of the block 40 apart: each touches the next along a face, and together they are one bar 120 long
    OnTheBlock {
        name: "copies touching face to face",
        kind: "LinearArray",
        apply: |p, b| p.add_linear_array(b, 40.0, 0.0, 0.0, 3),
        expect: Expect::Builds { volume: 3.0 * BLOCK_VOLUME, faces: Some(6) },
    },
    // walls 15 thick inside a block 30 wide meet in the middle: nothing is left hollow
    OnTheBlock {
        name: "a shell as thick as half the width",
        kind: "Shell",
        apply: |p, b| {
            let top = face_id(p, b, UP);
            p.add_shell(b, 15.0, vec![top], false)
        },
        expect: Expect::Refused,
    },
    // a floor as thick as the block is tall: nothing is left hollow
    OnTheBlock {
        name: "a shell as thick as the block is tall",
        kind: "Shell",
        apply: |p, b| {
            let top = face_id(p, b, UP);
            p.add_shell(b, 10.0, vec![top], false)
        },
        expect: Expect::Refused,
    },
    // the plane of the split lies on the bottom face: there is nothing on one side of it
    OnTheBlock { name: "a split on the plane of the bottom face", kind: "SplitBody", apply: |p, b| p.add_split_body(b, 0, 0, 0.0, 2)[0], expect: Expect::Refused },
    // a size of zero: nothing to build, for every primitive
    OnTheBlock { name: "a box of no height", kind: "Box3", apply: |p, _| p.add_box(10.0, 10.0, 0.0), expect: Expect::Says(qymcad_core::errors::CoreError::PrimitiveSizeNotAboveZero) },
    OnTheBlock { name: "a cylinder of no radius", kind: "Cylinder", apply: |p, _| p.add_cylinder(0.0, 10.0), expect: Expect::Says(qymcad_core::errors::CoreError::PrimitiveSizeNotAboveZero) },
    OnTheBlock { name: "a sphere of no radius", kind: "Sphere", apply: |p, _| p.add_sphere(0.0), expect: Expect::Says(qymcad_core::errors::CoreError::PrimitiveSizeNotAboveZero) },
    OnTheBlock { name: "a prism of no height", kind: "Prism", apply: |p, _| p.add_prism(10.0, 6, 0.0), expect: Expect::Says(qymcad_core::errors::CoreError::PrimitiveSizeNotAboveZero) },
    // a cone closed to a point at the top: a base and the cone, a third of the cylinder
    OnTheBlock {
        name: "a cone closed to a point",
        kind: "Cone",
        apply: |p, _| p.add_cone(10.0, 0.0, 10.0),
        expect: Expect::Builds { volume: std::f64::consts::PI * 100.0 * 10.0 / 3.0, faces: Some(2) },
    },
    OnTheBlock { name: "a cone of no height", kind: "Cone", apply: |p, _| p.add_cone(10.0, 5.0, 0.0), expect: Expect::Says(qymcad_core::errors::CoreError::PrimitiveSizeNotAboveZero) },
    OnTheBlock { name: "a torus whose tube has no thickness", kind: "Torus", apply: |p, _| p.add_torus(10.0, 0.0), expect: Expect::Says(qymcad_core::errors::CoreError::PrimitiveSizeNotAboveZero) },
    // a tube as thick as the ring passes through the axis
    OnTheBlock { name: "a torus whose tube is as thick as its ring", kind: "Torus", apply: |p, _| p.add_torus(10.0, 10.0), expect: Expect::Refused },
    // a profile extruded by nothing
    OnTheBlock {
        name: "a profile extruded by nothing",
        kind: "Extrude",
        apply: |p, _| {
            let (sid, c) = rect_sketch(p, p2(0.0, 0.0), p2(10.0, 10.0));
            p.add_extrude_multi(sid, c, 0.0, Reach::Forward, 0.0, vec![])
        },
        expect: Expect::Refused,
    },
    // a profile whose side lies on the axis it turns about: a solid cylinder with no hole along the axis
    OnTheBlock {
        name: "a profile turned about its own side",
        kind: "Revolve",
        apply: |p, _| {
            let (sid, c) = rect_sketch(p, p2(0.0, 0.0), p2(10.0, 10.0));
            p.add_revolve_axis(sid, c, 1, 360.0, 0, 0)
        },
        expect: Expect::Builds { volume: std::f64::consts::PI * 100.0 * 10.0, faces: Some(3) },
    },
    // two equal squares, one above the other: the loft is the prism between them
    OnTheBlock {
        name: "a loft between equal sections",
        kind: "Loft",
        apply: |p, _| {
            let (low, lows) = rect_sketch(p, p2(-10.0, -10.0), p2(10.0, 10.0));
            let pl = p.add_plane(qymcad_core::model::WorkPlane {
                id: 0,
                name: "z10".into(),
                origin: [0.0, 0.0, 10.0],
                normal: [0.0, 0.0, 1.0],
                rot_deg: 0.0,
                tilt: Default::default(),
                def: Default::default(),
            });
            let si = p.new_sketch("High");
            let high = p.sketches[si].id;
            p.sketches[si].plane = qymcad_core::feature::SketchPlane::Datum(pl);
            p.add_sketch_node(high, "High");
            p.add_rect_entity(si, -10.0, -10.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
            p.regen_sketch(si);
            let highs: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
            p.add_loft(vec![low, high], vec![lows[0], highs[0]], true, 0, 0, false)
        },
        expect: Expect::Builds { volume: 4000.0, faces: Some(6) },
    },
    // a square swept along a straight path square to it: the prism
    OnTheBlock {
        name: "a sweep along a straight path",
        kind: "Sweep",
        apply: |p, _| {
            let (prof, profiles) = rect_sketch(p, p2(-3.0, -3.0), p2(3.0, 3.0));
            let si = p.new_sketch("Path");
            let path = p.sketches[si].id;
            p.sketches[si].plane = qymcad_core::feature::SketchPlane::World(qymcad_core::feature::BasePlane::XZ);
            p.add_sketch_node(path, "Path");
            p.add_line_entity(si, 0.0, 0.0, 0.0, 40.0, qymcad_core::feature::Purpose::Real);
            p.regen_sketch(si);
            let line = p.sketches[si].contour_ids.first().copied().expect("the path");
            p.add_sweep(prof, profiles, path, line)
        },
        expect: Expect::Builds { volume: 36.0 * 40.0, faces: Some(6) },
    },
    // A TOOL ENDING EXACTLY ON A FACE OF THE BODY IT CUTS, for every kind of profile tool, not only an extrusion: OCCT
    // does not cut through a face it only touches, and leaves a skin over the hole
    // a square 4 x 4 swept from the bottom face exactly to the top: a hole through
    OnTheBlock {
        name: "a sweep cut from the bottom exactly to the top",
        kind: "Sweep",
        apply: |p, b| {
            let (prof, profiles) = rect_sketch(p, p2(-2.0, -2.0), p2(2.0, 2.0));
            let si = p.new_sketch("Path");
            let path = p.sketches[si].id;
            p.sketches[si].plane = qymcad_core::feature::SketchPlane::World(qymcad_core::feature::BasePlane::XZ);
            p.add_sketch_node(path, "Path");
            p.add_line_entity(si, 0.0, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
            p.regen_sketch(si);
            let line = p.sketches[si].contour_ids.first().copied().expect("the path");
            p.add_sweep_multi_op(prof, profiles, path, line, b, 0)
        },
        expect: Expect::Builds { volume: BLOCK_VOLUME - 160.0, faces: Some(10) },
    },
    // two squares 4 x 4, on the bottom face and on the top: the loft between them cuts a hole through
    OnTheBlock {
        name: "a loft cut between the bottom and the top",
        kind: "Loft",
        apply: |p, b| {
            let (low, lows) = rect_sketch(p, p2(-2.0, -2.0), p2(2.0, 2.0));
            let pl = p.add_plane(qymcad_core::model::WorkPlane {
                id: 0,
                name: "z10".into(),
                origin: [0.0, 0.0, 10.0],
                normal: [0.0, 0.0, 1.0],
                rot_deg: 0.0,
                tilt: Default::default(),
                def: Default::default(),
            });
            let si = p.new_sketch("High");
            let high = p.sketches[si].id;
            p.sketches[si].plane = qymcad_core::feature::SketchPlane::Datum(pl);
            p.add_sketch_node(high, "High");
            p.add_rect_entity(si, -2.0, -2.0, 2.0, 2.0, qymcad_core::feature::Purpose::Real);
            p.regen_sketch(si);
            let highs: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
            p.add_loft(vec![low, high], vec![lows[0], highs[0]], true, b, 0, false)
        },
        expect: Expect::Builds { volume: BLOCK_VOLUME - 160.0, faces: Some(10) },
    },
    // a ring groove turned into the top: the profile x 5..8 along its height from z = 6 up to the top face exactly
    OnTheBlock {
        name: "a groove turned down from the top face exactly",
        kind: "Revolve",
        apply: |p, b| {
            let si = p.new_sketch("Groove");
            let sid = p.sketches[si].id;
            p.sketches[si].plane = qymcad_core::feature::SketchPlane::World(qymcad_core::feature::BasePlane::XZ);
            p.add_sketch_node(sid, "Groove");
            p.add_rect_entity(si, 5.0, 6.0, 8.0, 10.0, qymcad_core::feature::Purpose::Real);
            let axis = p.add_line_entity(si, 0.0, -5.0, 0.0, 20.0, qymcad_core::feature::Purpose::Construction);
            p.regen_sketch(si);
            let c: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
            let ax = qymcad_core::model::RevolveAxis { axis: 0, datum: 0, line: axis };
            p.add_revolve_multi_op(sid, c, ax, qymcad_core::model::RevolveTurn { angle: 360.0, reach: Reach::Forward }, b, 0)
        },
        // the top is two faces now - outside the groove and the island inside it - beside the bottom, four sides and
        // the two walls and the floor of the groove
        expect: Expect::Builds { volume: BLOCK_VOLUME - std::f64::consts::PI * (64.0 - 25.0) * 4.0, faces: Some(10) },
    },
    // a draft of no angle tilts nothing
    OnTheBlock {
        name: "a draft of no angle",
        kind: "Draft",
        apply: |p, b| {
            let (front, bottom) = (face_id(p, b, [0.0, -1.0, 0.0]), face_id(p, b, [0.0, 0.0, -1.0]));
            p.add_draft(b, vec![front], bottom, 0.0, false)
        },
        expect: Expect::Refused,
    },
    // the top pushed down onto the bottom: no thickness left
    OnTheBlock {
        name: "the top pushed down onto the bottom",
        kind: "PushFace",
        apply: |p, b| {
            let key = face_key(p, b, UP);
            p.add_push_face(b, key, -10.0)
        },
        expect: Expect::Refused,
    },
    // the top thickened into the body: the plate adds nothing
    OnTheBlock {
        name: "the top thickened into the body",
        kind: "Thicken",
        apply: |p, b| {
            let top = face_id(p, b, UP);
            p.add_thicken(b, top, -2.0)
        },
        expect: Expect::Refused,
    },
    // the faces divided by the plane of the top face: nothing to divide
    OnTheBlock { name: "faces divided on the plane of the top", kind: "SplitFace", apply: |p, b| p.add_split_face(b, 0, 0, 10.0), expect: Expect::Refused },
    // the top of the block taken off: nothing closes it again
    OnTheBlock {
        name: "the top of the block taken off",
        kind: "RemoveFace",
        apply: |p, b| {
            let key = face_key(p, b, UP);
            p.add_remove_face(b, vec![key])
        },
        expect: Expect::Refused,
    },
    // two copies half a turn apart about Z: the block is centred on the axis, the copies coincide
    OnTheBlock { name: "two copies standing in one place", kind: "CircularArray", apply: |p, b| p.add_circular_array(b, 2, 360.0), expect: Expect::Builds { volume: BLOCK_VOLUME, faces: Some(6) } },
    // the image across the plane through the middle: the image is the block itself
    OnTheBlock { name: "a mirror onto itself", kind: "Mirror", apply: |p, b| p.add_mirror(b, 2, true, 0), expect: Expect::Builds { volume: BLOCK_VOLUME, faces: Some(6) } },
    // the part mirrored across its own right face: the image touches it face to face, a part of its own
    OnTheBlock {
        name: "a part mirrored across its own face",
        kind: "MirrorPart",
        apply: |p, b| {
            let part = p.body_owner(b).expect("the part of the block");
            p.add_mirror_part(part, [20.0, 0.0, 0.0], [1.0, 0.0, 0.0])
        },
        expect: Expect::Builds { volume: BLOCK_VOLUME, faces: Some(6) },
    },
    // moved by nothing
    OnTheBlock { name: "a move by nothing", kind: "Move", apply: |p, b| p.add_move(b, qymcad_core::feature::PLACE_IDENTITY), expect: Expect::Builds { volume: BLOCK_VOLUME, faces: Some(6) } },
    // two blocks touching face to face have nothing in common
    OnTheBlock {
        name: "an intersection of blocks touching face to face",
        kind: "BodyBoolean",
        apply: |p, b| {
            let other = p.add_box(40.0, 30.0, 10.0);
            let moved = p.add_move(other, [1.0, 0.0, 0.0, 40.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
            let _ = qymcad_testkit::regenerate(p);
            p.add_body_boolean(b, moved, 2)
        },
        expect: Expect::Refused,
    },
    // the top and the front replaced by a copy of the top alone: the front is left open
    OnTheBlock {
        name: "a sheet that does not close what it replaces",
        kind: "SurfaceReplace",
        apply: |p, b| {
            let (top, front) = (face_id(p, b, UP), face_id(p, b, [0.0, -1.0, 0.0]));
            let sheet = p.add_face_copy(b, Ref::picks(&[top]));
            let _ = qymcad_testkit::regenerate(p);
            p.add_surface_replace(b, Ref::picks(&[top, front]), sheet)
        },
        expect: Expect::Refused,
    },
    // a copy of no face
    OnTheBlock { name: "a copy of no face", kind: "FaceCopy", apply: |p, b| p.add_face_copy(b, Ref::picks(&[])), expect: Expect::Refused },
    // the side of a cylinder of radius 10 moved 10 in: it closes to its axis
    OnTheBlock {
        name: "a round face offset in by its radius",
        kind: "OffsetSurface",
        apply: |p, _| {
            let c = p.add_cylinder(10.0, 10.0);
            let _ = qymcad_testkit::regenerate(p);
            let side = p.regen_faces[&c].iter().find(|f| f.normal[2].abs() < 0.5).map(|f| f.id).expect("the side of the cylinder");
            p.add_offset_surface(c, Ref::picks(&[side]), -10.0)
        },
        expect: Expect::Says(qymcad_core::errors::CoreError::OffsetPastRadius { radius: 10.0 }),
    },
    // one straight edge is no opening to close
    OnTheBlock {
        name: "a patch over one edge",
        kind: "Patch",
        apply: |p, b| {
            let e = p.regen_edges[&b].iter().find(|e| e.a[2] > 9.99 && e.b[2] > 9.99).map(|e| e.id).expect("an edge of the top");
            p.add_patch(b, Ref::picks(&[e]), false)
        },
        expect: Expect::Refused,
    },
    // the top and the bottom are 10 apart: nothing to sew
    OnTheBlock {
        name: "sheets that do not touch stitched",
        kind: "Stitch",
        apply: |p, b| {
            let (top, bottom) = (face_id(p, b, UP), face_id(p, b, [0.0, 0.0, -1.0]));
            let a = p.add_face_copy(b, Ref::picks(&[top]));
            let c = p.add_face_copy(b, Ref::picks(&[bottom]));
            let _ = qymcad_testkit::regenerate(p);
            p.add_stitch(vec![a, c], 1e-4)
        },
        expect: Expect::Refused,
    },
    // a sheet trimmed by a body that does not reach it
    OnTheBlock {
        name: "a sheet trimmed by a body apart from it",
        kind: "Trim",
        apply: |p, b| {
            let top = face_id(p, b, UP);
            let sheet = p.add_face_copy(b, Ref::picks(&[top]));
            let far = p.add_box(10.0, 10.0, 10.0);
            let tool = p.add_move(far, [1.0, 0.0, 0.0, 200.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
            let _ = qymcad_testkit::regenerate(p);
            p.add_trim(sheet, tool, [0.0, 0.0, 10.0])
        },
        expect: Expect::Refused,
    },
    // a thread longer than the round face it is cut in
    OnTheBlock {
        name: "a thread longer than its face",
        kind: "Thread",
        apply: |p, _| {
            let c = p.add_cylinder(10.0, 10.0);
            let rim = top_rim(p, c);
            p.add_thread(c, rim, qymcad_core::thread::ThreadSpec { nominal_d: 20.0, ..Default::default() }, 20.0, 0.0, 0.0)
        },
        expect: Expect::Refused,
    },
    // a flight no wider than its shaft
    OnTheBlock {
        name: "an auger flight no wider than its shaft",
        kind: "Auger",
        apply: |p, _| {
            let c = p.add_cylinder(5.0, 40.0);
            let rim = top_rim(p, c);
            p.add_auger(c, rim, qymcad_core::thread::AugerSpec { outer_d: 10.0, ..Default::default() }, 20.0, 0.0, 0.0)
        },
        expect: Expect::Refused,
    },
];

#[test]
fn an_operation_lands_on_the_boundary_of_the_block() {
    let mut fails: Vec<String> = Vec::new();
    for cell in &ON_THE_BLOCK {
        let mut p = Project::default();
        p.new_document();
        let block = p.add_box(40.0, 30.0, 10.0);
        let _ = qymcad_testkit::regenerate(&mut p);
        let node = (cell.apply)(&mut p, block);
        let made = qymcad_testkit::kind_making(&p, node);
        if made.as_deref() != Some(cell.kind) {
            fails.push(format!("{}: the cell says it lays {}, the node of its body is {made:?}", cell.name, cell.kind));
        }
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        let error = report.errors.iter().find(|(id, _)| *id == node).map(|(_, e)| e.clone());
        match (&cell.expect, error) {
            (Expect::Refused, Some(qymcad_core::errors::CoreError::BrokenSolid)) => fails.push(format!("{}: a broken body instead of a refusal in words", cell.name)),
            (Expect::Refused, Some(_)) => {}
            (Expect::Says(want), Some(e)) if e == *want => {}
            (Expect::Says(want), Some(e)) => fails.push(format!("{}: refused as {e:?}, the words should be {want:?}", cell.name)),
            (Expect::Says(want), None) => fails.push(format!("{}: built where it should say {want:?}", cell.name)),
            (Expect::Refused, None) => {
                let v = shapes.get(&node).map(|s| s.volume()).unwrap_or(0.0);
                fails.push(format!("{}: built ({v:.3} mm^3) where nothing can be", cell.name));
            }
            (Expect::Builds { .. }, Some(e)) => fails.push(format!("{}: refused - {e:?}", cell.name)),
            (Expect::Builds { volume, faces }, None) => {
                let Some(shape) = shapes.get(&node) else {
                    fails.push(format!("{}: no body", cell.name));
                    continue;
                };
                if (shape.volume() - volume).abs() > 1e-3 * volume {
                    fails.push(format!("{}: volume {:.4}, expected {volume:.4}", cell.name, shape.volume()));
                }
                if shape.solid_count() != 1 {
                    fails.push(format!("{}: {} solids, a part is one", cell.name, shape.solid_count()));
                }
                let cracks = shape.sliver_faces();
                if cracks > 0 {
                    fails.push(format!("{}: {cracks} crack face(s) left", cell.name));
                }
                let got = face_count(&shapes, node);
                if faces.is_some_and(|f| f != got) {
                    fails.push(format!("{}: {got} faces, the clean result has {}", cell.name, faces.unwrap_or(0)));
                }
            }
        }
    }
    assert!(fails.is_empty(), "\nOPERATIONS ON THE BOUNDARY OF THE BLOCK ({}):\n{}", fails.len(), fails.join("\n"));
}

/// THE KINDS THE TABLES OF BLENDS AND OF CUTS LAY: the blends on the step, the cuts and bosses from the bottom.
const LAID_BY_THE_OTHER_TABLES: [&str; 3] = ["Fillet", "Chamfer", "Combine"];

/// THE KINDS THIS MATRIX HAS NO CELL FOR, and why.
const NOT_HERE: [qymcad_testkit::Uncovered; 11] = [
    qymcad_testkit::Uncovered { kind: "Sketch", why: "a sketch builds no body" },
    qymcad_testkit::Uncovered { kind: "Plane", why: "a datum plane builds no body" },
    qymcad_testkit::Uncovered { kind: "DatumPoint", why: "a datum point builds no body" },
    qymcad_testkit::Uncovered { kind: "DatumAxis", why: "a datum axis builds no body" },
    qymcad_testkit::Uncovered { kind: "PartInstance", why: "a copy of a part placed in an assembly repeats the body of the part as it is" },
    qymcad_testkit::Uncovered { kind: "ComponentPattern", why: "the copies of a pattern of parts repeat the body of the part as it is" },
    qymcad_testkit::Uncovered { kind: "Piece", why: "making a piece a part moves a built body as it is: it has no size to land on a boundary" },
    qymcad_testkit::Uncovered { kind: "Import", why: "an import has no operation to land on a boundary: what it reads is what the file holds" },
    qymcad_testkit::Uncovered { kind: "MeshPiece", why: "a piece of a mesh has no B-rep and no operation" },
    qymcad_testkit::Uncovered {
        kind: "MeshSolid",
        why: "the limits of a mesh - a hole in it, a triangle of no area - are the matrices of meshes (a_mesh_becomes_a_solid, a_small_triangle_of_a_mesh_is_no_crack)",
    },
    qymcad_testkit::Uncovered { kind: "MeshRecognised", why: "the limits of a mesh are the matrices of the recognition (qymcad-meshfit)" },
];

/// EVERY KIND OF NODE HAS A CELL HERE, or a reason for none.
#[test]
fn every_kind_of_node_has_a_cell() {
    let kinds: Vec<&str> = ON_THE_BLOCK.iter().map(|c| c.kind).chain(LAID_BY_THE_OTHER_TABLES).collect();
    let gaps = qymcad_testkit::coverage_gaps(&kinds, &NOT_HERE);
    assert!(gaps.is_empty(), "the matrix of limits leaves kinds of node unseen:\n{}", gaps.join("\n"));
}
