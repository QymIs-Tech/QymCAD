//! "FINISH" CLIMBS THE TREE, WHATEVER LINKS LED DOWN IT. Reported behaviour: the links of the status line, of the window
//! of faults and of the window of parameters lead into a part; after some twenty of them "Finish" above the canvas walked
//! back along the jumps, about thirty presses to the root assembly.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::model::Id;

/// A PART NAMED `name` IN THE CONTEXT IN HAND: a 60 x 40 plate extruded 12 by hand, its height named `<name>_h` in the
/// parameters, and a rounding of it that lost one of its two edges - a yellow fault that leads to the part.
fn part_with_a_driver_and_a_fault(app: &mut App, name: &str) -> Target {
    let driver = format!("{}_h", name.to_lowercase());
    let part = app.project.add_part(name);
    app.enter_component(part);
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    Hand::new(app).sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(60.0, 40.0);
    app.finish_sketch_edit();
    app.chosen.sel = qymcad_ui_state::Sel::Sketch(si);
    Hand::new(app).look_at([30.0, 20.0, 6.0], 8.0).tool(qymcad_ui_state::PartTool::Extrude).set("height", 12.0).enter();
    let extrude = app.project.timeline.last().map(|n| n.id).expect("the extrusion");
    assert!(app.project.add_named_feat_dim(driver.clone(), extrude, "height"), "setup: the height of {name} is named {driver}");
    let block = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the plate");
    let real = app.project.regen_edges.get(&block).and_then(|es| es.first().map(|e| e.id)).expect("an edge of the plate");
    // the second edge was never an edge of the plate: the rounding builds the first and warns of the second
    app.project.add_fillet_ref(block, 1.0, qymcad_core::refs::Ref::picks(&[real, 0x6000_7777]));
    let fault = app.project.timeline.last().map(|n| n.id).expect("the rounding");
    Target { part, driver, fault }
}

/// THE THREE DOORS a person leaves by for another part.
#[derive(Clone, Copy, Debug)]
enum Door {
    /// The path of a driver in the window of parameters.
    Driver,
    /// The counter of the status line, then the row in the window of faults it opens.
    Counter,
    /// The words of the status line, then the row in the window of faults.
    Words,
}

/// A PART THE LINKS LEAD TO: the part, its driver and the node of its fault.
struct Target {
    part: Id,
    driver: String,
    fault: Id,
}

/// PUT THE TWO WINDOWS AWAY with their crosses, as a person does before the next link: they open at the same corner,
/// and one lying over the other takes the click meant for it. Answers what stayed open.
fn put_away(hand: &mut Hand) -> Option<String> {
    shut(hand, crate::gui::WinKind::Faults, "win-faults").or_else(|| shut(hand, crate::gui::WinKind::Params, "win-params"))
}

/// Close the window `kind` titled by the catalogue key `title` with its cross; a window under another takes the second
/// press. Answers what stayed open.
fn shut(hand: &mut Hand, kind: crate::gui::WinKind, title: &str) -> Option<String> {
    let title = crate::i18n::tr(title);
    for _ in 0..2 {
        if hand.app.win.is(kind) {
            hand.close_titled(&title);
        }
    }
    hand.app.win.is(kind).then(|| format!("the window {title:?} does not close by its cross"))
}

/// Go to `to` through `door`, as a person does. Answers what did not work.
fn go(hand: &mut Hand, to: &Target, door: Door) -> Option<String> {
    if let Some(why) = put_away(hand) {
        return Some(why);
    }
    match door {
        Door::Driver => {
            if !hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)) {
                return Some("the button of the parameters is not on screen".into());
            }
            let Some(k) = hand.app.project.named_dims.iter().position(|d| d.name == to.driver) else { return Some(format!("no driver {}", to.driver)) };
            let path = qymcad_ui_state::path_in_words(&hand.app.project.driver_path(&hand.app.project.named_dims[k].target));
            hand.frame(Vec::new());
            let Some(window) = hand.window_at("win_params") else { return Some("the button opened no window of parameters".into()) };
            if !hand.press_word(&path, window.center()) {
                return Some(format!("the window of parameters has no path {path:?}"));
            }
        }
        Door::Counter | Door::Words => {
            let line = hand.status_painted();
            let badge = hand.status_badge(qymcad_ui_state::Tone::Warning);
            let at = match door {
                Door::Counter => badge.map(|b| b.rect.center()),
                _ => line.iter().filter(|w| badge.as_ref().is_none_or(|b| b.rect != w.rect)).max_by_key(|w| w.text.chars().count()).map(|w| w.rect.center()),
            };
            let Some(at) = at else { return Some(format!("{door:?}: nothing on the status line to click")) };
            hand.click_screen(at);
            hand.frame(Vec::new());
            let Some(window) = hand.window_at("win_faults").filter(|_| hand.app.win.is(crate::gui::WinKind::Faults)) else {
                return Some(format!("{door:?}: the click opened no window of faults"));
            };
            let path = qymcad_ui_state::path_in_words(&hand.app.project.feature_breadcrumbs(to.fault));
            if !hand.press_word(&path, window.center()) {
                return Some(format!("the window of faults has no row {path:?}"));
            }
        }
    }
    None
}

/// TWENTY JUMPS BY THE THREE DOORS among parts at three depths: after each the path above the canvas is the place of the
/// part in the tree, and "Finish" pressed as many times as the last part is deep comes to the root.
#[test]
fn finish_climbs_the_tree_after_jumps_by_links() {
    let mut app = App::default();
    app.project.new_document();
    let root = app.project.root;
    // root -> A -> B -> P, root -> A -> Q, root -> R
    let a = app.project.add_assembly("A");
    app.enter_component(a);
    let b = app.project.add_assembly("B");
    app.enter_component(b);
    let p = part_with_a_driver_and_a_fault(&mut app, "P");
    app.set_context_to(a);
    let q = part_with_a_driver_and_a_fault(&mut app, "Q");
    app.set_context_to(root);
    let r = part_with_a_driver_and_a_fault(&mut app, "R");
    app.set_context_to(root);
    let targets = [p, r, q];

    let mut problems: Vec<String> = Vec::new();
    let mut hand = Hand::new(&mut app);
    hand.settle();
    let warned = hand.app.project.regen_warnings.len();
    assert_eq!(warned, 3, "setup: each rounding warns: {:?}", hand.app.project.regen_warnings);
    let mut last = root;
    for i in 0..20 {
        let to = &targets[(i + i / 3) % 3];
        let door = [Door::Driver, Door::Counter, Door::Words][i % 3];
        if let Some(why) = go(&mut hand, to, door) {
            problems.push(format!("jump {i} by {door:?}: {why}"));
            continue;
        }
        let tree = qymcad_ui_state::context_path_to(&hand.app.project, to.part);
        if hand.app.active_path != tree {
            problems.push(format!("jump {i} by {door:?}: the path is {:?}, the place of the part in the tree is {tree:?}", hand.app.active_path));
        }
        last = to.part;
    }
    let depth = qymcad_ui_state::context_path_to(&hand.app.project, last).len() - 1;
    let finish = crate::i18n::tr("wb-finish");
    if let Some(why) = put_away(&mut hand) {
        problems.push(why);
    }
    for press in 0..depth {
        if !hand.press_word(&finish, egui::pos2(0.0, 0.0)) {
            problems.push(format!("press {press} of {depth}: \"Finish\" is not on screen"));
            break;
        }
    }
    hand.frame(Vec::new());
    if hand.app.active_path != vec![root] {
        problems.push(format!("{depth} presses of \"Finish\" from a part {depth} deep left the path {:?}, not the root", hand.app.active_path));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
