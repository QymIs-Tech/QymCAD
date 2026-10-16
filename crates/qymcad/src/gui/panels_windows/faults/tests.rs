//! THE FAULTS OF A DOCUMENT ARE SAID, COUNTED AND LED TO - read off the screen of whole frames.
use crate::gui::hand::{Hand, PaintedWords, Typed};
use crate::gui::{App, Sel};
use egui_phosphor::regular as ph;
use qymcad_core::model::Id;

/// The badge of a tone in the status line: its icon and a number.
fn badge(words: &[PaintedWords], icon: &str) -> Option<usize> {
    words.iter().find_map(|w| w.text.strip_prefix(icon).and_then(|n| n.trim().parse().ok()))
}

/// The message of the status line: the longest run along the bottom.
fn message(words: &[PaintedWords]) -> Option<&PaintedWords> {
    words.iter().max_by_key(|w| w.text.chars().count())
}

/// A HOUSING 60 x 40 AS TALL AS THE PARAMETER `H` = 25, and two round cuts 10 deep in its side face at the height
/// of 12: the document of the reported case. Returns the nodes of the two cuts.
fn housing_with_two_cuts(app: &mut App) -> Vec<Id> {
    app.project.parameters.push(qymcad_core::model::Param { name: "H".into(), expr: "25".into(), value: 25.0 });
    app.project.eval_parameters();
    let part = app.project.add_part("Housing");
    app.enter_component(part);
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    Hand::new(app).sk_tool(2).click2d(0.0, 0.0).click2d(60.0, 40.0);
    app.finish_sketch_edit();
    app.chosen.sel = Sel::Sketch(si);
    Hand::new(app).look_at([30.0, 20.0, 10.0], 8.0).tool(1).set("height", 25.0).formula(Typed { field: "height", text: "H" }).enter();
    let mut cuts = Vec::new();
    for x in [20.0, 40.0] {
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        let side = app.project.regen_faces[&body].iter().filter(|f| f.normal[1] < -0.9).max_by(|a, b| a.area.total_cmp(&b.area)).cloned().expect("the side face");
        let key = qymcad_core::feature::FaceKey { index: 0, centroid: [side.centroid.x, side.centroid.y, side.centroid.z], normal: side.normal, id: side.id };
        let sk = app.create_sketch_on(qymcad_core::feature::SketchPlane::Face(body, key));
        Hand::new(app).sk_tool(3).click2d(x, 12.0).click2d(x + 4.0, 12.0);
        app.finish_sketch_edit();
        app.chosen.sel = Sel::Sketch(sk);
        Hand::new(app).look_at([30.0, 20.0, 12.0], 8.0).tool(1).op(2).set("height", 10.0).enter();
        cuts.push(app.project.timeline.last().map(|n| n.id).expect("the cut"));
    }
    assert!(app.project.regen_errors.is_empty(), "setup: the housing and its cuts build: {:?}", app.project.regen_errors);
    cuts
}

