//! THE TEXT OF A DIMENSION STANDS CLEAR OF ITS LINES, and says what the settings ask for.
//!
//! Reported behaviour: the text of a dimension crosses the dimension line itself.
use egui::{pos2, vec2, Pos2, Rect, Vec2};
use qymcad_ui_state::{dim_text_place, dim_text_size, radial_text_place, DIM_TEXT_GAP};

/// How far the box of a text of `size` centred at `c` stands from the segment `a`-`b` (0 when they meet).
fn clearance(c: Pos2, size: Vec2, a: Pos2, b: Pos2) -> f32 {
    let r = Rect::from_center_size(c, size);
    let mut best = f32::INFINITY;
    for k in 0..=200 {
        let p = a + (b - a) * (k as f32 / 200.0);
        let dx = (r.min.x - p.x).max(p.x - r.max.x).max(0.0);
        let dy = (r.min.y - p.y).max(p.y - r.max.y).max(0.0);
        best = best.min(dx.hypot(dy));
    }
    best
}

/// A LINEAR DIMENSION, horizontal, vertical or at a slant: the box of its text stands clear of the dimension line.
#[test]
fn the_text_of_a_linear_dimension_does_not_cross_its_line() {
    let size = dim_text_size("25.0", 13.0);
    for (name, la, lb) in [
        ("horizontal", pos2(100.0, 300.0), pos2(400.0, 300.0)),
        ("vertical", pos2(300.0, 100.0), pos2(300.0, 400.0)),
        ("slanted", pos2(100.0, 100.0), pos2(300.0, 300.0)),
    ] {
        let dir = (lb - la).normalized();
        let perp = vec2(-dir.y, dir.x);
        let place = dim_text_place(la, lb, perp, size, None, false);
        assert!(place.shelf.is_none(), "{name}: a text with room between the arrows was carried out onto a shelf");
        let gap = clearance(place.center, size, la, lb);
        assert!(gap >= DIM_TEXT_GAP - 0.01, "{name}: the text stands {gap:.1} px from its dimension line - it crosses it or touches it");
    }
}

/// A TEXT THAT DOES NOT FIT BETWEEN THE ARROWS is carried past the arrow onto a shelf, clear of the dimension line.
#[test]
fn a_text_with_no_room_goes_out_onto_a_shelf() {
    let size = dim_text_size("2.5", 13.0);
    let (la, lb) = (pos2(200.0, 300.0), pos2(220.0, 300.0)); // 20 px between the arrows
    let place = dim_text_place(la, lb, vec2(0.0, -1.0), size, None, false);
    let shelf = place.shelf.expect("a text 5 letters wide between arrows 20 px apart must go out onto a shelf");
    assert!((shelf[0] - lb).length() < 0.01 && shelf[1].x > lb.x + size.x, "the shelf does not run past the arrow far enough to carry the text: {shelf:?}");
    assert!(clearance(place.center, size, la, lb) >= DIM_TEXT_GAP - 0.01, "the text carried out still stands on the dimension line");
    assert!(clearance(place.center, size, shelf[0], shelf[1]) >= DIM_TEXT_GAP - 0.01, "the text lies on its own shelf rather than above it");
}

/// A TEXT LED ALONG ITS LINE stands where it was led: between the arrows with no shelf, past an arrow on a shelf that
/// runs on from the arrow to the far end of the text - clear of the line either way.
#[test]
fn a_text_led_along_its_line_stands_where_it_was_led() {
    let size = dim_text_size("25.0", 13.0);
    let (la, lb) = (pos2(100.0, 300.0), pos2(400.0, 300.0));
    let inside = dim_text_place(la, lb, vec2(0.0, -1.0), size, Some(0.2), false);
    assert!(inside.shelf.is_none() && (inside.center.x - 160.0).abs() < 0.01, "a text led to a fifth of the line stands at {:?}, shelf {:?}", inside.center, inside.shelf);
    assert!(clearance(inside.center, size, la, lb) >= DIM_TEXT_GAP - 0.01, "the text led along stands on its line");
    for (name, t) in [("past the second arrow", 1.4), ("past the first arrow", -0.4)] {
        let out = dim_text_place(la, lb, vec2(0.0, -1.0), size, Some(t), false);
        let shelf = out.shelf.unwrap_or_else(|| panic!("{name}: the text has no shelf to stand on"));
        let (lo, hi) = (shelf[0].x.min(shelf[1].x), shelf[0].x.max(shelf[1].x));
        assert!(lo <= out.center.x - size.x / 2.0 && hi >= out.center.x + size.x / 2.0, "{name}: the shelf {shelf:?} does not run under the whole text at {:?}", out.center);
        assert!(clearance(out.center, size, shelf[0], shelf[1]) >= DIM_TEXT_GAP - 0.01, "{name}: the text lies on its shelf rather than above it");
    }
}

