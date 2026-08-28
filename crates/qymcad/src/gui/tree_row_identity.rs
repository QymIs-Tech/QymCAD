//! A TREE ROW IS ADDRESSED BY THE FEATURE Id, NOT BY ITS NUMBER IN THE TIMELINE. Written from a
//! reported crash.
//!
//! `index out of bounds: the len is 14 but the index is 15` in the walk over the timeline. The cause:
//! the numbers of all the rows are taken ONCE before the loop, while a row can delete a feature and
//! swap it with its neighbour inside that very same pass (immediate mode). After a deletion the
//! timeline is shorter — the last number runs off the end.
//!
//! THE GUARD EXISTED, BUT ON THE WRONG SIDE: `tree_feature_row` checked `ti >= len` on its own entry,
//! while the crash happened in the CALLER one line earlier, on reading `timeline[ti].id`. And even
//! had it fired, a quieter trouble would have remained: after a deletion the numbers SHIFT, a
//! surviving index lands in the NEIGHBOURING feature, the tree draws the wrong row, and a click on it
//! edits the wrong node. That is worse than a crash — it is silent.
//!
//! An Id survives both deletion and reordering.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// A part with three features in a row — there is something to delete from the middle.
    fn part_with_three(app: &mut App) -> Vec<u64> {
        for (i, h) in [(0usize, 10.0f64), (1, 4.0), (2, 3.0)] {
            let si = app.create_sketch_on(SketchPlane::default());
            let d = 20.0 - i as f64 * 4.0;
            app.project.add_rect_entity(si, -d, -d, d, d, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(si);
            app.finish_sketch_edit();
            app.sel = Sel::Sketch(si);
            app.start_feat_cmd(1);
            app.feat.op = 0;
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = h;
                p.txt = format!("{h}");
            }
            app.apply_feat_cmd();
        }
        app.project.timeline.iter().filter(|n| n.kind.body().is_some()).map(|n| n.id).collect()
    }

    /// THE TREE WALK RESOLVES THE NODE BY Id AT EVERY STEP.
    ///
    /// A guard over the source, because the trouble is in the way of addressing rather than in a
    /// value: an index taken before the loop stays syntactically correct after a deletion too, it
    /// simply points somewhere else.
    #[test]
    fn the_tree_loop_resolves_each_row_by_id() {
        let src = crate::gui::panels_source::PANELS;
        let code = src.split("#[cfg(test)]").next().expect("the working part");
        // Collecting the Ids BEFORE the loop is legitimate — the timeline is still intact there.
        // Look at the BODY of the loop: a number taken in advance can already point elsewhere.
        assert!(code.contains("let feat_ids:"), "the walk over the bodies must collect feature Ids, not only their numbers");
        let i = code.find("for (idx, &id) in feat_ids").expect("the row loop must run over feature Ids");
        let loop_body = &code[i..i + 700];
        assert!(
            loop_body.contains("timeline_index(id)"),
            "a tree row must resolve the node by Id AT EVERY step: a deletion or a reorder in the same pass \
             shifts the numbers, and an index taken in advance lands in somebody else's feature"
        );
        assert!(
            !loop_body.contains("timeline[ti]"),
            "inside the loop the timeline is indexed by a number taken in advance — exactly what crashed \
             (\"len is 14 but the index is 15\")"
        );
    }

    /// DELETING A FEATURE FROM THE MIDDLE DOES NOT KNOCK THE OTHER ROWS OFF.
    ///
    /// After a deletion the numbers shift; checked here is that every surviving feature is still
    /// found BY ITS OWN Id — that is, the tree draws that very feature and not its neighbour.
    #[test]
    fn surviving_features_still_resolve_to_themselves() {
        let mut app = App::default();
        let ids = part_with_three(&mut app);
        assert!(ids.len() >= 3, "setup: three body features were expected, and it came out {}", ids.len());

        let victim = ids[1];
        app.execute_delete(Sel::Feature(app.project.timeline_index(victim).expect("the node is there")));

        assert!(app.project.timeline_index(victim).is_none(), "a deleted feature must disappear from the timeline");
        for id in ids.iter().copied().filter(|&i| i != victim) {
            if let Some(ti) = app.project.timeline_index(id) {
                assert_eq!(app.project.timeline[ti].id, id, "a feature must be found by ITS OWN Id, not by a number that has shifted");
            }
        }
    }

    /// THE TREE DRAWS. A frame is built both before and after a deletion — there is no crash in
    /// the walk over the timeline.
    #[test]
    fn the_tree_panel_draws_before_and_after_a_deletion() {
        let mut app = App::default();
        let ids = part_with_three(&mut app);
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0));
        let draw = |app: &mut App| {
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            let _ = ctx.run_ui(input.clone(), |ctx| app.tree_panel(ctx));
            let _ = ctx.run_ui(input, |ctx| app.tree_panel(ctx));
        };
        draw(&mut app);
        if let Some(ti) = app.project.timeline_index(ids[1]) {
            app.execute_delete(Sel::Feature(ti));
        }
        draw(&mut app);
    }
}
