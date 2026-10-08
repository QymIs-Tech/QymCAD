//! ONE DOOR FOR BRINGING A FILE IN.
//!
//! There were four - "Import DXF", "Import STL", "Import STEP", "Import SVG" - each with a chooser showing
//! its own extension and nothing else, and every new format would have added a fifth. Now there is one: the
//! chooser lists everything the table `qymcad_io::Format` knows, and the file's extension decides what it
//! becomes - a solid, a mesh or a sketch. A file of a kind the program does not read gets an answer naming the
//! kinds it does, instead of a reader's error about a format the file was never in.
use std::path::PathBuf;

use qymcad_io::{Format, Lands};
use qymcad_ui_state::Want;

use qymcad_kernel::ExactFormat;

use qymcad_ui_state::MeshFormat;

use super::{open_exact, open_mesh, open_svg, App, Rebuilding};

/// Whether a file that becomes `lands` may come in at a place that wants `want`.
fn takes(want: Want, lands: Lands) -> bool {
    match want {
        Want::Anything => true,
        Want::Part => lands != Lands::Drawing,
    }
}

/// The formats the door offers at a place.
fn formats_for(want: Want) -> Vec<Format> {
    Format::ALL.into_iter().filter(|f| takes(want, f.lands())).collect()
}

/// The chooser's filters: everything together first, then each format on its own.
///
/// Both cases of every extension: the chooser matches case-sensitively on some systems, and a `PART.STP`
/// would then simply not be shown.
pub(crate) fn import_filters(want: Want) -> Vec<(String, Vec<String>)> {
    let formats = formats_for(want);
    let both = |f: &Format| f.extensions().iter().flat_map(|e| [e.to_string(), e.to_ascii_uppercase()]).collect::<Vec<_>>();
    let mut out = vec![(crate::i18n::tr("pk-import-all"), formats.iter().flat_map(both).collect())];
    out.extend(formats.iter().map(|f| (f.name().to_string(), both(f))));
    out
}

/// The chooser, with the filters above.
pub(crate) fn import_dialog(want: Want) -> rfd::AsyncFileDialog {
    import_filters(want).into_iter().fold(rfd::AsyncFileDialog::new(), |d, (name, exts)| d.add_filter(name, &exts))
}

/// What a picked file turned into.
pub(crate) enum Landing {
    /// A solid or a mesh is being read in the background; the job puts it into the document when it is done.
    Started,
    /// The curves of a drawing, waiting for the plane they are laid on.
    /// A drawing read into curves, with the kinds of entity it holds that were not read and how many of each.
    Drawing(Vec<qymcad_core::geom::ProfEdge>, Vec<(String, usize)>),
    /// Nothing came in, and this says why.
    Refused(String),
}

/// Read `path` the way its extension says, for a place that wants `want`.
pub(crate) fn land_import(regen: &mut Rebuilding, path: &str, want: Want) -> Landing {
    let Some(format) = Format::of_path(path) else {
        return Landing::Refused(crate::i18n::tr2("import-unknown", "file", &super::file_name(path), "formats", &Format::names_of(&formats_for(want))));
    };
    if !takes(want, format.lands()) {
        return Landing::Refused(crate::i18n::tr1("import-not-a-part", "format", format.name()));
    }
    let drawing = |read: Result<qymcad_io::ImportedSketch, String>| match read {
        Ok(sketch) => Landing::Drawing(sketch.curves, sketch.skipped),
        Err(why) => Landing::Refused(why),
    };
    // no wildcard: a format added to the table does not compile until it has a reader here
    match format {
        Format::Step => {
            open_exact(regen, path.to_string(), ExactFormat::Step);
            Landing::Started
        }
        Format::Iges => {
            open_exact(regen, path.to_string(), ExactFormat::Iges);
            Landing::Started
        }
        Format::Stl => {
            open_mesh(regen, path.to_string(), MeshFormat::Stl);
            Landing::Started
        }
        Format::Obj => {
            open_mesh(regen, path.to_string(), MeshFormat::Obj);
            Landing::Started
        }
        Format::Ply => {
            open_mesh(regen, path.to_string(), MeshFormat::Ply);
            Landing::Started
        }
        Format::Gltf => {
            open_mesh(regen, path.to_string(), MeshFormat::Glb);
            Landing::Started
        }
        Format::ThreeMf => {
            open_mesh(regen, path.to_string(), MeshFormat::ThreeMf);
            Landing::Started
        }
        Format::Amf => {
            open_mesh(regen, path.to_string(), MeshFormat::Amf);
            Landing::Started
        }
        Format::Dxf => drawing(qymcad_io::import_dxf(path).map_err(|e| crate::i18n::tr1("g-dxf-error", "error", &e.to_string()))),
        Format::Svg => {
            open_svg(regen, path.to_string());
            Landing::Started
        }
    }
}

