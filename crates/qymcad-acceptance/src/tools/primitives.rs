//! THE PRIMITIVES DESCRIBED: a box, a cylinder, a sphere, a cone, a torus and a six-sided prism. Each is taken by its
//! button, takes no picks - its sizes are fields at the geometry - and stands on the table about the origin.
use crate::contract::fixtures::Fixture;
use crate::contract::{Class, Context, Entry, Field, Flow, Outcome, Tool, When};

/// A body of the part with the numbers `volume`, `faces` and `edges`, standing in the box `min`..`max`.
const fn body(volume: f64, faces: usize, edges: usize, min: [f64; 3], max: [f64; 3]) -> Outcome {
    Outcome::Body { volume, faces, edges, min, max }
}

/// A BOX: three lengths, standing on the table about the origin.
const fn box_of(x: f64, y: f64, z: f64) -> Outcome {
    body(x * y * z, 6, 12, [-x / 2.0, -y / 2.0, 0.0], [x / 2.0, y / 2.0, z])
}

fn box_long(x: f64) -> Outcome {
    box_of(x, 20.0, 20.0)
}

fn box_wide(y: f64) -> Outcome {
    box_of(20.0, y, 20.0)
}

fn box_tall(z: f64) -> Outcome {
    box_of(20.0, 20.0, z)
}

pub static BOX: Tool = Tool {
    id: "part.box",
    flow: Flow::Command,
    title: "cmd-box",
    entries: &[Entry::Button("tb-box-hint"), Entry::Search("cmd-box")],
    other: (Entry::Button("tb-cylinder-hint"), "cmd-cylinder"),
    fixture: Fixture::FirstPart,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    fields: &[
        Field { caption: "f-length-x", by_placeholder: false, when: When::Before, class: Class::Length, typical: 20.0, lo: 0.1, hi: 100000.0, zero: false, negative: false, outcome: box_long },
        Field { caption: "f-width-y", by_placeholder: false, when: When::Before, class: Class::Length, typical: 20.0, lo: 0.1, hi: 100000.0, zero: false, negative: false, outcome: box_wide },
        Field { caption: "f-height-z", by_placeholder: false, when: When::Before, class: Class::Length, typical: 20.0, lo: 0.1, hi: 100000.0, zero: false, negative: false, outcome: box_tall },
    ],
    words: &[],
    modes: &[],
    result: box_of(20.0, 20.0, 20.0),
    node: "Box3",
    undo: "cmd-operation",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    refused_together: None,
    budget: (30, 2000),
    help: "part/19-primitives",
    not_applicable: &[
        (2, "a box has no modes: its bar holds its three lengths and nothing else"),
        (3, "a box takes no picks: its sizes are fields at the geometry"),
        (4, "a box takes no picks, so there are none to refuse"),
        (14, "a box stands first in the timeline of its part: there is nothing above it to change"),
        (15, "a box stands on nothing: it is made of its three numbers alone"),
        (17, "every size the fields take builds: what the kernel cannot build is refused by the fields themselves (see 6)"),
    ],
};

/// The points a primitive does not answer to, and why: it has no modes, takes no picks, stands first on nothing.
const NOT_APPLICABLE: &[(u8, &str)] = &[
    (2, "a primitive has no modes: its bar holds its sizes and nothing else"),
    (3, "a primitive takes no picks: its sizes are fields at the geometry"),
    (4, "a primitive takes no picks, so there are none to refuse"),
    (14, "a primitive stands first in the timeline of its part: there is nothing above it to change"),
    (15, "a primitive stands on nothing: it is made of its numbers alone"),
];

/// A CYLINDER of radius `r` and height `h`, standing on the table about the origin: its side, its top and its bottom;
/// the two circles and the seam.
const fn cylinder_of(r: f64, h: f64) -> Outcome {
    body(std::f64::consts::PI * r * r * h, 3, 3, [-r, -r, 0.0], [r, r, h])
}

fn cylinder_r(r: f64) -> Outcome {
    cylinder_of(r, 20.0)
}

fn cylinder_h(h: f64) -> Outcome {
    cylinder_of(10.0, h)
}

pub static CYLINDER: Tool = Tool {
    id: "part.cylinder",
    flow: Flow::Command,
    title: "cmd-cylinder",
    entries: &[Entry::Button("tb-cylinder-hint"), Entry::Search("cmd-cylinder")],
    other: (Entry::Button("tb-box-hint"), "cmd-box"),
    fixture: Fixture::FirstPart,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    fields: &[
        Field { caption: "f-radius", by_placeholder: false, when: When::Before, class: Class::Radius, typical: 10.0, lo: 0.05, hi: 100000.0, zero: false, negative: false, outcome: cylinder_r },
        Field { caption: "f-height", by_placeholder: false, when: When::Before, class: Class::Length, typical: 20.0, lo: 0.1, hi: 100000.0, zero: false, negative: false, outcome: cylinder_h },
    ],
    words: &[],
    modes: &[],
    result: cylinder_of(10.0, 20.0),
    node: "Cylinder",
    undo: "cmd-operation",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    refused_together: None,
    budget: (30, 2000),
    help: "part/19-primitives",
    not_applicable: &[
        NOT_APPLICABLE[0],
        NOT_APPLICABLE[1],
        NOT_APPLICABLE[2],
        NOT_APPLICABLE[3],
        NOT_APPLICABLE[4],
        (17, "every size the fields take builds a cylinder: what cannot be built is refused by the fields themselves (see 6)"),
    ],
};

