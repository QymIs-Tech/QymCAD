//! ACCEPTANCE CHECKS: every tool and function of the program, used the way a person uses it.
//!
//! A check here has the program's session and nothing else - the mouse, the keyboard, the answer to a file
//! chooser, and what the program shows. What lives in this library is what many checks share, written against
//! the session too.
#[cfg(test)]
mod the_door;
pub mod bodies;
pub mod chains;
pub mod completeness;
pub mod contract;
pub mod golden;
pub mod iso;
pub mod isolation;
pub mod matrix;
pub mod mouse;
pub mod oracles;
pub mod tools;

/// RUN `f` AND ANSWER THE MESSAGE IT FAILED WITH; empty when it did not fail. For the checks of a refusal: a
/// session that fails says why in its message.
pub fn refusal(f: impl FnOnce()) -> String {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(hook);
    match out {
        Ok(()) => String::new(),
        Err(e) => e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default(),
    }
}

/// A FILE OF THE PRIVATE SAMPLES (`samples/`), when this tree has it. The samples are not published; a check on one
/// of them in a public clone has nothing to open, and a red there would say the program is broken when only the file
/// is missing. So such a check is passed over, and says so on its output.
pub fn private_sample(name: &str) -> Option<String> {
    let path = format!("{}/../../samples/{name}", env!("CARGO_MANIFEST_DIR"));
    if std::path::Path::new(&path).exists() {
        Some(path)
    } else {
        eprintln!("PASSED OVER: the private sample {name} is not in this tree - the check on it runs only where the samples are");
        None
    }
}

/// DOES THE SKETCH SAY IT HOLDS MORE THAN IT NEEDS: the line of its panel that counts the redundant constraints whose
/// values agree - the program's own word that they are kept as references, not an error.
pub fn says_redundant(s: &mut qymcad::Session) -> bool {
    let line = s.word("sk-redundant-n");
    let lead = line.split('{').next().unwrap_or_default().trim().to_string();
    !lead.is_empty() && s.words().iter().any(|w| w.starts_with(&lead))
}

/// WHAT A PERSON BUILDS FIRST, built the way a person builds it: every step a gesture in the window.
pub mod build {
    use qymcad::{pos2, Key, Modifiers, PointerButton, Session};

