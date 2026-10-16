//! Meshes brought in: where, in what colours, as what parts, in what units and at what size; undo of an import.

use qymcad_ui_state::Notice;
use super::*;

/// A MESH COMES IN WHERE THE FILE PUTS IT, UNDER THE NAME THE FILE GIVES IT.
///
/// Measured on three bodies placed apart in one file: every mesh import was lowered so its
/// top sat at Z=0 - a habit of the machining program this one grew from, where zero is the top of the stock
/// - while STEP and IGES keep the file's coordinates. And the name a 3MF, glTF or AMF gives an object was
///   dropped on the way into the document.
#[test]
fn a_mesh_comes_in_where_the_file_puts_it_under_its_name() {
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join("bracket.3mf");
    let v = |x: f64, y: f64, z: f64| qymcad_core::geom::Point3::new(x, y, z);
    let lifted = qymcad_core::geom::Mesh {
        verts: vec![v(0.0, 0.0, 50.0), v(10.0, 0.0, 50.0), v(0.0, 10.0, 50.0), v(0.0, 0.0, 60.0)],
        tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        ..Default::default()
    };
    qymcad_io::export_3mf(&[lifted], &p.to_string_lossy()).expect("written");
    let (mut app, ctx) = running();
    let before = app.project.bodies.len();
    answer(&mut app, &ctx, Want::Anything, &p.to_string_lossy());
    settle(&mut app, &ctx);
    let b = app.project.bodies.get(before).expect("a body came in");
    let bb = b.mesh.bounds().expect("it has a box");
    assert!((bb.min.z - 50.0).abs() < 1e-9 && (bb.max.z - 60.0).abs() < 1e-9, "the body came in at z {}..{}, not where the file put it (50..60)", bb.min.z, bb.max.z);
    assert_eq!(b.name, "body_1", "the name the file gives the object was dropped");
}

/// A MESH COMES IN IN THE COLOURS ITS FILE GIVES: an OBJ's objects in the `Kd` of their materials.
///
/// Reported behaviour: the print head of the report came in grey from its OBJ, glTF and AMF, where each file colours
/// every part the way its STEP does.
#[test]
fn a_mesh_comes_in_in_the_colours_of_its_file() {
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    std::fs::write(dir.join("coloured.mtl"), "newmtl dark\nKd 0.149 0.149 0.165\nnewmtl red\nKd 0.8 0.1 0.1\n").expect("written");
    let tetra = |at: f64| format!("v {at} 0 0\nv {} 0 0\nv {at} 10 0\nv {at} 0 10\n", at + 10.0);
    let faces = |k: usize| [[1, 3, 2], [1, 2, 4], [1, 4, 3], [2, 3, 4]].iter().map(|f| format!("f {} {} {}\n", f[0] + k, f[1] + k, f[2] + k)).collect::<String>();
    let obj = format!("mtllib coloured.mtl\n{}{}o bolt\nusemtl dark\n{}o plate\nusemtl red\n{}", tetra(0.0), tetra(20.0), faces(0), faces(4));
    let p = dir.join("coloured.obj");
    std::fs::write(&p, obj).expect("written");
    let (mut app, ctx) = running();
    let before = app.project.bodies.len();
    answer(&mut app, &ctx, Want::Anything, &p.to_string_lossy());
    settle(&mut app, &ctx);
    let colours: Vec<[u8; 3]> = (before..app.project.bodies.len()).map(|i| app.project.mesh_color(i)).collect();
    assert_eq!(colours, [[38, 38, 42], [204, 26, 26]], "the colours the file gives its objects were dropped on the way in");
}

