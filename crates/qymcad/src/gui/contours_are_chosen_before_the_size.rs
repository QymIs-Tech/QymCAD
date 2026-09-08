//! CREATING A FEATURE ASKS WHICH CONTOURS FIRST, AND ONLY THEN HOW FAR.
//!
//! Reported behaviour: "as it is now you pick the sketch, press extrude, and to choose the contours you
//! have to press the 'Pick contours' button or U/Alt+U. Maybe on CREATION - not on editing an existing
//! feature - we open the contour picker straight away, and on the way out of it the person sets the
//! extrusion distance and the rest."
//!
//! It used to take every closed contour of the sketch and go to the 3D preview at once. That is right when
//! there is nothing to choose and wrong the moment there are two: the person is shown a body built out of
//! everything, and has to work out that the way to change it is a button whose name they have not read yet.
//!
//! WITH ONE CONTOUR THE PICKER IS SKIPPED, and that is a decision rather than an omission: a picker over a
//! single candidate is a step that can end only one way, and a step like that is worse than none.
//!
//! EDITING IS NOT TOUCHED. An edit opens with the contours the feature was built from - that is the answer
//! to "which ones", already given - and re-picking them stays on the button and on U/Alt+U.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// A part with a sketch of `n` separate closed rectangles, selected and ready for a command.
    fn a_sketch_of(n: usize) -> App {
        let mut app = App::default();
        app.set.gpu_viewport = false;
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        for k in 0..n {
            let x = k as f64 * 100.0;
            app.project.add_rect_entity(si, x, 0.0, x + 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        }
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        let closed = qymcad_ui_state::sketch_closed_contours(&app.project, si);
        assert_eq!(closed.len(), n, "GUARD: the scene must hold {n} closed contours, and it holds {}", closed.len());
        app.chosen.sel = super::super::Sel::Sketch(si);
        app
    }

    /// TWO CONTOURS: the command opens ON THE CONTOURS, in the flat view.
    ///
    /// The flat view is the picker: `enter_contour_reselect` leaves 3D so that a click lands on a contour
    /// rather than on a body.
    #[test]
    fn creating_an_extrude_over_two_contours_asks_which_ones() {
        let mut app = a_sketch_of(2);
        app.start_feat_cmd(1);

        assert!(app.tools.armed.cmd_kind() == 1, "GUARD: the extrude command must be open");
        assert!(
            !app.viewing.mode_3d,
            "the command went straight to the 3D preview over both contours: the person is shown a body built out of everything and has to guess that a button changes it"
        );
    }

    /// ONE CONTOUR: nothing to choose, so no picker - straight to the size.
    #[test]
    fn creating_an_extrude_over_one_contour_goes_straight_to_the_size() {
        let mut app = a_sketch_of(1);
        app.start_feat_cmd(1);

        assert!(app.tools.armed.cmd_kind() == 1, "GUARD: the extrude command must be open");
        assert!(app.viewing.mode_3d, "with a single contour there is nothing to pick, and a step that can end only one way is worse than none");
    }

    /// AND A REVOLVE ASKS THE SAME WAY: the rule is about creating a feature from a sketch, not about
    /// the extrude button.
    #[test]
    fn creating_a_revolve_over_two_contours_asks_which_ones() {
        let mut app = a_sketch_of(2);
        app.start_feat_cmd(3);
        assert!(!app.viewing.mode_3d, "a revolve created over two contours must ask which ones, exactly as an extrude does");
    }

    /// LEAVING THE PICKER LEADS TO THE SIZE, and only the NEXT Enter applies the feature.
    ///
    /// This is the two-step the report asks for, written down as a check: the first Enter answers "these
    /// contours", the second answers "this far". Without it the change above would be half a feature - a
    /// picker one enters and cannot leave.
    #[test]
    fn leaving_the_picker_leads_to_the_size_and_then_applies() {
        let mut app = a_sketch_of(2);
        app.start_feat_cmd(1);
        assert!(!app.viewing.mode_3d, "GUARD: the command must start in the picker");

        app.apply_feat_cmd(); // the first Enter: the contours are chosen
        assert!(app.viewing.mode_3d, "leaving the picker must bring the person to the size, in the 3D preview");
        assert!(app.tools.armed.cmd_kind() == 1, "and the command is still open - the first Enter chooses, it does not apply");

        app.apply_feat_cmd(); // the second Enter: the feature is made
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        assert!(
            app.project.timeline.iter().any(|n| n.kind.body().is_some()),
            "the second Enter must make the feature: status says \"{}\"",
            app.status
        );
    }

    /// EDITING AN EXISTING FEATURE DOES NOT ASK AGAIN.
    ///
    /// Without this the change above would be an improvement in one place and a nuisance in the other:
    /// re-opening a feature to change its height would throw the person into a contour picker every time.
    #[test]
    fn editing_a_feature_opens_on_its_size_not_on_the_contours() {
        let mut app = a_sketch_of(2);
        app.start_feat_cmd(1);
        app.apply_feat_cmd(); // out of the picker
        app.apply_feat_cmd(); // and the feature is made
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let fid = app.project.timeline.iter().rev().find(|n| n.kind.body().is_some()).map(|n| n.id).expect("the extrude is in the timeline");

        crate::gui::commands::start_feat_cmd_edit(&mut app.part_ctx(), fid);

        assert!(app.tools.cmd.edit.is_some(), "GUARD: this must be an edit, not a new command");
        assert!(app.viewing.mode_3d, "an edit opens on the size: which contours was answered when the feature was made, and re-picking them is on the button");
    }
}
