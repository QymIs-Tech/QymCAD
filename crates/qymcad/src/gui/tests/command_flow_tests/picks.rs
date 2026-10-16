//! What can be picked where: revolve, faces of ghost bodies, a neighbour's face, nested datums; the export plan.

use super::*;

/// A revolve from the button (feat_op is reset at the start) - the full flow.
#[test]
fn revolve_flow() {
    let mut app = App::default();
    let si = {
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 10.0, 0.0, 20.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        si
    };
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 2; // stale from a previous cut - the revolve must reset it to 0
    app.start_feat_cmd(3);
    assert_eq!(app.feat.op, 0, "the revolve starts on Add");
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "angle") {
        p.val = 360.0;
        p.txt = "360".into();
    }
    app.apply_feat_cmd();
    let v = total_volume(&app);
    let exp = std::f64::consts::PI * 300.0 * 30.0;
    assert!((v - exp).abs() / exp < 0.02, "the revolved tube: V={v:.0}, {exp:.0} expected; status: {}", app.status);
}

/// Reported behaviour: only the outer, visible faces one can reach with the mouse should be selectable. A
/// neighbouring part's ghost body (shown under "in context") is visible translucently in the background but
/// must NOT be pickable as a plane or a face for a mirror or a section - it is another part's reference
/// geometry, not one's own.
#[test]
fn ghost_body_face_not_pickable_for_plane_pick() {
    let mut app = App::default();
    let cube_a = build_cube(&mut app); // a block from 0 to 20 in part A (the root)
    let comp_a = app.project.body_owner(cube_a).expect("A's owner");
    let comp_b = app.project.add_part("B");
    app.enter_component(comp_b); // the context is now B (a neighbour of A, neither ancestor nor descendant)
    app.win.context = true;
    // the camera aims EXACTLY at the centre of block A's top face, so Screen::at(target) equals rect.center()
    app.viewing.cam.target = [10.0, 10.0, 20.0];
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let mi = app.project.mesh_index(cube_a).expect("block A's mesh");
    assert!(qymcad_ui_state::body_shown(qymcad_ui_state::body_view_of!(app), mi), "block A is shown in the background (show_context=true)");
    assert!(qymcad_ui_state::body_is_ghost(&app.draw_ctx(), mi), "block A is a ghost seen from context B (a neighbour, not one's own)");
    app.params.mirror.part = Some(comp_a);
    // there is nothing behind the ghost at this point - the pick either misses or (plausibly) falls through it
    // to a base plane; what matters is that it is NOT a face of ghost A
    let hit = crate::gui::pick::pick_sketch_plane_at(&app.painting(), rect, rect.center());
    if let Some(qymcad_core::feature::SketchPlane::Face(body, _)) = hit {
        assert_ne!(body, cube_a, "a GHOST's face (A) must not be pickable in mirror mode: {hit:?}");
    }
}

