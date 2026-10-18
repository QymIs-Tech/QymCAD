//! THE LIMITING CASES OF CHAMFERS AND FILLETS: a size that eats a face whole - a chamfer leg as high as the side it
//! lies on, a radius as high as the step it rounds, two roundings meeting across a face. A person drawing to a
//! dimension lands there on purpose. Each case is built and its volume measured against the one the drawing gives; no
//! face or edge a hair wide may stand where the eaten face was, and two rebuilds in a row give the same names. The
//! faults of all the cases are told together.
use qymcad_core::feature::ChamferMode;
use qymcad_core::model::{ChamferShape, Id, Project};
use std::f64::consts::PI;

/// The id of the edge of `body` whose middle stands nearest `at`, within 0.2 mm.
fn edge_near(p: &Project, body: Id, at: [f64; 3]) -> Option<u32> {
    let d = |m: [f64; 3]| (0..3).map(|i| (m[i] - at[i]).powi(2)).sum::<f64>().sqrt();
    p.regen_edges.get(&body)?.iter().filter(|e| d(e.mid) < 0.2).min_by(|a, b| d(a.mid).total_cmp(&d(b.mid))).map(|e| e.id)
}

/// A BODY AND WHAT IS LAID ON IT: the body, the middles of the edges taken, the blend, and the volume it leaves.
struct Case {
    what: &'static str,
    body: Body,
    edges: &'static [[f64; 3]],
    blend: Blend,
    volume: f64,
}

/// A BLOCK (centred on the axis, standing on XY), OR A SECTION drawn on XY and extruded along Z.
#[derive(Clone, Copy)]
enum Body {
    Block([f64; 3]),
    Section(Section),
}

/// THE CORNERS OF A SECTION and how far it is extruded.
#[derive(Clone, Copy)]
struct Section {
    corners: &'static [[f64; 2]],
    length: f64,
}

/// A trapezoid 40 wide, 10 high, its left side leaning at 60 deg from the base: 10 / tan 60 in from the left at the top.
const LEAN: f64 = 5.773502691896258;
const TRAPEZOID: &[[f64; 2]] = &[[0.0, 0.0], [40.0, 0.0], [40.0, 10.0], [LEAN, 10.0]];
/// The length of the leaning side, 10 / sin 60.
const SIDE: f64 = 11.547005383792516;

/// A ROUNDING OF A RADIUS, OR A CHAMFER OF TWO LEGS laid one way or the other.
#[derive(Clone, Copy)]
enum Blend {
    Fillet(f64),
    Chamfer(Legs),
}

/// THE LEGS OF A CHAMFER and which face the first lies on.
#[derive(Clone, Copy)]
struct Legs {
    first: f64,
    second: f64,
    flip: bool,
}

/// What a rounding of `r` takes off the section of an edge between two faces at a right angle.
fn rounded_off(r: f64) -> f64 {
    (1.0 - PI / 4.0) * r * r
}

