//! A REFERENCE HOLDS ITS ELEMENT THROUGH AN EDIT ABOVE IT, for every operation and every kind of edit: a fillet laid on
//! an edge of a result rounds the same edge - the one standing where that edge stands after the edit - once the base
//! sketch is wider, the base taller, a node between them suppressed, or the document saved and opened again.
//!
//! The edge is told by where it lies, as a function of the width and the height of the base: the same rule the person
//! applies looking at the part. Live OCCT, headless; failures accumulate and are told together.
use qymcad_core::feature::{FeatureKind, Purpose, Reach};
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

/// THE SIZE OF THE BASE BLOCK: a rectangle w x 30 on XY, extruded h.
#[derive(Clone, Copy)]
struct Size {
    w: f64,
    h: f64,
}

const BASE: Size = Size { w: 40.0, h: 10.0 };

/// A number of its own for every file a cell writes: the checks of this file run side by side in one process, and one
/// reading a document another is writing under the same name measured the race, not the names.
fn unique() -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    format!("{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

/// The base block, its sketch and its extrusion: the corner at the origin.
struct Base {
    sketch: u64,
    node: u64,
}

/// HOW THE LEFT SIDE OF THE BASE STARTS: one straight side, or a bump of two sides out to x = -5.
#[derive(Clone, Copy, PartialEq)]
enum Left {
    Straight,
    Bumped,
}

fn base(p: &mut Project, left: Left) -> Base {
    let mut corners = vec![(0.0, 0.0), (BASE.w, 0.0), (BASE.w, 30.0), (0.0, 30.0)];
    if left == Left::Bumped {
        corners.push((-5.0, 15.0));
    }
    let pts = corners.iter().map(|&(x, y)| Point2::new(x, y)).collect();
    let sketch = p.add_line_sketch("Base", pts, true);
    let si = p.sketch_index(sketch).expect("the sketch");
    p.regen_sketch(si);
    p.add_sketch_node(sketch, "Base");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let node = p.add_extrude_multi(sketch, closed, BASE.h, Reach::Forward, 0.0, vec![]);
    Base { sketch, node }
}

/// The edge of `body` whose middle is at `at`, to a micrometre.
fn edge_at(p: &Project, body: u64, at: [f64; 3]) -> Option<u32> {
    p.regen_edges.get(&body)?.iter().find(|e| (0..3).all(|i| (e.mid[i] - at[i]).abs() < 1e-3)).map(|e| e.id)
}

/// The id of the face of `body` facing `normal`.
fn face_id(p: &Project, body: u64, normal: [f64; 3]) -> u32 {
    p.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| (0..3).all(|i| (f.normal[i] - normal[i]).abs() < 1e-6))).map(|f| f.id).expect("the face of that normal")
}

/// The key of the face of `body` facing `normal`.
fn face_key(p: &Project, body: u64, normal: [f64; 3]) -> qymcad_core::feature::FaceKey {
    let f = p.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| (0..3).all(|i| (f.normal[i] - normal[i]).abs() < 1e-6))).cloned().expect("the face of that normal");
    qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
}

/// A post of radius 5 at (20, 15) on `src`, `tall` from the bottom, and its rim at the top: the body and the rim.
fn post_of(p: &mut Project, src: u64, tall: f64) -> (u64, u32) {
    let si = p.new_sketch("Post");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Post");
    p.add_circle_entity(si, 20.0, 15.0, 5.0, Purpose::Real);
    p.regen_sketch(si);
    let c: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let span = qymcad_core::model::CombineSpan { height: tall, down: 0.0, extent: qymcad_core::feature::Extent { reach: Reach::Forward, ..Default::default() }, fill: &[] };
    let post = p.add_combine_multi_op(src, sid, c, span, 1);
    let _ = qymcad_testkit::regenerate(p);
    let rim = p.regen_edges.get(&post).into_iter().flatten().filter(|e| e.radius > 1e-6).max_by(|a, b| a.mid[2].total_cmp(&b.mid[2])).map(|e| e.id).expect("the rim of the post");
    (post, rim)
}

/// AN OPERATION ON THE BASE and the edge of its result a reference is laid on: `build` lays the node on `src` and gives
/// the body of the result, `edge` says where the edge lies for a base of a size.
struct Row {
    name: &'static str,
    kind: &'static str,
    build: fn(&mut Project, u64) -> u64,
    edge: fn(Size) -> [f64; 3],
    top: Option<Top>,
}

/// THE TOP FACE OF A RESULT a sketch is laid on: a spot of it in plan, and its height for a base of a size.
struct Top {
    at: [f64; 2],
    z: fn(Size) -> f64,
}

