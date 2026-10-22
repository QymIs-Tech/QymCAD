//! THE CORNERS: a fillet and a chamfer of both forms between every pair of kinds - at the vertex they share, named by a
//! click on the vertex, by a click on one of the curves near it, or by the two curves picked before the tool - and at the
//! crossing of every kind with a line, the two picked with Shift and the quarter clicked. A cell reads the curves as
//! points: a fillet is an arc of the radius typed that touches both curves where it meets them; a chamfer is a line
//! with its ends on the two curves - a cut 2 long, its ends as far from the corner, or ends 2 and 3 from it; the corner
//! itself is cut away.
//!
//! A click on a curve names the corner of it nearest the click, and a second click on the same corner takes it away: two
//! curves meeting at a vertex are named by a click on one of them, not on each.
use super::*;

/// The vertex every pair of curves meets at.
const V: Point2 = Point2 { x: 40.0, y: 30.0 };

/// Where the upright line crossing a kind stands, for a corner at the crossing: 4 mm right of its place, clear of the
/// corners of the figures - 3 mm off the junction of a slot, 3 mm off the corners of a polygon.
fn cross_x(kind: &Kind) -> f64 {
    V.x + match kind.name {
        "slot" => 2.0,
        "polygon" => 5.0,
        _ => 4.0,
    }
}

/// A TOOL OF THE GROUP, with the values typed into it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    /// a fillet of radius 2
    Fillet,
    /// a chamfer 2 along both curves
    Chamfer,
    /// a chamfer 2 along the first curve and 3 along the second
    ChamferTwo,
}

const TOOLS: [Tool; 3] = [Tool::Fillet, Tool::Chamfer, Tool::ChamferTwo];

/// HOW A CORNER IS NAMED.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Name {
    /// a click on the vertex the curves share
    Vertex,
    /// a click on the first curve near the vertex, the tool in hand
    OneCurve,
    /// the two curves picked before the tool
    PickedBefore,
}

const NAMES: [Name; 3] = [Name::Vertex, Name::OneCurve, Name::PickedBefore];

/// The kinds that end at a vertex: they come to it from the left and leave it to the right.
const OPEN: [&str; 4] = ["line", "arc", "open spline", "arc of an ellipse"];

/// Whether a tool of the panel draws `kind` into a corner.
fn drawable(kind: &str) -> bool {
    matches!(kind, "line" | "arc" | "open spline" | "arc of an ellipse")
}

/// AN ARC OF AN ELLIPSE AS A PERSON DRAWS ONE, relative to the vertex: an ellipse by its centre, the end of its axis and
/// its width; a line cutting it through the vertex; a click of the trim on the part to go; a click on the line and Delete.
struct Cap {
    centre: Point2,
    axis: Point2,
    width: Point2,
    cut: [Point2; 2],
    trim_at: Point2,
    line_at: Point2,
}

fn draw_cap(h: &mut Hand, cap: &Cap) {
    let at = |h: &mut Hand, p: Point2| {
        h.click2d(V.x + p.x, V.y + p.y);
    };
    h.sk_tool(qymcad_ui_state::SketchShape::Ellipse);
    for p in [cap.centre, cap.axis, cap.width] {
        at(h, p);
    }
    h.key(egui::Key::Escape);
    h.sk_tool(qymcad_ui_state::SketchShape::Line);
    for p in cap.cut {
        at(h, p);
    }
    h.key(egui::Key::Escape).key(egui::Key::Escape);
    assert!(h.press_hint(&crate::i18n::tr("tb-trim-hint")), "no trim button");
    h.close_window();
    at(h, cap.trim_at);
    h.key(egui::Key::Escape);
    h.sk_select();
    at(h, cap.line_at);
    h.key(egui::Key::Delete);
}

