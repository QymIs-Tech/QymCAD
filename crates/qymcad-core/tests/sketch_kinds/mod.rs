//! SKETCHES OF EVERY KIND, laid straight into the sketch's records: the measure of speed (`sketch_speed.rs`) and the
//! reference of the solve by parts (`the_parts_solve_as_the_whole.rs`) build the same sketches.
//!
//! Not through `add_line_entity`, which looks a new end up among every point already there - building 70 000 lines
//! that way would itself take hours. Every shape carries work for the solver: a constraint not yet met, as a sketch
//! has right after an edit.
#![allow(dead_code)] // each test that includes this module uses its own share of it
use qymcad_core::model::{Constraint, EntityKind, Project, SketchEntity, SketchPoint};

/// A kind of sketch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// separate lines, each 2 deg off level and held Horizontal
    Lines,
    /// separate arcs, the two ends of each held Horizontal while standing 2 deg off - the sketch of issue #95
    Arcs,
    /// separate rectangles of four lines, square by Horizontal and Vertical, their width dimensioned 1 mm off
    Rectangles,
    /// separate circles, each with a diameter 0.5 mm off
    Circles,
    /// separate closed hexagons, every side Equal to the first, the first dimensioned 1 mm off
    Hexagons,
    /// separate circles each with a line Tangent to it, the line Horizontal while standing 2 deg off
    Tangents,
    /// one array: rectangles in a row, every width Equal to the first and each corner dimensioned 20 mm from the one
    /// before it along the row - one part, as big as the sketch
    Array,
    /// the separate shapes above in a mixture, chosen and tilted by a fixed pseudo-random sequence
    Mixed,
}

pub const KINDS: [Kind; 8] = [Kind::Lines, Kind::Arcs, Kind::Rectangles, Kind::Circles, Kind::Hexagons, Kind::Tangents, Kind::Array, Kind::Mixed];

/// Where the next record of the sketch goes.
struct Laying {
    p: Project,
    si: usize,
    /// the state of the pseudo-random sequence of `Kind::Mixed`
    seed: u64,
}

/// The place of a shape on the sheet: shapes are laid on a grid 20 mm apart.
#[derive(Clone, Copy)]
struct Cell {
    x: f64,
    y: f64,
    /// how far off its constraints the shape stands, deg
    tilt: f64,
}

impl Laying {
    fn new() -> Self {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("S");
        Laying { p, si, seed: 0x9E37_79B9_7F4A_7C15 }
    }

    /// A new id from the project itself: an id counted apart was handed out again by the project to the points and
    /// curves a tool makes, and a fillet of every corner made 118 942 loops of 500 rectangles.
    fn id(&mut self) -> u64 {
        self.p.alloc_id()
    }

    fn point(&mut self, x: f64, y: f64) -> u64 {
        let id = self.id();
        self.p.sketches[self.si].points.push(SketchPoint { id, x, y });
        id
    }

    fn entity(&mut self, kind: EntityKind) {
        let id = self.id();
        self.p.sketches[self.si].entities.push(SketchEntity { id, kind, construction: false });
    }

    fn constraint(&mut self, c: Constraint) {
        self.p.sketches[self.si].constraints.push(c);
    }

