//! THE REBUILD CARD HOLDS ITS OWN WORDS.
//!
//! Reported behaviour: the text runs out past the edges of the rebuild window. The card was a fixed 280 px
//! wide with the label painted centred inside it, which held only while the label was short - and the line
//! that names how many nodes are being rebuilt and that a thread is among them is half again as wide.
//!
//! Checked on what is actually painted rather than on the numbers that went in: the card and the words are
//! both taken out of a real frame, and every word has to lie inside the card.
#[cfg(test)]
mod tests {
    use crate::gui::App;

    const SCREEN: egui::Vec2 = egui::vec2(1400.0, 900.0);

    /// What one frame of the overlay painted: (the card, every piece of text with where it landed).
    fn painted(app: &App, label: &str, progress: Option<(usize, usize)>) -> (egui::Rect, Vec<(String, egui::Rect)>) {
        fn walk(s: &egui::epaint::Shape, cards: &mut Vec<egui::Rect>, texts: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::epaint::Shape::Text(t) => texts.push((t.galley.text().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size()))),
                // the card is the only rounded filled rectangle here; the dimming is square and the dots are circles
                egui::epaint::Shape::Rect(r) if r.corner_radius.nw > 0 && r.fill.a() > 0 => cards.push(r.rect),
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, cards, texts)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let raw = || egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)), ..Default::default() };
        // two passes: an egui area settles into place on the second, and the cancel button is one
        let _ = ctx.run_ui(raw(), |c| {
            app.draw_dim_overlay_with(c.ctx(), label, progress, egui::Rect::NOTHING);
        });
        let out = ctx.run_ui(raw(), |c| {
            app.draw_dim_overlay_with(c.ctx(), label, progress, egui::Rect::NOTHING);
        });
        let (mut cards, mut texts) = (Vec::new(), Vec::new());
        for cs in &out.shapes {
            walk(&cs.shape, &mut cards, &mut texts);
        }
        // the button paints a rounded rectangle of its own - the card is the largest of them
        let card = cards.into_iter().max_by(|a, b| a.area().total_cmp(&b.area())).expect("the card was not painted at all");
        (card, texts)
    }

    /// THE LINE FROM THE REPORT. The longest label the rebuild has, with the counter and the cancel button
    /// under it - exactly what was on screen when the text was seen hanging outside the card.
    #[test]
    fn the_longest_rebuild_line_stays_inside_the_card() {
        let app = App::default();
        let label = crate::i18n::tr1("io-rebuilding-heavy-n", "n", "28");
        let (card, texts) = painted(&app, &label, Some((9, 28)));

        assert!(texts.iter().any(|(t, _)| t.contains(&label)), "the label was not painted at all; painted: {:?}", texts.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>());
        for (t, r) in &texts {
            assert!(
                card.contains_rect(*r),
                "\"{t}\" is painted outside the card: the word sits at {r:?}, the card is {card:?} ({} px of it hang out to the left, {} to the right)",
                (card.min.x - r.min.x).max(0.0),
                (r.max.x - card.max.x).max(0.0)
            );
        }
    }

    /// A SHORT LABEL DOES NOT SHRINK THE CARD to the width of its word: the card keeps a floor, or it would
    /// jump about in size from one rebuild to the next.
    #[test]
    fn a_short_label_keeps_the_card_its_usual_size() {
        let app = App::default();
        let (card, texts) = painted(&app, "…", None);
        assert!(card.width() >= 280.0, "the card shrank to {} px wide", card.width());
        for (t, r) in &texts {
            assert!(card.contains_rect(*r), "\"{t}\" is painted outside the card");
        }
    }

    /// EVERY LABEL THE REBUILD CAN SHOW, in both languages: none of them may hang out. The catalogue is
    /// where these lines are edited, and a translation is free to come out longer than the original.
    #[test]
    fn no_rebuild_line_in_either_language_hangs_out() {
        let app = App::default();
        for lang in ["ru", "en"] {
            crate::i18n::set_language(lang);
            for (key, n) in [("io-rebuilding-heavy-n", Some("28")), ("io-rebuilding-quiet", Some("28")), ("io-rebuilding", None), ("io-export-step", None), ("io-export-stl", None), ("io-loading", None), ("io-brep-restore", None)] {
                let label = match n {
                    Some(n) => crate::i18n::tr1(key, "n", n),
                    None => crate::i18n::tr(key),
                };
                let (card, texts) = painted(&app, &label, Some((9, 28)));
                for (t, r) in &texts {
                    assert!(card.contains_rect(*r), "[{lang}] \"{t}\" ({key}) is painted outside the card: {r:?} against {card:?}");
                }
            }
        }
        crate::i18n::set_language("ru");
    }
}