/// A MESH LANDS AS PARTS, the way a solid does: a subassembly under the file's name, a part per piece under the
/// piece's own name - not bodies with no part, seen only at the top. One piece is one part, named after its file.
#[test]
fn a_mesh_lands_as_parts_under_its_file() {
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let tetra = |at: f64| format!("v {at} 0 0\nv {} 0 0\nv {at} 10 0\nv {at} 0 10\n", at + 10.0);
    let faces = |k: usize| [[1, 3, 2], [1, 2, 4], [1, 4, 3], [2, 3, 4]].iter().map(|f| format!("f {} {} {}\n", f[0] + k, f[1] + k, f[2] + k)).collect::<String>();
    let pair = dir.join("pair.obj");
    std::fs::write(&pair, format!("{}{}o bolt\n{}o plate\n{}", tetra(0.0), tetra(20.0), faces(0), faces(4))).expect("written");
    let (mut app, ctx) = running();
    let steps = app.disk.edits.undo.len();
    answer(&mut app, &ctx, Want::Anything, &pair.to_string_lossy());
    settle(&mut app, &ctx);
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter)); // the window about the unit, as the file has it
    let _ = frame(&mut app, &ctx, Vec::new());
    let group = app
        .project
        .components
        .iter()
        .find(|c| c.name == "pair" && c.kind == qymcad_core::feature::ComponentKind::Assembly)
        .map(|c| c.id)
        .unwrap_or_else(|| panic!("no subassembly under the file's name; the tree: {:?}", app.project.components.iter().map(|c| &c.name).collect::<Vec<_>>()));
    let parts: Vec<(String, usize)> = app.project.components.iter().filter(|c| c.parent == Some(group)).map(|c| (c.name.clone(), app.project.component_bodies(c.id).len())).collect();
    assert_eq!(parts, [("bolt".to_string(), 1), ("plate".to_string(), 1)], "a piece per part, each holding its body");
    assert_eq!(app.disk.edits.undo.len(), steps + 1, "the import is not one step of undo");
    // AND THE PROGRAM IS AT REST: a mesh piece waits for no B-rep, so nothing is left dirty and nothing is rebuilt
    for _ in 0..5 {
        let _ = frame(&mut app, &ctx, Vec::new());
    }
    assert!(app.project.timeline.iter().all(|n| !n.dirty), "a mesh piece is left waiting for a rebuild");
    assert!(app.regen.busy.is_none(), "a rebuild of nothing is running after a mesh came in: {}", app.status);

    let single = dir.join("lone.stl");
    std::fs::write(&single, cube_stl(10.0)).expect("written");
    let (mut app, ctx) = running();
    answer(&mut app, &ctx, Want::Anything, &single.to_string_lossy());
    settle(&mut app, &ctx);
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    let _ = frame(&mut app, &ctx, Vec::new());
    let lone = app
        .project
        .components
        .iter()
        .find(|c| c.name == "lone")
        .unwrap_or_else(|| panic!("no part under the file's name; the tree: {:?}", app.project.components.iter().map(|c| &c.name).collect::<Vec<_>>()));
    assert_eq!((lone.kind, app.project.component_bodies(lone.id).len()), (qymcad_core::feature::ComponentKind::Part, 1), "one piece is one part");
}

/// A FILE WITHOUT UNITS ASKS WHAT IT IS DRAWN IN, and lands in what the person says.
///
/// Reported behaviour: models from sample files came in so large the zoom could not take them in, or so small
/// they could not be seen. STL, OBJ and PLY carry no unit at all, and their numbers were read as millimetres
/// with no one asked.
#[test]
fn a_file_without_units_asks_what_it_is_drawn_in() {
    let p = scale_file("unit-cube.stl");
    std::fs::write(&p, cube_stl(1.0)).expect("written");
    let (mut app, ctx) = running();
    let (before, steps) = (app.project.bodies.len(), app.disk.edits.undo.len());
    let texts = read_and_look(&mut app, &ctx, &p);
    let metre = spot(&texts, &crate::i18n::tr("import-unit-m")).unwrap_or_else(|| panic!("no window asks for the unit; the screen shows: {:?}", names(&texts)));
    assert!((size_after(&app, before) - 1.0).abs() < 1e-9, "before a unit is chosen the model is not shown as the file has it: {}", size_after(&app, before));
    let _ = frame(&mut app, &ctx, click(metre));
    assert!((size_after(&app, before) - 1000.0).abs() < 1e-6, "a cube of one metre came to {} mm", size_after(&app, before));
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    let texts = frame(&mut app, &ctx, Vec::new());
    assert!(spot(&texts, &crate::i18n::tr("import-scale-import")).is_none(), "the window is still up after Enter");
    assert_eq!(app.disk.edits.undo.len(), steps + 1, "the import is not one step of undo");
    assert!((size_after(&app, before) - 1000.0).abs() < 1e-6, "Enter did not keep the unit chosen: {}", size_after(&app, before));
}