    /// The next number of the pseudo-random sequence, in 0..n.
    fn pick(&mut self, n: u64) -> u64 {
        self.seed = self.seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.seed >> 33) % n
    }

    fn line(&mut self, c: Cell) {
        let t = c.tilt.to_radians();
        let a = self.point(c.x, c.y);
        let b = self.point(c.x + 10.0 * t.cos(), c.y + 10.0 * t.sin());
        self.entity(qymcad_core::model::LineParts { a, b }.into());
        self.constraint(Constraint::Horizontal { a, b });
    }

    fn arc(&mut self, c: Cell) {
        let t = c.tilt.to_radians();
        let center = self.point(c.x + 5.0, c.y);
        let a = self.point(c.x + 5.0 + 5.0 * t.cos(), c.y + 5.0 * t.sin());
        let b = self.point(c.x + 5.0 - 5.0 * t.cos(), c.y + 5.0 * t.sin() + 0.2);
        self.entity(qymcad_core::model::ArcParts { center, a, b, ccw: true }.into());
        self.constraint(Constraint::Horizontal { a, b });
    }

    /// The four corners of a rectangle, its lines and its Horizontal and Vertical.
    fn rectangle(&mut self, c: Cell) -> [u64; 4] {
        let k = [self.point(c.x, c.y), self.point(c.x + 10.0, c.y), self.point(c.x + 10.0, c.y + 6.0), self.point(c.x, c.y + 6.0 + c.tilt * 0.1)];
        for i in 0..4 {
            self.entity(qymcad_core::model::LineParts { a: k[i], b: k[(i + 1) % 4] }.into());
        }
        self.constraint(Constraint::Horizontal { a: k[0], b: k[1] });
        self.constraint(Constraint::Horizontal { a: k[3], b: k[2] });
        self.constraint(Constraint::Vertical { a: k[1], b: k[2] });
        self.constraint(Constraint::Vertical { a: k[0], b: k[3] });
        k
    }

    fn sized_rectangle(&mut self, c: Cell) {
        let k = self.rectangle(c);
        self.constraint(distance(k[0], k[1], 11.0, 0));
    }

    fn circle(&mut self, c: Cell) {
        let center = self.point(c.x + 5.0, c.y + 5.0);
        self.entity(qymcad_core::model::CircleParts { center, r: 4.0 }.into());
        self.constraint(Constraint::Diameter { c: center, d: 4.5, off: 0.0, expr: String::new(), driven: false, diam: false, at: None });
    }

    fn hexagon(&mut self, c: Cell) {
        let t = c.tilt.to_radians();
        let k: Vec<u64> = (0..6)
            .map(|i| {
                let a = t + i as f64 * std::f64::consts::PI / 3.0;
                self.point(c.x + 5.0 + 5.0 * a.cos(), c.y + 5.0 + 5.0 * a.sin())
            })
            .collect();
        for i in 0..6 {
            self.entity(qymcad_core::model::LineParts { a: k[i], b: k[(i + 1) % 6] }.into());
        }
        for i in 1..6 {
            self.constraint(Constraint::Equal { a: k[0], b: k[1], c: k[i], d: k[(i + 1) % 6] });
        }
        self.constraint(distance(k[0], k[1], 6.0, 0));
    }

    fn tangent(&mut self, c: Cell) {
        let t = c.tilt.to_radians();
        let center = self.point(c.x + 5.0, c.y + 5.0);
        self.entity(qymcad_core::model::CircleParts { center, r: 3.0 }.into());
        let a = self.point(c.x, c.y + 1.5);
        let b = self.point(c.x + 10.0 * t.cos(), c.y + 1.5 + 10.0 * t.sin());
        self.entity(qymcad_core::model::LineParts { a, b }.into());
        self.constraint(Constraint::Tangent { a, b, c: center, r: 3.0 });
        self.constraint(Constraint::Horizontal { a, b });
    }
}

fn distance(a: u64, b: u64, d: f64, axis: u8) -> Constraint {
    Constraint::Distance { a, b, d, off: 2.0, expr: String::new(), driven: false, axis, at: None }
}

/// A sketch of `n` shapes of `kind`, each with a constraint not yet met. The sketch is the first of the project.
pub fn build(kind: Kind, n: usize) -> Project {
    let mut l = Laying::new();
    let side = (n as f64).sqrt().ceil() as usize;
    let mut first_width: Option<[u64; 2]> = None;
    let mut corner_before: Option<u64> = None;
    for k in 0..n {
        let cell = Cell { x: (k % side) as f64 * 20.0, y: (k / side) as f64 * 20.0, tilt: 2.0 };
        match kind {
            Kind::Lines => l.line(cell),
            Kind::Arcs => l.arc(cell),
            Kind::Rectangles => l.sized_rectangle(cell),
            Kind::Circles => l.circle(cell),
            Kind::Hexagons => l.hexagon(cell),
            Kind::Tangents => l.tangent(cell),
            Kind::Array => {
                let r = l.rectangle(Cell { x: k as f64 * 20.0, y: 0.0, tilt: 2.0 });
                match first_width {
                    None => {
                        l.constraint(distance(r[0], r[1], 11.0, 0));
                        first_width = Some([r[0], r[1]]);
                    }
                    Some([a, b]) => l.constraint(Constraint::Equal { a, b, c: r[0], d: r[1] }),
                }
                if let Some(before) = corner_before {
                    l.constraint(distance(before, r[0], 21.0, 1));
                }
                corner_before = Some(r[0]);
            }
            Kind::Mixed => {
                let cell = Cell { tilt: 1.0 + l.pick(5) as f64, ..cell };
                match l.pick(6) {
                    0 => l.line(cell),
                    1 => l.arc(cell),
                    2 => l.sized_rectangle(cell),
                    3 => l.circle(cell),
                    4 => l.hexagon(cell),
                    _ => l.tangent(cell),
                }
            }
        }
    }
    l.p
}
