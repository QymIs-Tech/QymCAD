//! THE FIELDS IN THE PARAMETERS WINDOW STRETCH ALONG WITH THE WINDOW.
//!
//! Reported behaviour: the width of the variable names should be elastic, otherwise typing is awkward
//! — everything is squeezed and a long name cannot be entered.
//!
//! And so it was: 90 points were fixed for the name and 120 for the formula. The window stretched as
//! far as one liked while the fields stayed as they were — a long name crawled off the edge, and one
//! had to type blind.
#[cfg(test)]
mod tests {
    use super::super::{param_field_widths, App};

    /// STRETCH THE WINDOW AND THE FIELDS STRETCH. The main requirement: the width depends on the
    /// window rather than being nailed down.
    #[test]
    fn wider_window_gives_wider_fields() {
        let (narrow_name, narrow_expr) = param_field_widths(360.0);
        let (wide_name, wide_expr) = param_field_widths(900.0);
        assert!(
            wide_name > narrow_name + 100.0,
            "the name did not stretch with the window: {narrow_name} -> {wide_name}"
        );
        assert!(
            wide_expr > narrow_expr + 100.0,
            "the formula did not stretch with the window: {narrow_expr} -> {wide_expr}"
        );
    }

    /// A NARROW WINDOW DOES NOT COLLAPSE THE FIELDS TO NOTHING. The old fixed numbers became the lower
    /// bound: elasticity must not turn into a field two characters wide when the window is squeezed.
    #[test]
    fn a_narrow_window_keeps_the_old_minimums() {
        for avail in [0.0, 40.0, 120.0, 300.0] {
            let (name, expr) = param_field_widths(avail);
            assert!(name >= 90.0, "the name shrank below the old 90 points at a width of {avail}: {name}");
            assert!(expr >= 120.0, "the formula shrank below the old 120 points at a width of {avail}: {expr}");
        }
    }

    /// THE FORMULA IS THE WIDER ONE. Expressions like `w*2+3` live in it, and it must get more
    /// room.
    #[test]
    fn the_expression_field_is_the_wider_one() {
        for avail in [360.0, 600.0, 1200.0] {
            let (name, expr) = param_field_widths(avail);
            assert!(expr > name, "at a width of {avail} the formula is not wider than the name: {name} against {expr}");
        }
    }

    /// AND THE TWO FIELDS TOGETHER LEAVE ROOM FOR THE THIRD COLUMN AND THE BUTTON. Otherwise the value
    /// and the bin slide off the edge, and one crampedness is exchanged for another.
    #[test]
    fn the_value_column_and_the_button_still_fit() {
        for avail in [600.0, 900.0, 1400.0] {
            let (name, expr) = param_field_widths(avail);
            assert!(
                name + expr <= avail * 0.75,
                "the fields ate {} of {avail} — nothing is left for the value and the button",
                name + expr
            );
        }
    }

    /// THE MOST IMPORTANT CHECK IS IN A REAL FRAME, NOT BY A NUMBER.
    ///
    /// The first attempt was checked by two tests: "the function computes a wider value" and "the call
    /// is in the source". Both green — and the fields in the window stayed narrow, and a screenshot
    /// came back. That is exactly an empty check: it confirmed the intention rather than the result.
    ///
    /// Here the REAL rows of the table are drawn in a ui of known width, and how much the drawn frame
    /// grew is measured. If the fields do not stretch, the width of the frame does not change.
    fn rows_out(avail: f32, count: usize) -> super::super::ParamRowsOut {
        rows_out_with(avail, count, 0)
    }