/// A top-down reference: a sketch placed on a face of a NEIGHBOURING part's body is a LIVE external reference
/// rather than a one-off copy. `resolve_placement_plane` used to substitute a fixed datum plane for the face
/// silently: the coordinates were right, but editing the neighbour no longer drove the part - which is exactly
/// what the "in context" mode exists for.
#[test]
fn sketch_on_neighbour_face_is_live_external_ref() {
    use qymcad_core::feature::{FaceKey, SketchPlane};
    let mut app = App::default();
    let cube = build_cube(&mut app); // part A (a block from 0 to 20)
    let comp_a = app.project.body_owner(cube).expect("A's owner");
    // block A's top face (a +Z normal), taken from the rebuild's data, as the viewport's pick does
    let faces = app.project.regen_faces.get(&cube).cloned().expect("the block's faces");
    let (fi, face) = faces.iter().enumerate().max_by(|a, b| a.1.normal[2].partial_cmp(&b.1.normal[2]).unwrap()).expect("the +Z face");
    let key = FaceKey { index: fi as u32, centroid: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, id: face.id };

    // B is a NEIGHBOUR of A (both sit in the root assembly) rather than nested inside it: moving A must change
    // their relative position, otherwise the test checks nothing
    app.project.set_active_component(Some(app.project.root));
    let part_b = app.project.add_part("B");
    app.enter_component(part_b);
    let si = app.create_sketch_on(SketchPlane::Face(cube, key));
    app.finish_sketch_edit();

    // 1. the plane stayed a LIVE face of the neighbour, and the cross-reference is authorised
    assert!(matches!(app.project.sketches[si].plane, SketchPlane::Face(b, _) if b == cube), "the sketch's plane is the neighbour's face, not a copy: {:?}", app.project.sketches[si].plane);
    let rid = app.project.external_ref_for(part_b, cube).expect("the external reference is registered").id;
    let f0 = app.project.sketch_frame(si).expect("the frame computes (the authorisation is there)");

    // 2. the neighbour moved and the sketch went with it (that is what top-down means)
    let mut mat = qymcad_core::feature::PLACE_IDENTITY;
    mat[3] = 25.0;
    app.project.set_component_transform(comp_a, mat);
    let f1 = app.project.sketch_frame(si).expect("the frame after the neighbour moved");
    assert!((f1.origin[0] - f0.origin[0] - 25.0).abs() < 1e-9, "the sketch did not follow the neighbour: {:?} -> {:?}", f0.origin, f1.origin);

    // 3. breaking the link from the part's properties freezes the geometry in place and leaves the part valid
    assert_eq!(app.project.break_external_ref(rid), 1, "one sketch was frozen");
    assert!(matches!(app.project.sketches[si].plane, SketchPlane::Datum(_)), "after the break it is a datum copy");
    let f2 = app.project.sketch_frame(si).expect("the copy's frame");
    for k in 0..3 {
        assert!((f2.origin[k] - f1.origin[k]).abs() < 1e-9, "the break moved the sketch: {:?} -> {:?}", f1.origin, f2.origin);
    }
}

/// STEP and STL come from ONE parse of the bodies and say the same thing about what was skipped. The scene
/// holds all three cases at once: a live B-rep, an imported mesh (there never was a B-rep) and a body whose
/// rebuild failed (the recipe is there, the B-rep is lost). STEP used to throw the last two away silently while
/// STL just as silently wrote them out as meshes - two files of one project disagreed on their contents.
#[test]
fn export_plan_is_shared_by_step_and_stl() {
    let mut app = App::default();
    let brep = build_cube(&mut app); // part one, with a live B-rep

    // part two with a LOST B-rep (emulating a failed rebuild: the mesh of the last good build remained)
    let part2 = app.project.add_part("Part two");
    app.enter_component(part2);
    let before: std::collections::HashSet<Id> = app.live.shapes.keys().copied().collect();
    let si = sketch_rect(&mut app, 40.0, 0.0, 60.0, 20.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 20.0;
        p.txt = "20".into();
    }
    app.apply_feat_cmd();
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let stale = *app.live.shapes.keys().find(|b| !before.contains(b) && !consumed.contains(b)).expect("part two's body");
    app.live.shapes.remove(&stale); // the B-rep is gone, the node is red and the mesh stayed on screen

    // An STL import: a mesh piece in a part, which never has a B-rep
    let mut mesh = qymcad_core::geom::Mesh::default();
    mesh.verts.extend([Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)]);
    mesh.tris.push([0, 1, 2]);
    let mesh_only = app.project.add_mesh(mesh);
    let source = app.project.add_source("mesh.stl", Vec::new());
    app.project.import_tree_as_parts(vec![qymcad_core::model::ImportNode { name: "mesh".into(), body: Some(mesh_only), mesh: true, ..Default::default() }], source, "mesh");

    let plan = app.export_plan(ExportTarget::Project);
    assert_eq!(plan.brep, vec![brep], "only a live B-rep goes into STEP");
    assert_eq!(plan.mesh_only, vec![mesh_only], "an STL import is a category of its own, not an error");
    assert_eq!(plan.stale, vec![stale], "a body with a recipe but no B-rep is a failed rebuild");
    let stl = plan.stl_bodies();
    assert_eq!(stl.len(), 3, "STL exports EVERYTHING visible: {stl:?}");
    for b in [brep, mesh_only, stale] {
        assert!(stl.contains(&b), "body {b} must land in the STL");
    }

    // both statuses name THE DIFFERENCE rather than keeping quiet about it
    let (step_note, stl_note) = (plan.note(true), plan.note(false));
    for (who, note) in [("STEP", &step_note), ("STL", &stl_note)] {
        assert!(note.contains(&crate::i18n::tr1("note-mesh-bodies", "n", "1")), "{who} must name the mesh body: {note}");
        assert!(note.contains(&crate::i18n::tr1("note-failed-bodies", "n", "1")), "{who} must name the failed rebuild: {note}");
    }
    assert!(step_note.contains(&crate::i18n::tr_prefix("note-not-exported", "what")), "STEP says those bodies are NOT in the file: {step_note}");
    assert!(stl_note.contains(&crate::i18n::tr_prefix("note-as-mesh", "what")), "STL says those bodies came out AS A MESH: {stl_note}");

    // a project with no defective bodies gets no note at all (nothing to alarm anyone about)
    let clean = App::default();
    assert!(clean.export_plan(ExportTarget::Project).note(true).is_empty(), "a clean project carries no note");
}

