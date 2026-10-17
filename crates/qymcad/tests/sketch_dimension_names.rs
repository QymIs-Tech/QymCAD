//! Independent dimension names through whole-window frames, in a separate process.
//! Session sets the process-wide paths and producer; they must not affect unit-test fixtures.

use qymcad::{Chooser, Kind, Machine, Session};
use qymcad_core::geom::Point2;
use qymcad_core::model::Constraint;

fn crossed_dimensions_file(directory: &std::path::Path) -> std::path::PathBuf {
    let mut project = qymcad_core::model::Project::default();
    let part = project.new_document();
    project.components.iter_mut().find(|component| component.id == part).expect("the fixture part exists").name = "Housing".into();
    let sketch = project.add_line_sketch("Profile", vec![Point2::new(0.0, 0.0), Point2::new(40.0, 30.0)], false);
    let si = project.sketch_index(sketch).expect("the fixture sketch exists");
    project.add_sketch_node(sketch, "Profile");
    let first = project.sketches[si].points[0].id;
    let second = project.sketches[si].points[1].id;
    project.sketches[si].constraints.push(Constraint::Fixed { p: first });
    project.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a: first, b: second, d: 40.0, off: 12.0, expr: String::new(), driven: false, axis: 1, at: None });
    project.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a: first, b: second, d: 30.0, off: 12.0, expr: String::new(), driven: false, axis: 2, at: None });
    assert!(project.solve_sketch(si) < 1e-8, "the fixture dimensions are consistent");
    let path = directory.join("crossed-dimensions.qcad");
    qymcad_io::save_project(&project, &path.to_string_lossy()).expect("the fixture is saved");
    path
}

fn open_crossed_dimensions(session: &mut Session, path: &std::path::Path) {
    let file = session.word("menu-file");
    let open = session.word("file-open");
    session.menu(&[&file, &open]);
    let discard = session.word("nav-dont-save");
    if let Some(button) = session.find(&discard, egui::Pos2::ZERO) {
        session.click(button.center());
    }
    assert_eq!(session.chooser(), Some(Chooser::Open), "Open must ask for a file");
    session.answer_file(path);
    let part = session.document().sketches.into_iter().find(|sketch| sketch.name == "Profile").and_then(|sketch| sketch.part).expect("the fixture sketch belongs to a part");
    let part_row = session.find(&part, egui::Pos2::ZERO).expect("the opened part appears in the tree");
    session.double_click(part_row.center());
    let row = session.find("Profile", egui::Pos2::ZERO).expect("the opened sketch appears in the tree");
    session.double_click(row.center());
    assert_eq!(session.document().editing.as_deref(), Some("Profile"), "the tree opens the sketch for editing");
}

fn name_dimension_in_frame(session: &mut Session, name: &str) {
    let placeholder = session.word("sk-name-placeholder");
    session.fill_hinted(&placeholder, name).key(egui::Key::Enter).key(egui::Key::Escape);
    assert_eq!(session.document().editing.as_deref(), Some("Profile"), "closing the popup leaves the sketch open");
}

fn read_session_qcad(session: &mut Session, path: &std::path::Path) -> qymcad_core::model::Project {
    session.save_copy(&path.to_string_lossy()).expect("the session document can be inspected");
    qymcad_io::load_project(&path.to_string_lossy()).expect("the saved inspection opens")
}

fn axial_captions_in_frame(session: &mut Session, expected: Point2) {
    let words = session.words();
    let x = format!("d5 = {:.1}", expected.x);
    let y = format!("d6 = {:.1}", expected.y);
    assert!(words.contains(&x) && words.contains(&y), "the frame must show the two independent captions {x:?} and {y:?}: {words:?}");
    let document = session.document();
    let sketch = document.sketches.iter().find(|sketch| sketch.name == "Profile").expect("the sketch remains in the document");
    let corner = Point2::new(sketch.max[0], sketch.max[1]);
    assert!(corner.dist(expected) < 1e-7, "the independently edited point stands at {corner:?}, not {expected:?}");
}

fn edit_axial_drivers(session: &mut Session, expected: Point2) {
    let parameters = session.word("wb-params");
    session.press_word(&parameters);
    session.fill("d5", &expected.x.to_string()).key(egui::Key::Enter);
    session.fill("d6", &expected.y.to_string());
    let x = session.field("d5");
    session.click(x.rect.center()).key(egui::Key::Enter);
    let title = session.word("win-params");
    session.close_window(&title);
    axial_captions_in_frame(session, expected);
}

