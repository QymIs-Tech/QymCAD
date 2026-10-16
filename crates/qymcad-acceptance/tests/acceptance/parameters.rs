//! THE PARAMETERS OF A DOCUMENT: the table, an expression written from another parameter, a dimension driven by one,
//! the names and the formulas that cannot be taken, and what follows when a parameter changes.
use qymcad::{Key, Modifiers, Session};
use qymcad_acceptance::{build, probe};

/// The table of parameters, opened.
fn open_the_table(s: &mut Session) {
    let params = s.word("wb-params");
    s.press_word_near(&params, qymcad::pos2(0.0, 0.0));
}

/// The table of parameters, put away again.
fn close_the_table(s: &mut Session) {
    let title = s.word("win-params");
    s.close_window(&title);
}

/// What the parameter `name` holds, as the document has it.
fn parameter(s: &mut Session, name: &str) -> qymcad::Parameter {
    s.document().parameters.iter().find(|p| p.name == name).cloned().unwrap_or_else(|| panic!("the document holds no parameter {name:?}: it holds {:?}", s.document().parameters))
}

/// WRITE `expr` INTO THE PARAMETER `name`: the table, the row that reads that name, the field beside it typed over.
fn set_parameter(s: &mut Session, name: &str, expr: &str) {
    open_the_table(s);
    // the row is sought beside the table's own button: the same name can stand elsewhere, in a field that uses it
    let add = s.word("par-add");
    let near = s.find(&add, qymcad::pos2(640.0, 400.0)).map_or(qymcad::pos2(640.0, 400.0), |r| r.center());
    let row = s.find(name, near).unwrap_or_else(|| panic!("the table shows no parameter {name:?}; on screen: {:?}", s.words()));
    let field = s
        .widgets()
        .into_iter()
        .filter(|w| w.kind == qymcad::Kind::TextField && w.rect.center().y > row.min.y && w.rect.center().y < row.max.y && w.rect.min.x > row.max.x)
        .min_by(|a, b| a.rect.min.x.total_cmp(&b.rect.min.x))
        .unwrap_or_else(|| panic!("the row of {name:?} has no field to write an expression in; on screen: {:?}", s.words()));
    s.click(field.rect.center()).chord(Modifiers::COMMAND, Key::A).type_text(expr).key(Key::Enter);
    close_the_table(s);
}

/// ADD A PARAMETER AND ANSWER WHAT THE TABLE SAYS about it, the table left open.
fn add_and_see(s: &mut Session, name: &str, expr: &str) -> Vec<String> {
    open_the_table(s);
    let add = s.word("par-add");
    s.press_word(&add);
    let example = s.word("par-example");
    s.fill_empty("w", name).fill_empty(&example, expr).key(Key::Enter);
    s.words()
}

probe! {
    /// A PARAMETER IS ADDED AND COUNTED: the name and the number a person typed are what the document holds.
    fn a_parameter_is_added_and_counted() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::parameter(&mut s, "w", "40");
        let w = parameter(&mut s, "w");
        assert!((w.value - 40.0).abs() < 1e-9, "the parameter w was written as 40 and holds {}", w.value);
        assert!(w.expr.trim() == "40", "the parameter w was written as \"40\" and the document keeps {:?}", w.expr);
    }
}

probe! {
    /// A PARAMETER IS WRITTEN FROM ANOTHER: h = w/2 of a w of 40 is 20, and it follows w when w changes.
    fn a_parameter_is_written_from_another_and_follows_it() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::parameter(&mut s, "w", "40");
        build::parameter(&mut s, "h", "w/2");
        assert!((parameter(&mut s, "h").value - 20.0).abs() < 1e-9, "h = w/2 of a w of 40 is 20, and it holds {}", parameter(&mut s, "h").value);
        set_parameter(&mut s, "w", "60");
        assert!((parameter(&mut s, "w").value - 60.0).abs() < 1e-9, "w was written as 60 and holds {}", parameter(&mut s, "w").value);
        assert!((parameter(&mut s, "h").value - 30.0).abs() < 1e-9, "w became 60 and h = w/2 holds {} instead of 30", parameter(&mut s, "h").value);
    }
}

