//! A persistence matrix: save, load the bundle, rebuild in full, and get the same volumes. It catches
//! regressions in serialisation — lost fields of features, ids, sketch planes, references.

use qymcad_core::model::Project;

const PI: f64 = std::f64::consts::PI;

/// A rich scene: a cube with a fillet, a ring-shaped cut on its top face, and a revolve in the same part.
fn build_scene() -> (Project, Vec<u64>) {
    let mut p = Project::default();
    p.new_document();
    // a 20 mm cube
    let si = p.new_sketch("base");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "base");
    p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
    let e = p.add_extrude_multi(sid, vec![cid], 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let cube = p.finish_base_body(e, 1);
    let _ = qymcad_testkit::regenerate(&mut p);
    // a fillet on the vertical edge at (0, 0)
    let eid = p.regen_edges.get(&cube).and_then(|es| es.iter().find(|e| e.a[0].abs() < 1e-6 && e.a[1].abs() < 1e-6 && (e.a[2] - e.b[2]).abs() > 1.0)).map(|e| e.id).expect("the edge");
    let f = p.add_fillet(cube, 3.0, vec![eid]);
    // a Ø8 cut 5 deep, sketched on the world XY: it need not go through from above, being cut from below
    let s2 = p.new_sketch("cut");
    let sid2 = p.sketches[s2].id;
    p.add_sketch_node(sid2, "cut");
    p.add_circle_entity(s2, 12.0, 12.0, 4.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s2);
    let c2 = p.sketches[s2].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
    let cut = p.add_combine_multi_op(f, sid2, vec![c2], 5.0, 0, qymcad_core::feature::Extent::default(), 0.0, vec![]);
    (p, vec![cut])
}

fn volumes(p: &mut Project, bodies: &[u64]) -> Vec<f64> {
    let (report, shapes) = qymcad_testkit::regenerate(p);
    assert!(report.errors.is_empty(), "the rebuild has errors: {:?}", report.errors);
    bodies.iter().map(|b| shapes.get(b).map(|s| s.volume()).unwrap_or(-1.0)).collect()
}

#[test]
fn roundtrip_same_volumes() {
    let (mut p, bodies) = build_scene();
    let v_before = volumes(&mut p, &bodies);
    // a sanity check that the cut really worked: 8000 for the cube, less the fillet, less π·16·5 for the
    // cylinder
    assert!(v_before[0] > 0.0 && v_before[0] < 8000.0 - PI * 16.0 * 5.0 + 1.0, "the scene assembled: V={:?}", v_before);
    let dir = std::env::temp_dir().join("qym_matrix_persist");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scene.qcad");
    let _faces: Vec<Vec<qymcad_core::geom::MeshFace>> = p.bodies.iter().map(|b| &b.mesh).map(|_| Vec::new()).collect();
    qymcad_io::save_project(&p, path.to_str().unwrap()).expect("save");
    let mut loaded = qymcad_io::load_project(path.to_str().unwrap()).expect("load");
    // a full rebuild from scratch, as opening a file does, with everything dirty
    for n in &mut loaded.timeline {
        n.dirty = true;
    }
    let v_after = volumes(&mut loaded, &bodies);
    for (i, (b, a)) in v_before.iter().zip(v_after.iter()).enumerate() {
        assert!(((b - a) / b).abs() < 1e-6, "the volume of body #{i} after the round trip: {a:.3} against {b:.3}");
    }
    // and the file itself reads a second time, the atomic save having not damaged the archive
    let _ = qymcad_io::load_project(path.to_str().unwrap()).expect("reload");
}