/// THE REPORTED CASE, BY THE PERSON'S PATH: the height of the housing is typed in the window of parameters, both cuts
/// are left above the housing and go red. The status line says so in red, the counter in the status line says how
/// many, a click on it opens the window of faults with each cut and its path, a click on a row selects that cut in
/// its part. The height put back, the badges go and the line is not left red.
///
/// Reported behaviour: after a rebuild the cut and everything after it went red, and the status line said so in the
/// plain colour of every other message.
#[test]
fn a_cut_gone_red_is_said_in_red_counted_and_led_to() {
    let mut app = App::default();
    let cuts = housing_with_two_cuts(&mut app);
    let pal = app.scheme.pal.clone();
    let mut hand = Hand::new(&mut app);
    let before = hand.status_painted();
    assert_eq!(badge(&before, ph::X_CIRCLE), None, "a healthy document shows an error badge: {:?}", before.iter().map(|w| &w.text).collect::<Vec<_>>());

    // the height typed in the window of parameters, as a person types it
    assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");
    assert!(hand.type_after_word("H", "5"), "the formula of H is not on screen");
    hand.key(egui::Key::Enter).settle();
    let red: Vec<Id> = cuts.iter().copied().filter(|c| hand.app.project.regen_errors.contains_key(c)).collect();
    assert_eq!(red.len(), 2, "setup: both cuts above a housing 5 tall go red: {:?}", hand.app.project.regen_errors);

    let after = hand.status_painted();
    let line = message(&after).expect("the status line says something");
    assert_eq!(line.colour, pal.error(), "the line after a rebuild that left red nodes is not red: {:?}", line.text);
    assert!(line.text.starts_with(ph::X_CIRCLE), "the line carries no error icon: {:?}", line.text);
    let n = hand.app.project.regen_errors.len();
    assert_eq!(badge(&after, ph::X_CIRCLE), Some(n), "the counter does not say the {n} red nodes: {:?}", after.iter().map(|w| &w.text).collect::<Vec<_>>());
    let counter = after.iter().find(|w| w.text.starts_with(ph::X_CIRCLE) && w.text.chars().count() < 8).expect("the error badge");
    assert_eq!(counter.colour, pal.error(), "the error badge is not red");

    // several faults: the click opens the window, a row for each cut with its path
    hand.click_screen(counter.rect.center());
    let title = crate::i18n::tr("win-faults");
    let window =
        hand.window_titled(&title).unwrap_or_else(|| panic!("a click on the counter opened no window of faults; open {:?}, at {:?}", hand.app.win.is(crate::gui::WinKind::Faults), counter.rect));
    let rows = hand.painted_words();
    let in_window: Vec<&PaintedWords> = rows.iter().filter(|w| window.contains(w.rect.center())).collect();
    for c in &red {
        let path = qymcad_ui_state::path_in_words(&hand.app.project.feature_breadcrumbs(*c));
        assert!(in_window.iter().any(|w| w.text == path), "the window has no row with the path {path:?}: {:?}", in_window.iter().map(|w| &w.text).collect::<Vec<_>>());
    }
    // a click on the row of the first cut selects that cut
    let first = qymcad_ui_state::path_in_words(&hand.app.project.feature_breadcrumbs(red[0]));
    let row = in_window.iter().find(|w| w.text == first).map(|w| w.rect.center()).expect("the row of the first cut");
    hand.click_screen(row);
    let ti = hand.app.project.timeline.iter().position(|n| n.id == red[0]);
    assert!(ti.is_some_and(|ti| hand.app.chosen.sel == Sel::Feature(ti)), "a click on the row of the cut did not select it");

    // the height put back: the badges go, and the line is not left red
    assert!(hand.type_after_word("H", "25"), "the formula of H is not on screen");
    hand.key(egui::Key::Enter).settle();
    let healed = hand.status_painted();
    assert!(hand.app.project.regen_errors.is_empty(), "setup: both cuts build again: {:?}", hand.app.project.regen_errors);
    assert_eq!(badge(&healed, ph::X_CIRCLE), None, "the error badge stayed after the faults were fixed");
    assert!(message(&healed).is_none_or(|w| w.colour != pal.error()), "the line stayed red after the faults were fixed");
}

/// The node of the timeline that makes `id`, or is `id`.
fn node_index(app: &App, id: Id) -> Option<usize> {
    app.project.timeline.iter().position(|n| n.id == id || n.kind.body() == Some(id))
}

/// A ROUNDING THAT LEFT OUT ONE OF ITS EDGES built the rest: the line says so in yellow with the warning's icon, a
/// yellow badge counts it beside no red one, and a click on the badge - one fault - goes straight to the rounding,
/// with no window to choose from.
#[test]
fn a_rounding_that_left_out_an_edge_is_said_in_yellow_and_led_to() {
    let mut app = crate::gui::screen_keys::tests::plate();
    let pal = app.scheme.pal.clone();
    let block = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the plate");
    let real = app.project.regen_edges.get(&block).and_then(|es| es.first().map(|e| e.id)).expect("an edge of the plate");
    // the rounding of a document whose second edge is gone: no edge of the plate has carried that name
    let fillet = app.project.add_fillet_ref(block, 1.0, qymcad_core::refs::Ref::picks(&[real, 0x6000_7777]));
    let mut hand = Hand::new(&mut app);
    hand.settle();
    let ti = node_index(hand.app, fillet).expect("the rounding");
    let node = hand.app.project.timeline[ti].id;
    assert!(
        hand.app.project.regen_warnings.contains_key(&node) && hand.app.project.regen_errors.is_empty(),
        "setup: the rounding warns and nothing is red: {:?} {:?}",
        hand.app.project.regen_warnings,
        hand.app.project.regen_errors
    );

    let words = hand.status_painted();
    let line = message(&words).expect("the status line says something");
    assert_eq!(line.colour, pal.warning(), "the line after a rebuild that left a warning is not yellow: {:?}", line.text);
    assert!(line.text.starts_with(ph::WARNING), "the line carries no warning icon: {:?}", line.text);
    assert_eq!(badge(&words, ph::X_CIRCLE), None, "a warning is counted as an error");
    assert_eq!(badge(&words, ph::WARNING), Some(1), "the warning is not counted: {:?}", words.iter().map(|w| &w.text).collect::<Vec<_>>());
    let counter = words.iter().find(|w| w.text.starts_with(ph::WARNING) && w.text.chars().count() < 8).expect("the warning badge");
    assert_eq!(counter.colour, pal.warning(), "the warning badge is not yellow");

    hand.click_screen(counter.rect.center());
    assert_eq!(hand.window_titled(&crate::i18n::tr("win-faults")), None, "one fault opened a window to choose from instead of going there");
    assert!(hand.app.chosen.sel == Sel::Feature(ti), "a click on the badge of the one warning did not select the rounding");
}