probe! {
    /// A DIMENSION DRIVEN BY A PARAMETER FOLLOWS IT, and the body built on that sketch follows the dimension.
    fn a_dimension_driven_by_a_parameter_follows_it() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::parameter(&mut s, "w", "40");
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        let rect = s.word("tb-rect-hint");
        s.press_hint(&rect);
        s.click_on_sketch(0.0, 0.0).click_on_sketch(40.0, 30.0);
        s.key(Key::Escape);
        // the lower side of the rectangle is given the length w
        let dim = s.word("tb-dim-hint");
        s.press_hint(&dim);
        s.click_on_sketch(20.0, 0.0);
        s.click_on_sketch(20.0, -10.0);
        let field = s.word("sk-expr-example");
        s.fill_hinted(&field, "w").key(Key::Enter);
        s.key(Key::Escape);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        s.key(Key::Enter);
        s.key(Key::Escape);
        let across = |s: &mut Session| {
            let b = s.document().bodies.iter().find(|b| !b.consumed && !b.sheet).cloned().unwrap_or_else(|| panic!("the part holds no body"));
            b.max[0] - b.min[0]
        };
        assert!((across(&mut s) - 40.0).abs() < 0.01, "the block was built on a side of w = 40 and it is {} across", across(&mut s));
        set_parameter(&mut s, "w", "60");
        let sheet = s.document().sketches.first().map(|sk| sk.max[0] - sk.min[0]).unwrap_or(f64::NAN);
        assert!((across(&mut s) - 60.0).abs() < 0.01, "w was made 60 and the block is {} across; the sketch it was built on is {sheet} wide", across(&mut s));
    }
}

probe! {
    /// A NAME ALREADY TAKEN IS REFUSED IN WORDS, and the table does not end up with two of them.
    fn a_name_already_taken_is_refused() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::parameter(&mut s, "w", "40");
        let said = add_and_see(&mut s, "w", "10");
        let taken = s.word("par-name-taken").replace("{ $name }", "w").replace("{ $where }", "");
        let head = taken.split("{").next().unwrap_or(&taken).trim().to_string();
        assert!(said.iter().any(|t| t.contains(&head)), "a second parameter w was added and nothing says the name is taken: {said:?}");
        assert!(s.document().parameters.iter().filter(|p| p.name == "w").count() == 1, "the document holds two parameters named w: {:?}", s.document().parameters);
    }
}

probe! {
    /// A NAME THAT CANNOT STAND IN A FORMULA IS REFUSED IN WORDS: a name beginning with a digit is not a name.
    fn a_name_that_cannot_stand_in_a_formula_is_refused() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let said = add_and_see(&mut s, "2w", "10");
        let bad = s.word("par-name-bad");
        let head = bad.split("{").next().unwrap_or(&bad).trim().to_string();
        assert!(said.iter().any(|t| t.contains(&head)), "a parameter called \"2w\" was added and nothing says the name cannot be used: {said:?}");
        assert!(!s.document().parameters.iter().any(|p| p.name == "2w"), "the document took a parameter called \"2w\": {:?}", s.document().parameters);
    }
}

probe! {
    /// A FORMULA THAT CANNOT BE COUNTED HAS NO VALUE AND THE TABLE SAYS SO: an unknown name, and a circle of two
    /// parameters that live off each other.
    fn a_formula_that_cannot_be_counted_says_so() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let said = add_and_see(&mut s, "h", "qqq + 1");
        assert!(said.iter().any(|t| t.contains("qqq")), "h was written as \"qqq + 1\" and nothing in the table names the word it does not know: {said:?}");
        close_the_table(&mut s);
        build::parameter(&mut s, "a", "1");
        build::parameter(&mut s, "b", "a + 1");
        set_parameter(&mut s, "a", "b + 1");
        open_the_table(&mut s);
        let said = s.words();
        close_the_table(&mut s);
        let complaint = said.iter().any(|t| t.contains(&s.word("par-no-value")) || t.to_lowercase().contains("circ") || t.contains("!"));
        assert!(complaint, "a = b + 1 and b = a + 1 were written and the table says nothing about it: {said:?}");
    }
}

