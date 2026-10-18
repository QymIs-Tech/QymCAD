//! EVERY FACE, EDGE AND VERTEX AN OPERATION MAKES CARRIES A NAME BY RECIPE, not a positional number. A reference - a
//! sketch on a face, a fillet of an edge, a mate - holds a name; a positional number is the place the element happened
//! to take in the walk of the result, and an edit above moves it to another face without a word.
//!
//! Measured 12.10.2026 over every operation built on a block extruded from a rectangle: the box and the prism
//! primitives named none of their faces and edges, the offset surface and the face copy none of their edges. Live
//! OCCT, headless; failures accumulate and are told together.
use qymcad_core::feature::{BasePlane, Purpose, Reach, SketchPlane};
use qymcad_core::model::WorkPlane;
use qymcad_core::geom::{Mesh, Point2, Point3};
use qymcad_core::model::Project;
use qymcad_core::names::NameTable;
use qymcad_core::refs::Ref;

/// A block 40 x 30 x 10 extruded from a rectangle on XY, corner at the origin: every element of it named.
fn extruded_block(p: &mut Project) -> u64 {
    let si = p.new_sketch("Block");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Block");
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, Purpose::Real);
    p.regen_sketch(si);
    let c: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let block = p.add_extrude_multi(sid, c, 10.0, Reach::Forward, 0.0, vec![]);
    let (report, _) = qymcad_testkit::regenerate(p);
    assert!(report.errors.is_empty(), "setup: the block did not build: {:?}", report.errors);
    block
}

/// The face of `body` whose normal is `normal`, by its id.
fn face(p: &Project, body: u64, normal: [f64; 3]) -> u32 {
    p.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| (0..3).all(|i| (f.normal[i] - normal[i]).abs() < 1e-6))).map(|f| f.id).expect("the face of that normal")
}

/// The key of that face.
fn face_key(p: &Project, body: u64, normal: [f64; 3]) -> qymcad_core::feature::FaceKey {
    let f = p.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| (0..3).all(|i| (f.normal[i] - normal[i]).abs() < 1e-6))).cloned().expect("the face of that normal");
    qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
}

/// The upright edge of `body` at (x, y).
fn upright(p: &Project, body: u64, x: f64, y: f64) -> u32 {
    p.regen_edges.get(&body).and_then(|es| es.iter().find(|e| (e.mid[0] - x).abs() < 1e-3 && (e.mid[1] - y).abs() < 1e-3 && (e.a[2] - e.b[2]).abs() > 1.0)).map(|e| e.id).expect("the upright edge")
}

/// The round edge of `body` that stands highest: the rim a thread or an auger starts from.
fn top_rim(p: &mut Project, body: u64) -> u32 {
    qymcad_testkit::regenerate(p);
    let rims = p.regen_edges.get(&body).into_iter().flatten().filter(|e| e.radius > 1e-6);
    rims.max_by(|a, b| a.mid[2].total_cmp(&b.mid[2])).map(|e| e.id).expect("a round edge")
}

/// A sketch on XY holding a rectangle and, where `axis` is given, a construction line - the sketch and its closed
/// profiles.
fn sketch_of(p: &mut Project, name: &str, corners: [Point2; 2], axis: Option<[Point2; 2]>) -> (u64, Vec<u64>, Vec<u64>) {
    let si = p.new_sketch(name);
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, name);
    p.add_rect_entity(si, corners[0].x, corners[0].y, corners[1].x, corners[1].y, Purpose::Real);
    let line = axis.map(|[a, b]| p.add_line_entity(si, a.x, a.y, b.x, b.y, Purpose::Construction));
    p.regen_sketch(si);
    let c: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    (sid, c, line.into_iter().collect())
}

/// A sheet: the copy of the face of `body` whose normal is `normal`.
fn sheet_of(p: &mut Project, body: u64, normal: [f64; 3]) -> u64 {
    let f = face(p, body, normal);
    let sheet = p.add_face_copy(body, Ref::picks(&[f]));
    qymcad_testkit::regenerate(p);
    sheet
}