/// EVERY KIND OF FAULT HAS ITS ROW AND ITS ROAD: a mate whose connector is gone, a sketch whose constraints conflict,
/// a parameter whose formula gives no value. The counter says three; the window lists each with its path; a click on
/// the mate selects it in its assembly, on the sketch selects the sketch in its part, on the parameter opens the table
/// of parameters narrowed to it.
#[test]
fn every_kind_of_fault_has_its_row_and_its_road() {
    let mut app = App::default();
    crate::gui::joint_flow::tests::add_part_at(&mut app, 0.0);
    // a line held at both ends 20 apart and dimensioned 10: the sketch cannot hold its constraints
    let part_a = qymcad_ui_state::current_ctx_id(&app.active_path, &app.project);
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.project.add_line_entity(si, 0.0, 30.0, 20.0, 30.0, qymcad_core::feature::Purpose::Real);
    let (a, b) = (app.project.sketches[si].points[0].id, app.project.sketches[si].points[1].id);
    let s = &mut app.project.sketches[si];
    s.constraints.extend([
        qymcad_core::model::Constraint::Fixed { p: a },
        qymcad_core::model::Constraint::Fixed { p: b },
        qymcad_core::model::Constraint::Distance { a, b, d: 10.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None },
    ]);
    app.finish_sketch_edit();
    crate::gui::joint_flow::tests::add_part_at(&mut app, 60.0);
    app.exit_context();
    let one = app.project.mesh_id(0).and_then(|b| app.project.body_owner(b)).expect("part A");
    let two = app.project.mesh_id(1).and_then(|b| app.project.body_owner(b)).expect("part B");
    app.project.set_grounded(one, true);
    let ca = app.project.add_connector(one, qymcad_core::feature::AnchorRef::Origin);
    let cb = app.project.add_connector(two, qymcad_core::feature::AnchorRef::Origin);
    let joint = app.project.add_joint(ca, cb, qymcad_core::feature::JointKind::Rigid);
    app.project.connectors.retain(|c| c.id != cb); // the connector of B is gone from the document
    app.project.parameters.push(qymcad_core::model::Param { name: "w".into(), expr: "60 +".into(), value: 0.0 });
    let sid = app.project.sketches[si].id;
    let mut hand = Hand::new(&mut app);
    hand.settle();
    assert!(!hand.app.project.sketch_conflicts(si).is_empty(), "setup: the sketch conflicts");

    let words = hand.status_painted();
    assert_eq!(badge(&words, ph::X_CIRCLE), Some(3), "the counter does not say the three faults: {:?}", words.iter().map(|w| &w.text).collect::<Vec<_>>());
    let counter = words.iter().find(|w| w.text.starts_with(ph::X_CIRCLE) && w.text.chars().count() < 8).map(|w| w.rect.center()).expect("the error badge");
    hand.click_screen(counter);
    let window = hand.window_titled(&crate::i18n::tr("win-faults")).expect("the window of faults");
    let mate_name = hand.app.project.joints.iter().find(|j| j.id == joint).map(|j| j.name.clone()).unwrap_or_default();
    let mate_path = if mate_name.is_empty() { crate::i18n::tr(qymcad_core::feature::JointKind::Rigid.label()) } else { qymcad_ui_state::path_in_words(&mate_name) };
    let sketch_path = qymcad_ui_state::path_in_words(&hand.app.project.sketch_breadcrumbs(sid));
    let row = |hand: &mut Hand, path: &str| -> egui::Pos2 {
        let rows = hand.painted_words();
        rows.iter()
            .find(|w| window.contains(w.rect.center()) && w.text == path)
            .map(|w| w.rect.center())
            .unwrap_or_else(|| panic!("the window has no row with the path {path:?}: {:?}", rows.iter().filter(|w| window.contains(w.rect.center())).map(|w| &w.text).collect::<Vec<_>>()))
    };

    let at = row(&mut hand, &mate_path);
    hand.click_screen(at);
    assert!(hand.app.chosen.sel == Sel::Joint(joint), "a click on the row of the mate did not select the mate");

    let at = row(&mut hand, &sketch_path);
    hand.click_screen(at);
    assert!(hand.app.chosen.sel == Sel::Sketch(si), "a click on the row of the sketch did not select the sketch");
    assert_eq!(qymcad_ui_state::current_ctx_id(&hand.app.active_path, &hand.app.project), part_a, "the click did not step into the part of the sketch");

    let at = row(&mut hand, "w");
    hand.click_screen(at);
    let table = format!("{} {}", ph::FUNCTION, crate::i18n::tr("win-params"));
    assert!(hand.window_titled(&table).is_some() || hand.window_titled(&crate::i18n::tr("win-params")).is_some(), "a click on the row of the parameter did not open the table of parameters");
    assert_eq!(hand.app.par_search, "w", "the table is not narrowed to the parameter");
}

