//! THE SKETCH MIRROR ASKS WHAT TO REFLECT AND THEN ABOUT WHAT.
//!
//! Reported behaviour: "in the sketcher the Mirror tool does not work like the common UX. It should work
//! like the rest: press the button - if the person has geometry selected, that is what we want mirrored;
//! if not, we offer to pick it. The next step is picking what to mirror ABOUT - construction geometry or
//! ordinary geometry, a point or a line - and it must be possible to click the main X or Y axis with the
//! mouse, not only Y by default as it is now."
//!
//! Two things were wrong, and the second is the one that bites.
//!
//! IT APPLIED ON THE FIRST CLICK. With the tool armed and nothing selected, the very first entity clicked
//! was mirrored at once - so several things could not be mirrored together at all, and a misclick was an
//! edit rather than a wrong selection.
//!
//! AND THE AXIS WAS WHATEVER TURNED UP. If a line happened to be among the selected entities it became the
//! axis - and it was mirrored too, being in the same set. If none was, the tool silently used Y and said so
//! afterwards. The X axis could not be asked for at all.
//!
//! Now it is two steps, like every other tool: what, then about what. The axes are real targets for a
//! click - they are drawn, and until now they were the only drawn thing a person could not point at.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// A sketch with a rectangle away from the origin, so a mirror about an axis visibly moves it.
    fn a_sketch_with_a_rectangle() -> (App, usize) {
        let mut app = App::default();
        app.set.gpu_viewport = false;
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 10.0, 20.0, 40.0, 50.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.chosen.sel = super::super::Sel::Sketch(si);
        (app, si)
    }

    /// The mirror button, pressed through the same door the toolbar presses it with.
    fn press_mirror(app: &mut App) {
        let mut asks = Vec::new();
        let bc = app.bar_ctx(&mut asks);
        qymcad_ui_state::modify_button(qymcad_ui_state::editing_in!(bc), &mut qymcad_ui_state::tools_in!(bc), *bc.sk_pat, &*bc.tool_prefs, 1);
    }

    /// Pointing at the X (0) or Y (1) axis as the thing to mirror about.
    fn mirror_about_axis(app: &mut App, which: usize) {
        let mut asks = Vec::new();
        let bc = app.bar_ctx(&mut asks);
        qymcad_ui_state::mirror_about_axis(qymcad_ui_state::editing_in!(bc), &mut *bc.sel_sk, which);
    }

    fn select_all_entities(app: &mut App, si: usize) {
        let ids: Vec<u64> = app.project.sketches[si].entities.iter().map(|e| e.id).collect();
        assert!(ids.len() >= 4, "GUARD: the rectangle must give at least four entities to mirror");
        app.tools.sel_sk.items = ids.into_iter().map(|id| (1u8, id)).collect();
    }

    /// PRESSING THE BUTTON WITH A SELECTION TAKES IT AND ASKS FOR THE AXIS.
    ///
    /// Nothing is mirrored yet: the tool has been told WHAT, and is waiting to be told ABOUT WHAT.
    #[test]
    fn the_button_takes_the_selection_and_then_asks_for_the_axis() {
        let (mut app, si) = a_sketch_with_a_rectangle();
        select_all_entities(&mut app, si);
        let before = app.project.sketches[si].entities.len();

        press_mirror(&mut app);

        assert!(!app.tools.sel_sk.mirror_of.is_empty(), "the tool must hold what it was told to mirror and wait for the axis");
        assert_eq!(
            app.project.sketches[si].entities.len(),
            before,
            "nothing may be mirrored yet: the tool has been told what, and not yet about what"
        );
    }

    /// PRESSING IT WITH NOTHING SELECTED ASKS FOR THE GEOMETRY, and does not mirror anything.
    #[test]
    fn the_button_with_nothing_selected_asks_what_to_mirror() {
        let (mut app, si) = a_sketch_with_a_rectangle();
        let before = app.project.sketches[si].entities.len();

        press_mirror(&mut app);

        assert_eq!(app.tools.sel_sk.modify, Some(1), "the tool must be in hand, waiting for the geometry");
        assert!(app.tools.sel_sk.mirror_of.is_empty(), "and it has nothing to mirror yet");
        assert_eq!(app.project.sketches[si].entities.len(), before, "nothing was mirrored");
    }

    /// A CLICK ON AN ENTITY WHILE THE TOOL WAITS SELECTS IT AND MIRRORS NOTHING.
    ///
    /// This is the half that bit hardest: the first entity clicked used to be mirrored at once, so several
    /// things could not be mirrored together and a misclick was an edit.
    #[test]
    fn a_click_while_choosing_selects_instead_of_mirroring() {
        let (mut app, si) = a_sketch_with_a_rectangle();
        let before = app.project.sketches[si].entities.len();
        press_mirror(&mut app); // armed, nothing selected

        let first = app.project.sketches[si].entities[0].id;
        app.tools.sel_sk.items.push((1, first));
        press_mirror(&mut app); // pressing again is what advances the tool; a bare click must not

        assert_eq!(app.project.sketches[si].entities.len(), before, "one click must select, not mirror: otherwise two entities can never be mirrored together");
    }

    /// THE X AXIS IS A TARGET FOR A CLICK, and mirroring about it flips the geometry across y = 0.
    #[test]
    fn the_x_axis_can_be_pointed_at() {
        let (mut app, si) = a_sketch_with_a_rectangle();
        select_all_entities(&mut app, si);
        press_mirror(&mut app);

        let ys_before: Vec<i64> = app.project.sketches[si].points.iter().map(|p| (p.y * 1000.0) as i64).collect();
        mirror_about_axis(&mut app, 0); // 0 = X
        let ys_after: Vec<i64> = app.project.sketches[si].points.iter().map(|p| (p.y * 1000.0) as i64).collect();

        assert!(ys_after.iter().any(|y| *y < 0), "mirroring about X must put copies below the axis, and the sketch sat entirely above it: {ys_before:?} -> {ys_after:?}");
        assert!(app.tools.sel_sk.mirror_of.is_empty(), "the tool is done and lets go");
    }

    /// THE SELECTION STAYS VISIBLE WHEN THE TOOL TAKES IT.
    ///
    /// Reported behaviour: "press the Mirror tool and the geometry you had selected simply loses its
    /// selection. It has to stay highlighted so one can see it is still chosen."
    ///
    /// The first edition moved the selection into the tool and cleared it - so the person saw nothing
    /// selected, could not tell what the tool was holding, and the next click reflected "everything" as far
    /// as they could see. Complaints one and three were the same defect from two ends.
    #[test]
    fn taking_the_selection_leaves_it_selected_on_screen() {
        let (mut app, si) = a_sketch_with_a_rectangle();
        select_all_entities(&mut app, si);
        let chosen = app.tools.sel_sk.items.len();

        press_mirror(&mut app);

        assert_eq!(
            app.tools.sel_sk.items.len(),
            chosen,
            "the selection vanished from the screen when the tool took it: a person cannot tell what is about to be mirrored"
        );
        assert!(!app.tools.sel_sk.mirror_of.is_empty(), "and the tool holds it");
    }

    /// ESC PUTS THE MIRROR DOWN, back to plain selection.
    ///
    /// Reported behaviour: "Esc does not reset the Mirror tool to the default Select. The selection is
    /// lost, and the tool stays active with its bar at the top."
    ///
    /// The Esc ladder carried a rung for every armed sketch tool except this family - the drawing tools,
    /// the dimension, the ruler, the pattern, the move all had one, and the editing tools (mirror, offset,
    /// fillet, chamfer, array) had none. So the whole family was unreleasable, not the mirror alone.
    #[test]
    fn escape_puts_the_mirror_down() {
        let (mut app, si) = a_sketch_with_a_rectangle();
        select_all_entities(&mut app, si);
        press_mirror(&mut app);
        assert_ne!(app.tools.armed, qymcad_ui_state::Armed::None, "GUARD: the tool must be in hand before Esc has anything to put down");

        app.on_escape();

        assert_eq!(app.tools.armed, qymcad_ui_state::Armed::None, "Esc must return to plain selection, or the bar goes on saying Mirror over a tool that no longer acts");
        assert!(app.tools.sel_sk.mirror_of.is_empty(), "and the tool must not go on holding geometry it will never mirror");
    }

    /// AND THE Y AXIS, which was the silent default and is now a choice.
    #[test]
    fn the_y_axis_can_be_pointed_at() {
        let (mut app, si) = a_sketch_with_a_rectangle();
        select_all_entities(&mut app, si);
        press_mirror(&mut app);

        mirror_about_axis(&mut app, 1); // 1 = Y
        let xs: Vec<i64> = app.project.sketches[si].points.iter().map(|p| (p.x * 1000.0) as i64).collect();

        assert!(xs.iter().any(|x| *x < 0), "mirroring about Y must put copies left of the axis: {xs:?}");
    }

    /// THE AXIS IS REACHED BY A CLICK, not only by calling the door.
    ///
    /// The two checks above go through `mirror_about_axis`, which is the door but not the gesture. If the
    /// click in the sketch did not route to it, the whole tool would be unreachable by hand and both of
    /// them would still be green - the shape of a green check over a feature nobody can use.
    #[test]
    fn clicking_the_x_axis_in_the_sketch_mirrors_about_it() {
        let (mut app, si) = a_sketch_with_a_rectangle();
        select_all_entities(&mut app, si);
        press_mirror(&mut app);
        assert!(!app.tools.sel_sk.mirror_of.is_empty(), "GUARD: the tool must be waiting for the axis");

        // The click lands ON the X axis: the sheet puts the origin somewhere in the rectangle, and a point
        // at y = 0 in sketch coordinates is on the axis whatever the zoom.
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        app.viewing.view.initialized = true;
        let on_axis = qymcad_ui_state::Sheet { view: app.viewing.view, rect }.at(qymcad_core::geom::Point2::new(80.0, 0.0));

        qymcad_sketch::sketch_select_click(&mut app.sketch_ctx(), rect, on_axis, false);

        let ys: Vec<i64> = app.project.sketches[si].points.iter().map(|p| (p.y * 1000.0) as i64).collect();
        assert!(ys.iter().any(|y| *y < 0), "a click on the X axis must mirror across it: {ys:?}");
        assert!(app.tools.sel_sk.mirror_of.is_empty(), "and the tool lets go afterwards");
    }
}