/// A cube of side 10 brought in as a bare triangle mesh, a part of its own as an STL comes in.
fn mesh_part(p: &mut Project) -> u64 {
    let source = p.add_source("cube.stl", vec![1]);
    let m = p.add_mesh(cube_mesh());
    p.import_tree_as_parts(vec![qymcad_core::model::ImportNode { name: "cube".into(), body: Some(m), mesh: true, ..Default::default() }], source, "cube").expect("the mesh came in");
    m
}

/// A cube of side 10 as a bare triangle mesh.
fn cube_mesh() -> Mesh {
    let v = |x: f64, y: f64, z: f64| Point3::new(x * 10.0, y * 10.0, z * 10.0);
    Mesh {
        verts: vec![v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0), v(0.0, 1.0, 0.0), v(0.0, 0.0, 1.0), v(1.0, 0.0, 1.0), v(1.0, 1.0, 1.0), v(0.0, 1.0, 1.0)],
        tris: vec![[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]],
        ..Default::default()
    }
}

const fn p2(x: f64, y: f64) -> Point2 {
    Point2 { x, y }
}

const UP: [f64; 3] = [0.0, 0.0, 1.0];
const FRONT: [f64; 3] = [0.0, -1.0, 0.0];

/// AN OPERATION AS A PERSON LAYS IT: `build` makes what it works on and lays the node of kind `kind` (the variant of
/// `FeatureKind`), giving the node's body.
struct Row {
    name: &'static str,
    kind: &'static str,
    build: fn(&mut Project) -> u64,
}