const ROWS: [Row; 25] = [
    // the top edge at the back, carried from the sketch
    Row { name: "the block itself", kind: "Extrude", build: |_, src| src, edge: |s| [s.w / 2.0, 30.0, s.h], top: Some(Top { at: [5.0, 5.0], z: |s| s.h }) },
    // the arc where the rounding of the front right corner meets the top, its middle at 45 deg
    Row {
        name: "a fillet of a corner",
        kind: "Fillet",
        build: |p, src| {
            let _ = qymcad_testkit::regenerate(p);
            let e = edge_at(p, src, [BASE.w, 0.0, BASE.h / 2.0]).expect("the front right upright edge");
            p.add_fillet(src, 2.0, vec![e])
        },
        edge: |s| [s.w - 2.0 + std::f64::consts::SQRT_2, 2.0 - std::f64::consts::SQRT_2, s.h],
        top: Some(Top { at: [5.0, 5.0], z: |s| s.h }),
    },
    Row {
        name: "a chamfer of a corner",
        kind: "Chamfer",
        build: |p, src| {
            let _ = qymcad_testkit::regenerate(p);
            let e = edge_at(p, src, [BASE.w, 0.0, BASE.h / 2.0]).expect("the front right upright edge");
            p.add_chamfer(src, 2.0, vec![e])
        },
        edge: |s| [s.w - 2.0, 0.0, s.h / 2.0],
        top: Some(Top { at: [5.0, 5.0], z: |s| s.h }),
    },
    // the upright inner corner at the front right of a shell open at the top, walls 2 thick, floor 2 thick
    Row {
        name: "a shell open at the top",
        kind: "Shell",
        build: |p, src| {
            let _ = qymcad_testkit::regenerate(p);
            let top = face_id(p, src, [0.0, 0.0, 1.0]);
            p.add_shell(src, 2.0, vec![top], false)
        },
        edge: |s| [s.w - 2.0, 2.0, (s.h + 2.0) / 2.0],
        top: None,
    },
    // the upright edge of a pocket cut 5 up from the bottom
    Row {
        name: "a pocket cut from the bottom",
        kind: "Combine",
        build: |p, src| {
            let si = p.new_sketch("Pocket");
            let sid = p.sketches[si].id;
            p.add_sketch_node(sid, "Pocket");
            p.add_rect_entity(si, 5.0, 5.0, 15.0, 15.0, Purpose::Real);
            p.regen_sketch(si);
            let c: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
            let span = qymcad_core::model::CombineSpan { height: 5.0, down: 0.0, extent: qymcad_core::feature::Extent { reach: Reach::Forward, ..Default::default() }, fill: &[] };
            p.add_combine_multi_op(src, sid, c, span, 0)
        },
        edge: |_| [5.0, 5.0, 2.5],
        top: Some(Top { at: [25.0, 5.0], z: |s| s.h }),
    },
    // the top edge at the back of the second copy, 100 along
    Row {
        name: "a linear pattern",
        kind: "LinearArray",
        build: |p, src| p.add_linear_array(src, 100.0, 0.0, 0.0, 2),
        edge: |s| [100.0 + s.w / 2.0, 30.0, s.h],
        top: Some(Top { at: [5.0, 5.0], z: |s| s.h }),
    },
    // the front upright edge at the far end of the mirror image across YZ: the image and the original join at x = 0
    Row { name: "a mirror kept with its original", kind: "Mirror", build: |p, src| p.add_mirror(src, 2, true, 0), edge: |s| [-s.w, 0.0, s.h / 2.0], top: Some(Top { at: [5.0, 5.0], z: |s| s.h }) },
    // the top face thickened 2 up: the top edge of the plate at the back
    Row {
        name: "the top face thickened",
        kind: "Thicken",
        build: |p, src| {
            let _ = qymcad_testkit::regenerate(p);
            let top = face_id(p, src, [0.0, 0.0, 1.0]);
            p.add_thicken(src, top, 2.0)
        },
        edge: |s| [s.w / 2.0, 30.0, s.h + 2.0],
        top: Some(Top { at: [5.0, 5.0], z: |s| s.h + 2.0 }),
    },
    // split by the plane z = 3: the top edge at the back of the upper piece
    Row {
        name: "the upper piece of a split",
        kind: "SplitBody",
        build: |p, src| p.add_split_body(src, 0, 0, 3.0, 2)[1],
        edge: |s| [s.w / 2.0, 30.0, s.h],
        top: Some(Top { at: [5.0, 5.0], z: |s| s.h }),
    },
    // a groove turned into the bottom: a rectangle 10 long and 4 high revolved about a line of its sketch along x at
    // y = 10; the half circle where the groove meets its end at x = 10 tops out at (10, 10, 4)
    Row {
        name: "a groove revolved into the bottom",
        kind: "Revolve",
        build: |p, src| {
            let si = p.new_sketch("Groove");
            let sid = p.sketches[si].id;
            p.add_sketch_node(sid, "Groove");
            p.add_rect_entity(si, 10.0, 10.0, 20.0, 14.0, Purpose::Real);
            let line = p.add_line_entity(si, 0.0, 10.0, 30.0, 10.0, Purpose::Construction);
            p.regen_sketch(si);
            let c: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
            let ax = qymcad_core::model::RevolveAxis { axis: 0, datum: 0, line };
            p.add_revolve_multi_op(sid, c, ax, qymcad_core::model::RevolveTurn { angle: 360.0, reach: Reach::Forward }, src, 0)
        },
        edge: |_| [10.0, 10.0, 4.0],
        top: Some(Top { at: [30.0, 25.0], z: |s| s.h }),
    },
    // the box primitive beside the base, centred on (0, 0) in plan: its top edge at the back
    Row { name: "the box primitive", kind: "Box3", build: |p, _| p.add_box(20.0, 10.0, 4.0), edge: |_| [0.0, 5.0, 4.0], top: Some(Top { at: [5.0, 2.0], z: |_| 4.0 }) },
    // a block brought in from a STEP file, apart from the base: its top edge at the back
    Row {
        name: "a body brought in from a STEP file",
        kind: "Import",
        build: |p, _| {
            let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/names-survive-an-edit"));
            std::fs::create_dir_all(&dir).expect("a folder for the check");
            let step = dir.join(format!("block-{}.step", unique()));
            let block = qymcad_kernel::specimen::extrude(&[200.0, 0.0, 220.0, 0.0, 220.0, 10.0, 200.0, 10.0], 4.0).expect("a block");
            qymcad_kernel::write_step(&[(&block, qymcad_core::feature::PLACE_IDENTITY)], &step.to_string_lossy()).expect("written");
            let source = p.add_source("block.step", std::fs::read(&step).expect("reads"));
            let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::import_step(&step.to_string_lossy(), 0.5).expect("imports").remove(0);
            let body = p.add_mesh(mesh);
            p.import_tree_as_parts(vec![qymcad_core::model::ImportNode { name: "block".into(), body: Some(body), ..Default::default() }], source, "block").expect("came in");
            let raised = qymcad_testkit::restore_import_shapes(p);
            let _ = qymcad_testkit::regenerate_with_shapes(p, raised);
            body
        },
        edge: |_| [210.0, 10.0, 4.0],
        top: Some(Top { at: [205.0, 5.0], z: |_| 4.0 }),
    },
    // a hole drilled into the middle of the top: the bottom edge at the front
    Row {
        name: "a hole in the top",
        kind: "Hole",
        build: |p, src| {
            let _ = qymcad_testkit::regenerate(p);
            let key = face_key(p, src, [0.0, 0.0, 1.0]);
            p.add_hole(src, key, 6.0, 5.0)
        },
        edge: |s| [s.w / 2.0, 0.0, 0.0],
        top: Some(Top { at: [5.0, 5.0], z: |s| s.h }),
    },
    // the front drafted 5 deg about the bottom: its edge on the bottom, where the draft turns it, stays put
    Row {
        name: "a draft of the front",
        kind: "Draft",
        build: |p, src| {
            let _ = qymcad_testkit::regenerate(p);
            let (front, bottom) = (face_id(p, src, [0.0, -1.0, 0.0]), face_id(p, src, [0.0, 0.0, -1.0]));
            p.add_draft(src, vec![front], bottom, 5.0, false)
        },
        edge: |s| [s.w / 2.0, 0.0, 0.0],
        top: Some(Top { at: [5.0, 25.0], z: |s| s.h }),
    },
    // the top pushed 3 up: the top edge at the front
    Row {
        name: "the top pushed up",
        kind: "PushFace",
        build: |p, src| {
            let _ = qymcad_testkit::regenerate(p);
            let key = face_key(p, src, [0.0, 0.0, 1.0]);
            p.add_push_face(src, key, 3.0)
        },
        edge: |s| [s.w / 2.0, 0.0, s.h + 3.0],
        top: Some(Top { at: [5.0, 5.0], z: |s| s.h + 3.0 }),
    },
    // the sides divided by the plane z = 5: the upper piece of the front right upright edge it cut in two (the line it
    // leaves on a side has the same plane on both of its sides, and no rounding takes it)
    Row {
        name: "the faces divided at z = 5",
        kind: "SplitFace",
        build: |p, src| p.add_split_face(src, 0, 0, 5.0),
        edge: |s| [s.w, 0.0, (5.0 + s.h) / 2.0],
        top: Some(Top { at: [5.0, 5.0], z: |s| s.h }),
    },
    // the front right corner rounded and the rounding taken off again: the upright edge is back
    Row {
        name: "a rounding taken off again",
        kind: "RemoveFace",
        build: |p, src| {
            let _ = qymcad_testkit::regenerate(p);
            let e = edge_at(p, src, [BASE.w, 0.0, BASE.h / 2.0]).expect("the front right upright edge");
            let f = p.add_fillet(src, 2.0, vec![e]);
            let _ = qymcad_testkit::regenerate(p);
            let round = p.regen_faces[&f].iter().find(|x| x.centroid.x > BASE.w - 2.0 && x.centroid.y < 2.0).cloned().expect("the rounding");
            let key = qymcad_core::feature::FaceKey { index: 0, centroid: [round.centroid.x, round.centroid.y, round.centroid.z], normal: round.normal, id: round.id };
            p.add_remove_face(f, vec![key])
        },
        edge: |s| [s.w, 0.0, s.h / 2.0],
        top: Some(Top { at: [5.0, 5.0], z: |s| s.h }),
    },
    // four copies about Z: the top edge of the copy turned half a turn, at its far side
    Row {
        name: "a circular pattern of four",
        kind: "CircularArray",
        build: |p, src| p.add_circular_array(src, 4, 360.0),
        edge: |s| [-s.w / 2.0, -30.0, s.h],
        top: Some(Top { at: [5.0, 5.0], z: |s| s.h }),
    },
    // moved 15 along x and 5 along y: the top edge at the front
    Row {
        name: "the block moved",
        kind: "Move",
        build: |p, src| p.add_move(src, [1.0, 0.0, 0.0, 15.0, 0.0, 1.0, 0.0, 5.0, 0.0, 0.0, 1.0, 0.0]),
        edge: |s| [s.w / 2.0 + 15.0, 5.0, s.h],
        top: Some(Top { at: [20.0, 10.0], z: |s| s.h }),
    },
    // a cylinder of radius 5 on the origin cut out of the corner, body from body: the top edge at the back
    Row {
        name: "a cylinder cut out, body from body",
        kind: "BodyBoolean",
        build: |p, src| {
            let c = p.add_cylinder(5.0, 30.0);
            let _ = qymcad_testkit::regenerate(p);
            p.add_body_boolean(src, c, 0)
        },
        edge: |s| [s.w / 2.0, 30.0, s.h],
        top: Some(Top { at: [25.0, 15.0], z: |s| s.h }),
    },
    // the top replaced by its own copy: the top edge at the front
    Row {
        name: "the top replaced by its copy",
        kind: "SurfaceReplace",
        build: |p, src| {
            let _ = qymcad_testkit::regenerate(p);
            let top = face_id(p, src, [0.0, 0.0, 1.0]);
            let sheet = p.add_face_copy(src, qymcad_core::refs::Ref::picks(&[top]));
            let _ = qymcad_testkit::regenerate(p);
            p.add_surface_replace(src, qymcad_core::refs::Ref::picks(&[top]), sheet)
        },
        edge: |s| [s.w / 2.0, 0.0, s.h],
        top: Some(Top { at: [5.0, 5.0], z: |s| s.h }),
    },
    // the part mirrored across YZ: the top edge at the front of the image
    Row {
        name: "the part mirrored",
        kind: "MirrorPart",
        build: |p, src| {
            let part = p.body_owner(src).expect("the part of the base");
            p.add_mirror_part(part, [0.0; 3], [1.0, 0.0, 0.0])
        },
        edge: |s| [-s.w / 2.0, 0.0, s.h],
        top: Some(Top { at: [-5.0, 5.0], z: |s| s.h }),
    },
    // split at z = 3 and the upper piece made a part of its own: its top edge at the back
    Row {
        name: "the upper piece made a part",
        kind: "Piece",
        build: |p, src| {
            let pieces = p.add_split_body(src, 0, 0, 3.0, 2);
            let _ = qymcad_testkit::regenerate(p);
            p.piece_to_part(pieces[1], "Upper".into()).expect("the piece made a part").1
        },
        edge: |s| [s.w / 2.0, 30.0, s.h],
        top: Some(Top { at: [5.0, 5.0], z: |s| s.h }),
    },
    // a post of radius 5 on the block, 30 tall from the bottom, threaded M10 for 8 from its top: the top edge of the
    // block at the front; the sketch stands on the end of the post, the highest face
    Row {
        name: "a thread on a post of the block",
        kind: "Thread",
        build: |p, src| {
            let (post, rim) = post_of(p, src, 30.0);
            p.add_thread(post, rim, qymcad_core::thread::ThreadSpec { nominal_d: 10.0, ..Default::default() }, 8.0, 0.0, 0.0)
        },
        edge: |s| [s.w / 2.0, 0.0, s.h],
        top: Some(Top { at: [20.0, 15.0], z: |_| 30.0 }),
    },
    // a post of radius 5, 50 tall, an auger flight on its upper 20: the top edge of the block at the front; the sketch
    // stands on the end of the post
    Row {
        name: "an auger flight on a post of the block",
        kind: "Auger",
        build: |p, src| {
            let (post, rim) = post_of(p, src, 50.0);
            p.add_auger(post, rim, qymcad_core::thread::AugerSpec::default(), 20.0, 0.0, 0.0)
        },
        edge: |s| [s.w / 2.0, 0.0, s.h],
        top: Some(Top { at: [20.0, 15.0], z: |_| 50.0 }),
    },
];

