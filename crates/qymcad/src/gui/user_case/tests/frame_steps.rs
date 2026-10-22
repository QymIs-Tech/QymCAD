use super::*;

/// The width and the height of the rectangle with side `side` of sketch `si`, as it stands.
struct Sized {
    width: f64,
    height: f64,
}

fn sized(app: &App, si: usize, side: u64) -> Option<Sized> {
    let c = app.project.rect_corners(si, side)?;
    Some(Sized { width: c[0].dist(c[1]), height: c[1].dist(c[2]) })
}

/// The parameter `name` given `value`, the formulas over it worked out, as the table of parameters writes it.
fn parameter(app: &mut App, name: &str, value: f64) {
    let expr = value.to_string();
    match app.project.parameters.iter_mut().find(|p| p.name == name) {
        Some(p) => p.expr = expr,
        None => app.project.parameters.push(qymcad_core::model::Param { name: name.into(), expr, value }),
    }
    app.project.eval_parameters();
}

/// ONE GESTURE IN A TOOL OF EVERY WORKBENCH, the frame of a tool being one: the box at the geometry takes a formula over
/// the parameter `fw`, Tab goes on to the next field, Enter keeps it, Esc puts the tool down, a double click opens the
/// box again with the formula in it, the key of a tool pressed again puts it down - in a rectangle of a sketch, in the
/// extrusion of a part and in the pattern of a part in the assembly - and `fw` changed, what was given it follows.
pub(super) fn one_gesture_in_every_workbench(app: &mut App, problems: &mut Vec<String>) {
    let step = "the frame of a tool";
    app.exit_context();
    let part = app.project.add_part("Frame");
    app.enter_component(part);
    parameter(app, "fw", 30.0);
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.chosen.sel = crate::gui::Sel::Sketch(si);

    // A SKETCH: the width a formula, Tab, the height a number, Enter
    let mut hand = Hand::new(app);
    hand.sk_tool(SketchShape::Rect).click2d(0.0, 0.0).click2d(40.0, 20.0);
    if !hand.type_after_word(&crate::i18n::tr("sk-width"), "fw") {
        problems.push(format!("[{step}] the rectangle drawn opened no box with its width"));
    }
    hand.key(egui::Key::Tab).type_text("15").key(egui::Key::Enter);
    hand.close_window();
    let side = app.project.sketches[si].rects().first().map(|r| r.sides[0]);
    let Some(side) = side else {
        problems.push(format!("[{step}] the rectangle was not drawn"));
        return;
    };
    match sized(app, si, side) {
        Some(Sized { width, height }) if (width - 30.0).abs() < 1e-6 && (height - 15.0).abs() < 1e-6 => {}
        other => problems.push(format!("[{step}] the width given fw = 30 and, after Tab, the height 15: the rectangle is {:?}", other.map(|s| [s.width, s.height]))),
    }
    check_all(app, &format!("{step}: a sketch, its box"), problems);
    // Esc puts the drawing tool down
    let mut hand = Hand::new(app);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.close_window();
    if app.tools.armed != qymcad_ui_state::Armed::None {
        problems.push(format!("[{step}] Esc left {:?} in hand in the sketch", app.tools.armed));
    }
    // a double click on a side opens the sizes again, the width with its formula
    let mut hand = Hand::new(app);
    hand.double_click2d(20.0, 0.0);
    let formula = hand.shows("fw");
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.close_window();
    if !formula {
        problems.push(format!("[{step}] the rectangle opened again by a double click does not show its width as fw"));
    }
    // the key of the dimension pressed again puts it down
    let mut hand = Hand::new(app);
    hand.key(egui::Key::D).key(egui::Key::D);
    hand.close_window();
    if app.tools.armed != qymcad_ui_state::Armed::None {
        problems.push(format!("[{step}] D pressed twice left {:?} in hand", app.tools.armed));
    }
    parameter(app, "fw", 50.0);
    app.project.solve_sketch(si);
    if !sized(app, si, side).is_some_and(|s| (s.width - 50.0).abs() < 1e-6) {
        problems.push(format!("[{step}] fw changed to 50: the width of the rectangle is {:?}", sized(app, si, side).map(|s| s.width)));
    }
    check_all(app, &format!("{step}: a sketch, opened again"), problems);
    app.finish_sketch_edit();

    // A PART: the extrusion of the rectangle, its length a formula, opened again from the tree
    app.chosen.sel = crate::gui::Sel::Sketch(si);
    let mut hand = Hand::new(app);
    hand.press_hint(&crate::i18n::tr("tb-extrude-hint"));
    if !hand.type_after_word(&crate::i18n::tr("f-length"), "fw/5") {
        problems.push(format!("[{step}] the extrusion opened no box with its length"));
    }
    hand.key(egui::Key::Enter);
    hand.close_window();
    let high = app.project.bodies.iter().filter(|b| app.project.body_owner(b.id) == Some(part)).filter_map(|b| b.mesh.bounds()).map(|bb| bb.max.z - bb.min.z).next();
    if !high.is_some_and(|h| (h - 10.0).abs() < 1e-6) {
        problems.push(format!("[{step}] the extrusion given fw/5 with fw = 50 stands {high:?} high"));
    }
    let row = crate::i18n::tr1("feat-extrude", "h", "10");
    let mut hand = Hand::new(app);
    let opened = hand.double_click_word(&row) && hand.shows("fw/5");
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.close_window();
    if !opened {
        problems.push(format!("[{step}] the extrusion opened again from the tree ({row:?}) does not show fw/5"));
    }
    check_all(app, &format!("{step}: a part, its extrusion"), problems);

    // AN ASSEMBLY: the part selected before the tool - the tree of the scenario runs past the window - laid out along X,
    // its pitch the formula fw
    let root = app.project.root;
    app.set_context_to(root);
    if let Some(ci) = app.project.components.iter().position(|c| c.id == part) {
        app.chosen.sel = crate::gui::Sel::Component(ci);
    }
    let mut hand = Hand::new(app);
    if !hand.press_hint(&crate::i18n::tr("tb-comp-lin-array-hint")) {
        problems.push(format!("[{step}] the assembly has no button of a linear pattern of parts"));
    }
    let pitch = hand.type_after_word(&crate::i18n::tr("f-pitch"), "fw");
    hand.key(egui::Key::Enter);
    hand.close_window();
    if !pitch {
        problems.push(format!("[{step}] the pattern of the part opened no box with its pitch"));
    }
    let laid = app.project.comp_pattern_of(part).is_some();
    if !laid {
        problems.push(format!("[{step}] the pattern of the part given the pitch fw was not made; the status line: {:?}", app.status));
    }
    check_all(app, &format!("{step}: an assembly, a pattern of a part"), problems);
}
