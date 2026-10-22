//! THE TOOLS THAT CUT AND LENGTHEN: trim by a click and by a stroke, break at a point, extend to a curve - on every
//! kind. A cell lays the kind with a line across it, works the tool by hand and reads the curves as points: what is
//! left lies where the curve lay, the piece clicked is gone, the end stretched lies on the line it was taken to.
use super::*;

/// The place the kind is drawn about.
const AT: Point2 = Point2 { x: 40.0, y: 30.0 };

/// THE LINE ACROSS THE KIND for trim and break: upright, a millimetre right of its place, 12 above and below.
const ACROSS: [Point2; 2] = [Point2 { x: 41.0, y: 18.0 }, Point2 { x: 41.0, y: 42.0 }];

/// A TOOL OF THE GROUP and the way it is worked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edit {
    /// trim, a click on the piece
    TrimClick,
    /// trim, a stroke dragged across the piece
    TrimStroke,
    /// break, a click on the curve
    Break,
    /// extend, the curve picked before the tool and Enter
    ExtendPicked,
    /// extend, the tool first, a click on the curve and a click on the curve it goes to
    ExtendToolFirst,
}

const EDITS: [Edit; 5] = [Edit::TrimClick, Edit::TrimStroke, Edit::Break, Edit::ExtendPicked, Edit::ExtendToolFirst];

/// WHERE AN OPEN KIND IS EXTENDED: the end that goes, and the line it goes to, both about the place of the kind.
struct Stretch {
    end: Point2,
    to: [Point2; 2],
}

/// The end of each open kind and the line on its way: a line along its own direction, an arc round its own circle to a
/// level line below, a spline along the tangent of its end.
fn stretch(kind: &Kind) -> Option<Stretch> {
    let up = [Point2::new(12.0, -12.0), Point2::new(12.0, 14.0)];
    match kind.name {
        "line" => Some(Stretch { end: Point2::new(8.0, 4.0), to: up }),
        "arc" => Some(Stretch { end: Point2::new(7.0, -3.0), to: [Point2::new(-4.0, -8.0), Point2::new(16.0, -8.0)] }),
        "open spline" => Some(Stretch { end: Point2::new(8.0, 1.0), to: up }),
        // the right end of the arc of the ellipse 8 by 4, where the line a millimetre below its centre cut it, round
        // below to a level line 3 below the centre
        "arc of an ellipse" => Some(Stretch { end: Point2::new(8.0 * (15.0f64 / 16.0).sqrt(), -1.0), to: [Point2::new(-4.0, -3.0), Point2::new(16.0, -3.0)] }),
        _ => None,
    }
}

fn about(p: Point2) -> Point2 {
    Point2::new(AT.x + p.x, AT.y + p.y)
}

/// WHY A CELL MAKES NO SENSE, when it does not.
fn senseless(kind: &Kind, edit: Edit) -> Option<&'static str> {
    let closed = matches!(kind.name, "circle" | "ellipse" | "closed spline");
    match edit {
        _ if kind.name == "point" => Some("a point has no piece to cut and no end to stretch"),
        Edit::Break if closed => Some("a closed curve broken at one point is the same loop; it is cut in two by Trim"),
        Edit::ExtendPicked | Edit::ExtendToolFirst if stretch(kind).is_none() => Some("a closed curve has no end, and a side of a closed figure meets its neighbours at both"),
        _ => None,
    }
}

/// A SIDE OF THE LINE ACROSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// A PLACE ON THE CURVES OF THE KIND, on one side of the line across: the point of their paths whose X stands nearest to
/// the middle of their run on that side, at least 2 mm from the line and from every point of the sketch - a press near a
/// point takes the point. A construction curve of a figure - the circle of a polygon - is not clicked: it lies too near
/// the sides for a click to tell which is meant.
fn on_side(d: &Drawing, side: Side) -> Option<Point2> {
    let right = side == Side::Right;
    let clear = |p: &Point2| d.points.iter().all(|q| q.at.dist(*p) > 2.0);
    let side = |p: &Point2| clear(p) && if right { p.x > ACROSS[0].x + 2.0 } else { p.x < ACROSS[0].x - 2.0 };
    let all: Vec<Point2> = d.curves.iter().filter(|c| !c.construction).flat_map(|c| c.path.iter().copied()).filter(side).collect();
    let (lo, hi) = all.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
    let mid = (lo + hi) / 2.0;
    all.into_iter().min_by(|a, b| (a.x - mid).abs().total_cmp(&(b.x - mid).abs()))
}

