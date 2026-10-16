//! The chooser of files and the menus: every format offered, drawings, missing files, reading in the background.

use super::*;

/// THE CHOOSER LISTS EVERY FORMAT THE TABLE KNOWS, in both cases, together and each on its own.
#[test]
fn the_chooser_lists_every_format_the_table_knows() {
    let filters = import_filters(Want::Anything);
    assert_eq!(filters.len(), 1 + Format::ALL.len(), "one filter for everything plus one per format: {filters:?}");
    let (_, all) = &filters[0];
    for f in Format::ALL {
        for ext in f.extensions() {
            for e in [ext.to_string(), ext.to_ascii_uppercase()] {
                assert!(all.contains(&e), "the chooser's first filter does not show .{e} ({})", f.name());
            }
        }
        assert!(filters.iter().any(|(name, _)| name == f.name()), "{} has no filter of its own", f.name());
    }
}

/// INTO AN ASSEMBLY THE DOOR OFFERS WHAT CAN BECOME A PART - a solid and a mesh, as the help says, and no
/// flat drawing. It used to offer STEP alone, while the tooltip promised STEP or STL.
#[test]
fn into_an_assembly_the_door_offers_what_can_become_a_part() {
    let names: Vec<String> = import_filters(Want::Part).into_iter().skip(1).map(|(n, _)| n).collect();
    assert_eq!(
        names,
        vec!["STEP".to_string(), "IGES".to_string(), "STL".to_string(), "OBJ".to_string(), "PLY".to_string(), "glTF".to_string(), "3MF".to_string(), "AMF".to_string()],
        "the assembly's door offers {names:?}"
    );
}

/// THE FILE MENU HAS ONE DOOR, and the four it replaced are gone.
#[test]
fn the_file_menu_has_one_door_for_every_format() {
    let (mut app, ctx) = running();
    let file = crate::i18n::tr("menu-file");
    let texts = frame(&mut app, &ctx, Vec::new());
    let at = texts.iter().find(|(t, _)| *t == file).map(|(_, r)| r.center()).expect("the menu bar has a File menu");
    let _ = frame(&mut app, &ctx, click(at));
    let texts = frame(&mut app, &ctx, Vec::new());
    let door = crate::i18n::tr("file-import");
    let doors = texts.iter().filter(|(t, _)| *t == door).count();
    assert_eq!(doors, 1, "the File menu shows \"{door}\" {doors} times; painted: {:?}", texts.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>());
    // no format has a line of its own in the File menu: not the old import doors, not the exports
    for f in Format::ALL {
        let stray: Vec<&String> = texts.iter().map(|(t, _)| t).filter(|t| t.ends_with(&format!("{}…", f.name()))).collect();
        assert!(stray.is_empty(), "{} still has a line of its own in the File menu: {stray:?}", f.name());
    }
}

/// THE EXPORT FORMATS LIVE IN ONE SUBMENU: one line in the File menu, and every format behind it.
///
/// Reported behaviour: the File menu listed "Export to ..." once per format, eight lines of them.
#[test]
fn the_export_formats_live_in_one_submenu() {
    let (mut app, ctx) = running();
    let file = crate::i18n::tr("menu-file");
    let texts = frame(&mut app, &ctx, Vec::new());
    let at = texts.iter().find(|(t, _)| *t == file).map(|(_, r)| r.center()).expect("the menu bar has a File menu");
    let _ = frame(&mut app, &ctx, click(at));
    let texts = frame(&mut app, &ctx, Vec::new());
    let export = crate::i18n::tr("file-export");
    let lines: Vec<&(String, egui::Rect)> = texts.iter().filter(|(t, _)| *t == export).collect();
    assert_eq!(lines.len(), 1, "the File menu shows \"{export}\" {} times", lines.len());
    // the submenu opens under the pointer, the way a person opens it
    let spot = lines[0].1.center();
    for _ in 0..4 {
        let _ = frame(&mut app, &ctx, vec![egui::Event::PointerMoved(spot)]);
    }
    let texts = frame(&mut app, &ctx, vec![egui::Event::PointerMoved(spot)]);
    for c in crate::gui::export_menu::choices() {
        let label = crate::i18n::tr1("file-export-as", "format", crate::gui::export_menu::name_of(c));
        assert!(texts.iter().any(|(t, _)| *t == label), "the submenu has no \"{label}\"; painted: {:?}", texts.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>());
    }
}

/// A DRAWING COMES IN AS A SKETCH waiting for the plane it goes on.
#[test]
fn a_drawing_comes_in_as_a_sketch_waiting_for_its_plane() {
    let (mut app, ctx) = running();
    answer(&mut app, &ctx, Want::Anything, &example("plate.dxf"));
    let waiting = app.tools.pending_import.curves.as_ref().map(|(c, _, _)| c.len()).unwrap_or(0);
    assert!(waiting > 0, "the drawing did not come in; the status says: {}", app.status);
}