    /// The same, but some of the rows are DRIVERS (named sketch dimensions). The question was about
    /// 150 variables AND DRIVERS, and they now live in one table: they have to be counted together.
    fn rows_out_with(avail: f32, count: usize, drivers: usize) -> super::super::ParamRowsOut {
        use qymcad_core::geom::Point2;
        use qymcad_core::model::{Constraint, Id};
        let mut app = App::default();
        app.project.new_document();
        for i in 0..count {
            app.project.parameters.push(qymcad_core::model::Param {
                name: format!("H_Okno_Verh_{i}"),
                expr: "22".into(),
                value: 22.0,
            });
        }
        for i in 0..drivers {
            let sid = app.project.add_line_sketch(
                &format!("Profile {i}"),
                vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 20.0), Point2::new(0.0, 20.0)],
                true,
            );
            let si = app.project.sketch_index(sid).unwrap();
            let pts: Vec<Id> = app.project.sketches[si].points.iter().take(2).map(|q| q.id).collect();
            app.project.sketches[si].constraints.push(Constraint::Distance {
                a: pts[0],
                b: pts[1],
                d: 40.0,
                off: 0.0,
                expr: String::new(),
                driven: false,
                axis: 0,
            });
            assert!(app.project.add_named_dim(format!("dlina_proema_{i}"), sid, pts));
        }
        let ctx = egui::Context::default();
        let mut out = super::super::ParamRowsOut::default();
        // SEVERAL FRAMES RATHER THAN ONE. `egui::Grid` lays out its columns by the MEMORY of the
        // previous frame: on the first drawing there is no width yet, and any field comes out 32 points
        // wide. A measurement over one frame would report "it did not stretch" even for a sound window
        // — that is the measuring, not a defect.
        for _ in 0..3 {
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    // THE WIDTH IS SET BY ALLOCATING ROOM RATHER THAN BY `set_max_width`: the latter
                    // did not work in this context — the ui stayed 9984 wide, and the narrow and wide
                    // cases were identical. A measurement that does not tell cases apart proves
                    // nothing.
                    ui.allocate_ui(egui::vec2(avail, 800.0), |ui| {
                        out = app.params_rows_ui(ui);
                    });
                });
            });
        }
        out
    }

    /// STRETCH THE WINDOW AND THE FIELD IN THE FRAME GETS WIDER. The rectangle OF THE FIELD ITSELF is
    /// measured rather than the number the function computed: the first attempt was green by the
    /// function and narrow on the screen.
    #[test]
    fn the_drawn_field_really_stretches_with_the_window() {
        let narrow = rows_out(360.0, 3).name_w;
        let wide = rows_out(1100.0, 3).name_w;
        assert!(narrow > 0.0 && wide > 0.0, "the field was not drawn: {narrow} / {wide}");
        assert!(
            wide > narrow + 150.0,
            "the name field did not stretch with the window: {narrow:.0} -> {wide:.0} points IN THE FRAME"
        );
    }

    /// A HUNDRED AND FIFTY ROWS OF VARIABLES **AND DRIVERS** SCROLL TOO.
    ///
    /// The question was exactly about that: what if there are some 150 variables and drivers. There was
    /// no scrolling: the table grew as it was, and the "add a parameter" button slid away with it. Now
    /// the list scrolls and its height is bounded.
    ///
    /// The drivers used to hang in a list of their own, and the scrolling check counted only the
    /// variables; now they are in one table, and the old proof no longer said anything about it.
    #[test]
    fn a_long_mixed_list_scrolls_too() {
        let few = rows_out_with(600.0, 3, 2);
        let many = rows_out_with(600.0, 75, 75);
        assert!(many.content_h > few.content_h * 5.0, "the contents did not grow with the list: {:.0} -> {:.0}", few.content_h, many.content_h);
        assert!(
            many.height <= super::super::PARAM_ROWS_MAX_H + 8.0,
            "150 rows (variables and drivers) stretched the window to {:.0} points — there is no scrolling",
            many.height
        );
        assert!(many.content_h > many.height + 100.0, "the contents ({:.0}) did not exceed the box ({:.0}) — there is nothing to scroll", many.content_h, many.height);
        // AND THE FIELDS DID NOT COLLAPSE: a long name can still be typed.
        assert!(many.name_w > 120.0, "the name field collapsed to {:.0} points — a long name cannot be typed", many.name_w);
    }

    #[test]
    fn a_long_list_scrolls_instead_of_growing() {
        let few = rows_out(600.0, 3);
        let many = rows_out(600.0, 150);
        // THE CONTENTS grow, so the list really is long and the measurement tells the cases apart.
        assert!(
            many.content_h > few.content_h * 10.0,
            "the contents did not grow with the list: {:.0} -> {:.0}",
            few.content_h,
            many.content_h
        );
        // AND THE BOX does not grow, so it is scrolled rather than shown whole.
        assert!(
            many.height <= super::super::PARAM_ROWS_MAX_H + 8.0,
            "150 parameters stretched the window to {:.0} points — there is no scrolling and the add button will slide off the screen",
            many.height
        );
        assert!(
            many.content_h > many.height + 100.0,
            "the contents ({:.0}) did not exceed the box ({:.0}) — there is nothing to scroll and the check is empty",
            many.content_h,
            many.height
        );
    }

    /// AND THE WINDOW REALLY DRAWS THESE ROWS rather than a copy of its own beside them.
    #[test]
    fn the_window_draws_the_same_rows() {
        let src = crate::gui::panels_source::PANELS;
        let a = src.find("pub(super) fn params_window").expect("the parameters window is there");
        let b = src[a..].find("\n    pub(super) fn ").map(|i| a + i).unwrap_or(src.len());
        let body = &src[a..b];
        assert!(body.contains("params_rows_ui"), "the parameters window draws a different method from the one the test checks");
        assert!(!body.contains("desired_width(90.0)"), "a fixed width for the name is left in the parameters window");
        assert!(!body.contains("desired_width(120.0)"), "a fixed width for the formula is left in the parameters window");
    }

}