fn cases() -> Vec<Case> {
    let block = 40.0 * 30.0 * 10.0;
    let thin = 40.0 * 10.0 * 10.0;
    let trapezoid = (40.0 + 40.0 - LEAN) / 2.0 * 10.0 * 20.0;
    let legs = |first: f64, second: f64, flip: bool| Blend::Chamfer(Legs { first, second, flip });
    vec![
        Case {
            what: "a chamfer 10 x 10 down the whole side of a block 10 high",
            body: Body::Block([40.0, 30.0, 10.0]),
            edges: &[[0.0, -15.0, 10.0]],
            blend: legs(10.0, 10.0, false),
            volume: block - 0.5 * 100.0 * 40.0,
        },
        Case {
            what: "a chamfer 10 x 5, its leg of 10 down the side 10 high",
            body: Body::Block([40.0, 30.0, 10.0]),
            edges: &[[0.0, -15.0, 10.0]],
            blend: legs(10.0, 5.0, false),
            volume: block - 0.5 * 50.0 * 40.0,
        },
        Case { what: "a chamfer 10 x 5 laid the other way", body: Body::Block([40.0, 30.0, 10.0]), edges: &[[0.0, -15.0, 10.0]], blend: legs(10.0, 5.0, true), volume: block - 0.5 * 50.0 * 40.0 },
        Case {
            what: "a rounding R10 down the whole side of a block 10 high",
            body: Body::Block([40.0, 30.0, 10.0]),
            edges: &[[0.0, -15.0, 10.0]],
            blend: Blend::Fillet(10.0),
            volume: block - rounded_off(10.0) * 40.0,
        },
        Case {
            what: "two roundings R5 meeting across a top 10 wide",
            body: Body::Block([40.0, 10.0, 10.0]),
            edges: &[[0.0, -5.0, 10.0], [0.0, 5.0, 10.0]],
            blend: Blend::Fillet(5.0),
            volume: thin - 2.0 * rounded_off(5.0) * 40.0,
        },
        Case {
            what: "two chamfers 5 meeting across a top 10 wide",
            body: Body::Block([40.0, 10.0, 10.0]),
            edges: &[[0.0, -5.0, 10.0], [0.0, 5.0, 10.0]],
            blend: legs(5.0, 5.0, false),
            volume: thin - 2.0 * 0.5 * 25.0 * 40.0,
        },
        // the corner of 120 deg at the top of the leaning side: legs of SIDE on it and 5 on the top take off a triangle
        // of 0.5 x SIDE x 5 x sin 120 = 25; the leaning side eaten whole
        Case {
            what: "a chamfer eating a side leaning at 60 deg, its corner 120 deg",
            body: Body::Section(Section { corners: TRAPEZOID, length: 20.0 }),
            edges: &[[LEAN, 10.0, 10.0]],
            blend: legs(SIDE, 5.0, false),
            volume: trapezoid - 25.0 * 20.0,
        },
        Case {
            what: "the same chamfer laid the other way",
            body: Body::Section(Section { corners: TRAPEZOID, length: 20.0 }),
            edges: &[[LEAN, 10.0, 10.0]],
            blend: legs(SIDE, 5.0, true),
            volume: trapezoid - 25.0 * 20.0,
        },
        // the corner of 60 deg at the foot of the leaning side: a radius touching it SIDE up, at its top
        Case {
            what: "a rounding of a corner of 60 deg touching the leaning side at its top",
            body: Body::Section(Section { corners: TRAPEZOID, length: 20.0 }),
            edges: &[[0.0, 0.0, 10.0]],
            blend: Blend::Fillet(SIDE * (PI / 6.0).tan()),
            volume: trapezoid - Rounding { radius: SIDE * (PI / 6.0).tan(), angle: PI / 3.0 }.off() * 20.0,
        },
    ]
}

/// A ROUNDING OF A CORNER: its radius and the angle between its faces.
struct Rounding {
    radius: f64,
    angle: f64,
}

impl Rounding {
    /// What it takes off the section of its edge: the two right triangles from the edge to the centre less the sector
    /// between the touching points.
    fn off(&self) -> f64 {
        let touch = self.radius / (self.angle / 2.0).tan();
        touch * self.radius - 0.5 * self.radius * self.radius * (PI - self.angle)
    }
}

