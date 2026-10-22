//! THE TOOLS OVER WHAT IS PICKED: construction, delete, move, copy, rotate, mirror, the two patterns and the offset, on
//! every kind by every way. What each must leave is told by the curves as points: the same curves moved, turned,
//! mirrored or repeated, the copies of the offset at its distance.
use super::*;

/// A TOOL OF THE GROUP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Construction,
    Delete,
    Move,
    Copy,
    Rotate,
    Mirror,
    LinearPattern,
    CircularPattern,
    Offset,
}

/// The centre the turn and the circular pattern go about: a clear place of the sheet below and left of the drawing.
const CENTRE: Point2 = Point2 { x: 20.0, y: 10.0 };

/// How far the move and the copy take the drawing, along X.
const SHIFT: f64 = 30.0;

/// The base point of the move and the copy: a clear place of the sheet on the grid, below the drawing, so that the click
/// snaps to nothing but the grid and the way is exactly `SHIFT`.
const BASE: Point2 = Point2 { x: 40.0, y: 14.0 };

/// The distance the offset opens with on its bar, in millimetres.
const OFFSET: f64 = 3.0;

/// WHY A CELL MAKES NO SENSE, when it does not: what the window does instead.
fn senseless(kind: &Kind, way: Way, tool: Tool) -> Option<&'static str> {
    match (way, tool) {
        (Way::ToolFirst, Tool::Construction) => Some("with nothing picked X and its button switch the drawing mode"),
        (Way::ToolFirst, Tool::Delete) => Some("Delete takes what is picked; with nothing picked there is nothing to take away"),
        (Way::Crossing, Tool::Construction) => None,
        (Way::Crossing, _) => Some("a crossing box picks a part of a figure; the cell checks the pick, the tools over a whole are the other ways"),
        _ if kind.name == "point" && matches!(tool, Tool::Construction | Tool::Offset) => Some("a point has no construction form and no side to be offset to"),
        _ => None,
    }
}

/// The middle of every point the curves are laid out by, or of the points of a drawing of points.
fn middle(d: &Drawing) -> Point2 {
    let all: Vec<Point2> = if d.curves.is_empty() { d.points.iter().map(|p| p.at).collect() } else { d.curves.iter().flat_map(|c| c.path.iter().copied()).collect() };
    let n = all.len().max(1) as f64;
    Point2::new(all.iter().map(|p| p.x).sum::<f64>() / n, all.iter().map(|p| p.y).sum::<f64>() / n)
}

/// A turn by `deg` about `c`.
fn turned(c: Point2, deg: f64) -> impl Fn(Point2) -> Point2 {
    let (s, k) = deg.to_radians().sin_cos();
    move |p: Point2| Point2::new(c.x + (p.x - c.x) * k - (p.y - c.y) * s, c.y + (p.x - c.x) * s + (p.y - c.y) * k)
}

/// THE SKETCH BEFORE A TOOL AND AFTER IT, and what the tool was given.
struct Change {
    before: Drawing,
    after: Drawing,
    picked: Vec<qymcad_ui_state::SketchItem>,
}

/// THE CURVES OR POINTS WANTED after the tool: the drawing before, each of `maps` applied to it, all together.
fn wanted(change: &Change, maps: &[&dyn Fn(Point2) -> Point2]) -> Result<(), String> {
    let Change { before, after, .. } = change;
    if before.curves.is_empty() {
        let want: Vec<Point2> = maps.iter().flat_map(|f| before.points.iter().map(|p| f(p.at))).collect();
        return points_match(&want, after);
    }
    let want: Vec<Want> = maps.iter().flat_map(|f| mapped(before, f)).collect();
    matches(&want, after)
}

/// The curves of `after` that do not lie on a curve of `before`: what the tool added.
fn added(change: &Change) -> Vec<Curve> {
    let Change { before, after, .. } = change;
    let on = |a: &[Point2], b: &[Point2]| a.iter().all(|p| off_path(*p, b) < NEAR);
    after.curves.iter().filter(|g| !before.curves.iter().any(|c| c.kind == g.kind && on(&c.path, &g.path) && on(&g.path, &c.path))).cloned().collect()
}

