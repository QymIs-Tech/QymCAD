//! THE RIGHT-CLICK MENU ON A PART OR A SUBASSEMBLY CAN DELETE IT.
//!
//! Reported behaviour: the menu offers Cut and Copy, and there is no Delete - so the one destructive
//! action of the three has to be reached another way, by selecting the row and finding the Del key.
//!
//! Everything below the menu was already there: Del on a selected component asks the question, and
//! `execute_delete` knows about component arrays and about deleting the context one is standing in. Only
//! the door was missing, and the item therefore goes through the SAME entry (`ask_delete`) rather than
//! calling the kernel itself - a second route to one action must not decide anything on its own.
//!
//! The menu is opened by a real right click on the row, in a real frame: the item is looked for in what
//! was painted, not in the source.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};
    use qymcad_core::model::Id;

    const SCREEN: egui::Rect = egui::Rect { min: egui::pos2(0.0, 0.0), max: egui::pos2(1400.0, 900.0) };

    /// An assembly with one part in it, and that part selected.
    fn assembly_with_a_part() -> (App, Id) {
        let app = super::super::screen_keys::tests::plate();
        let part = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        (app, part)
    }

    /// Draw the tree and return every painted label with where it was painted.
    fn draw(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<(String, egui::Rect)> {
        fn walk(s: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::epaint::Shape::Text(t) => {
                    // the phosphor icons live in the private-use block and are not part of a label
                    let text: String = t.galley.text().chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect();
                    out.push((text.trim().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size())));
                }
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let input = egui::RawInput { screen_rect: Some(SCREEN), events, ..Default::default() };
        let out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                app.build_tree_for_test(ui);
            });
        });
        let mut texts = Vec::new();
        for cs in &out.shapes {
            walk(&cs.shape, &mut texts);
        }
        texts
    }

    fn press(spot: egui::Pos2, button: egui::PointerButton) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(spot),
            egui::Event::PointerButton { pos: spot, button, pressed: true, modifiers: Default::default() },
            egui::Event::PointerButton { pos: spot, button, pressed: false, modifiers: Default::default() },
        ]
    }

    /// Open the row's menu with a real right click and return what it painted.
    fn menu_of(app: &mut App, ctx: &egui::Context, row: Id) -> Vec<(String, egui::Rect)> {
        let _ = draw(app, ctx, vec![]); // frame one: find out where the rows are
        let rect = app.tree.row_rects.iter().find(|(c, _)| *c == row).map(|(_, r)| *r).expect("the row of the part was not laid out");
        let _ = draw(app, ctx, press(rect.center(), egui::PointerButton::Secondary));
        draw(app, ctx, vec![]) // the menu is placed on the frame after the one that opened it
    }

    /// THE MENU OFFERS CUT AND COPY - without this the check below would pass on an empty menu.
    #[test]
    fn the_menu_of_a_part_offers_cut_and_copy() {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let (mut app, part) = assembly_with_a_part();
        let painted: Vec<String> = menu_of(&mut app, &ctx, part).into_iter().map(|(t, _)| t).collect();
        for key in ["act-copy-ctrl-c", "act-cut-ctrl-x"] {
            let label = crate::i18n::tr(key);
            assert!(painted.contains(&label), "setup: the menu has no `{label}`; painted: {painted:?}");
        }
    }

    /// AND IT OFFERS DELETE, WHICH ASKS. Exactly the reported complaint.
    #[test]
    fn the_menu_of_a_part_deletes_it_through_the_question() {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let (mut app, part) = assembly_with_a_part();
        let painted = menu_of(&mut app, &ctx, part);
        let label = crate::i18n::tr("act-delete-part");
        let spot = painted
            .iter()
            .find(|(t, _)| *t == label)
            .map(|(_, r)| r.center())
            .unwrap_or_else(|| panic!("the menu has no `{label}`; painted: {:?}", painted.iter().map(|(t, _)| t).collect::<Vec<_>>()));

        let _ = draw(&mut app, &ctx, press(spot, egui::PointerButton::Primary));
        assert!(
            matches!(app.deferred.delete, Some(Sel::Component(ci)) if app.project.components.get(ci).map(|c| c.id) == Some(part)),
            "the item was pressed and the question about this part was not asked"
        );
    }

    /// AND IT DOES NOT DELETE BEHIND ONE'S BACK: until the question is answered the part is still there.
    #[test]
    fn nothing_is_deleted_until_the_question_is_answered() {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let (mut app, part) = assembly_with_a_part();
        let before = app.project.components.len();
        let painted = menu_of(&mut app, &ctx, part);
        let label = crate::i18n::tr("act-delete-part");
        if let Some((_, r)) = painted.iter().find(|(t, _)| *t == label) {
            let _ = draw(&mut app, &ctx, press(r.center(), egui::PointerButton::Primary));
        }
        assert_eq!(app.project.components.len(), before, "the part went before the question was answered");
    }
}