/// A CURVE DRAWN BY HAND ARRIVING AT THE VERTEX from the left; a kind no tool draws is left out (`drawable`).
fn arrive(h: &mut Hand, kind: &str) {
    let at = |h: &mut Hand, x: f64, y: f64| {
        h.click2d(V.x + x, V.y + y);
    };
    match kind {
        "line" => {
            h.sk_tool(qymcad_ui_state::SketchShape::Line);
            at(h, -12.0, 0.0);
            at(h, 0.0, 0.0);
        }
        // round a centre 6 left of the vertex, from below it up to the vertex
        "arc" => {
            h.sk_tool(qymcad_ui_state::SketchShape::Arc);
            at(h, -6.0, 0.0);
            at(h, -6.0, -6.0);
            at(h, 0.0, 0.0);
        }
        // arriving from above, at 80 deg to the line leaving: arriving from below it ran on nearly straight into it, a
        // turn of 26 deg, and a fillet of 2 there was an arc of 0.6 mm standing within the reach of the curves it joins
        "open spline" => {
            h.sk_tool(qymcad_ui_state::SketchShape::Spline);
            at(h, -12.0, 3.0);
            at(h, -8.0, -1.0);
            at(h, -4.0, 2.0);
            h.double_click2d(V.x, V.y);
        }
        // the end of an ellipse 10 by 5 left of a line upright through the vertex, below and left of it: a line 6 from
        // the centre across the long axis meets the ellipse 5 * sqrt(1 - 0.6^2) = 4 off it, and every click stands on
        // the grid of 1 mm the clicks are drawn to
        "arc of an ellipse" => {
            let c = Point2::new(6.0, -4.0);
            let cap = Cap {
                centre: c,
                axis: c + Point2::new(10.0, 0.0),
                width: c + Point2::new(0.0, 5.0),
                cut: [Point2::new(0.0, 6.0), Point2::new(0.0, -12.0)],
                trim_at: c + Point2::new(10.0, 0.0),
                line_at: Point2::new(0.0, 4.0),
            };
            draw_cap(h, &cap);
        }
        _ => return,
    }
    h.key(egui::Key::Escape).key(egui::Key::Escape);
    h.sk_select();
}

/// A CURVE DRAWN BY HAND LEAVING THE VERTEX to the upper right, its first point clicked on the vertex.
fn leave(h: &mut Hand, kind: &str) {
    let at = |h: &mut Hand, x: f64, y: f64| {
        h.click2d(V.x + x, V.y + y);
    };
    match kind {
        "line" => {
            h.sk_tool(qymcad_ui_state::SketchShape::Line);
            at(h, 0.0, 0.0);
            at(h, 8.0, 10.0);
        }
        // leaving the vertex the way the line does, at 51 deg, round a centre 6 to its left, through 60 deg
        "arc" => {
            h.sk_tool(qymcad_ui_state::SketchShape::Arc);
            at(h, -4.68, 3.72);
            at(h, 0.0, 0.0);
            at(h, 0.9, 5.92);
        }
        // leaving the vertex at 50 deg, as the line does: through (3, 5) the spline of continuous curvature left it at 72
        // deg, 18 deg off the arc arriving upright, and a fillet of 2 there was an arc of 0.6 mm
        "open spline" => {
            h.sk_tool(qymcad_ui_state::SketchShape::Spline);
            at(h, 0.0, 0.0);
            at(h, 4.0, 4.0);
            at(h, 7.0, 6.0);
            h.double_click2d(V.x + 10.0, V.y + 11.0);
        }
        // the lower end of an upright ellipse 10 by 5 above a line level with the vertex, leaving it down and right; the
        // whole ellipse stands clear of every curve arriving from the left
        "arc of an ellipse" => {
            let c = Point2::new(4.0, 6.0);
            let cap = Cap {
                centre: c,
                axis: c + Point2::new(0.0, 10.0),
                width: c + Point2::new(5.0, 0.0),
                cut: [Point2::new(14.0, 0.0), Point2::new(0.0, 0.0)],
                trim_at: c + Point2::new(0.0, 10.0),
                line_at: Point2::new(12.0, 0.0),
            };
            draw_cap(h, &cap);
        }
        _ => return,
    }
    h.key(egui::Key::Escape).key(egui::Key::Escape);
    h.sk_select();
}

/// A CORNER OF THE MATRIX: two curves meeting at a vertex, or a figure with a corner of its own.
struct Pair {
    name: String,
    /// the figure drawn, when the corner is one of a figure's
    figure: Option<&'static Kind>,
    first: &'static str,
    second: &'static str,
}