/// How far the second part of the assembly stands turned from how it was built, in degrees.
fn second_part_turned(s: &mut Session) -> f64 {
    let p = s.document().parts[1].clone();
    let trace = p.axes[0][0] + p.axes[1][1] + p.axes[2][2];
    ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos().to_degrees()
}

probe! {
    /// THE ANGLE OF A HINGE DRIVEN BY A PARAMETER FOLLOWS IT: "pa" typed into the angle of the hinge taken in the list
    /// of mates turns the part by pa = 90, and pa made 45 turns it by 45.
    fn a_hinge_angle_driven_by_a_parameter_follows_it() {
        let mut s = qymcad_acceptance::contract::fixtures::Fixture::TwoHingesInAssembly.start();
        build::parameter(&mut s, "pa", "90");
        s.press_word("Revolute 1");
        let caption = s.word("j-angle-lower");
        s.fill(&caption, "pa");
        s.key(Key::Enter);
        let at_90 = second_part_turned(&mut s);
        assert!((at_90 - 90.0).abs() < 0.5, "the angle of the hinge was typed as pa = 90 and the part stands turned by {at_90:.2} deg");
        set_parameter(&mut s, "pa", "45");
        let at_45 = second_part_turned(&mut s);
        assert!((at_45 - 45.0).abs() < 0.5, "pa became 45 and the part driven by it stands turned by {at_45:.2} deg");
    }
}

/// THE HEIGHT THE TREE WRITES on the row of the extrusion, read off the screen; `None` when no row shows one.
fn height_in_the_tree(s: &mut Session) -> Option<f64> {
    let line = s.word("feat-extrude");
    let head = line.split('{').next().unwrap_or_default().to_string();
    s.words().iter().find_map(|w| w.split_once(head.as_str()).and_then(|(_, rest)| rest.trim().replace(',', ".").parse().ok()))
}

probe! {
    /// A PARAMETER NAMED IN CAPITALS DRIVES ITS NODE as one in small letters does: the body is rebuilt and the tree
    /// writes the new height on the row.
    /// Reported behaviour: a parameter H changed from 20 to 40 under an extrusion of height H, the properties said
    /// 40, the body stayed 20 high; with the name h the body was rebuilt, yet the tree row still said h=20.0.
    fn a_parameter_named_in_capitals_rebuilds_what_counts_from_it() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::parameter(&mut s, "H", "12");
        build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        let length = s.word("f-length");
        s.fill(&length, "H").key(Key::Enter);
        if !s.in_hand().is_empty() {
            s.key(Key::Enter); // the first Enter took the name from the list of names
        }
        let volume = |s: &mut Session| s.document().bodies.iter().find(|b| !b.consumed).map(|b| b.volume).unwrap_or(f64::NAN);
        assert!((volume(&mut s) - 14400.0).abs() < 1e-3, "setup: the block 40 x 30 x H = 12 is {} mm^3", volume(&mut s));
        set_parameter(&mut s, "H", "20");
        assert!((parameter(&mut s, "H").value - 20.0).abs() < 1e-9, "H was written as 20 and holds {}", parameter(&mut s, "H").value);
        assert!((volume(&mut s) - 24000.0).abs() < 1e-3, "H was made 20 and the block 40 x 30 x H is {} mm^3, not 24000", volume(&mut s));
        let shown = height_in_the_tree(&mut s);
        assert!(shown.is_some_and(|h| (h - 20.0).abs() < 1e-9), "H was made 20 and the tree writes the height of the extrusion as {shown:?}");
    }
}

