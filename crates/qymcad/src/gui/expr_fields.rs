//! A FORMULA IS ACCEPTED EVERYWHERE A NUMBER IS ENTERED.
//!
//! Reported behaviour: parameters cannot be used in the top bar, nor in many other places — the
//! sketcher too has tools where no parameter or formula can be entered. And so it was: the
//! application had exactly one field that accepted TEXT — the parameter popup at the geometry.
//! Everything else was a `DragValue`, where a formula cannot be entered at all: the copy count of a
//! pattern, the number of sides of a polygon, the radius of a sketch fillet, the offset distance, the
//! parameters of sketch patterns, the height of text.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// No field is left in the tool bars where an expression cannot be entered.
    #[test]
    fn the_tool_bars_have_no_number_only_fields() {
        let src = crate::gui::panels_source::PANELS;
        for fname in ["fn tool_options_bar", "fn feat_command_bar"] {
            let a = src.find(fname).expect("the bar is there");
            let b = src[a..].find("\n    pub(super) fn ").map(|i| a + i).unwrap_or(src.len());
            let n = src[a..b].matches("DragValue").count();
            assert_eq!(
                n, 0,
                "{n} DragValue fields are left in `{fname}` — neither a formula nor a global variable can be \
                 entered there, and these are the working dimensions of a part"
            );
        }
    }

    /// The field really does evaluate an expression over a global variable and rounds integers.
    #[test]
    fn a_bar_field_evaluates_a_global_variable() {
        let mut app = App::default();
        app.project.parameters.push(qymcad_core::model::Param { name: "n".into(), expr: "3".into(), value: 3.0 });
        app.project.parameters.push(qymcad_core::model::Param { name: "w".into(), expr: "8".into(), value: 8.0 });
        let ctx = egui::Context::default();

        let mut got_int = 0.0;
        let mut got_real = 0.0;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                app.bar_exprs.insert("t_count", "n*2".into());
                got_int = app.num_or_expr(ui, "t_count", 1.0, 1.0, 512.0, true, "");
                app.bar_exprs.insert("t_rad", "w/2 + 0.5".into());
                got_real = app.num_or_expr(ui, "t_rad", 1.0, 0.01, 10000.0, false, " mm");
            });
        });
        assert_eq!(got_int, 6.0, "an integer field: \"n*2\" with n=3 must give 6 (evaluate and round)");
        assert_eq!(got_real, 4.5, "a real field: \"w/2 + 0.5\" with w=8 must give 4.5");
    }

    /// A broken expression does NOT change the value — no rubbish travels into the model.
    #[test]
    fn a_broken_expression_keeps_the_previous_value() {
        let mut app = App::default();
        let ctx = egui::Context::default();
        let mut got = 0.0;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                app.bar_exprs.insert("t_bad", "1/0".into());
                got = app.num_or_expr(ui, "t_bad", 7.0, 0.0, 100.0, false, "");
            });
        });
        assert_eq!(got, 7.0, "a broken expression must leave the previous value instead of substituting rubbish");
    }

    /// The dimension fields AFTER DRAWING accept an expression, not a bare number alone.
    ///
    /// Entering the width and height of a rectangle and an ellipse, the radius and angle of a polygon,
    /// the corner fillet and the rotation all existed, but was parsed through `parse::<f64>()` —
    /// neither a formula nor a global variable could be entered. These are the working dimensions of a
    /// part: "width = housing - 2*wall" is an everyday thing.
    #[test]
    fn sketch_size_fields_accept_expressions() {
        let mut app = App::default();
        app.project.parameters.push(qymcad_core::model::Param { name: "korpus".into(), expr: "60".into(), value: 60.0 });
        app.project.parameters.push(qymcad_core::model::Param { name: "stenka".into(), expr: "2.5".into(), value: 2.5 });

        assert_eq!(app.parse_num("40"), Some(40.0), "a plain number must keep working as before");
        assert_eq!(app.parse_num(" 40,5 "), Some(40.5), "a comma as the decimal separator");
        assert_eq!(app.parse_num("korpus - 2*stenka"), Some(55.0), "an expression over global variables");
        assert_eq!(app.parse_num(""), None, "empty: do not change the value");
        assert_eq!(app.parse_num("1/0"), None, "a broken expression: do not change the value rather than substitute infinity");
        assert_eq!(app.parse_num("no_such_name"), None, "an unknown name: do not change the value");
    }

    /// Exactly one bare-number parse is left in the sketcher — and it is legitimate.
    ///
    /// It is the value field of a DIMENSION: there `parse::<f64>()` asks "is this a number or an
    /// expression" in order to decide whether to store the formula in the constraint. Every other
    /// field must go through `parse_num`. Comments do not count — the code is what is counted.
    #[test]
    fn the_sketcher_parses_values_through_one_door() {
        let src = include_str!("sketching.rs");
        let n = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .filter(|l| l.contains("parse::<f64>()"))
            .count();
        assert_eq!(
            n, 1,
            "parsing a value must go through `parse_num` everywhere except the dimension field (where it is \
             decided whether it is a number or a formula). Places with a bare parse: {n}"
        );
    }
}
