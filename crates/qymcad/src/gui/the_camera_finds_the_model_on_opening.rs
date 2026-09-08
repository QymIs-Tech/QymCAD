//! ON OPENING, THE CAMERA FINDS THE MODEL.
//!
//! Reported behaviour: "sometimes on starting the program, if there is a finished project already, the
//! camera flies terribly far away, or an empty 3D viewport opens. I get lost myself at moments like that,
//! never mind the users."
//!
//! WHERE IT WENT WRONG. `fit3d` measures the model to aim the camera, and it measures it IN THE WRONG
//! SPACE. A body's mesh lives in the coordinates of the component that owns it, and where that component
//! actually stands comes from `body_world_transform` - which the fit never asked for. So in an assembly
//! whose parts are placed apart, the camera was aimed at the local zero of the meshes while the parts were
//! drawn somewhere else entirely. Far enough apart, and there is nothing on screen at all.
//!
//! The same for the sketches: `project.contours` holds flat 2D coordinates of a sketch on ITS OWN plane,
//! and they were fed in as world X and Y with z = 0 - a sketch on the front plane, or on a face of a part
//! standing away from the origin, dragged the measurement to a place where nothing is.
//!
//! AND WHAT IS NOT DRAWN MUST NOT BE MEASURED: bodies consumed by later features and bodies hidden by a
//! tick box are not on screen, and framing the view around them aims it at nothing.
//!
//! MEASURED BY PROJECTING THE MODEL, not by comparing the camera's numbers: what matters is whether a
//! person sees the part, and that is the projection.
#[cfg(test)]
mod tests {
    use crate::gui::App;
    use qymcad_core::model::Id;

    const RECT: egui::Rect = egui::Rect { min: egui::Pos2 { x: 0.0, y: 0.0 }, max: egui::Pos2 { x: 900.0, y: 700.0 } };

    /// How much of the viewport a correctly framed model spans, at least.
    ///
    /// `fit3d` aims for 0.55 of the shorter side, and an isometric view foreshortens that: a right answer
    /// measures about 30 percent here. A wrong one measures single digits - 5 percent for the case below.
    /// The bar sits between the two rather than next to either.
    const FILLED: f32 = 0.2;

    /// An assembly of two parts, the second standing `away` millimetres off along X.
    fn two_parts_apart(away: f64) -> (App, Vec<Id>) {
        let mut app = App::default();
        app.set.gpu_viewport = false;
        let mut bodies = Vec::new();
        for i in 0..2 {
            let cid = app.project.add_part(format!("P{i}"));
            app.enter_component(cid);
            let c = app.project.add_cylinder(10.0, 20.0);
            let b = app.project.finish_base_body(c, 1);
            app.exit_context();
            if i == 1 {
                app.project.set_component_transform(cid, [1.0, 0.0, 0.0, away, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
            }
            bodies.push(b);
        }
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        app.viewing.mode_3d = true;
        app.viewing.cam.init = false; // as on opening a file
        (app, bodies)
    }

    /// The share of a body's world bounding box corners that land inside the viewport.
    fn on_screen(app: &App, body: Id) -> f64 {
        let Some(b) = app.project.bodies.iter().find(|b| b.id == body) else { return 0.0 };
        let wt = app.project.body_world_transform(body);
        let basis = app.viewing.cam.basis();
        let sc = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: RECT, basis: &basis };
        let (mut mn, mut mx) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for v in &b.mesh.verts {
            let p = qymcad_core::feature::apply12(&wt, [v.x as f64, v.y as f64, v.z as f64]);
            for a in 0..3 {
                mn[a] = mn[a].min(p[a]);
                mx[a] = mx[a].max(p[a]);
            }
        }
        if !mn[0].is_finite() {
            return 0.0;
        }
        let mut inside = 0;
        for i in 0..8 {
            let c = [if i & 1 == 0 { mn[0] } else { mx[0] }, if i & 2 == 0 { mn[1] } else { mx[1] }, if i & 4 == 0 { mn[2] } else { mx[2] }];
            if RECT.contains(sc.at(c).0) {
                inside += 1;
            }
        }
        inside as f64 / 8.0
    }

    /// BOTH PARTS OF AN ASSEMBLY ARE ON SCREEN after the fit, however far apart they stand.
    #[test]
    fn a_part_standing_away_from_the_origin_is_still_framed() {
        for away in [200.0, 2000.0] {
            let (mut app, bodies) = two_parts_apart(away);
            crate::gui::fit3d(&mut app.viewing.cam, &app.project, RECT);
            let missing: Vec<String> = bodies.iter().filter(|b| on_screen(&app, **b) < 1.0).map(|b| format!("body {b} shows {:.0}% of itself", on_screen(&app, *b) * 100.0)).collect();
            assert!(
                missing.is_empty(),
                "the parts stand {away} mm apart and the camera was aimed by the LOCAL coordinates of the meshes, so what is drawn is not where it was aimed: {}",
                missing.join(", ")
            );
        }
    }

    /// What share of the viewport a body's projection spans.
    fn filled_share(app: &App, body: Id) -> f32 {
        let Some(b) = app.project.bodies.iter().find(|b| b.id == body) else { return 0.0 };
        let wt = app.project.body_world_transform(body);
        let basis = app.viewing.cam.basis();
        let sc = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: RECT, basis: &basis };
        let (mut mn, mut mx) = (egui::pos2(f32::MAX, f32::MAX), egui::pos2(f32::MIN, f32::MIN));
        for v in &b.mesh.verts {
            let p = sc.at(qymcad_core::feature::apply12(&wt, [v.x as f64, v.y as f64, v.z as f64])).0;
            mn = egui::pos2(mn.x.min(p.x), mn.y.min(p.y));
            mx = egui::pos2(mx.x.max(p.x), mx.y.max(p.y));
        }
        ((mx.x - mn.x) / RECT.width()).max((mx.y - mn.y) / RECT.height())
    }

