//! THE SNAPS of a line being drawn, on every kind: its first click brought near an end, a middle, a centre, a quadrant
//! or a crossing of curves lands on it exactly; its second click brought near a round or a free curve from a point
//! outside lands where the line touches it.
use super::*;

/// WHAT A SNAP TAKES the line to.
#[derive(Clone, Copy, Debug)]
enum Feature {
    /// an end of the first curve: the end nearer `near`
    End,
    /// the middle of the first curve
    Middle,
    /// the centre of the first curve
    Centre,
    /// the point of the first curve farthest along a direction - a quadrant of a round curve, an end of an axis
    Quadrant(Point2),
    /// a point of the first shape, the one nearest this place
    Node(Point2),
    /// the crossing of the first curve with the second
    Crossing,
    /// the line from this place touching the first curve
    Touch(Point2),
}

/// A SHAPE OF A CASE, drawn by hand about (40, 30).
type Draw = fn(&mut Hand);

fn put_down(h: &mut Hand) {
    h.key(egui::Key::Escape).key(egui::Key::Escape);
    h.sk_select();
}

fn clicks(h: &mut Hand, tool: qymcad_ui_state::SketchShape, at: &[Point2]) {
    h.sk_tool(tool);
    for q in at {
        h.click2d(q.x, q.y);
    }
    put_down(h);
}

const fn p(at: [f64; 2]) -> Point2 {
    Point2 { x: at[0], y: at[1] }
}

mod shapes {
    use super::*;
    pub fn line(h: &mut Hand) {
        clicks(h, qymcad_ui_state::SketchShape::Line, &[p([32.0, 26.0]), p([48.0, 34.0])]);
    }
    pub fn arc(h: &mut Hand) {
        clicks(h, qymcad_ui_state::SketchShape::Arc, &[p([40.0, 27.0]), p([47.0, 27.0]), p([35.0, 32.0])]);
    }
    pub fn circle(h: &mut Hand) {
        clicks(h, qymcad_ui_state::SketchShape::Circle, &[p([40.0, 30.0]), p([46.0, 30.0])]);
    }
    pub fn ellipse(h: &mut Hand) {
        clicks(h, qymcad_ui_state::SketchShape::Ellipse, &[p([40.0, 30.0]), p([48.0, 30.0]), p([40.0, 34.0])]);
    }
    pub fn spline(h: &mut Hand) {
        h.sk_tool(qymcad_ui_state::SketchShape::Spline);
        for q in [p([32.0, 30.0]), p([37.0, 34.0]), p([43.0, 26.0])] {
            h.click2d(q.x, q.y);
        }
        h.double_click2d(48.0, 31.0);
        put_down(h);
    }
    pub fn closed_spline(h: &mut Hand) {
        clicks(h, qymcad_ui_state::SketchShape::Spline, &[p([32.0, 30.0]), p([40.0, 37.0]), p([48.0, 30.0]), p([40.0, 23.0]), p([32.0, 30.0])]);
    }
    pub fn rectangle(h: &mut Hand) {
        clicks(h, qymcad_ui_state::SketchShape::Rect, &[p([32.0, 25.0]), p([48.0, 35.0])]);
    }
    pub fn polygon(h: &mut Hand) {
        clicks(h, qymcad_ui_state::SketchShape::Polygon, &[p([40.0, 30.0]), p([47.0, 30.0])]);
    }
    pub fn slot(h: &mut Hand) {
        clicks(h, qymcad_ui_state::SketchShape::Slot, &[p([35.0, 30.0]), p([45.0, 30.0]), p([45.0, 33.0])]);
    }
    pub fn upright(h: &mut Hand) {
        clicks(h, qymcad_ui_state::SketchShape::Line, &[p([43.0, 14.0]), p([43.0, 46.0])]);
    }
    pub fn far_circle(h: &mut Hand) {
        clicks(h, qymcad_ui_state::SketchShape::Circle, &[p([48.0, 30.0]), p([53.0, 30.0])]);
    }
    pub fn nothing(_: &mut Hand) {}
}

use shapes::*;

/// A CASE: the shapes drawn, and the feature the line is drawn from (or to, for a touch).
struct Case {
    name: &'static str,
    first: Draw,
    second: Draw,
    feature: Feature,
}