/// A RADIUS OR A DIAMETER: the text stands on a shelf past the knee, clear of the leader.
#[test]
fn the_text_of_a_radius_stands_on_a_shelf() {
    let size = dim_text_size("R12.5", 13.0);
    for (name, toward) in [("to the right", vec2(1.0, 0.0)), ("to the left", vec2(-1.0, 0.0)), ("up and left", vec2(-0.6, -0.8))] {
        let centre = pos2(400.0, 400.0);
        let edge = centre + toward * 80.0;
        let knee = centre + toward * 94.0;
        let (at, shelf) = radial_text_place(knee, toward, size);
        assert!(clearance(at, size, edge, knee) >= DIM_TEXT_GAP - 0.01, "{name}: the text of the radius lies on its leader");
        assert!(clearance(at, size, shelf[0], shelf[1]) >= DIM_TEXT_GAP - 0.01, "{name}: the text lies on its shelf rather than above it");
        assert!((shelf[1].x - shelf[0].x).abs() >= size.x, "{name}: the shelf is shorter than the text standing on it");
    }
}

/// WHAT THE LABEL SAYS: the value alone, the name of a driver, the formula, both - as the settings ask.
#[test]
fn the_label_says_what_the_settings_ask_for() {
    use qymcad_core::model::{Constraint, Project};
    let mut p = Project::default();
    let si = p.new_sketch("S");
    p.add_line_entity(si, 0.0, 0.0, 110.0, 0.0, qymcad_core::feature::Purpose::Real);
    let (a, b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);
    p.parameters.push(qymcad_core::model::Param { name: "w".into(), expr: "50".into(), value: 50.0 });
    let c = Constraint::Distance { dimension: Default::default(), a, b, d: 110.0, off: 0.0, expr: "2*w+10".into(), driven: false, axis: 1, at: None };
    p.sketches[si].constraints.push(c);
    let sketch = p.sketches[si].id;
    assert!(p.name_sketch_dimension("len".into(), qymcad_core::model::SketchDimensionSelection { sketch, index: p.sketches[si].constraints.len() - 1 }), "setup: the dimension could not be named");
    let c = p.sketches[si].constraints.last().unwrap();
    let mut set = qymcad_ui_state::Settings::default();
    let says = |set: &qymcad_ui_state::Settings| qymcad_ui_state::dim_caption(&p, si, c, set).expect("a dimension has a label");
    assert_eq!(says(&set), "110.0", "with both switches off the label is the value alone");
    set.dim_show_name = true;
    assert_eq!(says(&set), "len = 110.0");
    set.dim_show_formula = true;
    assert_eq!(says(&set), "len = 2*w+10 = 110.0");
    set.dim_show_name = false;
    assert_eq!(says(&set), "2*w+10 = 110.0");
    // a number typed into the field is no formula: it would only repeat the value
    let plain = Constraint::Distance { dimension: Default::default(), a, b, d: 110.0, off: 0.0, expr: "110".into(), driven: false, axis: 1, at: None };
    assert_eq!(qymcad_ui_state::dim_caption(&p, si, &plain, &set).as_deref(), Some("110.0"));
}

