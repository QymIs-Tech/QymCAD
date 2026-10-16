//! The tools that run a profile along: sweep, loft, thread.

use super::*;

/// What a sweep or a loft leaves out: it works on sketches picked in the tree, and takes no value.
pub(crate) const FROM_SKETCHES: &[(u8, &str)] = &[
    (4, "the sections are rows of the tree; a click on empty space there picks nothing to refuse"),
    (5, "the tool takes no value: the shape is given by the sketches"),
    (6, "the tool takes no value to refuse"),
    (14, "the change above is a change of a sketch, which is the sketch's own contract of dimensions"),
    (15, "the sketches it stands on are deleted from the tree, which is the sketch's own matter"),
    (16, "the fixture builds its own part from the start: there is no second part to take it in"),
    (17, "sections that the kernel cannot join are a matter of the sketches, not of a value typed"),
];

/// SWEEP: the square of 10 swept along the straight line of 40 up the front plane - a bar of 10 x 10 x 40.
pub static SWEEP: Tool = Tool {
    id: "part.sweep",
    flow: Flow::Command,
    title: "cmd-sweep",
    entries: &[Entry::Button("tb-sweep-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-extrude-hint"), "cmd-extrude"),
    fixture: Fixture::SweepStraight,
    picks: &[Pick::Row("Sketch 2")],
    pick_trial: &[Pick::Row("Sketch 2")],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Body { volume: 4000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [10.0, 10.0, 40.0] },
    node: "Sweep",
    undo: "cmd-sweep",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[],
    refusal: None,
    budget: (30, 2000),
    help: "part/03-sweep",
    not_applicable: FROM_SKETCHES,
};

/// LOFT: from the square of 10 on the table to the square of 20 thirty above - a frustum of 30 / 3 * (100 + 400 +
/// sqrt(100 * 400)) = 7000.
pub static LOFT: Tool = Tool {
    id: "part.loft",
    flow: Flow::Command,
    title: "cmd-loft",
    entries: &[Entry::Button("tb-loft-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-extrude-hint"), "cmd-extrude"),
    fixture: Fixture::LoftSquares,
    picks: &[Pick::Row("Sketch 2")],
    pick_trial: &[Pick::Row("Sketch 2")],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Body { volume: 7000.0, faces: 6, edges: 12, min: [-5.0, -5.0, 0.0], max: [15.0, 15.0, 30.0] },
    node: "Loft",
    undo: "cmd-loft",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[],
    refusal: None,
    budget: (30, 2000),
    help: "part/04-loft",
    not_applicable: FROM_SKETCHES,
};

/// The cylinder of 20 across and 10 tall with an outer M20 (pitch 2.5) cut over `length` from its top: what the groove
/// takes is the standard's own profile (ISO 68-1), with no fit clearance and no run-out, to a hundredth.
pub(crate) fn threaded(length: f64) -> Outcome {
    let groove = crate::iso::outer_groove(20.0, 2.5, length);
    // the profile swept by Pappus is the average over whole turns; the two ends of a helix cut over part of a turn
    // stray from it by less than a cubic millimetre (0.57 measured over 0.5 of length, a fifth of a pitch)
    Outcome::Volume { volume: PI * 100.0 * 10.0 - groove, tol: groove * 0.01 + 1.0, min: [10.0, 5.0, 0.0], max: [30.0, 25.0, 10.0] }
}

/// The words of a thread made to the standard: the size of the face, no clearance, no run-in or run-out.
pub(crate) fn to_the_standard(_text: &str) -> Outcome {
    threaded(10.0)
}