/// What the door does with the chooser's answer. The checks hand the same function a path of their own.
pub(crate) fn import_answer(want: Want) -> impl FnOnce(&mut App, PathBuf) + 'static {
    move |app, p| {
        let path = p.to_string_lossy().into_owned();
        match land_import(&mut app.regen, &path, want) {
            Landing::Started => {}
            Landing::Drawing(curves, skipped) => {
                app.arm_sketch_import(curves, &path);
                // WHAT DID NOT COME IN IS SAID, beside what the door asks next: a text or a hatch silently lost is found
                // missing only later, by the person, on the drawing
                if !skipped.is_empty() {
                    let list = skipped.iter().map(|(kind, n)| format!("{kind} {n}")).collect::<Vec<_>>().join(", ");
                    app.status = format!("{} · {}", app.status, crate::i18n::tr1("import-not-read", "list", &list));
                }
            }
            Landing::Refused(why) => app.status = why,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{import_answer, import_filters};
    use crate::gui::App;
    use qymcad_io::Format;
    use qymcad_ui_state::Want;

    const SCREEN: egui::Vec2 = egui::vec2(1400.0, 900.0);

    /// One whole frame of the program with `events` in it; returns the painted labels, icons dropped.
    pub(crate) fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<(String, egui::Rect)> {
        fn walk(s: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::epaint::Shape::Text(t) => {
                    let text: String = t.galley.text().chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect();
                    out.push((text.trim().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size())));
                }
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)), events, ..Default::default() };
        let out = ctx.run_ui(raw, |ui| app.draw_frame(ui));
        let mut texts = Vec::new();
        for cs in &out.shapes {
            walk(&cs.shape, &mut texts);
        }
        texts
    }

    pub(crate) fn click(spot: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(spot),
            egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
            egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
        ]
    }

    pub(crate) fn running() -> (App, egui::Context) {
        let mut app = App::default();
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        app.waiting.splash_until = None;
        // AS THE WINDOW STARTS: in 3D at the isometric camera (`App::new`). `App::default` stands in 2D, and a picture
        // of the window taken from it drew every body as a flat silhouette from above, in one colour - which read as
        // "the capture without a graphics device draws a model under 1 mm or over 10 m wrong"
        app.viewing.mode_3d = true;
        app.viewing.cam = qymcad_ui_state::Cam3::default();
        for _ in 0..3 {
            frame(&mut app, &ctx, Vec::new());
        }
        (app, ctx)
    }

    /// The chooser answers with `path`, and the frame picks the answer up - the way the live window does.
    pub(crate) fn answer(app: &mut App, ctx: &egui::Context, want: Want, path: &str) {
        let (tx, rx) = std::sync::mpsc::channel();
        app.arm_file_ask(rx, import_answer(want));
        tx.send(Some(std::path::PathBuf::from(path))).expect("the chooser's channel is open");
        frame(app, ctx, Vec::new());
        assert!(!app.asking_for_a_file(), "the answer was not picked up by the frame");
    }

    fn example(name: &str) -> String {
        format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
    }

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
            assert!(app.status.contains(&said), "{ext}: a missing file is reported as {:?}, not as {said:?}", app.status);
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
        assert!(app.status.contains(&said), "a drawing {groups} levels deep is reported as {:?}, not as {said:?}", app.status);
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
        let cube = qymcad_kernel::Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("a cube");
        qymcad_kernel::write_iges(&[(&cube, qymcad_core::feature::PLACE_IDENTITY)], &igs.to_string_lossy(), qymcad_kernel::LengthUnit::Millimetre).expect("the IGES is written");
        // the STEP is written here too, like every other format of this check: a file brought from elsewhere is not in
        // every tree the check runs in
        let step = dir.join("cube.step");
        qymcad_kernel::write_step(&[(&cube, qymcad_core::feature::PLACE_IDENTITY)], &step.to_string_lossy()).expect("the STEP is written");
        let obj = dir.join("two-cubes.obj");
        let cube = |at: f64| qymcad_core::geom::Mesh {
            verts: [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0], [0.0, 0.0, 10.0]].iter().map(|p| qymcad_core::geom::Point3::new(p[0] + at, p[1], p[2])).collect(),
            tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
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
            assert!(app.status.contains(says), "{path}: the status does not name the format it read: {}", app.status);
        }
    }

    /// Frames until the background read has landed, with a ceiling.
    pub(crate) fn settle(app: &mut App, ctx: &egui::Context) {
        for _ in 0..600 {
            if app.regen.busy.is_none() {
                return;
            }
            frame(app, ctx, Vec::new());
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("still reading after half a minute");
    }

    /// One line in IGES, in the fixed columns IGES is written in: a drawing, not a single surface in it.
    fn iges_line() -> String {
        let g = "1H,,1H;,1Hc,1Hc,1Hc,1Hc,32,38,6,308,15,1Hc,1.,2,2HMM;";
        [
            format!("{:<72}S{:>7}", "a drawing for the check", 1),
            format!("{:<72}G{:>7}", g, 1),
            format!("{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}D{:>7}", 110, 1, 0, 0, 0, 0, 0, 0, "00000000", 1),
            format!("{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}D{:>7}", 110, 0, 0, 1, 0, "", "", "", 0, 2),
            format!("{:<64}{:>8}P{:>7}", "110,0.,0.,0.,10.,0.,0.;", 1, 1),
            format!("S{:>7}G{:>7}D{:>7}P{:>7}{:<40}T{:>7}", 1, 1, 2, 1, "", 1),
        ]
        .join("\n")
            + "\n"
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
        assert!(!app.status.is_empty() && !app.status.contains("cad-"), "the status shows a service code instead of words: {}", app.status);
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
        assert!(app.status.contains(&crate::i18n::tr("io-iges-definitions-only")), "the status does not say the file only defines: {}", app.status);
    }

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
        let lifted = qymcad_core::geom::Mesh { verts: vec![v(0.0, 0.0, 50.0), v(10.0, 0.0, 50.0), v(0.0, 10.0, 50.0), v(0.0, 0.0, 60.0)], tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]] };
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

    /// The folder the scale checks write their files into, under `target`.
    fn scale_file(name: &str) -> String {
        let dir = std::path::PathBuf::from(format!("{}/../../target/import-scale", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).expect("a folder for the check");
        dir.join(name).to_string_lossy().into_owned()
    }

    pub(crate) fn key(k: egui::Key) -> Vec<egui::Event> {
        vec![egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]
    }

    pub(crate) fn spot(texts: &[(String, egui::Rect)], label: &str) -> Option<egui::Pos2> {
        texts.iter().find(|(t, _)| t == label).map(|(_, r)| r.center())
    }

    fn names(texts: &[(String, egui::Rect)]) -> Vec<String> {
        texts.iter().map(|(t, _)| t.clone()).filter(|t| !t.is_empty()).collect()
    }

    /// The largest side of everything that came into the document after the first `before` bodies.
    fn size_after(app: &App, before: usize) -> f64 {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for b in app.project.bodies.iter().skip(before) {
            let Some(bb) = b.mesh.bounds() else { continue };
            for (k, (a, z)) in [(bb.min.x, bb.max.x), (bb.min.y, bb.max.y), (bb.min.z, bb.max.z)].into_iter().enumerate() {
                lo[k] = lo[k].min(a);
                hi[k] = hi[k].max(z);
            }
        }
        (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max)
    }

    /// A cube of `side` as an ASCII STL, the way a program without units writes it.
    pub(crate) fn cube_stl(side: f64) -> String {
        let c = |i: usize| [(i & 1) as f64 * side, ((i >> 1) & 1) as f64 * side, ((i >> 2) & 1) as f64 * side];
        let tris = [[0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4], [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5]];
        let mut out = String::from("solid cube\n");
        for t in tris {
            out.push_str("facet normal 0 0 0\nouter loop\n");
            for i in t {
                let p = c(i);
                out.push_str(&format!("vertex {} {} {}\n", p[0], p[1], p[2]));
            }
            out.push_str("endloop\nendfacet\n");
        }
        out + "endsolid cube\n"
    }

    /// The file is read and the window about its scale is up; returns what the screen shows.
    fn read_and_look(app: &mut App, ctx: &egui::Context, path: &str) -> Vec<(String, egui::Rect)> {
        answer(app, ctx, Want::Anything, path);
        settle(app, ctx);
        let _ = frame(app, ctx, Vec::new()); // a window settles on the second pass
        frame(app, ctx, Vec::new())
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
        assert_eq!(app.status, crate::i18n::tr("import-scale-cancelled"));
    }

    /// A FILE THAT NAMES ITS UNIT AND COMES IN AT A SENSIBLE SIZE LANDS WITHOUT A QUESTION.
    #[test]
    fn a_file_with_its_unit_and_a_sensible_size_lands_without_asking() {
        let p = scale_file("ten.3mf");
        let v = |x: f64, y: f64, z: f64| qymcad_core::geom::Point3::new(x, y, z);
        let tet = qymcad_core::geom::Mesh { verts: vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(0.0, 10.0, 0.0), v(0.0, 0.0, 10.0)], tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]] };
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
        let s = qymcad_kernel::Shape::extrude(&[0.0, 0.0, 0.5, 0.0, 0.5, 0.5, 0.0, 0.5], 0.5).expect("a cube");
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

    fn wheel(at: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, 120.0), phase: egui::TouchPhase::Move, modifiers: Default::default() },
        ]
    }

    /// A tetrahedron with legs of `side`, as a 3MF in millimetres by its own word.
    fn tet_3mf(name: &str, side: f64) -> String {
        let p = scale_file(name);
        let v = |x: f64, y: f64, z: f64| qymcad_core::geom::Point3::new(x * side, y * side, z * side);
        let tet = qymcad_core::geom::Mesh { verts: vec![v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0), v(0.0, 0.0, 1.0)], tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]] };
        qymcad_io::export_3mf(&[tet], &p).expect("written");
        p
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
        assert_eq!(app.status, crate::i18n::tr("in-import-cancelled"));
        let texts = frame(&mut app, &ctx, Vec::new());
        assert!(!texts.iter().any(|(t, _)| *t == cancel), "the card is still up after leaving the import");
    }

    /// A FILE OF A KIND THE PROGRAM DOES NOT READ gets an answer naming it and the kinds that are read.
    #[test]
    fn a_file_of_an_unknown_kind_is_answered_by_name() {
        let (mut app, ctx) = running();
        answer(&mut app, &ctx, Want::Anything, "/somewhere/notes.cdw");
        assert!(app.status.contains("notes.cdw"), "the answer does not name the file: {}", app.status);
        for f in Format::ALL {
            assert!(app.status.contains(f.name()), "the answer does not say that {} can be opened: {}", f.name(), app.status);
        }
        assert!(app.regen.busy.is_none() && app.tools.pending_import.curves.is_none(), "something was read from a file of an unknown kind");
    }

    /// A DRAWING IS NOT TAKEN INTO AN ASSEMBLY AS A PART, and the answer says where it does go.
    #[test]
    fn a_drawing_is_not_taken_into_an_assembly() {
        let (mut app, ctx) = running();
        answer(&mut app, &ctx, Want::Part, &example("plate.dxf"));
        assert!(app.tools.pending_import.curves.is_none(), "a flat drawing was taken in through the door of an assembly");
        assert_eq!(app.status, crate::i18n::tr1("import-not-a-part", "format", "DXF"), "the refusal does not say where a drawing goes");
    }

    /// Ctrl+Z as a hand presses it: the key held with Ctrl in the frame's own state, where the program reads it.
    fn undo_by_hand(app: &mut App, ctx: &egui::Context) {
        let command = egui::Modifiers::COMMAND;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)),
            modifiers: command,
            events: vec![egui::Event::Key { key: egui::Key::Z, physical_key: None, pressed: true, repeat: false, modifiers: command }],
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| app.draw_frame(ui));
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

    /// A cube of 10 mm from STL through the door, kept at the file's numbers; returns its body.
    fn stl_cube(app: &mut App, ctx: &egui::Context, name: &str) -> qymcad_core::model::Id {
        let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).expect("a folder for the check");
        let p = dir.join(name);
        std::fs::write(&p, cube_stl(10.0)).expect("written");
        answer(app, ctx, Want::Anything, &p.to_string_lossy());
        settle(app, ctx);
        let _ = frame(app, ctx, key(egui::Key::Enter)); // kept at the file's numbers
        let _ = frame(app, ctx, Vec::new());
        app.project.bodies.first().map(|b| b.id).expect("the cube came in")
    }

    /// Into the part `part` as a person goes in, unless its rows already show; returns where the row of its import's
    /// node, `row`, stands.
    fn node_row(app: &mut App, ctx: &egui::Context, part: &str, row: &str) -> egui::Pos2 {
        use crate::gui::a_component_stepped_into_is_not_lit::tests::double_click_at;
        let texts = frame(app, ctx, Vec::new());
        if !texts.iter().any(|(t, _)| t.ends_with(row)) {
            let at = spot(&texts, part).unwrap_or_else(|| panic!("no row of the part {part:?}: {:?}", names(&texts)));
            double_click_at(app, ctx, at);
        }
        let texts = frame(app, ctx, Vec::new());
        texts.iter().find(|(t, _)| t.ends_with(row)).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("no row of the import's node {row:?}: {:?}", names(&texts)))
    }

    /// Into the part `part` as a person goes in, unless its rows already show, and a double click on the row of its
    /// import's node, `row`.
    fn ask_again(app: &mut App, ctx: &egui::Context, part: &str, row: &str) {
        use crate::gui::a_component_stepped_into_is_not_lit::tests::double_click_at;
        let node = node_row(app, ctx, part, row);
        double_click_at(app, ctx, node);
        let _ = frame(app, ctx, Vec::new()); // a window settles on the second pass
        assert!(app.win.import_scale.is_some(), "a double click on the import's node does not ask about its scale again; status: {}", app.status);
    }

    /// The side of `body` along x, as it stands in its part.
    fn side_of(app: &App, body: qymcad_core::model::Id) -> f64 {
        app.project.mesh_index(body).and_then(|i| app.project.bodies[i].mesh.bounds()).map(|b| b.max.x - b.min.x).unwrap_or(0.0)
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
        assert_eq!(app.status, crate::i18n::tr("import-scale-kept"), "Esc does not say the scale is left as it was");
        assert!(app.project.timeline.iter().all(|n| !n.dirty) && app.regen.busy.is_none(), "Esc started a rebuild of the mesh piece");
        assert_eq!(qymcad_ui_state::current_ctx_id(&app.active_path, &app.project), inside, "Esc on the window also stepped out of the part");
    }

    /// A SOLID IS ASKED ABOUT AGAIN FROM ITS NODE TOO: a 10 mm cube from STEP, a sensible size, comes in without a
    /// question; asked again, "10" shows it ten times as large while the window asks and Esc brings it back, the live
    /// solid as well; asked once more, Enter keeps it at 100 mm, and the document builds it at that size on opening.
    #[test]
    fn a_solid_is_asked_about_again_from_its_node() {
        let p = scale_file("again.step");
        let s = qymcad_kernel::Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("a cube");
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

    /// A MESH TURNED INTO A SOLID TAKES A HOLE, the way a person makes one, every step a frame: an STL cube of 10
    /// through the door, "Turn into a solid" in its properties, K and a click on the middle of its top face - a sketch
    /// on that face; C and two clicks - a circle of radius 3 about the middle, Enter keeping the radius it offers;
    /// Ctrl+Enter out of the sketch; its row in the tree, Q, 4 typed into the depth field, Enter, Enter. The part holds
    /// 1000 - pi 3^2 4 = 886.90 mm^3: a blind hole 4 deep, not the default 10 (717.26).
    fn a_hole_by_hand_in_a_mesh_solid(stem: &str) -> (App, egui::Context) {
        use crate::gui::a_component_stepped_into_is_not_lit::tests::calm;
        use qymcad_core::feature::{FeatureKind, SketchPlane};
        let (mut app, ctx) = running();
        let file = format!("{stem}.stl");
        let src = stl_cube(&mut app, &ctx, &file);
        let texts = frame(&mut app, &ctx, Vec::new());
        let at = spot(&texts, stem).unwrap_or_else(|| panic!("no row of the part: {:?}", names(&texts)));
        crate::gui::a_component_stepped_into_is_not_lit::tests::double_click_at(&mut app, &ctx, at); // into the part
        (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
        app.start_feat_cmd(35); // the recognition tool from the Part's bar
        let texts = frame(&mut app, &ctx, Vec::new());
        // THE MESH AS IT IS - a polyhedron - is the tool's second choice, taken in its bar
        let asis = spot(&texts, &crate::i18n::tr("cmd-recognise-asis")).unwrap_or_else(|| panic!("the bar offers no polyhedron: {:?}", names(&texts)));
        let _ = frame(&mut app, &ctx, click(asis));
        let basis = app.viewing.cam.basis();
        let on = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([5.0, 5.0, 10.0]).0;
        let _ = frame(&mut app, &ctx, click(on));
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
        calm(&mut app, &ctx);
        let solid = app.project.timeline.iter().find_map(|n| match n.kind {
            FeatureKind::MeshSolid { src: s, body, .. } if s == src => Some(body),
            _ => None,
        });
        let solid = solid.unwrap_or_else(|| panic!("no solid made of the mesh: {}", app.status));

        let _ = frame(&mut app, &ctx, key(egui::Key::K));
        assert!(app.tools.picking.is_sketch_plane(), "K asked for no plane: {}", app.status);
        let _ = frame(&mut app, &ctx, Vec::new());
        let basis = app.viewing.cam.basis();
        let top = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([5.0, 5.0, 10.0]).0;
        let _ = frame(&mut app, &ctx, click(top));
        let si = qymcad_ui_state::edit_si(&app.project, &app.sketch_ses).unwrap_or_else(|| panic!("the click on the top face at {top:?} opened no sketch: {}", app.status));
        let plane = app.project.sketches[si].plane;
        assert!(matches!(plane, SketchPlane::Face(b, k) if b == solid && k.normal[2] > 0.9), "the sketch stands on {plane:?}, not on the top face of the solid {solid}");

        let _ = frame(&mut app, &ctx, Vec::new());
        let _ = frame(&mut app, &ctx, key(egui::Key::C));
        let fr = qymcad_ui_state::world_frame_of_plane(&app.draw_ctx(), &plane).expect("the frame of the top face");
        let mid = fr.project(qymcad_core::geom::Point3::new(5.0, 5.0, 10.0));
        let sheet = qymcad_ui_state::Sheet { view: app.viewing.view, rect: app.viewing.view_rect };
        let _ = frame(&mut app, &ctx, click(sheet.at(mid)));
        let _ = frame(&mut app, &ctx, click(sheet.at(qymcad_core::geom::Point2::new(mid.x + 3.0, mid.y))));
        // the drawn circle offers its radius in a field that holds the keyboard; Enter keeps it, and only then does
        // Ctrl+Enter reach the sketch - from inside the field it types nothing and leaves nothing
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
        let _ = frame(&mut app, &ctx, Vec::new());
        let drawn = app.project.sketches[si].entities.len();
        assert!(!ctx.egui_wants_keyboard_input(), "Enter left the radius field holding the keyboard: {}", app.status);
        let command = egui::Modifiers::COMMAND;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)),
            modifiers: command,
            events: vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: command }],
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| app.draw_frame(ui));
        let _ = frame(&mut app, &ctx, Vec::new());
        assert!(qymcad_ui_state::edit_si(&app.project, &app.sketch_ses).is_none(), "Ctrl+Enter did not leave the sketch of {drawn} entities: {}", app.status);

        let name = crate::i18n::name(&app.project.sketches[si].name);
        let texts = frame(&mut app, &ctx, Vec::new());
        let row = texts.iter().find(|(t, _)| t.ends_with(&name)).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("no row of the sketch {name:?}: {:?}", names(&texts)));
        let _ = frame(&mut app, &ctx, click(row));
        let _ = frame(&mut app, &ctx, key(egui::Key::Q));
        assert!(app.tools.cmd.sketch.is_some(), "Q took no sketch to cut with ({drawn} entities drawn): {}", app.status);
        // the depth is TYPED: the field at the geometry holds its own text and writes it on Enter, so a number put
        // straight into the command's parameters was overwritten by the field's "10" and a check of it saw nothing
        let _ = frame(&mut app, &ctx, Vec::new());
        assert!(ctx.egui_wants_keyboard_input(), "the depth field at the geometry does not take the keyboard: {}", app.status);
        let _ = frame(&mut app, &ctx, vec![egui::Event::Text("4".into())]);
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter)); // keeps the typed depth and lets the keyboard go
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter)); // applies the cut
        calm(&mut app, &ctx);
        let _ = frame(&mut app, &ctx, Vec::new());
        (app, ctx)
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

    /// A box with its top left open - the two triangles of the top, whose corners all stand at z = 10, left out -
    /// through the door, into its part, recognised by the tool.
    fn an_open_box_recognised(stem: &str) -> (App, egui::Context) {
        use crate::gui::a_component_stepped_into_is_not_lit::tests::{calm, double_click_at};
        let (mut app, ctx) = running();
        let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).expect("a folder for the check");
        let p = dir.join(format!("{stem}.stl"));
        let closed = cube_stl(10.0);
        // the two triangles of the top (corners 4 to 7 all stand at z = 10) are left out
        let chunks: Vec<&str> = closed.split("facet normal").collect();
        let open: String = chunks.iter().enumerate().filter(|(i, _)| *i != 3 && *i != 4).map(|(i, c)| if i == 0 { c.to_string() } else { format!("facet normal{c}") }).collect();
        std::fs::write(&p, open).expect("written");
        answer(&mut app, &ctx, Want::Anything, &p.to_string_lossy());
        settle(&mut app, &ctx);
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
        let texts = frame(&mut app, &ctx, Vec::new());
        let at = spot(&texts, stem).unwrap_or_else(|| panic!("no row of the part: {:?}", names(&texts)));
        double_click_at(&mut app, &ctx, at);
        (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
        app.start_feat_cmd(35);
        let _ = frame(&mut app, &ctx, Vec::new());
        let basis = app.viewing.cam.basis();
        let on = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([5.0, 0.0, 5.0]).0;
        let _ = frame(&mut app, &ctx, click(on));
        assert!(app.params.recognise.src.is_some(), "the click took no mesh: {}", app.status);
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
        calm(&mut app, &ctx);
        (app, ctx)
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
        assert!(app.status.contains(&said), "the door does not say the text was not read: {:?}", app.status);
    }
}
