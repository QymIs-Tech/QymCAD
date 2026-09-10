//! A DIMENSION FROM A CIRCLE TO A COORDINATE AXIS.
//!
//! Reported behaviour: "I placed a circle, I am trying to put a dimension from its centre to the X or Y
//! axis, and the axis WILL NOT be picked - a crooked dimension is placed instead."
//!
//! The distance from a hole to an edge or to a datum is the commonest dimension there is in a drawing.
//! Placed against the wrong reference it does not merely look wrong: the value is measured to something
//! else, and the part comes out of the machine to that other number.
//!
//! WHAT IT TURNED OUT TO BE. Measured on the reporter's own file: the sketch ORIGIN had moved.
//!
//! ```text
//! points:  id: 4, x: -23.454944474040893, y: 18.70469405576201   <- the origin
//!          id: 10, x: 1.0, y: 0.0                                 <- the X axis guide
//! origin: 4,  axis_pts: (10, 11)
//! DistancePL(p: 7, a: 4, b: 10, d: 55.27876336781576)
//! ```
//!
//! An axis is the infinite line through the origin and its guide point. With the origin at
//! (-23.45, 18.70) the "X axis" is a DIAGONAL, and a distance to it is honestly measured - to the wrong
//! line. Both numbers on the screen, 55.3 and 55.0, are exactly that.
//!
//! HOW IT MOVED. `sketch_hit` leaves the origin pickable on purpose, so that a point can be made
//! coincident with it - and a pickable point is a draggable one. `Constraint::Fixed` pins a point to
//! where it IS, not to zero, so one drag locks the reference frame at its new place for good.
//!
//! The origin and the axis guides are the frame of reference, not geometry. They do not move.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// A sketch being edited, with one circle away from both axes.
    fn sketch_with_a_circle() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        // drawn by hand, the way a person does it: the tool, the centre, a point on the rim
        Hand::new(&mut app).sk_tool(3).click2d(40.0, 60.0).click2d(55.0, 60.0);
        assert!(!app.project.sketches[si].entities.is_empty(), "setup: the circle was not drawn");
        (app, si)
    }

    /// The 2D canvas ready for clicking, WITHOUT touching what is in hand.
    ///
    /// `Hand::sk_tool` would do this too, but it also takes a drawing tool - and taking one puts the
    /// dimension tool down, which is exactly what must not happen here.
    fn canvas(app: &mut App) -> Hand<'_> {
        let hand = Hand::new(app);
        hand.app.viewing.mode_3d = false;
        hand.app.viewing.view.scale = 6.0;
        hand.app.viewing.view.center = super::super::Vec2::new(0.0, 0.0);
        hand.app.viewing.view.initialized = true;
        hand
    }

    /// Arm the linear dimension tool - the same door the toolbar button uses.
    fn take_the_dimension_tool(app: &mut App) {
        qymcad_ui_state::set_dim_tool(
            &mut qymcad_ui_state::tools_of!(app),
            &mut app.viewing.mode_3d,
            &app.project,
            app.chosen.sel,
            app.sketch_ses,
            &mut app.status,
            1,
        );
        assert_eq!(app.tools.armed.dim_kind(), 1, "setup: the dimension tool is not in hand");
    }

    /// Does any dimension added after `before` reference one of the sketch's axis points.
    fn goes_to_an_axis(app: &App, si: usize, before: usize) -> bool {
        let axis = &app.project.sketches[si].axis_pts;
        app.project.sketches[si].constraints[before..].iter().any(|c| {
            c.points().iter().any(|p| axis.contains(p))
        })
    }

    /// THE CLICK ON THE AXIS TAKES THE AXIS.
    #[test]
    fn a_dimension_from_a_circle_centre_to_the_x_axis() {
        let (mut app, si) = sketch_with_a_circle();
        take_the_dimension_tool(&mut app);
        let before = app.project.sketches[si].constraints.len();

        // the centre of the circle, then a point ON THE X AXIS well away from any geometry
        canvas(&mut app).click2d(40.0, 60.0);
        canvas(&mut app).click2d(-30.0, 0.0);

        let added = app.project.sketches[si].constraints.len() - before;
        assert!(added > 0, "two clicks and no dimension at all");
        assert!(
            goes_to_an_axis(&app, si, before),
            "the click on the X axis did not take the axis; what was made instead: {:?}",
            &app.project.sketches[si].constraints[before..]
        );
    }

    /// AND THE SAME FOR THE Y AXIS.
    #[test]
    fn a_dimension_from_a_circle_centre_to_the_y_axis() {
        let (mut app, si) = sketch_with_a_circle();
        take_the_dimension_tool(&mut app);
        let before = app.project.sketches[si].constraints.len();

        canvas(&mut app).click2d(40.0, 60.0);
        canvas(&mut app).click2d(0.0, -30.0);

        let added = app.project.sketches[si].constraints.len() - before;
        assert!(added > 0, "two clicks and no dimension at all");
        assert!(
            goes_to_an_axis(&app, si, before),
            "the click on the Y axis did not take the axis; what was made instead: {:?}",
            &app.project.sketches[si].constraints[before..]
        );
    }

    /// THE FRAME OF REFERENCE DOES NOT MOVE, however hard the drawing on top of it is pulled.
    #[test]
    fn the_frame_anchor_cannot_be_dragged_away_from_zero() {
        let (mut app, si) = sketch_with_a_circle();
        let (a, _) = app.project.ensure_axis(si, 0);
        assert_eq!(a, app.project.sketches[si].frame, "setup: the X axis does not stand on the anchor");
        assert_ne!(a, app.project.sketches[si].origin, "setup: the anchor and the origin are the same point again");

        canvas(&mut app).drag2d((0.0, 0.0), (20.0, 10.0));
        app.project.regen_sketch(si);

        let p = app.project.sketches[si].points.iter().find(|p| p.id == a).expect("the anchor is still there");
        assert!(
            p.x.abs() < 1e-9 && p.y.abs() < 1e-9,
            "the anchor was dragged to ({}, {}): the axes are now diagonals and every dimension to them measures the wrong line",
            p.x,
            p.y
        );
    }

    /// A CIRCLE DRAWN AT ZERO IS ITS OWN SHAPE, not the reference frame wearing a circle.
    ///
    /// Reported behaviour: "I cannot move the circle away from the origin", and, put more sharply, "the
    /// circle is not at the origin, but its CENTRE is the origin".
    ///
    /// A click at zero found the origin already standing there and `sketch_point_at` handed it over as
    /// the new circle's centre. From then on the circle WAS the origin: drag it and both axes tilt with
    /// it; pin the frame and the circle cannot move at all.
    ///
    /// 🟡 WHAT THIS DOES NOT ASK. The cause is checked, not the gesture: the drag of a selected shape is
    /// not driven here, because `Hand` has no action for it - see the note in `hand.rs`. A person's own
    /// drag of a circle away from zero is still unchecked.
    #[test]
    fn a_circle_drawn_at_zero_is_not_the_origin_itself() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        // a circle centred exactly at zero, drawn by hand
        Hand::new(&mut app).sk_tool(3).click2d(0.0, 0.0).click2d(25.0, 0.0);
        app.project.ensure_axis(si, 0); // as a dimension to the axis would
        app.project.regen_sketch(si);
        let centre = match app.project.sketches[si].entities.iter().find_map(|e| match e.kind {
            qymcad_core::model::EntityKind::Circle { center, .. } => Some(center),
            _ => None,
        }) {
            Some(c) => c,
            None => panic!("setup: no circle was drawn"),
        };

        // the tool goes down first: with a tool in hand a drag belongs to the tool, not to the selection
        let s = &app.project.sketches[si];
        assert_ne!(centre, s.origin, "the circle adopted the origin as its centre: moving it would drag the reference frame, and pinning the frame would freeze the circle");
        assert_ne!(centre, s.frame, "the circle adopted the frame anchor as its centre");
        assert!(!s.axis_pts.contains(&centre), "the circle adopted an axis guide as its centre");
        assert!(
            !s.constraints.iter().any(|c| matches!(c, qymcad_core::model::Constraint::Fixed { p } if *p == centre)),
            "the centre of the circle carries a Fixed of its own - it could never be moved"
        );
    }

    /// AND THE GUIDES OF THE AXES DO NOT MOVE EITHER.
    ///
    /// They are the second end of each infinite line. Move one and the axis tilts, exactly as it does
    /// when the origin moves.
    #[test]
    fn the_axis_guides_stay_where_the_frame_needs_them() {
        let (mut app, si) = sketch_with_a_circle();
        let (_, gx) = app.project.ensure_axis(si, 0);
        let (_, gy) = app.project.ensure_axis(si, 1);

        canvas(&mut app).drag2d((1.0, 0.0), (25.0, 25.0));
        canvas(&mut app).drag2d((0.0, 1.0), (-25.0, 25.0));
        app.project.regen_sketch(si);

        let at = |id| {
            let p = app.project.sketches[si].points.iter().find(|p| p.id == id).expect("the guide is still there");
            (p.x, p.y)
        };
        assert_eq!(at(gx), (1.0, 0.0), "the guide of the X axis was dragged: the axis is no longer horizontal");
        assert_eq!(at(gy), (0.0, 1.0), "the guide of the Y axis was dragged: the axis is no longer vertical");
    }

    /// A SHAPE HELD BY ITS CONSTRAINTS DOES NOT PRETEND TO HAVE MOVED.
    ///
    /// Reported behaviour: "I took the move tool, moved the circle far away, and the coincidence with the
    /// origin stayed - and stayed GREEN. Reopen the project and the circle is back at the centre; draw
    /// anything and it jumps back too."
    ///
    /// The move shifted the points and never asked the solver. The screen then showed a shape standing
    /// where nothing allows it to stand, and the lie held until the next edit - at which point the jump
    /// back looked like a defect of its own.
    #[test]
    fn a_constrained_circle_does_not_pretend_to_have_moved() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(3).click2d(0.0, 0.0).click2d(20.0, 0.0);
        let (eid, centre) = app.project.sketches[si]
            .entities
            .iter()
            .find_map(|e| match e.kind {
                qymcad_core::model::EntityKind::Circle { center, .. } => Some((e.id, center)),
                _ => None,
            })
            .expect("setup: no circle");

        // the centre is tied to the origin, exactly as drawing it at zero does
        let origin = app.project.ensure_origin(si);
        app.project.sketches[si].constraints.push(qymcad_core::model::Constraint::Coincident { a: centre, b: origin });
        app.project.solve_sketch(si);

        // the move tool carries it away...
        app.project.move_entities(si, &[eid], 40.0, 30.0);
        // ...and the solver has its say, as it now does at the moment of the move
        app.project.solve_sketch(si);

        let p = app.project.sketches[si].points.iter().find(|p| p.id == centre).expect("the centre is still there");
        assert!(
            p.x.abs() < 1e-6 && p.y.abs() < 1e-6,
            "a circle held at the origin by a coincidence ended up at ({}, {}): the drawing shows it somewhere its constraints forbid, and the next edit will snap it back",
            p.x,
            p.y
        );
    }

    /// A CIRCLE DRAWN BY HAND IS DEFINED BY WHAT IS SHOWN ON IT.
    ///
    /// Reported behaviour: "the sketch is not defined although the point is locked and a dimension is
    /// shown on the screen - it was placed automatically while drawing. I edit that dimension on purpose,
    /// and only then does it become a diameter, appear in the panel, and the sketch becomes defined."
    ///
    /// What stood on the drawing was the FIELD waiting for a value, not a constraint. A number that looks
    /// like a dimension and holds nothing is worse than no number: the reason the sketch stays short by
    /// one is then invisible.
    #[test]
    fn a_circle_drawn_by_hand_carries_a_real_diameter() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(3).click2d(0.0, 0.0).click2d(10.0, 0.0);

        let s = &app.project.sketches[si];
        let diameters: Vec<&qymcad_core::model::Constraint> =
            s.constraints.iter().filter(|c| matches!(c, qymcad_core::model::Constraint::Diameter { .. })).collect();
        assert_eq!(
            diameters.len(),
            1,
            "the circle was drawn and no diameter dimension was made: the number on screen holds nothing and the sketch stays under-defined. Constraints: {:?}",
            s.constraints
        );
        match diameters[0] {
            qymcad_core::model::Constraint::Diameter { d, driven, diam, .. } => {
                assert!((*d - 20.0).abs() < 1e-6, "the diameter is {d}, and the circle was drawn with a radius of 10");
                assert!(*diam, "it was made as a radius; a circle is dimensioned by its diameter unless asked otherwise");
                assert!(!*driven, "it was made a reference dimension, so it defines nothing");
            }
            other => panic!("not a diameter: {other:?}"),
        }
    }
}