probe! {
    /// A PARAMETER DELETED UNDER A NODE THAT COUNTS FROM IT TURNS THE NODE RED WITH ITS NAME, the body standing at its
    /// last good state; the deletion is a step of undo, and Ctrl+Z brings the parameter and a green node back.
    /// Reported behaviour: the parameter w deleted from under an extrusion h = w, no step of undo, and the extrusion
    /// stood green 12 high on a name nothing held any more.
    fn a_parameter_deleted_under_a_node_turns_it_red() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::parameter(&mut s, "w", "12");
        build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        let length = s.word("f-length");
        s.fill(&length, "w").key(Key::Enter);
        if !s.in_hand().is_empty() {
            s.key(Key::Enter); // the first Enter took the name from the list of names
        }
        let volume = |s: &mut Session| s.document().bodies.iter().find(|b| !b.consumed).map(|b| b.volume).unwrap_or(f64::NAN);
        assert!((volume(&mut s) - 14400.0).abs() < 1e-3, "setup: the block 40 x 30 x w = 12 is {} mm^3", volume(&mut s));
        open_the_table(&mut s);
        // the one row of the table, w: its button of deleting, found by its hint
        let delete = s.word("par-delete");
        s.press_hint(&delete);
        close_the_table(&mut s);
        let doc = s.document();
        let step = s.word("par-delete-step");
        assert!(doc.undo.last() == Some(&step), "the parameter was deleted and the last step of undo is {:?}, not {step:?}", doc.undo.last());
        let node = doc.features.iter().find(|f| f.kind == "Extrude").expect("the extrusion");
        assert!(node.error.as_deref().is_some_and(|e| e.contains('w')), "w was deleted from under h = w and the extrusion says {:?}", node.error);
        assert!((volume(&mut s) - 14400.0).abs() < 1e-3, "the red extrusion is to keep its last good body, it is {} mm^3", volume(&mut s));
        s.chord(Modifiers::COMMAND, Key::Z);
        let doc = s.document();
        assert!(doc.parameters.iter().any(|p| p.name == "w"), "Ctrl+Z did not bring w back: {:?}", doc.parameters);
        let node = doc.features.iter().find(|f| f.kind == "Extrude").expect("the extrusion");
        assert!(node.error.is_none(), "w is back and the extrusion still says {:?}", node.error);
    }
}

probe! {
    /// A STAIRCASE RESIZED BY ITS PARAMETERS KEEPS EVERY ROUNDING IN THE PART (a private sample): three steps 10 high,
    /// the first chamfered 10 x 10, the second rounded R10, the third chamfered 10 x 30 - each of their faces eaten to
    /// nothing - roundings of R2 after them, the last on the arc of the R10 on the side, its depth the parameter `h`.
    /// Reported behaviour: `h` written 60, the window rebuilt for minutes and left sheets and holes over the screen: the
    /// last rounding landed a body of 4.2e5 mm^3 reaching 14 m out of a part of 6.4e4 mm^3, built in 53 s.
    fn a_staircase_resized_by_its_parameters_keeps_every_rounding_in_the_part() {
        let Some(path) = qymcad_acceptance::private_sample("test_bridge_resize_body.qcad") else { return };
        let mut s = Session::start();
        build::open_project(&mut s, &path);
        let started = std::time::Instant::now();
        set_parameter(&mut s, "h", "60");
        let doc = s.document();
        let took = started.elapsed().as_secs_f64();
        assert!((parameter(&mut s, "h").value - 60.0).abs() < 1e-9, "setup: h was written as 60");
        // the steps stand in a block 50 across, 60 deep, 30 high
        let block = 50.0 * 60.0 * 30.0;
        for b in doc.bodies.iter().filter(|b| !b.consumed && !b.sheet) {
            assert!(b.volume > 0.0 && b.volume < block, "the part {:?} is {} mm^3, beyond the block of {block} mm^3 its steps stand in", b.name, b.volume);
            let out = b.face_centres.iter().find(|c| c[0] < -50.001 || c[0] > 0.001 || c[1] < -0.001 || c[1] > 60.001 || c[2] < -0.001 || c[2] > 30.001);
            assert!(out.is_none(), "the part {:?} has a face standing at {:?}, out of the block 50 x 60 x 30 its steps stand in", b.name, out);
        }
        assert!(took < 30.0, "the rebuild after h was written took {took:.0} s");
    }
}
