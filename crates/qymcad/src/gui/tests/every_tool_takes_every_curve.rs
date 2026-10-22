//! EVERY TOOL OF A SKETCH TAKES EVERY CURVE, BY EVERY WAY IT HAS: a line, a rectangle, a polygon, a slot, a circle, an
//! arc, an ellipse, an arc of an ellipse, an open and a closed spline and a point, each drawn by its tool in the window,
//! picked by a click, by Shift, by a window, by a crossing box or by the tool taken first, and given to every tool of the
//! sketch. A tool must do to each kind what it does to a line.
//!
//! The matrix is the measure of the work: every cell runs to its end, and what failed is said together, one line a
//! cell. A cell that makes no sense - a construction point, an ellipse made level - is written down in the group with
//! its reason, not left out in silence.
//!
//! What a cell sees of the sketch is read in one place, `drawing`: the kind of every curve in words, whether it is
//! construction geometry, and the curve laid out as points. The checks compare those points - moved, turned, mirrored,
//! offset - and never the records a kind is kept in.
//!
//! Reported behaviour: the tools worked on lines and rectangles, and on circles, arcs, ellipses and splines in part or
//! not at all - "all geometry must have the full set of the same cases, by every way the tool allows".
use crate::gui::hand::{Drag2d, Hand};
use crate::gui::App;
use qymcad_core::geom::Point2;

mod constraints;
mod corners;
mod dims;
mod edits;
mod over_selection;
mod snaps;

/// How close two curves must lie to count as one, in millimetres: well above the sag of the drawn points (0.02 mm on a
/// circle of radius 8 laid out by 96 points) and well below any change a tool makes.
const NEAR: f64 = 0.05;

/// A KIND OF CURVE OF THE MATRIX: its name, how a person draws it about a place with the tools of the panel, and how
/// many curves that leaves. A kind that no tool draws yet has no way to be drawn, and every cell of it is red for that.
pub(super) struct Kind {
    pub name: &'static str,
    pub draw: Option<fn(&mut Hand, Point2)>,
    pub curves: usize,
}

/// Clicks on the sheet with the tool in hand, at `places` about `c`.
fn clicks(hand: &mut Hand, c: Point2, places: &[Point2]) {
    for p in places {
        hand.click2d(c.x + p.x, c.y + p.y);
    }
}

