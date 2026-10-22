//! THE DIMENSIONS on every kind: the one dimension tool on a single curve - the length of a line, the diameter of a
//! circle, the radius of an arc, a semi-axis of an ellipse - and on two things, a point or a curve each, in both orders
//! of the clicks - their least distance; the radius tool on every round curve; the angle tool on two lines. Each value is
//! typed into the field of the new dimension, and the geometry must follow it.
//!
//! A linear and an angular dimension are placed by a click after what they measure; the radius tool opens its field on
//! the curve clicked.
use super::*;

/// A TOOL OF THE GROUP, by the hint of its button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Dimension,
    Radius,
    Angle,
}

impl Tool {
    fn hint(self) -> &'static str {
        match self {
            Tool::Dimension => "tb-dim-hint",
            Tool::Radius => "tb-dim-radius-hint",
            Tool::Angle => "tb-dim-angle-hint",
        }
    }
}

/// WHAT IS CLICKED for a dimension, and what is measured after it.
#[derive(Clone, Copy, Debug)]
enum Thing {
    /// the point drawn by the point tool
    Point,
    /// the first curve of a shape
    Curve(usize),
}

/// WHAT THE VALUE TYPED SETS.
#[derive(Clone, Copy, Debug)]
enum Sets {
    /// the length of the line
    Length,
    /// the diameter of the circle
    Diameter,
    /// the radius of the arc
    Radius,
    /// one of the semi-axes of the ellipse
    SemiAxis,
    /// the least distance between the two things
    Distance,
    /// the angle between the two lines, in degrees
    Angle,
}

/// A SHAPE OF A CASE, drawn by hand; the shapes of a case stand apart, the first left, the second right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Point,
    Line,
    SecondLine,
    Circle,
    Arc,
    Ellipse,
    OpenSpline,
}

fn put_down(h: &mut Hand) {
    h.key(egui::Key::Escape).key(egui::Key::Escape);
    h.sk_select();
}

/// Draw `shape` by hand about `at`.
fn draw(h: &mut Hand, shape: Shape, at: Point2) {
    let c = |dx: f64, dy: f64| Point2::new(at.x + dx, at.y + dy);
    let click = |h: &mut Hand, q: Point2| {
        h.click2d(q.x, q.y);
    };
    match shape {
        Shape::Point => {
            h.sk_tool(qymcad_ui_state::SketchShape::Point);
            click(h, c(0.0, 0.0));
        }
        Shape::Line => {
            h.sk_tool(qymcad_ui_state::SketchShape::Line);
            click(h, c(-4.0, -6.0));
            click(h, c(2.0, 6.0));
        }
        Shape::SecondLine => {
            h.sk_tool(qymcad_ui_state::SketchShape::Line);
            click(h, c(-6.0, -6.0));
            click(h, c(6.0, 2.0));
        }
        Shape::Circle => {
            h.sk_tool(qymcad_ui_state::SketchShape::Circle);
            click(h, c(0.0, 0.0));
            click(h, c(5.0, 0.0));
        }
        Shape::Arc => {
            h.sk_tool(qymcad_ui_state::SketchShape::Arc);
            click(h, c(0.0, -2.0));
            click(h, c(6.0, -2.0));
            click(h, c(-4.0, 2.5));
        }
        Shape::Ellipse => {
            h.sk_tool(qymcad_ui_state::SketchShape::Ellipse);
            click(h, c(0.0, 0.0));
            click(h, c(7.0, 0.0));
            click(h, c(0.0, 3.5));
        }
        Shape::OpenSpline => {
            h.sk_tool(qymcad_ui_state::SketchShape::Spline);
            click(h, c(-6.0, -1.0));
            click(h, c(-2.0, 4.0));
            click(h, c(2.0, -3.0));
            h.double_click2d(at.x + 6.0, at.y + 2.0);
        }
    }
    put_down(h);
}

/// A CASE OF THE MATRIX: the tool, the shapes, what is clicked in order, what the value sets, and the value.
struct Case {
    tool: Tool,
    shapes: &'static [Shape],
    clicks: &'static [Thing],
    sets: Sets,
    value: f64,
}