/// AN ASSEMBLY REBUILT WITH A MATE THAT CANNOT HOLD ends the rebuild with a red line naming the mate - "Edit -> Rebuild
/// everything", the menu a person reaches for when something looks off.
#[test]
fn a_rebuild_that_leaves_a_mate_not_holding_says_so_in_red() {
    let mut app = App::default();
    crate::gui::joint_flow::tests::add_part_at(&mut app, 0.0);
    crate::gui::joint_flow::tests::add_part_at(&mut app, 60.0);
    app.exit_context();
    let one = app.project.mesh_id(0).and_then(|b| app.project.body_owner(b)).expect("part A");
    let two = app.project.mesh_id(1).and_then(|b| app.project.body_owner(b)).expect("part B");
    app.project.set_grounded(one, true);
    let ca = app.project.add_connector(one, qymcad_core::feature::AnchorRef::Origin);
    let cb = app.project.add_connector(two, qymcad_core::feature::AnchorRef::Origin);
    app.project.add_joint(ca, cb, qymcad_core::feature::JointKind::Rigid);
    app.project.connectors.retain(|c| c.id != cb);
    let pal = app.scheme.pal.clone();
    let mut hand = Hand::new(&mut app);
    hand.settle();
    assert!(hand.press_word(&crate::i18n::tr("menu-edit"), egui::pos2(0.0, 0.0)), "the menu Edit is not on screen");
    assert!(hand.press_word(&crate::i18n::tr("menu-rebuild"), egui::pos2(0.0, 0.0)), "Rebuild everything is not in the menu");
    hand.settle();
    let words = hand.status_painted();
    let line = message(&words).expect("the status line says something");
    // the words before the place of the mate, in the language of the interface
    let head = crate::i18n::tr2("io-placement-left-errors", "n", "1", "where", "@").split('@').next().unwrap_or_default().to_string();
    assert!(line.text.contains(&head), "the line after the rebuild does not name the mate that does not hold ({head:?}): {:?}", line.text);
    assert_eq!(line.colour, pal.error(), "the line naming a mate that does not hold is not red: {:?}", line.text);
}

/// A FORMULA TYPED IN THE TABLE THAT GIVES NO VALUE is said in red on the status line, with the parameter's name.
#[test]
fn a_formula_without_a_value_is_said_in_red_on_the_line() {
    let mut app = App::default();
    app.project.parameters.push(qymcad_core::model::Param { name: "w".into(), expr: "10".into(), value: 10.0 });
    app.project.eval_parameters();
    let pal = app.scheme.pal.clone();
    let mut hand = Hand::new(&mut app);
    assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");
    assert!(hand.type_after_word("w", "10 +"), "the formula of w is not on screen");
    hand.key(egui::Key::Enter).settle();
    let words = hand.status_painted();
    let line = message(&words).expect("the status line says something");
    assert!(line.text.starts_with(ph::X_CIRCLE) && line.text.contains('w'), "the line does not say that w has no value: {:?}", line.text);
    assert_eq!(line.colour, pal.error(), "the line about a formula without a value is not red: {:?}", line.text);
    assert_eq!(hand.status_badge(qymcad_ui_state::Tone::Error).map(|b| b.text), Some(format!("{} 1", ph::X_CIRCLE)), "the formula without a value is not counted");
}