/// THE KINDS, each kept within 8 mm of its place across and 5 mm up and down, so that copies 20 mm apart do not touch.
pub(super) const KINDS: &[Kind] = &[
    Kind {
        name: "line",
        draw: Some(|h, c| {
            h.sk_tool(qymcad_ui_state::SketchShape::Line);
            clicks(h, c, &[Point2::new(-8.0, -4.0), Point2::new(8.0, 4.0)]);
            h.key(egui::Key::Escape);
        }),
        curves: 1,
    },
    Kind {
        name: "rectangle",
        draw: Some(|h, c| {
            h.sk_tool(qymcad_ui_state::SketchShape::Rect);
            clicks(h, c, &[Point2::new(-8.0, -5.0), Point2::new(8.0, 5.0)]);
        }),
        curves: 4,
    },
    Kind {
        name: "polygon",
        draw: Some(|h, c| {
            h.sk_tool(qymcad_ui_state::SketchShape::Polygon);
            clicks(h, c, &[Point2::new(0.0, 0.0), Point2::new(7.0, 0.0)]);
        }),
        curves: 7,
    },
    Kind {
        name: "slot",
        draw: Some(|h, c| {
            h.sk_tool(qymcad_ui_state::SketchShape::Slot);
            clicks(h, c, &[Point2::new(-5.0, 0.0), Point2::new(5.0, 0.0), Point2::new(5.0, 3.0)]);
        }),
        curves: 4,
    },
    Kind {
        name: "circle",
        draw: Some(|h, c| {
            h.sk_tool(qymcad_ui_state::SketchShape::Circle);
            clicks(h, c, &[Point2::new(0.0, 0.0), Point2::new(6.0, 0.0)]);
        }),
        curves: 1,
    },
    Kind {
        name: "arc",
        draw: Some(|h, c| {
            h.sk_tool(qymcad_ui_state::SketchShape::Arc);
            clicks(h, c, &[Point2::new(0.0, -3.0), Point2::new(7.0, -3.0), Point2::new(-5.0, 2.0)]);
        }),
        curves: 1,
    },
    Kind {
        name: "ellipse",
        draw: Some(|h, c| {
            h.sk_tool(qymcad_ui_state::SketchShape::Ellipse);
            clicks(h, c, &[Point2::new(0.0, 0.0), Point2::new(8.0, 0.0), Point2::new(0.0, 4.0)]);
        }),
        curves: 1,
    },
    // no tool draws an arc of an ellipse: a person draws the ellipse, a line across it a millimetre below its centre, trims
    // the smaller piece below the line away and deletes the line
    Kind {
        name: "arc of an ellipse",
        draw: Some(|h, c| {
            h.sk_tool(qymcad_ui_state::SketchShape::Ellipse);
            clicks(h, c, &[Point2::new(0.0, 0.0), Point2::new(8.0, 0.0), Point2::new(0.0, 4.0)]);
            h.key(egui::Key::Escape);
            h.sk_tool(qymcad_ui_state::SketchShape::Line);
            clicks(h, c, &[Point2::new(-12.0, -1.0), Point2::new(12.0, -1.0)]);
            h.key(egui::Key::Escape).key(egui::Key::Escape);
            assert!(h.press_hint(&crate::i18n::tr("tb-trim-hint")), "no trim button");
            h.close_window();
            h.click2d(c.x, c.y - 4.0);
            h.key(egui::Key::Escape);
            h.sk_select();
            h.click2d(c.x + 11.0, c.y - 1.0);
            h.key(egui::Key::Delete);
        }),
        curves: 1,
    },
    Kind {
        name: "open spline",
        draw: Some(|h, c| {
            h.sk_tool(qymcad_ui_state::SketchShape::Spline);
            clicks(h, c, &[Point2::new(-8.0, 0.0), Point2::new(-3.0, 4.0), Point2::new(3.0, -4.0)]);
            h.double_click2d(c.x + 8.0, c.y + 1.0);
        }),
        curves: 1,
    },
    Kind {
        name: "closed spline",
        draw: Some(|h, c| {
            h.sk_tool(qymcad_ui_state::SketchShape::Spline);
            clicks(h, c, &[Point2::new(-8.0, 0.0), Point2::new(0.0, 5.0), Point2::new(8.0, 0.0), Point2::new(0.0, -5.0), Point2::new(-8.0, 0.0)]);
        }),
        curves: 1,
    },
    Kind {
        name: "point",
        draw: Some(|h, c| {
            h.sk_tool(qymcad_ui_state::SketchShape::Point);
            clicks(h, c, &[Point2::new(0.0, 0.0)]);
        }),
        curves: 0,
    },
];

/// A CURVE OF THE SKETCH as a cell sees it: its kind in words, whether it is construction geometry, and its points in
/// order along it.
#[derive(Clone, Debug)]
pub(super) struct Curve {
    pub key: qymcad_ui_state::SketchItem,
    pub kind: &'static str,
    pub construction: bool,
    pub path: Vec<Point2>,
}

/// A POINT OF THE SKETCH as a cell sees it: what picks it, and where it stands.
#[derive(Clone, Copy, Debug)]
pub(super) struct Dot {
    pub key: qymcad_ui_state::SketchItem,
    pub at: Point2,
}

/// THE SKETCH AS A CELL SEES IT: its curves, and its points the person can pick - the frame of reference left out.
#[derive(Clone, Debug, Default)]
pub(super) struct Drawing {
    pub curves: Vec<Curve>,
    pub points: Vec<Dot>,
}

