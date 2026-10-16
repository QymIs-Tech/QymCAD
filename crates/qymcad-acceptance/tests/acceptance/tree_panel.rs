//! THE TREE OF THE DOCUMENT: a row taken by a click, renamed by F2 and by its menu, a part carried into a
//! sub-assembly by a drag, a menu on every kind of row, the search, the folding of a branch, and the Delete key.
use qymcad::{Key, Modifiers, PointerButton, Rect, Session};
use qymcad_acceptance::{build, probe};

/// THE BLOCK OF THE FIRST PART: a sketch and a step of the timeline to take hold of in the tree.
fn a_block() -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    s.key(Key::Escape);
    s
}

/// The row of the tree that reads `text`.
fn row(s: &mut Session, text: &str) -> Rect {
    s.find(text, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("{text:?} is not in the tree; on screen: {:?}", s.words()))
}

/// The row of the tree whose words begin as the catalogue line `key` does before its first number.
fn row_like(s: &mut Session, key: &str) -> Rect {
    let line = s.word(key);
    let head = line.split(['{', '=', '\u{d7}']).next().unwrap_or(&line).trim().to_string();
    s.words_at()
        .into_iter()
        .filter(|(w, r)| w.starts_with(&head) && r.max.x < 500.0)
        .map(|(_, r)| r)
        .min_by(|a, b| a.min.y.total_cmp(&b.min.y))
        .unwrap_or_else(|| panic!("no row of the tree begins with {head:?}; on screen: {:?}", s.words()))
}

/// The name of the one sketch of the document.
fn sketch_name(s: &mut Session) -> String {
    s.document().sketches.first().map(|sk| sk.name.clone()).unwrap_or_else(|| panic!("the document holds no sketch"))
}

probe! {
    /// A ROW IS TAKEN BY A CLICK AND RENAMED BY F2: the name typed is the name in the document.
    fn a_row_is_taken_by_a_click_and_renamed_by_f2() {
        let mut s = a_block();
        let was = sketch_name(&mut s);
        let at = row(&mut s, &was);
        s.click(at.center());
        s.key(Key::F2);
        s.chord(Modifiers::COMMAND, Key::A).type_text("Contour").key(Key::Enter);
        assert!(sketch_name(&mut s) == "Contour", "F2 on the row of {was:?} and \"Contour\" typed, and the sketch is called {:?}", sketch_name(&mut s));
        assert!(s.shows("Contour"), "the sketch was renamed and the tree still reads {:?}; on screen: {:?}", was, s.words());
    }
}

probe! {
    /// THE MENU OF A ROW RENAMES IT TOO: the same by the right button.
    fn the_menu_of_a_row_renames_it() {
        let mut s = a_block();
        let at = row_like(&mut s, "feat-extrude");
        s.click_with(at.center(), PointerButton::Secondary, Modifiers::default());
        let rename = s.word("act-rename");
        s.press_word_near(&rename, at.center());
        s.chord(Modifiers::COMMAND, Key::A).type_text("The block").key(Key::Enter);
        let named = s.document().features.iter().any(|f| f.name == "The block");
        assert!(named, "the step was renamed to \"The block\" by its menu and the timeline holds {:?}", s.document().features.iter().map(|f| f.name.clone()).collect::<Vec<_>>());
    }
}

probe! {
    /// A PART IS CARRIED INTO A SUB-ASSEMBLY BY A DRAG of its row onto the row of the sub-assembly.
    fn a_part_is_carried_into_a_subassembly_by_a_drag() {
        let mut s = Session::start();
        build::a_part_in_the_assembly(&mut s);
        let part = s.document().parts.iter().find(|p| !p.assembly).map(|p| p.name.clone()).expect("the part made");
        let sub = s.word("tb-new-subassembly-hint");
        s.press_hint(&sub);
        let inner = s.document().parts.iter().filter(|p| p.assembly).map(|p| p.name.clone()).next_back().expect("the new sub-assembly");
        let assembly = s.word("wb-assembly");
        s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
        let (from, to) = (row(&mut s, &part).center(), row(&mut s, &inner).center());
        s.drag(from, to, PointerButton::Primary, Modifiers::default());
        let parent = s.document().parts.iter().find(|p| p.name == part).and_then(|p| p.parent.clone());
        assert!(parent.as_deref() == Some(inner.as_str()), "the part {part:?} was dragged onto {inner:?} and stands under {parent:?}");
    }
}

