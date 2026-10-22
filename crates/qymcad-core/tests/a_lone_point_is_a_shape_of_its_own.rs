//! A LONE POINT IS A SHAPE OF ITS OWN: the editing tools take it as they take a curve - Move and Rotate carry it, Copy
//! and Mirror lay another, a pattern lays its copies and takes them back - and a curve deleted elsewhere leaves it.
//!
//! Reported behaviour: a point drawn alone was nothing to Move, Copy, Rotate, Mirror and the patterns, and it went away
//! when any line of the sketch was deleted.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{PatternKind, Project};

const NEAR: f64 = 1e-6;

/// A SKETCH HOLDING ONE POINT drawn alone at (10, 5).
struct Lone {
    p: Project,
    si: usize,
    pid: u64,
}

fn lone_point() -> Lone {
    let mut p = Project::default();
    let si = p.new_sketch("lone");
    let pid = p.sketch_point_at(si, 10.0, 5.0, 1e-6);
    Lone { p, si, pid }
}

fn at(p: &Project, si: usize, id: u64) -> [f64; 2] {
    let q = p.sketches[si].points.iter().find(|q| q.id == id).unwrap_or_else(|| panic!("no point {id}"));
    [q.x, q.y]
}

fn stands(p: &Project, si: usize, want: [f64; 2]) -> bool {
    p.sketches[si].points.iter().any(|q| (q.x - want[0]).abs() < NEAR && (q.y - want[1]).abs() < NEAR)
}

#[test]
fn a_curve_deleted_elsewhere_leaves_the_point() {
    let Lone { mut p, si, pid } = lone_point();
    let line = p.add_line_entity(si, -20.0, -20.0, -10.0, -20.0, Purpose::Real);
    p.delete_entities(si, &[line]);
    assert!(p.sketches[si].points.iter().any(|q| q.id == pid), "the point drawn alone went with a line 20 away");
}

#[test]
fn move_and_rotate_carry_the_point() {
    let Lone { mut p, si, pid } = lone_point();
    p.move_entities(si, &[pid], 3.0, -2.0);
    assert_eq!(at(&p, si, pid).map(|v| (v * 1e6).round() / 1e6), [13.0, 3.0], "moved by (3, -2)");
    p.rotate_entities(si, &[pid], 0.0, 0.0, 90.0);
    let [x, y] = at(&p, si, pid);
    assert!((x + 3.0).abs() < NEAR && (y - 13.0).abs() < NEAR, "turned 90 deg about the origin to (-3, 13), stands at ({x}, {y})");
}

#[test]
fn copy_and_mirror_lay_another_point() {
    let Lone { mut p, si, pid } = lone_point();
    let copies = p.copy_entities(si, &[pid], 0.0, 4.0);
    assert_eq!(copies.len(), 1, "one copy of one point, got {copies:?}");
    assert_eq!(at(&p, si, copies[0]).map(|v| (v * 1e6).round() / 1e6), [10.0, 9.0], "the copy 4 above");
    p.mirror_entities(si, &[pid], 0.0, 0.0, 0.0, 1.0);
    assert!(stands(&p, si, [-10.0, 5.0]), "no mirror image across the Y axis at (-10, 5)");
    assert_eq!(at(&p, si, pid), [10.0, 5.0], "the source stays");
}

#[test]
fn a_pattern_lays_its_copies_and_takes_them_back() {
    let Lone { mut p, si, pid } = lone_point();
    let three = PatternKind::Linear { dx: 5.0, dy: 0.0, count: 3, dx2: 0.0, dy2: 0.0, count2: 1 };
    p.add_pattern(si, &[pid], three);
    assert!(stands(&p, si, [15.0, 5.0]) && stands(&p, si, [20.0, 5.0]), "no copies at 15 and 20");
    let two = PatternKind::Linear { dx: 5.0, dy: 0.0, count: 2, dx2: 0.0, dy2: 0.0, count2: 1 };
    p.update_pattern(si, 0, two);
    assert!(stands(&p, si, [15.0, 5.0]), "the copy at 15 went with the count 3 -> 2");
    assert!(!stands(&p, si, [20.0, 5.0]), "the copy at 20 stayed after the count went 3 -> 2");
    p.delete_pattern(si, 0);
    assert!(!stands(&p, si, [15.0, 5.0]), "the copy at 15 stayed after the pattern was deleted");
    assert!(stands(&p, si, [10.0, 5.0]), "the source went with its pattern");
}

#[test]
fn the_end_of_a_line_deleted_takes_the_line_whole_and_leaves_the_point() {
    let Lone { mut p, si, pid } = lone_point();
    p.add_line_entity(si, -20.0, -20.0, -10.0, -20.0, Purpose::Real);
    let end = p.sketch_point_at(si, -10.0, -20.0, 1e-6);
    p.delete_points(si, &[end]);
    assert!(!stands(&p, si, [-20.0, -20.0]), "the far end of the line stayed a point of its own after its end was deleted");
    assert!(p.sketches[si].points.iter().any(|q| q.id == pid), "the point drawn alone went with the end of a line");
}
