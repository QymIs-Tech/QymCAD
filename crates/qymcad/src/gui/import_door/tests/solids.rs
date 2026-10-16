//! A mesh made a solid and recognised: holes, sketches, open shells; what a drawing could not read.

use super::*;

/// A MESH BECOMES A SOLID BY ITS TOOL, taking the mesh as it is: the tool from the Part's bar, "as it is" chosen in
/// that bar, a click on the mesh, Enter - the solid of its flat faces, the live body a cut and a drill work on,
/// 1000 mm^3 for the 10 mm cube, as one step of undo, with the mesh consumed.
#[test]
fn a_mesh_becomes_a_solid_by_its_tool() {
    let (mut app, ctx) = running();
    let src = stl_cube(&mut app, &ctx, "solidify.stl");
    let texts = frame(&mut app, &ctx, Vec::new());
    let at = spot(&texts, "solidify").unwrap_or_else(|| panic!("no row of the part: {:?}", names(&texts)));
    crate::gui::a_component_stepped_into_is_not_lit::tests::double_click_at(&mut app, &ctx, at); // into the part
    (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
    let steps = app.disk.edits.undo.len();
    app.start_feat_cmd(35);
    let texts = frame(&mut app, &ctx, Vec::new());
    let asis = spot(&texts, &crate::i18n::tr("cmd-recognise-asis")).unwrap_or_else(|| panic!("the bar offers no polyhedron: {:?}", names(&texts)));
    let _ = frame(&mut app, &ctx, click(asis));
    let basis = app.viewing.cam.basis();
    let on = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([5.0, 5.0, 10.0]).0;
    let _ = frame(&mut app, &ctx, click(on));
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    // as a person waits: the edit only schedules the rebuild, and a check that looked at once saw no solid yet
    crate::gui::a_component_stepped_into_is_not_lit::tests::calm(&mut app, &ctx);
    let _ = frame(&mut app, &ctx, Vec::new());
    let solid = app.project.timeline.iter().find_map(|n| match n.kind {
        qymcad_core::feature::FeatureKind::MeshSolid { src: s, body, .. } if s == src => Some(body),
        _ => None,
    });
    let solid = solid.unwrap_or_else(|| panic!("no solid made of the mesh; status: {}", app.status));
    let v = app.live.shapes.get(&solid).map(|s| s.volume());
    assert!(v.is_some_and(|v| (v - 1000.0).abs() < 1e-6), "the solid holds {v:?} mm^3, not the cube's 1000; errors {:?}", app.project.regen_errors);
    assert!(app.project.consumed_bodies().contains(&src), "the mesh stands beside the solid, a second body of the part");
    assert_eq!(app.disk.edits.undo.len(), steps + 1, "turning into a solid is not one step of undo");
}

/// A MESH TURNED INTO A SOLID, SEEN: the sample cylinder of 88 triangles through the door, its node chosen, "Turn
/// into a solid" pressed - the whole program and the view through the graphics device, into
/// `target/look/mesh-solid.window.png` and `mesh-solid.view.png`: a polyhedron, two flat caps and a faceted wall.
#[test]
#[ignore = "a picture to look at"]
fn a_mesh_turned_into_a_solid_is_looked_at() {
    let sample = format!("{}/../../target/format-samples/cylinder.3mf", env!("CARGO_MANIFEST_DIR"));
    let dir = std::path::PathBuf::from(format!("{}/../../target/look", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the pictures");
    let (mut app, ctx) = running();
    answer(&mut app, &ctx, Want::Anything, &sample);
    settle(&mut app, &ctx);
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter)); // as the file has it, where the window about the scale came up
    let _ = frame(&mut app, &ctx, Vec::new());
    let part = app.project.components.iter().find(|c| c.parent.is_some() && !app.project.component_bodies(c.id).is_empty()).map(|c| c.name.clone()).expect("the cylinder came in");
    let texts = frame(&mut app, &ctx, Vec::new());
    let at = spot(&texts, &qymcad_i18n::name(&part)).unwrap_or_else(|| panic!("no row of the part: {:?}", names(&texts)));
    crate::gui::a_component_stepped_into_is_not_lit::tests::double_click_at(&mut app, &ctx, at); // into the part
    (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
    app.start_feat_cmd(35);
    let texts = frame(&mut app, &ctx, Vec::new());
    let asis = spot(&texts, &crate::i18n::tr("cmd-recognise-asis")).unwrap_or_else(|| panic!("the bar offers no polyhedron: {:?}", names(&texts)));
    let _ = frame(&mut app, &ctx, click(asis));
    // the cylinder stands in the middle of the fitted view; a picture of a body that was never picked would be a lie
    let middle = app.viewing.view_rect.center();
    let _ = frame(&mut app, &ctx, click(middle));
    assert!(app.params.recognise.src.is_some(), "the click in the middle of the view took no mesh: {}", app.status);
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    crate::gui::a_component_stepped_into_is_not_lit::tests::calm(&mut app, &ctx);
    let bg = app.scheme.pal.viewport_bg();
    let pass = |app: &mut App| {
        crate::gui::help_raster::shot_ui([1280, 800], bg, |ui| {
            let ctx = &ui.ctx().clone();
            crate::gui::apply_theme(&mut app.scheme, &app.set, ctx);
            let shell = crate::gui::shell(&app.set);
            for slot in qymcad_shell::Slot::ORDER {
                shell.run_slot(slot, ui, app);
            }
        })
    };
    let _ = pass(&mut app); // the first pass only settles the canvas rectangle
    app.cache.label_tex.borrow_mut().clear();
    let img = pass(&mut app);
    std::fs::write(dir.join("mesh-solid.window.png"), crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
    let view = app.viewing.view_rect;
    if let Some(img) = crate::gui::gpu_shot::eyes::shot(&app.painting(), egui::Rect::from_min_size(egui::pos2(0.0, 0.0), view.size())) {
        std::fs::write(dir.join("mesh-solid.view.png"), crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
    }
    let solid = app.project.timeline.iter().find_map(|n| match n.kind {
        qymcad_core::feature::FeatureKind::MeshSolid { body, .. } => Some(body),
        _ => None,
    });
    eprintln!(
        "MESH SOLID: node {solid:?}, faces {:?}, volume {:?}, status {:?}",
        solid.and_then(|b| app.project.regen_faces.get(&b).map(|f| f.len())),
        solid.and_then(|b| app.live.shapes.get(&b).map(|s| s.volume())),
        app.status
    );
}

#[test]
fn a_mesh_turned_into_a_solid_takes_a_hole() {
    let (app, _ctx) = a_hole_by_hand_in_a_mesh_solid("holed");
    let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the part's body");
    let v = app.live.shapes.get(&body).map(|s| s.volume());
    let want = 1000.0 - std::f64::consts::PI * 9.0 * 4.0;
    let drawn = app.project.sketches.last().map(|s| s.entities.len());
    assert!(
        v.is_some_and(|v| (v - want).abs() < 1e-3),
        "the part holds {v:?} mm^3, not {want:.2} with the hole ({drawn:?} entities drawn); errors {:?}, status {}",
        app.project.regen_errors,
        app.status
    );
}

/// A HOLE IN A MESH SOLID, SEEN: the path above by hand, and the view through the graphics device into
/// `target/look/mesh-solid-hole.view.png` - the cube with a round blind hole in its top face.
#[test]
#[ignore = "a picture to look at"]
fn a_mesh_solid_with_a_hole_is_looked_at() {
    let dir = std::path::PathBuf::from(format!("{}/../../target/look", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the pictures");
    let (app, _ctx) = a_hole_by_hand_in_a_mesh_solid("holed-look");
    let view = app.viewing.view_rect;
    let img = crate::gui::gpu_shot::eyes::shot(&app.painting(), egui::Rect::from_min_size(egui::pos2(0.0, 0.0), view.size())).expect("a graphics device to look through");
    std::fs::write(dir.join("mesh-solid-hole.view.png"), crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
    let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body());
    eprintln!("MESH SOLID HOLE: body {body:?}, volume {:?}, 3D {}", body.and_then(|b| app.live.shapes.get(&b).map(|s| s.volume())), app.viewing.mode_3d);
}

/// A RECOGNISED BODY IS WORKED ON LIKE ANY OTHER: the STL cube recognised from its properties, then a sketch on the
/// face that was found - K and a click on its top - a circle of 3 about the middle, Ctrl+Enter, the sketch's row in
/// the tree, Q, 4 typed into the depth, Enter, Enter. The part holds 1000 - pi 3^2 4 = 886.90 mm^3: the guard the
/// plan asks for, that recognition gives a body a person can build on.
#[test]
fn a_recognised_body_takes_a_sketch_and_a_hole() {
    use crate::gui::a_component_stepped_into_is_not_lit::tests::calm;
    use qymcad_core::feature::{FeatureKind, SketchPlane};
    let (mut app, ctx) = running();
    let src = stl_cube(&mut app, &ctx, "worked.stl");
    let texts = frame(&mut app, &ctx, Vec::new());
    let at = spot(&texts, "worked").unwrap_or_else(|| panic!("no row of the part: {:?}", names(&texts)));
    crate::gui::a_component_stepped_into_is_not_lit::tests::double_click_at(&mut app, &ctx, at); // into the part
    (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
    app.start_feat_cmd(35); // the recognition tool from the Part's bar
    let _ = frame(&mut app, &ctx, Vec::new());
    let basis = app.viewing.cam.basis();
    let on = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([5.0, 5.0, 10.0]).0;
    let _ = frame(&mut app, &ctx, click(on));
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    calm(&mut app, &ctx);
    let body = app.project.timeline.iter().find_map(|n| match n.kind {
        FeatureKind::MeshRecognised { src: s, body, .. } if s == src => Some(body),
        _ => None,
    });
    let body = body.unwrap_or_else(|| panic!("nothing recognised; status: {}", app.status));

    let _ = frame(&mut app, &ctx, key(egui::Key::K));
    assert!(app.tools.picking.is_sketch_plane(), "K asked for no plane: {}", app.status);
    let _ = frame(&mut app, &ctx, Vec::new());
    let basis = app.viewing.cam.basis();
    let top = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([5.0, 5.0, 10.0]).0;
    let _ = frame(&mut app, &ctx, click(top));
    let si = qymcad_ui_state::edit_si(&app.project, &app.sketch_ses).unwrap_or_else(|| panic!("the click on the recognised top opened no sketch: {}", app.status));
    let plane = app.project.sketches[si].plane;
    assert!(matches!(plane, SketchPlane::Face(b, k) if b == body && k.normal[2] > 0.9), "the sketch stands on {plane:?}, not on the recognised top of {body}");

    let _ = frame(&mut app, &ctx, Vec::new());
    let _ = frame(&mut app, &ctx, key(egui::Key::C));
    let fr = qymcad_ui_state::world_frame_of_plane(&app.draw_ctx(), &plane).expect("the frame of the recognised face");
    let mid = fr.project(qymcad_core::geom::Point3::new(5.0, 5.0, 10.0));
    let sheet = qymcad_ui_state::Sheet { view: app.viewing.view, rect: app.viewing.view_rect };
    let _ = frame(&mut app, &ctx, click(sheet.at(mid)));
    let _ = frame(&mut app, &ctx, click(sheet.at(qymcad_core::geom::Point2::new(mid.x + 3.0, mid.y))));
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    let _ = frame(&mut app, &ctx, Vec::new());
    let command = egui::Modifiers::COMMAND;
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)),
        modifiers: command,
        events: vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: command }],
        ..Default::default()
    };
    let _ = ctx.run_ui(raw, |ui| app.draw_frame(ui));
    let _ = frame(&mut app, &ctx, Vec::new());

    let name = crate::i18n::name(&app.project.sketches[si].name);
    let texts = frame(&mut app, &ctx, Vec::new());
    let row = texts.iter().find(|(t, _)| t.ends_with(&name)).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("no row of the sketch {name:?}: {:?}", names(&texts)));
    let _ = frame(&mut app, &ctx, click(row));
    let _ = frame(&mut app, &ctx, key(egui::Key::Q));
    assert!(app.tools.cmd.sketch.is_some(), "Q took no sketch to cut with: {}", app.status);
    let _ = frame(&mut app, &ctx, Vec::new());
    let _ = frame(&mut app, &ctx, vec![egui::Event::Text("4".into())]);
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    calm(&mut app, &ctx);
    let _ = frame(&mut app, &ctx, Vec::new());
    let cut = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the part's body");
    let v = app.live.shapes.get(&cut).map(|s| s.volume());
    let want = 1000.0 - std::f64::consts::PI * 9.0 * 4.0;
    assert!(v.is_some_and(|v| (v - want).abs() < 1e-3), "the recognised body holds {v:?} mm^3, not {want:.2} with the hole; errors {:?}, status {}", app.project.regen_errors, app.status);
}

/// A MESH IS TURNED INTO A BODY BY A TOOL, not by buttons in a panel. Reported behaviour: an STL is imported, the
/// part is stepped into, and its properties offer nothing at all - the mesh cannot be made into an object. The way
/// a person reaches it is the Part's own bar: take the tool, click the mesh in the viewport, Enter.
#[test]
fn a_mesh_is_recognised_by_its_tool() {
    use crate::gui::a_component_stepped_into_is_not_lit::tests::{calm, double_click_at};
    use qymcad_core::feature::FeatureKind;
    let (mut app, ctx) = running();
    let src = stl_cube(&mut app, &ctx, "bytool.stl");
    let texts = frame(&mut app, &ctx, Vec::new());
    let at = spot(&texts, "bytool").unwrap_or_else(|| panic!("no row of the part: {:?}", names(&texts)));
    double_click_at(&mut app, &ctx, at); // into the part, as a person goes in
    (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
    assert!(crate::gui::panels_source::PANELS.contains("BarAsk::FeatCmd(35)"), "without a button in the bar the tool does not exist for a person");
    let steps = app.disk.edits.undo.len();
    app.start_feat_cmd(35);
    let _ = frame(&mut app, &ctx, Vec::new());
    let basis = app.viewing.cam.basis();
    let on = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([5.0, 5.0, 10.0]).0;
    let _ = frame(&mut app, &ctx, click(on));
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    calm(&mut app, &ctx);
    let _ = frame(&mut app, &ctx, Vec::new());
    let body = app.project.timeline.iter().find_map(|n| match n.kind {
        FeatureKind::MeshRecognised { src: s, body, .. } if s == src => Some(body),
        _ => None,
    });
    let body = body.unwrap_or_else(|| panic!("the tool recognised nothing; status: {}", app.status));
    let v = app.live.shapes.get(&body).map(|s| s.volume());
    assert!(v.is_some_and(|v| (v - 1000.0).abs() < 1e-6), "the tool's body holds {v:?} mm^3, not the cube's 1000; errors {:?}", app.project.regen_errors);
    assert_eq!(app.project.regen_faces.get(&body).map(|f| f.len()), Some(6), "the recognised cube has six faces");
    assert!(app.project.consumed_bodies().contains(&src), "the mesh stands beside the body the tool made");
    assert_eq!(app.disk.edits.undo.len(), steps + 1, "the tool is not one step of undo");
    let texts = frame(&mut app, &ctx, Vec::new());
    let named = crate::i18n::tr("feat-mesh-recognised");
    assert!(texts.iter().any(|(t, _)| t.contains(&named)), "the tree does not name what the tool made: {:?}", names(&texts));
}

/// THE TOOL SHOWS WHAT IT FOUND BEFORE ENTER: the mesh clicked, the surfaces on it are counted in the bar - the STL
/// cube, six planes - and nothing is left unrecognised. A person sees what Enter will build before building it.
#[test]
fn the_recognise_tool_counts_what_it_found_before_enter() {
    use crate::gui::a_component_stepped_into_is_not_lit::tests::double_click_at;
    let (mut app, ctx) = running();
    let _src = stl_cube(&mut app, &ctx, "counted.stl");
    let texts = frame(&mut app, &ctx, Vec::new());
    let at = spot(&texts, "counted").unwrap_or_else(|| panic!("no row of the part: {:?}", names(&texts)));
    double_click_at(&mut app, &ctx, at);
    (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
    let steps = app.disk.edits.undo.len();
    app.start_feat_cmd(35);
    let _ = frame(&mut app, &ctx, Vec::new());
    let basis = app.viewing.cam.basis();
    let on = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([5.0, 5.0, 10.0]).0;
    let _ = frame(&mut app, &ctx, click(on));
    let want = crate::i18n::tr1("recognise-planes", "n", "6");
    let mut seen = Vec::new();
    for _ in 0..400 {
        seen = frame(&mut app, &ctx, Vec::new());
        if spot(&seen, &want).is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(spot(&seen, &want).is_some(), "the bar does not count the planes found: {:?}", names(&seen));
    let unfound = crate::i18n::tr1("recognise-none", "n", "0");
    assert!(seen.iter().all(|(t, _)| !t.starts_with(&unfound[..unfound.len() - 1])), "the cube shows unrecognised regions: {:?}", names(&seen));
    assert_eq!(app.disk.edits.undo.len(), steps, "counting before Enter changed the document");
}

/// WHAT THE TOOL FOUND, SEEN before Enter: the cow of the samples through the door, the recognition tool, a click on
/// it and the count waited for - into `target/look/recognise-preview.window.png`: what lies on a surface in the
/// "added" colour, what fits none in the "removed" one.
#[test]
#[ignore = "a picture to look at"]
fn what_the_recognise_tool_found_is_looked_at() {
    use crate::gui::a_component_stepped_into_is_not_lit::tests::double_click_at;
    let sample = format!("{}/../../target/format-samples/cow.obj", env!("CARGO_MANIFEST_DIR"));
    let dir = std::path::PathBuf::from(format!("{}/../../target/look", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the pictures");
    let (mut app, ctx) = running();
    answer(&mut app, &ctx, Want::Anything, &sample);
    settle(&mut app, &ctx);
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
    let texts = frame(&mut app, &ctx, Vec::new());
    let at = spot(&texts, "cow").unwrap_or_else(|| panic!("no row of the part: {:?}", names(&texts)));
    double_click_at(&mut app, &ctx, at); // into the part
    (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
    app.start_feat_cmd(35);
    let _ = frame(&mut app, &ctx, Vec::new());
    // at the middle of the mesh as it stands on the screen
    let mesh = &app.project.bodies.iter().find(|b| !b.mesh.tris.is_empty()).expect("the mesh came in").mesh;
    let n = mesh.verts.len() as f64;
    let mid = mesh.verts.iter().fold([0.0; 3], |a, v| [a[0] + v.x / n, a[1] + v.y / n, a[2] + v.z / n]);
    let basis = app.viewing.cam.basis();
    let on = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at(mid).0;
    let _ = frame(&mut app, &ctx, click(on));
    assert!(
        app.params.recognise.src.is_some(),
        "the click at {on:?} on the mesh about {mid:?} in {:?} took nothing ({} bodies, armed {}, workbench 3D {}): {}",
        app.viewing.view_rect,
        app.project.bodies.len(),
        app.tools.armed.cmd_kind(),
        app.viewing.mode_3d,
        app.status
    );
    for _ in 0..2000 {
        let _ = frame(&mut app, &ctx, Vec::new());
        if app.params.recognise.ready(|_| ()).is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let counts = app.params.recognise.ready(|f| f.counts).expect("the count came in");
    let _ = frame(&mut app, &ctx, Vec::new());
    // THE WHOLE WINDOW: the colouring is drawn over the scene by the window's own painter, which a shot through the
    // graphics device alone does not carry
    let bg = app.scheme.pal.viewport_bg();
    let pass = |app: &mut App| {
        crate::gui::help_raster::shot_ui([1280, 800], bg, |ui| {
            let ctx = &ui.ctx().clone();
            crate::gui::apply_theme(&mut app.scheme, &app.set, ctx);
            let shell = crate::gui::shell(&app.set);
            for slot in qymcad_shell::Slot::ORDER {
                shell.run_slot(slot, ui, app);
            }
        })
    };
    let _ = pass(&mut app);
    app.cache.label_tex.borrow_mut().clear();
    let window = pass(&mut app);
    std::fs::write(dir.join("recognise-preview.window.png"), crate::gui::color_image_to_png(&window).expect("PNG")).expect("writing");
    let view = app.viewing.view_rect;
    let img = crate::gui::gpu_shot::eyes::shot(&app.painting(), egui::Rect::from_min_size(egui::pos2(0.0, 0.0), view.size())).expect("a graphics device to look through");
    std::fs::write(dir.join("recognise-preview.view.png"), crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
    eprintln!("RECOGNISE PREVIEW: counts {counts:?}");
}

/// A RECOGNISED BODY'S NODE REOPENS ITS TOOL on a double click in the tree, with its tolerance and sharp angle in the
/// fields, and Enter changes that node rather than adding another.
#[test]
fn a_recognised_node_reopens_its_tool() {
    use crate::gui::a_component_stepped_into_is_not_lit::tests::{calm, double_click_at};
    use qymcad_core::feature::FeatureKind;
    let (mut app, ctx) = running();
    let _ = stl_cube(&mut app, &ctx, "reopened.stl");
    let texts = frame(&mut app, &ctx, Vec::new());
    let at = spot(&texts, "reopened").unwrap_or_else(|| panic!("no row of the part: {:?}", names(&texts)));
    double_click_at(&mut app, &ctx, at);
    (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
    app.start_feat_cmd(35);
    let _ = frame(&mut app, &ctx, Vec::new());
    let basis = app.viewing.cam.basis();
    let on = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([5.0, 5.0, 10.0]).0;
    let _ = frame(&mut app, &ctx, click(on));
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    calm(&mut app, &ctx);
    let nodes = |app: &App| app.project.timeline.iter().filter(|n| matches!(n.kind, FeatureKind::MeshRecognised { .. })).count();
    assert_eq!(nodes(&app), 1, "no recognised node to reopen");
    (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
    let texts = frame(&mut app, &ctx, Vec::new());
    let named = crate::i18n::tr("feat-mesh-recognised");
    let row = texts.iter().find(|(t, _)| t.contains(&named)).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("no row of the recognised body: {:?}", names(&texts)));
    double_click_at(&mut app, &ctx, row);
    let _ = frame(&mut app, &ctx, Vec::new());
    assert_eq!(app.tools.armed.cmd_kind(), 35, "a double click on the recognised body opened no tool: {}", app.status);
    assert!(app.tools.cmd.edit.is_some(), "the tool did not open on the node, it opened a new one");
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    calm(&mut app, &ctx);
    assert_eq!(nodes(&app), 1, "Enter on the reopened tool added a second node");
}

/// A RECOGNISED BODY THAT DID NOT CLOSE SAYS SO, AND SHOWS WHERE: a box with its top left open, recognised - the tree
/// names it an open shell, and the border of the hole, one loop round the top at z = 10, is what the viewport draws.
#[test]
fn a_recognised_shell_shows_where_it_is_open() {
    let (mut app, ctx) = an_open_box_recognised("openbox");
    let texts = frame(&mut app, &ctx, Vec::new());
    let said = crate::i18n::tr("feat-mesh-recognised-open");
    assert!(texts.iter().any(|(t, _)| t.contains(&said)), "the tree does not say the body did not close: {:?}", names(&texts));
    let borders = app.cache.open_borders.borrow().value.values().flatten().cloned().collect::<Vec<_>>();
    assert_eq!(borders.len(), 1, "one hole, not {} loops", borders.len());
    assert!(borders[0].corners.len() >= 4 && borders[0].corners.iter().all(|q| (q[2] - 10.0).abs() < 1e-6), "the loop does not run round the open top: {:?}", borders[0]);
}

/// AN OPEN SHELL, SEEN: the open box recognised, the whole window into `target/look/open-shell.window.png` - the
/// tree row saying it did not close, and the border of the open top drawn in red.
#[test]
#[ignore = "a picture to look at"]
fn an_open_recognised_shell_is_looked_at() {
    let dir = std::path::PathBuf::from(format!("{}/../../target/look", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the pictures");
    let (mut app, _ctx) = an_open_box_recognised("openbox-look");
    let bg = app.scheme.pal.viewport_bg();
    let pass = |app: &mut App| {
        crate::gui::help_raster::shot_ui([1280, 800], bg, |ui| {
            let ctx = &ui.ctx().clone();
            crate::gui::apply_theme(&mut app.scheme, &app.set, ctx);
            let shell = crate::gui::shell(&app.set);
            for slot in qymcad_shell::Slot::ORDER {
                shell.run_slot(slot, ui, app);
            }
        })
    };
    let _ = pass(&mut app);
    app.cache.label_tex.borrow_mut().clear();
    let window = pass(&mut app);
    std::fs::write(dir.join("open-shell.window.png"), crate::gui::color_image_to_png(&window).expect("PNG")).expect("writing");
}

/// WHAT A DRAWING HOLDS AND WAS NOT READ IS SAID AT THE DOOR: a DXF of a line and a text through "Import" - the status
/// line, which asks where to put the drawing, names the text that did not come in, rather than leaving a person to
/// find it missing.
#[test]
fn a_drawing_says_what_it_did_not_read() {
    let (mut app, ctx) = running();
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join("with-text.dxf");
    std::fs::write(
        &p,
        "0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n11\n10.0\n21\n0.0\n31\n0.0\n0\nTEXT\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n40\n2.5\n1\nnote\n0\nENDSEC\n0\nEOF\n",
    )
    .expect("written");
    answer(&mut app, &ctx, Want::Anything, &p.to_string_lossy());
    let _ = frame(&mut app, &ctx, Vec::new());
    let said = crate::i18n::tr1("import-not-read", "list", "TEXT 1");
    assert!(app.status.words.contains(&said), "the door does not say the text was not read: {:?}", app.status);
}

/// WHAT A DRAWING HOLDS WITH INVALID NUMBERS IS SAID AT THE DOOR: a DXF spline whose knots run backwards, with fit
/// points - it comes in through its fit points, and the status line says so, once: not also as not read.
#[test]
fn a_drawing_says_what_it_held_invalid() {
    let (mut app, ctx) = running();
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join("backwards-spline.dxf");
    let knots: String = [1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0].iter().map(|k| format!("40\n{k:.1}\n")).collect();
    let control: String = [(0.0, 0.0), (10.0, 20.0), (30.0, -20.0), (40.0, 0.0)].iter().map(|(x, y)| format!("10\n{x:.1}\n20\n{y:.1}\n30\n0.0\n")).collect();
    let fit: String = [(0.0, 0.0), (20.0, 0.0), (40.0, 0.0)].iter().map(|(x, y)| format!("11\n{x:.1}\n21\n{y:.1}\n31\n0.0\n")).collect();
    std::fs::write(
        &p,
        format!(
            "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1027\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nSPLINE\n5\n2A\n100\nAcDbEntity\n8\n0\n100\nAcDbSpline\n70\n8\n71\n3\n72\n8\n73\n4\n74\n3\n{knots}{control}{fit}0\nENDSEC\n0\nEOF\n"
        ),
    )
    .expect("written");
    answer(&mut app, &ctx, Want::Anything, &p.to_string_lossy());
    let _ = frame(&mut app, &ctx, Vec::new());
    let waiting = app.tools.pending_import.curves.as_ref().map(|(c, _, _)| c.len()).unwrap_or(0);
    assert!(waiting > 0, "the spline did not come in through its fit points; the status says: {}", app.status);
    let said = crate::i18n::tr1("import-redrawn", "list", "SPLINE 1");
    assert!(app.status.words.contains(&said), "the door does not say the spline was drawn through its fit points: {:?}", app.status);
    for other in ["import-invalid", "import-not-read"] {
        let wrong = crate::i18n::tr1(other, "list", "SPLINE 1");
        assert!(!app.status.words.contains(&wrong), "a spline drawn through its fit points is also named as {other}: {:?}", app.status);
    }
}