const CASES: &[Case] = &[
    Case { name: "an end of a line", first: line, second: nothing, feature: Feature::End },
    Case { name: "the middle of a line", first: line, second: nothing, feature: Feature::Middle },
    Case { name: "an end of an arc", first: arc, second: nothing, feature: Feature::End },
    Case { name: "the middle of an arc", first: arc, second: nothing, feature: Feature::Middle },
    Case { name: "the centre of an arc", first: arc, second: nothing, feature: Feature::Centre },
    Case { name: "the centre of a circle", first: circle, second: nothing, feature: Feature::Centre },
    Case { name: "the right quadrant of a circle", first: circle, second: nothing, feature: Feature::Quadrant(p([1.0, 0.0])) },
    Case { name: "the top quadrant of a circle", first: circle, second: nothing, feature: Feature::Quadrant(p([0.0, 1.0])) },
    Case { name: "the centre of an ellipse", first: ellipse, second: nothing, feature: Feature::Centre },
    Case { name: "the right end of the major axis of an ellipse", first: ellipse, second: nothing, feature: Feature::Quadrant(p([1.0, 0.0])) },
    Case { name: "the left end of the major axis of an ellipse", first: ellipse, second: nothing, feature: Feature::Quadrant(p([-1.0, 0.0])) },
    Case { name: "the bottom end of the minor axis of an ellipse", first: ellipse, second: nothing, feature: Feature::Quadrant(p([0.0, -1.0])) },
    Case { name: "an end of an open spline", first: spline, second: nothing, feature: Feature::End },
    Case { name: "a node of an open spline", first: spline, second: nothing, feature: Feature::Node(p([37.0, 34.0])) },
    Case { name: "a node of a closed spline", first: closed_spline, second: nothing, feature: Feature::Node(p([40.0, 37.0])) },
    Case { name: "a corner of a rectangle", first: rectangle, second: nothing, feature: Feature::Node(p([48.0, 35.0])) },
    Case { name: "the middle of a side of a rectangle", first: rectangle, second: nothing, feature: Feature::Middle },
    Case { name: "the centre of a rectangle", first: rectangle, second: nothing, feature: Feature::Node(p([40.0, 30.0])) },
    Case { name: "a corner of a polygon", first: polygon, second: nothing, feature: Feature::Node(p([47.0, 30.0])) },
    Case { name: "the centre of a polygon", first: polygon, second: nothing, feature: Feature::Node(p([40.0, 30.0])) },
    Case { name: "the centre of an end of a slot", first: slot, second: nothing, feature: Feature::Node(p([45.0, 30.0])) },
    Case { name: "the crossing of a line and a line", first: upright, second: line, feature: Feature::Crossing },
    Case { name: "the crossing of a circle and a line", first: circle, second: upright, feature: Feature::Crossing },
    Case { name: "the crossing of an arc and a line", first: arc, second: upright, feature: Feature::Crossing },
    Case { name: "the crossing of an ellipse and a line", first: ellipse, second: upright, feature: Feature::Crossing },
    Case { name: "the crossing of an open spline and a line", first: spline, second: upright, feature: Feature::Crossing },
    Case { name: "the crossing of a closed spline and a line", first: closed_spline, second: upright, feature: Feature::Crossing },
    Case { name: "the crossing of two circles", first: circle, second: far_circle, feature: Feature::Crossing },
    Case { name: "a line touching a circle", first: circle, second: nothing, feature: Feature::Touch(p([40.0, 46.0])) },
    // from the right: from (40, 44) the line touching the circle of the arc on the left touches it at 155.7 deg, past the
    // end of the arc at 135
    Case { name: "a line touching an arc", first: arc, second: nothing, feature: Feature::Touch(p([52.0, 40.0])) },
    Case { name: "a line touching an ellipse", first: ellipse, second: nothing, feature: Feature::Touch(p([40.0, 44.0])) },
    Case { name: "a line touching a closed spline", first: closed_spline, second: nothing, feature: Feature::Touch(p([40.0, 46.0])) },
];

/// The crossings of two paths.
fn crossings(paths: [&[Point2]; 2]) -> Vec<Point2> {
    let [a, b] = paths;
    let mut out = Vec::new();
    for u in a.windows(2) {
        for v in b.windows(2) {
            let (r, s) = (u[1] - u[0], v[1] - v[0]);
            let den = r.x * s.y - r.y * s.x;
            if den.abs() < 1e-15 {
                continue;
            }
            let q = v[0] - u[0];
            let (t, w) = ((q.x * s.y - q.y * s.x) / den, (q.x * r.y - q.y * r.x) / den);
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&w) {
                out.push(along([u[0], u[1]], t));
            }
        }
    }
    out
}

/// The centre of a round curve or an ellipse from its points.
fn centre(c: &Curve) -> Point2 {
    if c.kind == "ellipse" || c.kind == "closed spline" {
        let n = (c.path.len() - 1) as f64;
        let m = c.path[..c.path.len() - 1].iter().fold(Point2::new(0.0, 0.0), |s, q| s + *q);
        return Point2::new(m.x / n, m.y / n);
    }
    let n = c.path.len();
    centre_of([c.path[0], c.path[n / 3], c.path[2 * n / 3]])
}