/// AN EDIT ABOVE THE OPERATION: what it does to a document laid as base -> (between) -> operation -> reference, and the
/// size of the base after it.
struct Edit {
    name: &'static str,
    left: Left,
    apply: fn(Project, &Laid) -> Project,
    size: Size,
}

/// What a row laid: the base, the node between it and the operation where there is one, the operation's body and the
/// fillet laid on its edge.
struct Laid {
    base: Base,
    between: Option<u64>,
    fillet: u64,
}

/// WHAT IS LAID ON THE RESULT and held through the edit.
#[derive(Clone, Copy, PartialEq)]
enum Reference {
    /// A fillet R0.5 on the edge the row names.
    Fillet,
    /// A circle of radius 1 at the spot the row names, sketched on the top face and extruded 2 up as a boss.
    Boss,
    /// A mate connector of the part on the edge the row names: what a joint of an assembly holds on to.
    Mate,
}

/// The lines of the base sketch with both ends at `x` < 0 or on it, by id, and the index of the sketch.
fn left_lines(p: &Project, sketch: u64) -> (usize, Vec<u64>) {
    let si = p.sketch_index(sketch).expect("the sketch");
    let s = &p.sketches[si];
    let x_of = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| q.x).unwrap_or(f64::MAX);
    let lines = s.entities.iter().filter_map(|e| match e.kind {
        qymcad_core::model::EntityKind::Line { a, b } if x_of(a) <= 1e-9 && x_of(b) <= 1e-9 => Some(e.id),
        _ => None,
    });
    (si, lines.collect())
}

