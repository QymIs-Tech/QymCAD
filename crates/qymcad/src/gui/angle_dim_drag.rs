//! THE LABEL OF AN ANGULAR DIMENSION IS DRAGGED WITH THE MOUSE, as a length's and a radius's is.
//!
//! Reported behaviour: an angle is placed, and then it cannot be dragged to where it reads well, as a radius or a
//! length can.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::model::Constraint;

    /// A sketch under edit holding two lines and an angle between them: from one vertex (`shared`), or between two
    /// lines that meet nowhere on themselves.
    fn sketch_with_an_angle(shared: bool) -> (App, usize, usize) {
        let mut app = App::default();
        let part = app.project.add_part("Plate");
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        qymcad_ui_state::begin_edit(&mut app.disk.edits, &app.project, "setup".to_string());
        let real = qymcad_core::feature::Purpose::Real;
        let (l1, l2) = if shared { ((0.0, 0.0, 40.0, 0.0), (0.0, 0.0, 30.0, 30.0)) } else { ((10.0, 0.0, 50.0, 0.0), (10.0, 20.0, 40.0, 50.0)) };
        app.project.add_line_entity(si, l1.0, l1.1, l1.2, l1.3, real);
        app.project.add_line_entity(si, l2.0, l2.1, l2.2, l2.3, real);
        let pt = |app: &App, x: f64, y: f64| app.project.sketches[si].points.iter().find(|p| (p.x - x).abs() < 1e-6 && (p.y - y).abs() < 1e-6).map(|p| p.id).expect("a point of the lines");
        let c = if shared {
            Constraint::Angle {
                dimension: Default::default(),
                a: pt(&app, 40.0, 0.0),
                b: pt(&app, 0.0, 0.0),
                c: pt(&app, 30.0, 30.0),
                deg: 45.0,
                expr: String::new(),
                driven: false,
                off: 0.0,
                at: None,
            }
        } else {
            Constraint::AngleLines {
                dimension: Default::default(),
                a: pt(&app, 10.0, 0.0),
                b: pt(&app, 50.0, 0.0),
                c: pt(&app, 10.0, 20.0),
                d: pt(&app, 40.0, 50.0),
                deg: 45.0,
                expr: String::new(),
                driven: false,
                off: 0.0,
                at: None,
            }
        };
        app.project.sketches[si].constraints.push(c);
        let ci = app.project.sketches[si].constraints.len() - 1;
        app.project.solve_sketch(si);
        app.project.regen_sketch(si);
        qymcad_ui_state::commit_edit(&mut app.rebuild_ctx());
        (app, si, ci)
    }

    /// The radius of the arc and the place of the label along it.
    fn placed(app: &App, si: usize, ci: usize) -> (f64, Option<f64>) {
        match app.project.sketches[si].constraints[ci] {
            Constraint::Angle { off, at, .. } | Constraint::AngleLines { off, at, .. } => (off, at),
            ref other => panic!("not an angle: {other:?}"),
        }
    }

    /// Where the label of the angle stands, in sketch units, as the canvas last laid it out.
    fn label_at(app: &App, si: usize, ci: usize) -> (f64, f64) {
        let sh = qymcad_ui_state::Sheet { view: app.viewing.view, rect: app.viewing.view_rect };
        let g = qymcad_ui_state::angle_dim_geom(&app.project, si, ci, &sh, &app.set).expect("the angle has a geometry");
        let w = qymcad_ui_state::to_world(&app.viewing.view, app.viewing.view_rect, g.label);
        (w.x, w.y)
    }

    fn drags_and_keeps(shared: bool) {
        let what = if shared { "an angle at a vertex" } else { "an angle between two lines" };
        let (mut app, si, ci) = sketch_with_an_angle(shared);
        app.disk.edits.saved_key = qymcad_ui_state::edit_key(&app.draw_ctx()); // as if just saved
        let mut h = Hand::new(&mut app);
        h.look2d((20.0, 15.0)).frame(Vec::new());
        let before = placed(h.app, si, ci);
        let from = label_at(h.app, si, ci);
        let centre = if shared { (0.0, 0.0) } else { (-10.0, 0.0) };
        // FARTHER OUT ALONG THE SAME DIRECTION: the arc grows, the value stays
        let out = (centre.0 + (from.0 - centre.0) * 2.0, centre.1 + (from.1 - centre.1) * 2.0);
        h.drag2d(from, out);
        let (r, at) = placed(h.app, si, ci);
        assert!(qymcad_ui_state::is_dirty(&mut h.app.rebuild_ctx()), "{what}: a label dragged is not an unsaved change - closing the window would lose it");
        assert!(r > before.0 + 1.0, "{what}: the label dragged outwards did not take the arc with it: the radius was {}, it is {r}", before.0);
        let deg = match h.app.project.sketches[si].constraints[ci] {
            Constraint::Angle { deg, .. } | Constraint::AngleLines { deg, .. } => deg,
            _ => f64::NAN,
        };
        assert!((deg - 45.0).abs() < 1e-6, "{what}: dragging the label changed the value to {deg}");
        let now = label_at(h.app, si, ci);
        assert!((now.0 - out.0).hypot(now.1 - out.1) < 3.0, "{what}: the label stands at {now:?}, not under the pointer at {out:?}");
        assert!(at.is_some_and(|t| (0.0..=1.0).contains(&t)), "{what}: a label left inside the angle stands at share {at:?}");

        // PAST A SIDE: below the first side the label stands outside the angle and the arc runs on to it
        let below = (centre.0 + 35.0, centre.1 - 12.0);
        let from = label_at(h.app, si, ci);
        h.drag2d(from, below);
        let (_, at) = placed(h.app, si, ci);
        assert!(at.is_some_and(|t| t < 0.0), "{what}: the label led past the first side stands at share {at:?}");
        let sh = qymcad_ui_state::Sheet { view: h.app.viewing.view, rect: h.app.viewing.view_rect };
        let g = qymcad_ui_state::angle_dim_geom(&h.app.project, si, ci, &sh, &h.app.set).expect("the angle has a geometry");
        let within = (g.arc.1 - g.arc.0).abs();
        assert!(within > g.sweep.abs() + 0.05, "{what}: the label past a side, and the arc does not run on to it: the arc spans {within} rad, the angle {}", g.sweep.abs());

        // ONE STEP OF UNDO takes the last drag back
        h.undo();
        let back = placed(h.app, si, ci);
        assert!(back.1.is_some_and(|t| (0.0..=1.0).contains(&t)), "{what}: Ctrl+Z did not take the drag past the side back: the label stands at {back:?}");

        // AND THE PLACE IS KEPT IN THE FILE
        let path = format!("{}/angle-label-{shared}.qcad", std::env::temp_dir().display());
        qymcad_io::save_project(&h.app.project, &path).expect("saved");
        let read = qymcad_io::load_project(&path).expect("read back");
        let kept = match read.sketches[si].constraints[ci] {
            Constraint::Angle { off, at, .. } | Constraint::AngleLines { off, at, .. } => (off, at),
            _ => (f64::NAN, None),
        };
        assert_eq!(kept, back, "{what}: the place of the label was not kept in the file");
    }

    #[test]
    fn the_label_of_an_angle_at_a_vertex_is_dragged() {
        drags_and_keeps(true);
    }

    #[test]
    fn the_label_of_an_angle_between_two_lines_is_dragged() {
        drags_and_keeps(false);
    }
}
