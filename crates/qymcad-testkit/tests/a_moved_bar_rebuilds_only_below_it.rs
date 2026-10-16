//! A MOVED ROLLBACK BAR REBUILDS ONLY WHAT STANDS BELOW IT: what is above both of its places rests on nothing it moved.
//!
//! Reported behaviour: the bar dragged one row up, above a rounding at the end of a part with two threads, and the
//! card counted all 27 nodes of the timeline, "a thread among them", for minutes. Every move of the bar marked every
//! node, the threads above it included.
use qymcad_core::feature::FeatureKind;
use qymcad_core::model::{Id, Project};
use qymcad_core::thread::{ThreadSpec, ThreadStandard};
use std::collections::HashMap;

/// A sleeve of 30 x 20 with a hole of 8.5, M10 inside it, and its outer rims rounded: the thread above, the rounding
/// at the end of the timeline. Built whole, with its live shapes.
struct Sleeve {
    p: Project,
    shapes: HashMap<Id, qymcad_kernel::Shape>,
    threaded: Id,
    rounded: Id,
}

fn sleeve() -> Sleeve {
    let mut p = Project::default();
    p.new_document();
    let outer = p.add_cylinder(15.0, 20.0);
    let hole = p.add_cylinder(4.25, 30.0);
    let sleeve = p.add_body_boolean(outer, hole, 0);
    let rim = qymcad_testkit::round_edge(&mut p, sleeve, 4.25);
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 10.0, pitch: 1.5, internal: true, fit: 0.0, ..Default::default() };
    let threaded = p.add_thread(sleeve, rim, spec, 8.0, 0.0, 0.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    let rims: Vec<u32> = p.regen_edges.get(&threaded).map(|es| es.iter().filter(|e| (e.radius - 15.0).abs() < 0.05).map(|e| e.id).collect()).unwrap_or_default();
    assert!(!rims.is_empty(), "setup: the outer rims of the threaded sleeve");
    let rounded = p.add_fillet(threaded, 1.0, rims);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the sleeve does not build: {:?}", report.errors);
    Sleeve { p, shapes, threaded, rounded }
}

fn index_of(p: &Project, body: Id) -> usize {
    p.timeline.iter().position(|n| n.kind.body() == Some(body)).expect("the node of the body")
}

fn threads_in_plan(p: &Project) -> Vec<String> {
    let plan = p.regen_plan();
    p.timeline.iter().filter(|n| plan.nodes.contains(&n.id) && matches!(n.kind, FeatureKind::Thread { .. })).map(|n| n.name.clone()).collect()
}

/// WHAT A BODY IS, to compare a rebuild with another: on screen or not, its volume, its faces.
#[derive(Debug, PartialEq)]
struct Seen {
    shown: bool,
    volume: Option<i64>,
    faces: usize,
}

fn seen(p: &Project, shapes: &HashMap<Id, qymcad_kernel::Shape>, body: Id) -> Seen {
    let shown = p.mesh_index(body).is_some();
    // to a hundredth of a mm^3: the same recipe built twice gives the same body
    let volume = shown.then(|| shapes.get(&body).map(|s| (s.volume() * 100.0).round() as i64)).flatten();
    Seen { shown, volume, faces: p.regen_faces.get(&body).map(|f| f.len()).unwrap_or(0) }
}

/// A document built whole, with its live shapes.
struct Whole {
    p: Project,
    shapes: HashMap<Id, qymcad_kernel::Shape>,
}

/// The same document, the bar at `at`, built whole from nothing: what a move of the bar is to come to.
fn built_whole(p: &Project, at: Option<usize>) -> Whole {
    let mut whole = p.clone();
    whole.set_rollback(at);
    for n in &mut whole.timeline {
        n.dirty = true;
    }
    let (_, shapes) = qymcad_testkit::regenerate(&mut whole);
    Whole { p: whole, shapes }
}

#[test]
fn the_bar_one_row_up_rebuilds_no_thread_above_it() {
    let Sleeve { mut p, shapes, threaded, rounded } = sleeve();
    let at = Some(index_of(&p, rounded));
    p.set_rollback(at);
    let threads = threads_in_plan(&p);
    assert!(threads.is_empty() && !p.regen_plan().heavy, "the bar went one row up, above the rounding, and the rebuild takes the threads above it: {threads:?}");
    let (_, shapes) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    let whole = built_whole(&p, at);
    for body in [threaded, rounded] {
        assert_eq!(seen(&p, &shapes, body), seen(&whole.p, &whole.shapes, body), "{:?} after the bar went up is not what a rebuild of the whole gives", qymcad_testkit::kind_making(&p, body));
    }
}

#[test]
fn the_bar_back_down_rebuilds_only_what_it_switches_on() {
    let Sleeve { mut p, mut shapes, threaded, rounded } = sleeve();
    for up in [index_of(&p, rounded), index_of(&p, threaded)] {
        p.set_rollback(Some(up));
        shapes = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes).1;
        p.set_rollback(None);
        let plan = p.regen_plan();
        let built: Vec<String> = p.timeline.iter().filter(|n| plan.nodes.contains(&n.id)).map(|n| n.name.clone()).collect();
        let below: Vec<String> = p.timeline.iter().skip(up).map(|n| n.name.clone()).collect();
        assert_eq!(built, below, "the bar went back down from row {up} and the rebuild takes more than the rows it switches on");
        shapes = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes).1;
        let whole = built_whole(&p, None);
        for body in [threaded, rounded] {
            assert_eq!(
                seen(&p, &shapes, body),
                seen(&whole.p, &whole.shapes, body),
                "{:?} after the bar went back down from row {up} is not what a rebuild of the whole gives",
                qymcad_testkit::kind_making(&p, body)
            );
        }
    }
}

/// Ctrl+Z over a move of the bar raises again the live bodies only of the rows whose side of the bar changed.
#[test]
fn undoing_a_move_of_the_bar_names_only_the_bodies_it_moved_across() {
    let Sleeve { p, threaded, rounded, .. } = sleeve();
    let mut up = p.clone();
    up.set_rollback(Some(index_of(&p, rounded)));
    let check = |way: &str, changed: Vec<Id>| {
        assert!(changed.contains(&rounded), "the bar moved {way} across the rounding and its body does not count as changed");
        assert!(!changed.contains(&threaded), "the bar moved {way} one row, below the thread, and the threaded sleeve counts as changed: {changed:?}");
    };
    check("up", up.changed_bodies_vs(&p));
    check("back down", p.changed_bodies_vs(&up));
}
