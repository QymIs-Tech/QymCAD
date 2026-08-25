//! A FACE IS ONE CONNECTED PATCH OF SURFACE. Not two different places on the part under one name.
//!
//! Reported behaviour: the "push face" tool selected TWO faces from a single click, and the push
//! lifted two opposite rims of a wall instead of one.
//!
//! There was no mis-click. On the part in question the top rim of the frame — face `0x40000075` —
//! turned out to be PINCHED: a fillet ate the rim at the front, a chamfer at the back, and the inner
//! contour ran into the outer one. What is left lies as two strips (x in [0,2] and x in [48,50]),
//! and in the kernel that is still ONE face with one id. Everything follows from that: a click
//! highlights both, a push lifts both, a fillet cuts both, and there is NOTHING to select just one
//! with.
//!
//! The same pinching has a milder second face: while the rim is eaten from ONE side only the patch
//! stays connected, but the contour is already unusable — and the tessellator gives up silently. The
//! face disappears from the screen entirely and the volume of the part is computed from a holed mesh.
//!
//! Its relative is `edge_names_unique.rs` (two edges under one name). There a name was handed out
//! twice; here the name is honest and it is the face itself that fell apart with nobody splitting it.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

/// Planar faces of a body that look upward and lie at the very top.
fn top_faces(p: &Project, body: u64) -> Vec<qymcad_core::geom::MeshFace> {
    p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9 && f.centroid.z > 49.0).cloned().collect()
}

/// The frame is built ONCE for the whole file, and that is not only about speed. The low-level OCCT
/// booleans that take the pinching apart share state with the rest of the kernel that is not built
/// for concurrency: while another part is being computed in a neighbouring thread, the answer drifts.
/// Tests within a file run in parallel — hence the geometry is computed under a `OnceLock` and each
/// test gets a copy.
fn frame_with_a_chewed_rim() -> (Project, u64, u64) {
    static ONCE: std::sync::OnceLock<(Project, u64, u64)> = std::sync::OnceLock::new();
    let (p, a, b) = ONCE.get_or_init(build_frame_with_a_chewed_rim);
    (p.clone(), *a, *b)
}

