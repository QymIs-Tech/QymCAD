//! A SELECTED BODY OF A NEIGHBOURING PART: what the two flags of the shading actually decide.
//!
//! `shade_tri` takes `hot` (the body is selected) and `ghost` (the body belongs to a neighbouring context),
//! and it is tempting to read them as one word of three, the way the shell's `outward` + `center` turned out
//! to be. They are not: `hot` decides the COLOUR and, with it, the opacity, while `ghost` decides WHICH PASS
//! the triangle is drawn in - the blended one, without a z-write.
//!
//! Measured here rather than argued, because the first reading of the same code was wrong: the `al` computed
//! beside the call looks like the ghost's transparency, but it only picks the bucket, and the alpha that
//! actually reaches the screen is the one `shade_tri` returned.
#[cfg(test)]
mod tests {
    use crate::gui::App;

    /// The palette and the light of a plain scene, so the numbers below are about the flags and nothing else.
    fn shade(hot: bool, ghost: bool) -> egui::Color32 {
        let app = App::default();
        App::shade_tri_for_test(&app.scheme.pal, app.set.ghost_alpha, hot, ghost, [120, 135, 162], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0])
    }

    #[test]
    fn a_ghost_is_translucent_until_it_is_selected_and_then_it_is_not() {
        let app = App::default();
        let plain = shade(false, false);
        let ghost = shade(false, true);
        let hot_ghost = shade(true, true);

        assert_eq!(plain.a(), 255, "a body of one's own context is opaque");
        assert_eq!(ghost.a(), app.set.ghost_alpha, "a neighbour's body shows what is behind it");
        assert!(app.set.ghost_alpha < 255, "otherwise the case being checked does not exist");

        // THE ANSWER TO THE QUESTION THIS FILE WAS WRITTEN FOR: selecting a ghost makes it OPAQUE. The
        // transparency is a property of the colour, and `hot` replaces the colour whole.
        assert_eq!(hot_ghost.a(), 255, "a SELECTED neighbour's body is drawn opaque, not see-through");
        assert_eq!(hot_ghost, shade(true, false), "and it is coloured exactly as any other selected body");
        assert_ne!(hot_ghost, ghost, "so the selection is visible on a ghost at all");
    }

    /// And the pass is still chosen by `ghost`, which is why the two flags are not one word of three.
    #[test]
    fn the_pass_of_a_selected_ghost_is_still_the_blended_one() {
        let src = crate::gui::render_source::RENDER;
        assert!(
            src.contains("let al = if ghost { self.set.ghost_alpha } else { 255 };"),
            "the bucket is chosen by `ghost` alone, with no regard to the selection"
        );
        assert!(src.contains("if ghost { ghost_tris.push(tri) } else { tris.push(tri) }"), "and the same rule holds on the CPU path");
    }
}
