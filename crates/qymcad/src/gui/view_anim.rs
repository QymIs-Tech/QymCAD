//! THE VIEW MOVED IN TIME: fitted to the model, animated to a side, a joint animated.

use super::*;

pub(crate) fn fit3d(cam: &mut Cam3, project: &Project, rect: Rect) {
    let mut mn = [f64::INFINITY; 3];
    let mut mx = [f64::NEG_INFINITY; 3];
    // A POINT THAT IS NOT A NUMBER DOES NOT MOVE THE BOUNDS, and that is why the bounds are widened by
    // COMPARISON rather than by `min`/`max`: every comparison against a NaN is false, so one bad vertex - an
    // import gone wrong, a degenerate face - is simply not measured. Held by a check of its own, because
    // written the obvious way it would poison both bounds, the finite test below would refuse the whole
    // fit, `cam.init` would stay false, and the viewport would open on nothing every frame for ever.
    let mut acc = |p: [f64; 3]| {
        for a in 0..3 {
            if p[a] < mn[a] {
                mn[a] = p[a];
            }
            if p[a] > mx[a] {
                mx[a] = p[a];
            }
        }
    };
    // THE BODIES IN WORLD SPACE, and only the ones that are actually drawn.
    //
    // Reported behaviour: "sometimes on starting the program, if there is a finished project already, the
    // camera flies terribly far away, or an empty 3D viewport opens."
    //
    // A body's mesh lives in the coordinates of the component that owns it; where that component stands
    // comes from `body_world_transform`, and this measurement never asked for it. In an assembly whose
    // parts are placed apart the camera was aimed at the local zero of the meshes while the parts were
    // drawn elsewhere: measured on two parts 200 mm apart, none of the far part's bounding box was on
    // screen at all. Consumed and hidden bodies are left out for the same reason - they are not on screen,
    // and framing the view around them aims it at nothing.
    let consumed = project.consumed_bodies();
    for b in project.bodies.iter().filter(|b| b.visible && !consumed.contains(&b.id)) {
        let wt = project.body_world_transform(b.id);
        for v in &b.mesh.verts {
            acc(qymcad_core::feature::apply12(&wt, [v.x, v.y, v.z]));
        }
    }
    // THE SKETCHES, LIFTED ONTO THEIR OWN PLANES. `project.contours` holds flat 2D coordinates of a sketch
    // on its own plane, and they were fed in as world X and Y with z = 0 - so a sketch on the front plane,
    // or one belonging to a part standing away from the origin, dragged the measurement to a place where
    // nothing is drawn.
    for (si, s) in project.sketches.iter().enumerate() {
        let Some(fr) = project.sketch_frame(si) else { continue };
        let wt = project.sketch_owner(s.id).map(|c| project.world_transform(c)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
        for cid in &s.contour_ids {
            let Some(ci) = project.contour_index(*cid) else { continue };
            for p in &project.contours[ci].points {
                let w = fr.lift(*p);
                acc(qymcad_core::feature::apply12(&wt, [w.x, w.y, w.z]));
            }
        }
    }
    if !mn[0].is_finite() {
        return;
    }
    cam.target = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0, (mn[2] + mx[2]) / 2.0];
    // a model under 1 mm is framed at its own size; only a scene with no extent at all, a single point, takes 1 mm
    let raw = (mx[0] - mn[0]).max(mx[1] - mn[1]).max(mx[2] - mn[2]);
    let ext = (if raw > 1e-9 { raw } else { 1.0 }) as f32;
    cam.scale = (rect.width().min(rect.height()) / ext) * 0.55;
    cam.fit = cam.scale;
    cam.init = true;
}