const EDITS: [Edit; 7] = [
    Edit {
        name: "the base sketch made wider, 40 -> 50",
        left: Left::Straight,
        apply: |mut p, laid| {
            let si = p.sketch_index(laid.base.sketch).expect("the sketch");
            for q in p.sketches[si].points.iter_mut() {
                if q.x > BASE.w / 2.0 {
                    q.x += 10.0;
                }
            }
            p.regen_sketch(si);
            p.mark_sketch_dirty(laid.base.sketch);
            p
        },
        size: Size { w: 50.0, h: 10.0 },
    },
    Edit {
        name: "the base made taller, 10 -> 15",
        left: Left::Straight,
        apply: |mut p, laid| {
            if let Some(FeatureKind::Extrude { height, .. }) = p.timeline.iter_mut().find(|n| n.id == laid.base.node).map(|n| &mut n.kind) {
                *height = 15.0;
            }
            p.mark_node_dirty(laid.base.node);
            p
        },
        size: Size { w: 40.0, h: 15.0 },
    },
    Edit {
        name: "the node between suppressed",
        left: Left::Straight,
        apply: |mut p, laid| {
            let between = laid.between.expect("a node between");
            if let Some(n) = p.timeline.iter_mut().find(|n| n.id == between) {
                n.suppressed = true;
            }
            p.mark_node_dirty(between);
            p
        },
        size: BASE,
    },
    Edit {
        name: "an edge added to the contour: the left side broken and bumped out",
        left: Left::Straight,
        apply: |mut p, laid| {
            let (si, lines) = left_lines(&p, laid.base.sketch);
            assert!(p.break_line(si, lines[0], 0.0, 15.0), "the left side did not break");
            if let Some(q) = p.sketches[si].points.iter_mut().find(|q| q.x.abs() < 1e-9 && (q.y - 15.0).abs() < 1e-6) {
                q.x = -5.0;
            }
            p.regen_sketch(si);
            p.mark_sketch_dirty(laid.base.sketch);
            p
        },
        size: BASE,
    },
    Edit {
        name: "an edge taken away: the bump of the left side made straight",
        left: Left::Bumped,
        apply: |mut p, laid| {
            let (si, lines) = left_lines(&p, laid.base.sketch);
            p.delete_entities(si, &lines);
            p.add_line_entity(si, 0.0, 30.0, 0.0, 0.0, Purpose::Real);
            p.regen_sketch(si);
            p.mark_sketch_dirty(laid.base.sketch);
            p
        },
        size: BASE,
    },
    Edit {
        name: "rolled back to the base and forward again",
        left: Left::Straight,
        apply: |mut p, laid| {
            let at = p.timeline.iter().position(|n| n.id == laid.base.node).expect("the base in the timeline");
            p.set_rollback(Some(at + 1));
            let raised = qymcad_testkit::restore_import_shapes(&p);
            let _ = qymcad_testkit::regenerate_with_shapes(&mut p, raised);
            p.set_rollback(None);
            p
        },
        size: BASE,
    },
    Edit {
        name: "saved and opened",
        left: Left::Straight,
        apply: |p, _| {
            let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/names-survive-an-edit"));
            std::fs::create_dir_all(&dir).expect("a folder for the check");
            let path = dir.join(format!("doc-{}.qcad", unique())).to_string_lossy().into_owned();
            qymcad_io::save_project(&p, &path).expect("saved");
            qymcad_io::load_project(&path).expect("opened")
        },
        size: BASE,
    },
];

