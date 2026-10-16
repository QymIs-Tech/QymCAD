//! An import asked about again from its node, and a raw mesh moved by its node.

use qymcad_ui_state::Notice;
use super::*;

/// CTRL+Z WHILE AN IMPORT ASKS ABOUT ITS SCALE TAKES THAT IMPORT BACK, as Cancel does: the window goes, the mesh
/// asked about goes - moved by the handle while it was asked or not - and the steps before it stay as they were.
/// Reported behaviour: the window stayed, the mesh went, and the person was taken to the part last edited, its last
/// step undone.
#[test]
fn ctrl_z_while_an_import_asks_takes_that_import_back() {
    let (mut app, ctx) = running();
    let first = stl_cube(&mut app, &ctx, "kept-first.stl");
    let steps = app.disk.edits.undo.len();
    let path = app.active_path.clone();
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    let p = dir.join("asked-second.stl");
    std::fs::write(&p, cube_stl(10.0)).expect("written");
    answer(&mut app, &ctx, Want::Anything, &p.to_string_lossy());
    settle(&mut app, &ctx);
    let _ = frame(&mut app, &ctx, Vec::new());
    assert!(app.win.import_scale.is_some(), "setup: the second STL asks about its scale");
    let second = app.project.bodies.iter().map(|b| b.id).find(|b| *b != first).expect("setup: the second cube is laid in while it asks");
    let mi = app.project.mesh_index(second).expect("setup: the second cube's mesh");
    qymcad_part::apply_body_move(&mut app.part_ctx(), mi, [1.0, 0.0, 0.0, 30.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let _ = frame(&mut app, &ctx, Vec::new());
    undo_by_hand(&mut app, &ctx);
    let _ = frame(&mut app, &ctx, Vec::new());
    assert!(app.win.import_scale.is_none(), "Ctrl+Z left the window of the scale open");
    assert!(app.project.mesh_index(second).is_none(), "the second cube is still in the document");
    assert!(app.project.mesh_index(first).is_some(), "Ctrl+Z took away the first cube, a step before the import");
    assert_eq!(app.disk.edits.undo.len(), steps, "Ctrl+Z took a step of the history before the import");
    assert_eq!(app.active_path, path, "Ctrl+Z took the person elsewhere");
}

/// A RAW MESH MOVES AS A SOLID DOES: by a node of the timeline, the move made by the rebuild in the document - its
/// mesh and the faces found on it carried along, nothing found on it again. The move is the one the handle of a body
/// makes when it is let go. Reported behaviour it replaces: the interface shifted the mesh in place and found its
/// faces anew, with no step in the timeline.
#[test]
fn a_raw_mesh_moves_by_a_node_of_the_timeline() {
    let (mut app, ctx) = running();
    let body = stl_cube(&mut app, &ctx, "moved.stl");
    let mi = app.project.mesh_index(body).expect("the cube's mesh");
    let before: Vec<(f64, [f64; 3])> = app.project.bodies[mi].faces.iter().map(|f| (f.area, [f.centroid.x, f.centroid.y, f.centroid.z])).collect();
    assert_eq!(before.len(), 6, "setup: the cube came in with {} faces", before.len());
    qymcad_part::apply_body_move(&mut app.part_ctx(), mi, [1.0, 0.0, 0.0, 50.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let _ = frame(&mut app, &ctx, Vec::new()); // the frame after the handle is let go starts the rebuild
    settle(&mut app, &ctx);
    let moved = app.project.timeline.iter().find_map(|n| match n.kind {
        qymcad_core::feature::FeatureKind::Move { src, body: b, .. } if src == body => Some(b),
        _ => None,
    });
    let moved = moved.unwrap_or_else(|| panic!("no node of the timeline moved the mesh; status: {}", app.status));
    let i = app.project.mesh_index(moved).unwrap_or_else(|| panic!("the moved mesh is no body of the document; errors {:?}, status: {}", app.project.regen_errors, app.status));
    let b = app.project.bodies[i].mesh.bounds().expect("the moved mesh has a box");
    assert!((b.min.x - 50.0).abs() < 1e-6 && (b.max.x - 60.0).abs() < 1e-6, "the moved cube stands from x {} to {}, not 50 to 60", b.min.x, b.max.x);
    let after: Vec<(f64, [f64; 3])> = app.project.bodies[i].faces.iter().map(|f| (f.area, [f.centroid.x, f.centroid.y, f.centroid.z])).collect();
    assert_eq!(after.len(), before.len(), "the moved cube has {} faces, it had {}", after.len(), before.len());
    for ((a0, c0), (a1, c1)) in before.iter().zip(&after) {
        assert!(
            (a0 - a1).abs() < 1e-9 && (c0[0] + 50.0 - c1[0]).abs() < 1e-9 && (c0[1] - c1[1]).abs() < 1e-9 && (c0[2] - c1[2]).abs() < 1e-9,
            "a face of {a0} at {c0:?} came to {a1} at {c1:?}, not 50 along x"
        );
    }
}

/// AN IMPORT IS ASKED ABOUT AGAIN FROM ITS NODE, as any feature is edited: inside its part a double click on its
/// node's row opens the window about units and scale again; "10" and Enter make the cube ten times as large as one
/// step of undo, the node keeps the factor, and Ctrl+Z brings the cube back with nothing rebuilt.
#[test]
fn an_import_is_asked_about_again_from_its_node() {
    let (mut app, ctx) = running();
    let body = stl_cube(&mut app, &ctx, "rescale.stl");
    let steps = app.disk.edits.undo.len();
    ask_again(&mut app, &ctx, "rescale", &crate::i18n::tr1("feat-mesh-piece", "file", "rescale.stl"));
    let texts = frame(&mut app, &ctx, Vec::new());
    assert!(spot(&texts, &crate::i18n::tr("import-scale-apply")).is_some(), "the window asked again offers to import the file once more: {:?}", names(&texts));
    let ten = spot(&texts, "10").unwrap_or_else(|| panic!("no factor 10 in the window: {:?}", names(&texts)));
    let _ = frame(&mut app, &ctx, click(ten));
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    let _ = frame(&mut app, &ctx, Vec::new());
    assert!((side_of(&app, body) - 100.0).abs() < 1e-6, "the cube is {} across, not ten times its 10 mm", side_of(&app, body));
    assert_eq!(app.disk.edits.undo.len(), steps + 1, "asking again is not one step of undo");
    assert_eq!(app.project.import_scale(body), Some(10.0), "the node does not keep the factor");
    settle(&mut app, &ctx); // the rebuild after the answer runs out first, as a person waits for it
    undo_by_hand(&mut app, &ctx);
    assert!((side_of(&app, body) - 10.0).abs() < 1e-6, "Ctrl+Z left the cube {} across", side_of(&app, body));
    assert_eq!(app.project.import_scale(body), Some(1.0), "Ctrl+Z left the node's factor behind");
    assert!(app.project.timeline.iter().all(|n| !n.dirty) && app.regen.busy.is_none(), "Ctrl+Z started a rebuild of the mesh piece: {}", app.status);
}

/// ESC LEAVES AN IMPORT ASKED ABOUT AGAIN AS IT WAS: the cube shown ten times as large while the window asks comes
/// back to its 10 mm, no step of undo is left, nothing is rebuilt, and the person stays in the part they were in.
#[test]
fn esc_leaves_an_import_asked_about_again_as_it_was() {
    let (mut app, ctx) = running();
    let body = stl_cube(&mut app, &ctx, "kept.stl");
    let steps = app.disk.edits.undo.len();
    ask_again(&mut app, &ctx, "kept", &crate::i18n::tr1("feat-mesh-piece", "file", "kept.stl"));
    let inside = qymcad_ui_state::current_ctx_id(&app.active_path, &app.project);
    let texts = frame(&mut app, &ctx, Vec::new());
    let ten = spot(&texts, "10").unwrap_or_else(|| panic!("no factor 10 in the window: {:?}", names(&texts)));
    let _ = frame(&mut app, &ctx, click(ten));
    let _ = frame(&mut app, &ctx, Vec::new());
    assert!((side_of(&app, body) - 100.0).abs() < 1e-6, "while the window asks, the cube shows {} across", side_of(&app, body));
    let _ = frame(&mut app, &ctx, key(egui::Key::Escape));
    let _ = frame(&mut app, &ctx, Vec::new());
    assert!(app.win.import_scale.is_none(), "Esc did not close the window");
    assert!((side_of(&app, body) - 10.0).abs() < 1e-6, "Esc left the cube {} across", side_of(&app, body));
    assert_eq!(app.disk.edits.undo.len(), steps, "Esc left a step of undo behind");
    assert_eq!(app.project.import_scale(body), Some(1.0), "Esc left the node's factor changed");
    assert_eq!(app.status, Notice::info(crate::i18n::tr("import-scale-kept")), "Esc does not say the scale is left as it was");
    assert!(app.project.timeline.iter().all(|n| !n.dirty) && app.regen.busy.is_none(), "Esc started a rebuild of the mesh piece");
    assert_eq!(qymcad_ui_state::current_ctx_id(&app.active_path, &app.project), inside, "Esc on the window also stepped out of the part");
}

/// A SOLID IS ASKED ABOUT AGAIN FROM ITS NODE TOO: a 10 mm cube from STEP, a sensible size, comes in without a
/// question; asked again, "10" shows it ten times as large while the window asks and Esc brings it back, the live
/// solid as well; asked once more, Enter keeps it at 100 mm, and the document builds it at that size on opening.
#[test]
fn a_solid_is_asked_about_again_from_its_node() {
    let p = scale_file("again.step");
    let s = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("a cube");
    qymcad_kernel::write_step(&[(&s, qymcad_core::feature::PLACE_IDENTITY)], &p).expect("written");
    let (mut app, ctx) = running();
    let before = app.project.bodies.len();
    let _ = read_and_look(&mut app, &ctx, &p);
    assert!(app.win.import_scale.is_none(), "a 10 mm solid was asked about as it came in");
    let body = app.project.bodies.get(before).map(|b| b.id).expect("the cube came in");
    // the kernel's box is widened by 1e-6 on every side
    let live = |app: &App| app.live.shapes.get(&body).and_then(|s| s.bbox()).map(|b| b[3] - b[0]).unwrap_or(0.0);
    let steps = app.disk.edits.undo.len();
    let row = crate::i18n::tr1("feat-import", "file", "again.step");
    let ten = |app: &mut App| {
        let texts = frame(app, &ctx, Vec::new());
        let at = spot(&texts, "10").unwrap_or_else(|| panic!("no factor 10 in the window: {:?}", names(&texts)));
        let _ = frame(app, &ctx, click(at));
        settle(app, &ctx); // the solid is rebuilt at the new size, as after any change
    };
    ask_again(&mut app, &ctx, "again", &row);
    ten(&mut app);
    assert!((size_after(&app, before) - 100.0).abs() < 1e-6, "while the window asks, the solid shows at {} mm", size_after(&app, before));
    let _ = frame(&mut app, &ctx, key(egui::Key::Escape));
    settle(&mut app, &ctx);
    assert!((size_after(&app, before) - 10.0).abs() < 1e-6, "Esc left the solid at {} mm", size_after(&app, before));
    assert!((live(&app) - 10.0).abs() < 1e-4, "Esc left the live solid at {} mm", live(&app));
    assert_eq!(app.disk.edits.undo.len(), steps, "Esc left a step of undo behind");
    ask_again(&mut app, &ctx, "again", &row);
    ten(&mut app);
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    settle(&mut app, &ctx);
    assert!((size_after(&app, before) - 100.0).abs() < 1e-6, "Enter left the solid at {} mm", size_after(&app, before));
    assert!((live(&app) - 100.0).abs() < 1e-4, "the live solid is {} mm", live(&app));
    assert_eq!(app.disk.edits.undo.len(), steps + 1, "asking again is not one step of undo");
    assert_eq!(app.project.import_scale(body), Some(10.0), "the node does not keep the factor");
    let restored = crate::gui::restore_import_shapes_for(&app.project);
    let b = restored.iter().find(|(id, _)| *id == body).and_then(|(_, s)| s.bbox()).expect("reopened, the solid is there");
    assert!(((b[3] - b[0]).abs() - 100.0).abs() < 1e-4, "reopened, the solid is {} mm, not the 100 mm chosen", (b[3] - b[0]).abs());
}

/// THE WINDOW ASKED AGAIN FROM ITS NODE, SEEN: the whole program as a person sees it after a double click on the
/// import's node inside its part - the node's row in the tree, and the window at the factor the file stands at with
/// its buttons to apply and to cancel - into `target/import-door/asked-again.png`.
#[test]
#[ignore = "a picture to look at"]
fn the_window_asked_again_is_looked_at() {
    let (mut app, ctx) = running();
    let _ = stl_cube(&mut app, &ctx, "looked-at.stl");
    ask_again(&mut app, &ctx, "looked-at", &crate::i18n::tr1("feat-mesh-piece", "file", "looked-at.stl"));
    let bg = app.scheme.pal.viewport_bg();
    let pass = |app: &mut App| {
        crate::gui::help_raster::shot_ui([1280, 800], bg, |ui| {
            let ctx = &ui.ctx().clone();
            crate::gui::apply_theme(&mut app.scheme, &app.set, ctx);
            let shell = crate::gui::shell(&app.set);
            for slot in qymcad_shell::Slot::ORDER {
                shell.run_slot(slot, ui, app);
            }
            crate::gui::import_scale::import_scale_window(&mut app.win_ctx(&mut Vec::new()), ui.ctx());
        })
    };
    let _ = pass(&mut app); // the first pass only settles the canvas rectangle
    app.cache.label_tex.borrow_mut().clear();
    let img = pass(&mut app);
    let out = std::path::PathBuf::from(format!("{}/../../target/import-door/asked-again.png", env!("CARGO_MANIFEST_DIR")));
    std::fs::write(&out, crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
}

/// THE EDIT BUTTON OF AN IMPORT ASKS AGAIN TOO: the node chosen with one click, its properties show the button every
/// feature has to reopen its command, and an import's command is the window about units and scale.
#[test]
fn the_edit_button_of_an_import_asks_again() {
    let (mut app, ctx) = running();
    let _ = stl_cube(&mut app, &ctx, "props.stl");
    let node = node_row(&mut app, &ctx, "props", &crate::i18n::tr1("feat-mesh-piece", "file", "props.stl"));
    // a pause before each click, as a person makes one: two clicks close together are a double click
    let pause = |app: &mut App| (0..40).for_each(|_| drop(frame(app, &ctx, Vec::new())));
    pause(&mut app);
    let _ = frame(&mut app, &ctx, click(node));
    pause(&mut app);
    let texts = frame(&mut app, &ctx, Vec::new());
    // the button in the properties, on the right: "Edit" is a menu of the window too
    let edit = texts.iter().filter(|(t, _)| *t == crate::i18n::tr("props-edit")).map(|(_, r)| r.center()).max_by(|a, b| a.x.total_cmp(&b.x));
    let edit = edit.unwrap_or_else(|| panic!("no Edit button in the import's properties: {:?}", names(&texts)));
    let _ = frame(&mut app, &ctx, click(edit));
    let _ = frame(&mut app, &ctx, Vec::new());
    assert!(app.win.import_scale.is_some(), "the Edit button of an import's properties does not ask about its scale again; status: {}", app.status);
}