const ROWS: [Row; 40] = [
    Row { name: "an extruded block", kind: "Extrude", build: extruded_block },
    Row { name: "the box primitive", kind: "Box3", build: |p| p.add_box(40.0, 30.0, 10.0) },
    Row { name: "the prism primitive", kind: "Prism", build: |p| p.add_prism(10.0, 6, 10.0) },
    Row { name: "the cylinder primitive", kind: "Cylinder", build: |p| p.add_cylinder(10.0, 10.0) },
    Row {
        name: "a fillet of one edge",
        kind: "Fillet",
        build: |p| {
            let b = extruded_block(p);
            let e = upright(p, b, 0.0, 0.0);
            p.add_fillet(b, 2.0, vec![e])
        },
    },
    Row {
        name: "a chamfer of one edge",
        kind: "Chamfer",
        build: |p| {
            let b = extruded_block(p);
            let e = upright(p, b, 0.0, 0.0);
            p.add_chamfer(b, 2.0, vec![e])
        },
    },
    Row {
        name: "a shell open at the top",
        kind: "Shell",
        build: |p| {
            let b = extruded_block(p);
            let top = face(p, b, UP);
            p.add_shell(b, 2.0, vec![top], false)
        },
    },
    Row {
        name: "a hole in the top",
        kind: "Hole",
        build: |p| {
            let b = extruded_block(p);
            let key = face_key(p, b, UP);
            p.add_hole(b, key, 6.0, 5.0)
        },
    },
    Row {
        name: "an offset surface of the top",
        kind: "OffsetSurface",
        build: |p| {
            let b = extruded_block(p);
            let top = face(p, b, UP);
            p.add_offset_surface(b, Ref::picks(&[top]), 3.0)
        },
    },
    Row {
        name: "a copy of the top face",
        kind: "FaceCopy",
        build: |p| {
            let b = extruded_block(p);
            let top = face(p, b, UP);
            p.add_face_copy(b, Ref::picks(&[top]))
        },
    },
    Row {
        name: "a linear pattern",
        kind: "LinearArray",
        build: |p| {
            let b = extruded_block(p);
            p.add_linear_array(b, 50.0, 0.0, 0.0, 3)
        },
    },
    Row {
        name: "a mirror kept with its original",
        kind: "Mirror",
        build: |p| {
            let b = extruded_block(p);
            p.add_mirror(b, 1, true, 0)
        },
    },
    Row {
        name: "a split of the faces",
        kind: "SplitFace",
        build: |p| {
            let b = extruded_block(p);
            p.add_split_face(b, 0, 0, 5.0)
        },
    },
    Row {
        name: "a pushed face",
        kind: "PushFace",
        build: |p| {
            let b = extruded_block(p);
            let key = face_key(p, b, UP);
            p.add_push_face(b, key, 3.0)
        },
    },
    Row {
        name: "a thickened face",
        kind: "Thicken",
        build: |p| {
            let b = extruded_block(p);
            let top = face(p, b, UP);
            p.add_thicken(b, top, 2.0)
        },
    },
    Row {
        name: "a draft of the front",
        kind: "Draft",
        build: |p| {
            let b = extruded_block(p);
            let (front, bottom) = (face(p, b, FRONT), face(p, b, [0.0, 0.0, -1.0]));
            p.add_draft(b, vec![front], bottom, 5.0, false)
        },
    },
    Row {
        name: "a revolved ring",
        kind: "Revolve",
        build: |p| {
            let si = p.new_sketch("Ring");
            let sid = p.sketches[si].id;
            p.add_sketch_node(sid, "Ring");
            p.add_rect_entity(si, 10.0, 0.0, 20.0, 30.0, Purpose::Real);
            p.regen_sketch(si);
            let c: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
            p.add_revolve_axis(sid, c, 1, 360.0, 0, 0)
        },
    },
    Row {
        name: "a sweep of a circle along a line",
        kind: "Sweep",
        build: |p| {
            let (prof, profiles, _) = sketch_of(p, "Profile", [p2(-3.0, -3.0), p2(3.0, 3.0)], None);
            let spath = p.new_sketch("Path");
            let path_sid = p.sketches[spath].id;
            p.sketches[spath].plane = SketchPlane::World(BasePlane::XZ);
            p.add_sketch_node(path_sid, "Path");
            p.add_line_entity(spath, 0.0, 0.0, 0.0, 40.0, Purpose::Real);
            p.regen_sketch(spath);
            let path = p.sketches[spath].contour_ids.first().copied().expect("the path");
            p.add_sweep(prof, profiles, path_sid, path)
        },
    },
    Row {
        name: "a revolved cut",
        kind: "Revolve",
        build: |p| {
            let b = extruded_block(p);
            let (sid, profiles, line) = sketch_of(p, "Groove", [p2(10.0, 10.0), p2(20.0, 14.0)], Some([p2(0.0, 10.0), p2(40.0, 10.0)]));
            let ax = qymcad_core::model::RevolveAxis { axis: 0, datum: 0, line: line[0] };
            p.add_revolve_multi_op(sid, profiles, ax, qymcad_core::model::RevolveTurn { angle: 360.0, reach: Reach::Forward }, b, 0)
        },
    },
    Row {
        name: "two sheets stitched",
        kind: "Stitch",
        build: |p| {
            let b = extruded_block(p);
            let (a, c) = (sheet_of(p, b, UP), sheet_of(p, b, FRONT));
            p.add_stitch(vec![a, c], 1e-4)
        },
    },
    Row {
        name: "a rounding taken off again",
        kind: "RemoveFace",
        build: |p| {
            let b = extruded_block(p);
            let e = upright(p, b, 0.0, 0.0);
            let f = p.add_fillet(b, 2.0, vec![e]);
            qymcad_testkit::regenerate(p);
            let round = p.regen_faces[&f].iter().find(|x| x.centroid.x < 2.0 && x.centroid.y < 2.0 && (x.centroid.z - 5.0).abs() < 1.0).cloned().expect("the rounding");
            let key = qymcad_core::feature::FaceKey { index: 0, centroid: [round.centroid.x, round.centroid.y, round.centroid.z], normal: round.normal, id: round.id };
            p.add_remove_face(f, vec![key])
        },
    },
    Row {
        name: "a patch over the opening of a shell",
        kind: "Patch",
        build: |p| {
            let b = extruded_block(p);
            let top = face(p, b, UP);
            let shell = p.add_shell(b, 2.0, vec![top], false);
            qymcad_testkit::regenerate(p);
            let rim: Vec<u32> = p.regen_edges[&shell]
                .iter()
                .filter(|e| e.a[2] > 9.99 && e.b[2] > 9.99 && [e.a, e.b].iter().all(|q| q[0].abs() < 1e-6 || (q[0] - 40.0).abs() < 1e-6 || q[1].abs() < 1e-6 || (q[1] - 30.0).abs() < 1e-6))
                .map(|e| e.id)
                .collect();
            p.add_patch(shell, Ref::picks(&rim), false)
        },
    },
    Row {
        name: "a sheet trimmed by a body",
        kind: "Trim",
        build: |p| {
            let b = extruded_block(p);
            let sheet = sheet_of(p, b, UP);
            let (sid, profiles, _) = sketch_of(p, "Cutter", [p2(20.0, -10.0), p2(60.0, 40.0)], None);
            let tool = p.add_extrude_multi(sid, profiles, 30.0, Reach::Forward, 0.0, vec![]);
            qymcad_testkit::regenerate(p);
            p.add_trim(sheet, tool, [10.0, 15.0, 10.0])
        },
    },
    Row {
        name: "a mesh made a solid",
        kind: "MeshSolid",
        build: |p| {
            let m = mesh_part(p);
            p.add_mesh_solid(m, 0.0)
        },
    },
    Row {
        name: "a mesh recognised",
        kind: "MeshRecognised",
        build: |p| {
            let m = mesh_part(p);
            p.add_mesh_recognised(m, 1.0, 30.0, 0.0)
        },
    },
    Row {
        name: "a body brought in from a STEP file",
        kind: "Import",
        build: |p| {
            let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/names-of-an-import"));
            std::fs::create_dir_all(&dir).expect("a folder for the check");
            let step = dir.join("block.step");
            let block = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 30.0, 0.0, 30.0], 10.0).expect("a block");
            qymcad_kernel::write_step(&[(&block, qymcad_core::feature::PLACE_IDENTITY)], &step.to_string_lossy()).expect("written");
            let source = p.add_source("block.step", std::fs::read(&step).expect("reads"));
            let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::import_step(&step.to_string_lossy(), 0.5).expect("imports").remove(0);
            let body = p.add_mesh(mesh);
            p.import_tree_as_parts(vec![qymcad_core::model::ImportNode { name: "block".into(), body: Some(body), ..Default::default() }], source, "block").expect("came in");
            body
        },
    },
    Row { name: "the sphere primitive", kind: "Sphere", build: |p| p.add_sphere(10.0) },
    Row { name: "the cone primitive", kind: "Cone", build: |p| p.add_cone(10.0, 5.0, 10.0) },
    Row { name: "the torus primitive", kind: "Torus", build: |p| p.add_torus(20.0, 5.0) },
    Row {
        name: "a loft between two squares",
        kind: "Loft",
        build: |p| {
            let (low, lows, _) = sketch_of(p, "Low", [p2(-10.0, -10.0), p2(10.0, 10.0)], None);
            let pl = p.add_plane(WorkPlane { id: 0, name: "z10".into(), origin: [0.0, 0.0, 10.0], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, tilt: Default::default(), def: Default::default() });
            let si = p.new_sketch("High");
            let high = p.sketches[si].id;
            p.sketches[si].plane = SketchPlane::Datum(pl);
            p.add_sketch_node(high, "High");
            p.add_rect_entity(si, -5.0, -5.0, 5.0, 5.0, Purpose::Real);
            p.regen_sketch(si);
            let highs: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
            p.add_loft(vec![low, high], vec![lows[0], highs[0]], true, 0, 0, false)
        },
    },
    Row {
        name: "a pocket cut into the top",
        kind: "Combine",
        build: |p| {
            let b = extruded_block(p);
            let (sid, profiles, _) = sketch_of(p, "Pocket", [p2(5.0, 5.0), p2(15.0, 15.0)], None);
            let span = qymcad_core::model::CombineSpan { height: 5.0, down: 0.0, extent: qymcad_core::feature::Extent { reach: Reach::Forward, ..Default::default() }, fill: &[] };
            p.add_combine_multi_op(b, sid, profiles, span, 0)
        },
    },
    Row {
        name: "the top replaced by its own copy",
        kind: "SurfaceReplace",
        build: |p| {
            let b = extruded_block(p);
            let top = face(p, b, UP);
            let sheet = p.add_face_copy(b, Ref::picks(&[top]));
            qymcad_testkit::regenerate(p);
            p.add_surface_replace(b, Ref::picks(&[top]), sheet)
        },
    },
    Row {
        name: "a circular pattern of three",
        kind: "CircularArray",
        build: |p| {
            let b = extruded_block(p);
            p.add_circular_array(b, 3, 360.0)
        },
    },
    Row {
        name: "a part mirrored",
        kind: "MirrorPart",
        build: |p| {
            let b = extruded_block(p);
            let part = p.body_owner(b).expect("the part of the block");
            p.add_mirror_part(part, [0.0; 3], [1.0, 0.0, 0.0])
        },
    },
    Row {
        name: "a thread on a cylinder",
        kind: "Thread",
        build: |p| {
            let c = p.add_cylinder(10.0, 20.0);
            let rim = top_rim(p, c);
            p.add_thread(c, rim, qymcad_core::thread::ThreadSpec { nominal_d: 20.0, ..Default::default() }, 10.0, 0.0, 0.0)
        },
    },
    Row {
        name: "an auger flight on a shaft",
        kind: "Auger",
        build: |p| {
            let c = p.add_cylinder(5.0, 40.0);
            let rim = top_rim(p, c);
            p.add_auger(c, rim, qymcad_core::thread::AugerSpec::default(), 30.0, 0.0, 0.0)
        },
    },
    Row {
        name: "the upper piece of a split",
        kind: "SplitBody",
        build: |p| {
            let b = extruded_block(p);
            p.add_split_body(b, 0, 0, 5.0, 2)[1]
        },
    },
    Row {
        name: "a block moved",
        kind: "Move",
        build: |p| {
            let b = extruded_block(p);
            p.add_move(b, [1.0, 0.0, 0.0, 15.0, 0.0, 1.0, 0.0, 5.0, 0.0, 0.0, 1.0, 0.0])
        },
    },
    Row {
        name: "a piece of a split made a part",
        kind: "Piece",
        build: |p| {
            let b = extruded_block(p);
            let pieces = p.add_split_body(b, 0, 0, 5.0, 2);
            qymcad_testkit::regenerate(p);
            p.piece_to_part(pieces[1], "Upper".into()).expect("the piece made a part").1
        },
    },
    Row {
        name: "a cylinder cut out of a block, body from body",
        kind: "BodyBoolean",
        build: |p| {
            let b = extruded_block(p);
            let c = p.add_cylinder(5.0, 30.0);
            qymcad_testkit::regenerate(p);
            p.add_body_boolean(b, c, 0)
        },
    },
];

