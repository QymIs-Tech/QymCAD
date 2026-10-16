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

    /// The palette and the light of a plain scene - a face looking straight at the eye - so the numbers below are about
    /// the flags and nothing else.
    fn shade(hot: bool, ghost: bool) -> egui::Color32 {
        let app = App::default();
        let lights = qymcad_ui_state::scene_lights(&([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]));
        crate::gui::shade_tri_for_test(&app.scheme.pal, app.set.ghost_alpha, hot, ghost, [120, 135, 162], [0.0, 0.0, 1.0], &lights)
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
    ///
    /// On the CARD the same rule lives in two numbers rather than in a colour: the look of a body carries the
    /// two flags as separate BITS, the fragment paints by the selection bit and the pass is picked by the
    /// ghost bit. Written as one number of three - 0 ordinary, 1 selected, 2 a ghost - a selected ghost lost
    /// its ghostliness and jumped into the opaque pass.
    #[test]
    fn the_pass_of_a_selected_ghost_is_still_the_blended_one() {
        let src = crate::gui::render_source::RENDER;
        assert!(
            crate::gui::render_source::has(src, "if ghost { ghost_tris.push(tri) } else { tris.push(tri) }"),
            "on the CPU path the bucket is chosen by `ghost` alone, with no regard to the selection"
        );

        // THE CARD, checked by the look table rather than by the text of the source: a body of a neighbouring
        // part, selected, must carry BOTH bits.
        let mut app = App::default();
        let root = app.project.root;
        let first = crate::gui::scene_chunks::entering_a_context::part_inside(&mut app, root, 0.0);
        crate::gui::scene_chunks::entering_a_context::part_inside(&mut app, root, 40.0);
        app.win.context = true; // "in context": the neighbouring parts stay on screen as ghosts
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        app.enter_component(first);
        let ghostly = crate::gui::render_scene::scene_looks(&app.painting()).iter().position(|l| l.state & qymcad_ui_state::LOOK_GHOST != 0).expect("setup: the neighbouring part must be a ghost");
        app.chosen.sel = qymcad_ui_state::Sel::Mesh(ghostly);

        let look = crate::gui::render_scene::scene_looks(&app.painting())[ghostly];
        assert!(look.state & qymcad_ui_state::LOOK_HOT != 0, "the selection did not reach the look of the body");
        assert!(look.state & qymcad_ui_state::LOOK_GHOST != 0, "selecting a ghost took its ghostliness away, and with it the blended pass");
        assert!(crate::gui::render_source::has(crate::viewport_gpu::SHADER, "if ((look.state & 2u) != 0u) {"), "the shader must read the ghost as a bit of its own, not as a number of three");
    }
}
