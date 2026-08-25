//! A COMPONENT PATTERN — INSTANCE copies, not "insert the part again".
//!
//! Body patterns existed, component patterns did not: bolts around a circle were placed by hand.
//! What is checked here is exactly what separates a pattern from manual placement: the copies stand
//! in their places, an edit of the source part reaches all of them, an edit of the layout moves the
//! row, and deleting the pattern takes the copies away without touching the source.
use qymcad_core::model::{CompPatternKind, Project};

/// An assembly with a single 20x20x10 box part at the origin. Returns (project, part component).
fn assembly_with_part() -> (Project, u64) {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let part = p.add_part("Bolt");
    p.set_active_component(Some(part));
    p.add_box(20.0, 20.0, 10.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    (p, part)
}

/// The position of a component within its parent.
fn pos(p: &Project, c: u64) -> [f64; 3] {
    let t = p.component_transform(c);
    [t[3], t[7], t[11]]
}

/// The volume of a component's body (0 means there is no body).
fn body_volume(p: &Project, c: u64) -> f64 {
    p.component_bodies(c).first().and_then(|b| p.bodies.iter().find(|x| x.id == *b)).map(|b| b.mesh.volume()).unwrap_or(0.0)
}

/// A LINEAR PATTERN: 4 instances at a step of 30 — three copies in their places, each with a body.
#[test]
fn a_linear_pattern_places_real_copies_with_geometry() {
    let (mut p, part) = assembly_with_part();
    let id = p.add_comp_pattern(part, CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 30.0, count: 4 });
    assert_ne!(id, 0, "the pattern must be created");
    let _ = qymcad_testkit::regenerate(&mut p);

    let copies = p.comp_pattern_of(part).expect("the pattern was found").copies.clone();
    assert_eq!(copies.len(), 3, "4 instances = the source plus 3 copies, and there are {} copies", copies.len());
    for (i, c) in copies.iter().enumerate() {
        let want = 30.0 * (i + 1) as f64;
        let got = pos(&p, *c);
        assert!((got[0] - want).abs() < 1e-9, "copy {i} must stand at {want}, and it stands at {got:?}");
        let v = body_volume(&p, *c);
        assert!((v - 4000.0).abs() < 1.0, "copy {i} must have the BODY of the source (4000), and it came out {v}");
    }
}

/// A CIRCULAR PATTERN: 6 instances over a full circle — copies every 60 degrees, at the radius of the
/// source.
#[test]
fn a_circular_pattern_spreads_copies_around_the_axis() {
    let (mut p, part) = assembly_with_part();
    // move the source off the axis, otherwise there is nothing to revolve
    p.set_component_transform(part, [1.0, 0.0, 0.0, 50.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let id = p.add_comp_pattern(part, CompPatternKind::Circular { origin: [0.0; 3], dir: [0.0, 0.0, 1.0], angle: 360.0, count: 6 });
    assert_ne!(id, 0, "the pattern must be created");
    let _ = qymcad_testkit::regenerate(&mut p);

    let copies = p.comp_pattern_of(part).expect("the pattern").copies.clone();
    assert_eq!(copies.len(), 5, "6 instances = the source plus 5 copies");
    for (i, c) in copies.iter().enumerate() {
        let a = ((i + 1) as f64 * 60.0f64).to_radians();
        let (want_x, want_y) = (50.0 * a.cos(), 50.0 * a.sin());
        let got = pos(&p, *c);
        assert!((got[0] - want_x).abs() < 1e-6 && (got[1] - want_y).abs() < 1e-6, "copy {i} must stand at {want_x:.3};{want_y:.3}, and it stands at {got:?}");
        // THE RADIUS IS PRESERVED: a copy revolves around the axis rather than sliding off in a straight line
        assert!((got[0].hypot(got[1]) - 50.0).abs() < 1e-6, "copy {i} must stay at radius 50");
    }
}

/// AN EDIT OF THE SOURCE PART REACHES EVERY COPY — that is why a copy is made an instance.
#[test]
fn editing_the_source_part_updates_every_copy() {
    let (mut p, part) = assembly_with_part();
    p.add_comp_pattern(part, CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 30.0, count: 3 });
    let _ = qymcad_testkit::regenerate(&mut p);
    let copies = p.comp_pattern_of(part).expect("the pattern").copies.clone();
    assert!((body_volume(&p, copies[0]) - 4000.0).abs() < 1.0, "setup: the copy reproduces the source");

    // the source box became twice as tall
    let src_body = p.active_body(part).expect("the source body");
    if let Some(n) = p.timeline.iter_mut().find(|n| n.kind.bodies().contains(&src_body)) {
        if let qymcad_core::feature::FeatureKind::Box3 { dz, .. } = &mut n.kind {
            *dz = 20.0;
        }
        n.dirty = true;
    }
    let _ = qymcad_testkit::regenerate(&mut p);

    for (i, c) in copies.iter().enumerate() {
        let v = body_volume(&p, *c);
        assert!((v - 8000.0).abs() < 1.0, "copy {i} must follow the source (8000), and it has {v}");
    }
}

/// MOVE THE SOURCE — the whole row follows it.
#[test]
fn moving_the_source_moves_the_whole_row() {
    let (mut p, part) = assembly_with_part();
    p.add_comp_pattern(part, CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 30.0, count: 3 });
    let _ = qymcad_testkit::regenerate(&mut p);
    let copies = p.comp_pattern_of(part).expect("the pattern").copies.clone();

    p.set_component_transform(part, [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 100.0, 0.0, 0.0, 1.0, 0.0]);
    let _ = qymcad_testkit::regenerate(&mut p);
    for (i, c) in copies.iter().enumerate() {
        let got = pos(&p, *c);
        assert!((got[1] - 100.0).abs() < 1e-9, "copy {i} must follow the source along Y, and it stands at {got:?}");
        assert!((got[0] - 30.0 * (i + 1) as f64).abs() < 1e-9, "the pattern step must be preserved");
    }
}

