//! THE MOVE TOOL IS WORKED WITH THE MOUSE, IN THE WINDOW.
//!
//! Moving, copying and turning a shape in a sketch is three clicks - the shape, the base point, the target -
//! and for a turn the angle typed into a popup at the centre. Here every click and every key goes through the
//! whole frame of the window, so what the frame decides is checked too: who takes the press, what the top bar
//! calls the tool in hand, and whether the edit becomes a step of undo named after it.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::{Purpose, SketchPlane};

    /// A running window with a sketch open for editing and one free line in it, from (10,0) to (30,0).
    ///
    /// The line is laid by the core rather than drawn: the line tool puts a horizontal constraint on a
    /// horizontal line, and a line that may not turn is no test of turning.
    fn a_free_line() -> (App, usize) {
        let (mut app, _ctx) = crate::gui::import_door::tests::running();
        // THE SETUP IS AN OPERATION OF ITS OWN, so the gesture's step of undo starts where the setup ended. Made
        // outside one, the setup is taken into the document's key by the next frame's rebuild check while the
        // snapshot undo restores stays behind it: a step without a name then took the sketch away as well.
        qymcad_ui_state::begin_edit(&mut app.disk.edits, &app.project, "setup");
        let si = app.create_sketch_on(SketchPlane::default());
        app.project.add_line_entity(si, 10.0, 0.0, 30.0, 0.0, Purpose::Real);
        qymcad_ui_state::commit_edit(&mut app.rebuild_ctx());
        app.chosen.sel = Sel::Sketch(si);
        (app, si)
    }

    /// The ends of every line of the sketch, rounded to a thousandth so a turn by 90 deg compares exactly.
    fn lines(app: &App, si: usize) -> Vec<[(f64, f64); 2]> {
        let Some(sk) = app.project.sketches.get(si) else { return Vec::new() };
        let at = |id: u64| sk.points.iter().find(|p| p.id == id).map(|p| ((p.x * 1e3).round() / 1e3, (p.y * 1e3).round() / 1e3));
        let mut out: Vec<[(f64, f64); 2]> = sk
            .entities
            .iter()
            .filter_map(|e| match e.kind.as_line() {
                Some(qymcad_core::model::LineParts { a, b }) => Some([at(a)?, at(b)?]),
                _ => None,
            })
            .map(|mut l| {
                l.sort_by(|p, q| p.partial_cmp(q).expect("coordinates are numbers"));
                l.map(|(x, y)| (x + 0.0, y + 0.0)) // -0.0 prints apart from 0.0
            })
            .collect();
        out.sort_by(|p, q| p.partial_cmp(q).expect("coordinates are numbers"));
        out
    }

    /// A tool by what it does, the key of its step of undo, and the lines it should leave.
    struct Case {
        what: &'static str,
        key: &'static str,
        want: Vec<[(f64, f64); 2]>,
    }

    fn case(what: &'static str, key: &'static str, want: Vec<[(f64, f64); 2]>) -> Case {
        Case { what, key, want }
    }

    /// EACH OF THE THREE LANDS WHERE IT WAS ASKED, IS ONE STEP OF UNDO NAMED AFTER THE TOOL, AND Ctrl+Z TAKES IT
    /// BACK.
    ///
    /// Failures are gathered and given out together, so one run shows all three tools.
    #[test]
    fn the_move_copy_and_turn_land_where_asked_and_undo_by_name() {
        let start = vec![[(10.0, 0.0), (30.0, 0.0)]];
        let cases: [Case; 3] = [
            case("move", "tool-move", vec![[(14.0, 4.0), (34.0, 4.0)]]),
            case("copy", "tool-copy", vec![[(10.0, 0.0), (30.0, 0.0)], [(14.0, 4.0), (34.0, 4.0)]]),
            case("turn", "tool-rotate", vec![[(20.0, -10.0), (20.0, 10.0)]]),
        ];
        let mut problems = Vec::new();
        for Case { what, key, want } in cases {
            let (mut app, si) = a_free_line();
            let steps = app.disk.edits.undo.len();
            let mut hand = Hand::new(&mut app);
            match what {
                "move" => hand.sk_move(qymcad_ui_state::MoveKind::Move, (20.0, 0.0), (20.0, 0.0), (24.0, 4.0)),
                "copy" => hand.sk_move(qymcad_ui_state::MoveKind::Copy, (20.0, 0.0), (20.0, 0.0), (24.0, 4.0)),
                _ => hand.sk_rotate((20.0, 0.0), (20.0, 0.0), 90.0),
            };
            let got = lines(hand.app, si);
            if got != want {
                problems.push(format!("{what}: the lines stand at {got:?}, asked for {want:?}; status {:?}", hand.app.status));
            }
            let named: Vec<String> = hand.app.disk.edits.undo.iter().skip(steps).map(|s| s.name.clone()).collect();
            if named != [crate::i18n::tr(key)] {
                problems.push(format!("{what}: the steps of undo it left are {named:?}, not one step named {:?}", crate::i18n::tr(key)));
            }
            hand.undo();
            let back = lines(hand.app, si);
            if back != start {
                problems.push(format!("{what}: Ctrl+Z left the lines at {back:?}, they stood at {start:?}"));
            }
        }
        assert!(problems.is_empty(), "the move tool worked in the window:\n{}", problems.join("\n"));
    }

    /// THE TOP BAR NAMES THE TOOL IN HAND - each of the three by its own name.
    #[test]
    fn the_top_bar_names_the_tool_in_hand() {
        use crate::gui::import_door::tests::frame;
        let mut problems = Vec::new();
        for (op, key) in [(qymcad_ui_state::MoveKind::Move, "tool-move"), (qymcad_ui_state::MoveKind::Copy, "tool-copy"), (qymcad_ui_state::MoveKind::Rotate, "tool-rotate")] {
            let (mut app, _si) = a_free_line();
            let ctx = egui::Context::default();
            crate::gui::install_fonts(&ctx);
            Hand::new(&mut app).sk_select().sk_move_tool(op);
            frame(&mut app, &ctx, Vec::new());
            let texts = frame(&mut app, &ctx, Vec::new());
            let names: Vec<&str> = ["tool-move", "tool-copy", "tool-rotate"].into_iter().filter(|k| texts.iter().any(|(t, _)| *t == crate::i18n::tr(k))).collect();
            if names != [key] {
                problems.push(format!("tool {op:?}: the bar says {:?}, it should say {:?}", names.iter().map(|k| crate::i18n::tr(k)).collect::<Vec<_>>(), crate::i18n::tr(key)));
            }
        }
        assert!(problems.is_empty(), "the top bar misnames the tool in hand:\n{}", problems.join("\n"));
    }
}