/// How many of `ids` are not names of the document: positional numbers, or descriptors its table does not hold.
fn unnamed(names: &NameTable, ids: impl Iterator<Item = u32>) -> (usize, usize) {
    let all: Vec<u32> = ids.collect();
    (all.iter().filter(|d| !names.knows(**d)).count(), all.len())
}

#[test]
fn every_operation_names_every_face_and_edge_it_makes() {
    let mut fails: Vec<String> = Vec::new();
    for row in &ROWS {
        let mut p = Project::default();
        p.new_document();
        let node = (row.build)(&mut p);
        let made = qymcad_testkit::kind_making(&p, node);
        if made.as_deref() != Some(row.kind) {
            fails.push(format!("{}: the row says it lays {}, the node of its body is {made:?}", row.name, row.kind));
        }
        // opened as the program opens a document: a body brought in from a file is raised from it first
        let raised = qymcad_testkit::restore_import_shapes(&p);
        let (report, _) = qymcad_testkit::regenerate_with_shapes(&mut p, raised);
        if let Some((_, e)) = report.errors.iter().find(|(id, _)| *id == node) {
            fails.push(format!("{}: did not build - {e:?}", row.name));
            continue;
        }
        let (faces, of_faces) = unnamed(&p.names, p.regen_faces.get(&node).into_iter().flatten().map(|f| f.id));
        let (edges, of_edges) = unnamed(&p.names, p.regen_edges.get(&node).into_iter().flatten().map(|e| e.id));
        if of_faces == 0 {
            fails.push(format!("{}: no faces to look at", row.name));
        }
        // a vertex is named by the edges meeting at it: a positional number among them is a vertex without a name
        let spots = p.vertex_spots(node);
        let vertices = spots.iter().filter(|v| v.name.edges.iter().any(|d| *d != 0 && !p.names.knows(*d))).count();
        if faces > 0 || edges > 0 || vertices > 0 {
            fails.push(format!("{}: {faces} of {of_faces} faces, {edges} of {of_edges} edges and {vertices} of {} vertices without a name", row.name, spots.len()));
        }
    }
    assert!(fails.is_empty(), "\nELEMENTS ON POSITIONAL NUMBERS ({}):\n{}", fails.len(), fails.join("\n"));
}

