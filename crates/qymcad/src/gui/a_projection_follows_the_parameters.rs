//! A PROJECTION FOLLOWS THE PARAMETERS OF THE BODY IT CAME FROM, on the path of the report: a square from its centre
//! sized by the parameters w and h, extruded l; on its top face a sketch with the outline of the face projected, made
//! construction, and a square inside it 5 off each side; that extruded 10. The parameters changed - the projection
//! follows the face, and the square tied to it follows the projection.
//!
//! Reported behaviour: "tie a point to the projection by dimensions or constraints, then change the body above in the
//! tree: does not work at all" - w and h made 30 and 20, the outline of the projection and the square inside stood at
//! 60 x 60.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::geom::Point2;
    use qymcad_core::model::{Constraint, Param};

    /// The box around the points `ids` of sketch `si`: (min x, min y, max x, max y).
    fn bounds(app: &App, si: usize, ids: &[u64]) -> [f64; 4] {
        let s = &app.project.sketches[si];
        ids.iter().filter_map(|id| s.points.iter().find(|q| q.id == *id)).fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q.x), b[1].min(q.y), b[2].max(q.x), b[3].max(q.y)])
    }

    /// The parameters w 60, h 60, l 50 and the base: a square from its centre, its sides held by w and h, extruded by
    /// hand 50. Answers the body.
    fn a_base_sized_by_parameters(app: &mut App) -> u64 {
        // the parameters and the base: a square 60 x 60 from its centre, its sides held by w and h, extruded 50
        for (name, v) in [("w", 60.0), ("h", 60.0), ("l", 50.0)] {
            app.project.parameters.push(Param { name: name.into(), expr: v.to_string(), value: v });
        }
        let part = app.project.add_part("Part");
        app.enter_component(part);
        let si1 = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        let ids = app.project.add_rect_from_centre(si1, Point2::new(0.0, 0.0), Point2::new(30.0, 30.0), qymcad_core::feature::Purpose::Real);
        assert!(app.project.dimension_rect(si1, ids[0]), "GUARD: the width and the height laid");
        // the dimension along a side running in X is the width, w; the other the height, h
        let level: Vec<bool> = app.project.sketches[si1]
            .constraints
            .iter()
            .map(|c| match c {
                Constraint::Distance { a, b, .. } => {
                    let y = |id: &u64| app.project.sketches[si1].points.iter().find(|q| q.id == *id).map(|q| q.y).unwrap_or(0.0);
                    (y(a) - y(b)).abs() < 1e-6
                }
                _ => false,
            })
            .collect();
        for (c, level) in app.project.sketches[si1].constraints.iter_mut().zip(level) {
            if let Constraint::Distance { expr, .. } = c {
                *expr = if level { "w".into() } else { "h".into() };
            }
        }
        app.project.eval_parameters();
        app.project.solve_sketch(si1);
        app.finish_sketch_edit();
        app.chosen.sel = Sel::Sketch(si1);
        Hand::new(app).look_at([0.0, 0.0, 25.0], 6.0).tool(1).set("height", 50.0).enter();
        app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the base")
    }

    /// The top face of `body` as a face to seat a sketch on.
    fn top_of(app: &App, body: u64) -> qymcad_core::feature::FaceKey {
        let top = app.project.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).cloned().expect("the top");
        qymcad_core::feature::FaceKey { index: 0, centroid: [top.centroid.x, top.centroid.y, top.centroid.z], normal: top.normal, id: top.id }
    }

    /// w and h changed as the window of the parameters changes them, in a live window: a frame drawn, the edit made, the
    /// rebuild run in the background and waited for.
    fn parameters_changed(app: &mut App, w: f64, h: f64) {
        for (name, v) in [("w", w), ("h", h)] {
            if let Some(p) = app.project.parameters.iter_mut().find(|p| p.name == name) {
                p.expr = v.to_string();
            }
        }
        let mut hand = Hand::new(app);
        hand.frame(Vec::new());
        let mut asks = Vec::new();
        crate::gui::panels_windows::apply_param_edit(&mut hand.app.win_ctx(&mut asks));
        assert!(asks.iter().any(|a| matches!(a, qymcad_ui_state::WinAsk::RegenerateAll)), "GUARD: the parameters ask for a rebuild");
        qymcad_ui_state::regenerate_all(&mut hand.app.rebuild_ctx());
        hand.close_window();
    }

    #[test]
    fn a_projection_and_what_is_tied_to_it_follow_the_parameters() {
        let mut app = App::default();
        let body = a_base_sized_by_parameters(&mut app);
        // on its top face: the outline projected, made construction, a square inside 5 off each side
        let key = top_of(&app, body);
        let si2 = app.create_sketch_on(qymcad_core::feature::SketchPlane::Face(body, key));
        let mut hand = Hand::new(&mut app);
        assert!(hand.press_hint(&crate::i18n::tr("tb-project-body-hint")) && hand.press_word(&crate::i18n::tr("opt-face-outline"), egui::pos2(0.0, 0.0)), "the projection tool and its Face outline");
        hand.click2d(0.0, 0.0).key(egui::Key::Escape);
        let outline: Vec<u64> = hand.app.project.sketches[si2].projected_points().into_iter().collect();
        let curves: Vec<u64> = hand.app.project.sketches[si2].projected_entities().into_iter().collect();
        assert_eq!(curves.len(), 4, "GUARD: the outline of the top face projected");
        hand.app.project.toggle_construction(si2, &curves);
        let b = bounds(hand.app, si2, &outline);
        let inner = hand.app.project.add_rect_entity(si2, b[0] + 5.0, b[1] + 5.0, b[2] - 5.0, b[3] - 5.0, qymcad_core::feature::Purpose::Real);
        let _ = inner;
        // each corner of the inner square 5 off the corner of the outline nearest it, along X and along Y
        let corners: Vec<u64> = {
            let s = &hand.app.project.sketches[si2];
            s.rects.last().map(|r| r.corners.to_vec()).expect("the inner square")
        };
        for c in corners.clone() {
            let s = &hand.app.project.sketches[si2];
            let at = s.points.iter().find(|q| q.id == c).map(|q| (q.x, q.y)).expect("the corner");
            let near = *outline
                .iter()
                .min_by(|x, y| {
                    let d = |id: &u64| s.points.iter().find(|q| q.id == *id).map(|q| (q.x - at.0).hypot(q.y - at.1)).unwrap_or(f64::MAX);
                    d(x).total_cmp(&d(y))
                })
                .expect("an outline corner");
            let n = s.points.iter().find(|q| q.id == near).map(|q| (q.x, q.y)).expect("it");
            for (axis, d) in [(1u8, (at.0 - n.0).abs()), (2u8, (at.1 - n.1).abs())] {
                hand.app.project.add_constraint_if_independent(si2, Constraint::Distance { a: near, b: c, d, off: 3.0, expr: String::new(), driven: false, axis, at: None });
            }
        }
        hand.app.project.solve_sketch(si2);
        hand.app.finish_sketch_edit();
        hand.app.chosen.sel = Sel::Sketch(si2);
        Hand::new(&mut app).look_at([0.0, 0.0, 55.0], 6.0).tool(1).set("height", 10.0).enter();
        let before = bounds(&app, si2, &outline);

        parameters_changed(&mut app, 30.0, 20.0);

        let si2 = app.project.sketches.iter().position(|s| s.projections.iter().any(|p| !p.entities.is_empty())).or(Some(si2)).expect("the sketch");
        let now = bounds(&app, si2, &outline);
        let inner_now = bounds(&app, si2, &corners);
        let (w, h) = (now[2] - now[0], now[3] - now[1]);
        let (iw, ih) = (inner_now[2] - inner_now[0], inner_now[3] - inner_now[1]);
        assert!(
            (w - 30.0).abs() < 1e-3 && (h - 20.0).abs() < 1e-3 && (iw - 20.0).abs() < 1e-3 && (ih - 10.0).abs() < 1e-3,
            "w 30 and h 20: the projected outline is {w:.3} x {h:.3} (it was {:.3} x {:.3}), the square tied inside it {iw:.3} x {ih:.3}; still projected {}",
            before[2] - before[0],
            before[3] - before[1],
            !app.project.sketches[si2].projections.is_empty()
        );
    }

    #[test]
    fn a_line_drawn_from_a_grey_corner_of_the_face_follows_the_parameters() {
        let mut app = App::default();
        let body = a_base_sized_by_parameters(&mut app);
        let key = top_of(&app, body);
        let si2 = app.create_sketch_on(qymcad_core::feature::SketchPlane::Face(body, key));
        // a line from the corner (-30, -30) of the face, an edge of the body under the sketch, into the face
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(1).click2d(-30.0, -30.0).click2d(-10.0, -20.0).key(egui::Key::Escape).key(egui::Key::Escape);
        let s = &hand.app.project.sketches[si2];
        let start = s.entities.iter().find_map(|e| match e.kind {
            qymcad_core::model::EntityKind::Line { a, .. } if !e.construction => Some(a),
            _ => None,
        });
        let start = start.expect("the line drawn");
        assert!(!s.projections.is_empty(), "the edge the line starts on was not taken into the sketch");
        hand.app.finish_sketch_edit();
        parameters_changed(&mut app, 30.0, 20.0);
        let at = app.project.sketches[si2].points.iter().find(|q| q.id == start).map(|q| (q.x, q.y));
        assert!(at.is_some_and(|p| (p.0 + 15.0).abs() < 1e-3 && (p.1 + 10.0).abs() < 1e-3), "w 30 and h 20: the line drawn from the corner of the face starts at {at:?}, the corner is at (-15, -10)");
    }
}
