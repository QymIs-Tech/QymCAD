//! SEVERAL ROWS PICKED IN THE TREE ARE DELETED TOGETHER: parts of an assembly and sketches of a part, picked with Shift
//! from the first to the last, Delete and yes - every one of them goes, and one undo brings them all back.
//!
//! Reported behaviour: several parts picked in the tree with Shift, Delete removed the lowest of them alone; sketches could
//! not be picked several at once at all.
use qymcad::{Key, Modifiers, PointerButton, Pos2, Session};
use qymcad_acceptance::{build, probe};

/// The row of the tree that reads `name` - left of the canvas and below its top.
fn row(s: &mut Session, name: &str) -> Pos2 {
    let canvas = s.canvas();
    let rows: Vec<Pos2> = s.words_at().into_iter().filter(|(w, r)| r.max.x < canvas.min.x && r.min.y > canvas.min.y && r.max.y < canvas.max.y && w == name).map(|(_, r)| r.center()).collect();
    match rows.as_slice() {
        [r] => *r,
        other => panic!("the row {name:?} is not one row of the tree: {other:?}; on screen: {:?}", s.words()),
    }
}

/// Pick the rows `first` and `last` of the tree and the run between them with Shift, then Delete and yes.
fn delete_the_run(s: &mut Session, first: &str, last: &str) {
    let at = row(s, first);
    s.click(at);
    let at = row(s, last);
    s.click_with(at, PointerButton::Primary, Modifiers::SHIFT);
    s.key(Key::Delete);
    s.key(Key::Enter);
}

probe! {
    /// THREE PARTS OF AN ASSEMBLY picked from the first to the last with Shift are all deleted.
    fn three_parts_picked_in_the_tree_are_deleted_together() {
        let mut s = Session::start();
        for _ in 0..3 {
            build::a_part_in_the_assembly(&mut s);
        }
        let named: Vec<String> = s.document().parts.iter().filter(|p| !p.assembly).map(|p| p.name.clone()).collect();
        assert!(named.len() >= 3, "setup: the assembly holds the parts {named:?}");
        let (first, last) = (named[named.len() - 3].clone(), named[named.len() - 1].clone());
        delete_the_run(&mut s, &first, &last);
        let left: Vec<String> = s.document().parts.iter().filter(|p| !p.assembly).map(|p| p.name.clone()).collect();
        let kept: Vec<&String> = named[named.len() - 3..].iter().filter(|n| left.contains(n)).collect();
        assert!(kept.is_empty(), "the parts picked from {first} to {last} were not all deleted: {kept:?} left of {named:?}");
    }
}

probe! {
    /// THREE SKETCHES OF A PART picked from the first to the last with Shift are all deleted.
    fn three_sketches_picked_in_the_tree_are_deleted_together() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let finish = s.word("wb-finish");
        for k in 0..3 {
            if k > 0 {
                s.key(Key::K); // a new sketch: the planes to pick it on
            }
            build::rectangle_on_xy(&mut s);
            s.key(Key::Escape);
            s.press_word(&finish);
        }
        let named: Vec<String> = s.document().sketches.iter().map(|k| k.name.clone()).collect();
        assert_eq!(named.len(), 3, "setup: the part holds the sketches {named:?}");
        delete_the_run(&mut s, &named[0], &named[2]);
        let left: Vec<String> = s.document().sketches.iter().map(|k| k.name.clone()).collect();
        assert!(left.is_empty(), "the sketches picked from {} to {} were not all deleted: {left:?} left", named[0], named[2]);
        // one undo brings the whole group back
        s.chord(Modifiers::COMMAND, Key::Z);
        let back: Vec<String> = s.document().sketches.iter().map(|k| k.name.clone()).collect();
        assert_eq!(back, named, "one undo after the group delete gave back {back:?}, not the three sketches");
    }
}