/// A WAY ROUND A CENTRE: radii along and across `rot`, from the angle `from` through `sweep`, all in radians.
struct Turn {
    centre: Point2,
    radii: Point2,
    rot: f64,
    from: f64,
    sweep: f64,
}

/// The points of a turn, 96 to a whole one.
fn round(t: Turn) -> Vec<Point2> {
    let n = ((t.sweep.abs() / std::f64::consts::TAU) * 96.0).ceil().max(8.0) as usize;
    let (s, k) = t.rot.sin_cos();
    (0..=n)
        .map(|i| {
            let a = t.from + t.sweep * i as f64 / n as f64;
            let (u, v) = (t.radii.x * a.cos(), t.radii.y * a.sin());
            Point2::new(t.centre.x + u * k - v * s, t.centre.y + u * s + v * k)
        })
        .collect()
}

/// The point a share `t` of the way along a segment.
pub(super) fn along(seg: [Point2; 2], t: f64) -> Point2 {
    Point2::new(seg[0].x + (seg[1].x - seg[0].x) * t, seg[0].y + (seg[1].y - seg[0].y) * t)
}

/// A CURVE READ AT 512 EVEN PLACES ALONG IT, a closed one back to its start: the outline it is drawn by stands its
/// points up to 0.95 mm apart along a spline, and the way of the curve read off three of them was 1.2 deg off; an
/// ellipse read at 96 stood them 0.9 mm apart round an end bending at 1.4 mm.
fn dense(s: &qymcad_core::model::Sketch, e: &qymcad_core::model::SketchEntity) -> Vec<Point2> {
    qymcad_core::model::curve::Curve::of(s, e).map(|c| (0..=512).map(|k| c.at(k as f64 / 512.0)).collect()).unwrap_or_default()
}

/// THE SKETCH OPEN FOR EDITING, read in words: the one place that knows how each kind of curve is kept.
pub(super) fn drawing(app: &App) -> Drawing {
    let Some(si) = qymcad_ui_state::edit_si(&app.project, &app.sketch_ses) else { return Drawing::default() };
    let s = &app.project.sketches[si];
    let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y)).unwrap_or(Point2::new(f64::NAN, f64::NAN));
    let curves: Vec<Curve> = s
        .entities
        .iter()
        .map(|e| {
            let k = &e.kind;
            let (kind, path) = if let Some(l) = k.as_line() {
                ("line", (0..=16).map(|i| along([at(l.a), at(l.b)], i as f64 / 16.0)).collect::<Vec<Point2>>())
            } else if let Some(c) = k.as_circle() {
                ("circle", round(Turn { centre: at(c.center), radii: Point2::new(c.r, c.r), rot: 0.0, from: 0.0, sweep: std::f64::consts::TAU }))
            } else if let Some(arc) = k.as_arc() {
                let (c, pa, pb) = (at(arc.center), at(arc.a), at(arc.b));
                let r = c.dist(pa);
                let (ta, tb) = ((pa.y - c.y).atan2(pa.x - c.x), (pb.y - c.y).atan2(pb.x - c.x));
                let sweep = if arc.ccw { (tb - ta).rem_euclid(std::f64::consts::TAU) } else { -(ta - tb).rem_euclid(std::f64::consts::TAU) };
                ("arc", round(Turn { centre: c, radii: Point2::new(r, r), rot: 0.0, from: ta, sweep }))
            } else if let Some(el) = k.as_ellipse() {
                (if el.span.is_some() { "arc of an ellipse" } else { "ellipse" }, dense(s, e))
            } else {
                let closed = k.as_spline().is_some_and(|sp| sp.ends == qymcad_core::feature::Ends::Closed);
                (if closed { "closed spline" } else { "open spline" }, dense(s, e))
            };
            Curve { key: qymcad_ui_state::SketchItem::curve(e.id), kind, construction: e.construction, path }
        })
        .collect();
    let unseen = s.unseen_points();
    let points = s.points.iter().filter(|q| !unseen.contains(&q.id)).map(|q| Dot { key: qymcad_ui_state::SketchItem::point(q.id), at: Point2::new(q.x, q.y) }).collect();
    Drawing { curves, points }
}