probe! {
    /// EVERY KIND OF ROW OFFERS A MENU: a sketch, a step of the timeline, a datum and a part each answer the right
    /// button with something to do.
    fn every_kind_of_row_offers_a_menu() {
        let mut s = a_block();
        let datum = s.word("g-datum-plane-hint");
        s.press_hint(&datum);
        let middle = s.in_space([0.0, 0.0, 0.0]);
        s.click(middle);
        s.key(Key::Enter);
        s.key(Key::Escape);
        let plane = s.document().datums.first().map(|d| d.name.clone()).expect("the datum plane");
        let sketch = sketch_name(&mut s);
        let part = s.document().parts.first().map(|p| p.name.clone()).expect("the first part");
        let rows: Vec<Rect> = vec![row(&mut s, &sketch), row_like(&mut s, "feat-extrude"), row(&mut s, &plane), row(&mut s, &part)];
        for (what, at) in [sketch.clone(), s.word("feat-extrude"), plane.clone(), part.clone()].into_iter().zip(rows) {
            s.click_with(at.center(), PointerButton::Secondary, Modifiers::default());
            let items: Vec<String> = s.widgets().into_iter().filter(|w| w.kind == qymcad::Kind::Button && w.rect.center().distance(at.center()) < 400.0 && !w.label.is_empty()).map(|w| w.label).collect();
            assert!(items.len() > 1, "the right button on the row of {what:?} offers {items:?}");
            s.key(Key::Escape);
        }
    }
}

probe! {
    /// THE SEARCH NARROWS THE TREE to the rows that answer it, and clearing it brings the rest back.
    fn the_search_narrows_the_tree() {
        let mut s = a_block();
        let step = s.word("feat-extrude");
        let head = step.split(['{', '=']).next().unwrap_or(&step).trim().to_string();
        assert!(s.words().iter().any(|w| w.starts_with(&head)), "the step is not in the tree to begin with; on screen: {:?}", s.words());
        let (search, sketch) = (s.word("tree-search"), sketch_name(&mut s));
        // the field says what it is for in grey inside itself, so it is clicked where those words stand
        let box_of_the_search = row(&mut s, &search);
        s.click(box_of_the_search.center());
        s.type_text(&sketch).key(Key::Enter);
        assert!(!s.words().iter().any(|w| w.starts_with(&head)), "the tree was searched for the sketch and the step is still in it; on screen: {:?}", s.words());
        assert!(s.shows(&sketch), "the tree was searched for the sketch and the sketch is not in it; on screen: {:?}", s.words());
        s.click(box_of_the_search.center()).chord(Modifiers::COMMAND, Key::A).key(Key::Backspace);
        assert!(s.words().iter().any(|w| w.starts_with(&head)), "the search was cleared and the step did not come back; on screen: {:?}", s.words());
    }
}

probe! {
    /// A BRANCH FOLDS AND UNFOLDS: the sketches are put away and brought back by their heading.
    fn a_branch_folds_and_unfolds() {
        let mut s = a_block();
        let sketch = sketch_name(&mut s);
        let heading = s.word("tree-sketches");
        s.press_word_near(&heading, qymcad::pos2(0.0, 300.0));
        assert!(!s.shows(&sketch), "the branch of the sketches was folded and {sketch:?} is still in the tree; on screen: {:?}", s.words());
        s.press_word_near(&heading, qymcad::pos2(0.0, 300.0));
        assert!(s.shows(&sketch), "the branch of the sketches was unfolded and {sketch:?} did not come back; on screen: {:?}", s.words());
    }
}