/// A SPHERE of radius `r` about the origin: one face, the seam and its two poles.
const fn sphere_of(r: f64) -> Outcome {
    body(4.0 / 3.0 * std::f64::consts::PI * r * r * r, 1, 3, [-r, -r, -r], [r, r, r])
}

fn sphere_r(r: f64) -> Outcome {
    sphere_of(r)
}

pub static SPHERE: Tool = Tool {
    id: "part.sphere",
    flow: Flow::Command,
    title: "cmd-sphere",
    entries: &[Entry::Button("tb-sphere-hint"), Entry::Search("cmd-sphere")],
    other: (Entry::Button("tb-box-hint"), "cmd-box"),
    fixture: Fixture::FirstPart,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    fields: &[Field { caption: "f-radius", by_placeholder: false, when: When::Before, class: Class::Radius, typical: 10.0, lo: 0.05, hi: 100000.0, zero: false, negative: false, outcome: sphere_r }],
    words: &[],
    modes: &[],
    result: sphere_of(10.0),
    node: "Sphere",
    undo: "cmd-operation",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    refused_together: None,
    budget: (30, 2000),
    help: "part/19-primitives",
    not_applicable: &[
        NOT_APPLICABLE[0],
        NOT_APPLICABLE[1],
        NOT_APPLICABLE[2],
        NOT_APPLICABLE[3],
        NOT_APPLICABLE[4],
        (17, "every radius the field takes builds a sphere: what cannot be built is refused by the field itself (see 6)"),
    ],
};

/// A CONE from `r1` at the bottom to `r2` at the top over `h`, standing on the table about the origin: a frustum has
/// its side and two flat ends, a full cone one end; the circles and the seam.
fn cone_of(r1: f64, r2: f64, h: f64) -> Outcome {
    let r = r1.max(r2);
    let (faces, edges) = if r2 > 0.0 && r1 > 0.0 { (3, 3) } else { (2, 3) };
    body(std::f64::consts::PI * h / 3.0 * (r1 * r1 + r1 * r2 + r2 * r2), faces, edges, [-r, -r, 0.0], [r, r, h])
}

fn cone_bottom(r1: f64) -> Outcome {
    cone_of(r1, 0.0, 20.0)
}

fn cone_top(r2: f64) -> Outcome {
    cone_of(10.0, r2, 20.0)
}

fn cone_h(h: f64) -> Outcome {
    cone_of(10.0, 0.0, h)
}

pub static CONE: Tool = Tool {
    id: "part.cone",
    flow: Flow::Command,
    title: "cmd-cone",
    entries: &[Entry::Button("tb-cone-hint"), Entry::Search("cmd-cone")],
    other: (Entry::Button("tb-box-hint"), "cmd-box"),
    fixture: Fixture::FirstPart,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    fields: &[
        Field { caption: "f-radius-bottom", by_placeholder: false, when: When::Before, class: Class::Radius, typical: 10.0, lo: 0.1, hi: 100000.0, zero: false, negative: false, outcome: cone_bottom },
        Field { caption: "f-radius-top", by_placeholder: false, when: When::Before, class: Class::Radius, typical: 0.0, lo: 0.0, hi: 100000.0, zero: true, negative: false, outcome: cone_top },
        Field { caption: "f-height", by_placeholder: false, when: When::Before, class: Class::Length, typical: 20.0, lo: 0.1, hi: 100000.0, zero: false, negative: false, outcome: cone_h },
    ],
    words: &[],
    modes: &[],
    result: body(std::f64::consts::PI * 20.0 / 3.0 * 100.0, 2, 3, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]),
    node: "Cone",
    undo: "cmd-operation",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    refused_together: None,
    budget: (30, 2000),
    help: "part/19-primitives",
    not_applicable: &[
        NOT_APPLICABLE[0],
        NOT_APPLICABLE[1],
        NOT_APPLICABLE[2],
        NOT_APPLICABLE[3],
        NOT_APPLICABLE[4],
        (17, "every size the fields take builds a cone: two radii of nothing are refused by the fields themselves (see 6)"),
    ],
};