/// The kind of curve an offset of `kind` is: the same, but for an ellipse, whose offset is no ellipse.
fn offset_kind(kind: &'static str) -> &'static str {
    match kind {
        "ellipse" => "closed spline",
        "arc of an ellipse" => "open spline",
        k => k,
    }
}

/// WHAT IS PICKED IS WHAT THE WAY TOOK: every curve of the drawing (every point of a drawing of points), or what the
/// crossing box touched and nothing else.
fn pick_took(before: &Drawing, way: Way, got: &[qymcad_ui_state::SketchItem]) -> Result<(), String> {
    let of_points = before.curves.is_empty();
    let curves: Vec<qymcad_ui_state::SketchItem> = got.iter().copied().filter(|p| of_points || p.kind != qymcad_ui_state::ItemKind::Point).collect();
    let every: Vec<qymcad_ui_state::SketchItem> = if of_points { before.points.iter().map(|p| p.key).collect() } else { before.curves.iter().map(|c| c.key).collect() };
    let want = if way == Way::Crossing { crossed(before) } else { every };
    let missing = want.iter().filter(|w| !curves.contains(w)).count();
    let extra = if way == Way::Crossing { curves.iter().filter(|c| !want.contains(c)).count() } else { 0 };
    if missing > 0 || extra > 0 {
        return Err(format!("the pick missed {missing} of {} curves and took {extra} it should not", want.len()));
    }
    Ok(())
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

/// Clicks on every spot, the first plain and the rest with Shift.
fn click_all(hand: &mut Hand, at: &[Point2]) {
    for (k, p) in at.iter().enumerate() {
        if k == 0 {
            hand.click2d(p.x, p.y);
        } else {
            hand.shift_click2d(p.x, p.y);
        }
    }
}

/// THE TOOL WORKED BY HAND on what is picked, or, taken first, on the spots clicked after it.
fn work(hand: &mut Hand, tool: Tool, way: Way, at: &[Point2]) -> Result<(), String> {
    let to = Point2::new(BASE.x + SHIFT, BASE.y);
    let first_tool = way == Way::ToolFirst;
    match tool {
        Tool::Construction => {
            hand.key(egui::Key::X);
        }
        Tool::Delete => {
            hand.key(egui::Key::Delete);
        }
        Tool::Move | Tool::Copy => {
            hand.sk_move_tool(if tool == Tool::Move { qymcad_ui_state::MoveKind::Move } else { qymcad_ui_state::MoveKind::Copy });
            if first_tool {
                click_all(hand, at);
            }
            hand.click2d(BASE.x, BASE.y).click2d(to.x, to.y);
        }
        Tool::Rotate => {
            hand.sk_move_tool(qymcad_ui_state::MoveKind::Rotate);
            if first_tool {
                click_all(hand, at);
            }
            hand.mouse2d(CENTRE.x, CENTRE.y);
            hand.frame(Vec::new());
            hand.type_text("90").key(egui::Key::Enter);
        }
        Tool::Mirror => {
            press(hand, "tb-mirror-sketch-hint")?;
            if first_tool {
                click_all(hand, at);
                press(hand, "tb-mirror-sketch-hint")?;
            }
            hand.click2d(0.0, 30.0); // the Y axis
        }
        Tool::LinearPattern => {
            press(hand, "tb-lin-array-hint")?;
            if first_tool {
                click_all(hand, at);
            }
            hand.key(egui::Key::Enter);
        }
        Tool::CircularPattern => {
            press(hand, "tb-circ-array-hint")?;
            if first_tool {
                click_all(hand, at);
            }
            hand.click2d(CENTRE.x, CENTRE.y).key(egui::Key::Enter);
        }
        Tool::Offset => {
            press(hand, "tb-offset-hint")?;
            if first_tool {
                click_all(hand, at);
            }
            let b = bounds(&drawing(hand.app));
            hand.hover2d((b.lo.x + b.hi.x) / 2.0, b.hi.y + 6.0).key(egui::Key::Enter);
        }
    }
    hand.close_window();
    Ok(())
}

/// WHAT THE TOOL LEFT, against what it should.
fn left(tool: Tool, change: &Change) -> Result<(), String> {
    let Change { before, after, .. } = change;
    let same = |p: Point2| p;
    match tool {
        Tool::Construction => {
            let real = after.curves.iter().filter(|c| change.picked.contains(&c.key) && !c.construction).count();
            if real > 0 {
                return Err(format!("{real} of {} curves picked are not construction geometry", change.picked.len()));
            }
            wanted(change, &[&same])
        }
        Tool::Delete => {
            if after.curves.is_empty() && after.points.is_empty() {
                Ok(())
            } else {
                Err(format!("{} curves and {} points stay", after.curves.len(), after.points.len()))
            }
        }
        Tool::Move | Tool::Copy => {
            let went = middle(after) - middle(before);
            let d = if tool == Tool::Copy { Point2::new(went.x * 2.0, went.y * 2.0) } else { went };
            if d.dist(Point2::new(SHIFT, 0.0)) > NEAR {
                return Err(format!("the drawing went by ({:.3}, {:.3}), not ({SHIFT}, 0)", d.x, d.y));
            }
            let by = move |p: Point2| p + d;
            if tool == Tool::Copy {
                wanted(change, &[&same, &by])
            } else {
                wanted(change, &[&by])
            }
        }
        Tool::Rotate => wanted(change, &[&turned(CENTRE, 90.0)]),
        Tool::Mirror => wanted(change, &[&same, &|p: Point2| Point2::new(-p.x, p.y)]),
        Tool::LinearPattern => wanted(change, &[&same, &|p: Point2| p + Point2::new(20.0, 0.0), &|p: Point2| p + Point2::new(40.0, 0.0)]),
        Tool::CircularPattern => wanted(change, &[&same, &turned(CENTRE, 120.0), &turned(CENTRE, 240.0)]),
        Tool::Offset => {
            // a construction curve gives a construction copy, not counted; a spline bending tighter than the distance
            // gives several pieces, its loop cut away at the corner where they meet
            let new: Vec<Curve> = added(change).into_iter().filter(|c| !c.construction).collect();
            let mut want: Vec<&'static str> = before.curves.iter().filter(|c| !c.construction).map(|c| offset_kind(c.kind)).collect();
            let mut got: Vec<&'static str> = new.iter().map(|c| c.kind).collect();
            want.sort_unstable();
            got.sort_unstable();
            let pieces = |of: &[&'static str]| -> Vec<&'static str> {
                let mut kinds: Vec<&'static str> = of.iter().copied().filter(|k| k.ends_with("spline")).collect();
                kinds.dedup();
                kinds
            };
            let others = |of: &[&'static str]| -> Vec<&'static str> { of.iter().copied().filter(|k| !k.ends_with("spline")).collect() };
            let splines_enough = got.iter().filter(|k| k.ends_with("spline")).count() >= want.iter().filter(|k| k.ends_with("spline")).count();
            if others(&want) != others(&got) || pieces(&want) != pieces(&got) || !splines_enough {
                return Err(format!("the offset added {got:?}, not {want:?}"));
            }
            // the middle of each copy stands at the distance from the source; its ends, where copies meet past a sharp
            // corner of the source, stand farther, never nearer
            let from_source = |p: &Point2| before.curves.iter().filter(|c| !c.construction).map(|c| off_path(*p, &c.path)).fold(f64::MAX, f64::min);
            let middles = new.iter().flat_map(|c| [0.3, 0.4, 0.5, 0.6, 0.7].map(|t| share_along(&c.path, t)));
            let worst = middles.map(|p| (from_source(&p) - OFFSET).abs()).fold(0.0, f64::max);
            let nearest = new.iter().flat_map(|c| c.path.iter()).map(from_source).fold(f64::MAX, f64::min);
            if worst > NEAR || nearest < OFFSET - NEAR {
                return Err(format!("the middle of a copy stands {worst:.3} mm off the distance {OFFSET}, and a point of it {nearest:.3} from the source"));
            }
            Ok(())
        }
    }
}

/// THE CELLS OF ONE TOOL: every kind, every way; the failures said together.
fn tool_cells(tool: Tool) {
    let mut failures = Vec::new();
    let mut cells = 0;
    for kind in KINDS {
        for way in WAYS {
            if senseless(kind, way, tool).is_some() {
                continue;
            }
            cells += 1;
            cell(&mut failures, format!("{tool:?} on {} by {way:?}", kind.name), || {
                let mut app = laid(kind, &places(way))?;
                let before = drawing(&app);
                let mut hand = Hand::new(&mut app);
                let mut given = before.curves.iter().map(|c| c.key).collect();
                if way != Way::ToolFirst {
                    pick(&mut hand, way);
                    given = picked(hand.app);
                    pick_took(&before, way, &given)?;
                }
                let at = spots(&mut hand, &before);
                work(&mut hand, tool, way, &at)?;
                let change = Change { before, after: drawing(&app), picked: given };
                left(tool, &change).map_err(|why| format!("{why}; the status line says {:?}", app.status))
            });
        }
    }
    said(&format!("{tool:?} over what is picked"), cells, &failures);
}

#[test]
fn construction_takes_every_kind() {
    tool_cells(Tool::Construction);
}

#[test]
fn delete_takes_every_kind() {
    tool_cells(Tool::Delete);
}

#[test]
fn move_takes_every_kind() {
    tool_cells(Tool::Move);
}

#[test]
fn copy_takes_every_kind() {
    tool_cells(Tool::Copy);
}

#[test]
fn rotate_takes_every_kind() {
    tool_cells(Tool::Rotate);
}

#[test]
fn mirror_takes_every_kind() {
    tool_cells(Tool::Mirror);
}

#[test]
fn linear_pattern_takes_every_kind() {
    tool_cells(Tool::LinearPattern);
}

#[test]
fn circular_pattern_takes_every_kind() {
    tool_cells(Tool::CircularPattern);
}

#[test]
fn offset_takes_every_kind() {
    tool_cells(Tool::Offset);
}
