//! THE TOOLS OF A SKETCH, each through the door of the project the window calls, on the first sketch of a project: the
//! list the matrices of the tools run over. A big sketch may hold nothing a tool works on - a sketch of lines held on
//! lines has no corner - so the targets of the tools are laid beside it first (`lay_targets`): a rectangle, an open
//! chain, a line short of a wall, a circle, an arc and two lines a little off level. Each tool says what it leaves
//! behind (`Effect`), and the matrices hold it to that and the loops to those of a rebuild from all the curves.
#![allow(dead_code)] // each probe that takes this module in uses a part of it

use qymcad_core::feature::{Ends, Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::{ChamferLegs, Constraint, CornerAt, CornerCut, EntityKind, FilletSize, Id, PatternKind, Project};

/// The lines of the sketch, the first `n`.
pub fn lines(p: &Project, n: usize) -> Vec<Id> {
    p.sketches[0].entities.iter().filter(|e| matches!(e.kind, EntityKind::Line { .. }) && !e.construction).map(|e| e.id).take(n).collect()
}

/// The ends of line `eid`.
fn ends(p: &Project, eid: Id) -> (Id, Id) {
    match p.sketches[0].entity(eid).map(|e| e.kind) {
        Some(EntityKind::Line { a, b }) | Some(EntityKind::Arc { a, b, .. }) => (a, b),
        _ => (0, 0),
    }
}

/// Where point `id` stands.
fn at(p: &Project, id: Id) -> Point2 {
    p.sketches[0].point(id).map(|q| Point2::new(q.x, q.y)).unwrap_or(Point2::new(0.0, 0.0))
}

/// The point a third along line `eid`.
fn third_along(p: &Project, eid: Id) -> Point2 {
    let (a, b) = ends(p, eid);
    let (pa, pb) = (at(p, a), at(p, b));
    Point2::new(pa.x + (pb.x - pa.x) / 3.0, pa.y + (pb.y - pa.y) / 3.0)
}

/// The corners where exactly two of the curves `among` meet.
fn corners_of(p: &Project, among: &[Id]) -> Vec<CornerAt> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for &e in among {
        let (a, b) = ends(p, e);
        for q in [a, b] {
            if seen.insert(q) {
                if let [pair] = p.vertex_pairs(0, q)[..] {
                    out.push(CornerAt { point: q, pair });
                }
            }
        }
    }
    out
}

/// WHAT THE TOOLS WORK ON, laid beside the sketch: past the right of everything in it.
pub struct Targets {
    /// the four sides of a rectangle 40 x 30: four corners
    pub rect: Vec<Id>,
    /// an open chain of three lines
    pub chain: Vec<Id>,
    /// a vertical line and a level one ending 5 mm short of it
    pub wall: Id,
    pub short: Id,
    /// a circle of radius 10, by its centre
    pub circle: Id,
    /// an arc of radius 10 a quarter round
    pub arc: Id,
    /// two lines a little off level and off parallel
    pub one: Id,
    pub two: Id,
}

/// Lay the targets of the tools in the first sketch, solved, and answer them.
pub fn lay_targets(p: &mut Project) -> Targets {
    let x0 = p.sketches[0].points.iter().map(|q| q.x).fold(0.0_f64, f64::max) + 50.0;
    let rect = p.add_rect_entity(0, x0, 0.0, x0 + 40.0, 30.0, Purpose::Real);
    let chain = vec![
        p.add_line_entity(0, x0, 50.0, x0 + 20.0, 50.0, Purpose::Real),
        p.add_line_entity(0, x0 + 20.0, 50.0, x0 + 20.0, 70.0, Purpose::Real),
        p.add_line_entity(0, x0 + 20.0, 70.0, x0 + 40.0, 70.0, Purpose::Real),
    ];
    let wall = p.add_line_entity(0, x0 + 60.0, 0.0, x0 + 60.0, 40.0, Purpose::Real);
    let short = p.add_line_entity(0, x0 + 45.0, 20.0, x0 + 55.0, 20.0, Purpose::Real);
    let circle_line = p.add_circle_entity(0, x0 + 90.0, 20.0, 10.0, Purpose::Real);
    let circle = match p.sketches[0].entity(circle_line).map(|e| e.kind) {
        Some(EntityKind::Circle { center, .. }) => center,
        _ => 0,
    };
    let before: std::collections::HashSet<Id> = p.sketches[0].entities.iter().map(|e| e.id).collect();
    p.add_arc_entity(0, Point2::new(x0 + 130.0, 20.0), Point2::new(x0 + 140.0, 20.0), Point2::new(x0 + 130.0, 30.0), Winding::Ccw, Purpose::Real);
    let arc = p.sketches[0].entities.iter().find(|e| !before.contains(&e.id) && matches!(e.kind, EntityKind::Arc { .. })).map(|e| e.id).unwrap_or(0);
    let one = p.add_line_entity(0, x0, 100.0, x0 + 30.0, 103.0, Purpose::Real);
    let two = p.add_line_entity(0, x0 + 5.0, 110.0, x0 + 33.0, 118.0, Purpose::Real);
    p.solve_sketch(0);
    Targets { rect, chain, wall, short, circle, arc, one, two }
}