/// WHERE THE FEATURE STANDS in the drawing, read from its curves and points.
fn feature_at(case: &Case, d: &Drawing, shapes: &[Vec<qymcad_ui_state::SketchItem>]) -> Result<Point2, String> {
    let first = shapes[0].first().and_then(|k| d.curves.iter().find(|c| c.key == *k)).filter(|c| !c.construction).or_else(|| d.curves.iter().find(|c| shapes[0].contains(&c.key) && !c.construction));
    let first = first.ok_or("the first shape has no curve")?;
    let ends = [first.path[0], first.path[first.path.len() - 1]];
    Ok(match case.feature {
        Feature::End => ends[0],
        Feature::Middle => share_along(&first.path, 0.5),
        Feature::Centre => centre(first),
        Feature::Quadrant(w) => first.path.iter().copied().max_by(|a, b| (a.x * w.x + a.y * w.y).total_cmp(&(b.x * w.x + b.y * w.y))).ok_or("no points")?,
        Feature::Node(at) => d.points.iter().map(|q| q.at).min_by(|a, b| a.dist(at).total_cmp(&b.dist(at))).ok_or("no points")?,
        Feature::Crossing => {
            let second = shapes[1].first().and_then(|k| d.curves.iter().find(|c| c.key == *k)).ok_or("the second shape has no curve")?;
            crossings([&first.path, &second.path]).into_iter().max_by(|a, b| a.y.total_cmp(&b.y)).ok_or("the curves do not cross")?
        }
        Feature::Touch(from) => from,
    })
}

/// ONE CASE: the shapes drawn, a line drawn with its first click 0.3 mm off the feature and its second clear of it - or,
/// for a touch, from the place outside to near where it touches; the end of the line read against the feature.
fn snapped(case: &Case) -> Result<(), String> {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    let mut shapes: Vec<Vec<qymcad_ui_state::SketchItem>> = Vec::new();
    for draw in [case.first, case.second] {
        let had: Vec<qymcad_ui_state::SketchItem> = drawing(hand.app).curves.iter().map(|c| c.key).collect();
        draw(&mut hand);
        shapes.push(drawing(hand.app).curves.iter().map(|c| c.key).filter(|k| !had.contains(k)).collect());
    }
    let d = drawing(hand.app);
    let at = feature_at(case, &d, &shapes)?;
    let old: Vec<qymcad_ui_state::SketchItem> = d.curves.iter().map(|c| c.key).collect();
    hand.sk_tool(qymcad_ui_state::SketchShape::Line);
    let (start, end) = match case.feature {
        Feature::Touch(from) => {
            // near where a line from the place above touches the left of the curve: the point of the curve seen from
            // there farthest to the left of straight down
            let first = d.curves.iter().find(|c| shapes[0].contains(&c.key) && !c.construction).ok_or("no curve")?;
            let seen = |q: &Point2| (q.x - from.x).atan2(from.y - q.y);
            let touch = first.path.iter().copied().min_by(|a, b| seen(a).total_cmp(&seen(b))).ok_or("no points")?;
            (from, Point2::new(touch.x - 0.3, touch.y))
        }
        _ => (Point2::new(at.x + 0.3, at.y + 0.25), Point2::new(at.x - 9.0, at.y + 13.0)),
    };
    hand.click2d(start.x, start.y).click2d(end.x, end.y);
    put_down(&mut hand);
    let after = drawing(hand.app);
    let new = after.curves.iter().find(|c| !old.contains(&c.key) && c.kind == "line").ok_or_else(|| format!("no line was drawn; the status line says {:?}", hand.app.status))?;
    let (a, b) = (new.path[0], new.path[new.path.len() - 1]);
    match case.feature {
        Feature::Touch(from) => {
            let first = after.curves.iter().find(|c| shapes[0].contains(&c.key) && !c.construction).ok_or("the curve is gone")?;
            let end = if a.dist(from) < b.dist(from) { b } else { a };
            if off_path(end, &first.path) > NEAR {
                return Err(format!("the end of the line stands {:.3} off the curve", off_path(end, &first.path)));
            }
            // where the line touches, the curve runs along it: the way of the curve there - square to the radius of the
            // circle through its three points about the end - is the way of the line. Read as how far its points within
            // 0.6 mm rise off the line, a touching where the curve bends at 1 mm read 0.18 off
            let way = end - from;
            let n = first.path.len();
            let k = (0..n).min_by(|x, y| first.path[*x].dist(end).total_cmp(&first.path[*y].dist(end))).unwrap_or(0).clamp(1, n - 2);
            let m = centre_of([first.path[k - 1], first.path[k], first.path[k + 1]]);
            let radial = end - m;
            let turn = ((radial.x * way.x + radial.y * way.y) / (radial.len() * way.len())).abs().min(1.0).asin().to_degrees();
            if turn > 1.0 {
                return Err(format!("the line meets the curve where it ends at {turn:.2} deg, not touching"));
            }
            Ok(())
        }
        // the feature where it stands once the line is made: the automatic constraints of the line may move the shape - a
        // line as long as a side of a rectangle is made as long, and the corner it starts at moved 0.056 - and the line
        // starts on the feature still
        _ => {
            let at = feature_at(case, &after, &shapes)?;
            let off = a.dist(at).min(b.dist(at));
            if off > NEAR {
                return Err(format!("the line starts {off:.3} off the feature at ({:.3}, {:.3}); the status line says {:?}", at.x, at.y, hand.app.status));
            }
            Ok(())
        }
    }
}

#[test]
fn a_line_drawn_snaps_to_every_kind() {
    let mut failures = Vec::new();
    for case in CASES {
        cell(&mut failures, format!("the snap to {}", case.name), || snapped(case));
    }
    said("the snaps", CASES.len(), &failures);
}
