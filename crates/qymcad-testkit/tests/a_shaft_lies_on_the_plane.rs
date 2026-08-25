//! TANGENCY: A SHAFT LIES ON A PLANE INSTEAD OF PASSING THROUGH IT.
//!
//! Tangency is the one mate that needs no connectors: two selected surfaces are enough.
//! Geometrically "a cylinder touches a plane" is two conditions at once: the axis is parallel to the
//! plane (otherwise the cylinder intersects it) and the distance from the axis to the plane equals
//! the radius.
//!
//! Checked on LIVE geometry: the cylinder is built by the OCCT kernel and the radius is taken from a
//! real face rather than handed in.
use qymcad_core::feature::{apply12, AnchorRef, FaceKey};
use qymcad_core::model::{Id, Project};

/// A 60x60x10 plate at the origin — its top face is the plane.
fn plate(p: &mut Project) -> (Id, Id) {
    let c = p.add_part("plate");
    p.set_active_component(Some(c));
    let s = p.new_sketch("plate");
    let sid = p.sketches[s].id;
    p.add_sketch_node(sid, "plate");
    p.add_rect_entity(s, 0.0, 0.0, 60.0, 60.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s);
    let body = p.add_extrude(sid, 10.0);
    p.finish_base_body(body, 1);
    (c, body)
}

/// A d10 shaft (a cylinder from a revolved circle) — its side face is the cylinder.
fn shaft(p: &mut Project) -> (Id, Id) {
    let c = p.add_part("shaft");
    p.set_active_component(Some(c));
    let s = p.new_sketch("shaft");
    let sid = p.sketches[s].id;
    p.add_sketch_node(sid, "shaft");
    p.add_circle_entity(s, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s);
    let body = p.add_extrude(sid, 40.0);
    p.finish_base_body(body, 1);
    (c, body)
}

#[test]
fn a_tangent_puts_the_shaft_on_the_plane() {
    let mut p = Project::default();
    p.new_document();
    let (cp, bp) = plate(&mut p);
    let (cs, bs) = shaft(&mut p);
    let r = qymcad_testkit::open_like_the_app(&mut p);
    assert!(r.errors.is_empty(), "did not build: {:?}", r.errors);
    p.set_grounded(cp, true);

    // The top face of the plate.
    let f = p.regen_faces.get(&bp).and_then(|fs| fs.iter().filter(|f| f.normal[2] > 0.99).max_by(|a, b| a.area.partial_cmp(&b.area).unwrap()).cloned()).expect("top of the plate");
    let plane = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
    // The side face of the shaft — the one where the kernel finds an AXIS and a radius.
    let (cyl, radius) = p
        .regen_faces
        .get(&bs)
        .expect("faces of the shaft")
        .iter()
        .find_map(|f| {
            let k = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
            p.face_cylinder(bs, &k).map(|(_, _, r)| (k, r))
        })
        .expect("the shaft has a cylindrical face");
    assert!((radius - 5.0).abs() < 0.2, "the cylinder radius came out as {radius:.3} instead of 5 — then the tangency would land in the wrong place too");
    // A CYLINDER DOES NOT PASS FOR A SPHERE, even though the vertices of its side face sit on two
    // rims that lie on ONE sphere, so the fit comes out perfect. Only the radial normal tells them
    // apart.
    assert!(p.face_sphere(bs, &cyl).is_none(), "the shaft side was taken for a sphere — the tangency would be computed against a sphere that does not exist");

    // THE SHAFT LIES DOWN (axis along X) AND HANGS IN THE AIR — that is what has to be lowered onto
    // the plate.
    //
    // It cannot be stood upright: a cylinder standing on its end is never tangent to the plane at
    // all. That is not a quibble of the check — it is a degenerate starting position from which
    // "tangency" is unreachable by any motion, and the solver has nothing to look for there.
    let i = p.component_index(cs).expect("shaft");
    p.components[i].transform = [0.0, 0.0, 1.0, 20.0, 0.0, 1.0, 0.0, 20.0, -1.0, 0.0, 0.0, 40.0];
    p.add_tangent(cp, AnchorRef::FaceCenter(bp, plane.clone()), cs, AnchorRef::FaceCenter(bs, cyl));
    p.solve_joints();

    // THE SHAFT AXIS SITS EXACTLY ONE RADIUS ABOVE THE PLANE: the shaft lies ON it.
    let at = apply12(&p.world_transform(cs), [0.0, 0.0, 0.0]);
    let above = at[2] - plane.centroid[2];
    assert!(
        (above - radius).abs() < 1e-3,
        "the shaft did not lie on the plane: its axis is {above:.3} above the face, and the radius is {radius:.3}"
    );
}