/// EDITING THE COUNT: copies are added and removed, while existing ones KEEP their ids.
///
/// The ids matter: mates may stand on a copy, and re-creating the pattern would break them on every
/// change of the count.
#[test]
fn changing_the_count_keeps_the_existing_copies() {
    let (mut p, part) = assembly_with_part();
    let id = p.add_comp_pattern(part, CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 30.0, count: 3 });
    let before = p.comp_pattern_of(part).expect("the pattern").copies.clone();
    assert_eq!(before.len(), 2);

    p.set_comp_pattern(id, CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 30.0, count: 5 });
    let grown = p.comp_pattern_of(part).expect("the pattern").copies.clone();
    assert_eq!(grown.len(), 4, "5 instances now = 4 copies");
    assert_eq!(&grown[..2], &before[..], "the former copies must keep their ids");

    p.set_comp_pattern(id, CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 30.0, count: 2 });
    let shrunk = p.comp_pattern_of(part).expect("the pattern").copies.clone();
    assert_eq!(shrunk.len(), 1, "2 instances now = 1 copy");
    assert_eq!(shrunk[0], before[0], "the remaining copy is the same one");
    assert!(!p.components.iter().any(|c| c.id == before[1]), "the surplus copy must leave the project");
}

/// EDITING THE STEP moves the row without re-creating it.
#[test]
fn changing_the_step_moves_the_row() {
    let (mut p, part) = assembly_with_part();
    let id = p.add_comp_pattern(part, CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 30.0, count: 3 });
    let copies = p.comp_pattern_of(part).expect("the pattern").copies.clone();
    p.set_comp_pattern(id, CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 45.0, count: 3 });
    assert_eq!(p.comp_pattern_of(part).expect("the pattern").copies, copies, "editing the step does not re-create the copies");
    assert!((pos(&p, copies[0])[0] - 45.0).abs() < 1e-9, "the first copy must move to the new step");
    assert!((pos(&p, copies[1])[0] - 90.0).abs() < 1e-9, "the second, to two steps");
}

/// DELETING THE PATTERN takes the copies away and spares the source.
#[test]
fn deleting_the_pattern_removes_copies_and_spares_the_source() {
    let (mut p, part) = assembly_with_part();
    let id = p.add_comp_pattern(part, CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 30.0, count: 4 });
    let _ = qymcad_testkit::regenerate(&mut p);
    let copies = p.comp_pattern_of(part).expect("the pattern").copies.clone();

    assert!(p.delete_comp_pattern(id), "the pattern must be deleted");
    for c in &copies {
        assert!(!p.components.iter().any(|x| x.id == *c), "copy {c} must go");
    }
    assert!(p.components.iter().any(|c| c.id == part), "THE SOURCE is the user's part and it stays");
    assert!(p.active_body(part).is_some(), "and so does its body");
    assert!(p.comp_pattern_of(part).is_none(), "the pattern record is gone");
}

/// A source with no body is not patterned — an honest refusal instead of a row of empty components.
#[test]
fn a_source_without_a_body_is_refused() {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let empty = p.add_part("Empty");
    assert_eq!(p.add_comp_pattern(empty, CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 10.0, count: 3 }), 0, "there is nothing to copy — no pattern is created");
    assert!(p.comp_patterns.is_empty(), "and no record is left behind");
}