fn pairs() -> Vec<Pair> {
    let mut out: Vec<Pair> = OPEN.iter().flat_map(|a| OPEN.iter().map(move |b| Pair { name: format!("{a} to {b}"), figure: None, first: a, second: b })).collect();
    for f in KINDS.iter().filter(|k| matches!(k.name, "rectangle" | "polygon")) {
        out.push(Pair { name: format!("a corner of a {}", f.name), figure: Some(f), first: "line", second: "line" });
    }
    out
}

/// A DOCUMENT WITH A PAIR DRAWN, and the corner of the pair.
struct Drawn {
    app: App,
    corner: Point2,
}

/// A NEW DOCUMENT with the pair drawn by hand; answers the corner, or why the pair could not be drawn.
fn drawn(pair: &Pair) -> Result<Drawn, String> {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    let corner = match pair.figure {
        Some(f) => {
            (f.draw.ok_or("no tool draws the figure")?)(&mut hand, V);
            hand.key(egui::Key::Escape);
            hand.sk_select();
            // the lower left corner of the rectangle, the corner at the right of the polygon
            if f.name == "rectangle" {
                Point2::new(V.x - 8.0, V.y - 5.0)
            } else {
                Point2::new(V.x + 7.0, V.y)
            }
        }
        None => {
            if let Some(missing) = [pair.first, pair.second].into_iter().find(|k| !drawable(k)) {
                return Err(format!("no tool draws {missing}"));
            }
            arrive(&mut hand, pair.first);
            leave(&mut hand, pair.second);
            V
        }
    };
    let ends = drawing(&app).curves.iter().filter(|c| !c.construction && [c.path[0], c.path[c.path.len() - 1]].iter().any(|e| e.dist(corner) < NEAR)).count();
    if ends != 2 {
        return Err(format!("{} curves end at the corner, not two", ends));
    }
    Ok(Drawn { app, corner })
}

/// THE TWO CURVES AT THE CORNER, the one drawn first first.
fn at_corner(d: &Drawing, corner: Point2) -> Vec<Curve> {
    d.curves.iter().filter(|c| !c.construction && [c.path[0], c.path[c.path.len() - 1]].iter().any(|e| e.dist(corner) < NEAR)).cloned().collect()
}

/// A place on the curve along it from the corner, from 1 mm to 3 mm or 0.45 of its length, where a click takes the curve
/// and names this corner, the nearest of its ends: of those, the one farthest from every point of the sketch. At a fixed
/// 3 mm the click stood 1.5 mm off a node of a spline, and at a quarter of the 6.2 mm of an arc 1.4 mm off the corner,
/// and the point there was picked in place of the curve.
fn off_corner(c: &Curve, corner: Point2, points: &[Dot]) -> Point2 {
    let path: Vec<Point2> = if c.path[0].dist(corner) < NEAR { c.path.clone() } else { c.path.iter().rev().copied().collect() };
    let length: f64 = path.windows(2).map(|w| w[0].dist(w[1])).sum();
    let (lo, hi) = ((1.0 / length).min(0.2), (3.0 / length).min(0.45));
    let clear = |p: &Point2| points.iter().map(|q| q.at.dist(*p)).fold(f64::MAX, f64::min);
    (0..=20).map(|k| share_along(&path, lo + (hi - lo) * k as f64 / 20.0)).max_by(|a, b| clear(a).total_cmp(&clear(b))).unwrap_or(corner)
}

/// The direction of a path at the point of it nearest to `p`: the segment of the path that point lies on.
fn direction(path: &[Point2], p: Point2) -> Point2 {
    let k = path.windows(2).enumerate().min_by(|a, b| off_path(p, a.1).total_cmp(&off_path(p, b.1))).map_or(0, |(k, _)| k);
    let (a, b) = (path[k], path[(k + 1).min(path.len() - 1)]);
    let l = a.dist(b).max(1e-12);
    Point2::new((b.x - a.x) / l, (b.y - a.y) / l)
}

