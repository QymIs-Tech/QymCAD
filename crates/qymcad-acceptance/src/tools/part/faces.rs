//! The tools that move or cut faces: push a face, thicken, split a body or a face, draft.

use super::*;

/// The block with its top pushed along its own normal by `offset`: out for more, in for less. The neighbours follow,
/// so a block stays a block.
pub(crate) const fn pushed(offset: f64) -> Outcome {
    Outcome::Body { volume: 1200.0 * (10.0 + offset), faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0 + offset] }
}

/// PUSH FACE: the top of the block moved 5 out. A minus moves it in; in by the whole 10 of the height there would be
/// no body left, which the kernel refuses.
pub static PUSH_FACE: Tool = Tool {
    id: "part.push-face",
    flow: Flow::Command,
    title: "f-push-face",
    entries: &[Entry::Button("tb-push-face-hint"), Entry::Search("f-push-face")],
    other: (Entry::Button("tb-fillet-body-hint"), "cmd-fillet"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "f-offset", by_placeholder: false, when: When::Before, class: Class::Length, typical: 5.0, lo: -9.5, hi: 100000.0, zero: false, negative: true, outcome: pushed }],
    modes: &[],
    result: pushed(5.0),
    node: "PushFace",
    undo: "f-push-face",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Body { volume: 1200.0 * 25.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 25.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: Some(-10.0),
    budget: (30, 2000),
    help: "part/11-push-face",
    not_applicable: &[(2, "the bar of push face holds no modes: it takes a flat face and an offset, plus or minus")],
};

/// The block with a plate of `thickness` grown on its top: the sides of the plate stand in the sides of the block, so
/// a block made taller keeps the six faces of a block.
pub(crate) const fn thickened(thickness: f64) -> Outcome {
    Outcome::Body { volume: 12000.0 + 1200.0 * thickness, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0 + thickness] }
}

/// THICKEN: the top of the block grown into a plate 2 thick, joined to the body - the part stays one body.
pub static THICKEN: Tool = Tool {
    id: "part.thicken",
    flow: Flow::Command,
    title: "feat-name-thicken",
    entries: &[Entry::Button("tb-thicken-hint"), Entry::Search("feat-name-thicken")],
    other: (Entry::Button("tb-fillet-body-hint"), "cmd-fillet"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "f-thickness", by_placeholder: false, when: When::Before, class: Class::Length, typical: 2.0, lo: 0.1, hi: 100000.0, zero: false, negative: false, outcome: thickened }],
    modes: &[],
    result: thickened(2.0),
    node: "Thicken",
    undo: "f-operation",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Body { volume: 24000.0 + 2400.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 22.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/15-thicken",
    not_applicable: &[
        (2, "the bar of thicken holds no modes: it takes a face and a thickness, and the plate grows outwards"),
        (17, "a plate grown on a face always builds: the thickness has no value the kernel cannot make of it"),
    ],
};

/// The block cut across by a plane `depth` below its top: two pieces of 1200 mm^2 by what is left of the 10 of
/// height, together the block itself.
pub(crate) const fn cut_at(depth: f64) -> Outcome {
    let (upper, lower) = (1200.0 * depth, 1200.0 * (10.0 - depth));
    Outcome::Bodies { count: 2, volume: 12000.0, largest: if upper > lower { upper } else { lower } }
}

/// SPLIT_BODY: the block cut in two by a plane taken from its own top face, 5 below it.
pub static SPLIT_BODY: Tool = Tool {
    id: "part.split-body",
    flow: Flow::Command,
    title: "feat-name-split-body",
    entries: &[Entry::Button("tb-split-body-hint"), Entry::Search("feat-name-split-body")],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    // the offset runs along the normal of the face taken, so a cut inside the block is a negative one: 0 lies on the
    // face itself and -10 on the bottom, and both are the plane touching the body rather than cutting it
    fields: &[Field { caption: "f-offset", by_placeholder: false, when: When::Before, class: Class::Length, typical: -5.0, lo: -9.9, hi: -0.1, zero: false, negative: true, outcome: cut_at_below }],
    modes: &[],
    result: cut_at(5.0),
    node: "SplitBody",
    undo: "f-operation",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Bodies { count: 2, volume: 24000.0, largest: 18000.0 },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/13-split-body",
    not_applicable: &[
        (2, "the bar of the cut holds no modes: it takes a plane, a datum or a face and an offset from it"),
        (17, "a plane that cuts the body always cuts it; a plane that misses it is refused in words, which is the point about values"),
    ],
};