/// A STEP OR AN IGES THAT IS NOT THERE IS SAID TO BE NOT THERE, by the import a person makes.
///
/// Reported behaviour: importing a STEP path with no file behind it said "STEP: the geometry could not be read or
/// handed over", the words of a broken file, while a missing DXF said "No such file or directory".
#[test]
fn a_missing_step_or_iges_is_said_to_be_not_found() {
    for ext in ["step", "igs"] {
        let path = format!("{}/../../target/import-door/no-such-file.{ext}", env!("CARGO_MANIFEST_DIR"));
        let _ = std::fs::remove_file(&path);
        let (mut app, ctx) = running();
        answer(&mut app, &ctx, Want::Anything, &path);
        settle(&mut app, &ctx);
        let said = crate::i18n::name(&format!("cad-file-not-found#{path}"));
        assert!(app.status.words.contains(&said), "{ext}: a missing file is reported as {:?}, not as {said:?}", app.status);
    }
}

/// AN SVG IS READ IN THE BACKGROUND, as a solid or a mesh is, and comes in as a sketch waiting for its plane. One
/// nested deeper than the reader takes is refused with a message, and the program goes on.
#[test]
fn an_svg_is_read_in_the_background() {
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let plate = dir.join("plate.svg");
    std::fs::write(&plate, r#"<svg xmlns="http://www.w3.org/2000/svg" width="40mm" height="30mm" viewBox="0 0 40 30"><rect x="5" y="5" width="30" height="20"/></svg>"#).expect("written");
    let (mut app, ctx) = running();
    answer(&mut app, &ctx, Want::Anything, &plate.to_string_lossy());
    assert!(app.regen.busy.is_some(), "the SVG was read on the UI thread: nothing runs in the background");
    settle(&mut app, &ctx);
    let waiting = app.tools.pending_import.curves.as_ref().map(|(c, _, _)| c.len()).unwrap_or(0);
    assert!(waiting > 0, "the drawing did not come in; the status says: {}", app.status);

    let deep = dir.join("deep.svg");
    let groups = 5_000;
    std::fs::write(&deep, format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{}<path d=\"M 1 1 L 9 1\"/>{}</svg>", "<g>".repeat(groups), "</g>".repeat(groups))).expect("written");
    let (mut app, ctx) = running();
    answer(&mut app, &ctx, Want::Anything, &deep.to_string_lossy());
    settle(&mut app, &ctx);
    let said = crate::i18n::name(&format!("io-svg-too-deep#{}", qymcad_io::SVG_MAX_DEPTH));
    assert!(app.status.words.contains(&said), "a drawing {groups} levels deep is reported as {:?}, not as {said:?}", app.status);
}