/// A SHAFT STANDING ON ITS END CANNOT BE LAID DOWN BY A TANGENCY — AND THAT DOES NOT TEAR THE
/// ASSEMBLY APART.
///
/// A cylinder whose axis is perpendicular to the plane is never tangent to it: no such position
/// exists and the solver has nothing to look for. What matters is that the part STAYS WHERE IT IS
/// instead of drifting into a compromise — if it does not converge, nothing moves.
#[test]
fn a_shaft_standing_on_its_end_is_left_alone() {
    let mut p = Project::default();
    p.new_document();
    let (cp, bp) = plate(&mut p);
    let (cs, bs) = shaft(&mut p);
    let r = qymcad_testkit::open_like_the_app(&mut p);
    assert!(r.errors.is_empty(), "did not build: {:?}", r.errors);
    p.set_grounded(cp, true);

    let f = p.regen_faces.get(&bp).and_then(|fs| fs.iter().filter(|f| f.normal[2] > 0.99).max_by(|a, b| a.area.partial_cmp(&b.area).unwrap()).cloned()).expect("top of the plate");
    let plane = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
    let cyl = p
        .regen_faces
        .get(&bs)
        .expect("faces of the shaft")
        .iter()
        .find_map(|f| {
            let k = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
            p.face_cylinder(bs, &k).map(|_| k)
        })
        .expect("cylindrical face");

    // the shaft STANDS (axis along Z, same as the plane normal) — the degenerate position
    p.move_component(cs, [20.0, 20.0, 40.0]);
    let before = apply12(&p.world_transform(cs), [0.0, 0.0, 0.0]);
    p.add_tangent(cp, AnchorRef::FaceCenter(bp, plane), cs, AnchorRef::FaceCenter(bs, cyl));
    p.solve_joints();
    let after = apply12(&p.world_transform(cs), [0.0, 0.0, 0.0]);

    let moved = ((after[0] - before[0]).powi(2) + (after[1] - before[1]).powi(2) + (after[2] - before[2]).powi(2)).sqrt();
    assert!(moved < 1e-9, "a tangency that cannot exist dragged the part {moved:.3} mm");
    assert!(p.mates_conflict, "an unsatisfiable tangency is not flagged as a conflict — the user would think everything is fine");
}

/// A shaft of arbitrary radius and length — the second cylinder for cylinder-to-cylinder tangency.
fn shaft_of(p: &mut Project, name: &str, r: f64, len: f64) -> (Id, Id) {
    let c = p.add_part(name);
    p.set_active_component(Some(c));
    let s = p.new_sketch(name);
    let sid = p.sketches[s].id;
    p.add_sketch_node(sid, name);
    p.add_circle_entity(s, 0.0, 0.0, r, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s);
    let body = p.add_extrude(sid, len);
    p.finish_base_body(body, 1);
    (c, body)
}

/// A cylindrical face of a body and its radius.
fn cyl_face(p: &Project, body: Id) -> (FaceKey, f64) {
    p.regen_faces
        .get(&body)
        .expect("faces of the body")
        .iter()
        .find_map(|f| {
            let k = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
            p.face_cylinder(body, &k).map(|(_, _, r)| (k, r))
        })
        .expect("the body has a cylindrical face")
}

