use super::*;
use crate::gui::hand::Carry;

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

/// THE PROFILE OF THE EXTRUSION IN HAND LET GO AND TAKEN AGAIN BY BOXES: a crossing drawn right to left over empty space
/// lets it go, a window drawn left to right round the contours of sketch `si` takes them back. Answers what did not work.
fn profile_boxed(hand: &mut Hand, si: usize) -> Option<String> {
    let points: Vec<qymcad_core::geom::Point2> = qymcad_ui_state::sketch_closed_contours(&hand.app.project, si)
        .into_iter()
        .filter_map(|id| hand.app.project.contour_index(id))
        .flat_map(|ci| hand.app.project.contours[ci].points.clone())
        .collect();
    if points.is_empty() {
        return Some("the sketch has no closed contour".into());
    }
    // the person looks at the sketch first: the middle of its contours
    let lo = points.iter().fold(qymcad_core::geom::Point2::new(f64::MAX, f64::MAX), |m, p| qymcad_core::geom::Point2::new(m.x.min(p.x), m.y.min(p.y)));
    let hi = points.iter().fold(qymcad_core::geom::Point2::new(f64::MIN, f64::MIN), |m, p| qymcad_core::geom::Point2::new(m.x.max(p.x), m.y.max(p.y)));
    let mid = qymcad_core::geom::Point2::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
    hand.look_at([mid.x, mid.y, 0.0], 6.0);
    let mut round = egui::Rect::NOTHING;
    for p in &points {
        round.extend_with(hand.on_screen3d([p.x, p.y, 0.0]));
    }
    let round = round.expand(20.0);
    // left of the contours: the box of the length stands by the arrow
    let empty = round.left_top() - egui::vec2(10.0, 0.0);
    hand.carry(Carry { from: empty, to: empty + egui::vec2(-30.0, 20.0) }).let_go(empty + egui::vec2(-30.0, 20.0));
    if !hand.app.tools.gsel.profiles.is_empty() {
        return Some(format!("a crossing over empty space at {empty:?} (the contours within {round:?}) left {} profiles in the extrusion", hand.app.tools.gsel.profiles.len()));
    }
    hand.carry(Carry { from: round.left_top(), to: round.right_bottom() }).let_go(round.right_bottom());
    let taken = hand.app.tools.gsel.profiles.len();
    (taken != 1).then(|| format!("a window round the rectangle took {taken} profiles into the extrusion, 1 expected"))
}

/// ONE GESTURE IN A TOOL OF EVERY WORKBENCH, the frame of a tool being one: the box at the geometry takes a formula over
/// the parameter `fw`, Tab goes on to the next field, Enter keeps it, Esc puts the tool down, a double click opens the
/// box again with the formula in it, the key of a tool pressed again puts it down - in a rectangle of a sketch, in the
/// extrusion of a part (its profile let go and taken back by boxes) and in the pattern of a part in the assembly - and `fw` changed, what was given it follows.
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
    if let Some(why) = profile_boxed(&mut hand, si) {
        problems.push(format!("[{step}] {why}"));
    }
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

/// The volume and the height of the body of `part`, as its mesh holds them.
struct Held {
    volume: f64,
    top: f64,
}

fn held(app: &App, part: u64) -> Option<Held> {
    let b = app.project.bodies.iter().rfind(|b| app.project.body_owner(b.id) == Some(part) && !app.project.consumed_bodies().contains(&b.id))?;
    Some(Held { volume: b.mesh.volume(), top: b.mesh.bounds()?.max.z })
}

/// A RAIL SWEPT ALONG AN EDGE OF A BODY, by hand: out to the assembly, a new part, a block of 40 x 30 extruded 10, a
/// square of 4 drawn on the side plane over its top front edge, the sweep taken with the square selected and the edge
/// clicked in the view - the block holds the rail of 4 x 4 along that edge; the block made 12 tall, the path rises with
/// the edge and the rail stays its length.
pub(super) fn a_rail_swept_along_an_edge(app: &mut App, problems: &mut Vec<String>) {
    let step = "a sweep along an edge";
    let (tr, corner) = (crate::i18n::tr, egui::pos2(0.0, 0.0));
    let mut hand = Hand::new(app);
    hand.key(egui::Key::Escape);
    // out of the sketch and the part the scenario stands in, by the button that finishes each, to the assembly
    for _ in 0..3 {
        if !hand.press_word(&tr("wb-finish"), corner) {
            break;
        }
    }
    let made = hand.press_hint(&tr("tb-new-part-hint"));
    // the block: a rectangle from the origin on the table, 40 by 30, extruded 10
    let drawn = hand.press_word(&tr("plane-xy-table"), corner);
    hand.sk_tool(SketchShape::Rect).click2d(0.0, 0.0).click2d(40.0, 30.0);
    hand.type_after_word(&tr("sk-width"), "40");
    hand.key(egui::Key::Tab).type_text("30").key(egui::Key::Enter);
    hand.close_window();
    hand.press_word(&tr("wb-finish"), corner);
    hand.press_hint(&tr("tb-extrude-hint"));
    hand.type_after_word(&tr("f-length"), "10");
    hand.key(egui::Key::Enter);
    hand.close_window();
    let part = app.project.components.iter().rev().find(|c| app.project.component_is_part(c.id)).map(|c| c.id).unwrap_or(0);
    if !made || !drawn {
        problems.push(format!("[{step}] a new part from the button of the assembly ({made}), a sketch on the table in it ({drawn})"));
    }
    check_all(app, &format!("{step}: the block"), problems);
    // the profile: a square of 4 on the side plane x = 0, over the top front edge, y 0..4, z 10..14
    let mut hand = Hand::new(app);
    hand.key(egui::Key::Escape);
    hand.press_word(&tr("plane-yz-side"), corner);
    hand.sk_tool(SketchShape::Rect).click2d(0.0, 10.0).click2d(4.0, 14.0);
    hand.type_after_word(&tr("sk-width"), "4");
    hand.key(egui::Key::Tab).type_text("4").key(egui::Key::Enter);
    hand.close_window();
    hand.press_word(&tr("wb-finish"), corner);
    hand.close_window();
    check_all(app, &format!("{step}: the square over the edge"), problems);
    // the sweep: the square left selected by its finish, the edge clicked in the view, Enter
    let mut hand = Hand::new(app);
    hand.look_at([20.0, 15.0, 5.0], 8.0).tool(PartTool::Sweep).click([20.0, 0.0, 10.0]).enter();
    hand.close_window();
    match held(app, part) {
        Some(Held { volume, top }) if (volume - 12640.0).abs() < 1.0 && (top - 14.0).abs() < 1e-3 => {}
        other => problems.push(format!("[{step}] the block with a rail of 4 x 4 along its top front edge holds 12640 up to z 14: the body holds {:?}", other.map(|h| [h.volume, h.top]))),
    }
    check_all(app, &format!("{step}: the rail"), problems);
    // the block made 12 tall from its row in the tree: the edge rises, the rail stays its length
    let row = crate::i18n::tr1("feat-extrude", "h", "10");
    let mut hand = Hand::new(app);
    hand.double_click_word(&row);
    hand.type_after_word(&crate::i18n::tr("f-length"), "12");
    hand.key(egui::Key::Enter);
    hand.close_window();
    match held(app, part) {
        Some(Held { volume, .. }) if (volume - 14720.0).abs() < 1.0 => {}
        other => problems.push(format!("[{step}] the block made 12 tall with the rail 2 above it holds 14720: the body holds {:?}", other.map(|h| [h.volume, h.top]))),
    }
    check_all(app, &format!("{step}: the block made taller under the rail"), problems);
}