/// THE DIRECTION OF A CURVE AT `p`: square to the radius on a round curve, whose laid out points stand a 96th of a turn
/// apart and would tilt it by half of that, 1.9 deg; along a line, the segment `p` lies on; on any other curve square to
/// the radius of the circle through the three laid out points about `p` - the segment alone tilted the way an arc of an
/// ellipse runs by 1.25 deg.
fn tangent(c: &Curve, p: Point2) -> Point2 {
    let square_to = |centre: Point2| {
        let (radial, r) = (p - centre, p.dist(centre));
        Point2::new(-radial.y / r, radial.x / r)
    };
    let n = c.path.len();
    if matches!(c.kind, "arc" | "circle") {
        return square_to(centre_of([c.path[0], c.path[n / 3], c.path[2 * n / 3]]));
    }
    if c.kind == "line" || n < 3 {
        return direction(&c.path, p);
    }
    let k = (0..n).min_by(|a, b| c.path[*a].dist(p).total_cmp(&c.path[*b].dist(p))).unwrap_or(0).clamp(1, n - 2);
    let [a, m, b] = [c.path[k - 1], c.path[k], c.path[k + 1]];
    let bend = (m.x - a.x) * (b.y - a.y) - (m.y - a.y) * (b.x - a.x);
    if bend.abs() < 1e-12 {
        return direction(&c.path, p);
    }
    square_to(centre_of([a, m, b]))
}

/// The point of a path nearest to `p`.
fn onto(p: Point2, path: &[Point2]) -> Point2 {
    path.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let l2 = a.dist(b).powi(2);
            let t = if l2 > 1e-18 { (((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2).clamp(0.0, 1.0) } else { 0.0 };
            along([a, b], t)
        })
        .min_by(|a, b| a.dist(p).total_cmp(&b.dist(p)))
        .unwrap_or(p)
}

/// The angle between two directions, in degrees, either way along them.
fn apart(ways: [Point2; 2]) -> f64 {
    let [a, b] = ways;
    (a.x * b.x + a.y * b.y).abs().min(1.0).acos().to_degrees()
}

/// TYPE THE VALUES of the tool into the field at the corner and press Enter.
fn answer(hand: &mut Hand, tool: Tool) {
    hand.frame(Vec::new());
    hand.type_text("2");
    if tool == Tool::ChamferTwo {
        hand.key(egui::Key::Tab).type_text("3");
    }
    hand.key(egui::Key::Enter);
}

/// PRESS the button of the tool, and the word of its mode for the chamfer of two distances.
fn take(hand: &mut Hand, tool: Tool) -> Result<(), String> {
    let key = if tool == Tool::Fillet { "tb-fillet-sketch-hint" } else { "tb-chamfer-sketch-hint" };
    if !hand.press_hint(&crate::i18n::tr(key)) {
        return Err(format!("no button {key}"));
    }
    if tool == Tool::ChamferTwo && !hand.press_word(&crate::i18n::tr("cmd-two-distances"), egui::pos2(400.0, 0.0)) {
        return Err("no mode of two distances on the bar of the chamfer".into());
    }
    hand.close_window();
    Ok(())
}

/// A CORNER BEFORE THE TOOL: the drawing, the two curves that make it, and where they meet.
struct Corner {
    before: Drawing,
    sides: Vec<Curve>,
    at: Point2,
}

