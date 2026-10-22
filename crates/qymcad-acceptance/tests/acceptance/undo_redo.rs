//! TAKING A STEP BACK AND PUTTING IT AGAIN: Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z and the Edit menu, after every kind of
//! edit a person makes - drawing, dimensioning, building a body, a parameter, a rename, a deletion, moving a part.
use qymcad::{Document, Key, Modifiers, Session};
use qymcad_acceptance::{build, probe};

/// What a round trip compares: the steps of the timeline with their errors, the sketches with what is drawn in
/// them, the bodies with their numbers, the parameters and the names of the parts.
fn shape(doc: &Document) -> String {
    let features: Vec<(String, String, Option<String>)> = doc.features.iter().map(|f| (f.name.clone(), f.kind.clone(), f.error.clone())).collect();
    let sketches: Vec<(String, usize, usize, usize, usize)> = doc.sketches.iter().map(|sk| (sk.name.clone(), sk.lines, sk.circles, sk.arcs, sk.constraints)).collect();
    let bodies: Vec<(String, i64, usize)> = doc.bodies.iter().filter(|b| !b.consumed).map(|b| (b.name.clone(), (b.volume * 1e3).round() as i64, b.faces)).collect();
    let parts: Vec<(String, [i64; 3])> = doc.parts.iter().map(|p| (p.name.clone(), p.at.map(|v| (v * 1e3).round() as i64))).collect();
    let params: Vec<(String, String)> = doc.parameters.iter().map(|p| (p.name.clone(), p.expr.clone())).collect();
    format!("{features:?} {sketches:?} {bodies:?} {parts:?} {params:?}")
}

/// The block of the first part, with nothing in hand.
fn a_block() -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    s.key(Key::Escape);
    s
}

/// Take a step back with Ctrl+Z.
fn take_back(s: &mut Session) {
    s.chord(Modifiers::COMMAND, Key::Z);
}

probe! {
    /// CTRL+Z TAKES BACK THE LAST STEP AND CTRL+Y PUTS IT AGAIN, both exactly.
    fn ctrl_z_takes_back_a_step_and_ctrl_y_puts_it_again() {
        let mut s = a_block();
        let built = shape(&s.document());
        assert!(!s.document().undo.is_empty(), "a block was built and there is nothing to take back");
        take_back(&mut s);
        let back = shape(&s.document());
        assert!(back != built, "Ctrl+Z was pressed and the document is what it was: {built}");
        s.chord(Modifiers::COMMAND, Key::Y);
        assert!(shape(&s.document()) == built, "Ctrl+Y put the step again and the document is {} instead of {built}", shape(&s.document()));
        // and once more, so that the pair works twice over
        take_back(&mut s);
        assert!(shape(&s.document()) == back, "the second Ctrl+Z gives {} instead of {back}", shape(&s.document()));
    }
}

