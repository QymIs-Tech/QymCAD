//! AN ERROR IN A FORMULA SAYS WHAT IS WRONG.
//!
//! The expression parser ALWAYS knew the reason: `ExprError` lists nine kinds, and all nine are
//! translated into both languages. The reason was lost at the last step — in the interface, and
//! differently in each of four places:
//!
//! * the parameters window printed `(!)` — a red bracket with no words;
//! * the sketch dimension popup showed `Display`, that is, ENGLISH text in a non-English interface;
//! * the gizmo popup and the command popup said "the expression was not evaluated" — a phrase
//!   identical for a typo, an unknown name and a division by zero, that is, saying nothing at all;
//! * the command popup on top of that did not name the FIELD, though a command can have four of them.
//!
//! Now there is one door — `i18n::expr_error_text` — and the guards hold three things: the reason
//! reaches the screen, it is in the language of the interface, and the catch-all excuse has not come
//! back.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::errors::ExprError;

    /// EVERY KIND OF ERROR IS TRANSLATED AND SHOWS NO KEY.
    ///
    /// The kinds are listed by name: add a tenth one and the test will not compile until it is written
    /// in. That is cheaper than a guard over the source and more honest: `ExprError` is small and
    /// changes rarely.
    #[test]
    fn every_kind_of_expression_error_speaks_words() {
        let prev = crate::i18n::language();
        for code in ["ru", "en"] {
            crate::i18n::set_language(code);
            for e in [
                ExprError::UnknownChar("§".into()),
                ExprError::UnknownFn("foo".into()),
                ExprError::NeedsOneArg("sin".into()),
                ExprError::NeedsTwoArgs("pow".into()),
                ExprError::UnexpectedToken("/".into()),
                ExprError::TrailingInput(")".into()),
                ExprError::ExpectedParen,
                ExprError::ExpectedParenAfterArgs,
                ExprError::NotANumber,
                ExprError::UnexpectedEnd,
                ExprError::UnknownName("w".into()),
            ] {
                let msg = crate::i18n::expr_error_text(&e);
                assert!(!msg.trim().is_empty(), "{code}: {e:?} — an empty message");
                assert!(!msg.contains("err-"), "{code}: {e:?} showed a catalogue key: \"{msg}\"");
                assert!(msg.chars().count() > 5, "{code}: {e:?} — the message \"{msg}\" explains nothing");
            }
        }
        crate::i18n::set_language(&prev);
    }

    /// AND THE LANGUAGE IS REAL: the message in one language differs from the message in the other.
    ///
    /// Without this half the guard is green even for `Display`, which is English always — and that is
    /// exactly how the dimension popup behaved.
    #[test]
    fn the_message_is_in_the_interface_language() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        let ru = crate::i18n::expr_error_text(&ExprError::UnknownFn("foo".into()));
        crate::i18n::set_language("en");
        let en = crate::i18n::expr_error_text(&ExprError::UnknownFn("foo".into()));
        crate::i18n::set_language(&prev);
        assert_ne!(ru, en, "the two messages came out the same, so nobody did the translating");
        assert!(ru.contains("foo") && en.contains("foo"), "the name FROM THE INPUT must stay in the message: \"{ru}\" / \"{en}\"");
    }

    /// THE REASON REACHES THE SCREEN in the parameters window.
    #[test]
    fn the_params_window_shows_the_reason() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        let mut app = App::default();
        app.project.parameters = vec![qymcad_core::model::Param { name: "w".into(), expr: "60 +".into(), value: 0.0 }];
        app.win.params = true;
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.params_window(c));
        let want = crate::i18n::expr_error_text(&app.project.eval_expr("60 +").expect_err("the expression is broken"));
        crate::i18n::set_language(&prev);
        assert!(texts.iter().any(|t| t.contains(&want)), "the parameters window carries no reason \"{want}\": {texts:?}");
        assert!(!texts.iter().any(|t| t.trim() == "(!)"), "the wordless red bracket is back");
    }

    /// AND FOR A FEATURE DIMENSION TOO — THAT WAS THE LAST SILENT DOOR.
    ///
    /// The feature dimension field (the same field as the value of a relation) knew how to REFUSE: an
    /// unusable expression does not reach the model, the letters are intact, the caret is in place. But
    /// there was nobody to say what was wrong — Enter is pressed, nothing happens, no explanation.
    ///
    /// The teller `expr_value_label` lay right next to it WRITTEN AND NEVER ONCE CALLED. A compiler
    /// warning about a method never being used stood over it — and drowned among 175 others.
    #[test]
    fn a_feature_dimension_says_what_is_wrong_with_the_formula() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        let mut app = super::super::screen_keys::tests::plate();
        let node = app.project.timeline.iter().rev().find(|n| n.kind.body().is_some()).map(|n| n.id).expect("the feature node");
        app.project.set_feat_dim(node, "height", "60 +".into());
        let want = crate::i18n::expr_error_text(&app.project.eval_expr("60 +").expect_err("the expression is broken"));
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| {
            egui::CentralPanel::default().show(c, |ui| {
                a.dim_expr_field_for_test(ui, node, "height");
            });
        });
        crate::i18n::set_language(&prev);
        assert!(texts.iter().any(|t| t.contains(&want)), "the feature dimension field says nothing about the error \"{want}\": {texts:?}");
    }

    /// AND IN THE COMMAND POPUP — WITH THE NAME OF THE FIELD.
    ///
    /// A command can have four fields; "the expression was not evaluated" without the name of the field
    /// forces one to check them one by one.
    #[test]
    fn the_command_popup_names_the_field_and_the_reason() {
        let mut app = super::super::screen_keys::tests::plate();
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        app.sel = super::super::Sel::Mesh(app.project.mesh_index(body).expect("the mesh"));
        app.start_feat_cmd(7); // a hole: diameter + depth
        let label = {
            let p = app.cmd.params.first_mut().expect("a field of the command");
            p.txt = "10 /".into();
            p.label()
        };
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.viewport(c));
        let shown = texts.iter().any(|t| t.contains(&label) && t.contains(':'));
        assert!(shown, "the command popup did not name the field \"{label}\" with a reason: {texts:?}");
    }

    /// THE END OF THE INPUT IS EXPLAINED IN WORDS RATHER THAN BY A DEBUG NAME.
    ///
    /// `w/` and `60 +` are the commonest typo: the tail was erased and never finished. The parser
    /// answered `UnexpectedToken("None")`, and the parameters window read "Unexpected token None" — the
    /// innards of Rust instead of an explanation. Caught on a screenshot AFTER the reason was brought
    /// out to the screen: while a red bracket stood in its place, the defect in the message itself
    /// could not be seen.
    #[test]
    fn an_unfinished_expression_says_so() {
        let app = App::default();
        for src in ["60 +", "2 *", "3 -"] {
            let e = app.project.eval_expr(src).expect_err("the expression breaks off");
            assert_eq!(e, ExprError::UnexpectedEnd, "\"{src}\" gave {e:?} instead of an unexpected end");
            let msg = crate::i18n::expr_error_text(&e);
            assert!(!msg.contains("None") && !msg.contains("Some"), "the innards of Rust are in the message: \"{msg}\"");
        }
    }

    /// A BARE NAME IS NOT A FUNCTION.
    ///
    /// `w/2` with no parameter `w` answered "unknown function: w". Advice wide of the mark: a parameter
    /// was meant, and functions were being talked about — that is, the search went the wrong way.
    #[test]
    fn a_bare_name_is_not_reported_as_a_function() {
        let app = App::default();
        let e = app.project.eval_expr("w/2").expect_err("there is no such parameter");
        assert_eq!(e, ExprError::UnknownName("w".into()), "a name without brackets was called a function: {e:?}");
        // and with brackets it is a function indeed
        let e = app.project.eval_expr("wat(2)").expect_err("there is no such function");
        assert_eq!(e, ExprError::UnknownFn("wat".into()), "a call with brackets must be a function: {e:?}");
    }

    /// AND THE TOKEN IN THE MESSAGE IS THE ONE THAT WAS TYPED.
    #[test]
    fn the_offending_token_is_shown_as_typed() {
        let app = App::default();
        let e = app.project.eval_expr("60 )").expect_err("a stray bracket");
        let msg = crate::i18n::expr_error_text(&e);
        assert!(msg.contains(')'), "the message does not show the character itself: \"{msg}\"");
        assert!(!msg.contains("RParen"), "the message shows the name of the variant: \"{msg}\"");
    }

    /// THE CATCH-ALL EXCUSE HAS NOT COME BACK.
    ///
    /// "The expression was not evaluated" is identical for a typo, an unknown name and a division by
    /// zero — such a phrase looks like a message and carries no information. The keys are left in the
    /// catalogue (they may be called from places where there is no error at all), but in these four
    /// places they must not be.
    #[test]
    fn the_useless_catch_all_is_gone_from_the_expression_fields() {
        for (name, src) in [
            ("the command and gizmo popup", include_str!("commands.rs")),
            ("the parameters window", crate::gui::panels_source::PANELS),
            ("the sketch dimension popup", include_str!("sketching.rs")),
        ] {
            assert!(!src.contains("cmd-expr-not-evaluated"), "{name}: the catch-all phrase is back instead of the reason");
            assert!(src.contains("expr_error_text"), "{name}: the expression error is shown past the common door");
        }
    }
}