/// Lay the row on a fresh base: the base, for the edit of suppression a pocket cut 3 up from the bottom between - away
/// from every edge a row names - the operation, and a fillet R0.5 on the edge the row names.
fn lay(row: &Row, with_between: bool, left: Left, reference: Reference) -> Result<(Project, Laid), String> {
    let mut p = Project::default();
    p.new_document();
    let b = base(&mut p, left);
    let _ = qymcad_testkit::regenerate(&mut p);
    let mut src = b.node;
    let mut between = None;
    if with_between {
        let si = p.new_sketch("Between");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "Between");
        p.add_rect_entity(si, 30.0, 20.0, 35.0, 25.0, Purpose::Real);
        p.regen_sketch(si);
        let c: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
        let span = qymcad_core::model::CombineSpan { height: 3.0, down: 0.0, extent: qymcad_core::feature::Extent { reach: Reach::Forward, ..Default::default() }, fill: &[] };
        let cut = p.add_combine_multi_op(src, sid, c, span, 0);
        let _ = qymcad_testkit::regenerate(&mut p);
        between = Some(cut);
        src = cut;
    }
    let body = (row.build)(&mut p, src);
    // a row that lays nothing works on the base itself
    let made = if body == src { p.timeline.iter().find(|n| n.id == b.node).map(|n| qymcad_testkit::kind_name(&n.kind)) } else { qymcad_testkit::kind_making(&p, body) };
    if made.as_deref() != Some(row.kind) {
        return Err(format!("the row says it lays {}, the node of its body is {made:?}", row.kind));
    }
    let raised = qymcad_testkit::restore_import_shapes(&p);
    let _ = qymcad_testkit::regenerate_with_shapes(&mut p, raised);
    // laid in the part the body stands in, as a person stepped into it lays it
    let part = p.timeline.iter().find(|n| n.kind.owns_body(body)).and_then(|n| n.parent);
    p.set_active_component(part);
    let fillet = match reference {
        Reference::Fillet => {
            let e = edge_at(&p, body, (row.edge)(BASE)).ok_or_else(|| format!("no edge at {:?} before the edit", (row.edge)(BASE)))?;
            p.add_fillet(body, 0.5, vec![e])
        }
        Reference::Boss => {
            let top = row.top.as_ref().ok_or("no top face")?;
            let z = (top.z)(BASE);
            let near = |f: &&qymcad_core::geom::MeshFace| (f.centroid.x - top.at[0]).powi(2) + (f.centroid.y - top.at[1]).powi(2);
            let f = p
                .regen_faces
                .get(&body)
                .and_then(|fs| fs.iter().filter(|f| f.normal[2] > 0.99 && (f.centroid.z - z).abs() < 1e-6).min_by(|a, b| near(a).total_cmp(&near(b))))
                .cloned()
                .ok_or("no top face")?;
            let key = qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
            let si = p.new_sketch("Boss");
            let sid = p.sketches[si].id;
            p.sketches[si].plane = qymcad_core::feature::SketchPlane::Face(body, key);
            p.add_sketch_node(sid, "Boss");
            p.add_circle_entity(si, top.at[0], top.at[1], 1.0, Purpose::Real);
            p.regen_sketch(si);
            let c: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
            let span = qymcad_core::model::CombineSpan { height: 2.0, down: 0.0, extent: qymcad_core::feature::Extent { reach: Reach::Forward, ..Default::default() }, fill: &[] };
            p.add_combine_multi_op(body, sid, c, span, 1)
        }
        Reference::Mate => {
            let e = edge_at(&p, body, (row.edge)(BASE)).ok_or_else(|| format!("no edge at {:?} before the edit", (row.edge)(BASE)))?;
            let owner = part.ok_or("the body stands in no part")?;
            p.add_connector(owner, qymcad_core::feature::AnchorRef::EdgeMid(body, e))
        }
    };
    let raised = qymcad_testkit::restore_import_shapes(&p);
    let (report, _) = qymcad_testkit::regenerate_with_shapes(&mut p, raised);
    if let Some((_, err)) = report.errors.iter().find(|(id, _)| *id == fillet) {
        return Err(format!("the reference refused before the edit - {err:?}"));
    }
    if reference == Reference::Mate && mate_at(&p, fillet).is_none() {
        return Err("the mate does not stand anywhere before the edit".into());
    }
    Ok((p, Laid { base: b, between, fillet }))
}