/// A TORUS of ring `ring` and tube `tube` about the origin, lying on the table: one face, two seams.
const fn torus_of(ring: f64, tube: f64) -> Outcome {
    let out = ring + tube;
    body(2.0 * std::f64::consts::PI * std::f64::consts::PI * ring * tube * tube, 1, 2, [-out, -out, -tube], [out, out, tube])
}

fn torus_ring(ring: f64) -> Outcome {
    torus_of(ring, 4.0)
}

fn torus_tube(tube: f64) -> Outcome {
    torus_of(12.0, tube)
}

pub static TORUS: Tool = Tool {
    id: "part.torus",
    flow: Flow::Command,
    title: "cmd-torus",
    entries: &[Entry::Button("tb-torus-hint"), Entry::Search("cmd-torus")],
    other: (Entry::Button("tb-box-hint"), "cmd-box"),
    fixture: Fixture::FirstPart,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    fields: &[
        // the ring stays wider than the tube of 4 it holds, and the tube thinner than the ring of 12: a tube as thick
        // as its ring or thicker would pass through itself (see 17)
        Field { caption: "f-ring-r", by_placeholder: false, when: When::Before, class: Class::Radius, typical: 12.0, lo: 4.1, hi: 100000.0, zero: false, negative: false, outcome: torus_ring },
        Field { caption: "f-tube-r", by_placeholder: false, when: When::Before, class: Class::Radius, typical: 4.0, lo: 0.1, hi: 11.9, zero: false, negative: false, outcome: torus_tube },
    ],
    words: &[],
    modes: &[],
    result: torus_of(12.0, 4.0),
    node: "Torus",
    undo: "cmd-operation",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    // a ring of 3 about the tube of 4 is of the right form and passes through itself
    refusal: Some(3.0),
    refused_together: None,
    budget: (30, 2000),
    help: "part/19-primitives",
    not_applicable: NOT_APPLICABLE,
};

/// A PRISM of `n` sides in a circle of radius `r`, `h` tall, standing on the table about the origin: its sides, its
/// top and its bottom; three edges to a side.
fn prism_of(n: f64, r: f64, h: f64) -> Outcome {
    let k = n.round() as usize;
    let area = n / 2.0 * r * r * (2.0 * std::f64::consts::PI / n).sin();
    // the box of its corners, the first on +X: a triangle in r = 10 runs from x -5 to 10 and y -8.66 to 8.66, a hexagon
    // x -10 to 10 and y -8.66 to 8.66 - not the circle's box, which only a polygon with a corner on every axis fills
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for i in 0..k {
        let a = 2.0 * std::f64::consts::PI * i as f64 / n;
        let c = [r * a.cos(), r * a.sin()];
        for j in 0..2 {
            lo[j] = lo[j].min(c[j]);
            hi[j] = hi[j].max(c[j]);
        }
    }
    body(area * h, k + 2, 3 * k, [lo[0], lo[1], 0.0], [hi[0], hi[1], h])
}

fn prism_r(r: f64) -> Outcome {
    prism_of(6.0, r, 20.0)
}

fn prism_h(h: f64) -> Outcome {
    prism_of(6.0, 10.0, h)
}

fn prism_sides(n: f64) -> Outcome {
    prism_of(n, 10.0, 20.0)
}

pub static PRISM: Tool = Tool {
    id: "part.prism",
    flow: Flow::Command,
    title: "cmd-prism",
    entries: &[Entry::Button("tb-prism-hint"), Entry::Search("cmd-prism")],
    other: (Entry::Button("tb-box-hint"), "cmd-box"),
    fixture: Fixture::FirstPart,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    fields: &[
        Field { caption: "f-radius-circ", by_placeholder: false, when: When::Before, class: Class::Radius, typical: 10.0, lo: 0.1, hi: 100000.0, zero: false, negative: false, outcome: prism_r },
        Field { caption: "f-height", by_placeholder: false, when: When::Before, class: Class::Length, typical: 20.0, lo: 0.1, hi: 100000.0, zero: false, negative: false, outcome: prism_h },
        Field { caption: "cmd-sides", by_placeholder: false, when: When::Before, class: Class::Count, typical: 6.0, lo: 3.0, hi: 64.0, zero: false, negative: false, outcome: prism_sides },
    ],
    words: &[],
    modes: &[],
    result: body(1.5 * 1.7320508075688772 * 100.0 * 20.0, 8, 18, [-10.0, -8.660254037844387, 0.0], [10.0, 8.660254037844387, 20.0]),
    node: "Prism",
    undo: "cmd-operation",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    refused_together: None,
    budget: (30, 2000),
    help: "part/19-primitives",
    not_applicable: &[
        NOT_APPLICABLE[0],
        NOT_APPLICABLE[1],
        NOT_APPLICABLE[2],
        NOT_APPLICABLE[3],
        NOT_APPLICABLE[4],
        (17, "every size the fields take builds a prism: what cannot be built is refused by the fields themselves (see 6)"),
    ],
};