/// ONE STEP OF THE SWEEP: move the degree of freedom and re-solve the assembly.
///
/// It travels there and back rather than round in a circle: a travel or an angle with limits has no
/// circle to it, and jumping from one end to the other would read as a jerk rather than as a mechanism
/// moving.
pub(crate) fn step_joint_anim(joint_anim: &mut Option<JointAnim>, project: &mut Project, dt: f64) {
    let Some(a) = joint_anim.as_mut() else { return };
    let step = dt / 2.0; // a full pass in one direction takes two seconds
    if a.forward {
        a.t += step;
        if a.t >= 1.0 {
            a.t = 1.0;
            a.forward = false;
        }
    } else {
        a.t -= step;
        if a.t <= 0.0 {
            a.t = 0.0;
            a.forward = true;
        }
    }
    let (joint, slot, v) = (a.joint, a.slot, a.from + (a.to - a.from) * a.t);
    project.set_joint_drive(joint, slot, Some(v));
    project.solve_joints();
}

/// Advance the turning animation. Called every frame; while it runs it asks for a repaint.
pub(crate) fn tick_view_anim(cam: &mut Cam3, view_anim: &mut Option<ViewTurn>, ctx: &egui::Context) {
    let Some(ViewTurn { from, to, since: t0, roll, target }) = view_anim else { return };
    const DUR: f32 = 0.22;
    let t = (t0.elapsed().as_secs_f32() / DUR).clamp(0.0, 1.0);
    // easing in and out: a linear turn looks mechanical
    let e = (t * t * (3.0 - 2.0 * t)) as f64;
    cam.yaw = from.0 + (to.0 - from.0) * e;
    cam.pitch = from.1 + (to.1 - from.1) * e;
    cam.roll = roll.0 + (roll.1 - roll.0) * e;
    if let Some((a, b)) = *target {
        cam.target = [a[0] + (b[0] - a[0]) * e, a[1] + (b[1] - a[1]) * e, a[2] + (b[2] - a[2]) * e];
    }
    if t >= 1.0 {
        // on arrival at a standard view refit the scale and the target, as the instant jump used to; a look at a point
        // keeps both
        if target.is_none() {
            cam.init = false;
        }
        *view_anim = None;
    } else {
        ctx.request_repaint();
    }
}

/// The faces of the ViewCube: (normal, four screen points, depth) in the corner of the viewport.
/// TURN THE VIEW SMOOTHLY to the given angles (about 220 ms). An instant jump reads as something having
/// broken: the eye has nothing to hold on to, and on a complex assembly the part has to be found again.
pub(crate) fn animate_view_to(cam: Cam3, mode_3d: &mut bool, view_anim: &mut Option<ViewTurn>, yaw: f64, pitch: f64) {
    *mode_3d = true;
    // THE SHORTEST WAY ROUND IN YAW: without normalisation a turn from -170 deg to +170 deg would go
    // all the way round — 340 deg instead of 20.
    let mut from_yaw = cam.yaw;
    while yaw - from_yaw > std::f64::consts::PI {
        from_yaw += std::f64::consts::TAU;
    }
    while yaw - from_yaw < -std::f64::consts::PI {
        from_yaw -= std::f64::consts::TAU;
    }
    *view_anim = Some(ViewTurn { from: (from_yaw, cam.pitch), to: (yaw, pitch), since: std::time::Instant::now(), roll: (cam.roll, 0.0), target: None });
}

/// MOVE THE CENTRE OF THE VIEW SMOOTHLY to `to`, the way and the scale kept: a look at a point, as the programs of the
/// layouts that have it move it rather than jump.
pub(crate) fn animate_look_to(cam: Cam3, view_anim: &mut Option<ViewTurn>, to: [f64; 3]) {
    *view_anim = Some(ViewTurn { from: (cam.yaw, cam.pitch), to: (cam.yaw, cam.pitch), since: std::time::Instant::now(), roll: (cam.roll, cam.roll), target: Some((cam.target, to)) });
}

/// Advance the degree-of-freedom sweep. Called every frame; while it runs it asks for a repaint.
pub(crate) fn tick_joint_anim(joint_anim: &mut Option<JointAnim>, project: &mut Project, ctx: &egui::Context) {
    if joint_anim.is_none() {
        return;
    }
    // THE TIME COMES FROM THE FRAME rather than from counting frames: on a slow machine a frame count
    // would stretch the travel, and "two seconds" would turn into who knows how many.
    let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1) as f64;
    step_joint_anim(joint_anim, project, dt);
    ctx.request_repaint();
}
