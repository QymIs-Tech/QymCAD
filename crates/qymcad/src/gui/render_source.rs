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
    include_str!("../../../qymcad-render/src/lib.rs"),
    "\n",
    include_str!("render.rs"),
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
    // red on a rewrap that changed nothing. A formatter over the whole tree is a step of the plan, so a
    // guard that a rewrap can break is a guard that will break on that day, all of them at once.
    let flat = |s: &str| dense(s).replace(",)", ")").replace(",]", "]").replace(",}", "}");
    flat(src).contains(&flat(what))
}