/// ESC LEAVES NOTHING IMPORTED: no body, no step of undo, and the status line says so.
#[test]
fn escape_leaves_nothing_imported() {
    let p = scale_file("unit-cube-esc.stl");
    std::fs::write(&p, cube_stl(1.0)).expect("written");
    let (mut app, ctx) = running();
    let (before, steps) = (app.project.bodies.len(), app.disk.edits.undo.len());
    let texts = read_and_look(&mut app, &ctx, &p);
    assert!(spot(&texts, &crate::i18n::tr("import-scale-import")).is_some(), "no window asks for the unit; the screen shows: {:?}", names(&texts));
    let _ = frame(&mut app, &ctx, key(egui::Key::Escape));
    let _ = frame(&mut app, &ctx, Vec::new());
    assert_eq!(app.project.bodies.len(), before, "Esc left the model in the document");
    assert_eq!(app.disk.edits.undo.len(), steps, "Esc left a step of undo behind");
    assert_eq!(app.status, Notice::info(crate::i18n::tr("import-scale-cancelled")));
}

/// A FILE THAT NAMES ITS UNIT AND COMES IN AT A SENSIBLE SIZE LANDS WITHOUT A QUESTION.
#[test]
fn a_file_with_its_unit_and_a_sensible_size_lands_without_asking() {
    let p = scale_file("ten.3mf");
    let v = |x: f64, y: f64, z: f64| qymcad_core::geom::Point3::new(x, y, z);
    let tet = qymcad_core::geom::Mesh {
        verts: vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(0.0, 10.0, 0.0), v(0.0, 0.0, 10.0)],
        tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        ..Default::default()
    };
    qymcad_io::export_3mf(&[tet], &p).expect("written");
    let (mut app, ctx) = running();
    let (before, steps) = (app.project.bodies.len(), app.disk.edits.undo.len());
    let texts = read_and_look(&mut app, &ctx, &p);
    assert!(spot(&texts, &crate::i18n::tr("import-scale-import")).is_none(), "a 10 mm model in millimetres was asked about");
    assert_eq!(app.disk.edits.undo.len(), steps + 1, "the import is not one step of undo");
    assert!((size_after(&app, before) - 10.0).abs() < 1e-9, "the model came in at {} mm", size_after(&app, before));
}

/// A FILE THAT LIES ABOUT ITS UNIT IS SCALED, AND THE SCALE STAYS WITH THE PART.
///
/// Measured on sample files: an IGES bearing and a hammer both say they are in millimetres, and their surfaces
/// in the file span 0.1 mm and 38.9 m. The unit a file names can be wrong, so an exact file is asked about too
/// when it comes in at no sensible size, and the factor chosen is kept by the import node: reopening the
/// document builds the solid at the same size.
#[test]
fn a_file_that_lies_about_its_unit_is_scaled_and_keeps_its_scale() {
    let p = scale_file("half-mm.step");
    let s = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 0.5, 0.0, 0.5, 0.5, 0.0, 0.5], 0.5).expect("a cube");
    qymcad_kernel::write_step(&[(&s, qymcad_core::feature::PLACE_IDENTITY)], &p).expect("written");
    let (mut app, ctx) = running();
    let before = app.project.bodies.len();
    let texts = read_and_look(&mut app, &ctx, &p);
    let ten = spot(&texts, "10").unwrap_or_else(|| panic!("a half-millimetre solid was not asked about; the screen shows: {:?}", names(&texts)));
    let _ = frame(&mut app, &ctx, click(ten));
    settle(&mut app, &ctx); // the solid is rebuilt at the new size in the background, as after any change
    assert!((size_after(&app, before) - 5.0).abs() < 1e-6, "while the window asks, the solid shows at {} mm, not at the factor chosen", size_after(&app, before));
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    settle(&mut app, &ctx);
    assert!((size_after(&app, before) - 5.0).abs() < 1e-6, "the solid came in at {} mm, not ten times half a millimetre", size_after(&app, before));
    let restored = crate::gui::restore_import_shapes_for(&app.project);
    assert_eq!(restored.len(), 1, "reopened, the document has {} solids", restored.len());
    let b = restored[0].1.bbox().expect("a box");
    // the kernel's box is widened by 1e-6 on every side
    assert!(((b[3] - b[0]).abs() - 5.0).abs() < 1e-4, "reopened, the solid is {} mm, not the 5 mm chosen", (b[3] - b[0]).abs());
}

