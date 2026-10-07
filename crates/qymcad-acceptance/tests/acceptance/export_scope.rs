//! WHAT THE PROGRAM WRITES OUT AND FOR WHOM: one part on its own, the whole assembly, what is hidden and so does
//! not go out, a sketch as a flat drawing, and the quality a mesh is asked for - every written file read back in.
use qymcad::{Key, Modifiers, PointerButton, Session};
use qymcad_acceptance::{build, probe};

/// The path of a file of this check's own.
fn a_path(name: &str, ext: &str) -> String {
    format!("{}/written-{name}.{ext}", std::env::temp_dir().display())
}

/// AN ASSEMBLY OF TWO BLOCKS, each in a part of its own: 40 x 30 x 10 at the origin and 20 x 20 x 10 beside it at
/// x 60. The person is left standing in the assembly.
fn two_blocks() -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    build::into_a_new_part(&mut s);
    let xy = s.word("plane-xy-table");
    s.press_word(&xy);
    let rect = s.word("tb-rect-hint");
    s.press_hint(&rect);
    s.click_on_sketch(60.0, 0.0).click_on_sketch(80.0, 20.0);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    let extrude = s.word("tb-extrude-hint");
    s.press_hint(&extrude);
    s.key(Key::Enter);
    let assembly = s.word("wb-assembly");
    s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
    s
}

/// The row of the tree that reads `name`.
fn row(s: &mut Session, name: &str) -> qymcad::Rect {
    s.find(name, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("{name:?} is not in the tree; on screen: {:?}", s.words()))
}

/// Answer the chooser with `path` and see that something was written there.
fn written_to(s: &mut Session, path: &str, what: &str) {
    // a file left at the same place by an earlier run would pass for the one written now
    let _ = std::fs::remove_file(path);
    s.answer_file(path);
    assert!(std::path::Path::new(path).exists(), "{what} was not written to {path}: the program says {:?}", s.status());
}

/// WRITE OUT WHAT THE TREE ROW `name` STANDS FOR, through its own menu: the right button on the row, the export
/// submenu, the format. The mesh formats ask for a quality first; `quality` is the word to press.
fn export_from_the_tree(s: &mut Session, name: &str, item: &str, quality: &str, path: &str) {
    let at = row(s, name).center();
    s.click_with(at, PointerButton::Secondary, Modifiers::default());
    let export = s.word("act-export");
    s.press_word_near(&export, at);
    s.press_word_near(item, at);
    let quality = s.word(quality);
    if s.shows(&quality) {
        s.press_word(&quality);
    }
    written_to(s, path, name);
}

/// WRITE THE WHOLE PROJECT OUT through the File menu.
fn export_the_project(s: &mut Session, item: &str, path: &str) {
    let (file, export) = (s.word("menu-file"), s.word("file-export"));
    s.menu(&[&file, &export, item]);
    let quality = s.word("stl-standard");
    if s.shows(&quality) {
        s.press_word(&quality);
    }
    written_to(s, path, "the project");
}

/// The bodies of a fresh document the file at `path` was brought into.
fn read_back(path: &str) -> (Session, Vec<qymcad::Solid>) {
    let mut s = Session::start();
    build::into_the_first_part(&mut s);
    build::import(&mut s, path);
    let bodies = s.document().bodies.into_iter().filter(|b| !b.consumed && !b.sheet).collect();
    (s, bodies)
}

/// The name of the part made second - the 20 x 20 block.
fn the_second_part(s: &mut Session) -> String {
    s.document().parts.iter().filter(|p| !p.assembly).map(|p| p.name.clone()).next_back().unwrap_or_else(|| panic!("the assembly holds no part"))
}