/// WHAT THE TOOL MADE OF THE CORNER: a fillet arc of radius 2 touching both sides, or a chamfer line with its ends one
/// on each side as typed; the corner itself cut away.
fn made(tool: Tool, corner: &Corner, after: &Drawing) -> Result<(), String> {
    let Corner { before, sides, at: corner } = corner;
    let (sides, corner) = (&sides[..], *corner);
    let kind = if tool == Tool::Fillet { "arc" } else { "line" };
    // what the tool laid: a curve of its kind that lies on no curve there was before
    let Some(cut) = after.curves.iter().find(|c| c.kind == kind && c.path.iter().any(|p| off_curves(*p, &before.curves) > NEAR)) else {
        return Err(format!("no new {kind} at the corner; the sketch holds {:?}", after.curves.iter().map(|c| c.kind).collect::<Vec<_>>()));
    };
    let ends = [cut.path[0], cut.path[cut.path.len() - 1]];
    let on: Vec<usize> = ends.iter().filter_map(|e| sides.iter().position(|s| off_path(*e, &s.path) < NEAR)).collect();
    if on.len() != 2 || on[0] == on[1] {
        return Err(format!("the ends of the {kind} do not lie one on each curve: {ends:?}"));
    }
    // the stretch of each side from the corner to the cut is gone - no curve stands on the side half way along it; at a
    // crossing the halves past it stay. Looked for a quarter of the stretch around, it found the half of the upright
    // line in the next quarter of a crossing of a polygon, 30 deg off and 0.134 mm away
    let others: Vec<&Curve> = after.curves.iter().filter(|c| c.key != cut.key && !c.construction).collect();
    for (e, side) in ends.iter().zip(&on) {
        let half = onto(along([corner, *e], 0.5), &sides[*side].path);
        if others.iter().any(|c| off_path(half, &c.path) < NEAR) {
            return Err(format!("the side between the corner and the cut is still there at ({:.2}, {:.2})", half.x, half.y));
        }
    }
    match tool {
        Tool::Fillet => {
            let n = cut.path.len();
            let c = centre_of([cut.path[0], cut.path[n / 2], cut.path[n - 1]]);
            let r = c.dist(cut.path[0]);
            if (r - 2.0).abs() > NEAR {
                return Err(format!("the fillet has a radius of {r:.3}, not 2"));
            }
            for e in ends {
                let side = others.iter().min_by(|a, b| off_path(e, &a.path).total_cmp(&off_path(e, &b.path)));
                let Some(side) = side else { return Err("no curve meets the fillet".into()) };
                let radial = e - c;
                let turn = apart([Point2::new(-radial.y / r, radial.x / r), tangent(side, e)]);
                if turn > 1.0 {
                    return Err(format!("the fillet meets a curve at {turn:.2} deg, not touching it"));
                }
            }
        }
        // the symmetric chamfer is a cut 2 long with its ends as far from the corner; the other, ends 2 and 3 from it
        Tool::Chamfer => {
            let (legs, cut_length) = ([ends[0].dist(corner), ends[1].dist(corner)], ends[0].dist(ends[1]));
            if (legs[0] - legs[1]).abs() > NEAR || (cut_length - 2.0).abs() > NEAR {
                return Err(format!("the chamfer is {cut_length:.3} long with its ends {:.3} and {:.3} from the corner, not 2 long and as far", legs[0], legs[1]));
            }
        }
        Tool::ChamferTwo => {
            let mut legs: Vec<f64> = ends.iter().map(|e| e.dist(corner)).collect();
            legs.sort_by(f64::total_cmp);
            if (legs[0] - 2.0).abs() > NEAR || (legs[1] - 3.0).abs() > NEAR {
                return Err(format!("the chamfer cuts {:.3} and {:.3} from the corner, not 2 and 3", legs[0], legs[1]));
            }
        }
    }
    Ok(())
}

/// ONE CORNER AT A VERTEX, named one way and cut by one tool.
fn at_vertex(pair: &Pair, name: Name, tool: Tool) -> Result<(), String> {
    let Drawn { mut app, corner: at } = drawn(pair)?;
    let before = drawing(&app);
    let sides = at_corner(&before, at);
    let spots = [off_corner(&sides[0], at, &before.points), off_corner(&sides[1], at, &before.points)];
    let corner = Corner { before, sides, at };
    let mut hand = Hand::new(&mut app);
    match name {
        Name::Vertex => {
            take(&mut hand, tool)?;
            hand.click2d(at.x, at.y);
        }
        Name::OneCurve => {
            take(&mut hand, tool)?;
            hand.click2d(spots[0].x, spots[0].y);
        }
        Name::PickedBefore => {
            hand.click2d(spots[0].x, spots[0].y).shift_click2d(spots[1].x, spots[1].y);
            take(&mut hand, tool)?;
        }
    }
    answer(&mut hand, tool);
    made(tool, &corner, &drawing(hand.app)).map_err(|why| format!("{why}; the status line says {:?}", hand.app.status))
}