/// A sheet 800 x 600 at 4 px to the unit, the origin in the middle, and a sketch with a line from (0, 0) to (40, 0)
/// and a vertical one from (0, 0) to (0, 30), each with a length dimension led out by `off`.
fn sheet_with_lengths(off_h: f64, off_v: f64) -> (qymcad_core::model::Project, usize, qymcad_ui_state::Sheet) {
    use qymcad_core::model::{Constraint, Project};
    let mut p = Project::default();
    let si = p.new_sketch("S");
    p.add_line_entity(si, 0.0, 0.0, 40.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 0.0, 0.0, 0.0, 30.0, qymcad_core::feature::Purpose::Real);
    let pt = |p: &Project, x: f64, y: f64| p.sketches[si].points.iter().find(|q| (q.x - x).abs() < 1e-9 && (q.y - y).abs() < 1e-9).map(|q| q.id).expect("a point");
    let (o, e, t) = (pt(&p, 0.0, 0.0), pt(&p, 40.0, 0.0), pt(&p, 0.0, 30.0));
    p.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a: o, b: e, d: 40.0, off: off_h, expr: String::new(), driven: true, axis: 1, at: None });
    p.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a: o, b: t, d: 30.0, off: off_v, expr: String::new(), driven: true, axis: 2, at: None });
    let sh = qymcad_ui_state::Sheet { view: qymcad_ui_state::View2d::default(), rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0)) };
    (p, si, sh)
}

/// THE TEXT STANDS ON THE FAR SIDE OF ITS LINE FROM THE GEOMETRY: a dimension led above the geometry has its text
/// above its line, one led below below it; a vertical one led right has it to the right, led left to the left.
/// Reported behaviour: the text of a length stood under its line whatever side of the geometry the line was on.
#[test]
fn the_text_stands_on_the_far_side_of_its_line_from_the_geometry() {
    let set = qymcad_ui_state::Settings::default();
    for (name, off, horizontal, beyond) in [("above", -5.0, true, -1.0f32), ("below", 5.0, true, 1.0), ("to the right", 5.0, false, 1.0), ("to the left", -5.0, false, -1.0)] {
        let (p, si, sh) = if horizontal { sheet_with_lengths(off, 0.0) } else { sheet_with_lengths(0.0, off) };
        let ci = if horizontal { 0 } else { 1 };
        let (line_a, _, _) = qymcad_ui_state::linear_dim_line(&p, si, &p.sketches[si].constraints[ci], &sh).expect("a line");
        let (place, _) = qymcad_ui_state::linear_text_of(&p, si, ci, &sh, &set).expect("a text");
        let (text, line) = if horizontal { (place.center.y, line_a.y) } else { (place.center.x, line_a.x) };
        assert!((text - line).signum() == beyond, "a dimension led {name} of the geometry has its text on the other side of its line: text at {text}, line at {line}");
    }
}

/// ALONG THE LINE a vertical dimension's text stands upright, read from the bottom up, and clear of its line; level, it
/// lies level.
#[test]
fn along_the_line_a_vertical_text_stands_upright() {
    let (p, si, sh) = sheet_with_lengths(0.0, 6.0);
    let mut set = qymcad_ui_state::Settings::default();
    assert_eq!(set.dim_text, qymcad_ui_state::DimTextTurn::AlongLine, "the text of a dimension goes along its line unless the settings say otherwise");
    let (place, size) = qymcad_ui_state::linear_text_of(&p, si, 1, &sh, &set).expect("a text");
    assert!((place.angle + std::f32::consts::FRAC_PI_2).abs() < 1e-4, "the text of a vertical dimension is turned by {} rad, not read from the bottom up", place.angle);
    let (la, lb, _) = qymcad_ui_state::linear_dim_line(&p, si, &p.sketches[si].constraints[1], &sh).expect("a line");
    // turned upright, the text is `size.y` wide across the line and `size.x` tall along it
    let upright = vec2(size.y, size.x);
    assert!(clearance(place.center, upright, la, lb) >= DIM_TEXT_GAP - 0.01, "the upright text lies on its line");
    set.dim_text = qymcad_ui_state::DimTextTurn::Horizontal;
    let (level, _) = qymcad_ui_state::linear_text_of(&p, si, 1, &sh, &set).expect("a text");
    assert!(level.angle.abs() < 1e-6, "asked to lie level, the text is turned by {} rad", level.angle);
}