/// THE VIEW TAKES IN A MODEL OF ANY SIZE, AND THE WHEEL ZOOMS ON FROM WHERE IT FRAMED IT.
///
/// Reported behaviour: some sample files came in so large the zoom could not take them in, and some so small.
/// Measured: the camera framed a 38 m hammer at 0.0087 px/mm while the wheel was held to 0.05 - 400 px/mm, so
/// the first notch threw the view to 0.05 and the hammer out of the frame; and a model under 1 mm was framed as
/// if it were 1 mm, with the wheel stopping at 400 px/mm and a 0.1 mm bearing 48 px across.
#[test]
fn the_view_takes_in_a_model_of_any_size_and_zooms_on_from_there() {
    for (name, side) in [("forty-metres.3mf", 40_000.0), ("tenth-of-a-mm.3mf", 0.1)] {
        let p = tet_3mf(name, side);
        let (mut app, ctx) = running();
        crate::gui::hand::Hand::new(&mut app); // the 3D view, as a person works in it
        let _ = read_and_look(&mut app, &ctx, &p);
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter)); // as the file has it
        let _ = frame(&mut app, &ctx, Vec::new());
        let r = app.viewing.view_rect;
        let across = app.viewing.cam.scale as f64 * side;
        let (short, long) = (r.width().min(r.height()) as f64, r.width().max(r.height()) as f64);
        assert!(across > 0.3 * short && across < long, "{name}: the model is framed {across:.0} px across on a view {short:.0} px high");
        let fit = app.viewing.cam.scale;
        let _ = frame(&mut app, &ctx, vec![egui::Event::PointerMoved(r.center())]); // the hand comes over the view first
        let _ = frame(&mut app, &ctx, wheel(r.center()));
        for _ in 0..10 {
            let _ = frame(&mut app, &ctx, vec![egui::Event::PointerMoved(r.center())]);
            // the wheel is smoothed over frames
        }
        let now = app.viewing.cam.scale;
        assert!(now > fit && now < fit * 2.0, "{name}: one notch of the wheel took the view from {fit} to {now} px/mm");
    }
}

/// A MESH IMPORTED INTO A BLANK DOCUMENT SENDS THE START SCREEN AWAY.
///
/// Measured while checking the zoom: an STL laid into an empty window left the start screen over it, and the
/// screen took the wheel. The screen went only once the timeline had a node, and a mesh comes in as a body with
/// no node - so it stayed in front of the very geometry it promises never to stand in front of.
#[test]
fn a_mesh_imported_into_a_blank_document_sends_the_start_screen_away() {
    let p = scale_file("start-screen.stl");
    std::fs::write(&p, cube_stl(10.0)).expect("written");
    let (mut app, ctx) = running();
    let title = crate::i18n::tr("start-title");
    let texts = frame(&mut app, &ctx, Vec::new());
    assert!(spot(&texts, &title).is_some(), "the blank document shows no start screen, so this check would see nothing: {:?}", names(&texts));
    let _ = read_and_look(&mut app, &ctx, &p);
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    let texts = frame(&mut app, &ctx, Vec::new());
    assert!(spot(&texts, &title).is_none(), "the start screen stands over the model that came in");
}