/// ONE CORNER AT THE CROSSING of `kind` with an upright line, the two picked with Shift far from the crossing and the
/// quarter above and right of the crossing clicked.
fn at_crossing(kind: &Kind, tool: Tool) -> Result<(), String> {
    let x = cross_x(kind);
    let mut app = laid(kind, &[V])?;
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(x, V.y - 14.0).click2d(x, V.y + 14.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_select();
    let before = drawing(hand.app);
    let upright = before.curves.iter().find(|c| c.kind == "line" && c.path.iter().all(|p| (p.x - x).abs() < NEAR)).cloned().ok_or("no upright line")?;
    let own: Vec<Curve> = before.curves.iter().filter(|c| c.key != upright.key && !c.construction).cloned().collect();
    let crossings: Vec<Point2> = own
        .iter()
        .flat_map(|c| {
            c.path.windows(2).filter(|w| (w[0].x - x) * (w[1].x - x) <= 0.0 && (w[0].x - w[1].x).abs() > 1e-12).map(|w| along([w[0], w[1]], (x - w[0].x) / (w[1].x - w[0].x))).collect::<Vec<_>>()
        })
        .collect();
    // the crossing nearest to 3 mm above the place of the kind
    let at = crossings.iter().copied().min_by(|a, b| (a.y - V.y - 3.0).abs().total_cmp(&(b.y - V.y - 3.0).abs())).ok_or("the kind does not cross the upright line")?;
    let curve = own.iter().find(|c| off_path(at, &c.path) < NEAR).cloned().ok_or("no curve at the crossing")?;
    // the curve is clicked where it stands farthest from what the zone of a corner catches - a point two curves end at,
    // a crossing: 2 mm off a corner of a rectangle the click named that corner, and 4.6 mm off the crossing of an arc it
    // cut the crossing before the line was chosen
    let ends: Vec<Point2> = own.iter().chain([&upright]).flat_map(|c| [c.path[0], c.path[c.path.len() - 1]]).collect();
    let corners: Vec<Point2> = ends.iter().copied().filter(|e| ends.iter().filter(|o| o.dist(*e) < NEAR).count() >= 2).collect();
    let clear = |p: &Point2| corners.iter().chain(crossings.iter()).map(|q| q.dist(*p)).fold(f64::MAX, f64::min);
    // within 8 mm of the crossing, in sight once the sheet is brought nearer
    let on_curve = curve.path.iter().copied().filter(|p| p.dist(at) <= 8.0).max_by(|a, b| clear(a).total_cmp(&clear(b))).ok_or("no place to click the curve")?;
    let on_line = Point2::new(x, if at.y > V.y { at.y + 6.0 } else { at.y - 6.0 });
    // THE SHEET BROUGHT NEARER over the crossing, as a person does on a small figure, to four times the scale the matrix
    // opens with: a side of the polygon is 7 mm, and at 6 px to the mm every place on it stood within the zone of a
    // corner or of the crossing, 24 px
    hand.look2d((at.x, at.y)).frame(Vec::new());
    for _ in 0..40 {
        if hand.app.viewing.view.scale >= 24.0 {
            break;
        }
        hand.wheel2d((at.x, at.y), true);
    }
    let corner = Corner { before, sides: vec![curve, upright], at };
    take(&mut hand, tool)?;
    hand.shift_click2d(on_curve.x, on_curve.y).shift_click2d(on_line.x, on_line.y);
    hand.click2d(at.x + 0.8, at.y + 0.8);
    answer(&mut hand, tool);
    made(tool, &corner, &drawing(hand.app)).map_err(|why| format!("at the crossing ({:.2}, {:.2}): {why}; the status line says {:?}", at.x, at.y, hand.app.status))
}

fn corner_cells(tool: Tool) {
    let mut failures = Vec::new();
    let mut cells = 0;
    for pair in pairs() {
        for name in NAMES {
            cells += 1;
            cell(&mut failures, format!("{tool:?} of {} by {name:?}", pair.name), || at_vertex(&pair, name, tool));
        }
    }
    for kind in KINDS.iter().filter(|k| k.name != "point") {
        cells += 1;
        cell(&mut failures, format!("{tool:?} at the crossing of {} and a line", kind.name), || at_crossing(kind, tool));
    }
    said(&format!("{tool:?} of every corner"), cells, &failures);
}

#[test]
fn a_fillet_takes_every_corner() {
    corner_cells(TOOLS[0]);
}

#[test]
fn a_chamfer_takes_every_corner() {
    corner_cells(TOOLS[1]);
}

#[test]
fn a_chamfer_of_two_distances_takes_every_corner() {
    corner_cells(TOOLS[2]);
}
