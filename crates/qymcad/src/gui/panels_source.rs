//! THE PANELS AS ONE TEXT, for the checks that read them.
//!
//! Fifty-seven checks assert that a button, a tree row or a string exists by looking for it in the panels'
//! source. That is a strong guard - a tool nobody can reach does not exist for a person, and a text search is
//! the only thing that notices when the button is gone - and splitting the file into four must not weaken it.
//! So the four parts are handed over exactly as the one file used to be.

/// The whole of the panels, in the order the file used to have them.
#[cfg(test)]
pub(crate) const PANELS: &str = concat!(
    include_str!("panels_tree.rs"),
    "\n",
    include_str!("panels_props.rs"),
    "\n",
    include_str!("panels_bars.rs"),
    "\n",
    include_str!("panels_windows.rs"),
);
