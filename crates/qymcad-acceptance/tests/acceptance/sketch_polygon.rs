//! A POLYGON OF A SKETCH REOPENED: its sides, its size and the names given to them, as a person changes them after it
//! is drawn.
use qymcad::{Key, Session};
use qymcad_acceptance::build::{draw, empty_sketch};
use qymcad_acceptance::probe;

/// A hexagon about the origin, its corner at (10, 0), drawn with the polygon tool and put down: the arrow in hand.
fn a_hexagon() -> Session {
    let mut s = empty_sketch();
    draw(&mut s, "tb-polygon-hint", &[(0.0, 0.0), (10.0, 0.0)]);
    s.key(Key::Escape).key(Key::Escape);
    s
}

/// The box of the polygon opened by a double click on its side between the corners (10, 0) and (5, 8.66).
fn reopened(s: &mut Session) {
    let side = s.on_sketch(7.5, 4.33);
    s.double_click(side);
}

probe! {
    /// THE SIDES OF A POLYGON ARE IN ITS BOX WHEN IT IS REOPENED: a double click on a side of a hexagon opens its box
    /// with the number of its sides, and 8 typed there makes it an octagon. Reported behaviour: the box reopened held
    /// the radius and the turn, and the sides could not be changed.
    fn the_sides_of_a_polygon_reopened_are_in_its_box() {
        let mut s = a_hexagon();
        assert!(s.document().sketches[0].lines == 6, "setup: the hexagon has {} sides", s.document().sketches[0].lines);
        reopened(&mut s);
        let sides = s.word("opt-sides");
        s.fill(&sides, "8").key(Key::Enter);
        let lines = s.document().sketches[0].lines;
        assert!(lines == 8, "8 typed into the sides of the hexagon reopened: it has {lines} sides");
    }
}

/// THE NAME FIELD OF THE ROW OF THE BOX WHOSE VALUE READS `value`: the field of the name standing on the line of that
/// value, right of it.
fn name_beside(s: &mut Session, value: &str) -> qymcad::Widget {
    let hint = s.word("sk-name-placeholder");
    let widgets = s.widgets();
    let field = widgets.iter().find(|w| w.kind == qymcad::Kind::TextField && w.value.trim() == value).unwrap_or_else(|| panic!("no field reads {value:?}: {widgets:?}"));
    let y = field.rect.center().y;
    widgets
        .iter()
        .filter(|w| w.kind == qymcad::Kind::TextField && w.placeholder == hint && (w.rect.center().y - y).abs() < field.rect.height() && w.rect.min.x > field.rect.max.x)
        .min_by(|a, b| a.rect.min.x.total_cmp(&b.rect.min.x))
        .cloned()
        .unwrap_or_else(|| panic!("no name stands beside the field reading {value:?}: {widgets:?}"))
}

/// WHAT THE TWO PARAMETERS READING THE NAMES OF A POLYGON SHOULD HOLD: `q = pr*2` and `w = pa+1`.
#[derive(Clone, Copy)]
struct Reads {
    q: f64,
    w: f64,
}

/// What the parameter `name` holds, read from the document.
fn parameter(s: &mut Session, name: &str) -> Option<f64> {
    s.document().parameters.iter().find(|p| p.name == name).map(|p| p.value)
}

/// The value field of the box reading `value`.
fn field_reading(s: &mut Session, value: &str) -> qymcad::Widget {
    s.widgets().into_iter().find(|w| w.kind == qymcad::Kind::TextField && w.value.trim() == value).unwrap_or_else(|| panic!("no field reads {value:?}"))
}

/// Type `text` over what field `w` holds.
fn type_over(s: &mut Session, w: &qymcad::Widget, text: &str) {
    s.click(w.rect.center()).chord(qymcad::Modifiers::COMMAND, Key::A).type_text(text);
}