/// Two projected dimensions of one point pair keep their own captions, name validation and parameter edits
/// through whole-window pointer and keyboard frames, including saving and reopening the document.
#[test]
fn x_and_y_on_the_same_points_keep_independent_names_after_reopening() {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("the clock follows the epoch").as_nanos();
    let directory = std::env::temp_dir().join(format!("qymcad-dimension-popup-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("the fixture directory is created");
    let path = crossed_dimensions_file(&directory);
    let mut machine = Machine::default();
    eframe::set_value(&mut machine.kept, "settings", &qymcad_ui_state::Settings { dim_show_name: true, ..Default::default() });
    let mut session = Session::start_on(machine);
    open_crossed_dimensions(&mut session, &path);

    let baseline = read_session_qcad(&mut session, &directory.join("before-popup.qcad"));
    let canvas = session.canvas();
    let x = session.find("40.0", canvas.center()).expect("the X dimension is drawn");
    session.double_click(x.center());
    let opened = read_session_qcad(&mut session, &directory.join("opened-popup.qcad"));
    assert_eq!(opened.next_id, baseline.next_id, "opening a dimension popup allocated an identity");
    assert_eq!(qymcad_ui_state::doc_key(&opened), qymcad_ui_state::doc_key(&baseline), "opening the popup changed the document");
    let placeholder = session.word("sk-name-placeholder");
    session.fill_hinted(&placeholder, "d5");
    let typed = read_session_qcad(&mut session, &directory.join("typed-name.qcad"));
    assert_eq!(typed.next_id, baseline.next_id, "typing a driver name allocated an identity before commit");
    assert_eq!(qymcad_ui_state::doc_key(&typed), qymcad_ui_state::doc_key(&baseline), "typing a name changed the document before commit");
    name_dimension_in_frame(&mut session, "d5");
    let words = session.words();
    assert!(words.contains(&String::from("d5 = 40.0")), "the named X caption is absent: {words:?}");
    assert!(!words.contains(&String::from("d5 = 30.0")), "the Y dimension inherited the X name: {words:?}");
    let y = session.find("30.0", canvas.center()).expect("the Y dimension is still unnamed");
    session.double_click(y.center());

    let field = session.widgets().into_iter().find(|widget| widget.kind == Kind::TextField && widget.placeholder == placeholder).expect("the driver name field is drawn");
    assert!(field.value.is_empty(), "the Y popup inherited the X name: {:?}", field.value);
    session.fill_hinted(&placeholder, "d5").key(egui::Key::Enter);
    let check = directory.join("refused-name.qcad");
    session.save_copy(&check.to_string_lossy()).expect("the refused edit can be inspected");
    let refused = qymcad_io::load_project(&check.to_string_lossy()).expect("the saved inspection opens");
    assert_eq!(refused.named_dims.len(), 1, "the other dimension's name was accepted");
    assert_eq!(refused.param_map().get("d5"), Some(&40.0), "the refused name displaced the X driver");
    name_dimension_in_frame(&mut session, "d6");
    axial_captions_in_frame(&mut session, Point2::new(40.0, 30.0));
    let image = session.snapshot();
    if let Some(path) = std::env::var_os("QYM_DIMENSION_POPUP_IMAGE") {
        std::fs::write(path, image.png()).expect("the real-frame dimension image is saved");
    }
    edit_axial_drivers(&mut session, Point2::new(57.25, 18.5));

    session.chord(egui::Modifiers::COMMAND, egui::Key::S).settle();
    let saved = qymcad_io::load_project(&path.to_string_lossy()).expect("the actual save opens");
    assert_eq!(saved.named_dims.len(), 2, "saving lost a driver name");
    assert_ne!(saved.named_dims[0].target, saved.named_dims[1].target, "the saved drivers identify the same dimension");
    assert_eq!(saved.param_map().get("d5"), Some(&57.25));
    assert_eq!(saved.param_map().get("d6"), Some(&18.5));
    open_crossed_dimensions(&mut session, &path);
    axial_captions_in_frame(&mut session, Point2::new(57.25, 18.5));
    edit_axial_drivers(&mut session, Point2::new(23.75, 46.125));
    drop(session);
    std::fs::remove_dir_all(directory).expect("the fixture files are removed");
}