probe! {
    /// A PART GOES OUT ON ITS OWN: what its own menu writes holds that part's body and nothing of its neighbour.
    fn a_part_goes_out_on_its_own() {
        let path = a_path("one-part", "step");
        let mut s = two_blocks();
        let second = the_second_part(&mut s);
        export_from_the_tree(&mut s, &second, "STEP\u{2026}", "stl-standard", &path);
        let (mut s, bodies) = read_back(&path);
        assert!(bodies.len() == 1, "one part was written out and {} bodies came back: {:?}; the program says {:?}", bodies.len(), bodies.iter().map(|b| b.volume).collect::<Vec<_>>(), s.status());
        let b = &bodies[0];
        assert!((b.volume - 4000.0).abs() < 1.0, "the 20 x 20 x 10 part came back holding {} instead of 4000", b.volume);
    }
}

probe! {
    /// THE WHOLE ASSEMBLY GOES OUT: both parts, each holding what it held and standing where it stood.
    fn the_whole_assembly_goes_out() {
        let path = a_path("an-assembly", "step");
        let mut s = two_blocks();
        export_the_project(&mut s, "STEP\u{2026}", &path);
        let (mut s, bodies) = read_back(&path);
        assert!(bodies.len() == 2, "an assembly of two parts came back as {} bodies; the program says {:?}", bodies.len(), s.status());
        let mut held: Vec<i64> = bodies.iter().map(|b| b.volume.round() as i64).collect();
        held.sort_unstable();
        assert!(held == vec![4000, 12000], "the two parts came back holding {held:?} instead of 4000 and 12000");
        let beside = bodies.iter().find(|b| b.volume < 8000.0).expect("the smaller of the two");
        assert!((beside.min[0] - 60.0).abs() < 0.01, "the part that stood at x 60 came back at x {}", beside.min[0]);
    }
}

probe! {
    /// WHAT IS HIDDEN DOES NOT GO OUT: the part whose checkbox is cleared is not in the file.
    fn what_is_hidden_does_not_go_out() {
        let path = a_path("half-hidden", "step");
        let mut s = two_blocks();
        let second = the_second_part(&mut s);
        let at = row(&mut s, &second);
        let box_of_the_row = s
            .widgets()
            .into_iter()
            .filter(|w| w.kind == qymcad::Kind::CheckBox && w.rect.center().y > at.min.y && w.rect.center().y < at.max.y && w.rect.max.x <= at.min.x + 1.0)
            .max_by(|a, b| a.rect.max.x.total_cmp(&b.rect.max.x))
            .unwrap_or_else(|| panic!("the row of {second:?} has no checkbox to hide it with; on screen: {:?}", s.words()));
        s.click(box_of_the_row.rect.center());
        assert!(s.document().parts.iter().any(|p| p.name == second && !p.visible), "the part {second:?} was hidden and the document still shows it");
        export_the_project(&mut s, "STEP\u{2026}", &path);
        let (mut s, bodies) = read_back(&path);
        assert!(bodies.len() == 1, "the hidden part went out too: {} bodies came back, {:?}; the program says {:?}", bodies.len(), bodies.iter().map(|b| b.volume).collect::<Vec<_>>(), s.status());
        assert!((bodies[0].volume - 12000.0).abs() < 1.0, "the part left showing came back holding {} instead of 12000", bodies[0].volume);
    }
}

/// WRITE THE SKETCH `name` OUT AS A FLAT DRAWING through its own menu in the tree.
fn export_the_sketch(s: &mut Session, name: &str, item: &str, path: &str) {
    let at = row(s, name).center();
    s.click_with(at, PointerButton::Secondary, Modifiers::default());
    let item = s.word(item);
    s.press_word_near(&item, at);
    written_to(s, path, name);
}