probe! {
    /// THE NAMES OF THE RADIUS AND THE TURN OF A POLYGON STAY as the polygon is changed: named `pr` and `pa` in its box
    /// and read by `q = pr*2` and `w = pa+1`, they stay through the radius typed anew in the box a double click on the
    /// dimension opens, through the turn typed anew there, and through `pr` changed in the table of parameters. Reported
    /// behaviour: the names given to the drivers went when the polygon was changed, in the table or by a double click on
    /// its angle or its radius, and everything fell apart.
    fn the_names_of_a_polygon_stay_as_it_is_changed() {
        let mut s = a_hexagon();
        reopened(&mut s);
        let radius = name_beside(&mut s, "10");
        s.click(radius.rect.center()).type_text("pr");
        let turn = name_beside(&mut s, "0");
        // Enter in the field of a name applies the box, as in the field of a value
        s.click(turn.rect.center()).type_text("pa").key(Key::Enter);
        s.key(Key::Escape).key(Key::Escape);
        qymcad_acceptance::build::parameter(&mut s, "q", "pr*2");
        qymcad_acceptance::build::parameter(&mut s, "w", "pa+1");
        let mut fails = Vec::new();
        let mut judge = |s: &mut Session, when: &str, Reads { q, w }: Reads| {
            let (gq, gw) = (parameter(s, "q"), parameter(s, "w"));
            if gq.is_none_or(|v| (v - q).abs() > 1e-6) || gw.is_none_or(|v| (v - w).abs() > 1e-6) {
                fails.push(format!("{when}: q = pr*2 reads {gq:?} (should {q}), w = pa+1 reads {gw:?} (should {w})"));
            }
        };
        judge(&mut s, "named in the box", Reads { q: 20.0, w: 1.0 });
        // the dimension of the radius double-clicked opens the box of the polygon
        let canvas = s.canvas();
        let label = s.words_at().into_iter().find(|(t, r)| t.starts_with('R') && t.contains("10") && canvas.contains(r.center())).map(|(_, r)| r).unwrap_or_else(|| panic!("no dimension of the radius on the sheet: {:?}", s.words()));
        s.double_click(label.center());
        let r = field_reading(&mut s, "10");
        type_over(&mut s, &r, "12");
        s.key(Key::Enter);
        s.key(Key::Escape).key(Key::Escape);
        judge(&mut s, "the radius made 12 in the box of its dimension", Reads { q: 24.0, w: 1.0 });
        reopened_at(&mut s, 12.0);
        let a = field_reading(&mut s, "0");
        type_over(&mut s, &a, "15");
        s.key(Key::Enter);
        s.key(Key::Escape).key(Key::Escape);
        judge(&mut s, "the turn made 15 in the box", Reads { q: 24.0, w: 16.0 });
        qymcad_acceptance::build::set_parameter(&mut s, "pr", "8");
        judge(&mut s, "pr made 8 in the table", Reads { q: 16.0, w: 16.0 });
        assert!(fails.is_empty(), "{}", fails.join("\n"));
    }
}

/// The box of a hexagon of radius `r` about the origin, unturned, opened by a double click on its first side.
fn reopened_at(s: &mut Session, r: f64) {
    let side = s.on_sketch(0.75 * r, 0.433 * r);
    s.double_click(side);
}

/// THE DIAGONAL OF A REGULAR PENTAGON of side 1: the golden ratio.
const PENTAGON_DIAGONAL: f64 = 1.618_033_988_749_895;

/// Whether two places of the sketch stand `d` apart.
fn two_stand_apart(s: &mut Session, d: f64) -> bool {
    let places = s.document().sketches[0].places.clone();
    places.iter().enumerate().any(|(i, a)| places.iter().skip(i + 1).any(|b| ((a[0] - b[0]).hypot(a[1] - b[1]) - d).abs() < 1e-6))
}