/// WHAT A TOOL LEAVES BEHIND, to be held to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    /// more curves than before
    Adds,
    /// fewer curves than before
    Removes,
    /// more constraints than before
    Constrains,
    /// a point stands elsewhere
    Moves,
    /// the curves of a sketch turned to construction
    Turns,
    /// nothing to hold it to beyond the loops
    Any,
}

/// Whether `after` shows the effect against `before`; the words where it does not.
pub fn shows(effect: Effect, before: &Project, after: &Project) -> Option<String> {
    let (b, a) = (&before.sketches[0], &after.sketches[0]);
    let curves = |s: &qymcad_core::model::Sketch| s.entities.len() + s.splines.len();
    let ok = match effect {
        Effect::Adds => curves(a) > curves(b),
        Effect::Removes => curves(a) < curves(b),
        Effect::Constrains => a.constraints.len() > b.constraints.len(),
        Effect::Moves => a.points.iter().zip(&b.points).any(|(p, q)| p.id == q.id && (p.x != q.x || p.y != q.y)) || a.points.len() != b.points.len(),
        Effect::Turns => a.entities.iter().filter(|e| e.construction).count() != b.entities.iter().filter(|e| e.construction).count(),
        Effect::Any => true,
    };
    (!ok).then(|| format!("{effect:?} expected: {} -> {} curves, {} -> {} constraints", curves(b), curves(a), b.constraints.len(), a.constraints.len()))
}

/// The loops of the first sketch as a sorted list of what each is: closed, its entities, its centroid and area to 1e-6.
pub fn loops(p: &Project) -> Vec<String> {
    let mut out: Vec<String> = p.sketches[0]
        .contour_ids
        .iter()
        .filter_map(|cid| {
            let c = &p.contours[p.contour_index(*cid)?];
            let mut src = c.edge_src.clone();
            src.sort_unstable();
            src.dedup();
            let n = c.points.len().max(1) as f64;
            let (sx, sy) = c.points.iter().fold((0.0, 0.0), |(a, b), q| (a + q.x, b + q.y));
            Some(format!("{} {:?} ({:.6}, {:.6}) {:.6}", c.closed, src, sx / n, sy / n, c.signed_area().abs()))
        })
        .collect();
    out.sort();
    out
}

/// A constraint laid as the window lays one: only where it constrains something, the sketch solved, the relations it
/// makes follow lifted (`independent_of`, `drop_implied_relations`).
fn lay(p: &mut Project, c: Constraint) {
    let among: std::collections::HashSet<Id> = c.points().into_iter().collect();
    let new = p.independent_of(0, vec![c]);
    let had = p.sketches[0].constraints.len();
    p.sketches[0].constraints.extend(new);
    p.solve_sketch(0);
    let _ = p.drop_implied_relations(0, had, &among);
}

/// A tool, by its name: its work on the first sketch with the targets laid, and what it leaves behind.
pub struct Tool {
    pub name: &'static str,
    pub work: fn(&mut Project, &Targets),
    pub effect: Effect,
}