probe! {
    /// CTRL+SHIFT+Z PUTS THE STEP AGAIN TOO - the other hand's way of the same thing.
    fn ctrl_shift_z_puts_a_step_again() {
        let mut s = a_block();
        let built = shape(&s.document());
        take_back(&mut s);
        s.chord(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        assert!(shape(&s.document()) == built, "Ctrl+Shift+Z put the step again and the document is {} instead of {built}", shape(&s.document()));
    }
}

probe! {
    /// THE EDIT MENU TAKES BACK AND PUTS AGAIN, and names the step it is about to take back.
    fn the_edit_menu_takes_back_and_puts_again() {
        let mut s = a_block();
        let built = shape(&s.document());
        let step = s.document().undo.last().cloned().expect("the step that built the block");
        let edit = s.word("menu-edit");
        // the items name the step they are about: "Undo: Extrusion"
        let named = |line: String| line.replace("{ $what }", &step).replace("{$what}", &step);
        let (undo, redo) = (named(s.word("menu-undo-named")), named(s.word("menu-redo-named")));
        s.press_word(&edit);
        assert!(s.shows(&undo), "the Edit menu does not offer {undo:?}; on screen: {:?}", s.words());
        s.press_word(&undo);
        assert!(shape(&s.document()) != built, "the Edit menu took the step back and the document is what it was");
        s.press_word(&edit);
        s.press_word(&redo);
        assert!(shape(&s.document()) == built, "the Edit menu put the step again and the document is {} instead of {built}", shape(&s.document()));
    }
}

probe! {
    /// EVERY KIND OF EDIT CAN BE TAKEN BACK AND PUT AGAIN: drawing, a dimension, a body, a parameter, a rename, a
    /// deletion and the placing of a part - each one a step of its own, each one exact both ways.
    fn every_kind_of_edit_can_be_taken_back_and_put_again() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        let edits: Vec<Edit> = vec![
            edit("drawing a rectangle", |s| {
                let rect = s.word("tb-rect-hint");
                s.press_hint(&rect);
                s.click_on_sketch(0.0, 0.0).click_on_sketch(40.0, 30.0);
                s.key(Key::Escape);
            }),
            edit("a dimension", |s| {
                let dim = s.word("tb-dim-hint");
                s.press_hint(&dim);
                s.click_on_sketch(20.0, 0.0);
                s.click_on_sketch(20.0, -10.0);
                let field = s.word("f-number-or-expr");
                s.fill_hinted(&field, "40").key(Key::Enter);
                s.key(Key::Escape);
            }),
            edit("a body", |s| {
                let finish = s.word("wb-finish");
                s.press_word(&finish);
                let extrude = s.word("tb-extrude-hint");
                s.press_hint(&extrude);
                s.key(Key::Enter);
                s.key(Key::Escape);
            }),
            edit("a parameter", |s| build::parameter(s, "w", "40")),
            edit("a rename", |s| {
                let sketch = s.document().sketches.first().map(|sk| sk.name.clone()).expect("the sketch");
                let row = s.find(&sketch, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the sketch {sketch:?} is not in the tree"));
                s.click(row.center());
                s.key(Key::F2);
                s.chord(Modifiers::COMMAND, Key::A).type_text("Contour").key(Key::Enter);
            }),
        ];
        let problems = each_taken_back_and_put_again(&mut s, edits);
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}

probe! {
    /// A STEP IS NAMED IN THE LIST OF WHAT CAN BE TAKEN BACK, so a person knows what Ctrl+Z is about to undo.
    fn a_step_is_named_in_the_list() {
        let mut s = a_block();
        let steps = s.document().undo;
        assert!(!steps.is_empty(), "a block was built and the list of steps is empty");
        assert!(steps.iter().all(|w| !w.trim().is_empty() && !w.contains('-')), "the steps of the list are not words a person reads: {steps:?}");
    }
}

probe! {
    /// TAKING BACK WITH NOTHING TO TAKE BACK DOES NOTHING and says nothing is broken.
    fn taking_back_with_nothing_to_take_back_does_nothing() {
        let mut s = Session::start();
        s.key(Key::Escape);
        let before = shape(&s.document());
        assert!(s.document().undo.is_empty(), "a first start already has steps to take back: {:?}", s.document().undo);
        take_back(&mut s);
        s.chord(Modifiers::COMMAND, Key::Y);
        assert!(shape(&s.document()) == before, "Ctrl+Z and Ctrl+Y on a fresh start changed the document to {}", shape(&s.document()));
    }
}

probe! {
    /// FACES SPLIT BY A PLANE KEEP THEIR NAMES THROUGH UNDO AND REDO: the block's sides divided across the middle, a
    /// step back and forward again, and every name of a face stands on the face it stood on - where a sketch or a
    /// rounding taken on it will look for it.
    fn split_faces_keep_their_names_through_undo_and_redo() {
        let mut s = a_block();
        let hint = s.word("tb-split-face-hint");
        s.press_hint(&hint);
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        let offset = s.word("f-offset");
        s.fill(&offset, "-5").key(Key::Enter);
        let named = |s: &mut Session| {
            let doc = s.document();
            let body = doc.bodies.iter().rfind(|b| !b.consumed).cloned().unwrap_or_else(|| panic!("the part holds no body"));
            let mut out: Vec<(u32, [i64; 3])> = body.face_names.iter().zip(&body.face_centres).map(|(n, c)| (*n, c.map(|v| (v * 100.0).round() as i64))).collect();
            out.sort();
            out
        };
        let before = named(&mut s);
        assert!(before.len() == 10, "the four sides were not divided: the body has {} faces", before.len());
        take_back(&mut s);
        s.chord(Modifiers::COMMAND, Key::Y);
        let after = named(&mut s);
        let moved: Vec<String> = before.iter().filter(|b| !after.contains(b)).map(|(n, c)| format!("{n} from {c:?} to {:?}", after.iter().find(|a| a.0 == *n).map(|a| a.1))).collect();
        assert!(moved.is_empty(), "after a step back and forward the names moved to other faces (centres in hundredths of a mm): {moved:?}");
    }
}

probe! {
    /// A ROUNDING ON AN EDGE OF DIVIDED FACES STAYS ON THAT EDGE THROUGH UNDO AND REDO: the block's sides divided
    /// across the middle, the top right edge rounded 1, two steps back and two forward - the rounding is where it was,
    /// taking the 30 of that edge and not another.
    fn a_rounding_on_divided_faces_stays_on_its_edge_through_undo_and_redo() {
        let mut s = a_block();
        let hint = s.word("tb-split-face-hint");
        s.press_hint(&hint);
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        let offset = s.word("f-offset");
        s.fill(&offset, "-5").key(Key::Enter);
        let fillet = s.word("tb-fillet-body-hint");
        s.press_hint(&fillet);
        let edge = s.edge_at([40.0, 15.0, 10.0]);
        s.click(edge);
        let radius = s.word("f-radius");
        s.fill(&radius, "1").key(Key::Enter);
        let body = |s: &mut Session| {
            let doc = s.document();
            let b = doc.bodies.iter().rfind(|b| !b.consumed).cloned().unwrap_or_else(|| panic!("the part holds no body"));
            let mut faces: Vec<[i64; 3]> = b.face_centres.iter().map(|c| c.map(|v| (v * 100.0).round() as i64)).collect();
            faces.sort();
            ((b.volume * 1e3).round() as i64, faces)
        };
        let rounded = body(&mut s);
        let taken = 12000.0 - rounded.0 as f64 / 1e3;
        let want = 30.0 * (1.0 - std::f64::consts::PI / 4.0);
        assert!((taken - want).abs() < 0.01, "the rounding of 1 on the top right edge takes {want}, and it took {taken}");
        for _ in 0..2 {
            take_back(&mut s);
        }
        for _ in 0..2 {
            s.chord(Modifiers::COMMAND, Key::Y);
        }
        let again = body(&mut s);
        assert!(again == rounded, "after two steps back and two forward the body is not the one rounded: {} mm^3 over faces at {:?}, it was {} over {:?}", again.0 as f64 / 1e3, again.1, rounded.0 as f64 / 1e3, rounded.1);
    }
}

/// An edit a person makes, by what it is called in a report.
struct Edit {
    what: &'static str,
    make: fn(&mut Session),
}

fn edit(what: &'static str, make: fn(&mut Session)) -> Edit {
    Edit { what, make }
}

/// EVERY EDIT IN TURN, TAKEN BACK AND PUT AGAIN step by step: an edit made of several things a person did (a new part,
/// then its placement) lays a named step for each, and taking them all back gives the document before it. What fails is
/// told at the end - one kind that cannot be taken back must not hide the rest.
fn each_taken_back_and_put_again(s: &mut Session, edits: Vec<Edit>) -> Vec<String> {
    let mut problems: Vec<String> = Vec::new();
    let unnamed = s.word("undo-edit");
    for Edit { what, make: edit } in edits {
        let before = shape(&s.document());
        let steps = s.document().undo.len();
        edit(s);
        let after = shape(&s.document());
        if after == before {
            problems.push(format!("{what} left the document as it was"));
            continue;
        }
        let laid: Vec<String> = s.document().undo.iter().skip(steps).cloned().collect();
        if laid.is_empty() || laid.contains(&unnamed) {
            problems.push(format!("{what} laid the steps {laid:?}: each thing done is a step with its own name"));
        }
        let n = laid.len().max(1);
        for _ in 0..n {
            take_back(s);
        }
        if shape(&s.document()) != before {
            problems.push(format!("{what}: taken back {n} steps, the document is {} instead of {before}", shape(&s.document())));
        }
        for _ in 0..n {
            s.chord(Modifiers::COMMAND, Key::Y);
        }
        if shape(&s.document()) != after {
            problems.push(format!("{what}: put again, the document is {} instead of {after}", shape(&s.document())));
        }
    }
    problems
}

probe! {
    /// A SECOND PART PUT BESIDE, AND ITS DELETION, are each taken back by one Ctrl+Z and put again by one Ctrl+Y: making
    /// a part, drawing and building it, and moving it along X is one thing a person did.
    fn a_second_part_and_its_deletion_are_taken_back_and_put_again() {
        let mut s = Session::start();
        build::block(&mut s);
        let edits: Vec<Edit> = vec![
            edit("a second part put beside", |s| {
                build::into_a_new_part(s);
                let assembly = s.word("wb-assembly");
                s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
                let made = s.document().parts.iter().filter(|p| !p.assembly).map(|p| p.name.clone()).next_back().expect("the new part");
                let row = s.find(&made, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the part {made:?} is not in the tree"));
                s.click(row.center());
                s.fill("X", "40").key(Key::Enter);
            }),
            edit("a deletion", |s| {
                let made = s.document().parts.iter().filter(|p| !p.assembly).map(|p| p.name.clone()).next_back().expect("the part to delete");
                let row = s.find(&made, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the part {made:?} is not in the tree"));
                s.click(row.center());
                s.key(Key::Delete);
                let yes = s.word("confirm-yes");
                if s.shows(&yes) {
                    s.press_word(&yes);
                }
            }),
        ];
        let problems = each_taken_back_and_put_again(&mut s, edits);
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}

probe! {
    /// A NEW PART TAKEN BACK AND PUT AGAIN IS STOOD IN AGAIN: the next sketch goes into it, not into a second part of
    /// the same name. Reported behaviour: after Ctrl+Z and Ctrl+Y of "New part" a sketch on XY made another "Part 1".
    fn a_new_part_put_again_is_stood_in() {
        let mut s = Session::start();
        s.key(Key::Escape);
        let new_part = s.word("tb-new-part-hint");
        s.press_hint(&new_part);
        take_back(&mut s);
        s.chord(Modifiers::COMMAND, Key::Y);
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        let parts: Vec<String> = s.document().parts.iter().filter(|p| !p.assembly).map(|p| p.name.clone()).collect();
        assert!(parts.len() == 1, "a sketch after the new part was taken back and put again made parts {parts:?}");
    }
}

probe! {
    /// REBUILDING EVERYTHING DOES NOT SPEAK OF IMPORTS THAT ARE NOT THERE: in a document of no imported bodies the
    /// quiet raising of their geometry finds none and says nothing - its "restored (0 bodies)" came late and wrote
    /// over the line of whatever the person did next.
    fn rebuilding_everything_says_nothing_of_imports_it_has_not() {
        let mut s = a_block();
        let (edit, rebuild) = (s.word("menu-edit"), s.word("menu-rebuild"));
        s.menu(&[&edit, &rebuild]);
        let status = s.status();
        let restored = s.word("io-brep-restored-n");
        let said = restored.split('(').next().unwrap_or_default().trim().to_string();
        assert!(!status.starts_with(&said), "Rebuild everything in a document of no imports said {status:?}");
    }
}

probe! {
    /// A BODY LEFT BY A NODE THAT NO LONGER BUILDS IS THE SAME WHICHEVER WAY IT WAS COME TO: its sketch deleted step by
    /// step, and the same document rebuilt from the start - the mesh stays on screen, and neither holds the live body
    /// of the extrusion that was. Step by step the old live body stayed (12 edges), rebuilt it was gone.
    fn a_body_left_by_a_red_node_is_the_same_rebuilt() {
        let mut s = a_block();
        let sketch = s.document().features.iter().find(|f| f.kind == "Sketch").map(|f| f.name.clone()).expect("the block has its sketch");
        let left = s.canvas().min.x;
        let row = s.words_at().into_iter().find(|(w, r)| r.max.x < left && w.starts_with(&sketch)).map(|(_, r)| r.center()).expect("the sketch row is in the tree");
        s.click(row).key(Key::Delete);
        let yes = s.word("confirm-yes");
        if let Some(r) = s.find(&yes, qymcad::pos2(640.0, 400.0)) {
            s.click(r.center());
        }
        let edges = |d: &Document| d.bodies.iter().filter(|b| !b.consumed).map(|b| (b.name.clone(), b.edges)).collect::<Vec<_>>();
        let step = edges(&s.document());
        let (edit, rebuild) = (s.word("menu-edit"), s.word("menu-rebuild"));
        s.menu(&[&edit, &rebuild]);
        let rebuilt = edges(&s.document());
        assert!(step == rebuilt, "the bodies after the sketch was deleted are {step:?} step by step and {rebuilt:?} rebuilt");
    }
}