const fn one(tool: Tool, shape: &'static [Shape], sets: Sets, value: f64) -> Case {
    Case { tool, shapes: shape, clicks: &[Thing::Curve(0)], sets, value }
}

const fn two(shapes: &'static [Shape], clicks: &'static [Thing]) -> Case {
    Case { tool: Tool::Dimension, shapes, clicks, sets: Sets::Distance, value: 4.0 }
}

const FIRST: Thing = Thing::Curve(0);
const SECOND: Thing = Thing::Curve(1);

const CASES: &[Case] = &[
    one(Tool::Dimension, &[Shape::Line], Sets::Length, 15.0),
    one(Tool::Dimension, &[Shape::Circle], Sets::Diameter, 14.0),
    one(Tool::Dimension, &[Shape::Arc], Sets::Radius, 9.0),
    one(Tool::Dimension, &[Shape::Ellipse], Sets::SemiAxis, 9.0),
    one(Tool::Radius, &[Shape::Circle], Sets::Diameter, 14.0),
    one(Tool::Radius, &[Shape::Arc], Sets::Radius, 9.0),
    one(Tool::Radius, &[Shape::Ellipse], Sets::SemiAxis, 9.0),
    Case { tool: Tool::Angle, shapes: &[Shape::Line, Shape::SecondLine], clicks: &[FIRST, SECOND], sets: Sets::Angle, value: 50.0 },
    Case { tool: Tool::Angle, shapes: &[Shape::Line, Shape::SecondLine], clicks: &[SECOND, FIRST], sets: Sets::Angle, value: 50.0 },
    // the least distance between a point and every curve, and between curves, in both orders
    two(&[Shape::Point, Shape::Line], &[Thing::Point, SECOND]),
    two(&[Shape::Point, Shape::Line], &[SECOND, Thing::Point]),
    two(&[Shape::Point, Shape::Circle], &[Thing::Point, SECOND]),
    two(&[Shape::Point, Shape::Circle], &[SECOND, Thing::Point]),
    two(&[Shape::Point, Shape::Arc], &[Thing::Point, SECOND]),
    two(&[Shape::Point, Shape::Arc], &[SECOND, Thing::Point]),
    two(&[Shape::Point, Shape::Ellipse], &[Thing::Point, SECOND]),
    two(&[Shape::Point, Shape::Ellipse], &[SECOND, Thing::Point]),
    two(&[Shape::Point, Shape::OpenSpline], &[Thing::Point, SECOND]),
    two(&[Shape::Point, Shape::OpenSpline], &[SECOND, Thing::Point]),
    two(&[Shape::Line, Shape::Circle], &[FIRST, SECOND]),
    two(&[Shape::Line, Shape::Circle], &[SECOND, FIRST]),
    two(&[Shape::Circle, Shape::Circle], &[FIRST, SECOND]),
    two(&[Shape::Circle, Shape::Ellipse], &[FIRST, SECOND]),
    two(&[Shape::Circle, Shape::Ellipse], &[SECOND, FIRST]),
    two(&[Shape::Line, Shape::Ellipse], &[FIRST, SECOND]),
    two(&[Shape::Circle, Shape::OpenSpline], &[FIRST, SECOND]),
];

/// Where the shapes of a case stand: the first left of the middle, the second right.
const PLACES: [Point2; 2] = [Point2 { x: 28.0, y: 30.0 }, Point2 { x: 50.0, y: 30.0 }];

/// The least distance between two point sets laid as paths.
fn least(paths: [&[Point2]; 2]) -> f64 {
    let [a, b] = paths;
    a.iter().map(|p| off_path(*p, b)).fold(f64::MAX, f64::min).min(b.iter().map(|p| off_path(*p, a)).fold(f64::MAX, f64::min))
}