pub fn tools() -> Vec<Tool> {
    vec![
        // the patterns and arrays
        Tool {
            name: "a linear pattern of 10",
            work: |p, t| {
                let _ = p.add_pattern(0, &[t.one], PatternKind::Linear { dx: 3.0, dy: 3.0, count: 10, dx2: 0.0, dy2: 0.0, count2: 0 });
            },
            effect: Effect::Adds,
        },
        Tool {
            name: "a pattern changed to 20",
            work: |p, t| {
                let _ = p.add_pattern(0, &[t.one], PatternKind::Linear { dx: 3.0, dy: 3.0, count: 10, dx2: 0.0, dy2: 0.0, count2: 0 });
                let pi = p.sketches[0].patterns.len() - 1;
                p.update_pattern(0, pi, PatternKind::Linear { dx: 3.0, dy: 3.0, count: 20, dx2: 0.0, dy2: 0.0, count2: 0 });
            },
            effect: Effect::Adds,
        },
        Tool {
            name: "a circular pattern of 8",
            work: |p, t| {
                let c = at(p, t.circle);
                let _ = p.add_pattern(0, &[t.short], PatternKind::Circular { cx: c.x, cy: c.y, count: 8, total_deg: 360.0 });
            },
            effect: Effect::Adds,
        },
        Tool { name: "a linear array of 10", work: |p, t| p.array_linear(0, &[t.one], 3.0, 3.0, 10), effect: Effect::Adds },
        Tool {
            name: "a circular array of 8",
            work: |p, t| {
                let c = at(p, t.circle);
                p.array_circular(0, &[t.short], c.x, c.y, 8, 360.0)
            },
            effect: Effect::Adds,
        },
        // the edits of a selection
        Tool { name: "50 lines mirrored", work: |p, _| p.mirror_entities(0, &lines(p, 50), 0.0, -100.0, 1.0, -100.0), effect: Effect::Adds },
        Tool {
            name: "50 lines copied",
            work: |p, _| {
                let _ = p.copy_entities(0, &lines(p, 50), 0.0, -500.0);
            },
            effect: Effect::Adds,
        },
        Tool {
            name: "50 lines pasted",
            work: |p, _| {
                let clip = p.copy_sketch_geometry(0, &lines(p, 50), 0.0, 0.0);
                let _ = p.paste_sketch_geometry(0, &clip, 0.0, -500.0);
            },
            effect: Effect::Adds,
        },
        Tool { name: "50 lines moved", work: |p, _| p.move_entities(0, &lines(p, 50), 1.0, 1.0), effect: Effect::Moves },
        Tool { name: "50 lines turned", work: |p, _| p.rotate_entities(0, &lines(p, 50), 0.0, 0.0, 5.0), effect: Effect::Moves },
        Tool { name: "50 lines scaled", work: |p, _| p.scale_entities(0, &lines(p, 50), 0.0, 0.0, 1.1), effect: Effect::Moves },
        Tool {
            name: "a rectangle offset",
            work: |p, t| {
                let _ = p.offset_entities(0, &t.rect, 2.0);
            },
            effect: Effect::Adds,
        },
        Tool {
            name: "an open chain offset",
            work: |p, t| {
                let _ = p.offset_entities(0, &t.chain, 2.0);
            },
            effect: Effect::Adds,
        },
        Tool {
            name: "20 lines made construction",
            work: |p, _| {
                let _ = p.toggle_construction(0, &lines(p, 20));
            },
            effect: Effect::Turns,
        },
        Tool { name: "50 lines deleted", work: |p, _| p.delete_entities(0, &lines(p, 50)), effect: Effect::Removes },
        // the corners
        Tool {
            name: "the corners of a rectangle filleted as a set",
            work: |p, t| {
                let c = corners_of(p, &t.rect);
                let _ = p.cut_corner_set(0, &c, CornerCut::Fillet(FilletSize::radius(3.0)), None);
            },
            effect: Effect::Adds,
        },
        Tool {
            name: "the corners of a rectangle chamfered as a set",
            work: |p, t| {
                let c = corners_of(p, &t.rect);
                let _ = p.cut_corner_set(0, &c, CornerCut::Chamfer(ChamferLegs::equal(3.0)), None);
            },
            effect: Effect::Adds,
        },
        Tool {
            name: "every corner of a rectangle and a chain filleted",
            work: |p, t| {
                let only: std::collections::HashSet<Id> = t.rect.iter().chain(&t.chain).copied().collect();
                let _ = p.fillet_all_corners_by(0, FilletSize::radius(2.0), Some(&only));
            },
            effect: Effect::Adds,
        },
        // the cuts of a line
        Tool {
            name: "a side of a rectangle trimmed",
            work: |p, t| {
                let at = third_along(p, t.chain[1]);
                let _ = p.trim_line(0, t.chain[1], at.x, at.y);
            },
            effect: Effect::Any,
        },
        Tool {
            name: "a line broken",
            work: |p, t| {
                let at = third_along(p, t.one);
                let _ = p.break_line(0, t.one, at.x, at.y);
            },
            effect: Effect::Adds,
        },
        Tool {
            name: "a line extended to a wall",
            work: |p, t| {
                let (_, b) = ends(p, t.short);
                let tip = at(p, b);
                let _ = p.extend_line(0, t.short, tip.x - 1.0, tip.y);
            },
            effect: Effect::Moves,
        },
        Tool {
            name: "close points stitched",
            work: |p, _| {
                let _ = p.merge_close_points(0, 0.01);
            },
            effect: Effect::Any,
        },
        // the drawing tools
        Tool {
            name: "a line drawn",
            work: |p, _| {
                let _ = p.add_line_entity(0, -50.0, -50.0, -40.0, -45.0, Purpose::Real);
            },
            effect: Effect::Adds,
        },
        Tool {
            name: "a rectangle drawn",
            work: |p, _| {
                let _ = p.add_rect_entity(0, -80.0, -80.0, -60.0, -70.0, Purpose::Real);
            },
            effect: Effect::Adds,
        },
        Tool {
            name: "a circle drawn",
            work: |p, _| {
                let _ = p.add_circle_entity(0, -100.0, -100.0, 5.0, Purpose::Real);
            },
            effect: Effect::Adds,
        },
        Tool {
            name: "an arc drawn",
            work: |p, _| p.add_arc_entity(0, Point2::new(-130.0, -100.0), Point2::new(-125.0, -100.0), Point2::new(-130.0, -95.0), Winding::Ccw, Purpose::Real),
            effect: Effect::Adds,
        },
        Tool {
            name: "an ellipse drawn",
            work: |p, _| {
                let _ = p.add_ellipse_entity(0, Point2::new(-160.0, -100.0), 8.0, 4.0, 0.3, Purpose::Real);
            },
            effect: Effect::Adds,
        },
        Tool {
            name: "a spline drawn",
            work: |p, _| p.add_spline(0, vec![Point2::new(-200.0, -100.0), Point2::new(-195.0, -92.0), Point2::new(-188.0, -98.0), Point2::new(-180.0, -90.0)], Ends::Open, Purpose::Real),
            effect: Effect::Adds,
        },
        Tool {
            name: "a polygon drawn",
            work: |p, _| {
                let _ = p.add_polygon_entity(0, Point2::new(-120.0, -120.0), Point2::new(-115.0, -120.0), 6, Purpose::Real);
            },
            effect: Effect::Adds,
        },
        Tool { name: "a slot drawn", work: |p, _| p.add_slot_entity(0, Point2::new(-150.0, -150.0), Point2::new(-140.0, -150.0), 2.0, Purpose::Real), effect: Effect::Adds },
        Tool {
            name: "a line drawn from a point on a line",
            work: |p, t| {
                // as the line tool lays it: the line, its first end held on the line under it, the sketch solved
                let from = third_along(p, t.one);
                let new = p.add_line_entity(0, from.x, from.y, from.x - 7.0, from.y - 5.0, Purpose::Real);
                let (start, _) = ends(p, new);
                let (a, b) = ends(p, t.one);
                lay(p, Constraint::PointOnLine { p: start, a, b });
            },
            effect: Effect::Adds,
        },
        // the constraints, each as its button lays it
        Tool {
            name: "Horizontal laid",
            work: |p, t| {
                let (a, b) = ends(p, t.one);
                lay(p, Constraint::Horizontal { a, b });
            },
            effect: Effect::Constrains,
        },
        Tool {
            name: "Vertical laid",
            work: |p, t| {
                let (a, b) = ends(p, t.two);
                lay(p, Constraint::Vertical { a, b });
            },
            effect: Effect::Constrains,
        },
        Tool {
            name: "Parallel laid",
            work: |p, t| {
                let ((a, b), (c, d)) = (ends(p, t.one), ends(p, t.two));
                lay(p, Constraint::Parallel { a, b, c, d });
            },
            effect: Effect::Constrains,
        },
        Tool {
            name: "Perpendicular laid",
            work: |p, t| {
                let ((a, b), (c, d)) = (ends(p, t.one), ends(p, t.two));
                lay(p, Constraint::Perpendicular { a, b, c, d });
            },
            effect: Effect::Constrains,
        },
        Tool {
            name: "Equal laid",
            work: |p, t| {
                let ((a, b), (c, d)) = (ends(p, t.one), ends(p, t.two));
                lay(p, Constraint::Equal { a, b, c, d });
            },
            effect: Effect::Constrains,
        },
        Tool {
            name: "Collinear laid",
            work: |p, t| {
                let ((a, b), (c, d)) = (ends(p, t.one), ends(p, t.two));
                lay(p, Constraint::Collinear { a, b, c, d });
            },
            effect: Effect::Constrains,
        },
        Tool {
            name: "Coincident laid",
            work: |p, t| {
                let ((_, b), (c, _)) = (ends(p, t.one), ends(p, t.two));
                lay(p, Constraint::Coincident { a: b, b: c });
            },
            effect: Effect::Constrains,
        },
        Tool {
            name: "Midpoint laid",
            work: |p, t| {
                let ((a, b), (c, _)) = (ends(p, t.one), ends(p, t.two));
                lay(p, Constraint::Midpoint { p: c, a, b });
            },
            effect: Effect::Constrains,
        },
        Tool {
            name: "Tangent laid",
            work: |p, t| {
                let (a, b) = ends(p, t.short);
                lay(p, Constraint::Tangent { a, b, c: t.circle, r: 10.0 });
            },
            effect: Effect::Constrains,
        },
        Tool {
            name: "Symmetric laid",
            work: |p, t| {
                let ((a, b), (la, lb)) = (ends(p, t.one), ends(p, t.wall));
                lay(p, Constraint::Symmetric { a, b, la, lb });
            },
            effect: Effect::Constrains,
        },
        Tool {
            name: "Fixed laid",
            work: |p, t| {
                let (a, _) = ends(p, t.two);
                lay(p, Constraint::Fixed { p: a });
            },
            effect: Effect::Constrains,
        },
        // the dimensions
        Tool {
            name: "a distance laid and solved",
            work: |p, t| {
                let (a, b) = ends(p, t.one);
                lay(p, Constraint::Distance { a, b, d: 25.0, off: 2.0, expr: String::new(), driven: false, axis: 0, at: None });
            },
            effect: Effect::Constrains,
        },
        Tool {
            name: "an angle laid and solved",
            work: |p, t| {
                let (a, b) = ends(p, t.chain[0]);
                let (_, c) = ends(p, t.chain[1]);
                lay(p, Constraint::Angle { a, b, c, deg: 80.0, expr: String::new(), driven: false, off: 2.0, at: None });
            },
            effect: Effect::Constrains,
        },
        Tool {
            name: "a diameter laid and solved",
            work: |p, t| lay(p, Constraint::Diameter { c: t.circle, d: 24.0, off: 0.0, expr: String::new(), driven: false, diam: true, at: None }),
            effect: Effect::Constrains,
        },
        Tool {
            name: "a dimension laid as the window lays it",
            work: |p, t| {
                // solved, asked whether it is redundant and whether it conflicts (`finish_dim`)
                let (a, b) = ends(p, t.two);
                let ci = p.sketches[0].constraints.len();
                let d = (at(p, a).x - at(p, b).x).hypot(at(p, a).y - at(p, b).y);
                p.sketches[0].constraints.push(Constraint::Distance { a, b, d, off: 2.0, expr: String::new(), driven: false, axis: 0, at: None });
                p.solve_sketch(0);
                let _ = p.dim_redundant(0, ci);
                let _ = p.sketch_conflicts(0);
            },
            effect: Effect::Constrains,
        },
    ]
}
