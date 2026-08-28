//! A NAMED CAUSE THAT CANNOT BE READ IS SILENCE ONE STEP FURTHER ALONG.
//!
//! The parameter window says why an expression did not evaluate - the reason is in words, not an icon. But it
//! was written into the narrowest column of a four-column table, and there it ran off the edge of the window:
//! the interface sweep at 700 points wide caught "the expression ends too early: a number or a na", cut
//! mid-word. Hovering showed the rest, and nobody hovers over something they have just been told is wrong.
//!
//! WHAT IS CHECKED IS GEOMETRY, NOT THE PRESENCE OF THE STRING. The first edition of this guard asked whether
//! the sentence appears among the texts of the frame - and it passed with the old drawing too, because the
//! text is laid out whole and only CLIPPED when painted. A guard that is green before the fix measures
//! nothing. So the question here is the one a person's eye asks: does the painted text fit inside the area it
//! is clipped to?
#[cfg(test)]
mod tests {
    use crate::gui::App;

    /// Every text shape of the frame, with the rectangle it is clipped to.
    fn painted(app: &mut App, draw: impl Fn(&mut App, &mut egui::Ui)) -> Vec<(String, egui::Rect, egui::Rect)> {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(700.0, 400.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let _ = ctx.run_ui(input.clone(), |c| draw(app, c)); // the first pass lays out, the second is what is seen
        let out = ctx.run_ui(input, |c| draw(app, c));
        let mut found = Vec::new();
        for cs in &out.shapes {
            collect(&cs.shape, cs.clip_rect, &mut found);
        }
        found
    }

    fn collect(shape: &egui::epaint::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect, egui::Rect)>) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push((t.galley.text().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size()), clip)),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| collect(s, clip, out)),
            _ => {}
        }
    }

    /// The whole reason is inside the popup at the geometry too.
    ///
    /// This one already behaved when the guard was written - the popup widens to fit the line and shows it
    /// under the fields. It is held here because the rule is the same rule, and the parameter window proved
    /// that one place can quietly stop obeying it while the other goes on being right.
    #[test]
    fn the_reason_is_painted_whole_in_the_popup_at_the_geometry_as_well() {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = crate::gui::Sel::Sketch(si);
        app.start_feat_cmd(1); // an extrude: its height is an expression field
        if let Some(f) = app.cmd.params.first_mut() {
            f.txt = "10 /".into();
        }
        // THE EXPECTED WORDS COME FROM THE CATALOGUE, not from a literal: a literal would be one language's
        // and would quietly stop matching in the other.
        let whole = crate::i18n::expr_error_text(&app.project.eval_expr("10 /").expect_err("the expression is broken"));
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let texts = painted(&mut app, |a, ctx| a.feat_cmd_popup(ctx, rect));
        // NOT "if it drew nothing, there is nothing to check": a guard that passes by drawing no reason at all
        // is the blindness this file was rewritten to escape. The field holds a broken expression, so the
        // reason MUST be on the screen.
        let (text, r, clip) = texts
            .into_iter()
            .find(|(t, _, _)| t.contains(&whole))
            .expect("the popup must show the reason while the field holds a broken expression");
        assert!(
            r.max.x <= clip.max.x + 0.5,
            "the reason in the popup is painted past the edge it is clipped to: text {text:?} ends at x={:.1}, the clip ends at x={:.1}",
            r.max.x,
            clip.max.x
        );
    }

    /// The whole reason is inside the window, not running off its edge.
    #[test]
    fn the_reason_an_expression_failed_is_painted_whole() {
        let mut app = App::default();
        app.project.parameters = vec![
            qymcad_core::model::Param { name: "w".into(), expr: "60".into(), value: 60.0 },
            qymcad_core::model::Param { name: "bad".into(), expr: "w/".into(), value: 0.0 },
        ];
        app.project.eval_parameters();
        app.win.params = true;
        let whole = crate::i18n::expr_error_text(&app.project.eval_expr("w/").expect_err("the expression is broken"));

        let texts = painted(&mut app, |a, ui| a.params_window(ui.ctx()));
        let (text, rect, clip) = texts
            .into_iter()
            .find(|(t, _, _)| t.contains(&whole))
            .unwrap_or_else(|| panic!("the reason {whole:?} never reached the screen at all"));
        assert!(
            rect.max.x <= clip.max.x + 0.5,
            "the reason is painted past the edge it is clipped to and comes out cut: text {text:?} ends at x={:.1}, the clip ends at x={:.1}",
            rect.max.x,
            clip.max.x
        );
    }
}