/// Where the connector `id` stands, in the frame of its part.
fn mate_at(p: &Project, id: u64) -> Option<[f64; 3]> {
    let conn = p.connectors.iter().find(|c| c.id == id)?;
    p.connector_frame(conn).map(|f| f.origin)
}

/// Where a connector laid now, by the owner of connector `id`, on the edge of its body standing at `at` would stand: on
/// an arc that is the centre of the arc, not the middle of the edge.
fn fresh_mate_at(p: &Project, id: u64, at: [f64; 3]) -> Option<[f64; 3]> {
    let conn = p.connectors.iter().find(|c| c.id == id)?;
    let qymcad_core::feature::AnchorRef::EdgeMid(body, _) = conn.anchor else { return None };
    let e = edge_at(p, body, at)?;
    let mut fresh = p.clone();
    let laid = fresh.add_connector(conn.owner, qymcad_core::feature::AnchorRef::EdgeMid(body, e));
    mate_at(&fresh, laid)
}

/// The body a fillet node works on, and the first edge name it holds.
fn fillet_target(p: &Project, fillet: u64) -> Option<(u64, u32)> {
    p.timeline.iter().find(|n| n.id == fillet).and_then(|n| match &n.kind {
        FeatureKind::Fillet { src, edges, .. } => edges.query.picked_descs().first().map(|d| (*src, *d)),
        _ => None,
    })
}

#[test]
fn a_reference_holds_its_edge_through_an_edit_above() {
    let mut fails: Vec<String> = Vec::new();
    for row in &ROWS {
        for edit in &EDITS {
            let cell = format!("{} / {}", row.name, edit.name);
            let (p, laid) = match lay(row, edit.name.contains("suppressed"), edit.left, Reference::Fillet) {
                Ok(v) => v,
                Err(e) => {
                    fails.push(format!("{cell}: setup - {e}"));
                    continue;
                }
            };
            let mut q = (edit.apply)(p, &laid);
            let raised = qymcad_testkit::restore_import_shapes(&q);
            let (report, _) = qymcad_testkit::regenerate_with_shapes(&mut q, raised);
            if let Some((_, err)) = report.errors.iter().find(|(id, _)| *id == laid.fillet) {
                fails.push(format!("{cell}: the fillet refused after the edit - {err:?}"));
                continue;
            }
            let Some((on, name)) = fillet_target(&q, laid.fillet) else {
                fails.push(format!("{cell}: the fillet holds no edge"));
                continue;
            };
            let want = (row.edge)(edit.size);
            match q.regen_edges.get(&on).and_then(|es| es.iter().find(|e| e.id == name)) {
                None => fails.push(format!("{cell}: the edge it holds is gone from the body, it should stand at {want:?}")),
                Some(e) if (0..3).any(|i| (e.mid[i] - want[i]).abs() > 1e-3) => fails.push(format!("{cell}: the reference slid to {:?}, the edge stands at {want:?}", e.mid)),
                Some(_) => {}
            }
        }
    }
    assert!(fails.is_empty(), "\nREFERENCES AFTER AN EDIT ABOVE ({}):\n{}", fails.len(), fails.join("\n"));
}