    /// AND THE VIEW IS NOT ZOOMED OUT INTO NOTHING: the model fills a fair share of the viewport.
    ///
    /// The other side of the same bar. Aiming right and then framing the model as a speck is the "flew
    /// terribly far away" half of the report, and a check that only asked "is it on screen" would pass over
    /// it.
    #[test]
    fn the_model_is_not_a_speck_in_the_middle() {
        let (mut app, bodies) = two_parts_apart(200.0);
        crate::gui::fit3d(&mut app.viewing.cam, &app.project, RECT);
        let basis = app.viewing.cam.basis();
        let sc = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: RECT, basis: &basis };
        let (mut mn, mut mx) = (egui::pos2(f32::MAX, f32::MAX), egui::pos2(f32::MIN, f32::MIN));
        for body in &bodies {
            let wt = app.project.body_world_transform(*body);
            for v in &app.project.bodies.iter().find(|b| b.id == *body).expect("the body").mesh.verts {
                let p = sc.at(qymcad_core::feature::apply12(&wt, [v.x as f64, v.y as f64, v.z as f64])).0;
                mn = egui::pos2(mn.x.min(p.x), mn.y.min(p.y));
                mx = egui::pos2(mx.x.max(p.x), mx.y.max(p.y));
            }
        }
        let share = ((mx.x - mn.x) / RECT.width()).max((mx.y - mn.y) / RECT.height());
        assert!(share > FILLED, "the model occupies {:.0}% of the viewport - it is a speck in the middle of an empty screen", share * 100.0);
    }

    /// A SKETCH TRAVELS WITH THE PART THAT OWNS IT.
    ///
    /// THE FIRST EDITION OF THIS CHECK PROVED NOTHING. It put a sketch on the front plane with y running
    /// 280 to 320 in its own coordinates and demanded that the body stay large - but read correctly, that
    /// sketch really IS 300 mm away from the body, and a view holding both of them really does make the
    /// body small. The check was calling a right answer wrong.
    ///
    /// What separates a right reading from a wrong one is OWNERSHIP: the sketch belongs to a part standing
    /// 200 mm off, and its flat coordinates are small. Read right, the sketch sits on top of its part and
    /// the two frame together. Read as bare world X and Y, the sketch stays at the origin while the part
    /// is out at 200 mm, and the view stretches over a gap with nothing in it.
    #[test]
    fn a_sketch_is_framed_where_its_own_part_stands() {
        let mut app = App::default();
        app.set.gpu_viewport = false;
        let cid = app.project.add_part("P");
        app.enter_component(cid);
        let c = app.project.add_cylinder(10.0, 20.0);
        let body = app.project.finish_base_body(c, 1);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 30.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.exit_context();
        app.project.set_component_transform(cid, [1.0, 0.0, 0.0, 200.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        app.viewing.mode_3d = true;

        crate::gui::fit3d(&mut app.viewing.cam, &app.project, RECT);

        // BOTH HALVES, because either alone lets an answer through. `filled_share` measures the SPAN of the
        // projection and says nothing about where it lands, so a part aimed at the wrong place can still
        // span plenty; and being on screen says nothing about being large enough to see.
        assert!(on_screen(&app, body) >= 1.0, "the part is not on screen at all: the view was aimed by coordinates that are not where anything is drawn");
        let share = filled_share(&app, body);
        assert!(
            share > FILLED,
            "the sketch belongs to a part standing 200 mm off and its flat coordinates were read as bare world X and Y, so the view stretched over the gap between the two and left the part filling {:.0}% of it",
            share * 100.0
        );
    }

    /// ONE BAD VERTEX DOES NOT COST THE WHOLE VIEW.
    ///
    /// A GUARD, NOT A FIX, AND SAID SO PLAINLY: this one was green before anything was changed. Written to
    /// catch the other way an empty viewport happens - a single vertex that is not a number poisoning both
    /// bounds, the fit finding them not finite and refusing, `cam.init` staying false and the refusal
    /// repeating every frame - it turned out the code already survives that, because the bounds are widened
    /// by COMPARISON and every comparison against a NaN is false. An explicit finite check was written,
    /// measured to change nothing, and taken out again.
    ///
    /// The check stays: the property is real, it is held by accident of how the bounds are written, and the
    /// obvious rewrite (`min`/`max`, or a running `fold`) breaks it silently.
    #[test]
    fn a_vertex_that_is_not_a_number_does_not_empty_the_viewport() {
        let mut app = App::default();
        app.set.gpu_viewport = false;
        let cid = app.project.add_part("P");
        app.enter_component(cid);
        let c = app.project.add_cylinder(10.0, 20.0);
        let body = app.project.finish_base_body(c, 1);
        app.exit_context();
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        app.viewing.mode_3d = true;
        let bi = app.project.bodies.iter().position(|b| b.id == body).expect("the body");
        app.project.bodies[bi].mesh.verts.push(qymcad_core::geom::Point3 { x: f64::NAN, y: 0.0, z: 0.0 });

        crate::gui::fit3d(&mut app.viewing.cam, &app.project, RECT);

        assert!(app.viewing.cam.init, "the fit refused because of one bad vertex, and a refusal leaves the camera where it was - and repeats every frame");
        assert!(filled_share(&app, body) > FILLED, "the body is on screen but a speck: {:.0}%", filled_share(&app, body) * 100.0);
    }
}