/// The node of `case` laid on its block, built; the faults it shows.
fn check(case: &Case) -> Vec<String> {
    let mut out = Vec::new();
    let mut p = Project::default();
    p.new_document();
    let block = match case.body {
        Body::Block([dx, dy, dz]) => p.add_box(dx, dy, dz),
        Body::Section(Section { corners, length }) => {
            let sketch = p.add_line_sketch("Section", corners.iter().map(|c| qymcad_core::geom::Point2::new(c[0], c[1])).collect(), true);
            let Some(si) = p.sketch_index(sketch) else { return vec![format!("{}: setup - no sketch", case.what)] };
            p.regen_sketch(si);
            p.add_sketch_node(sketch, "Section");
            let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
            p.add_extrude_multi(sketch, closed, length, qymcad_core::feature::Reach::Forward, 0.0, vec![])
        }
    };
    let _ = qymcad_testkit::regenerate(&mut p);
    let Some(edges) = case.edges.iter().map(|at| edge_near(&p, block, *at)).collect::<Option<Vec<u32>>>() else {
        return vec![format!("{}: setup - no edge stands where the case lays it", case.what)];
    };
    let node = match case.blend {
        Blend::Fillet(r) => p.add_fillet(block, r, edges),
        Blend::Chamfer(Legs { first, second, flip }) => p.add_chamfer_ex(block, first, ChamferShape { mode: ChamferMode::TwoDist, d2: second, flip, ref_face: 0 }, edges),
    };
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    if let Some(e) = p.regen_errors.get(&node) {
        return vec![format!("{}: refused - {e:?}", case.what)];
    }
    if let Some(w) = p.regen_warnings.get(&node) {
        out.push(format!("{}: yellow - {w:?}", case.what));
    }
    let volume = shapes.get(&node).map(|s| s.volume()).unwrap_or(0.0);
    // a build exact to the drawing is off by 1e-12 of its volume; one stepped back a hair short of the size and its strip
    // taken out, by 2e-9 (measured on the chamfer 10 x 10)
    if (volume - case.volume).abs() > 1e-10 * case.volume {
        out.push(format!("{}: a volume of {volume:.9}, the drawing gives {:.9} ({:+.2e})", case.what, case.volume, volume - case.volume));
    }
    let faces = p.regen_faces.get(&node).cloned().unwrap_or_default();
    if let Some(f) = faces.iter().find(|f| f.area < 1e-3) {
        out.push(format!("{}: a face of {:.2e} mm^2 stands where the eaten face was ({})", case.what, f.area, p.names.describe(f.id)));
    }
    let edges_now = p.regen_edges.get(&node).cloned().unwrap_or_default();
    if let Some(e) = edges_now.iter().find(|e| (0..3).map(|k| (e.a[k] - e.b[k]).powi(2)).sum::<f64>().sqrt() < 1e-4 && e.radius == 0.0) {
        out.push(format!("{}: an edge a hair long ({})", case.what, p.names.describe(e.id)));
    }
    let names = |p: &Project| -> Vec<u32> {
        let mut v: Vec<u32> = p.regen_faces.get(&node).map(|fs| fs.iter().map(|f| f.id).collect()).unwrap_or_default();
        v.sort_unstable();
        v
    };
    let first = names(&p);
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let _ = qymcad_testkit::regenerate(&mut p);
    if names(&p) != first {
        out.push(format!("{}: a second rebuild renamed its faces", case.what));
    }
    let _ = report;
    out
}

#[test]
fn every_limiting_blend_is_built_as_drawn() {
    let fails: Vec<String> = cases().iter().flat_map(check).collect();
    assert!(fails.is_empty(), "{} faults:\n  {}", fails.len(), fails.join("\n  "));
}

/// A LIMITING BLEND BUILT A HAIR SHORT SAYS SO. Four roundings of R5 on the top of a cube of 10 meet at its corners -
/// a corner is a blend of its own, which the exact construction does not take - and are built stepped back: the node
/// stands yellow with the words, not green off its drawing.
#[test]
fn a_limiting_blend_built_a_hair_short_says_so() {
    let mut p = Project::default();
    p.new_document();
    let cube = p.add_box(10.0, 10.0, 10.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    let top: Vec<u32> = p.regen_edges[&cube].iter().filter(|e| (e.a[2] - 10.0).abs() < 1e-9 && (e.b[2] - 10.0).abs() < 1e-9).map(|e| e.id).collect();
    assert_eq!(top.len(), 4, "setup: the top of the cube has four edges");
    let node = p.add_fillet(cube, 5.0, top);
    let _ = qymcad_testkit::regenerate(&mut p);
    assert!(!p.regen_errors.contains_key(&node), "the rounding went red: {:?}", p.regen_errors.get(&node));
    assert_eq!(p.regen_warnings.get(&node), Some(&qymcad_core::errors::CoreError::BuiltSteppedBack), "the rounding built a hair short does not say so");
}
