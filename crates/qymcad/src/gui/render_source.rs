//! THE VIEWPORT AS ONE TEXT, for the checks that read it.
//!
//! Nineteen checks assert that a tool draws its preview, or that a click in the viewport picks the right
//! thing, by looking for it in the source. Splitting the drawing from the mouse must not weaken them, so
//! the parts are handed over exactly as the one file used to be.
//!
//! THE DRAWING MOVED TO A CRATE OF ITS OWN (`qymcad-render`), and the list here moved with it. A guard that
//! reads a file by name guards that file and no other: leaving the list as it was would have left nineteen
//! checks reading two hundred lines of what used to be four thousand - green, and blind.

/// The whole of the viewport, in the order the file used to have it.
#[cfg(test)]
pub(crate) const RENDER: &str = concat!(
    concat!(
        include_str!("../../../qymcad-render/src/cards.rs"),
        "\n",
        include_str!("../../../qymcad-render/src/dim_text.rs"),
        "\n",
        include_str!("../../../qymcad-render/src/gizmos.rs"),
        "\n",
        include_str!("../../../qymcad-render/src/glyphs.rs"),
        "\n",
        include_str!("../../../qymcad-render/src/part_previews.rs"),
        "\n",
        include_str!("../../../qymcad-render/src/pickers.rs"),
        "\n",
        include_str!("../../../qymcad-render/src/raster.rs"),
        "\n",
        include_str!("../../../qymcad-render/src/sketch_marks.rs"),
        "\n",
        include_str!("../../../qymcad-render/src/sketch_overlay.rs"),
        "\n",
        include_str!("../../../qymcad-render/src/sketch_previews.rs"),
        "\n",
        include_str!("../../../qymcad-render/src/lib.rs")
    ),
    "\n",
    include_str!("render.rs"),
    "\n",
    include_str!("rebuild_sign.rs"),
    "\n",
    include_str!("viewport_3d.rs"),
    "\n",
    include_str!("viewcube.rs"),
    "\n",
    include_str!("render_scene.rs"),
);

/// SOURCE TEXT WITH EVERY SPACE TAKEN OUT, for a check that looks for a piece of code inside it.
///
/// A guard that searches the source for a literal breaks the moment anything rewraps that line or drops a
/// borrow from it - and then it reports the shape of the line, not what the code does. Three of them broke
/// this way in one day: two when a formatter was tried over the tree, a third when `clippy --fix` turned
/// `draw_projection_overlay(&painter, ..)` into `draw_projection_overlay(painter, ..)`. Comparing without
/// spaces keeps what the guard is about and drops what it is not.
#[cfg(test)]
pub(crate) fn dense(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// DOES THE SOURCE CONTAIN THIS PIECE OF CODE, spacing aside.
///
/// The form to reach for. `crate::gui::render_source::has(src, "draw_split_preview(pn, painter, rect);")` says it is about the
/// call; what it actually checks is the call AND its spacing, so dropping one borrow or rewrapping one line
/// turns it red while the code goes on doing exactly what the guard is about.
///
/// NOT FOR TEXT A PERSON READS. Spacing is part of an interface string and part of a help article; a check
/// over those must keep it. This is for source only - code, and the shader, which is code too.
#[cfg(test)]
pub(crate) fn has(src: &str, what: &str) -> bool {
    // THE TRAILING COMMA GOES TOO, and that is not tidiness. Breaking a call across lines gives it one:
    // `draw_split_preview(pn, painter, rect);` becomes `draw_split_preview(\n pn,\n painter,\n rect,\n);`
    // and without spaces that is `...rect,);` against `...rect);`. Measured with this very check - it went
    // red on a rewrap that changed nothing. A guard that a rewrap can break breaks on the day the tree is
    // formatted, all of them at once.
    let flat = |s: &str| dense(s).replace(",)", ")").replace(",]", "]").replace(",}", "}");
    flat(src).contains(&flat(what))
}

/// THE TOOLS OF A PART THE DRAWING ANSWERS TO: every tool a line of `src` that looks at the tool in hand names, by
/// its name, by a question the tool answers yes to, or by how the tool shows what it took.
#[cfg(test)]
pub(crate) fn tools_the_drawing_names(src: &str) -> Vec<qymcad_ui_state::PartTool> {
    use qymcad_ui_state::PartTool;
    /// A QUESTION THE DRAWING MAY ASK OF THE TOOL IN HAND, by its name in the source.
    struct Question {
        name: &'static str,
        holds: fn(PartTool) -> bool,
    }
    const QUESTIONS: [Question; 17] = [
        Question { name: "is_a_primitive", holds: PartTool::is_a_primitive },
        Question { name: "is_a_datum", holds: PartTool::is_a_datum },
        Question { name: "picks_profiles", holds: PartTool::picks_profiles },
        Question { name: "blends_edges", holds: PartTool::blends_edges },
        Question { name: "picks_edges", holds: PartTool::picks_edges },
        Question { name: "reads_the_live_brep", holds: PartTool::reads_the_live_brep },
        Question { name: "is_a_pattern", holds: PartTool::is_a_pattern },
        Question { name: "sizes_what_is_picked", holds: PartTool::sizes_what_is_picked },
        Question { name: "splits", holds: PartTool::splits },
        Question { name: "says_what_it_wants", holds: PartTool::says_what_it_wants },
        Question { name: "wants_a_solid", holds: PartTool::wants_a_solid },
        Question { name: "picks_faces", holds: PartTool::picks_faces },
        Question { name: "gathers_faces_of_one_body", holds: PartTool::gathers_faces_of_one_body },
        Question { name: "toggles_faces", holds: PartTool::toggles_faces },
        Question { name: "takes_one_face", holds: PartTool::takes_one_face },
        Question { name: "picks_bodies", holds: PartTool::picks_bodies },
        Question { name: "keeps_the_faces_it_takes", holds: PartTool::keeps_the_faces_it_takes },
    ];
    // whole names: `PartTool::Shell` is not found inside a longer name
    let names = |line: &str, word: &str| {
        let name = format!("PartTool::{word}");
        line.match_indices(&name).any(|(i, _)| !line[i + name.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_'))
    };
    let mut named = Vec::new();
    for line in src.lines().filter(|l| l.contains("armed.part_tool()")) {
        for t in PartTool::ALL {
            if names(line, &format!("{t:?}")) || QUESTIONS.iter().any(|q| names(line, q.name) && (q.holds)(t)) {
                named.push(t);
            }
        }
    }
    // and by how the tool shows what it took: a branch on `Shows::FaceSet` answers every tool that shows so
    let shown = |line: &str, t: PartTool| {
        let name = format!("Shows::{:?}", t.shows());
        t.shows() != qymcad_ui_state::Shows::Nothing && line.match_indices(&name).any(|(i, _)| !line[i + name.len()..].starts_with(|c: char| c.is_alphanumeric()))
    };
    for line in src.lines() {
        named.extend(PartTool::ALL.into_iter().filter(|t| shown(line, *t)));
    }
    named
}