/// A 50x50x50 FRAME with a 2 mm wall. Returns (project, body after the FILLET, body after the
/// CHAMFER): the fillet eats the top rim at the front, the chamfer at the back. Exactly the recipe of
/// the real part, boiled down to its essence.
fn build_frame_with_a_chewed_rim() -> (Project, u64, u64) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Sketch 1",
        vec![Point2::new(0.0, 0.0), Point2::new(50.0, 0.0), Point2::new(50.0, 50.0), Point2::new(0.0, 50.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Sketch 1");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let boxy = p.add_extrude_multi(sid, closed, 50.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);

    // an open frame: the top and the bottom are removed
    let open: Vec<u32> = p.regen_faces[&boxy].iter().filter(|f| f.normal[2].abs() > 0.9).map(|f| f.id).collect();
    assert_eq!(open.len(), 2, "the box has a top and a bottom");
    let frame = p.add_shell_mode(boxy, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);

    // the FRONT rim is eaten by a fillet: two top edges at y about 0 and y about 2, radius 1 — exactly
    // the full width of the wall, as in the real part.
    let front: Vec<u32> = p.regen_edges[&frame]
        .iter()
        .filter(|e| (e.a[2] - 50.0).abs() < 1e-6 && (e.b[2] - 50.0).abs() < 1e-6 && e.mid[1] < 2.5)
        .map(|e| e.id)
        .collect();
    assert_eq!(front.len(), 2, "there are two edges at the top front (outer and inner), and {} were found", front.len());
    let filleted = p.add_fillet_ref(frame, 1.0, qymcad_core::refs::Ref::picks(&front));
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the fillet must build: {:?}", rep.errors);

    // the BACK rim is eaten by a chamfer on the outer edge at y about 50.
    let back: Vec<u32> = p.regen_edges[&filleted]
        .iter()
        .filter(|e| (e.a[2] - 50.0).abs() < 1e-6 && (e.b[2] - 50.0).abs() < 1e-6 && e.mid[1] > 49.5)
        .map(|e| e.id)
        .collect();
    assert_eq!(back.len(), 1, "there is one outer edge at the top back, and {} were found", back.len());
    let chamfered = p.add_chamfer_ref(filleted, 2.0, qymcad_core::refs::Ref::picks(&back));
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the chamfer must build: {:?}", rep.errors);
    (p, filleted, chamfered)
}

/// A RIM EATEN FROM ONE SIDE STAYS ON THE PART.
///
/// The fillet ate the front strip — three remain, in a U shape, of area 384-96=288. The patch is
/// connected, but the contour after such a cut is unusable and the tessellator silently returned
/// nothing: the rim was neither visible nor selectable, and the mesh volume drifted (14379 instead of
/// 19200 — less than the part WITH the chamfer, even though a chamfer only removes material).
#[test]
fn a_rim_eaten_from_one_side_does_not_vanish_from_the_part() {
    let (p, filleted, _) = frame_with_a_chewed_rim();
    let top = top_faces(&p, filleted);
    assert_eq!(top.len(), 1, "after the fillet the rim is one U-shaped face, and {} were found: {:?}", top.len(), top.iter().map(|f| f.area).collect::<Vec<_>>());
    assert!((top[0].area - 288.0).abs() < 1.0, "the rim area must be 288 mm^2 (384 minus the eaten strip), and it came out {:.2}", top[0].area);
    assert!(!top[0].triangles.is_empty(), "the rim must be in the mesh: without triangles it can be neither seen nor selected");

    // and the mesh became honest: the FRAME holds more material than the same frame with the chamfer taken
    let v = p.bodies.iter().find(|b| b.id == filleted).unwrap().mesh.volume();
    assert!((v - 19178.5).abs() < 1.0, "the volume of the filleted frame is about 19178.5 mm^3, and the mesh gives {v:.2} — there is a hole in it");
}

/// A PINCHED RIM IS TWO FACES, NOT ONE.
#[test]
fn a_pinched_rim_is_two_faces_not_one() {
    let (p, _, body) = frame_with_a_chewed_rim();
    let top = top_faces(&p, body);
    assert_eq!(
        top.len(),
        2,
        "after a fillet at the front and a chamfer at the back the rim lies as TWO strips — so there are two faces, and {} were found: {:?}",
        top.len(),
        top.iter().map(|f| (format!("{:#x}", f.id), f.area)).collect::<Vec<_>>()
    );
    for f in &top {
        assert!((f.area - 94.0).abs() < 0.5, "each strip is 2x47 mm = 94 mm^2, and it came out {:.2}", f.area);
    }
    assert_ne!(top[0].id, top[1].id, "the two faces must have DIFFERENT names, otherwise there is nothing to select one with");
}

/// AND THE CONSEQUENCE A PERSON SEES: a push lifts ONE strip, not both at once.
#[test]
fn pushing_one_rim_strip_lifts_only_it() {
    let (mut p, _, body) = frame_with_a_chewed_rim();
    let v0 = p.bodies.iter().find(|b| b.id == body).expect("the body").mesh.volume();
    let strip = top_faces(&p, body).into_iter().min_by(|a, b| a.centroid.x.total_cmp(&b.centroid.x)).expect("the left rim strip");

    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [strip.centroid.x, strip.centroid.y, strip.centroid.z], normal: strip.normal, id: strip.id };
    let pushed = p.add_push_face(body, key, 5.0);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the push must build: {:?}", rep.errors);

    let v1 = p.bodies.iter().find(|b| b.id == pushed).expect("the new body").mesh.volume();
    let want = strip.area * 5.0;
    assert!(
        (v1 - v0 - want).abs() < 1.0,
        "pushing a strip of S={:.2} by 5 mm must add {want:.2} mm^3, and it added {:.2} — more than one face went up",
        strip.area,
        v1 - v0
    );
}

/// AND THE SECOND STRIP PUSHES JUST LIKE THE FIRST.
///
/// Reported behaviour: the opposite face pushes without trouble while this particular one does not —
/// the face is no longer in the source body, the reference is stale. There is one difference between
/// the strips: the first kept the name of the source face, while the second gets one derived ("piece
/// 1 of face N"). That derivation ran AFTER the faces had gone out to the application — the mouse was
/// clicking on a positional number while resolution searched by name. What can be clicked is only
/// what has been recorded.
#[test]
fn the_other_rim_strip_pushes_too() {
    let (mut p, _, body) = frame_with_a_chewed_rim();
    let v0 = p.bodies.iter().find(|b| b.id == body).expect("the body").mesh.volume();
    // the RIGHT strip is the one whose name is derived
    let strip = top_faces(&p, body).into_iter().max_by(|a, b| a.centroid.x.total_cmp(&b.centroid.x)).expect("the right rim strip");
    assert!(strip.centroid.x > 25.0, "take exactly the opposite strip");

    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [strip.centroid.x, strip.centroid.y, strip.centroid.z], normal: strip.normal, id: strip.id };
    let pushed = p.add_push_face(body, key, 5.0);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "pushing the second strip must build just like the first: {:?}", rep.errors);
    let v1 = p.bodies.iter().find(|b| b.id == pushed).expect("the new body").mesh.volume();
    assert!((v1 - v0 - strip.area * 5.0).abs() < 1.0, "the wrong strip went up: added {:.2} instead of {:.2}", v1 - v0, strip.area * 5.0);
}
