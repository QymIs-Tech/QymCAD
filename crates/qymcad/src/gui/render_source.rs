//! THE VIEWPORT AS ONE TEXT, for the checks that read it.
//!
//! Nineteen checks assert that a tool draws its preview, or that a click in the viewport picks the right
//! thing, by looking for it in the source. Splitting the drawing from the mouse must not weaken them, so the
//! three parts are handed over exactly as the one file used to be.

/// The whole of the viewport, in the order the file used to have it.
#[cfg(test)]
pub(crate) const RENDER: &str = concat!(
    include_str!("render.rs"),
    "\n",
    include_str!("viewport_3d.rs"),
    "\n",
    include_str!("render_scene.rs"),
);