/// THREAD: an outer M20 the whole 10 of the cylinder - the size typed to match the face (20 across), the fit clearance
/// and the run-in and run-out set to nothing, so the groove is the standard's profile: pi * 100 * 10 - 469.11 =
/// 2672.48, to a hundredth of the groove. The matrix of threads carries the inner thread, another size and a flat face.
pub static THREAD: Tool = Tool {
    id: "part.thread",
    flow: Flow::Command,
    title: "cmd-thread",
    entries: &[Entry::Button("tb-thread-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::CylinderBody,
    picks: &[Pick::Face([27.071, 7.929, 5.0])],
    pick_trial: &[Pick::Face([27.071, 7.929, 5.0])],
    wrong_picks: &[],
    words: &[
        Words { caption: "f-nominal-d", by_placeholder: false, typical: "20", valid: &[], invalid: &[], outcome: to_the_standard, enter: false },
        Words { caption: "th-fit-clearance", by_placeholder: false, typical: "0", valid: &[], invalid: &[], outcome: to_the_standard, enter: false },
        Words { caption: "th-lead-in", by_placeholder: false, typical: "0", valid: &[], invalid: &[], outcome: to_the_standard, enter: false },
        Words { caption: "th-lead-out", by_placeholder: false, typical: "0", valid: &[], invalid: &[], outcome: to_the_standard, enter: false },
    ],
    fields: &[Field { caption: "f-length", by_placeholder: false, when: When::Before, class: Class::Length, typical: 10.0, lo: 0.5, hi: 10.0, zero: false, negative: false, outcome: threaded }],
    modes: &[],
    // pi * 100 * 10 - 469.11 (the groove of M20 x 2.5 over 10, worked out by Pappus from the ISO 68-1 profile)
    result: Outcome::Volume { volume: 2672.479, tol: 5.69, min: [10.0, 5.0, 0.0], max: [30.0, 25.0, 10.0] },
    node: "Thread",
    undo: "cmd-thread",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[],
    refusal: None,
    budget: (60, 2000),
    help: "part/10-thread",
    not_applicable: &[
        (2, "the kinds of thread in the bar (outer, inner, standards) are the matrix of threads' cases"),
        (14, "the cylinder under the thread is made by the fixture's own extrusion; a change of it is the matrix's matter"),
        (15, "deleting the extrusion under the thread is the matrix of threads' matter"),
        (16, "the fixture builds its own part from the start: there is no second part to take it in"),
        (17, "a thread of another size than the face is refused - that is the matrix of threads' case"),
    ],
};

/// The cylinder with an outer thread of 20 over its whole 10, cut at a pitch of one's own - the coarse pitch a printed
/// thread wants: the groove is the ISO 60-degree profile drawn at that pitch, so the same Pappus sum gives it.
pub(crate) fn threaded_at(pitch: f64) -> Outcome {
    // a pitch of 0 is the standard's own, as the field's caption says: 2.5 for M20
    let pitch = if pitch == 0.0 { 2.5 } else { pitch };
    let groove = crate::iso::outer_groove(20.0, pitch, 10.0);
    Outcome::Volume { volume: PI * 100.0 * 10.0 - groove, tol: groove * 0.01 + 1.0, min: [10.0, 5.0, 0.0], max: [30.0, 25.0, 10.0] }
}

/// THREAD_OWN_PITCH: the same M20 over the whole 10, at a pitch of 4 typed in place of the standard 2.5 - a thread for a
/// printed part. A pitch finer than 1 does not print and one coarser than 5 leaves no thread on a shaft of 20; a pitch
/// ten times over that must be refused.
pub static THREAD_OWN_PITCH: Tool = Tool {
    id: "part.thread-own-pitch",
    flow: Flow::Command,
    title: "cmd-thread",
    entries: &[Entry::Button("tb-thread-hint")],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::CylinderBody,
    picks: &[Pick::Face([27.071, 7.929, 5.0])],
    pick_trial: &[Pick::Face([27.071, 7.929, 5.0])],
    wrong_picks: &[],
    words: &[
        Words { caption: "f-nominal-d", by_placeholder: false, typical: "20", valid: &[], invalid: &[], outcome: to_the_standard, enter: false },
        Words { caption: "th-fit-clearance", by_placeholder: false, typical: "0", valid: &[], invalid: &[], outcome: to_the_standard, enter: false },
        Words { caption: "th-lead-in", by_placeholder: false, typical: "0", valid: &[], invalid: &[], outcome: to_the_standard, enter: false },
        Words { caption: "th-lead-out", by_placeholder: false, typical: "0", valid: &[], invalid: &[], outcome: to_the_standard, enter: false },
        Words { caption: "f-length", by_placeholder: false, typical: "10", valid: &[], invalid: &[], outcome: to_the_standard, enter: false },
    ],
    fields: &[Field { caption: "f-pitch-std", by_placeholder: false, when: When::Before, class: Class::Length, typical: 4.0, lo: 1.0, hi: 5.0, zero: true, negative: false, outcome: threaded_at }],
    modes: &[],
    // pi * 100 * 10 - 722.52 (the ISO profile drawn at a pitch of 4 over 10, by Pappus)
    result: Outcome::Volume { volume: 2419.076, tol: 8.23, min: [10.0, 5.0, 0.0], max: [30.0, 25.0, 10.0] },
    node: "Thread",
    undo: "cmd-thread",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[],
    refusal: None,
    budget: (60, 2000),
    help: "part/10-thread",
    not_applicable: &[
        (2, "the kinds of thread in the bar are the matrix of threads' cases"),
        (14, "the cylinder under the thread is made by the fixture's own extrusion; a change of it is the matrix's matter"),
        (15, "deleting the extrusion under the thread is the matrix of threads' matter"),
        (16, "the fixture builds its own part from the start: there is no second part to take it in"),
        (17, "a thread of another size than the face is refused - that is the matrix of threads' case"),
    ],
};
