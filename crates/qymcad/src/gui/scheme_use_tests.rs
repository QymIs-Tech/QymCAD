//! THE APPLICATION'S SIDE OF THE COLOUR SCHEMES.
//!
//! These two guards read the sources of PANELS, so they stayed here when the schemes moved into a crate
//! of their own. A guard that needs the crate it watches is not a guard but a part of the watched.
#[cfg(test)]
mod tests {
/// THE MEASURING LINE IS ONE ACROSS THE WHOLE CAD. It used to be amber in 3D and green in a sketch; it
/// was brought to green on request. A guard against the discord returning: one tool has one colour.
#[test]
fn the_measure_line_has_a_single_colour_everywhere() {
    let d = crate::palette::dark();
    assert_eq!(d.measure, [120, 220, 160], "the measuring line is green — that was the decision");
    assert_ne!(d.measure, d.cut_line, "the cutting line is another tool and has a colour of its own");
    let src = crate::gui::sketch_source::SKETCH;
    assert!(src.contains("self.scheme.pal.measure()"), "a sketch measures in the same colour as 3D");
    assert!(!src.contains("measure_sketch"), "there is no separate colour of the measuring line for a sketch any more");
}

/// A GUARD AGAINST A RETURN: no text typed straight into the code is left in the screen of the scheme.
#[test]
fn the_scheme_screen_takes_its_words_from_the_catalogue() {
    let panels = crate::gui::panels_source::PANELS;
    // THE ANCHORS ARE THE FREE FUNCTIONS NOW: the scheme editor came off `App`, and `scheme_section` with
    // `duplicate_scheme` became `section` and `duplicate`. What is guarded - no text typed straight into
    // the screen of the scheme - did not change.
    // THE NEEDLE CARRIES NO VISIBILITY. It said `pub(crate) fn section(`, and a blanket widening of
    // visibility during the move rewrote the needle along with the code - the guard then looked for a form
    // that existed nowhere. What is guarded is that the function is there, not how it is declared.
    let from = panels.find("fn section(").expect("the section of the scheme is in place");
    let to = panels.find("fn duplicate(").expect("the next function");
    let screen = &panels[from..to];
    let cyrillic: Vec<&str> = screen
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .filter(|l| l.contains('"') && l.chars().any(|c| ('а'..='я').contains(&c) || ('А'..='Я').contains(&c)))
        .collect();
    assert!(cyrillic.is_empty(), "text typed straight into the code has appeared in the screen of the scheme again:\n{}", cyrillic.join("\n"));
}
}