probe! {
    /// THE DELETE KEY REMOVES THE ROW THAT IS TAKEN, asking first. What leaned on it stays, red with the reason, unless
    /// the tick asks for it to go too.
    fn the_delete_key_removes_the_row_that_is_taken() {
        let mut s = a_block();
        let sketch = sketch_name(&mut s);
        let delete = |s: &mut qymcad::Session, with_dependents: bool| {
            let at = row(s, &sketch);
            s.click(at.center());
            s.key(Key::Delete);
            let yes = s.word("confirm-yes");
            assert!(s.shows(&yes), "Delete was pressed on the row of a sketch a body leans on and nothing asked first; on screen: {:?}", s.words());
            if with_dependents {
                let tick = s.word("confirm-with-dependents");
                s.press_word(&tick);
            }
            s.press_word(&yes);
            assert!(s.document().sketches.is_empty(), "the sketch was deleted and the document still holds {:?}", s.document().sketches.iter().map(|sk| sk.name.clone()).collect::<Vec<_>>());
        };
        delete(&mut s, false);
        let extrusion = s.document().features.iter().find(|f| f.kind == "Extrude").cloned();
        assert!(extrusion.as_ref().is_some_and(|f| f.error.as_deref().is_some_and(|e| !e.is_empty())), "the extrusion on the deleted sketch is not red with a reason: {extrusion:?}");
        s.chord(Modifiers::COMMAND, Key::Z);
        delete(&mut s, true);
        let left: Vec<String> = s.document().bodies.iter().filter(|b| !b.consumed).map(|b| b.name.clone()).collect();
        assert!(left.is_empty(), "the sketch was deleted with what is built on it and the part still holds {left:?}");
        assert!(qymcad_acceptance::oracles::whole(&s.document()).is_empty(), "after the deletion the document is not whole: {:?}", qymcad_acceptance::oracles::whole(&s.document()));
    }
}

probe! {
    /// A NAME TYPED AFTER F2 STANDS INSTEAD OF THE OLD ONE: a person pressing F2 and typing means the new name,
    /// not an addition to the old.
    fn a_name_typed_after_f2_stands_instead_of_the_old_one() {
        let mut s = a_block();
        let was = sketch_name(&mut s);
        let at = row(&mut s, &was);
        s.click(at.center());
        s.key(Key::F2);
        s.type_text("Contour").key(Key::Enter);
        assert!(sketch_name(&mut s) == "Contour", "F2 was pressed on {was:?} and \"Contour\" typed, and the sketch is called {:?}", sketch_name(&mut s));
    }
}

/// The brightest channel sum of the picture inside `r` - how light a word is drawn.
fn brightest(pic: &qymcad::Picture, r: Rect) -> u32 {
    let mut best = 0;
    for y in r.min.y as usize..r.max.y as usize {
        for x in r.min.x as usize..r.max.x as usize {
            let c = pic.colour_at(qymcad::pos2(x as f32, y as f32));
            best = best.max(c.iter().map(|v| *v as u32).sum());
        }
    }
    best
}

probe! {
    /// THE HEADING OF THE COMPONENTS IS AS LIGHT AS THE OTHER HEADINGS: in an assembly with a part, the word of the
    /// branch of the components is drawn no darker than Origin above it. Reported behaviour: the heading stood greyer
    /// than the rest of the tree.
    fn the_heading_of_the_components_is_as_light_as_the_others() {
        let mut s = Session::start();
        build::a_part_in_the_assembly(&mut s);
        let (origin, comps) = (s.word("tree-origin"), s.word("tree-components"));
        let canvas = s.canvas();
        let in_tree = |w: &str, r: &Rect, want: &str| w.ends_with(want) && r.max.x < canvas.min.x && r.min.y > canvas.min.y;
        let words = s.words_at();
        let o = words.iter().find(|(w, r)| in_tree(w, r, &origin)).map(|(_, r)| *r).expect("the Origin row");
        let c = words.iter().find(|(w, r)| in_tree(w, r, &comps)).map(|(_, r)| *r).expect("the Components heading");
        let pic = s.snapshot();
        let (lo, lc) = (brightest(&pic, o), brightest(&pic, c));
        assert!(lc + 15 >= lo, "the heading of the components is drawn at {lc}, Origin at {lo}");
    }
}