/// A LONG IMPORT SHOWS HOW LONG IT HAS GONE AND CAN BE LEFT.
///
/// Reported behaviour: a big IGES assembly kept the card up with a spinner and nothing else, for as long as
/// the reading took (it ran past half an hour) - it looked like a hang, and there was no way out.
#[test]
fn a_long_import_shows_its_time_and_can_be_left() {
    let (mut app, ctx) = running();
    let (_tx, rx) = std::sync::mpsc::channel(); // a reading that never answers
    app.regen.busy = Some(qymcad_ui_state::Busy {
        started: std::time::Instant::now(),
        label: crate::i18n::tr1("io-exact-importing", "format", "IGES"),
        rx,
        kind: qymcad_ui_state::BgKind::ImportShapes,
        pulse: None,
        quiet: false,
    });
    let _ = frame(&mut app, &ctx, Vec::new()); // the card settles on the second pass, as every window here does
    let texts = frame(&mut app, &ctx, Vec::new());
    let cancel = crate::i18n::tr("io-cancel-import");
    let at = texts
        .iter()
        .find(|(t, _)| *t == cancel)
        .map(|(_, r)| r.center())
        .unwrap_or_else(|| panic!("the card has no way out; it shows: {:?}", texts.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>()));
    assert!(
        texts.iter().any(|(t, _)| t.len() >= 4 && t.contains(':') && t.chars().all(|c| c.is_ascii_digit() || c == ':')),
        "the card does not say how long it has gone: {:?}",
        texts.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>()
    );
    let _ = frame(&mut app, &ctx, click(at));
    assert!(app.regen.busy.is_none(), "the import was not left");
    assert_eq!(app.status, Notice::info(crate::i18n::tr("in-import-cancelled")));
    let texts = frame(&mut app, &ctx, Vec::new());
    assert!(!texts.iter().any(|(t, _)| *t == cancel), "the card is still up after leaving the import");
}

/// A FILE OF A KIND THE PROGRAM DOES NOT READ gets an answer naming it and the kinds that are read.
#[test]
fn a_file_of_an_unknown_kind_is_answered_by_name() {
    let (mut app, ctx) = running();
    answer(&mut app, &ctx, Want::Anything, "/somewhere/notes.cdw");
    assert!(app.status.words.contains("notes.cdw"), "the answer does not name the file: {}", app.status);
    for f in Format::ALL {
        assert!(app.status.words.contains(f.name()), "the answer does not say that {} can be opened: {}", f.name(), app.status);
    }
    assert!(app.regen.busy.is_none() && app.tools.pending_import.curves.is_none(), "something was read from a file of an unknown kind");
}

/// A DRAWING IS NOT TAKEN INTO AN ASSEMBLY AS A PART, and the answer says where it does go.
#[test]
fn a_drawing_is_not_taken_into_an_assembly() {
    let (mut app, ctx) = running();
    answer(&mut app, &ctx, Want::Part, &example("plate.dxf"));
    assert!(app.tools.pending_import.curves.is_none(), "a flat drawing was taken in through the door of an assembly");
    assert_eq!(app.status, Notice::refusal(crate::i18n::tr1("import-not-a-part", "format", "DXF")), "the refusal does not say where a drawing goes");
}

/// A MESH PART WHOSE REMOVAL IS UNDONE COMES BACK AS IT WAS, AND NOTHING IS REBUILT: a piece of a mesh is its own
/// geometry - the undo step holds its mesh and faces - and there is nothing to rebuild it from. Marked for a
/// rebuild, it started one of nothing, as a mesh piece that came in dirty once did.
#[test]
fn a_mesh_part_brought_back_by_undo_is_not_rebuilt() {
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join("undeleted.stl");
    std::fs::write(&p, cube_stl(10.0)).expect("written");
    let (mut app, ctx) = running();
    answer(&mut app, &ctx, Want::Anything, &p.to_string_lossy());
    settle(&mut app, &ctx);
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter)); // kept at the file's numbers
    let _ = frame(&mut app, &ctx, Vec::new());
    let body = app.project.bodies.first().map(|b| b.id).expect("the cube came in");
    // the part stands selected as it came in: Delete, and Enter for "yes"
    let _ = frame(&mut app, &ctx, key(egui::Key::Delete));
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    let _ = frame(&mut app, &ctx, Vec::new());
    assert!(app.project.bodies.iter().all(|b| b.id != body), "the part was not deleted; status: {}", app.status);
    settle(&mut app, &ctx); // the rebuild after the removal runs out first, as a person waits for it
    undo_by_hand(&mut app, &ctx);
    let tris = app.project.mesh_index(body).map(|i| app.project.bodies[i].mesh.tris.len());
    assert_eq!(tris, Some(12), "the cube did not come back with the undo");
    let dirty: Vec<&str> = app.project.timeline.iter().filter(|n| n.dirty).map(|n| n.name.as_str()).collect();
    assert!(dirty.is_empty(), "the mesh piece brought back is marked for a rebuild: {dirty:?}");
    assert!(app.regen.busy.is_none(), "a rebuild of nothing is running after the undo: {}", app.status);
}
