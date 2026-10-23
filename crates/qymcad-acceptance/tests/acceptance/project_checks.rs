//! A PROJECT OF REAL SIZE CHECKED THROUGH ITS OWN TIMELINE: every rounding and chamfer that says in the tree how many
//! edges it takes - "Fillet R2 (4 edges)" - takes them all. The timeline is rolled back to the node from the menu of
//! its row, so that what comes after it does not answer for it: the node must not stand yellow (the words of an edge
//! it left out), and, suppressed, the body of its part must change. Counting faces does not tell it: a rounding as
//! wide as the wall it runs into eats that wall, one face for one - R2 in a pocket 2 wide, four edges, faces +0 and
//! 4 x (4 - pi) x 4 = 13.735 mm^3 taken off, exactly what it should.
//!
//! Reported behaviour: roundings and chamfers stand green and are not drawn; three of four are there, one silently
//! not, and so in many places.
use std::time::Duration;

use qymcad::{Modifiers, PointerButton, Rect, Session};
use qymcad_acceptance::{build, probe};

/// The rows of the tree that begin with the word `key` gives, before its first value.
fn rows(s: &mut Session, key: &str) -> Vec<(String, Rect)> {
    // the arguments filled with a mark, so the words before the first of them are the head of every such row
    const MARK: &str = "\u{1}";
    let args = ["r", "size", "which"].map(|name| qymcad::Arg { name, value: MARK });
    let line = s.word_filled(key, &args);
    let head = line.split(MARK).next().unwrap_or(&line).to_string();
    let left = s.canvas().min.x;
    let mut found: Vec<(String, Rect)> = s.words_at().into_iter().filter(|(w, r)| w.starts_with(&head) && r.max.x < left).collect();
    found.sort_by(|a, b| a.1.min.y.total_cmp(&b.1.min.y));
    found
}

/// How many edges a row says its node takes - the number in its last brackets; `None` for "all edges".
fn edges_said(row: &str) -> Option<usize> {
    let inside = row.rsplit('(').next()?.trim_end_matches(')');
    inside.split_whitespace().next()?.parse().ok()
}

/// Press `key` in the menu of the row at `at`.
fn from_the_menu(s: &mut Session, at: Rect, key: &str) -> bool {
    s.click_with(at.center(), PointerButton::Secondary, Modifiers::default());
    let word = s.word(key);
    let item = s.widgets().into_iter().filter(|w| w.label.contains(&word) && w.enabled).min_by(|a, b| a.rect.center().distance(at.center()).total_cmp(&b.rect.center().distance(at.center())));
    match item {
        Some(w) => {
            s.click(w.rect.center());
            true
        }
        None => {
            s.key(qymcad::Key::Escape);
            false
        }
    }
}

/// The faces of the body of the part `part`.
fn faces_of(s: &mut Session, part: &str) -> Option<(usize, f64)> {
    s.document().bodies.iter().rfind(|b| b.part.as_deref() == Some(part) && !b.consumed && !b.sheet).map(|b| (b.faces, b.volume))
}

