//! MATRIX OF CHAINS: editing a sketch UNDER modifiers — fillet/chamfer/shell must survive a rebuild
//! of the base. Without persistent edge ids they fall off on any sketch edit, including edits on the
//! far side of the profile that do not touch the referenced edge at all. Edge references are
//! persistent ids; this checks both the surviving application (by volume) and honest errors.

use qymcad_core::model::Project;

const PI: f64 = std::f64::consts::PI;

fn check(fails: &mut Vec<String>, label: &str, got: f64, exp: f64, tol: f64) {
    if exp <= 0.0 || ((got - exp) / exp).abs() > tol {
        fails.push(format!("{label}: V={got:.1}, expected {exp:.1}"));
    }
}

/// A W x W x H box from a sketch; returns (project, sid, body).
fn cube(w: f64, h: f64) -> (Project, u64, u64) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("s");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    p.add_rect_entity(si, 0.0, 0.0, w, w, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
    let e = p.add_extrude_multi(sid, vec![cid], h, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let body = p.finish_base_body(e, 1);
    (p, sid, body)
}

/// Stretch the sketch rectangle: points at x==from move to x=to (a drag in the sketcher).
fn stretch(p: &mut Project, sid: u64, from: f64, to: f64) {
    let si = p.sketch_index(sid).unwrap();
    for pt in &mut p.sketches[si].points {
        if (pt.x - from).abs() < 1e-9 {
            pt.x = to;
        }
    }
    p.solve_sketch(si);
    p.regen_sketch(si);
    p.mark_sketch_dirty(sid);
}

/// Vertical edge of the box at corner (cx,cy).
fn vert_edge(p: &Project, body: u64, cx: f64, cy: f64) -> Option<u32> {
    p.regen_edges.get(&body).and_then(|es| {
        es.iter()
            .find(|e| (e.a[0] - cx).abs() < 1e-6 && (e.a[1] - cy).abs() < 1e-6 && (e.b[0] - cx).abs() < 1e-6 && (e.b[1] - cy).abs() < 1e-6 && (e.a[2] - e.b[2]).abs() > 1.0)
            .map(|e| e.id)
    })
}

/// A fillet on the edge FAR from the edit survives stretching the base sketch.
#[test]
fn fillet_survives_sketch_stretch() {
    let mut fails: Vec<String> = Vec::new();
    let (mut p, sid, body) = cube(20.0, 20.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    // fillet the edge at corner (0,20) — the edit pulls the OPPOSITE side, x=20
    let Some(eid) = vert_edge(&p, body, 0.0, 20.0) else {
        panic!("edge (0,20) not found");
    };
    let f = p.add_fillet(body, 4.0, vec![eid]);
    let (r1, s1) = qymcad_testkit::regenerate(&mut p);
    for (id, e) in &r1.errors {
        fails.push(format!("before edit: ERROR {id}: {e}"));
    }
    let bite = (16.0 - PI * 4.0) * 20.0; // (r^2 - pi*r^2/4) * h
    let v1 = s1.get(&f).map(|s| s.volume()).unwrap_or(0.0);
    check(&mut fails, "fillet before edit", v1, 8000.0 - bite, 0.01);
    // SKETCH EDIT: box 20x20 -> 30x20 (drag x=20 to 30; the filleted edge at (0,20) is untouched)
    stretch(&mut p, sid, 20.0, 30.0);
    let (r2, s2) = qymcad_testkit::regenerate(&mut p);
    for (id, e) in &r2.errors {
        fails.push(format!("after edit: ERROR {id}: {e}"));
    }
    let v2 = s2.get(&f).map(|s| s.volume()).unwrap_or(0.0);
    check(&mut fails, "fillet after the stretch (same edge)", v2, 12000.0 - bite, 0.01);
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Chamfer and shell in a chain survive an edit of the base; also an edit of the extrude PARAMETER
/// underneath a modifier.
#[test]
fn chamfer_shell_survive_edits() {
    let mut fails: Vec<String> = Vec::new();
    // chamfer
    {
        let (mut p, sid, body) = cube(20.0, 20.0);
        let _ = qymcad_testkit::regenerate(&mut p);
        let Some(eid) = vert_edge(&p, body, 0.0, 20.0) else {
            panic!("edge not found");
        };
        let f = p.add_chamfer(body, 3.0, vec![eid]);
        let _ = qymcad_testkit::regenerate(&mut p);
        stretch(&mut p, sid, 20.0, 26.0);
        let (r, s) = qymcad_testkit::regenerate(&mut p);
        for (id, e) in &r.errors {
            fails.push(format!("chamfer: ERROR {id}: {e}"));
        }
        let v = s.get(&f).map(|x| x.volume()).unwrap_or(0.0);
        check(&mut fails, "chamfer after the stretch", v, 26.0 * 20.0 * 20.0 - 4.5 * 20.0, 0.01);
    }
    // shell: edit the extrude HEIGHT underneath it
    {
        let (mut p, _sid, body) = cube(20.0, 20.0);
        let _ = qymcad_testkit::regenerate(&mut p);
        let top = p.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.9)).map(|f| f.id).expect("top face");
        let sh = p.add_shell(body, 2.0, vec![top], false);
        let _ = qymcad_testkit::regenerate(&mut p);
        // height 20 -> 30
        if let Some(n) = p.timeline.iter_mut().find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Extrude { .. })) {
            if let qymcad_core::feature::FeatureKind::Extrude { height, .. } = &mut n.kind {
                *height = 30.0;
            }
            n.dirty = true;
        }
        let (r, s) = qymcad_testkit::regenerate(&mut p);
        for (id, e) in &r.errors {
            fails.push(format!("shell: ERROR {id}: {e}"));
        }
        let v = s.get(&sh).map(|x| x.volume()).unwrap_or(0.0);
        // box 20x20x30, wall 2, top open: 12000 - 16*16*28
        check(&mut fails, "shell after the height change", v, 12000.0 - 16.0 * 16.0 * 28.0, 0.01);
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// The edge is GONE after the edit (the filleted corner is cut away) -> an honest node error, not
/// silent garbage.
#[test]
fn fillet_on_vanished_edge_errors_honestly() {
    let mut fails: Vec<String> = Vec::new();
    let (mut p, sid, body) = cube(20.0, 20.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    let Some(eid) = vert_edge(&p, body, 20.0, 20.0) else {
        panic!("edge (20,20) not found");
    };
    let f = p.add_fillet(body, 4.0, vec![eid]);
    let _ = qymcad_testkit::regenerate(&mut p);
    // the edit DESTROYS corner (20,20): the rectangle shrinks to x=10, so the edge is now at (10,20)
    stretch(&mut p, sid, 20.0, 10.0);
    let (r, s) = qymcad_testkit::regenerate(&mut p);
    let v = s.get(&f).map(|x| x.volume()).unwrap_or(0.0);
    let errored = r.errors.iter().any(|(id, _)| *id == f);
    // ACCEPTABLE: (a) the edge resolved onto the new corner (10,20) and the fillet applied there,
    // (b) an honest error plus pass-through. NOT ACCEPTABLE: silently no fillet and no error.
    let bite = (16.0 - PI * 4.0) * 20.0;
    let filleted = (v - (4000.0 - bite)).abs() < 40.0;
    let passthrough = (v - 4000.0).abs() < 40.0;
    if !filleted && !(errored && passthrough) {
        fails.push(format!("vanished edge: V={v:.0}, errored={errored} — neither an applied fillet nor an honest error"));
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}
