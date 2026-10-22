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
    use crate::gui::WinKind;
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

    /// The whole reason is said where the tool speaks too, and painted inside the screen.
    ///
    /// The rule is the same rule as the parameter window's, and that window proved that one place can quietly stop
    /// obeying it while the other goes on being right.
    #[test]
    fn the_reason_is_said_whole_where_the_tool_speaks_as_well() {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.chosen.sel = qymcad_ui_state::Sel::Sketch(si);
        app.start_feat_cmd(qymcad_ui_state::PartTool::Extrude); // an extrude: its height is an expression field
        if let Some(f) = app.tools.cmd.params.first_mut() {
            f.txt = "10 /".into();
        }
        // THE EXPECTED WORDS COME FROM THE CATALOGUE, not from a literal: a literal would be one language's
        // and would quietly stop matching in the other.
        let whole = crate::gui::error_words::expr_error_text(&app.project.eval_expr("10 /").expect_err("the expression is broken"));
        let mut hand = super::super::hand::Hand::as_it_stands(&mut app);
        // NOT "if it said nothing, there is nothing to check": a guard that passes by saying no reason at all is the
        // blindness this file was rewritten to escape. The field holds a broken expression, so the reason MUST be said.
        let said = hand.tool_says();
        assert!(said.iter().any(|n| n.words.contains(&whole)), "the tool must say the reason while the field holds a broken expression: {said:?}");
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, hand.screen());
        let painted: Vec<egui::Rect> = hand.words_drawn().iter().filter(|(t, _)| t.contains(&whole)).map(|(_, r)| *r).collect();
        assert!(!painted.is_empty(), "the reason is said and not painted");
        assert!(painted.iter().all(|r| screen.contains_rect(*r)), "the reason is painted past the edge of the screen: {painted:?} on {screen:?}");
    }

    /// The whole reason is inside the window, not running off its edge.
    #[test]
    fn the_reason_an_expression_failed_is_painted_whole() {
        let mut app = App::default();
        app.project.parameters = vec![qymcad_core::model::Param { name: "w".into(), expr: "60".into(), value: 60.0 }, qymcad_core::model::Param { name: "bad".into(), expr: "w/".into(), value: 0.0 }];
        app.project.eval_parameters();
        app.win.open(WinKind::Params);
        let whole = crate::gui::error_words::expr_error_text(&app.project.eval_expr("w/").expect_err("the expression is broken"));

        let texts = painted(&mut app, |a, ui| {
            let mut asks = Vec::new();
            crate::gui::panels_windows::params_window(&mut a.win_ctx(&mut asks), ui.ctx());
            a.do_win_asks(asks, ui.ctx());
        });
        let (text, rect, clip) = texts.into_iter().find(|(t, _, _)| t.contains(&whole)).unwrap_or_else(|| panic!("the reason {whole:?} never reached the screen at all"));
        assert!(
            rect.max.x <= clip.max.x + 0.5,
            "the reason is painted past the edge it is clipped to and comes out cut: text {text:?} ends at x={:.1}, the clip ends at x={:.1}",
            rect.max.x,
            clip.max.x
        );
    }
}