/// The radii of a round curve or an ellipse: for an ellipse the far and the near distance of its points from the middle
/// of them, for a circle or an arc its radius twice.
fn round_of(c: &Curve) -> [f64; 2] {
    if c.kind == "ellipse" {
        let n = (c.path.len() - 1) as f64;
        let m = c.path[..c.path.len() - 1].iter().fold(Point2::new(0.0, 0.0), |s, q| s + *q);
        let m = Point2::new(m.x / n, m.y / n);
        let far = c.path.iter().map(|q| q.dist(m)).fold(0.0, f64::max);
        let near = c.path.iter().map(|q| q.dist(m)).fold(f64::MAX, f64::min);
        return [far, near];
    }
    let n = c.path.len();
    let m = centre_of([c.path[0], c.path[n / 3], c.path[2 * n / 3]]);
    [m.dist(c.path[0]), m.dist(c.path[0])]
}

/// WHERE A LINEAR OR ANGULAR DIMENSION IS PLACED: 3 mm across and 3 mm up or down from the middle of the way between the
/// nearest points of the two things - or of the ends of the one line - on the side of its normal.
fn placed(d: &Drawing, things: &[Option<qymcad_ui_state::SketchItem>]) -> Option<Point2> {
    let path = |key: qymcad_ui_state::SketchItem| -> Option<Vec<Point2>> {
        if key.kind == qymcad_ui_state::ItemKind::Point {
            d.points.iter().find(|q| q.key == key).map(|q| vec![q.at])
        } else {
            d.curves.iter().find(|c| c.key == key).map(|c| c.path.clone())
        }
    };
    let (a, b) = match things {
        [Some(one)] => {
            let p = path(*one)?;
            (p[0], p[p.len() - 1])
        }
        [Some(one), Some(two)] => {
            let (pa, pb) = (path(*one)?, path(*two)?);
            let a = pa.iter().copied().min_by(|x, y| off_path(*x, &pb).total_cmp(&off_path(*y, &pb)))?;
            let b = pb.iter().copied().min_by(|x, y| x.dist(a).total_cmp(&y.dist(a)))?;
            (a, b)
        }
        _ => return None,
    };
    // the cursor at 45 deg off the middle, on the side of the normal: a dimension turns level when the cursor stands 1.7
    // times farther up than across from the middle, upright the other way, aligned in between
    let n = Point2::new(-(b.y - a.y), b.x - a.x);
    let side = |v: f64| if v < 0.0 { -3.0 } else { 3.0 };
    let m = along([a, b], 0.5);
    Some(Point2::new(m.x + side(n.x), m.y + side(n.y)))
}

/// THE VALUE THE GEOMETRY HAS NOW for what the case sets.
fn measured(case: &Case, things: &[Option<qymcad_ui_state::SketchItem>], d: &Drawing) -> Result<f64, String> {
    let curve = |k: usize| -> Result<&Curve, String> { things[k].and_then(|key| d.curves.iter().find(|c| c.key == key)).ok_or_else(|| "a curve of the case is gone".into()) };
    // a distance to a line is to the whole line through it, as in every sketcher
    let path = |k: usize| -> Result<Vec<Point2>, String> {
        match things[k] {
            Some(key) if key.kind == qymcad_ui_state::ItemKind::Point => d.points.iter().find(|q| q.key == key).map(|q| vec![q.at]).ok_or_else(|| "the point is gone".into()),
            _ => {
                let c = curve(k)?;
                if c.kind != "line" {
                    return Ok(c.path.clone());
                }
                let (a, b) = (c.path[0], c.path[c.path.len() - 1]);
                let w = b - a;
                Ok(vec![Point2::new(a.x - w.x * 100.0, a.y - w.y * 100.0), Point2::new(b.x + w.x * 100.0, b.y + w.y * 100.0)])
            }
        }
    };
    Ok(match case.sets {
        Sets::Length => {
            let p = &curve(0)?.path;
            p[0].dist(p[p.len() - 1])
        }
        Sets::Diameter => 2.0 * round_of(curve(0)?)[0],
        Sets::Radius => round_of(curve(0)?)[0],
        Sets::SemiAxis => {
            let r = round_of(curve(0)?);
            if (r[0] - case.value).abs() < (r[1] - case.value).abs() {
                r[0]
            } else {
                r[1]
            }
        }
        Sets::Distance => least([&path(0)?, &path(1)?]),
        Sets::Angle => {
            let way = |k: usize| -> Result<Point2, String> {
                let p = &curve(k)?.path;
                Ok(p[p.len() - 1] - p[0])
            };
            let (a, b) = (way(0)?, way(1)?);
            let cos = (a.x * b.x + a.y * b.y) / (a.len() * b.len());
            let deg = cos.clamp(-1.0, 1.0).acos().to_degrees();
            deg.min(180.0 - deg)
        }
    })
}