/// A regression of the previous test. Reported behaviour: face selection inside a Part broke while "in context"
/// was on, which is exactly how a sketch is built off another part. THE SAME scene as in that test, but WITHOUT
/// mirror or section mode - this is an ordinary plane pick (New sketch, or Plane from a face). A ghost's face
/// MUST be pickable: the ghosts are shown precisely for the top-down case where a sketch on a neighbour's face
/// becomes an external reference.
#[test]
fn ghost_body_face_is_pickable_for_plain_sketch_plane_pick() {
    let mut app = App::default();
    let cube_a = build_cube(&mut app); // a block from 0 to 20 in part A (the root)
    let comp_b = app.project.add_part("B");
    app.enter_component(comp_b); // we are inside the neighbouring part B
    app.win.context = true;
    app.viewing.cam.target = [10.0, 10.0, 20.0]; // the centre of block A's top face is under the cursor
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let mi = app.project.mesh_index(cube_a).expect("block A's mesh");
    assert!(qymcad_ui_state::body_is_ghost(&app.draw_ctx(), mi), "block A is a ghost seen from context B");
    assert!(app.params.mirror.part.is_none() && !app.side.section.pick, "an ordinary pick, not a mirror or a section");
    let hit = crate::gui::pick::pick_sketch_plane_at(&app.painting(), rect, rect.center());
    match hit {
        Some(qymcad_core::feature::SketchPlane::Face(body, _)) => {
            assert_eq!(body, cube_a, "the pick must land on ghost A's face rather than on some other body");
        }
        other => panic!("ghost A's face must be pickable for an ordinary sketch, got {other:?}"),
    }
}

/// What was asked for: only the datums of the assembly one is currently in should be drawn, not those from
/// nested assemblies and parts. A datum plane created inside ANOTHER component (not the current context, be it
/// a direct child or a deeper descendant) is NOT pickable and is NOT drawn in mirror or section mode. Only the
/// context's OWN datums are visible - entering the owning Part makes it and its own datum visible.
#[test]
fn nested_datum_not_pickable_outside_owning_context() {
    use qymcad_core::model::{PlaneDef, WorkPlane};
    let mut app = App::default();
    let asm = app.project.add_assembly("Assembly");
    app.project.set_active_component(Some(asm));
    let part = app.project.add_part("Part");
    app.project.set_active_component(Some(part));
    let wp = WorkPlane { name: "Datum".into(), origin: [0.0, 0.0, 5.0], normal: [0.0, 0.0, 1.0], def: PlaneDef::Manual, ..Default::default() };
    let datum_id = app.project.add_plane(wp);
    app.project.active_component = Some(app.project.root);
    app.params.mirror.part = Some(asm); // the pick mode is active
                                        // from THE ROOT: the Part (the datum's owner) is not the current context, so it is hidden
    assert!(qymcad_ui_state::datum_render_transform(&app.painting(), datum_id).is_none(), "another component's datum is not visible from the root");
    // from the Assembly (the Part's direct parent, but NOT the owner itself) it is hidden too
    app.active_path = vec![app.project.root, asm];
    assert!(qymcad_ui_state::datum_render_transform(&app.painting(), datum_id).is_none(), "a DIRECT child's datum is NOT visible - it is not this context's own");
    // entering the Part (the owner itself) makes it visible
    app.active_path = vec![app.project.root, asm, part];
    assert!(qymcad_ui_state::datum_render_transform(&app.painting(), datum_id).is_some(), "the owning context's own datum is visible");
}