/// THE BODIES WITH NO HISTORY, whose faces with no named neighbour are named by where they lie (`Role::Placed`): a body
/// brought in from a file and a body made of a mesh have no recipe for their faces. Any other operation names by its
/// history and its recipe.
const NAMED_BY_PLACE: [qymcad_testkit::Uncovered; 3] = [
    qymcad_testkit::Uncovered { kind: "Import", why: "a body brought in from a file comes with no recipe for its faces" },
    qymcad_testkit::Uncovered { kind: "MeshSolid", why: "a solid made of a mesh has faces of triangles and no recipe for them" },
    qymcad_testkit::Uncovered { kind: "MeshRecognised", why: "the faces recognised in a mesh have no recipe of the operations that made it" },
];

/// NO OPERATION NAMES WHAT IT MAKES BY PLACE but the bodies with no history: a name by place is the last resort of the
/// rule for what the history is silent about, and an operation that reaches it lost its history somewhere.
#[test]
fn no_operation_names_by_place_but_a_body_with_no_history() {
    let mut fails: Vec<String> = Vec::new();
    for row in &ROWS {
        let mut p = Project::default();
        p.new_document();
        let node = (row.build)(&mut p);
        let raised = qymcad_testkit::restore_import_shapes(&p);
        let (report, _) = qymcad_testkit::regenerate_with_shapes(&mut p, raised);
        if report.errors.iter().any(|(id, _)| *id == node) {
            continue; // told by the check above
        }
        let placed = p.regen_faces.get(&node).into_iter().flatten().filter(|f| p.names.get(f.id).is_some_and(|n| n.role == qymcad_core::names::Role::Placed)).count();
        let allowed = NAMED_BY_PLACE.iter().any(|u| u.kind == row.kind);
        if placed > 0 && !allowed {
            fails.push(format!("{}: {placed} faces named by where they lie", row.name));
        }
    }
    assert!(fails.is_empty(), "\nFACES NAMED BY PLACE ({}):\n{}", fails.len(), fails.join("\n"));
}