/// SOLIDS AND A MESH ARE READ IN THE BACKGROUND, each by its own reader.
#[test]
fn a_solid_and_a_mesh_are_read_in_the_background() {
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let stl = dir.join("tetra.stl");
    std::fs::write(
        &stl,
        "solid t\nfacet normal 0 0 -1\nouter loop\nvertex 0 0 0\nvertex 0 10 0\nvertex 10 0 0\nendloop\nendfacet\n\
         facet normal 0 -1 0\nouter loop\nvertex 0 0 0\nvertex 10 0 0\nvertex 0 0 10\nendloop\nendfacet\n\
         facet normal -1 0 0\nouter loop\nvertex 0 0 0\nvertex 0 0 10\nvertex 0 10 0\nendloop\nendfacet\n\
         facet normal 1 1 1\nouter loop\nvertex 10 0 0\nvertex 0 10 0\nvertex 0 0 10\nendloop\nendfacet\nendsolid t\n",
    )
    .expect("the mesh is written");
    let igs = dir.join("cube.igs");
    let cube = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("a cube");
    qymcad_kernel::write_iges(&[(&cube, qymcad_core::feature::PLACE_IDENTITY)], &igs.to_string_lossy(), qymcad_kernel::LengthUnit::Millimetre).expect("the IGES is written");
    // the STEP is written here too, like every other format of this check: a file brought from elsewhere is not in
    // every tree the check runs in
    let step = dir.join("cube.step");
    qymcad_kernel::write_step(&[(&cube, qymcad_core::feature::PLACE_IDENTITY)], &step.to_string_lossy()).expect("the STEP is written");
    let obj = dir.join("two-cubes.obj");
    let cube = |at: f64| qymcad_core::geom::Mesh {
        verts: [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0], [0.0, 0.0, 10.0]].iter().map(|p| qymcad_core::geom::Point3::new(p[0] + at, p[1], p[2])).collect(),
        tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        ..Default::default()
    };
    qymcad_io::export_obj(&[cube(0.0), cube(30.0)], &obj.to_string_lossy()).expect("the OBJ is written");
    let ply = dir.join("tetra.ply");
    qymcad_io::export_ply(&[cube(0.0)], &ply.to_string_lossy()).expect("the PLY is written");
    let glb = dir.join("two-cubes.glb");
    qymcad_io::export_glb(&[cube(0.0), cube(30.0)], &glb.to_string_lossy()).expect("the GLB is written");
    let three = dir.join("two-cubes.3mf");
    qymcad_io::export_3mf(&[cube(0.0), cube(30.0)], &three.to_string_lossy()).expect("the 3MF is written");
    let amf = dir.join("two-cubes.amf");
    qymcad_io::export_amf(&[cube(0.0), cube(30.0)], &amf.to_string_lossy()).expect("the AMF is written");
    for (path, says) in [
        (step.to_string_lossy().into_owned(), "STEP"),
        (igs.to_string_lossy().into_owned(), "IGES"),
        (stl.to_string_lossy().into_owned(), "STL"),
        (obj.to_string_lossy().into_owned(), "OBJ"),
        (ply.to_string_lossy().into_owned(), "PLY"),
        (glb.to_string_lossy().into_owned(), "glTF"),
        (three.to_string_lossy().into_owned(), "3MF"),
        (amf.to_string_lossy().into_owned(), "AMF"),
    ] {
        let (mut app, ctx) = running();
        let before = app.project.bodies.len();
        answer(&mut app, &ctx, Want::Anything, &path);
        assert!(app.regen.busy.is_some(), "{path}: nothing started reading; the status says: {}", app.status);
        // AND IT LANDS. Waited for rather than left running: a reader still at work in OCCT when the checks
        // end meets the process tearing its statics down, and that crashed a run that had passed.
        for _ in 0..600 {
            if app.regen.busy.is_none() {
                break;
            }
            frame(&mut app, &ctx, Vec::new());
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(app.regen.busy.is_none(), "{path}: still reading after half a minute");
        assert!(app.project.bodies.len() > before, "{path}: nothing came into the document; the status says: {}", app.status);
        assert!(app.status.words.contains(says), "{path}: the status does not name the format it read: {}", app.status);
    }
}

/// AN IGES WITH NO SURFACES IS A DRAWING, and it comes in as a sketch waiting for its plane.
///
/// Reported behaviour: such a file did not open at all, and the status line showed the kernel's code.
#[test]
fn an_iges_drawing_comes_in_as_a_sketch() {
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join("drawing.igs");
    std::fs::write(&p, iges_line()).expect("written");
    let (mut app, ctx) = running();
    answer(&mut app, &ctx, Want::Anything, &p.to_string_lossy());
    settle(&mut app, &ctx);
    let waiting = app.tools.pending_import.curves.as_ref().map(|(c, _, _)| c.len()).unwrap_or(0);
    assert_eq!(waiting, 1, "the drawing's one line did not come in; the status says: {}", app.status);
}

/// A FILE THE KERNEL CANNOT READ IS ANSWERED IN WORDS, not with the kernel's code.
#[test]
fn a_file_the_kernel_cannot_read_is_answered_in_words() {
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join("broken.stp");
    std::fs::write(&p, "this is not STEP\n").expect("written");
    let (mut app, ctx) = running();
    answer(&mut app, &ctx, Want::Anything, &p.to_string_lossy());
    settle(&mut app, &ctx);
    assert!(!app.status.is_empty() && !app.status.words.contains("cad-"), "the status shows a service code instead of words: {}", app.status);
}

/// THE REPORTED FILE, THROUGH THE DOOR IT WAS OPENED BY. `QYM_IGES` names it.
#[test]
#[ignore = "a file on this machine"]
fn the_reported_iges_opens_through_the_door() {
    let Ok(path) = std::env::var("QYM_IGES") else { return };
    let (mut app, ctx) = running();
    answer(&mut app, &ctx, Want::Anything, &path);
    settle(&mut app, &ctx);
    let waiting = app.tools.pending_import.curves.as_ref().map(|(c, _, _)| c.len()).unwrap_or(0);
    eprintln!("PROBE {path}: {waiting} curves waiting for their plane; the status: {}", app.status);
    assert!(waiting >= 80, "the cell came in as {waiting} curves; the status says: {}", app.status);
    assert!(app.status.words.contains(&crate::i18n::tr("io-iges-definitions-only")), "the status does not say the file only defines: {}", app.status);
}