/// A NEW DOCUMENT with one sketch open for editing and `kind` drawn by its tool about each of `places`, the tool then
/// put down. Answers why it could not be drawn.
pub(super) fn laid(kind: &Kind, places: &[Point2]) -> Result<App, String> {
    let draw = kind.draw.ok_or_else(|| format!("no tool draws {}", kind.name))?;
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    // the field of the size that comes up at a new shape is closed by Esc, which puts the tool down, as a person does
    // before taking the arrow: with the field holding the keyboard the hint of no button comes up
    for p in places {
        draw(&mut hand, *p);
        hand.key(egui::Key::Escape);
        hand.sk_select();
    }
    let d = drawing(&app);
    let (want, got) = if kind.curves == 0 { (places.len(), d.points.len()) } else { (kind.curves * places.len(), d.curves.len()) };
    if got != want {
        return Err(format!("{} drawn by its tool {} times left {got} curves or points, not {want}; the status line says {:?}", kind.name, places.len(), app.status));
    }
    Ok(app)
}

/// The point a share `t` of the length along a path.
pub(super) fn share_along(path: &[Point2], t: f64) -> Point2 {
    let lengths: Vec<f64> = path.windows(2).map(|w| w[0].dist(w[1])).collect();
    let mut left = lengths.iter().sum::<f64>() * t;
    for (w, l) in path.windows(2).zip(&lengths) {
        if left <= *l && *l > 0.0 {
            return along([w[0], w[1]], left / l);
        }
        left -= l;
    }
    path.first().copied().unwrap_or(Point2::new(0.0, 0.0))
}

/// A PLACE ON A CURVE a click picks it by, found as a person finds it: along the curve, from a third of the way on,
/// away from its ends and its middle, which the snaps take for points, until what lights up under the pointer is the
/// curve - a caption of a dimension lying over it takes the click first. A curve no place picks is clicked a third of
/// the way along, and the cell sees what that picked.
pub(super) fn spot(hand: &mut Hand, c: &Curve) -> Point2 {
    const SHARES: [f64; 12] = [0.37, 0.3, 0.44, 0.23, 0.52, 0.16, 0.6, 0.09, 0.68, 0.76, 0.84, 0.92];
    SHARES.iter().map(|t| share_along(&c.path, *t)).find(|p| hand.would_pick2d((p.x, p.y), c.key)).unwrap_or_else(|| share_along(&c.path, SHARES[0]))
}

/// THE SPOTS a person clicks to pick the whole drawing: one on every curve, or every point of a drawing of points.
pub(super) fn spots(hand: &mut Hand, d: &Drawing) -> Vec<Point2> {
    if d.curves.is_empty() {
        d.points.iter().map(|p| p.at).collect()
    } else {
        d.curves.iter().map(|c| spot(hand, c)).collect()
    }
}

/// THE BOX OF THE DRAWING: its lower left and upper right corners.
pub(super) struct Bounds {
    pub lo: Point2,
    pub hi: Point2,
}

pub(super) fn bounds(d: &Drawing) -> Bounds {
    let all = d.curves.iter().flat_map(|c| c.path.iter()).chain(d.points.iter().map(|p| &p.at));
    let (mut lo, mut hi) = (Point2::new(f64::MAX, f64::MAX), Point2::new(f64::MIN, f64::MIN));
    for p in all {
        lo = Point2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    Bounds { lo, hi }
}

/// HOW A PERSON PICKS what a tool works on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Way {
    /// a click on the curve, Shift and a click on each further curve of a figure
    Click,
    /// two of the kind, every curve of both picked by a click and then Shift and a click
    Shift,
    /// a window dragged left to right round the whole drawing
    Window,
    /// a crossing box dragged right to left over the left half of the drawing: what it touches is picked
    Crossing,
    /// the tool taken with nothing picked, then a click on the curve
    ToolFirst,
}

