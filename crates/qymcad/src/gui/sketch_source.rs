//! THE WORKBENCHES AS ONE TEXT, for the checks that read them.
//!
//! A dozen checks assert that a tool is wired, that a value is parsed through one door, that a properties
//! card starts with the shared header - and they do it by reading the source. Splitting the workbenches
//! into crates must not weaken them.
//!
//! THIS IS THE CURE FOR A DEBT, not a convenience. A guard that reads a file BY NAME guards that file and
//! no other: when the drawing moved to a crate, nineteen checks went on reading two hundred lines of what
//! used to be four thousand - green, and blind. Here the list of files is gathered in one place, and the
//! next move edits it once.
//!
//! THESE LISTS OVERLAP: `PANELS` carries the workbench crates too, because that is where the buttons are.
//! A check that COUNTS occurrences must therefore be handed ONE of them, not both - otherwise one
//! definition is counted twice and reported as a copy that does not exist.

/// The sketch, whole: the workbench crate plus what stayed with the application.
#[cfg(test)]
pub(crate) const SKETCH: &str = concat!(
    concat!(
        include_str!("../../../qymcad-sketch/src/clicks.rs"),
        "\n",
        include_str!("../../../qymcad-sketch/src/constraints.rs"),
        "\n",
        include_str!("../../../qymcad-sketch/src/corners.rs"),
        "\n",
        include_str!("../../../qymcad-sketch/src/dims.rs"),
        "\n",
        include_str!("../../../qymcad-sketch/src/drag.rs"),
        "\n",
        include_str!("../../../qymcad-sketch/src/extend.rs"),
        "\n",
        include_str!("../../../qymcad-sketch/src/menu.rs"),
        "\n",
        include_str!("../../../qymcad-sketch/src/offset.rs"),
        "\n",
        include_str!("../../../qymcad-sketch/src/popups.rs"),
        "\n",
        include_str!("../../../qymcad-sketch/src/props.rs"),
        "\n",
        include_str!("../../../qymcad-sketch/src/snap.rs"),
        "\n",
        include_str!("../../../qymcad-sketch/src/ties.rs"),
        "\n",
        include_str!("../../../qymcad-sketch/src/lib.rs")
    ),
    "\n",
    include_str!("sketching.rs"),
);

/// The picking, whole: the module crate plus the remnant in the application.
#[cfg(test)]
pub(crate) const PICK: &str = concat!(
    concat!(
        include_str!("../../../qymcad-pick/src/body_hits.rs"),
        "\n",
        include_str!("../../../qymcad-pick/src/infer.rs"),
        "\n",
        include_str!("../../../qymcad-pick/src/sketch_hits.rs"),
        "\n",
        include_str!("../../../qymcad-pick/src/lib.rs")
    ),
    "\n",
    include_str!("pick.rs"),
);

/// The Part, whole: the workbench crate plus what stayed with the application.
#[cfg(test)]
pub(crate) const PART: &str = concat!(
    concat!(
        concat!(
            include_str!("../../../qymcad-part/src/apply/copies.rs"),
            "\n",
            include_str!("../../../qymcad-part/src/apply/faces.rs"),
            "\n",
            include_str!("../../../qymcad-part/src/apply/solids.rs"),
            "\n",
            include_str!("../../../qymcad-part/src/apply/update.rs"),
            "\n",
            include_str!("../../../qymcad-part/src/apply.rs")
        ),
        "\n",
        concat!(
            include_str!("../../../qymcad-part/src/bars/feat_bar.rs"),
            "\n",
            include_str!("../../../qymcad-part/src/bars/options.rs"),
            "\n",
            include_str!("../../../qymcad-part/src/bars/toolbar.rs"),
            "\n",
            include_str!("../../../qymcad-part/src/bars.rs")
        ),
        "\n",
        include_str!("../../../qymcad-part/src/params.rs"),
        "\n",
        include_str!("../../../qymcad-part/src/picks.rs"),
        "\n",
        include_str!("../../../qymcad-part/src/popups.rs"),
        "\n",
        include_str!("../../../qymcad-part/src/reopen.rs"),
        "\n",
        include_str!("../../../qymcad-part/src/start.rs"),
        "\n",
        include_str!("../../../qymcad-part/src/topology.rs"),
        "\n",
        include_str!("../../../qymcad-part/src/lib.rs")
    ),
    "\n",
    include_str!("commands.rs"),
    "\n",
    include_str!("panels_bars.rs"),
);