/// The way across the curves at `p`, 1.5 mm to each side: a stroke of the trim drawn through the piece.
fn stroke_through(d: &Drawing, p: Point2) -> [Point2; 2] {
    let path = d.curves.iter().map(|c| &c.path).min_by(|a, b| off_path(p, a).total_cmp(&off_path(p, b))).cloned().unwrap_or_default();
    let k = path.iter().enumerate().min_by(|a, b| a.1.dist(p).total_cmp(&b.1.dist(p))).map_or(0, |(k, _)| k);
    let (a, b) = (path[k.saturating_sub(1)], path[(k + 1).min(path.len() - 1)]);
    let l = a.dist(b).max(1e-9);
    let n = Point2::new(-(b.y - a.y) / l * 1.5, (b.x - a.x) / l * 1.5);
    [Point2::new(p.x - n.x, p.y - n.y), p + n]
}

/// A LINE DRAWN BY HAND from `a` to `b`, the tool put down after it.
fn line(hand: &mut Hand, seg: [Point2; 2]) {
    hand.sk_tool(qymcad_ui_state::SketchShape::Line);
    hand.click2d(seg[0].x, seg[0].y).click2d(seg[1].x, seg[1].y);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_select();
}

/// PRESS the button of `key` in the panel, or fail naming it.
fn press(hand: &mut Hand, key: &str) -> Result<(), String> {
    if hand.press_hint(&crate::i18n::tr(key)) {
        hand.close_window();
        Ok(())
    } else {
        Err(format!("no button {key} in the panel"))
    }
}

/// EVERY POINT OF `after` LIES WHERE ONE OF THE CURVES `before` LAY: the tool changed no shape.
fn kept_shape(after: &Drawing, before: &[Curve]) -> Result<(), String> {
    let worst = after.curves.iter().flat_map(|c| c.path.iter()).map(|p| off_curves(*p, before)).fold(0.0, f64::max);
    if worst > NEAR {
        return Err(format!("a curve left stands {worst:.3} mm off where the curves lay"));
    }
    Ok(())
}

/// TRIM: the piece under the click or the stroke gone, the far side kept, nothing moved.
fn trim(hand: &mut Hand, edit: Edit, before: &Drawing) -> Result<(), String> {
    let (cut, kept) = (on_side(before, Side::Right).ok_or("no piece right of the line")?, on_side(before, Side::Left).ok_or("nothing left of the line")?);
    press(hand, "tb-trim-hint")?;
    if edit == Edit::TrimClick {
        hand.click2d(cut.x, cut.y);
    } else {
        let [a, b] = stroke_through(before, cut);
        hand.drag2d((a.x, a.y), (b.x, b.y));
    }
    let after = drawing(hand.app);
    kept_shape(&after, &before.curves)?;
    if off_curves(cut, &after.curves) < 0.2 {
        return Err(format!("the piece at ({:.2}, {:.2}) is still there", cut.x, cut.y));
    }
    if off_curves(kept, &after.curves) > NEAR {
        return Err(format!("the far side at ({:.2}, {:.2}) went with the piece", kept.x, kept.y));
    }
    Ok(())
}

/// BREAK: one curve more, the shape the same, an end of a curve where the click was.
fn break_at(hand: &mut Hand, before: &Drawing) -> Result<(), String> {
    let at = on_side(before, Side::Right).ok_or("no curve right of the line")?;
    press(hand, "tb-break-hint")?;
    hand.click2d(at.x, at.y);
    let after = drawing(hand.app);
    kept_shape(&after, &before.curves)?;
    if after.curves.len() != before.curves.len() + 1 {
        return Err(format!("{} curves, not {}", after.curves.len(), before.curves.len() + 1));
    }
    let end = after.curves.iter().flat_map(|c| [c.path[0], c.path[c.path.len() - 1]]).map(|e| e.dist(at)).fold(f64::MAX, f64::min);
    if end > 0.5 {
        return Err(format!("no end of a curve where it was clicked: the nearest is {end:.3} mm off"));
    }
    Ok(())
}