/// ONE CASE: the shapes drawn, the tool taken, the things clicked in order, the dimension placed above them, the value
/// typed into its field and Enter; then the geometry measured.
fn sized(case: &Case) -> Result<(), String> {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    let mut keys: Vec<Option<qymcad_ui_state::SketchItem>> = Vec::new();
    for (shape, at) in case.shapes.iter().zip(PLACES) {
        let before: Vec<u64> = drawing(hand.app).curves.iter().map(|c| c.key.id).chain(drawing(hand.app).points.iter().map(|q| q.key.id)).collect();
        draw(&mut hand, *shape, at);
        let d = drawing(hand.app);
        let new = if *shape == Shape::Point {
            d.points.iter().map(|q| q.key).find(|k| !before.contains(&k.id))
        } else {
            d.curves.iter().map(|c| c.key).find(|k| !before.contains(&k.id))
        };
        keys.push(new);
    }
    if keys.iter().any(Option::is_none) {
        return Err("a shape of the case left nothing on the sheet".into());
    }
    let things: Vec<Option<qymcad_ui_state::SketchItem>> = case
        .clicks
        .iter()
        .map(|t| match t {
            Thing::Point => keys[0],
            Thing::Curve(k) => keys[*k],
        })
        .collect();
    let d = drawing(hand.app);
    let mut places = Vec::new();
    for key in things.iter().flatten() {
        let at = match d.curves.iter().find(|c| c.key == *key) {
            Some(c) => spot(&mut hand, c),
            None => d.points.iter().find(|q| q.key == *key).map(|q| q.at).ok_or("no point to click")?,
        };
        places.push(at);
    }
    let b = bounds(&d);
    if !hand.press_hint(&crate::i18n::tr(case.tool.hint())) {
        return Err(format!("no button {}", case.tool.hint()));
    }
    hand.close_window();
    for at in &places {
        hand.click2d(at.x, at.y);
    }
    // a linear or an angular dimension is placed by one more click: beside the way between the nearest points of what it
    // measures, square to it - the place a person puts an aligned dimension at, which the cursor turns level or upright
    // elsewhere; the radius tool opens its field on the curve
    if case.tool != Tool::Radius {
        let at = placed(&d, &things).unwrap_or(Point2::new((b.lo.x + b.hi.x) / 2.0, b.hi.y + 6.0));
        hand.click2d(at.x, at.y);
    }
    // the value goes into the field that holds the keyboard, or into the one showing the example in grey while empty
    let value = format!("{}", case.value);
    hand.frame(Vec::new());
    if hand.typing() {
        hand.chord(egui::Modifiers::COMMAND, egui::Key::A).type_text(&value);
    } else if !hand.type_into_hinted(&crate::i18n::tr("sk-expr-example"), case.value) {
        return Err(format!("no field of a dimension came up; the status line says {:?}", hand.app.status));
    }
    hand.key(egui::Key::Enter);
    let got = measured(case, &things, &drawing(hand.app))?;
    if (got - case.value).abs() > NEAR {
        return Err(format!("it measures {got:.3}, not {}; the status line says {:?}", case.value, hand.app.status));
    }
    Ok(())
}

#[test]
fn every_dimension_takes_every_kind() {
    let mut failures = Vec::new();
    for case in CASES {
        let what = case.shapes.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>().join(" and ");
        let order = case.clicks.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(" then ");
        cell(&mut failures, format!("{:?} of {what} ({order}): {:?}", case.tool, case.sets), || sized(case));
    }
    said("the dimensions", CASES.len(), &failures);
}
