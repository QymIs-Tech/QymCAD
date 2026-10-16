//! THE BAR HOLDS ON MORE THAN THE TWO FAMILIAR DOCUMENTS.
//!
//! Everything that measured naming stood on two files, both taken apart by hand and both repaired
//! against measurements taken from themselves. "Works on two documents" is not yet "works on any
//! geometry", which is what the bar actually says. Until other geometry is seen, what was achieved
//! may simply be fitted to the familiar.
//!
//! Here OTHER parts are built — same tools, different order and different shape — and each one is
//! held to the same requirements as the familiar ones:
//!
//! * no two faces share a name;
//! * no two edges share a name;
//! * no face on a body is left unnamed;
//! * a rebuild with no edit at all yields THE SAME names.
use qymcad_core::feature::{BasePlane, FaceKey, SketchPlane};
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project, WorkPlane};
use qymcad_core::model::ArrayAxis;

/// Stock: a rectangular sketch turned into a prism. Each case shapes its own part from there.
fn box_body(p: &mut Project, w: f64, h: f64, up: f64) -> Id {
    let sid = p.add_line_sketch("Sketch", vec![Point2::new(0.0, 0.0), Point2::new(w, 0.0), Point2::new(w, h), Point2::new(0.0, h)], true);
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Sketch");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let b = p.add_extrude_multi(sid, closed, up, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(p);
    b
}

/// Total face area of a body. Exact for PLANAR faces, a tessellation approximation for curved ones,
/// so it proves the FACT that an operation did work ("material changed"), not an exact value.
fn area_of(p: &Project, b: Id) -> f64 {
    p.regen_faces.get(&b).map(|f| f.iter().map(|x| x.area).sum()).unwrap_or(0.0)
}

/// AN OPERATION MUST DO WORK, NOT MERELY RENAME. A "the names changed" check passes a tool glued on
/// instead of cut away, and an imprinted circle instead of a hole — both were found in this very
/// suite. So ask for a change in material.
fn demand_material_changed(title: &str, was: f64, now: f64) {
    assert!((now - was).abs() > 0.5, "{title}: the operation left the material untouched — area was {was:.2}, is now {now:.2}");
}

fn snapshot(p: &Project) -> Vec<(Id, Vec<u32>)> {
    let mut v: Vec<(Id, Vec<u32>)> = p
        .regen_faces
        .iter()
        .map(|(b, f)| {
            let mut ids: Vec<u32> = f.iter().map(|x| x.id).collect();
            ids.sort_unstable();
            (*b, ids)
        })
        .collect();
    v.sort_unstable_by_key(|(b, _)| *b);
    v
}

/// All four requirements at once, on any assembled project.
fn demand_the_bar(title: &str, p: &mut Project) {
    let consumed = p.consumed_bodies();
    let mut bad: Vec<String> = Vec::new();
    let bodies: Vec<Id> = p.regen_faces.keys().copied().filter(|b| !consumed.contains(b)).collect();
    assert!(!bodies.is_empty(), "{title}: no live bodies left — nothing to check, the case did not build");

    for b in &bodies {
        let faces: Vec<u32> = p.regen_faces.get(b).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
        // AN EMPTY BODY WOULD PASS EVERYTHING. The requirements are phrased as "no two alike" and
        // "none unnamed" — a body with no faces satisfies all of that by itself, and the check would
        // quietly degenerate into nothing. So ask whether there is anything to measure at all.
        //
        // The threshold here is "not empty", not "more than three": a swept cylinder and a loft of
        // two circles have exactly three faces (the side and two caps), and that is CORRECT geometry.
        // How many faces to expect is known to each case, and each case states it itself.
        assert!(!faces.is_empty(), "{title}: body {b} has no faces — the case did not build, nothing to measure");
        let uniq: std::collections::HashSet<u32> = faces.iter().copied().collect();
        if uniq.len() != faces.len() {
            bad.push(format!("body {b}: {} faces, {} distinct names", faces.len(), uniq.len()));
        }
        let pos = faces.iter().filter(|d| !qymcad_core::names::NameTable::is_named(**d)).count();
        if pos > 0 {
            bad.push(format!("body {b}: {pos} unnamed faces out of {}", faces.len()));
        }
        let edges: Vec<u32> = p.regen_edges.get(b).map(|e| e.iter().map(|x| x.id).collect()).unwrap_or_default();
        let euniq: std::collections::HashSet<u32> = edges.iter().copied().collect();
        if euniq.len() != edges.len() {
            bad.push(format!("body {b}: {} edges, {} distinct names", edges.len(), euniq.len()));
        }
    }

    let first = snapshot(p);
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let _ = qymcad_testkit::regenerate(p);
    if snapshot(p) != first {
        bad.push("a rebuild with no edits produced DIFFERENT face names".into());
    }

    assert!(bad.is_empty(), "{title}:\n  {}", bad.join("\n  "));
    // AND THE STRONGEST PART — AN EDIT. Everything above measures a model at rest; the bar is stated
    // about changing the model. Move a point of the first sketch that has one and demand the same as
    // on the familiar documents: the edit adds no failing nodes and wakes no geometric fallback.
    if let Some(si) = (0..p.sketches.len()).find(|i| !p.sketches[*i].points.is_empty()) {
        let sid = p.sketches[si].id;
        let (before, _) = qymcad_testkit::regenerate(p);
        let was: std::collections::HashSet<Id> = before.errors.iter().map(|(n, _)| *n).collect();
        p.sketches[si].points[0].x += 1.5;
        p.solve_sketch(si);
        p.regen_sketch(si);
        p.mark_sketch_dirty(sid);
        p.snap_rebinds.store(0, std::sync::atomic::Ordering::Relaxed);
        let (after, _) = qymcad_testkit::regenerate(p);
        let snaps = p.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed);
        let reds: Vec<String> = after.errors.iter().filter(|(n, _)| !was.contains(n)).map(|(n, e)| format!("node {n}: {e:?}")).collect();
        assert!(reds.is_empty(), "{title}: the sketch edit broke nodes that stood before it:\n  {}", reds.join("\n  "));
        assert_eq!(snaps, 0, "{title}: after the edit {snaps} references resolved BY POSITION instead of by name");
    }

    let total: usize = bodies.iter().map(|b| p.regen_faces.get(b).map(|f| f.len()).unwrap_or(0)).sum();
    eprintln!("{title}: {} live bodies, {total} faces — requirements held, the edit survived", bodies.len());
}

mod shells;
mod threads_and_holes;
mod patterns;
mod edges;
mod shapes;
