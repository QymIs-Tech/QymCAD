//! A COPY CARRIES THE DIMENSIONS OF WHAT IT COPIES, AND LAYS THEM WHERE THE COPY LAYS THE CURVES: a mirror, a mirror about
//! a point, a linear and a circular pattern of a circle with its diameter or its radius, an arc with its radius, a slanted
//! line with its length, a rectangle with its width and height and an ellipse with its half axis - every copy has every
//! dimension of the source, and each stands where the map takes the dimension of the source: a dimension laid outside the
//! source stands outside its mirror image, and holds the copy where it was laid - a solve after it moves no point. A
//! source with no dimension gives copies with none.
//!
//! Reported behaviour: the mirror of a circle with a diameter had none, nor had its linear and circular patterns; the
//! width and height of a mirrored rectangle stood inside it where those of the source stood outside.
use qymcad_core::feature::{Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, Id, PatternKind, Project};

/// A SHAPE WITH ITS DIMENSIONS: its name and how it is drawn into a sketch, answering the curves taken.
struct Shape {
    name: &'static str,
    draw: fn(&mut Project, usize) -> Vec<Id>,
}

/// The centre of the first round curve of sketch `si`.
fn centre_of(p: &Project, si: usize) -> Id {
    p.sketches[si].entities.iter().find_map(|e| e.kind.as_circle().map(|c| c.center).or(e.kind.as_arc().map(|a| a.center))).expect("a round curve")
}

/// The id of point `pid` of sketch `si` placed at `q`, by its place.
fn place(p: &Project, si: usize, pid: Id) -> Point2 {
    p.sketches[si].point(pid).map(|q| Point2::new(q.x, q.y)).expect("the point")
}

/// WHICH SIZE OF A ROUND CURVE a radial dimension gives.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Radial {
    Diameter,
    Radius,
}

/// The radial dimension of the round curve of sketch `si`, its leader turned to `angle` radians on the screen.
fn radial(p: &mut Project, si: usize, size: Radial, angle: f64) {
    let diam = size == Radial::Diameter;
    let c = centre_of(p, si);
    // the door of a circle; an arc is given its radius as the menu of the arc gives it
    let rim = p.sketches[si].entities.iter().find_map(|e| e.kind.as_arc()).map(|a| place(p, si, a.a));
    let k = match rim {
        Some(q) => {
            let r = place(p, si, c).dist(q);
            p.sketches[si].constraints.push(Constraint::Diameter { c, d: r, off: 0.0, expr: String::new(), driven: false, diam, at: None });
            p.sketches[si].constraints.len() - 1
        }
        None => p.ensure_diameter(si, c, diam).expect("the radial dimension"),
    };
    if let Constraint::Diameter { off, .. } = &mut p.sketches[si].constraints[k] {
        *off = angle;
    }
}

/// A linear dimension between `a` and `b` along `axis` (0 aligned, 1 horizontal, 2 upright), laid `off` aside.
fn linear(p: &mut Project, si: usize, ends: [Id; 2], axis: u8, off: f64) {
    let [qa, qb] = ends.map(|id| place(p, si, id));
    let d = match axis {
        1 => (qb.x - qa.x).abs(),
        2 => (qb.y - qa.y).abs(),
        _ => qa.dist(qb),
    };
    p.sketches[si].constraints.push(Constraint::Distance { a: ends[0], b: ends[1], d, off, expr: String::new(), driven: false, axis, at: None });
}

fn all(p: &Project, si: usize) -> Vec<Id> {
    p.sketches[si].entities.iter().map(|e| e.id).collect()
}