/// A FAULT AT THE BOTTOM OF A LONG TREE IS SHOWN THERE: the click on the counter selects the node and its row comes
/// into the view of the tree - selected out of sight is not "led to".
#[test]
fn a_fault_led_to_is_shown_in_the_tree() {
    let mut app = crate::gui::screen_keys::tests::plate();
    let block = app.project.timeline.iter().find_map(|n| n.kind.body()).expect("the plate");
    let part = app.project.body_owner(block).expect("the part of the plate");
    // fifty sketches in the part of the plate, then the rounding under them: the part's tree is longer than the window
    for _ in 0..50 {
        app.enter_component(part);
        app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.finish_sketch_edit();
    }
    app.enter_component(part);
    app.project.add_fillet_ref(block, 1.0, qymcad_core::refs::Ref::picks(&[0x6000_7777]));
    app.chosen.sel = Sel::None;
    let mut hand = Hand::new(&mut app);
    hand.settle();
    // the row of the red node in the tree on the left: it starts with the error's sign; a row out of the view is not drawn
    let row = |hand: &mut Hand| hand.painted_words().into_iter().find(|w| w.rect.min.x < 370.0 && w.rect.max.y < 860.0 && w.text.starts_with(ph::X_CIRCLE)).map(|w| w.rect);
    assert_eq!(row(&mut hand), None, "setup: the row of the rounding is in view before the click");
    let badge = hand.status_badge(qymcad_ui_state::Tone::Error).expect("the red badge of the rounding");
    hand.click_screen(badge.rect.center());
    // the tree scrolls there smoothly, in a third of a second at most: the hand looks once it has come to rest
    for _ in 0..30 {
        hand.frame(Vec::new());
    }
    let shown = row(&mut hand).expect("the rounding led to stays out of the view of the tree");
    assert!(shown.min.y > 0.0, "the row of the rounding stands above the window, at {shown:?}");
}

/// A DRIVER WHOSE DIMENSION IS GONE is said where the click lands and fixed where it is kept: a click on its yellow
/// badge says on the status line what is wrong and where, and opens the table of parameters narrowed to the driver -
/// where its row says the dimension was not found, in yellow, beside the bin that takes it away.
///
/// Reported behaviour: the warning badge lit, a click on it went into a sketch of the part and said nothing, and not
/// one node of the tree was yellow.
#[test]
fn a_driver_without_its_dimension_is_said_and_led_to_the_table() {
    let mut app = crate::gui::screen_keys::tests::plate();
    let sid = app.project.sketches[0].id;
    // the driver names a dimension of the sketch the sketch no longer holds
    app.project.named_dims.push(qymcad_core::model::NamedDim { name: "T_out".into(), target: qymcad_core::model::DimTarget::Sketch { sketch: sid, refs: vec![0x6000_7777] } });
    let pal = app.scheme.pal.clone();
    let mut hand = Hand::new(&mut app);
    hand.settle();
    let badge = hand.status_badge(qymcad_ui_state::Tone::Warning).expect("the yellow badge of the driver");
    hand.click_screen(badge.rect.center());
    let words = hand.status_painted();
    let line = message(&words).expect("the status line says something");
    assert!(line.text.contains("T_out") && line.colour == pal.warning(), "a click on the badge did not say in yellow what is wrong: {:?}", line.text);
    assert!(hand.window_titled(&crate::i18n::tr("win-params")).is_some(), "the click did not open the table of parameters");
    assert_eq!(hand.app.par_search, "T_out", "the table is not narrowed to the driver");
    let missing = crate::i18n::tr1("par-dim-missing", "name", "T_out");
    let row = hand.painted_words().into_iter().find(|w| w.text.contains(&missing) && w.rect.max.y < 860.0);
    assert!(row.as_ref().is_some_and(|w| w.colour == pal.warning()), "the row of the driver in the table does not say in yellow that its dimension is gone: {:?}", row.map(|w| (w.text, w.colour)));
}