pub(super) const WAYS: [Way; 5] = [Way::Click, Way::Shift, Way::Window, Way::Crossing, Way::ToolFirst];

/// The places a kind is drawn about for a way: one, or two for Shift.
pub(super) fn places(way: Way) -> Vec<Point2> {
    match way {
        Way::Shift => vec![Point2::new(40.0, 30.0), Point2::new(40.0, 50.0)],
        _ => vec![Point2::new(40.0, 30.0)],
    }
}

/// PICK the drawing as `way` picks it, the select tool in hand: every curve and point by clicks, or a box. Not used for
/// `ToolFirst`, which picks after the tool.
pub(super) fn pick(hand: &mut Hand, way: Way) {
    let d = drawing(hand.app);
    let b = bounds(&d);
    hand.sk_select();
    match way {
        Way::Click | Way::Shift | Way::ToolFirst => {
            for (k, p) in spots(hand, &d).iter().enumerate() {
                if k == 0 {
                    hand.click2d(p.x, p.y);
                } else {
                    hand.shift_click2d(p.x, p.y);
                }
            }
        }
        Way::Window => {
            let (from, to) = ((b.lo.x - 4.0, b.hi.y + 4.0), (b.hi.x + 4.0, b.lo.y - 4.0));
            hand.drag2d_held(Drag2d { from, to, button: egui::PointerButton::Primary, modifiers: egui::Modifiers::NONE });
        }
        Way::Crossing => {
            let right = crossing_edge(&b);
            let (from, to) = ((right, b.hi.y + 4.0), (b.lo.x - 4.0, b.lo.y - 4.0));
            hand.drag2d_held(Drag2d { from, to, button: egui::PointerButton::Primary, modifiers: egui::Modifiers::NONE });
        }
    }
}

/// WHAT IS PICKED, as the window keeps it.
pub(super) fn picked(app: &App) -> Vec<qymcad_ui_state::SketchItem> {
    app.tools.sel_sk.items.clone()
}

/// THE RIGHT EDGE OF A CROSSING BOX: the middle of the drawing, or past it when the drawing has no width - a point.
fn crossing_edge(b: &Bounds) -> f64 {
    if b.hi.x - b.lo.x < 1.0 {
        b.hi.x + 4.0
    } else {
        (b.lo.x + b.hi.x) / 2.0
    }
}

/// WHAT A CROSSING BOX over the left half of the drawing touches: the curves with a point left of its right edge, and
/// the points there.
pub(super) fn crossed(d: &Drawing) -> Vec<qymcad_ui_state::SketchItem> {
    let edge = crossing_edge(&bounds(d));
    let curves = d.curves.iter().filter(|c| c.path.iter().any(|p| p.x <= edge)).map(|c| c.key);
    curves.chain(d.points.iter().filter(|p| d.curves.is_empty() && p.at.x <= edge).map(|p| p.key)).collect()
}

/// The least distance from `p` to the path.
pub(super) fn off_path(p: Point2, path: &[Point2]) -> f64 {
    path.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let (vx, vy) = (b.x - a.x, b.y - a.y);
            let l2 = vx * vx + vy * vy;
            let t = if l2 > 1e-18 { (((p.x - a.x) * vx + (p.y - a.y) * vy) / l2).clamp(0.0, 1.0) } else { 0.0 };
            p.dist(Point2::new(a.x + t * vx, a.y + t * vy))
        })
        .fold(if path.len() == 1 { p.dist(path[0]) } else { f64::MAX }, f64::min)
}