const SHAPES: [Shape; 7] = [
    Shape {
        name: "circle with its diameter",
        draw: |p, si| {
            p.add_circle_entity(si, 30.0, 10.0, 5.0, Purpose::Real);
            radial(p, si, Radial::Diameter, 0.6);
            all(p, si)
        },
    },
    Shape {
        name: "circle with its radius",
        draw: |p, si| {
            p.add_circle_entity(si, 30.0, 10.0, 5.0, Purpose::Real);
            radial(p, si, Radial::Radius, 2.0);
            all(p, si)
        },
    },
    Shape {
        name: "arc with its radius",
        draw: |p, si| {
            p.add_arc_entity(si, Point2::new(30.0, 10.0), Point2::new(36.0, 10.0), Point2::new(30.0, 16.0), Winding::Ccw, Purpose::Real);
            radial(p, si, Radial::Radius, -0.4);
            all(p, si)
        },
    },
    Shape {
        name: "slanted line with its length",
        draw: |p, si| {
            let l = p.add_line_entity(si, 20.0, 5.0, 40.0, 15.0, Purpose::Real);
            let ends = p.sketches[si].entity(l).and_then(|e| e.kind.as_line()).map(|l| [l.a, l.b]).expect("the line");
            linear(p, si, ends, 0, 3.0);
            all(p, si)
        },
    },
    Shape {
        name: "rectangle with its width and height",
        draw: |p, si| {
            p.add_rect_entity(si, 20.0, 2.0, 30.0, 10.0, Purpose::Real);
            let corners = p.sketches[si].rects()[0].corners;
            let at = |p: &Project, x: f64, y: f64| *corners.iter().find(|c| place(p, si, **c).dist(Point2::new(x, y)) < 1e-9).expect("a corner");
            let (bl, br, tr) = (at(p, 20.0, 2.0), at(p, 30.0, 2.0), at(p, 30.0, 10.0));
            // the width below the rectangle, the height to the right of it, as a person lays them
            linear(p, si, [bl, br], 1, 5.0);
            linear(p, si, [br, tr], 2, 5.0);
            all(p, si)
        },
    },
    Shape {
        name: "ellipse with its half axis",
        draw: |p, si| {
            let c = p.add_ellipse_entity(si, Point2::new(40.0, 20.0), 8.0, 4.0, 0.3, Purpose::Real);
            let ma = p.sketches[si].entities.iter().find_map(|e| e.kind.as_ellipse()).map(|el| el.ma).expect("the ellipse");
            linear(p, si, [c, ma], 0, 2.0);
            all(p, si)
        },
    },
    Shape {
        name: "line with no dimension",
        draw: |p, si| {
            p.add_line_entity(si, 20.0, 5.0, 40.0, 15.0, Purpose::Real);
            all(p, si)
        },
    },
];

/// AN EDITING TOOL that lays copies, and the maps of the sheet that take the source onto each copy.
struct Tool {
    name: &'static str,
    apply: fn(&mut Project, usize, &[Id]),
    maps: fn() -> Vec<Map>,
}

/// A map of the sheet: a linear part (its columns, where X and Y go) and a shift.
#[derive(Clone, Copy)]
struct Map {
    x: [f64; 2],
    y: [f64; 2],
    shift: [f64; 2],
}

impl Map {
    fn turn(deg: f64) -> Map {
        let (s, c) = deg.to_radians().sin_cos();
        Map { x: [c, s], y: [-s, c], shift: [0.0, 0.0] }
    }
    fn shift(by: [f64; 2]) -> Map {
        Map { x: [1.0, 0.0], y: [0.0, 1.0], shift: by }
    }
    fn of(&self, q: Point2) -> Point2 {
        Point2::new(self.x[0] * q.x + self.y[0] * q.y + self.shift[0], self.x[1] * q.x + self.y[1] * q.y + self.shift[1])
    }
}

const TOOLS: [Tool; 6] = [
    Tool {
        name: "Copy",
        apply: |p, si, e| {
            p.copy_entities(si, e, 0.0, -50.0);
        },
        maps: || vec![Map::shift([0.0, -50.0])],
    },
    Tool { name: "Mirror", apply: |p, si, e| p.mirror_entities(si, e, 0.0, 0.0, 0.0, 1.0), maps: || vec![Map { x: [-1.0, 0.0], y: [0.0, 1.0], shift: [0.0, 0.0] }] },
    Tool { name: "Mirror about a point", apply: |p, si, e| p.mirror_about_point(si, e, 0.0, 0.0), maps: || vec![Map::turn(180.0)] },
    Tool {
        name: "Linear pattern",
        apply: |p, si, e| {
            p.add_pattern(si, e, PatternKind::Linear { dx: 0.0, dy: -50.0, count: 3, dx2: 0.0, dy2: 0.0, count2: 0 });
        },
        maps: || vec![Map::shift([0.0, -50.0]), Map::shift([0.0, -100.0])],
    },
    Tool {
        name: "Circular pattern of 90 deg",
        apply: |p, si, e| {
            p.add_pattern(si, e, PatternKind::Circular { cx: 0.0, cy: 0.0, count: 4, total_deg: 360.0 });
        },
        maps: || vec![Map::turn(90.0), Map::turn(180.0), Map::turn(270.0)],
    },
    Tool {
        name: "Circular pattern of 120 deg",
        apply: |p, si, e| {
            p.add_pattern(si, e, PatternKind::Circular { cx: 0.0, cy: 0.0, count: 3, total_deg: 360.0 });
        },
        maps: || vec![Map::turn(120.0), Map::turn(240.0)],
    },
];