/// The same cut read from the offset a person types: below the top face by `offset`, taken as a depth.
pub(crate) fn cut_at_below(offset: f64) -> Outcome {
    cut_at(-offset)
}

/// SPLIT_FACE: the same plane, but the body stays one and its four sides are each cut in two - 6 faces become 10,
/// 12 edges become 20 (the four sides split, and the seam runs round the body).
pub static SPLIT_FACE: Tool = Tool {
    id: "part.split-face",
    flow: Flow::Command,
    title: "feat-name-split-face",
    entries: &[Entry::Button("tb-split-face-hint"), Entry::Search("feat-name-split-face")],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "f-offset", by_placeholder: false, when: When::Before, class: Class::Length, typical: -5.0, lo: -9.9, hi: -0.1, zero: false, negative: true, outcome: split_faces }],
    modes: &[],
    result: split_faces(-5.0),
    node: "SplitFace",
    undo: "f-operation",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Body { volume: 24000.0, faces: 10, edges: 20, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 20.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/14-split-face",
    not_applicable: &[
        (2, "the bar of the face cut holds no modes: it takes a plane, a datum or a face and an offset from it"),
        (17, "a plane that crosses the body always divides its faces; a plane that misses it is refused in words, which is the point about values"),
    ],
};

/// The block with its sides divided by a plane: the body and its box are untouched, the faces and edges are not.
pub(crate) const fn split_faces(_offset: f64) -> Outcome {
    Outcome::Body { volume: 12000.0, faces: 10, edges: 20, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] }
}

/// The block with the side at y = 0 tilted by `angle` about its top edge, where the neutral face holds it: the face
/// leans out by 10 * tan(angle) at the bottom, and the wedge under it is 40 * 10 * 10 * tan(angle) / 2.
pub(crate) fn drafted(angle: f64) -> Outcome {
    let lean = 10.0 * (angle * PI / 180.0).tan();
    // a face that leans out carries the body past y = 0; one that leans in leaves the neutral edge where it was
    let low = if lean > 0.0 { -lean } else { 0.0 };
    Outcome::Body { volume: 12000.0 + 200.0 * lean, faces: 6, edges: 12, min: [0.0, low, 0.0], max: [40.0, 30.0, 10.0] }
}

/// DRAFT: one side of the block tilted 3 degrees, the top face holding the edge it turns about.
pub static DRAFT: Tool = Tool {
    id: "part.draft",
    flow: Flow::Command,
    title: "f-draft",
    entries: &[Entry::Button("tb-draft-hint"), Entry::Search("feat-name-draft")],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    // the face to tilt, then the word that says the next click is the neutral one, then the face that holds still
    picks: &[Pick::Face([20.0, 0.0, 5.0]), Pick::Bar("cmd-neutral-face"), Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Face([20.0, 0.0, 5.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "f-angle", by_placeholder: false, when: When::Before, class: Class::Angle, typical: 3.0, lo: -60.0, hi: 60.0, zero: false, negative: true, outcome: drafted }],
    modes: &[],
    // 12000 + 200 * 10 * tan(3 deg) = 12104.82
    result: Outcome::Body { volume: 12104.8155585661, faces: 6, edges: 12, min: [0.0, -0.5240777730942, 0.0], max: [40.0, 30.0, 10.0] },
    node: "Draft",
    // the step of undo is named by the tool, as the window writes it: "Draft", not the "Operation" of the others
    undo: "f-draft",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        // the wedge grows with the height it leans over: 24000 + 40 * 20 * 20 * tan(3 deg) / 2
        then: Outcome::Body { volume: 24419.2622342644, faces: 6, edges: 12, min: [0.0, -1.0481555461884, 0.0], max: [40.0, 30.0, 20.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/09-draft",
    not_applicable: &[(17, "a face tilted by an angle the field accepts always builds: what the kernel could not make of it is a matter of the faces beside it, not of the value typed")],
};