/// EVERY ROUNDING AND CHAMFER OF THE PROJECT AT `path` MADE A FACE FOR EACH EDGE IT SAYS IT TAKES: the problems, in
/// words.
fn every_rounding_takes_its_edges(path: &str) -> Vec<String> {
    let mut s = Session::start();
    s.budget(Duration::from_secs(600));
    build::open_project(&mut s, path);
    let doc = s.document();
    let mut parts: Vec<String> = doc.features.iter().filter(|f| f.kind == "Fillet" || f.kind == "Chamfer").filter_map(|f| f.part.clone()).collect();
    parts.sort();
    parts.dedup();
    let mut problems = Vec::new();
    let mut checked = 0;
    for part in &parts {
        // a part inside subassemblies is reached through them, from the top down
        let mut path = vec![part.clone()];
        while let Some(parent) = s.document().parts.iter().find(|p| p.name == *path.last().unwrap()).and_then(|p| p.parent.clone()) {
            path.push(parent);
        }
        path.reverse();
        let assembly = s.word("wb-assembly");
        s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
        let mut lost = None;
        for step in &path {
            let Some(row) = s.find(step, qymcad::pos2(0.0, 300.0)) else {
                lost = Some(step.clone());
                break;
            };
            std::thread::sleep(Duration::from_millis(700));
            s.double_click(row.center());
        }
        if let Some(step) = lost {
            problems.push(format!("the part {part:?} could not be reached: {step:?} is not in the tree on the way {path:?}"));
            continue;
        }
        if s.document().context != *part {
            problems.push(format!("the part {part:?} could not be stepped into"));
            continue;
        }
        for key in ["feat-fillet", "feat-chamfer"] {
            let count = rows(&mut s, key).len();
            for i in 0..count {
                let Some((label, at)) = rows(&mut s, key).get(i).cloned() else { break };
                if edges_said(&label).is_none() {
                    continue;
                }
                if !from_the_menu(&mut s, at, "act-rollback-here") {
                    continue;
                }
                let Some(before) = faces_of(&mut s, part) else { continue };
                // WHAT THE NODE LEFT OUT IT SAYS: an edge the kernel could not take is dropped with a warning in the
                // tree, so a node standing yellow is a node that did not take all it says
                let node = s.document().features.iter().find(|f| f.name == label && f.part.as_deref() == Some(part.as_str())).cloned();
                if let Some(w) = node.as_ref().and_then(|n| n.warning.clone()) {
                    problems.push(format!("{part:?} / {label:?}: stands yellow - {w}"));
                }
                let Some((_, at)) = rows(&mut s, key).get(i).cloned() else { break };
                if !from_the_menu(&mut s, at, "act-suppress") {
                    continue;
                }
                let after = faces_of(&mut s, part);
                checked += 1;
                match after {
                    // THE SAME BODY is the same volume AND the same faces: a chamfer of equal legs on four outer and four
                    // inner edges takes off exactly what it puts on - 20949.966 mm^3 either way, 93 faces against 85
                    Some(after) if (after.1 - before.1).abs() < 1e-6 && after.0 == before.0 => {
                        problems.push(format!("{part:?} / {label:?}: suppressed, the body is the same - the node did nothing"));
                    }
                    None => problems.push(format!("{part:?} / {label:?}: suppressed, the part holds no body")),
                    _ => {}
                }
                if let Some((_, at)) = rows(&mut s, key).get(i).cloned() {
                    from_the_menu(&mut s, at, "act-unsuppress");
                }
                if let Some((_, at)) = rows(&mut s, key).get(i).cloned() {
                    from_the_menu(&mut s, at, "act-clear-rollback");
                }
            }
        }
    }
    if checked == 0 && !parts.is_empty() {
        problems.push(format!("the roundings and chamfers of {parts:?} could not be checked: no row of them was found"));
    }
    problems
}

probe! {
    budget = 1800;
    /// THE ROUNDINGS AND CHAMFERS OF THE SAMPLE FILTER each made a face for every edge they say they take.
    fn every_rounding_of_the_filter_sample_takes_its_edges() {
        let p = every_rounding_takes_its_edges(&format!("{}/../../examples/Filter-v2.qcad", env!("CARGO_MANIFEST_DIR")));
        assert!(p.is_empty(), "{}", p.join("\n"));
    }
}

probe! {
    budget = 1800;
    /// THE ROUNDINGS AND CHAMFERS OF THE SAMPLE PC CASE each made a face for every edge they say they take.
    fn every_rounding_of_the_case_sample_takes_its_edges() {
        // the same steps have their round trips made on the filter sample; here they would cost hundreds of rebuilds
        qymcad_acceptance::oracles::no_round_trips_here();
        let p = every_rounding_takes_its_edges(&format!("{}/../../examples/QymBoxPc-Case.qcad", env!("CARGO_MANIFEST_DIR")));
        assert!(p.is_empty(), "{}", p.join("\n"));
    }
}

probe! {
    budget = 3000;
    /// THE VACUUM CLEANER SAMPLE - a cross-check: each of its roundings and chamfers made a face for every edge it
    /// says it takes.
    fn every_rounding_of_the_vacuum_cleaner_sample_takes_its_edges() {
        let Some(path) = qymcad_acceptance::private_sample("vacuumCleaner.qcad") else { return };
        let p = every_rounding_takes_its_edges(&path);
        assert!(p.is_empty(), "{}", p.join("\n"));
    }
}