/// A 40 x 30 RECTANGLE DRAWN OUT AS A DRAWING AND READ BACK: four lines, 40 by 30.
fn a_sketch_goes_out_as_a_drawing(item: &str, ext: &str) {
    let path = a_path("a-drawing", ext);
    let mut s = Session::start();
    build::into_the_first_part(&mut s);
    build::rectangle_on_xy(&mut s);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    let sketch = s.document().sketches.last().map(|sk| sk.name.clone()).expect("the sketch of the rectangle");
    export_the_sketch(&mut s, &sketch, item, &path);
    let mut s = Session::start();
    build::into_the_first_part(&mut s);
    build::import(&mut s, &path);
    let table = s.in_space([0.0, 0.0, 0.0]);
    s.click(table);
    let sk = s.document().sketches.last().cloned().unwrap_or_else(|| panic!("the drawing written as {ext} came back as no sketch: the program says {:?}", s.status()));
    assert!(sk.lines == 4, "a rectangle written as {ext} came back as {} lines, {} arcs, {} splines", sk.lines, sk.arcs, sk.splines);
    let size = [sk.max[0] - sk.min[0], sk.max[1] - sk.min[1]];
    assert!((size[0] - 40.0).abs() < 0.01 && (size[1] - 30.0).abs() < 0.01, "a rectangle of 40 by 30 written as {ext} came back {size:?}");
}

probe! {
    /// A SKETCH GOES OUT AS A DXF and comes back the same rectangle.
    fn a_sketch_goes_out_as_a_dxf() {
        a_sketch_goes_out_as_a_drawing("act-export-dxf", "dxf");
    }
}

probe! {
    /// A SKETCH GOES OUT AS AN SVG and comes back the same rectangle.
    fn a_sketch_goes_out_as_an_svg() {
        a_sketch_goes_out_as_a_drawing("act-export-svg", "svg");
    }
}

probe! {
    /// THE QUALITY ASKED FOR IS HELD TO: a shaft written at the finest quality is nearer to the round it stands for
    /// than the same shaft written as a draft. The round is worked out from the shaft that came back - half its
    /// width for the radius, its height for the height - so the tool's own numbers are not guessed at.
    fn the_quality_asked_for_is_held_to() {
        let mut off = Vec::new();
        for (quality, ext) in [("stl-draft", "draft"), ("stl-max", "max")] {
            let path = a_path(ext, "stl");
            let mut s = Session::start();
            build::into_the_first_part(&mut s);
            let cylinder = s.word("tb-cylinder-hint");
            s.press_hint(&cylinder);
            s.key(Key::Enter);
            let part = s.document().parts.iter().filter(|p| !p.assembly).map(|p| p.name.clone()).next_back().expect("the part of the shaft");
            let assembly = s.word("wb-assembly");
            s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
            export_from_the_tree(&mut s, &part, "STL\u{2026}", quality, &path);
            let (mut s, bodies) = read_back(&path);
            let body = bodies.first().unwrap_or_else(|| panic!("the shaft written as {quality} came back as nothing; the program says {:?}", s.status()));
            let (r, h) = ((body.max[0] - body.min[0]) / 2.0, body.max[2] - body.min[2]);
            let round = std::f64::consts::PI * r * r * h;
            off.push(((body.volume - round).abs(), round));
        }
        assert!(off[1].0 < off[0].0, "a shaft written at the finest quality is {} away from the round of {} it stands for, and the draft is {}", off[1].0, off[1].1, off[0].0);
        assert!(off[1].0 / off[1].1 < 0.001, "the finest quality is {} away from a shaft of {}, over a thousandth of it", off[1].0, off[1].1);
    }
}

/// A FOLDER OF THIS CHECK'S OWN under the temporary one, made fresh.
fn a_folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("qym-export-folder").join(name);
    std::fs::create_dir_all(&dir).expect("the folder for the check");
    dir
}

/// PUT UP THE CHOOSER OF A PROJECT EXPORT through the File menu and answer the folder it opens in, leaving it up.
fn the_export_chooser_opens_in(s: &mut Session, item: &str) -> Option<std::path::PathBuf> {
    let (file, export) = (s.word("menu-file"), s.word("file-export"));
    s.menu(&[&file, &export, item]);
    let quality = s.word("stl-standard");
    if s.shows(&quality) {
        s.press_word(&quality);
    }
    s.chooser_folder()
}

