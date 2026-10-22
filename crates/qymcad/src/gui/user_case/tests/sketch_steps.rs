//! STEPS OF THE SCENARIO IN A SKETCH: the sizes laid only when typed, a rectangle with its centre fixed and the tools
//! taken by a click, in the sketch of every tool; and every kind of curve cut, extended and copied in a sketch of its own,
//! the sketch then opened again and its own steps taken back and brought again.
use super::*;

/// THE SIZES OF THE SKETCH OF EVERY TOOL: a size laid only when typed, then a rectangle with its centre fixed.
pub(super) fn sizes(app: &mut App, si: usize, problems: &mut Vec<String>) {
    a_size_only_when_typed(app, si, problems);
    a_rectangle_with_its_centre_fixed(app, si, problems);
}

/// A SIZE ONLY WHEN TYPED: the circles and the arc above were drawn without a value typed and carry no size of
/// their own; a circle drawn with 12 typed carries one diameter of 12.
fn a_size_only_when_typed(app: &mut App, si: usize, problems: &mut Vec<String>) {
    // the sizes on the rims of the circles and arcs drawn as shapes; the construction circle of the polygon keeps
    // the radius that holds the polygon
    let sizes = |a: &App| {
        let sk = &a.project.sketches[si];
        let shapes: Vec<u64> = sk.entities.iter().filter(|e| !e.construction).filter_map(|e| e.kind.round_centre()).collect();
        sk.constraints
            .iter()
            .filter_map(|c| match c {
                qymcad_core::model::Constraint::Diameter { c, d, diam, .. } if shapes.contains(c) => Some((*diam, *d)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    if !sizes(app).is_empty() {
        problems.push(format!("sketch: circles and an arc drawn without a value typed carry sizes {:?}", sizes(app)));
    }
    let mut hand = Hand::new(app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(100.0, 10.0).click2d(106.0, 10.0).type_text("12").key(egui::Key::Enter);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    if sizes(app) != vec![(true, 12.0)] {
        problems.push(format!("sketch: a circle drawn with 12 typed carries sizes {:?}", sizes(app)));
    }
    check_all(app, "sketch: a circle sized by a value typed", problems);
}

/// A RECTANGLE WITH ITS CENTRE FIXED: drawn from a corner, the centre picked and fixed, a corner dragged by the
/// mouse - it resizes about the centre, which stays
fn a_rectangle_with_its_centre_fixed(app: &mut App, si: usize, problems: &mut Vec<String>) {
    let mut hand = Hand::new(app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(110.0, 30.0).click2d(130.0, 50.0).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_select();
    let centre = hand.app.project.sketches[si].rects().last().map(|r| r.centre);
    match centre {
        Some(c) if hand.select2d(&[qymcad_ui_state::SketchItem::point(c)]) => {
            hand.constraint(qymcad_ui_state::ConstraintKind::Fix);
            hand.key(egui::Key::Escape);
            hand.drag2d((130.0, 50.0), (134.0, 53.0));
            let sk = &hand.app.project.sketches[si];
            let stays = sk.points.iter().find(|q| q.id == c).is_some_and(|q| (q.x - 120.0).abs() < 0.6 && (q.y - 40.0).abs() < 0.6);
            let moved = sk.points.iter().any(|q| (q.x - 134.0).abs() < 0.6 && (q.y - 53.0).abs() < 0.6);
            if !stays || !moved {
                problems.push(format!("sketch: a corner of a rectangle with its centre fixed dragged to (134, 53): the centre stays {stays}, the corner went there {moved}"));
            }
        }
        _ => problems.push("sketch: the centre of a rectangle drawn from a corner could not be picked".into()),
    }
    check_all(app, "sketch: a rectangle with its centre fixed resized by a corner", problems);
}

/// EVERY KIND OF CURVE CUT, EXTENDED AND COPIED, by hand in a part of its own: an ellipse 12 by 6 between two uprights at
/// x = -6 and x = 6, its top trimmed away between them - an arc of the ellipse is left; an open spline left of the left
/// upright, its end extended to it - the spline ends on the upright; a hexagon taken by a window, copied to the clipboard and
/// pasted - two hexagons, each a figure of its own; two circles crossing, the corner above them rounded by a click on each
/// and one in its quarter; a circle touching two lines, and a circle by three points, modes of the circle. Each kind took
/// its own path through these tools once, and a chain of them is where such paths part.
pub(super) fn every_kind_of_curve_cut_extended_and_copied(app: &mut App, problems: &mut Vec<String>) {
    app.exit_context();
    let part = app.project.add_part("Curves");
    app.enter_component(part);
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    let curves = |a: &App| a.project.sketches[si].entities.iter().filter_map(|e| qymcad_core::model::curve::Curve::of(&a.project.sketches[si], e)).collect::<Vec<_>>();
    let mut hand = Hand::new(app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Ellipse).click2d(0.0, 40.0).click2d(12.0, 40.0).click2d(0.0, 46.0).key(egui::Key::Escape).key(egui::Key::Escape);
    for x in [-6.0, 6.0] {
        hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(x, 25.0).click2d(x, 55.0).key(egui::Key::Escape).key(egui::Key::Escape);
    }
    check_all(app, "curves: an ellipse between two uprights", problems);
    // the top of the ellipse, between the uprights, trimmed away
    let mut hand = Hand::new(app);
    assert!(hand.press_hint(&qymcad_i18n::tr("tb-trim-hint")), "no Trim button");
    hand.close_window();
    hand.click2d(0.0, 46.0).key(egui::Key::Escape);
    let arcs = curves(app).iter().filter(|c| matches!(c, qymcad_core::model::curve::Curve::Oval { .. }) && !c.closed()).count();
    let top_gone = curves(app).iter().all(|c| c.nearest(qymcad_core::geom::Point2::new(0.0, 46.0)).off > 1e-3);
    if arcs != 1 || !top_gone {
        problems.push(format!("curves: the top of the ellipse trimmed away - {arcs} arcs of an ellipse, the top gone {top_gone}"));
    }
    check_all(app, "curves: the top of an ellipse trimmed away", problems);
    // an open spline left of the left upright, its right end extended to it - the way it leaves its end meets the upright at
    // y = 30
    let mut hand = Hand::new(app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(-30.0, 30.0).click2d(-24.0, 34.0).double_click2d(-16.0, 33.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_select();
    assert!(hand.press_hint(&qymcad_i18n::tr("tb-extend-hint")), "no Extend button");
    hand.close_window();
    hand.click2d(-16.8, 33.2).click2d(-6.0, 40.0).key(egui::Key::Escape).key(egui::Key::Escape);
    let reaches = curves(app).iter().filter(|c| matches!(c, qymcad_core::model::curve::Curve::Spline { .. })).any(|c| c.ends().iter().any(|e| (e.x + 6.0).abs() < 1e-6));
    if !reaches {
        problems.push("curves: the spline extended to the left upright does not end on it".into());
    }
    check_all(app, "curves: a spline extended to an upright", problems);
    // a hexagon copied to the clipboard and pasted
    let mut hand = Hand::new(app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Polygon).click2d(30.0, 40.0).click2d(36.0, 40.0).key(egui::Key::Escape).key(egui::Key::Escape);
    // the hexagon taken whole by a window drawn round it
    hand.sk_select().drag2d((21.0, 31.0), (39.0, 49.0));
    hand.copy();
    hand.click2d(30.0, 40.0);
    hand.paste();
    hand.click2d(50.0, 40.0);
    hand.key(egui::Key::Escape);
    let figures = app.project.sketches[si].figures.len();
    if figures != 2 {
        problems.push(format!("curves: a hexagon copied and pasted - {figures} figures, not two"));
    }
    check_all(app, "curves: a hexagon copied and pasted", problems);
    // two circles crossing at (76, 8) and (76, -8), the fillet taken, a click on each circle and one above the upper
    // crossing: the corner is rounded with the radius of the bar
    let mut hand = Hand::new(app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(70.0, 0.0).click2d(80.0, 0.0).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(82.0, 0.0).click2d(92.0, 0.0).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_select();
    assert!(hand.press_hint(&qymcad_i18n::tr("tb-fillet-sketch-hint")), "no Fillet button");
    hand.close_window();
    let r = hand.app.tool_prefs.fillet;
    hand.click2d(60.0, 0.0).click2d(92.0, 0.0);
    hand.hover2d(76.0, 11.0).click2d(76.0, 11.0);
    hand.frame(Vec::new());
    hand.chord(egui::Modifiers::COMMAND, egui::Key::A).type_text(&r.to_string()).key(egui::Key::Enter);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let rounded = curves(app).iter().any(|c| matches!(c, qymcad_core::model::curve::Curve::Round { centre, r: k, .. } if (k - r).abs() < 1e-6 && centre.y > 8.0));
    if !rounded {
        problems.push(format!("curves: the corner above the crossing of two circles is not rounded with R{r}"));
    }
    check_all(app, "curves: a corner at the crossing of two circles", problems);
    // two lines meeting at (110, 0), the circle in its mode of touching two curves: a click on each line and one inside
    // the corner lay a circle of the radius of the bar touching both
    let mut hand = Hand::new(app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(110.0, 0.0).double_click2d(140.0, 0.0);
    hand.click2d(110.0, 0.0).double_click2d(110.0, 30.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle);
    assert!(hand.press_word(&qymcad_i18n::tr("opt-tangent-m"), egui::pos2(400.0, 0.0)), "no mode of touching on the bar of the circle");
    let r = hand.app.tool_prefs.circ_r;
    hand.click2d(130.0, 0.0).click2d(110.0, 20.0).hover2d(116.0, 6.0).click2d(116.0, 6.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let touching = curves(app)
        .iter()
        .any(|c| matches!(c, qymcad_core::model::curve::Curve::Round { centre, r: k, .. } if (k - r).abs() < 1e-6 && centre.dist(qymcad_core::geom::Point2::new(110.0 + r, r)) < 1e-4));
    if !touching {
        problems.push(format!("curves: no circle of R{r} touching the two lines met at (110, 0)"));
    }
    check_all(app, "curves: a circle touching two lines", problems);
    // the circle in its mode of three points: three clicks lay the circle of radius 10 about (130, 30)
    let mut hand = Hand::new(app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle);
    assert!(hand.press_word(&qymcad_i18n::tr("opt-rect-3pt"), egui::pos2(400.0, 0.0)), "no mode of three points on the bar of the circle");
    hand.click2d(140.0, 30.0).click2d(130.0, 40.0).click2d(120.0, 30.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let through = curves(app)
        .iter()
        .any(|c| matches!(c, qymcad_core::model::curve::Curve::Round { centre, r, .. } if centre.dist(qymcad_core::geom::Point2::new(130.0, 30.0)) < 1e-6 && (r - 10.0).abs() < 1e-6));
    if !through {
        problems.push("curves: three clicks in the mode of three points laid no circle of radius 10 about (130, 30)".into());
    }
    check_all(app, "curves: a circle by three points", problems);
    app.finish_sketch_edit();
    a_sketch_reopened_walks_its_own_steps(app, si, problems);
}

/// THE SKETCH OF THE CURVES OPENED AGAIN, A SESSION OF ITS OWN: finished, it was one step of the document; two lines
/// drawn in it again, three Ctrl+Z take them back and touch no step of the document, two Ctrl+Shift+Z bring them;
/// finished, one Ctrl+Z takes both lines away and one Ctrl+Shift+Z brings them.
fn a_sketch_reopened_walks_its_own_steps(app: &mut App, si: usize, problems: &mut Vec<String>) {
    let name = qymcad_i18n::tr1("g-sketch-edited", "name", &crate::i18n::name(&app.project.sketches[si].name));
    let top = |a: &App| a.disk.edits.undo.last().map(|s| s.name.clone());
    if top(app).as_ref() != Some(&name) {
        problems.push(format!("history: the finished sketch of the curves is the step {:?}, not {name:?}", top(app)));
    }
    let lines = |a: &App| a.project.sketches[si].entities.iter().filter(|e| e.kind.as_line().is_some()).count();
    let depth = app.disk.edits.undo.len();
    app.enter_sketch_edit(si);
    let opened = lines(app);
    let mut hand = Hand::new(app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, -20.0).double_click2d(20.0, -20.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, -30.0).double_click2d(20.0, -30.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let drawn = lines(hand.app);
    for _ in 0..3 {
        hand.undo();
    }
    let redo = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
    let (back, kept) = (lines(hand.app), hand.app.disk.edits.undo.len());
    hand.chord(redo, egui::Key::Z).chord(redo, egui::Key::Z).close_window();
    if drawn != opened + 2 || back != opened || kept != depth || lines(app) != drawn {
        problems.push(format!(
            "history: two lines in the sketch reopened - {opened} lines, {drawn} drawn, {back} after three Ctrl+Z with {kept} steps of the {depth} of the document, {} brought again",
            lines(app)
        ));
    }
    check_all(app, "history: the steps of a sketch reopened", problems);
    app.finish_sketch_edit();
    let mut hand = Hand::new(app);
    hand.undo();
    let undone = lines(hand.app);
    hand.chord(redo, egui::Key::Z).close_window();
    if undone != opened || lines(app) != drawn || app.disk.edits.undo.len() != depth + 1 {
        problems.push(format!("history: the sketch finished again - one Ctrl+Z left {undone} lines of {opened}, one Ctrl+Shift+Z {} of {drawn}, {} steps", lines(app), app.disk.edits.undo.len()));
    }
    check_all(app, "history: a finished sketch is one step", problems);
}

/// THE TOOLS TAKEN BY A CLICK on the lines of the sketch of every tool: a chamfer, a trim, an extension and a break,
/// each clicked once - none of them does nothing in silence.
pub(super) fn the_click_tools(app: &mut App, si: usize, problems: &mut Vec<String>) {
    let count = |a: &App| a.project.sketches[si].entities.len() + a.project.sketches[si].points.len();
    let ops = [
        drawing(qymcad_ui_state::ClickTool::CornerChamfer, "chamfer", &[(30.0, 0.0)]),
        drawing(qymcad_ui_state::ClickTool::Trim, "trim", &[(40.0, 0.0)]),
        drawing(qymcad_ui_state::ClickTool::Extend, "extend", &[(30.0, 7.0)]),
        drawing(qymcad_ui_state::ClickTool::Break, "break", &[(50.0, 7.0)]),
    ];
    for op in ops {
        let before = count(app);
        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_of!(app), &mut app.viewing.mode_3d, op.tool);
        for &(x, y) in op.clicks {
            Hand::new(app).click2d(x, y);
        }
        if count(app) == before && app.status.words.trim().is_empty() {
            problems.push(format!("sketch: \"{}\" silently did nothing", op.name));
        }
        check_all(app, &format!("sketch: {}", op.name), problems);
    }
    qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_of!(app));
}