/// A MATE HOLDS ITS EDGE through an edit above: a connector of the part on an edge of the result stands after the edit
/// where a connector laid then on that edge would stand - the anchor a joint of an assembly takes, held by the name of
/// the edge as a rounding is.
#[test]
fn a_mate_holds_its_edge_through_an_edit_above() {
    let mut fails: Vec<String> = Vec::new();
    for row in &ROWS {
        for edit in &EDITS {
            let cell = format!("{} / {}", row.name, edit.name);
            let (p, laid) = match lay(row, edit.name.contains("suppressed"), edit.left, Reference::Mate) {
                Ok(v) => v,
                Err(e) => {
                    fails.push(format!("{cell}: setup - {e}"));
                    continue;
                }
            };
            let mut q = (edit.apply)(p, &laid);
            let raised = qymcad_testkit::restore_import_shapes(&q);
            let _ = qymcad_testkit::regenerate_with_shapes(&mut q, raised);
            let edge = (row.edge)(edit.size);
            let Some(want) = fresh_mate_at(&q, laid.fillet, edge) else {
                fails.push(format!("{cell}: no edge of the body stands at {edge:?} after the edit"));
                continue;
            };
            match mate_at(&q, laid.fillet) {
                None => fails.push(format!("{cell}: the mate stands nowhere after the edit, a mate on the edge at {edge:?} stands at {want:?}")),
                Some(at) if (0..3).any(|i| (at[i] - want[i]).abs() > 1e-3) => fails.push(format!("{cell}: the mate slid to {at:?}, a mate on the edge at {edge:?} stands at {want:?}")),
                Some(_) => {}
            }
        }
    }
    assert!(fails.is_empty(), "\nMATES AFTER AN EDIT ABOVE ({}):\n{}", fails.len(), fails.join("\n"));
}

/// A SKETCH LAID ON A FACE HOLDS THE FACE through an edit above: the boss on it stands on the top face where the top
/// face stands after the edit, at the spot it was drawn.
#[test]
fn a_sketch_on_a_face_holds_its_face_through_an_edit_above() {
    let mut fails: Vec<String> = Vec::new();
    for row in ROWS.iter().filter(|r| r.top.is_some()) {
        let Some(top) = row.top.as_ref() else { continue };
        for edit in &EDITS {
            let cell = format!("{} / {}", row.name, edit.name);
            let (p, laid) = match lay(row, edit.name.contains("suppressed"), edit.left, Reference::Boss) {
                Ok(v) => v,
                Err(e) => {
                    fails.push(format!("{cell}: setup - {e}"));
                    continue;
                }
            };
            let mut q = (edit.apply)(p, &laid);
            let raised = qymcad_testkit::restore_import_shapes(&q);
            let (report, _) = qymcad_testkit::regenerate_with_shapes(&mut q, raised);
            if let Some((_, err)) = report.errors.iter().find(|(id, _)| *id == laid.fillet) {
                fails.push(format!("{cell}: the boss refused after the edit - {err:?}"));
                continue;
            }
            let want = (top.z)(edit.size) + 2.0;
            let Some(mesh) = q.mesh_index(laid.fillet).map(|i| &q.bodies[i].mesh) else {
                fails.push(format!("{cell}: the boss has no body"));
                continue;
            };
            let peak: Vec<&qymcad_core::geom::Point3> = mesh.verts.iter().filter(|v| (v.z - want).abs() < 1e-6).collect();
            let on_spot = !peak.is_empty() && peak.iter().all(|v| ((v.x - top.at[0]).powi(2) + (v.y - top.at[1]).powi(2)).sqrt() < 1.0 + 1e-3);
            let highest = mesh.verts.iter().map(|v| v.z).fold(f64::MIN, f64::max);
            if !on_spot || (highest - want).abs() > 1e-6 {
                fails.push(format!("{cell}: the boss tops out at {highest:.4} ({} points at {want:.4}), it should stand on the top face at {:?}, up to {want:.4}", peak.len(), top.at));
            }
        }
    }
    assert!(fails.is_empty(), "\nSKETCHES ON A FACE AFTER AN EDIT ABOVE ({}):\n{}", fails.len(), fails.join("\n"));
}