/// THE NODES THAT BUILD NO BODY OF THEIR OWN BY AN OPERATION: nothing for this matrix to look at.
const NOT_HERE: [qymcad_testkit::Uncovered; 7] = [
    qymcad_testkit::Uncovered { kind: "Sketch", why: "a sketch builds no body" },
    qymcad_testkit::Uncovered { kind: "Plane", why: "a datum plane builds no body" },
    qymcad_testkit::Uncovered { kind: "DatumPoint", why: "a datum point builds no body" },
    qymcad_testkit::Uncovered { kind: "DatumAxis", why: "a datum axis builds no body" },
    qymcad_testkit::Uncovered { kind: "PartInstance", why: "a copy of a part placed in an assembly repeats the body of the part, its names with it" },
    qymcad_testkit::Uncovered { kind: "ComponentPattern", why: "the copies of a pattern of parts repeat the body of the part, its names with it" },
    qymcad_testkit::Uncovered { kind: "MeshPiece", why: "a piece of a mesh has no B-rep: its faces are the mesh's, kept by the document" },
];

/// EVERY KIND OF NODE HAS A ROW HERE, or a reason for none: a kind added without a row is an operation whose names nobody
/// looked at.
#[test]
fn every_kind_of_node_has_a_row() {
    let kinds: Vec<&str> = ROWS.iter().map(|r| r.kind).collect();
    let gaps = qymcad_testkit::coverage_gaps(&kinds, &NOT_HERE);
    assert!(gaps.is_empty(), "the matrix of names leaves kinds of node unseen:\n{}", gaps.join("\n"));
}