/// TWO SHAFTS TOUCH SIDE BY SIDE: the distance between the axes equals the SUM of the radii.
///
/// Tangency used to handle only "cylinder and plane", and a pair of cylinders was dropped from the
/// problem silently: the constraint sits in the list and does nothing. Two rollers, pulleys or gear
/// blanks are the most ordinary case in a mechanism, and there is no "plane in between" to prop it up
/// with.
#[test]
fn two_shafts_touch_side_by_side_at_the_sum_of_their_radii() {
    let mut p = Project::default();
    p.new_document();
    let (c1, b1) = shaft_of(&mut p, "large shaft", 8.0, 40.0);
    let (c2, b2) = shaft_of(&mut p, "small shaft", 3.0, 40.0);
    let r = qymcad_testkit::open_like_the_app(&mut p);
    assert!(r.errors.is_empty(), "did not build: {:?}", r.errors);
    p.set_grounded(c1, true);

    let (k1, r1) = cyl_face(&p, b1);
    let (k2, r2) = cyl_face(&p, b2);
    assert!((r1 - 8.0).abs() < 0.3 && (r2 - 3.0).abs() < 0.3, "the radii came out as {r1:.3} and {r2:.3} instead of 8 and 3 — then the tangency would land in the wrong place too");

    // the second shaft stands NEARBY with parallel axes, but far away — it has to be moved into contact
    p.move_component(c2, [30.0, 0.0, 0.0]);
    p.add_tangent(c1, AnchorRef::FaceCenter(b1, k1), c2, AnchorRef::FaceCenter(b2, k2));
    p.solve_joints();

    let at = apply12(&p.world_transform(c2), [0.0, 0.0, 0.0]);
    let between = (at[0] * at[0] + at[1] * at[1]).sqrt(); // both axes along Z, the first at the origin
    assert!(
        (between - (r1 + r2)).abs() < 1e-2,
        "the shafts did not touch side by side: {between:.3} between the axes, and the sum of the radii is {:.3}",
        r1 + r2
    );
    // AND IT WAS NOT PULLED INSIDE: the side is chosen by proximity, and the shaft stood OUTSIDE.
    assert!(between > r1, "the smaller shaft was dragged inside the larger one: {between:.3} between the axes at radius {r1:.3}");
}