/// A BODY BROUGHT IN FROM A FILE HOLDS ITS NAMES WHEN ITS SCALE IS CHANGED, and after saving and opening at the new
/// scale: a fillet on an edge of it rounds the same edge, at twice its place. The window rescales the live body with its
/// names; opening raises it from the file at the new scale, and its faces are named by where they lie - so where they
/// lie has to be told in the frame of the file, or every name changes with the scale.
#[test]
fn a_rescaled_import_holds_its_names_through_opening() {
    let row = ROWS.iter().find(|r| r.name.contains("STEP")).expect("the row of an import");
    let (mut p, laid) = lay(row, false, Left::Straight, Reference::Fillet).expect("laid");
    let (body, _) = fillet_target(&p, laid.fillet).expect("the fillet's body");
    // rescaled as the window does it: the factor kept by the import node, the live body at the factor with its names
    let mut live = qymcad_testkit::restore_import_shapes(&p);
    let raised = live.remove(&body).expect("the body raised");
    let mut shapes: std::collections::HashMap<u64, qymcad_kernel::Shape> = live;
    shapes.insert(body, raised);
    let (_, mut shapes) = qymcad_testkit::regenerate_with_shapes(&mut p, shapes);
    let named = shapes.remove(&body).expect("the live body named");
    p.set_import_scale(body, 2.0);
    shapes.insert(body, qymcad_kernel::specimen::placed(&named, &[2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0]).expect("scaled"));
    let (report, _) = qymcad_testkit::regenerate_with_shapes(&mut p, shapes);
    assert!(report.errors.iter().all(|(id, _)| *id != laid.fillet), "the fillet refused at the new scale: {:?}", report.errors);
    let q = (EDITS.iter().find(|e| e.name.contains("saved")).expect("the edit of saving").apply)(p, &laid);
    let mut q = q;
    let raised = qymcad_testkit::restore_import_shapes(&q);
    let (report, _) = qymcad_testkit::regenerate_with_shapes(&mut q, raised);
    assert!(report.errors.iter().all(|(id, _)| *id != laid.fillet), "the fillet refused after opening at the new scale: {:?}", report.errors);
    let (on, name) = fillet_target(&q, laid.fillet).expect("the fillet holds an edge");
    let want = (row.edge)(BASE).map(|v| v * 2.0);
    let at = q.regen_edges.get(&on).and_then(|es| es.iter().find(|e| e.id == name)).map(|e| e.mid);
    assert!(at.is_some_and(|m| (0..3).all(|i| (m[i] - want[i]).abs() < 1e-3)), "after opening at twice the scale the fillet holds an edge at {at:?}, the edge stands at {want:?}");
}

/// THE KINDS THIS MATRIX HAS NO ROW FOR, and why: no edit of the base above reaches them, or neither of the two
/// references it lays stands on what they build.
const NOT_HERE: [qymcad_testkit::Uncovered; 21] = [
    qymcad_testkit::Uncovered { kind: "Sketch", why: "a sketch builds no body" },
    qymcad_testkit::Uncovered { kind: "Plane", why: "a datum plane builds no body" },
    qymcad_testkit::Uncovered { kind: "DatumPoint", why: "a datum point builds no body" },
    qymcad_testkit::Uncovered { kind: "DatumAxis", why: "a datum axis builds no body" },
    qymcad_testkit::Uncovered { kind: "PartInstance", why: "a copy of a part placed in an assembly repeats the body of the part, its names with it" },
    qymcad_testkit::Uncovered { kind: "ComponentPattern", why: "the copies of a pattern of parts repeat the body of the part, its names with it" },
    qymcad_testkit::Uncovered { kind: "MeshPiece", why: "a piece of a mesh has no B-rep and no node above it" },
    qymcad_testkit::Uncovered { kind: "Cylinder", why: "a primitive is built from its sizes alone: an edit of the base reaches nothing of it" },
    qymcad_testkit::Uncovered { kind: "Sphere", why: "a primitive is built from its sizes alone: an edit of the base reaches nothing of it" },
    qymcad_testkit::Uncovered { kind: "Cone", why: "a primitive is built from its sizes alone: an edit of the base reaches nothing of it" },
    qymcad_testkit::Uncovered { kind: "Torus", why: "a primitive is built from its sizes alone: an edit of the base reaches nothing of it" },
    qymcad_testkit::Uncovered { kind: "Prism", why: "a primitive is built from its sizes alone: an edit of the base reaches nothing of it" },
    qymcad_testkit::Uncovered { kind: "Loft", why: "a loft is built from its own sections: an edit of the base reaches nothing of it" },
    qymcad_testkit::Uncovered { kind: "Sweep", why: "a sweep is built from its own profile and path: an edit of the base reaches nothing of it" },
    qymcad_testkit::Uncovered { kind: "MeshSolid", why: "a body of a mesh is built from the mesh: an edit of the base reaches nothing of it" },
    qymcad_testkit::Uncovered { kind: "MeshRecognised", why: "a body of a mesh is built from the mesh: an edit of the base reaches nothing of it" },
    qymcad_testkit::Uncovered { kind: "FaceCopy", why: "a sheet: neither a rounding nor a boss, the two references laid here, stands on it" },
    qymcad_testkit::Uncovered { kind: "OffsetSurface", why: "a sheet: neither a rounding nor a boss, the two references laid here, stands on it" },
    qymcad_testkit::Uncovered { kind: "Patch", why: "a sheet: neither a rounding nor a boss, the two references laid here, stands on it" },
    qymcad_testkit::Uncovered { kind: "Stitch", why: "a sheet: neither a rounding nor a boss, the two references laid here, stands on it" },
    qymcad_testkit::Uncovered { kind: "Trim", why: "a sheet: neither a rounding nor a boss, the two references laid here, stands on it" },
];

/// EVERY KIND OF NODE HAS A ROW HERE, or a reason for none.
#[test]
fn every_kind_of_node_has_a_row() {
    let kinds: Vec<&str> = ROWS.iter().map(|r| r.kind).collect();
    let gaps = qymcad_testkit::coverage_gaps(&kinds, &NOT_HERE);
    assert!(gaps.is_empty(), "the matrix of edits leaves kinds of node unseen:\n{}", gaps.join("\n"));
}
