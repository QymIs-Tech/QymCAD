//! A PROJECTION IS GEOMETRY OF THE SKETCH, worked by hand: the ends, middles and curves of a projected outline are
//! snapped to, it turns construction and back, it is made ordinary geometry, and a piece of it deleted, trimmed or broken
//! leaves ordinary geometry behind.
//!
//! Reported behaviour: "projected geometry does not work adequately" - nothing snapped to it, it could not be made
//! construction or ordinary geometry, and it could not be told apart while projected.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::model::{Constraint, EntityKind};

    /// A part with a cube and a sketch on its top face, the outline of that face projected into it. Answers the sketch.
    fn a_projected_outline(app: &mut App) -> usize {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let mi = app.project.mesh_index(body).expect("the mesh");
        let key = app.project.bodies[mi]
            .faces
            .iter()
            .filter(|f| f.normal[2] > 0.9)
            .max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z))
            .map(|f| qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
            .expect("the top face is there");
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::Face(body, key));
        app.chosen.sel = Sel::Sketch(si);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_of!(app), &mut app.viewing.mode_3d, 6);
        app.tools.tool.proj_face = true;
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        crate::gui::sketching::project_clicked_edge(&mut app.sketch_ctx(), si, rect, egui::pos2(450.0, 350.0));
        assert_eq!(app.project.sketches[si].projected_entities().len(), 4, "GUARD: the outline of the top face projected");
        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_of!(app), &mut app.viewing.mode_3d, 6); // the tool put down
        si
    }

    /// The points of the projection, where they stand.
    fn projected_points(app: &App, si: usize) -> Vec<(u64, (f64, f64))> {
        let s = &app.project.sketches[si];
        let mut ids: Vec<u64> = s.projected_points().into_iter().collect();
        ids.sort_unstable();
        ids.into_iter().filter_map(|id| s.points.iter().find(|q| q.id == id).map(|q| (id, (q.x, q.y)))).collect()
    }

    /// Whether points `a` and `b` are one point, or held as one by a coincidence.
    fn tied(app: &App, si: usize, a: u64, b: u64) -> bool {
        a == b || app.project.sketches[si].constraints.iter().any(|c| matches!(*c, Constraint::Coincident { a: x, b: y } if (x == a && y == b) || (x == b && y == a)))
    }

    #[test]
    fn a_line_drawn_from_a_corner_of_a_projection_is_tied_to_it() {
        let mut app = App::default();
        let si = a_projected_outline(&mut app);
        let (corner, at) = projected_points(&app, si)[0];
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(1).click2d(at.0, at.1).click2d(at.0 + 7.0, at.1 + 11.0).key(egui::Key::Escape).key(egui::Key::Escape);
        let s = &hand.app.project.sketches[si];
        let drawn = s.entities.iter().rev().find_map(|e| match e.kind {
            EntityKind::Line { a, .. } if !s.projected_entities().contains(&e.id) => Some(a),
            _ => None,
        });
        let start = drawn.expect("a line was drawn");
        assert!(tied(hand.app, si, start, corner), "a line drawn from the corner {at:?} of the projection is not tied to it");
    }
    #[test]
    fn a_line_drawn_from_the_middle_of_a_projected_side_or_along_it_is_tied_there() {
        let mut failures = Vec::new();
        for (how, t) in [("the middle", 0.5), ("a quarter along", 0.25)] {
            let mut app = App::default();
            let si = a_projected_outline(&mut app);
            let s = &app.project.sketches[si];
            let side = s.entities.iter().find(|e| s.projected_entities().contains(&e.id)).expect("a projected side");
            let EntityKind::Line { a, b } = side.kind else { panic!("the side is a line") };
            let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("the end");
            let ((ax, ay), (bx, by)) = (at(a), at(b));
            let p = (ax + (bx - ax) * t, ay + (by - ay) * t);
            let mut hand = Hand::new(&mut app);
            hand.sk_tool(1).click2d(p.0, p.1).click2d(p.0 + 9.0, p.1 + 13.0).key(egui::Key::Escape).key(egui::Key::Escape);
            let s = &hand.app.project.sketches[si];
            let start = s.entities.iter().rev().find_map(|e| match e.kind {
                EntityKind::Line { a, .. } if !s.projected_entities().contains(&e.id) => Some(a),
                _ => None,
            });
            let Some(start) = start else {
                failures.push(format!("{how}: no line drawn"));
                continue;
            };
            let held = s.constraints.iter().any(|c| match *c {
                Constraint::Midpoint { p, a: x, b: y } => p == start && [x, y] == [a, b] || p == start && [y, x] == [a, b],
                Constraint::PointOnLine { p, a: x, b: y } => p == start && ([x, y] == [a, b] || [y, x] == [a, b]),
                _ => false,
            });
            if !held {
                failures.push(format!("{how} of a projected side: the line drawn from it is not tied to the side"));
            }
        }
        assert!(failures.is_empty(), "snapping to a projection:\n{}", failures.join("\n"));
    }

    /// The middle of the first projected side and the side.
    fn a_projected_side(app: &App, si: usize) -> (u64, (f64, f64)) {
        let s = &app.project.sketches[si];
        let side = s.entities.iter().find(|e| s.projected_entities().contains(&e.id)).expect("a projected side");
        let EntityKind::Line { a, b } = side.kind else { panic!("the side is a line") };
        let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("the end");
        let ((ax, ay), (bx, by)) = (at(a), at(b));
        (side.id, ((ax + bx) / 2.0, (ay + by) / 2.0))
    }

    #[test]
    fn a_projection_made_ordinary_by_its_menu_stays_and_drags() {
        let mut app = App::default();
        let si = a_projected_outline(&mut app);
        let (side, mid) = a_projected_side(&app, si);
        let curves = app.project.sketches[si].entities.len();
        let mut hand = Hand::new(&mut app);
        assert!(hand.select2d(&[(1, side)]), "the projected side could not be picked");
        hand.right_click2d(mid.0, mid.1);
        assert!(hand.press_word(&crate::i18n::tr("sk-make-ordinary"), hand.on_screen2d(mid)), "no Make ordinary geometry in the menu of a projected side");
        let s = &hand.app.project.sketches[si];
        assert!(s.projections.is_empty(), "the projection is still tied to the body");
        assert_eq!(s.entities.len(), curves, "making it ordinary changed the curves");
        // its corner drags now, as a corner drawn by hand
        let (corner, at) = projected_points_of(&hand, si, side);
        hand.drag2d(at, (at.0 + 5.0, at.1 + 3.0));
        let moved = hand.app.project.sketches[si].points.iter().find(|q| q.id == corner).map(|q| (q.x - at.0).hypot(q.y - at.1));
        assert!(moved.is_some_and(|d| d > 1.0), "the corner of the projection made ordinary did not drag: it went {moved:?}");
    }

    /// The first end of `side` and where it stands.
    fn projected_points_of(hand: &Hand, si: usize, side: u64) -> (u64, (f64, f64)) {
        let s = &hand.app.project.sketches[si];
        let Some(EntityKind::Line { a, .. }) = s.entities.iter().find(|e| e.id == side).map(|e| e.kind) else { panic!("the side") };
        (a, s.points.iter().find(|q| q.id == a).map(|q| (q.x, q.y)).expect("the end"))
    }

    /// The construction toggle of the menu of the right button, over the side at `mid`.
    fn toggle_construction(hand: &mut Hand, side: u64, mid: (f64, f64)) {
        assert!(hand.select2d(&[(1, side)]), "the projected side could not be picked");
        hand.right_click2d(mid.0, mid.1);
        assert!(hand.press_word(&crate::i18n::tr("sk-construction-toggle"), hand.on_screen2d(mid)), "no construction toggle in the menu of a projected side");
    }

    #[test]
    fn a_projected_side_turns_construction_drawn_pink_dashed_and_thin_and_back() {
        let mut app = App::default();
        let si = a_projected_outline(&mut app);
        let (side, mid) = a_projected_side(&app, si);
        let mut hand = Hand::new(&mut app);
        let pink = hand.app.scheme.pal.sketch_driven_construction();
        toggle_construction(&mut hand, side, mid);
        let construction = |hand: &Hand| hand.app.project.sketches[si].entities.iter().any(|e| e.id == side && e.construction);
        assert!(construction(&hand), "the projected side did not turn construction");
        assert!(!hand.app.project.sketches[si].projections.is_empty(), "turning it construction let the projection go");
        hand.key(egui::Key::Escape).hover2d(mid.0 + 30.0, mid.1 + 30.0);
        let along = |hand: &Hand| hand.segments_in(pink).iter().filter(|seg| seg.iter().any(|p| p.distance(hand.on_screen2d(mid)) < 40.0)).count();
        assert!(along(&hand) >= 2, "the projected construction side is not drawn as pink dashes: {} near its middle", along(&hand));
        toggle_construction(&mut hand, side, mid);
        hand.key(egui::Key::Escape).hover2d(mid.0 + 30.0, mid.1 + 30.0);
        assert!(!construction(&hand) && along(&hand) == 0, "turned back, the side is still construction {} or pink {}", construction(&hand), along(&hand));
    }

    /// HOW A PIECE OF THE PROJECTION IS CUT.
    #[derive(Clone, Copy, Debug)]
    enum Cut {
        Delete,
        Break,
        Trim,
    }

    #[test]
    fn a_projection_cut_leaves_ordinary_lines_held_by_what_they_plainly_are() {
        let mut failures = Vec::new();
        for cut in [Cut::Delete, Cut::Break, Cut::Trim] {
            let mut app = App::default();
            let si = a_projected_outline(&mut app);
            let (side, mid) = a_projected_side(&app, si);
            let outline: Vec<u64> = app.project.sketches[si].projected_entities().into_iter().collect();
            let mut hand = Hand::new(&mut app);
            match cut {
                Cut::Delete => {
                    assert!(hand.select2d(&[(1, side)]), "the side could not be picked");
                    hand.key(egui::Key::Delete);
                }
                Cut::Break => {
                    assert!(hand.press_hint(&crate::i18n::tr("tb-break-hint")), "no Break button");
                    hand.click2d(mid.0, mid.1).key(egui::Key::Escape);
                }
                Cut::Trim => {
                    // a line of its own across the side, a little off its middle, and the shorter piece of the side trimmed
                    let s = &hand.app.project.sketches[si];
                    let Some(EntityKind::Line { a, b }) = s.entities.iter().find(|e| e.id == side).map(|e| e.kind) else { panic!("the side") };
                    let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("the end");
                    let ((ax, ay), (bx, by)) = (at(a), at(b));
                    let cross = (ax + (bx - ax) * 0.25, ay + (by - ay) * 0.25);
                    let (nx, ny) = (-(by - ay), bx - ax);
                    let n = nx.hypot(ny);
                    hand.sk_tool(1).click2d(cross.0 + nx / n * 6.0, cross.1 + ny / n * 6.0).click2d(cross.0 - nx / n * 6.0, cross.1 - ny / n * 6.0).key(egui::Key::Escape).key(egui::Key::Escape);
                    assert!(hand.press_hint(&crate::i18n::tr("tb-trim-hint")), "no Trim button");
                    hand.click2d(ax + (bx - ax) * 0.1, ay + (by - ay) * 0.1).key(egui::Key::Escape);
                }
            }
            let s = &hand.app.project.sketches[si];
            if !s.projections.is_empty() {
                failures.push(format!("{cut:?}: what is left is still a projection"));
                continue;
            }
            let left: Vec<u64> = outline.iter().copied().filter(|e| s.entities.iter().any(|x| x.id == *e)).collect();
            let held = s.constraints.iter().filter(|c| matches!(c, Constraint::Horizontal { .. } | Constraint::Vertical { .. })).count();
            if left.is_empty() || held < left.len() {
                failures.push(format!("{cut:?}: {} sides of the outline left, {held} of them held level or upright", left.len()));
            }
        }
        assert!(failures.is_empty(), "a projection cut:\n{}", failures.join("\n"));
    }
}