probe! {
    /// A POLYGON DRAWN BY ITS EDGE IS SIZED BY ITS EDGE: a pentagon drawn with the polygon tool by edge, from (0, 0) to
    /// (10, 0), given 20 in its box, has sides of 20 - its diagonals 20 x 1.618 - and reopened, its box holds the side 20
    /// and 30 typed there makes the sides 30. Reported behaviour: by edge too the number went to the radius, as the
    /// polygon was drawn and as it was edited.
    fn a_polygon_drawn_by_its_edge_is_sized_by_its_edge() {
        let mut s = empty_sketch();
        let hint = s.word("tb-polygon-hint");
        s.press_hint(&hint);
        let sides = s.word("opt-sides");
        s.fill(&sides, "5").key(Key::Enter);
        let by_edge = s.word("opt-by-edge");
        s.press_word(&by_edge);
        for (x, y) in [(0.0, 0.0), (10.0, 0.0)] {
            let at = s.on_sketch(x, y);
            s.click(at);
        }
        let side = field_reading(&mut s, "10");
        type_over(&mut s, &side, "20");
        s.key(Key::Enter);
        s.key(Key::Escape).key(Key::Escape);
        let mut fails = Vec::new();
        if !two_stand_apart(&mut s, 20.0 * PENTAGON_DIAGONAL) {
            fails.push(format!("drawn by edge and given 20: no two corners stand {:.4} apart, the corners are {:?}", 20.0 * PENTAGON_DIAGONAL, s.document().sketches[0].places));
        }
        let middle = s.on_sketch(10.0, 0.0);
        s.double_click(middle);
        let side = field_reading(&mut s, "20");
        type_over(&mut s, &side, "30");
        s.key(Key::Enter);
        s.key(Key::Escape).key(Key::Escape);
        if !two_stand_apart(&mut s, 30.0 * PENTAGON_DIAGONAL) {
            fails.push(format!("reopened and given 30: no two corners stand {:.4} apart, the corners are {:?}", 30.0 * PENTAGON_DIAGONAL, s.document().sketches[0].places));
        }
        assert!(fails.is_empty(), "{}", fails.join("\n"));
    }
}

/// HOW MANY PIXELS NEAR (x, y) OF THE SHEET ARE DRAWN BRIGHT - a curve, not the dark sheet and its faint grid - in the
/// 7 x 7 pixels about it.
fn bright_near(s: &mut Session, (x, y): (f64, f64)) -> usize {
    let at = s.on_sketch(x, y);
    let p = s.snapshot();
    let (cx, cy) = (at.x.round() as i64, at.y.round() as i64);
    let mut n = 0;
    for dy in -3..=3 {
        for dx in -3..=3 {
            let (px, py) = (cx + dx, cy + dy);
            if px < 0 || py < 0 || px as usize >= p.width || py as usize >= p.height {
                continue;
            }
            let i = (py as usize * p.width + px as usize) * 4;
            if p.rgba[i..i + 3].iter().any(|c| *c > 150) {
                n += 1;
            }
        }
    }
    n
}

probe! {
    /// THE SIDES OF A POLYGON TURN WITH IT FROM THE TABLE: a hexagon of radius 10, its turn named `pa` in its box and
    /// made 30 in the table of parameters, is drawn turned - no side stands any more through the middle of the side it
    /// had from (10, 0) to (5, 8.66). Reported behaviour: the points turned, and the sides were drawn where they stood;
    /// a double click on the polygon and the same turn in its box turned them.
    fn the_sides_of_a_polygon_turn_with_it_from_the_table() {
        let mut s = a_hexagon();
        reopened(&mut s);
        let turn = name_beside(&mut s, "0");
        s.click(turn.rect.center()).type_text("pa").key(Key::Enter);
        s.key(Key::Escape).key(Key::Escape);
        assert!(bright_near(&mut s, (7.5, 4.33)) > 0, "setup: no side is drawn through (7.5, 4.33)");
        qymcad_acceptance::build::set_parameter(&mut s, "pa", "30");
        let left = bright_near(&mut s, (7.5, 4.33));
        assert!(left == 0, "the turn made 30 in the table: a side is still drawn through (7.5, 4.33), {left} pixels; the corners stand at {:?}", s.document().sketches[0].places);
    }
}