/// A DIMENSION AS IT STANDS: the places of the points it measures, its value, and the place its line or its leader is
/// laid at - the middle of the line of a linear one, the end of the leader of a radial one, the base gap of the screen
/// left out.
struct Laid {
    ends: Vec<Point2>,
    value: f64,
    at: Point2,
}

fn laid(p: &Project, si: usize, c: &Constraint) -> Option<Laid> {
    match *c {
        Constraint::Distance { a, b, d, off, axis, .. } => {
            let (qa, qb) = (place(p, si, a), place(p, si, b));
            let mid = Point2::new((qa.x + qb.x) / 2.0, (qa.y + qb.y) / 2.0);
            // the screen runs Y down: a line shifted by off on the screen stands off lower on the sheet
            let at = match axis {
                1 => Point2::new(mid.x, mid.y - off),
                2 => Point2::new(mid.x + off, mid.y),
                _ => {
                    let len = qa.dist(qb).max(1e-12);
                    Point2::new(mid.x + (qb.y - qa.y) / len * off, mid.y - (qb.x - qa.x) / len * off)
                }
            };
            Some(Laid { ends: vec![qa, qb], value: d, at })
        }
        Constraint::Diameter { c, d, off, diam, .. } => {
            let q = place(p, si, c);
            let r = if diam { d / 2.0 } else { d };
            Some(Laid { ends: vec![q], value: d, at: Point2::new(q.x + r * off.cos(), q.y - r * off.sin()) })
        }
        _ => None,
    }
}

fn check(shape: &Shape, tool: &Tool) -> Vec<String> {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("dimensions");
    let eids = (shape.draw)(&mut p, si);
    p.solve_sketch(si);
    let source: Vec<Laid> = p.sketches[si].constraints.iter().filter_map(|c| laid(&p, si, c)).collect();
    let before = p.sketches[si].constraints.len();
    (tool.apply)(&mut p, si, &eids);
    let mut out = Vec::new();
    // the dimensions carried hold the copies where they were laid: a solve moves no point
    let stood: Vec<Point2> = p.sketches[si].points.iter().map(|q| Point2::new(q.x, q.y)).collect();
    p.solve_sketch(si);
    let moved = p.sketches[si].points.iter().zip(&stood).map(|(q, s)| Point2::new(q.x, q.y).dist(*s)).fold(0.0, f64::max);
    if moved > 1e-6 {
        out.push(format!("a solve after the copy moved a point by {moved:.3}"));
    }
    let after: Vec<Laid> = p.sketches[si].constraints.iter().filter_map(|c| laid(&p, si, c)).collect();
    let maps = (tool.maps)();
    if after.len() != source.len() * (1 + maps.len()) {
        out.push(format!("{} dimensions, not {} ({} constraints before)", after.len(), source.len() * (1 + maps.len()), before));
    }
    for (k, m) in maps.iter().enumerate() {
        for d in &source {
            let ends: Vec<Point2> = d.ends.iter().map(|q| m.of(*q)).collect();
            let found = after.iter().find(|c| (c.value - d.value).abs() < 1e-9 && c.ends.len() == ends.len() && c.ends.iter().zip(&ends).all(|(a, b)| a.dist(*b) < 1e-6));
            match found {
                None => out.push(format!("copy {}: no dimension of {:.3} on the copy", k + 1, d.value)),
                Some(c) => {
                    let want = m.of(d.at);
                    if c.at.dist(want) > 1e-6 {
                        out.push(format!("copy {}: the dimension of {:.3} stands at ({:.2}, {:.2}), not where the map takes it, ({:.2}, {:.2})", k + 1, d.value, c.at.x, c.at.y, want.x, want.y));
                    }
                }
            }
        }
    }
    out
}

#[test]
fn a_copy_carries_the_dimensions_of_what_it_copies() {
    let mut failures = Vec::new();
    for shape in &SHAPES {
        for tool in &TOOLS {
            failures.extend(check(shape, tool).into_iter().map(|f| format!("{}, {}: {f}", shape.name, tool.name)));
        }
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
}