/// Whether `p` lies on the way the curve goes on past its end: the line itself, the circle of the arc, the straight on
/// from the end of a spline.
fn on_the_way(kind: &Kind, path: &[Point2], p: Point2) -> bool {
    let (n, end) = (path.len(), path[path.len() - 1]);
    match kind.name {
        "line" => off_path(p, &[path[0] + (path[0] - end), end + (end - path[0])]) < NEAR,
        "arc" => {
            let c = centre_of([path[0], path[n / 2], end]);
            (p.dist(c) - path[0].dist(c)).abs() < NEAR
        }
        // an arc of an ellipse goes on round its ellipse: 8 by 4 about the place of the kind
        "arc of an ellipse" => (((p.x - AT.x) / 8.0).powi(2) + ((p.y - AT.y) / 4.0).powi(2) - 1.0).abs() < 2.0 * NEAR / 4.0,
        // a spline goes on straight: the point lies on the line from its end, which leaves it the way its last piece
        // runs - within the turn of that piece, the points of a spline being laid out 0.005 mm off it
        _ => {
            let run = p - end;
            let last = end - path[n - 2];
            let cross = (run.x * last.y - run.y * last.x).abs() / (run.len() * last.len()).max(1e-12);
            run.x * last.x + run.y * last.y > 0.0 && cross < 0.1
        }
    }
}

/// EXTEND: the end of the kind on the line it was taken to, the curve kept and gone on along its own way.
fn extend(hand: &mut Hand, kind: &Kind, edit: Edit, before: &Drawing) -> Result<(), String> {
    let s = stretch(kind).ok_or("no end")?;
    let to = [about(s.to[0]), about(s.to[1])];
    line(hand, to);
    let curve = before.curves.iter().find(|c| c.path.iter().any(|p| p.dist(about(s.end)) < 0.5)).ok_or("no curve ends at the end to stretch")?;
    let near_end = share_along(&curve.path, if curve.path[0].dist(about(s.end)) < 0.5 { 0.15 } else { 0.85 });
    let target = along(to, 0.5);
    if edit == Edit::ExtendPicked {
        hand.sk_select();
        hand.click2d(near_end.x, near_end.y);
        press(hand, "tb-extend-hint")?;
        hand.hover2d(target.x, target.y);
        hand.key(egui::Key::Enter);
    } else {
        press(hand, "tb-extend-hint")?;
        hand.click2d(near_end.x, near_end.y);
        hand.click2d(target.x, target.y);
    }
    let after = drawing(hand.app);
    // the curves of the kind: every curve but the line it was taken to
    let mine: Vec<&Curve> = after.curves.iter().filter(|c| !c.path.iter().all(|p| off_path(*p, &to) < NEAR)).collect();
    let reaches = mine.iter().flat_map(|c| [c.path[0], c.path[c.path.len() - 1]]).any(|e| off_path(e, &to) < NEAR);
    if !reaches {
        return Err("no end of the curve lies on the line it was taken to".into());
    }
    if curve.path.iter().any(|p| mine.iter().all(|c| off_path(*p, &c.path) > NEAR)) {
        return Err("a part of the curve went".into());
    }
    let ordered = if curve.path[0].dist(about(s.end)) < 0.5 { curve.path.iter().rev().copied().collect() } else { curve.path.clone() };
    let astray = mine.iter().flat_map(|c| c.path.iter()).filter(|p| off_path(**p, &curve.path) > NEAR && !on_the_way(kind, &ordered, **p)).count();
    if astray > 0 {
        return Err(format!("{astray} points of the stretch go off the way of the curve"));
    }
    Ok(())
}

/// THE CELLS OF ONE TOOL: every kind; the failures said together.
fn edit_cells(edit: Edit) {
    let mut failures = Vec::new();
    let mut cells = 0;
    for kind in KINDS {
        if senseless(kind, edit).is_some() {
            continue;
        }
        cells += 1;
        cell(&mut failures, format!("{edit:?} on {}", kind.name), || {
            let mut app = laid(kind, &[AT])?;
            let mut hand = Hand::new(&mut app);
            if matches!(edit, Edit::ExtendPicked | Edit::ExtendToolFirst) {
                let before = drawing(hand.app);
                return extend(&mut hand, kind, edit, &before).map_err(|why| format!("{why}; the status line says {:?}", hand.app.status));
            }
            line(&mut hand, ACROSS);
            let before = drawing(hand.app);
            let done = if edit == Edit::Break { break_at(&mut hand, &before) } else { trim(&mut hand, edit, &before) };
            done.map_err(|why| format!("{why}; the status line says {:?}", hand.app.status))
        });
    }
    said(&format!("{edit:?}"), cells, &failures);
}

#[test]
fn trim_by_a_click_takes_every_kind() {
    edit_cells(EDITS[0]);
}

#[test]
fn trim_by_a_stroke_takes_every_kind() {
    edit_cells(EDITS[1]);
}

#[test]
fn break_takes_every_kind() {
    edit_cells(EDITS[2]);
}

#[test]
fn extend_picked_first_takes_every_kind() {
    edit_cells(EDITS[3]);
}

#[test]
fn extend_taken_first_takes_every_kind() {
    edit_cells(EDITS[4]);
}