probe! {
    /// AN EXPORT OPENS IN THE FOLDER OF THE PROJECT, and once a person has written an export somewhere else, in
    /// that folder - for every format and for a sketch's drawing alike.
    ///
    /// Reported behaviour: on Linux the export chooser opened in the home folder every time.
    fn an_export_opens_where_the_project_lives_then_where_the_last_export_went() {
        let (home, elsewhere) = (a_folder("project"), a_folder("elsewhere"));
        let mut s = Session::start();
        build::block(&mut s);
        build::save_as(&mut s, &home.join("plate.qcad").display().to_string());

        let opened = the_export_chooser_opens_in(&mut s, "STEP\u{2026}");
        assert_eq!(opened.as_deref(), Some(home.as_path()), "the first export of a saved project opened outside its folder");
        written_to(&mut s, &elsewhere.join("plate.step").display().to_string(), "the project");

        let opened = the_export_chooser_opens_in(&mut s, "STL\u{2026}");
        assert_eq!(opened.as_deref(), Some(elsewhere.as_path()), "the next export forgot the folder the last one was written to");
        written_to(&mut s, &elsewhere.join("plate.stl").display().to_string(), "the project");

        let sketch = s.document().sketches.last().map(|sk| sk.name.clone()).expect("the sketch of the block");
        let at = row(&mut s, &sketch).center();
        s.click_with(at, PointerButton::Secondary, Modifiers::default());
        let item = s.word("act-export-dxf");
        s.press_word_near(&item, at);
        assert_eq!(s.chooser_folder().as_deref(), Some(elsewhere.as_path()), "the drawing of a sketch opened outside the folder of the last export");
    }
}

probe! {
    /// ANOTHER PROJECT OPENED, ITS OWN FOLDER: the folder a person wrote the last export of one project to is not
    /// offered for the next project.
    fn another_project_opens_its_exports_in_its_own_folder() {
        let (first, second, elsewhere) = (a_folder("first"), a_folder("second"), a_folder("elsewhere-of-first"));
        let other = second.join("other.qcad").display().to_string();
        let mut s = Session::start();
        build::block(&mut s);
        build::save_as(&mut s, &other);
        let mut s = Session::start();
        build::block(&mut s);
        build::save_as(&mut s, &first.join("first.qcad").display().to_string());
        the_export_chooser_opens_in(&mut s, "STEP\u{2026}");
        written_to(&mut s, &elsewhere.join("first.step").display().to_string(), "the first project");

        build::open_project(&mut s, &other);
        let opened = the_export_chooser_opens_in(&mut s, "STEP\u{2026}");
        assert_eq!(opened.as_deref(), Some(second.as_path()), "the export of the project opened second went to the folder of the first one's export");
    }
}

probe! {
    /// A PROJECT NEVER SAVED HAS NO FOLDER: the chooser is left to open where the system opens it.
    fn a_project_never_saved_leaves_the_folder_to_the_system() {
        let mut s = Session::start();
        build::block(&mut s);
        assert_eq!(the_export_chooser_opens_in(&mut s, "STEP\u{2026}"), None);
    }
}

probe! {
    /// SAVE AS OPENS IN THE FOLDER OF THE PROJECT, not in the folder of its last export.
    fn save_as_opens_where_the_project_lives() {
        let (home, elsewhere) = (a_folder("saved-as"), a_folder("elsewhere-of-saved-as"));
        let mut s = Session::start();
        build::block(&mut s);
        build::save_as(&mut s, &home.join("plate.qcad").display().to_string());
        the_export_chooser_opens_in(&mut s, "STEP\u{2026}");
        written_to(&mut s, &elsewhere.join("plate.step").display().to_string(), "the project");

        let (file, save_as) = (s.word("menu-file"), s.word("file-save-as"));
        s.menu(&[&file, &save_as]);
        assert_eq!(s.chooser_folder().as_deref(), Some(home.as_path()), "Save As of a saved project opened outside its folder");
    }
}