/// The least distance from `p` to any of the curves.
pub(super) fn off_curves(p: Point2, curves: &[Curve]) -> f64 {
    curves.iter().map(|c| off_path(p, &c.path)).fold(f64::MAX, f64::min)
}

/// The centre of the circle through three points.
pub(super) fn centre_of(p: [Point2; 3]) -> Point2 {
    let [a, b, c] = p;
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    let s = |q: Point2| q.x * q.x + q.y * q.y;
    Point2::new((s(a) * (b.y - c.y) + s(b) * (c.y - a.y) + s(c) * (a.y - b.y)) / d, (s(a) * (c.x - b.x) + s(b) * (a.x - c.x) + s(c) * (b.x - a.x)) / d)
}

/// A CURVE EXPECTED after a tool: its kind and its points.
#[derive(Clone, Debug)]
pub(super) struct Want {
    pub kind: &'static str,
    pub path: Vec<Point2>,
}

/// Each curve of `d` with `f` applied to its points.
pub(super) fn mapped(d: &Drawing, f: impl Fn(Point2) -> Point2) -> Vec<Want> {
    d.curves.iter().map(|c| Want { kind: c.kind, path: c.path.iter().map(|p| f(*p)).collect() }).collect()
}

/// WHETHER THE CURVES OF `got` ARE THE CURVES WANTED: as many of each kind, and every wanted curve lying on a curve of
/// its kind and that curve on it. Answers what differs, in words.
pub(super) fn matches(want: &[Want], got: &Drawing) -> Result<(), String> {
    let count = |kinds: &mut Vec<&'static str>| {
        kinds.sort_unstable();
        kinds.clone()
    };
    let (w, g) = (count(&mut want.iter().map(|c| c.kind).collect()), count(&mut got.curves.iter().map(|c| c.kind).collect()));
    if w != g {
        return Err(format!("the sketch holds {g:?}, not {w:?}"));
    }
    let on = |a: &[Point2], b: &[Point2]| a.iter().all(|p| off_path(*p, b) < NEAR);
    let mut free: Vec<&Curve> = got.curves.iter().collect();
    for c in want {
        let Some(k) = free.iter().position(|g| g.kind == c.kind && on(&c.path, &g.path) && on(&g.path, &c.path)) else {
            let near = got.curves.iter().filter(|g| g.kind == c.kind).map(|g| c.path.iter().map(|p| off_path(*p, &g.path)).fold(0.0, f64::max)).fold(f64::MAX, f64::min);
            return Err(format!("no {} lies where it should: the nearest of its kind is {near:.3} mm off", c.kind));
        };
        free.remove(k);
    }
    Ok(())
}

/// WHETHER THE POINTS OF `got` ARE THE POINTS WANTED, each within `NEAR` of one wanted, as many as wanted.
pub(super) fn points_match(want: &[Point2], got: &Drawing) -> Result<(), String> {
    if want.len() != got.points.len() {
        return Err(format!("{} points, not {}", got.points.len(), want.len()));
    }
    match want.iter().find(|p| got.points.iter().all(|q| q.at.dist(**p) >= NEAR)) {
        Some(p) => Err(format!("no point at ({:.3}, {:.3})", p.x, p.y)),
        None => Ok(()),
    }
}

/// ONE CELL RUN TO ITS END: what it answers or the words of its panic, set down under its name when it fails.
pub(super) fn cell(failures: &mut Vec<String>, name: String, work: impl FnOnce() -> Result<(), String>) {
    let said = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)) {
        Ok(Ok(())) => return,
        Ok(Err(why)) => why,
        Err(panic) => panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "it panicked".into()),
    };
    failures.push(format!("{name}: {said}"));
}

/// THE CELLS OF A GROUP SAID TOGETHER: how many of how many failed, and each one on its own line.
pub(super) fn said(group: &str, cells: usize, failures: &[String]) {
    assert!(failures.is_empty(), "{group}: {} of {cells} cells failed\n  {}", failures.len(), failures.join("\n  "));
}