    /// THE PART OF A FIRST START, OPEN FOR BUILDING: the start screen put away with Esc, the part entered with a
    /// double click on its row in the tree.
    pub fn into_the_first_part(s: &mut Session) {
        s.key(Key::Escape);
        // a start holds an empty assembly - the first part is made with the button of the assembly and stood in; a new
        // project holds its first part - it is stepped into from the tree
        match s.document().parts.iter().find(|p| !p.assembly).map(|p| p.name.clone()) {
            None => {
                let new_part = s.word("tb-new-part-hint");
                s.press_hint(&new_part);
            }
            Some(part) => {
                let row = s.find(&part, pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the first part {part:?} is not in the tree; on screen: {:?}", s.words()));
                s.double_click(row.center());
            }
        }
    }

    /// A 40 x 30 RECTANGLE ON XY with a corner at the origin: a sketch on the plane, the rectangle tool by its
    /// hint, two corners clicked on the canvas; the sketch is left open.
    pub fn rectangle_on_xy(s: &mut Session) {
        rectangle_at(s, [(0.0, 0.0), (40.0, 30.0)]);
    }

    /// A RECTANGLE ON THE XY PLANE BETWEEN THE TWO CORNERS GIVEN, the sketch left open.
    ///
    /// A check that wants SEVERAL bodies out of one part needs its sketches apart: the same rectangle drawn
    /// again lands on the first one, so the second extrusion grows out of the first and the part holds one
    /// body where three were asked for.
    pub fn rectangle_at(s: &mut Session, corners: [(f64, f64); 2]) {
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        let rect = s.word("tb-rect-hint");
        s.press_hint(&rect);
        s.click_on_sketch(corners[0].0, corners[0].1).click_on_sketch(corners[1].0, corners[1].1);
    }

    /// AN EMPTY SKETCH ON THE XY PLANE OF THE FIRST PART, with the auto constraints turned off - so that a check of
    /// one tool sees what that tool did and nothing the program added while drawing.
    pub fn empty_sketch() -> Session {
        let mut s = Session::start();
        into_the_first_part(&mut s);
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        let auto = s.word("wb-auto-constraints");
        let switch = s.widgets().into_iter().find(|w| w.label.ends_with(&auto)).unwrap_or_else(|| panic!("the sketch has no switch of the auto constraints; on screen: {:?}", s.words()));
        assert!(switch.checked == Some(true), "the auto constraints are off before anything is drawn: the switch says {:?}", switch.checked);
        s.click(switch.rect.center());
        s
    }

    /// A SKETCH SEATED ON THE FACE OF A BODY through `p`: the pencil of the panel, then the face clicked in the 3D
    /// view. The sketch is left open.
    pub fn sketch_on_face(s: &mut Session, p: [f64; 3]) {
        let pencil = s.word("g-sketch-pick-hint");
        s.press_hint(&pencil);
        let at = s.face_at(p);
        s.click(at);
    }

    /// Draw with the tool whose hint is `hint`, clicking these places of the sheet in turn, and take the arrow back.
    pub fn draw(s: &mut Session, hint: &str, places: &[(f64, f64)]) {
        let hint = s.word(hint);
        s.press_hint(&hint);
        for (x, y) in places {
            s.click_on_sketch(*x, *y);
        }
        s.key(Key::Escape);
        let arrow = s.word("tb-select-hint");
        s.press_hint(&arrow);
    }

    /// A line of the open sketch from `a` to `b`.
    pub fn line(s: &mut Session, a: (f64, f64), b: (f64, f64)) {
        draw(s, "tb-line-hint", &[a, b]);
    }

    /// A circle of the open sketch about `c` through `rim`.
    pub fn circle(s: &mut Session, c: (f64, f64), rim: (f64, f64)) {
        draw(s, "tb-circle-hint", &[c, rim]);
    }

    /// A point of the open sketch at `p`.
    pub fn point(s: &mut Session, p: (f64, f64)) {
        draw(s, "tb-point-hint", &[p]);
    }

    /// Pick what lies at (x, y) of the open sketch, adding to what is picked when `add`.
    pub fn pick(s: &mut Session, x: f64, y: f64, add: bool) {
        let at = s.on_sketch(x, y);
        if add {
            s.click_with(at, PointerButton::Primary, Modifiers::SHIFT);
        } else {
            s.click(at);
        }
    }

    /// A NEW PART OF THE ASSEMBLY, OPEN FOR BUILDING: back to the assembly by its word at the head of the path, then
    /// the new-part button, which steps inside the part it makes.
    /// A START WITH ONE PART IN ITS ASSEMBLY, the person standing in the assembly: the part is made with the button of
    /// the assembly and the path leads back up. A start holds an empty assembly - a part to work with is made.
    pub fn a_part_in_the_assembly(s: &mut Session) {
        s.key(Key::Escape);
        let new_part = s.word("tb-new-part-hint");
        s.press_hint(&new_part);
        let assembly = s.word("wb-assembly");
        s.press_word_near(&assembly, pos2(0.0, 0.0));
    }

    pub fn into_a_new_part(s: &mut Session) {
        let assembly = s.word("wb-assembly");
        s.press_word_near(&assembly, pos2(0.0, 0.0));
        let new_part = s.word("tb-new-part-hint");
        s.press_hint(&new_part);
    }

    /// A PARAMETER NAMED `name` HOLDING `expr`, added in the table of parameters, the table closed again.
    pub fn parameter(s: &mut Session, name: &str, expr: &str) {
        let params = s.word("wb-params");
        s.press_word_near(&params, pos2(0.0, 0.0));
        let add = s.word("par-add");
        s.press_word(&add);
        let example = s.word("par-example");
        s.fill_empty("w", name).fill_empty(&example, expr).key(Key::Enter);
        let title = s.word("win-params");
        s.close_window(&title);
    }

    /// WRITE `expr` INTO THE PARAMETER `name`: the table, the row that reads that name, the field beside it typed over,
    /// the table closed again.
    pub fn set_parameter(s: &mut Session, name: &str, expr: &str) {
        let params = s.word("wb-params");
        s.press_word_near(&params, pos2(0.0, 0.0));
        // the row is sought beside the table's own button: the same name can stand elsewhere, in a field that uses it
        let add = s.word("par-add");
        let near = s.find(&add, pos2(640.0, 400.0)).map_or(pos2(640.0, 400.0), |r| r.center());
        let row = s.find(name, near).unwrap_or_else(|| panic!("the table shows no parameter {name:?}; on screen: {:?}", s.words()));
        let field = s
            .widgets()
            .into_iter()
            .filter(|w| w.kind == qymcad::Kind::TextField && w.rect.center().y > row.min.y && w.rect.center().y < row.max.y && w.rect.min.x > row.max.x)
            .min_by(|a, b| a.rect.min.x.total_cmp(&b.rect.min.x))
            .unwrap_or_else(|| panic!("the row of {name:?} has no field to write an expression in; on screen: {:?}", s.words()));
        s.click(field.rect.center()).chord(qymcad::Modifiers::COMMAND, Key::A).type_text(expr).key(Key::Enter);
        let title = s.word("win-params");
        s.close_window(&title);
    }

    /// DELETE THE ONE PARAMETER OF THE TABLE by the button of its row, found by its hint, the table closed again.
    pub fn delete_the_parameter(s: &mut Session) {
        let params = s.word("wb-params");
        s.press_word_near(&params, pos2(0.0, 0.0));
        let delete = s.word("par-delete");
        s.press_hint(&delete);
        let title = s.word("win-params");
        s.close_window(&title);
    }

    /// OPEN THE PROJECT AT `path` as a person does: File, Open project, past the question about unsaved work if it
    /// is asked, and the path given to the chooser.
    pub fn open_project(s: &mut Session, path: &str) {
        let (file, open) = (s.word("menu-file"), s.word("file-open"));
        s.menu(&[&file, &open]);
        let dont_save = s.word("nav-dont-save");
        if let Some(button) = s.find(&dont_save, pos2(0.0, 0.0)) {
            s.click(button.center());
        }
        s.answer_file(path);
    }

    /// BRING THE FILE AT `path` INTO THE OPEN DOCUMENT: File, Import, the path given to the chooser, and the window
    /// of units and scale answered as it stands - the meshes ask, the exact formats come straight in.
    pub fn import(s: &mut Session, path: &str) {
        let (file, import) = (s.word("menu-file"), s.word("file-import"));
        s.menu(&[&file, &import]);
        s.answer_file(path);
        let go = s.word("import-scale-import");
        if let Some(button) = s.find(&go, pos2(1200.0, 700.0)) {
            s.click(button.center());
        }
    }

    /// SAVE THE PROJECT AS `path`: File, Save as, the path given to the chooser.
    pub fn save_as(s: &mut Session, path: &str) {
        // a file left at the same place by an earlier run would pass for the one saved now, were the save to fail
        let _ = std::fs::remove_file(path);
        let (file, save_as) = (s.word("menu-file"), s.word("file-save-as"));
        s.menu(&[&file, &save_as]);
        s.answer_file(path);
    }

    /// A 40 x 30 x 10 BLOCK in the first part: the rectangle, Finish, Extrude by its hint, Enter.
    pub fn block(s: &mut Session) {
        into_the_first_part(s);
        rectangle_on_xy(s);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude).key(Key::Enter);
    }
}

#[cfg(test)]
mod samples {
    /// A SAMPLE THAT IS NOT IN THE TREE IS PASSED OVER, not opened: the public clone has no samples.
    #[test]
    fn a_missing_sample_is_passed_over() {
        assert!(super::private_sample("no-such-sample.qcad").is_none(), "a sample that is not there was given a path to open");
    }
}