/// A ball of radius `r` — a semicircle (arc plus diameter) revolved around the sketch X axis. A real
/// SPHERICAL face.
fn ball(p: &mut Project, r: f64) -> (Id, Id) {
    let c = p.add_part("ball");
    p.set_active_component(Some(c));
    let s = p.new_sketch("ball");
    let sid = p.sketches[s].id;
    p.add_sketch_node(sid, "ball");
    p.add_arc_entity(s, 0.0, 0.0, -r, 0.0, r, 0.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(s, r, 0.0, -r, 0.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s);
    let cid = p.sketches[s].contour_ids.iter().copied().find(|k| p.contour_profile_xy(*k).is_some()).expect("semicircle contour");
    let body = p.add_revolve_axis(sid, vec![cid], 0, 360.0, 0, 0);
    p.finish_base_body(body, 1);
    (c, body)
}

#[test]
fn two_balls_touch_at_the_sum_of_their_radii() {
    let mut p = Project::default();
    p.new_document();
    let (c1, b1) = ball(&mut p, 9.0);
    let (c2, b2) = ball(&mut p, 4.0);
    let r = qymcad_testkit::open_like_the_app(&mut p);
    assert!(r.errors.is_empty(), "did not build: {:?}", r.errors);
    p.set_grounded(c1, true);
    let sphere_of = |p: &Project, b: Id| -> (FaceKey, f64) {
        p.regen_faces
            .get(&b)
            .expect("faces of the ball")
            .iter()
            .find_map(|f| {
                let k = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
                p.face_sphere(b, &k).map(|(_, r)| (k, r))
            })
            .expect("the ball has a spherical face")
    };
    let (k1, r1) = sphere_of(&p, b1);
    let (k2, r2) = sphere_of(&p, b2);
    p.move_component(c2, [40.0, 0.0, 0.0]);
    p.add_tangent(c1, AnchorRef::FaceCenter(b1, k1), c2, AnchorRef::FaceCenter(b2, k2));
    p.solve_joints();
    let a = apply12(&p.world_transform(c1), [0.0, 0.0, 0.0]);
    let b = apply12(&p.world_transform(c2), [0.0, 0.0, 0.0]);
    let d = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
    assert!((d - (r1 + r2)).abs() < 1e-3, "the balls must touch: {d:.4} between the centres instead of {:.4}", r1 + r2);
}

/// A BALL SETTLES ON THE PLATE INSTEAD OF FALLING THROUGH IT (sphere-to-plane tangency).
///
/// The geometry of a ball is simpler than that of a cylinder: tangency is one condition, "the centre
/// one radius above the plane". All the difficulty is in RECOGNITION: a sphere has no axis and
/// `face_cylinder` says nothing about it, while a flat patch is approximated beautifully by an
/// enormous sphere.
#[test]
fn a_ball_settles_on_the_plate_at_exactly_its_radius() {
    let mut p = Project::default();
    p.new_document();
    let (cp, bp) = plate(&mut p);
    let (cb, bb) = ball(&mut p, 7.0);
    let r = qymcad_testkit::open_like_the_app(&mut p);
    assert!(r.errors.is_empty(), "did not build: {:?}", r.errors);
    p.set_grounded(cp, true);

    let f = p.regen_faces.get(&bp).and_then(|fs| fs.iter().filter(|f| f.normal[2] > 0.99).max_by(|a, b| a.area.partial_cmp(&b.area).unwrap()).cloned()).expect("top of the plate");
    let plane = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
    // A PLANAR FACE DOES NOT COUNT AS A SPHERE — otherwise the tangency would compute against
    // something other than what was selected.
    assert!(p.face_sphere(bp, &plane).is_none(), "the top of the plate was taken for a sphere — recognition lies");

    let (sph, radius) = p
        .regen_faces
        .get(&bb)
        .expect("faces of the ball")
        .iter()
        .find_map(|f| {
            let k = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
            p.face_sphere(bb, &k).map(|(_, r)| (k, r))
        })
        .expect("the ball has a spherical face");
    assert!((radius - 7.0).abs() < 0.1, "the ball radius came out as {radius:.3} instead of 7 — then the tangency would land in the wrong place too");

    // the ball hangs in the air above the plate — that is what has to be lowered
    p.move_component(cb, [20.0, 20.0, 45.0]);
    p.add_tangent(cp, AnchorRef::FaceCenter(bp, plane.clone()), cb, AnchorRef::FaceCenter(bb, sph));
    p.solve_joints();

    let at = apply12(&p.world_transform(cb), [0.0, 0.0, 0.0]);
    let above = at[2] - plane.centroid[2];
    assert!(
        (above - radius).abs() < 1e-3,
        "the ball did not settle on the plate: its centre is {above:.3} above the face, and the radius is {radius:.3}"
    );
    // and it was NOT flipped under the plate: the side is chosen by the current position
    assert!(above > 0.0, "the ball ended up UNDER the plate ({above:.3}) — the wrong side was chosen");
}

/// A 40x40x20 plate with a through d20 hole — its bore wall is the INNER cylinder.
fn plate_with_a_hole(p: &mut Project) -> (Id, Id) {
    let c = p.add_part("plate with a hole");
    p.set_active_component(Some(c));
    let s = p.new_sketch("plate with a hole");
    let sid = p.sketches[s].id;
    p.add_sketch_node(sid, "plate with a hole");
    p.add_rect_entity(s, 0.0, 0.0, 40.0, 40.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(s, 20.0, 20.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s);
    let body = p.add_extrude(sid, 20.0);
    p.finish_base_body(body, 1);
    (c, body)
}

/// A SHAFT SETTLES INSIDE A BORE: the distance between the axes equals the DIFFERENCE of the radii.
///
/// Outside, cylinders touch at the sum of the radii; inside, at the difference. The side is chosen by
/// proximity to the current position: a shaft standing IN the bore must not pop out, and the other
/// way round. Only the "outside" half of the condition used to be covered.
#[test]
fn a_shaft_inside_a_bore_touches_at_the_difference_of_radii() {
    let mut p = Project::default();
    p.new_document();
    let (cp, bp) = plate_with_a_hole(&mut p);
    let (cs, bs) = shaft_of(&mut p, "shaft", 3.0, 60.0);
    let r = qymcad_testkit::open_like_the_app(&mut p);
    assert!(r.errors.is_empty(), "did not build: {:?}", r.errors);
    p.set_grounded(cp, true);

    let (bore, r_bore) = cyl_face(&p, bp);
    let (shaft, r_shaft) = cyl_face(&p, bs);
    assert!((r_bore - 10.0).abs() < 0.3, "the bore radius came out as {r_bore:.3} instead of 10");
    assert!((r_shaft - 3.0).abs() < 0.3, "the shaft radius came out as {r_shaft:.3} instead of 3");

    // the shaft stands INSIDE the bore but off centre: the bore axis is at (20, 20)
    p.move_component(cs, [22.0, 20.0, -20.0]);
    p.add_tangent(cp, AnchorRef::FaceCenter(bp, bore), cs, AnchorRef::FaceCenter(bs, shaft));
    p.solve_joints();

    let at = apply12(&p.world_transform(cs), [0.0, 0.0, 0.0]);
    let between = ((at[0] - 20.0).powi(2) + (at[1] - 20.0).powi(2)).sqrt();
    let want = r_bore - r_shaft;
    assert!(
        (between - want).abs() < 1e-2,
        "the shaft did not settle against the bore wall FROM INSIDE: {between:.3} between the axes, and the difference of the radii is {want:.3}"
    );
    // AND IT DID NOT POP OUT: the sum of the radii would be 13, and the shaft stood inside.
    assert!(between < r_bore, "the shaft popped out of the bore: {between:.3} between the axes at bore radius {r_bore:.3}");
}
