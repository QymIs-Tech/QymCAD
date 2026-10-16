//! A SHAPE IS DRAWN AND FINISHED THE WAY A PERSON FINISHES IT, IN THE WINDOW.
//!
//! A chain of lines and a spline take as many clicks as a person wants, so they end with a gesture: a double
//! click on the last place, or Esc. In a whole frame a double click is two clicks AND a double click - egui
//! reports both clicks before it reports the double one - so the drawing tool hears the last place twice.
//!
//! Held here: a chain ended by a double click carries no line of no length, a spline ended so has one node per
//! place clicked, Esc keeps the nodes of a spline as its hint says, and every drawing tool names its step of
//! undo after what it drew.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::EntityKind;

    fn a_sketch() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        (app, si)
    }

    /// The lines of the sketch as their ends.
    fn lines(app: &App, si: usize) -> Vec<((f64, f64), (f64, f64))> {
        let sk = &app.project.sketches[si];
        let at = |id: u64| sk.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
        sk.entities
            .iter()
            .filter_map(|e| match e.kind {
                EntityKind::Line { a, b } => Some((at(a)?, at(b)?)),
                _ => None,
            })
            .collect()
    }

    /// The nodes of every spline of the sketch, as places.
    fn spline_nodes(app: &App, si: usize) -> Vec<Vec<(f64, f64)>> {
        let sk = &app.project.sketches[si];
        sk.splines.iter().map(|s| s.points.iter().filter_map(|id| sk.points.iter().find(|p| p.id == *id).map(|p| (p.x, p.y))).collect()).collect()
    }

    /// A DOUBLE CLICK ENDS A CHAIN OF LINES WITHOUT A LINE OF NO LENGTH.
    #[test]
    fn a_double_click_ends_a_chain_without_a_line_of_no_length() {
        let (mut app, si) = a_sketch();
        Hand::new(&mut app).sk_tool(1).click2d(0.0, 0.0).click2d(10.0, 10.0).click2d(20.0, 0.0).double_click2d(30.0, 10.0);
        let got = lines(&app, si);
        let empty: Vec<_> = got.iter().filter(|(a, b)| (a.0 - b.0).hypot(a.1 - b.1) < 1e-6).collect();
        assert!(empty.is_empty() && got.len() == 3, "the chain of three segments ended by a double click holds {} lines, of no length: {empty:?}", got.len());
        assert!(app.tools.tool.pts.is_empty(), "the chain did not end: {:?} still waits for the next click", app.tools.tool.pts);
    }

    /// A DOUBLE CLICK ENDS A SPLINE ON THE PLACE IT CLICKED - once, and as one step of undo named after it.
    #[test]
    fn a_double_click_ends_a_spline_on_the_node_it_clicked() {
        let (mut app, si) = a_sketch();
        Hand::new(&mut app).sk_tool(9).click2d(0.0, 0.0).click2d(10.0, 10.0).click2d(20.0, 0.0).double_click2d(30.0, 10.0);
        let got = spline_nodes(&app, si);
        assert_eq!(got, vec![vec![(0.0, 0.0), (10.0, 10.0), (20.0, 0.0), (30.0, 10.0)]], "four places were clicked for the spline");
        let step = app.disk.edits.undo.last().map(|s| s.name.clone());
        assert_eq!(step.as_deref(), Some(crate::i18n::tr("sk-spline").as_str()), "the spline is not one step of undo named after it");
    }

    /// THE NODES OF A SPLINE ARE DRAGGED WITH THE MOUSE in a sketch that holds nothing but the spline - the ends and a
    /// middle one - and an undo puts each back.
    ///
    /// Reported behaviour (issue #30): the tangent handle turned the curve, but a node did not move, because a sketch of
    /// splines alone was not taken for one that can be edited.
    #[test]
    fn the_nodes_of_a_lone_spline_are_dragged() {
        // off the axes: a node put on one is held there
        let drawn = vec![vec![(10.0, 10.0), (30.0, 20.0), (50.0, 10.0)]];
        let mut problems = Vec::new();
        for (node, to) in [(0usize, (10.0, 0.5)), (1, (30.0, 35.0)), (2, (55.0, 2.0))] {
            let (mut app, si) = a_sketch();
            Hand::new(&mut app).sk_tool(9).click2d(10.0, 10.0).click2d(30.0, 20.0).double_click2d(50.0, 10.0);
            assert_eq!(spline_nodes(&app, si), drawn, "setup: the spline as clicked");
            Hand::new(&mut app).sk_tool(0).drag2d(drawn[0][node], to);
            let got = spline_nodes(&app, si)[0][node];
            if (got.0 - to.0).hypot(got.1 - to.1) > 0.5 {
                problems.push(format!("node {node} dragged to {to:?} stands at {got:?}"));
            }
            Hand::new(&mut app).undo();
            if spline_nodes(&app, si) != drawn {
                problems.push(format!("node {node}: an undo leaves {:?}", spline_nodes(&app, si)));
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// A CONSTRAINT ON SEVERAL SELECTED LINES IS PUT ON EVERY ONE OF THEM: Vertical and Horizontal on each, Parallel,
    /// Equal and Collinear tying each to the first; one undo takes them all back.
    ///
    /// Reported behaviour (issue #34): with three lines selected, Vertical turned one of them and said "The constraint
    /// is added".
    #[test]
    fn a_constraint_goes_on_every_selected_line() {
        use qymcad_core::model::{Constraint, EntityKind};
        /// A constraint button, its name, and how many constraints of its kind three selected lines should get.
        struct Button {
            code: u8,
            name: &'static str,
            want: usize,
        }
        let buttons = [
            Button { code: 2, name: "Vertical", want: 3 },
            Button { code: 1, name: "Horizontal", want: 3 },
            Button { code: 3, name: "Parallel", want: 2 },
            Button { code: 5, name: "Equal", want: 2 },
            Button { code: 7, name: "Collinear", want: 2 },
        ];
        let slanted = |app: &App, si: usize| -> usize {
            let sk = &app.project.sketches[si];
            let at = |id: u64| sk.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y)).expect("a point");
            sk.entities
                .iter()
                .filter(|e| match e.kind {
                    EntityKind::Line { a, b } => (at(a).0 - at(b).0).abs() > 1e-6,
                    _ => false,
                })
                .count()
        };
        let mut problems = Vec::new();
        for Button { code: button, name, want } in buttons {
            let (mut app, si) = a_sketch();
            // three slants of their own: lines drawn at one slant are tied Parallel as they are drawn, and then one
            // Vertical turns them all
            for (x, lean) in [(0.0, 5.0), (20.0, 9.0), (40.0, 3.0)] {
                Hand::new(&mut app).sk_tool(1).click2d(x, 0.0).double_click2d(x + lean, 12.0);
                Hand::new(&mut app).key(egui::Key::Escape);
            }
            let lines: Vec<(u8, u64)> = app.project.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Line { .. })).map(|e| (1u8, e.id)).collect();
            assert_eq!(lines.len(), 3, "setup: three lines");
            assert_eq!(slanted(&app, si), 3, "setup: the three lines are slanted");
            let ties = app.project.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::Parallel { .. } | Constraint::Equal { .. } | Constraint::Collinear { .. })).count();
            assert_eq!(ties, 0, "setup: nothing ties the lines together");
            let before = app.project.sketches[si].constraints.len();
            assert!(Hand::new(&mut app).select2d(&lines), "setup: the three lines are picked");
            Hand::new(&mut app).constraint(button);
            let added = app.project.sketches[si].constraints.len() - before;
            let of_kind = app.project.sketches[si].constraints[before..]
                .iter()
                .filter(|c| {
                    matches!(
                        (button, c),
                        (2, Constraint::Vertical { .. }) | (1, Constraint::Horizontal { .. }) | (3, Constraint::Parallel { .. }) | (5, Constraint::Equal { .. }) | (7, Constraint::Collinear { .. })
                    )
                })
                .count();
            if of_kind != want {
                problems.push(format!("{name} on three lines added {of_kind} of its kind ({added} in all), not {want}; status {:?}", app.status));
            }
            if button == 2 && slanted(&app, si) > 0 {
                problems.push(format!("after Vertical {} of the three lines are not vertical", slanted(&app, si)));
            }
            Hand::new(&mut app).undo();
            if app.project.sketches[si].constraints.len() != before {
                problems.push(format!("one undo after {name} leaves {} constraints, not {before}", app.project.sketches[si].constraints.len()));
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// A CONSTRAINT THAT WOULD SHRINK A LINE TO A POINT IS NOT KEPT: Horizontal on lines already Vertical is met only
    /// by a line of no length, and the sketch must say so rather than take it - one line, and three at once.
    ///
    /// Reported behaviour (found checking issue #34): three lines made Vertical, then Horizontal - the lines shrank to
    /// points and both constraints stood green.
    #[test]
    fn a_constraint_that_shrinks_a_line_to_a_point_is_refused() {
        use qymcad_core::model::{Constraint, EntityKind};
        let mut problems = Vec::new();
        for count in [1usize, 3] {
            let (mut app, si) = a_sketch();
            for (x, lean) in [(0.0, 5.0), (20.0, 9.0), (40.0, 3.0)].into_iter().take(count) {
                Hand::new(&mut app).sk_tool(1).click2d(x, 0.0).double_click2d(x + lean, 12.0);
                Hand::new(&mut app).key(egui::Key::Escape);
            }
            let lines: Vec<(u8, u64)> = app.project.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Line { .. })).map(|e| (1u8, e.id)).collect();
            assert!(Hand::new(&mut app).select2d(&lines), "setup: the lines are picked");
            Hand::new(&mut app).constraint(2);
            let vertical = app.project.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::Vertical { .. })).count();
            assert_eq!(vertical, count, "setup: every line is Vertical");
            assert!(Hand::new(&mut app).select2d(&lines), "setup: the lines are picked again");
            Hand::new(&mut app).constraint(1);
            let sk = &app.project.sketches[si];
            let at = |id: u64| sk.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y)).expect("a point");
            let shortest = sk
                .entities
                .iter()
                .filter_map(|e| match e.kind {
                    EntityKind::Line { a, b } => Some((at(a).0 - at(b).0).hypot(at(a).1 - at(b).1)),
                    _ => None,
                })
                .fold(f64::MAX, f64::min);
            let horizontal = sk.constraints.iter().filter(|c| matches!(c, Constraint::Horizontal { .. })).count();
            if shortest < 1.0 || horizontal != 0 {
                problems.push(format!("{count} line(s): Horizontal over Vertical left the shortest line {shortest:.3} long and {horizontal} Horizontal in the sketch; status {:?}", app.status));
            }
            if app.status == qymcad_ui_state::Notice::info(crate::i18n::tr("sk-constraint-added")) {
                problems.push(format!("{count} line(s): the status says the constraint is added"));
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// A CONSTRAINT ALREADY HELD IS NOT LAID TWICE, AND ONE A NEW RELATION IMPLIES GOES: Vertical on three lines one of
    /// which is Vertical already adds two, not a second on that one; Vertical again on all three adds nothing and says
    /// so; Collinear on the three vertical lines takes away the Vertical it makes redundant - no constraint is left
    /// redundant at any step.
    ///
    /// Reported behaviour (found checking issue #34): a line made Vertical while it was drawn got a second Vertical with
    /// the rest, and the sketch turned yellow; Collinear on vertical lines did the same.
    #[test]
    fn a_constraint_already_held_is_not_laid_twice() {
        use qymcad_core::model::{Constraint, EntityKind};
        let (mut app, si) = a_sketch();
        for (x, lean) in [(0.0, 5.0), (20.0, 9.0), (40.0, 3.0)] {
            Hand::new(&mut app).sk_tool(1).click2d(x, 0.0).double_click2d(x + lean, 12.0);
            Hand::new(&mut app).key(egui::Key::Escape);
        }
        let lines: Vec<(u8, u64)> = app.project.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Line { .. })).map(|e| (1u8, e.id)).collect();
        let count = |app: &App, vertical: bool| {
            app.project.sketches[si].constraints.iter().filter(|c| if vertical { matches!(c, Constraint::Vertical { .. }) } else { matches!(c, Constraint::Collinear { .. }) }).count()
        };
        let mut problems = Vec::new();
        assert!(Hand::new(&mut app).select2d(&lines[..1]), "setup: the first line is picked");
        Hand::new(&mut app).constraint(2);
        assert_eq!(count(&app, true), 1, "setup: the first line is Vertical");

        assert!(Hand::new(&mut app).select2d(&lines), "setup: the three lines are picked");
        Hand::new(&mut app).constraint(2);
        let redundant = app.project.sketch_redundant_constraints(si);
        if count(&app, true) != 3 || !redundant.is_empty() {
            problems.push(format!("Vertical on three lines, one Vertical already: {} Vertical, redundant {redundant:?}; status {:?}", count(&app, true), app.status));
        }

        assert!(Hand::new(&mut app).select2d(&lines), "setup: the three lines are picked again");
        Hand::new(&mut app).constraint(2);
        if count(&app, true) != 3 || app.status != qymcad_ui_state::Notice::info(crate::i18n::tr("sk-constraint-already")) {
            problems.push(format!("Vertical again on three vertical lines: {} Vertical; status {:?}", count(&app, true), app.status));
        }

        assert!(Hand::new(&mut app).select2d(&lines), "setup: the three lines are picked for Collinear");
        Hand::new(&mut app).constraint(7);
        let redundant = app.project.sketch_redundant_constraints(si);
        if count(&app, false) != 2 || count(&app, true) != 1 || !redundant.is_empty() {
            problems.push(format!("Collinear on three vertical lines: {} Collinear, {} Vertical, redundant {redundant:?}; status {:?}", count(&app, false), count(&app, true), app.status));
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// WITH A DRAWING TOOL IN HAND A DRAG NEITHER SELECTS NOR MOVES WHAT IS DRAWN: a band over two lines selects nothing,
    /// a drag from the end of a line leaves it where it is - with every drawing tool; in selection mode both work as
    /// before.
    ///
    /// Reported behaviour (issue #33): with Line in hand a drag over empty space selected the geometry in the band, and a
    /// drag from a point moved the point.
    #[test]
    fn a_drawing_tool_in_hand_takes_no_drag() {
        let lines_drawn = |app: &mut App| {
            for y in [0.0, 10.0] {
                Hand::new(app).sk_tool(1).click2d(0.0, y).double_click2d(20.0, y);
                Hand::new(app).key(egui::Key::Escape);
            }
        };
        let end_at = |app: &App, si: usize| app.project.sketches[si].points.iter().any(|p| (p.x - 20.0).abs() < 1e-6 && (p.y - 10.0).abs() < 1e-6);
        let mut problems = Vec::new();
        for tool in [1u8, 2, 3, 4, 7] {
            let (mut app, si) = a_sketch();
            lines_drawn(&mut app);
            Hand::new(&mut app).sk_tool(tool).drag2d((-5.0, -5.0), (25.0, 15.0));
            if !app.tools.sel_sk.items.is_empty() {
                problems.push(format!("tool {tool}: a band selected {} items; status {:?}", app.tools.sel_sk.items.len(), app.status));
            }
            Hand::new(&mut app).drag2d((20.0, 10.0), (25.0, 18.0));
            if !end_at(&app, si) {
                problems.push(format!("tool {tool}: a drag from the end of a line moved it"));
            }
        }
        // in selection mode the same drags still select and move
        let (mut app, si) = a_sketch();
        lines_drawn(&mut app);
        Hand::new(&mut app).sk_tool(0).drag2d((-5.0, -5.0), (25.0, 15.0));
        if app.tools.sel_sk.items.is_empty() {
            problems.push("selection mode: a band selected nothing".to_string());
        }
        Hand::new(&mut app).key(egui::Key::Escape);
        Hand::new(&mut app).drag2d((20.0, 10.0), (25.0, 18.0));
        if end_at(&app, si) {
            problems.push("selection mode: a drag from the end of a line did not move it".to_string());
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// The place, the angle and the box of the glyphs of the texts of a sketch.
    fn text_boxes(app: &App, si: usize) -> Vec<TextBox> {
        app.project.sketches[si]
            .texts
            .iter()
            .map(|t| {
                let pts: Vec<_> = t.glyphs.iter().flatten().collect();
                TextBox {
                    at: (t.x, t.y),
                    angle: t.angle,
                    lo: (pts.iter().map(|p| p.x).fold(f64::MAX, f64::min), pts.iter().map(|p| p.y).fold(f64::MAX, f64::min)),
                    hi: (pts.iter().map(|p| p.x).fold(f64::MIN, f64::max), pts.iter().map(|p| p.y).fold(f64::MIN, f64::max)),
                }
            })
            .collect()
    }

    /// Where a text stands, its angle, and the corners of the box round its glyphs.
    #[derive(Debug, Clone, Copy)]
    struct TextBox {
        at: (f64, f64),
        angle: f64,
        lo: (f64, f64),
        hi: (f64, f64),
    }

    /// A sketch with the text `CAD`, 10 high, placed at the origin; the point of a stroke of its first letter.
    fn a_text() -> (App, usize, (f64, f64)) {
        let (mut app, si) = a_sketch();
        Hand::new(&mut app).sk_text("CAD", 10.0).click2d(0.0, 0.0).key(egui::Key::Escape);
        let g = app.project.sketches[si].texts[0].glyphs[0][0];
        (app, si, (g.x, g.y))
    }

    /// THE TEXT TOOL IS PUT DOWN ONCE A LABEL IS PLACED: a second click places nothing, as in the CAD programs people
    /// know - a label is placed once, and a stray click must not lay a copy of it.
    ///
    /// Reported behaviour (found checking issue #32): after a label was placed the tool stayed in hand, offering to place
    /// the same label again on every click until Esc.
    #[test]
    fn the_text_tool_is_put_down_once_a_label_is_placed() {
        let (mut app, si) = a_sketch();
        Hand::new(&mut app).sk_text("CAD", 10.0).click2d(0.0, 0.0);
        let after_one = app.project.sketches[si].texts.len();
        let in_hand = app.tools.armed.draw_kind();
        Hand::new(&mut app).click2d(0.0, 30.0);
        let after_two = app.project.sketches[si].texts.len();
        assert!(after_one == 1 && in_hand != 11 && after_two == 1, "one label placed: {after_one}; the tool in hand after it: {in_hand}; labels after a second click: {after_two}");
    }

    /// TAKEN UP AGAIN, THE TEXT TOOL OFFERS AN EMPTY STRING: the label placed or the one edited is not offered once more;
    /// the height and the font stay as they were set.
    ///
    /// Reported behaviour (found checking issue #32): with the tool taken again, the string typed for the last label
    /// stood in the field, and a click placed the same label again.
    #[test]
    fn the_text_tool_taken_again_offers_an_empty_string() {
        let (mut app, si) = a_sketch();
        Hand::new(&mut app).sk_text("CAD", 10.0).click2d(0.0, 0.0);
        Hand::new(&mut app).sk_tool(11);
        let after_placing = (app.tool_prefs.text.clone(), app.tool_prefs.text_h);
        Hand::new(&mut app).key(egui::Key::Escape);
        let g = app.project.sketches[si].texts[0].glyphs[0][0];
        Hand::new(&mut app).sk_edit_text((g.x, g.y), "CADX", 10.0);
        Hand::new(&mut app).sk_tool(11);
        let after_editing = app.tool_prefs.text.clone();
        assert!(
            after_placing.0.is_empty() && (after_placing.1 - 10.0).abs() < 1e-9 && after_editing.is_empty(),
            "taken again after placing, the field holds {:?} (height {}); after an edit, {:?}",
            after_placing.0,
            after_placing.1,
            after_editing
        );
    }

    /// WHILE A LABEL IS EDITED, NO NEW ONE FOLLOWS THE POINTER: the text tool shows where a click would put a label when it
    /// is taken to write one, and not when a double click opened an existing label for editing.
    ///
    /// Reported behaviour (found checking issue #32): a double click on a text, and a yellow copy of it moved about under
    /// the cursor.
    #[test]
    fn an_edited_label_has_no_ghost_at_the_pointer() {
        // the pointer far from the label: whatever is drawn there in the colour of the sketch follows the pointer
        let away = (60.0, -40.0);
        let (mut app, _si, on) = a_text();
        let ink = app.scheme.pal.sketch_line();
        // the tool taken up again offers an empty string: a new one is typed, as for the next label
        Hand::new(&mut app).sk_text("QYM", 10.0);
        let writing = Hand::new(&mut app).strokes_near2d(away, ink);
        assert!(writing > 0, "GUARD: the text tool taken to write shows no label at the pointer, so the check below would see nothing");
        Hand::new(&mut app).key(egui::Key::Escape);
        let mut hand = Hand::new(&mut app);
        assert!(hand.sk_open_text_edit(on), "setup: the double click opens the label for editing");
        let editing = hand.strokes_near2d(away, ink);
        assert_eq!(editing, 0, "a label being edited has {editing} strokes of the text tool's ghost following the pointer");
    }

    /// A TEXT IS TURNED, MOVED AND COPIED BY THE TOOLS, alone and together with a line, and stays turned after its string
    /// is edited; the outlines a profile is taken from turn with it.
    ///
    /// Reported behaviour (issue #32): with Rotate in hand a click on a text did not pick it, and the status kept asking
    /// to click an entity; Move did the same. A text could be dragged by hand only, and stood level always.
    #[test]
    fn a_text_is_turned_moved_and_copied_by_the_tools() {
        let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05;
        let mut problems = Vec::new();

        // turned by 90 deg about the origin: the box of the glyphs turns with it
        let (mut app, si, on) = a_text();
        let level = text_boxes(&app, si)[0];
        Hand::new(&mut app).sk_rotate(on, (0.0, 0.0), 90.0);
        let turned = text_boxes(&app, si)[0];
        let want_lo = (-level.hi.1, level.lo.0);
        let want_hi = (-level.lo.1, level.hi.0);
        if (turned.angle - 90.0).abs() > 1e-6 || !near(turned.lo, want_lo) || !near(turned.hi, want_hi) {
            problems.push(format!("Rotate by 90: {level:?} became {turned:?}, the box should be {want_lo:?}-{want_hi:?}; status {:?}", app.status));
        }
        // the outlines of the profile turned too: the contours of the sketch lie in the turned box
        let contours: Vec<_> = app.project.sketches[si].contour_ids.iter().filter_map(|c| app.project.contour_index(*c)).flat_map(|ci| app.project.contours[ci].points.clone()).collect();
        let cx = contours.iter().map(|p| p.x).fold(f64::MIN, f64::max);
        if contours.is_empty() || cx > turned.hi.0 + 0.05 {
            problems.push(format!("the outlines of the profile did not turn: they reach x = {cx:.2}, the text {turned:?}"));
        }
        // its string edited afterwards: it stays turned
        let mid = ((turned.lo.0 + turned.hi.0) / 2.0, (turned.lo.1 + turned.hi.1) / 2.0);
        let g = app.project.sketches[si].texts[0].glyphs[0][0];
        Hand::new(&mut app).sk_edit_text((g.x, g.y), "CADX", 10.0);
        let edited = text_boxes(&app, si)[0];
        if app.project.sketches[si].texts[0].text != "CADX" || (edited.angle - 90.0).abs() > 1e-6 || edited.hi.1 - edited.lo.1 < edited.hi.0 - edited.lo.0 {
            problems.push(format!("edited after the turn: {:?} {edited:?} - a turned text stands taller than wide (the middle was {mid:?})", app.project.sketches[si].texts[0].text));
        }

        // moved
        let (mut app, si, on) = a_text();
        let before = text_boxes(&app, si)[0];
        Hand::new(&mut app).sk_move(1, on, (0.0, 0.0), (10.0, 10.0));
        let after = text_boxes(&app, si)[0];
        if !near(after.at, (10.0, 10.0)) || !near(after.lo, (before.lo.0 + 10.0, before.lo.1 + 10.0)) {
            problems.push(format!("Move by (10, 10): {before:?} became {after:?}; status {:?}", app.status));
        }

        // copied
        let (mut app, si, on) = a_text();
        Hand::new(&mut app).sk_move(2, on, (0.0, 0.0), (0.0, 20.0));
        let all = text_boxes(&app, si);
        if all.len() != 2 || !all.iter().any(|t| near(t.at, (0.0, 20.0))) || !all.iter().any(|t| near(t.at, (0.0, 0.0))) {
            problems.push(format!("Copy to (0, 20): the texts stand at {:?}; status {:?}", all.iter().map(|t| t.at).collect::<Vec<_>>(), app.status));
        }

        // a text and a line turned together: a slanted line, which drawing ties to nothing
        let (mut app, si, on) = a_text();
        Hand::new(&mut app).sk_tool(1).click2d(0.0, -10.0).double_click2d(15.0, -18.0);
        Hand::new(&mut app).key(egui::Key::Escape);
        Hand::new(&mut app).sk_tool(0).click2d(on.0, on.1).shift_click2d(7.5, -14.0);
        Hand::new(&mut app).sk_rotate_selected((0.0, 0.0), 90.0);
        let text = text_boxes(&app, si)[0];
        let pts: Vec<(f64, f64)> = app.project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect();
        let turned_line = [(10.0, 0.0), (18.0, 15.0)].iter().all(|q| pts.iter().any(|p| near(*p, *q)));
        if (text.angle - 90.0).abs() > 1e-6 || !turned_line {
            problems.push(format!("a text and a line turned by 90: the text {text:?}, the points {pts:?}; status {:?}", app.status));
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// Esc ENDS A SPLINE, AS ITS HINT SAYS, keeping the nodes that were clicked.
    #[test]
    fn escape_ends_a_spline_as_its_hint_says() {
        let (mut app, si) = a_sketch();
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(9).click2d(0.0, 0.0).click2d(10.0, 10.0).click2d(20.0, 0.0);
        let hint = hand.app.status.clone();
        hand.key(egui::Key::Escape).close_window();
        assert_eq!(spline_nodes(&app, si), vec![vec![(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)]], "Esc on a spline of three nodes, under the hint {hint:?}");
    }

    /// A drawing tool by its number, the key of its name, and the clicks that draw with it.
    struct Drawing {
        tool: u8,
        key: &'static str,
        clicks: &'static [(f64, f64)],
    }

    fn drawing(tool: u8, key: &'static str, clicks: &'static [(f64, f64)]) -> Drawing {
        Drawing { tool, key, clicks }
    }

    /// EVERY DRAWING TOOL NAMES ITS STEP OF UNDO AFTER WHAT IT DREW.
    ///
    /// Failures are gathered and given out together.
    #[test]
    fn every_drawing_tool_names_its_step_of_undo() {
        let tools: [Drawing; 11] = [
            drawing(1, "sk-line", &[(0.0, 0.0), (20.0, 0.0)]),
            drawing(2, "sk-rect", &[(0.0, 0.0), (20.0, 15.0)]),
            drawing(3, "sk-circle", &[(0.0, 0.0), (8.0, 0.0)]),
            drawing(4, "sk-arc", &[(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)]),
            drawing(5, "sk-point", &[(5.0, 5.0)]),
            drawing(6, "sk-polygon", &[(0.0, 0.0), (8.0, 0.0)]),
            drawing(7, "sk-slot", &[(0.0, 0.0), (15.0, 0.0), (15.0, 5.0)]),
            drawing(8, "sk-ellipse", &[(0.0, 0.0), (12.0, 0.0), (6.0, 7.0)]),
            drawing(9, "sk-spline", &[(0.0, 0.0), (7.0, 7.0), (15.0, 0.0)]),
            drawing(10, "sk-circle", &[(0.0, 0.0), (8.0, 5.0), (12.0, -3.0)]),
            drawing(11, "sk-text", &[(0.0, 0.0)]),
        ];
        let mut problems = Vec::new();
        for Drawing { tool, key, clicks } in tools {
            let (mut app, _si) = a_sketch();
            let mut hand = Hand::new(&mut app);
            if tool == 11 {
                hand.sk_text("CAD", 5.0);
            } else {
                hand.sk_tool(tool);
            }
            let (last, before) = clicks.split_last().expect("every tool takes a click");
            for (x, y) in before {
                hand.click2d(*x, *y);
            }
            if tool == 9 {
                hand.double_click2d(last.0, last.1);
            } else {
                hand.click2d(last.0, last.1);
            }
            let step = app.disk.edits.undo.last().map(|s| s.name.clone());
            if step.as_deref() != Some(crate::i18n::tr(key).as_str()) {
                problems.push(format!("tool {tool}: the step of undo is {step:?}, it should be {:?}", crate::i18n::tr(key)));
            }
        }
        assert!(problems.is_empty(), "a drawing is undone under another name:\n{}", problems.join("\n"));
    }
}
