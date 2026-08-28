//! THE LIMITS OF A DEGREE OF FREEDOM ARE VISIBLE.
//!
//! The range is held by the solver and stops a drag — that is checked by the acceptance matrix. But
//! until now it existed only in the "min" and "max" fields: a person pulled the handle and ran into
//! an invisible wall without understanding where it came from. Grown-up CAD draws the range of a
//! limited joint.
//!
//! What is checked is the FRAME, not the flags: limits are what a person SEES.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::{AnchorRef, JointKind};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// An assembly of two parts with a slider between their origins. Returns the joint id.
    fn assembly_with_a_slider(app: &mut App) -> Id {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        let a = app.project.mesh_id(0).and_then(|b| app.project.body_owner(b)).expect("part A");
        let b = app.project.mesh_id(1).and_then(|b| app.project.body_owner(b)).expect("part B");
        app.project.set_grounded(a, true);
        let ca = app.project.add_connector(a, AnchorRef::Origin);
        let cb = app.project.add_connector(b, AnchorRef::Origin);
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 6.0;
        app.project.add_joint(ca, cb, JointKind::Slider)
    }

    /// Every segment of the frame of the joint gizmo.
    fn segments(app: &mut App, jid: Id) -> Vec<[egui::Pos2; 2]> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut out = Vec::new();
        // TWO FRAMES: egui areas settle into place on the second pass.
        for _ in 0..2 {
            let res = ctx.run_ui(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                egui::CentralPanel::default().show(c, |ui| {
                    let painter = ui.painter().clone();
                    app.draw_joint_gizmo_for_test(&painter, viewport(), jid);
                });
            });
            out.clear();
            for cs in &res.shapes {
                collect_segments(&cs.shape, &mut out);
            }
        }
        out
    }

    fn collect_segments(s: &egui::epaint::Shape, out: &mut Vec<[egui::Pos2; 2]>) {
        match s {
            egui::epaint::Shape::LineSegment { points, .. } => out.push(*points),
            egui::epaint::Shape::Path(p) => {
                for w in p.points.windows(2) {
                    out.push([w[0], w[1]]);
                }
            }
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| collect_segments(x, out)),
            _ => {}
        }
    }

    /// A LIMITED DEGREE DRAWS ITS RANGE, AND A FREE ONE DOES NOT.
    #[test]
    fn a_limited_degree_draws_its_range() {
        let mut app = App::default();
        let j = assembly_with_a_slider(&mut app);

        let free = segments(&mut app, j).len();
        {
            let jj = app.project.joints.iter_mut().find(|x| x.id == j).expect("the joint");
            jj.limit_min[1] = Some(-10.0);
            jj.limit_max[1] = Some(30.0);
        }
        let limited = segments(&mut app, j).len();
        assert!(
            limited > free,
            "a limited degree does not draw its range: {free} segments without limits, {limited} with them — a person will run into an invisible wall"
        );
    }

    /// THE STOP IS DRAWN WHERE THE PART WILL ACTUALLY STOP.
    ///
    /// The picture must agree with the arithmetic: the mark of the limit stands at the point "origin
    /// of the joint plus the maximum along the axis of travel" — the very axis the solver computes
    /// with. Two pictures of one range would drift apart silently, and the lying one would be exactly
    /// the one a person looks at.
    #[test]
    fn the_stop_is_drawn_where_the_part_will_stop() {
        let mut app = App::default();
        let j = assembly_with_a_slider(&mut app);
        {
            let jj = app.project.joints.iter_mut().find(|x| x.id == j).expect("the joint");
            jj.limit_min[1] = Some(0.0);
            jj.limit_max[1] = Some(30.0);
        }
        let segs = segments(&mut app, j);

        // Where the part will stop: the origin of the joint + 30 along the axis of travel.
        let ctx_id = app.current_ctx_id_for_test();
        let m = app.project.joint_frame(j, ctx_id).expect("the frame of the joint");
        let dir = app.project.joint_slot_axis(j, 1, ctx_id).expect("the axis of travel");
        let stop = [m[3] + dir[0] * 30.0, m[7] + dir[1] * 30.0, m[11] + dir[2] * 30.0];
        let basis = app.cam.basis();
        let want = app.project3(stop, viewport(), &basis).0;

        let near = segs
            .iter()
            .map(|[a, b]| {
                let mid = egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
                (mid - want).length()
            })
            .fold(f32::MAX, f32::min);
        assert!(near < 2.0, "the stop is not where the part will stop: the nearest drawn mark is {near:.1} points from the place {want:?}");
    }

    /// THE AXIS THAT WAS PICKED IS VISIBLE IN 3D.
    ///
    /// A person pointed at an edge — and must see WHAT exactly they pointed at. Without that,
    /// "point at the axis" turns into faith: something was picked, the part moved somehow, and there
    /// is nothing to check whether the one matched the other.
    #[test]
    fn the_axis_you_picked_is_drawn_on_the_geometry_you_picked() {
        use qymcad_core::feature::AnchorRef;

        let mut app = App::default();
        let j = assembly_with_a_slider(&mut app);
        app.cam.target = [30.0, 10.0, 5.0];
        app.refresh_edges();
        app.ensure_brep_for_test();
        let body = app.project.mesh_id(0).expect("body A");
        let e = app.project.regen_edges.get(&body).and_then(|es| es.iter().find(|e| !e.is_circular()).cloned()).expect("a straight edge");

        let plain = segments(&mut app, j).len();
        let ca = app.project.joints.iter().find(|x| x.id == j).map(|x| x.a).expect("anchor A");
        app.project.connectors.iter_mut().find(|c| c.id == ca).expect("the connector").axis_ref = Some(AnchorRef::EdgeMid(body, e.id));
        let shown = segments(&mut app, j);
        assert!(shown.len() > plain, "the axis that was pointed at is not drawn: {plain} segments without it, {} with it", shown.len());

        // AND IT IS DRAWN ON THAT VERY EDGE: the ends of the segment coincide with the ends of the edge.
        let ctx_id = app.current_ctx_id_for_test();
        let wt = app.project.body_display_transform(body, ctx_id);
        let basis = app.cam.basis();
        let pt = |v: [f64; 3]| app.project3(qymcad_core::feature::apply12(&wt, v), viewport(), &basis).0;
        let (a, b) = (pt(e.a), pt(e.b));
        let hit = shown.iter().any(|[p, q]| ((*p - a).length() < 2.0 && (*q - b).length() < 2.0) || ((*p - b).length() < 2.0 && (*q - a).length() < 2.0));
        assert!(hit, "the axis is not drawn on the edge that was pointed at: the expected segment was {a:?}-{b:?}");
    }
}
