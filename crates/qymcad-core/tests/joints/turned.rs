//! Joints on a turned anchor: the gizmo, limits, the field, a remade joint; a twist, the bounds of a driver.

use super::*;

/// The gizmo arrow points where the body travels, including when the anchor was turned.
///
/// The mating side is chosen as the nearer to the current placement: when the anchor axes face each other the
/// solver turns the first anchor itself while the stored flag stays false. The turn is 180 degrees about X and
/// reverses the Y and Z axes, that is, the travel axis itself. A gizmo asking for the frame without that turn
/// draws the arrow the other way, so dragging forwards moves the body backwards.
#[test]
fn the_gizmo_arrow_agrees_with_the_travel_even_when_the_anchor_is_turned_around() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // The second body is turned by 180 degrees about X, so its anchor faces the first one.
    set_transform(&mut p, b, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Slider);

    let go = |p: &mut Project, v: f64| {
        if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[1] = Some(v);
        }
        p.solve_joints();
        apply12(&p.world_transform(b), [0.0, 0.0, 0.0])
    };
    let at0 = go(&mut p, 0.0);
    let at1 = go(&mut p, 10.0);
    let d = [at1[0] - at0[0], at1[1] - at0[1], at1[2] - at0[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    assert!((len - 10.0).abs() < 1e-3, "setup: 10 was specified but the body travelled {len:.4}");

    let ax = p.joint_slot_axis(jid, 1, p.root).expect("a slider has a travel axis");
    let along = (d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2]) / len;
    assert!(along > 0.999, "the body does not travel where the arrow points: agreement {along:.4} (travel {:?}, arrow {ax:?})", [d[0] / len, d[1] / len, d[2] / len]);
}

/// A limit holds where its mark is drawn, including when the anchor was turned.
///
/// The limit, the reading and the arrow are one measurement. When the solver turns the first anchor itself
/// (the axes facing each other), all three have to turn together; otherwise the body stops somewhere other
/// than where the mark is drawn and the limit appears not to work.
///
/// What this test holds and what it does not was checked by corruption. Removing the turn from the arrow
/// reddens it ("travelled -10.0000"). Removing the turn from the measurement does not: the specified value is
/// baked into the same turned anchor and the limit agrees with it regardless of the measurement. The
/// measurement is covered by the gates on real documents, where the reading is compared against the specified
/// angle.
#[test]
fn a_limit_stops_the_part_where_its_mark_is_drawn_even_on_a_turned_anchor() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // The anchors face each other, so the solver turns the first one itself.
    set_transform(&mut p, b, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Slider);

    let dir = p.joint_slot_axis(jid, 1, p.root).expect("the travel axis");
    let go = |p: &mut Project, v: f64| {
        if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[1] = Some(v);
        }
        p.solve_joints();
        apply12(&p.world_transform(b), [0.0, 0.0, 0.0])
    };
    let at0 = go(&mut p, 0.0);

    // The limit is 10 mm while 50 is requested, so the body has to stop exactly at the mark.
    if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
        j.limit_min[1] = Some(0.0);
        j.limit_max[1] = Some(10.0);
    }
    let at1 = go(&mut p, 50.0);
    let d = [at1[0] - at0[0], at1[1] - at0[1], at1[2] - at0[2]];
    let along = d[0] * dir[0] + d[1] * dir[1] + d[2] * dir[2];
    let across = ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) - along * along).max(0.0).sqrt();
    assert!(across < 1e-3, "the body moved {across:.4} mm across the travel axis");
    assert!((along - 10.0).abs() < 1e-3, "a limit of 10 mm: the body must stop at the mark (10 along the arrow) but travelled {along:.4}");
}

/// Hold as built works on a turned anchor too.
///
/// The declaration is captured from the frame turned the same way the solver turns it, and afterwards the side
/// flag is cleared so the solver decides the side again from the current axes. If what is captured and what is
/// applied differ even in the order of multiplication, the body jumps when the button is pressed.
#[test]
fn holding_as_it_stands_works_on_a_turned_anchor_too() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // The anchors face each other and the body stands aside: without a declaration the mate would pull it
    // in.
    set_transform(&mut p, b, [1.0, 0.0, 0.0, 25.0, 0.0, -1.0, 0.0, 13.0, 0.0, 0.0, -1.0, 40.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Rigid);

    let before = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    assert!(p.set_joint_as_built(jid), "the hold-as-built declaration must succeed");
    p.solve_joints();
    let after = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    let d = ((after[0] - before[0]).powi(2) + (after[1] - before[1]).powi(2) + (after[2] - before[2]).powi(2)).sqrt();
    assert!(d < 1e-3, "the hold-as-built declaration moved the body {d:.4} mm: was {before:?}, now {after:?}");

    // And it holds: displaced by hand, the body returns to the same place.
    set_transform(&mut p, b, [1.0, 0.0, 0.0, 90.0, 0.0, -1.0, 0.0, 13.0, 0.0, 0.0, -1.0, 40.0]);
    p.solve_joints();
    let back = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    let d2 = ((back[0] - before[0]).powi(2) + (back[1] - before[1]).powi(2) + (back[2] - before[2]).powi(2)).sqrt();
    assert!(d2 < 1e-3, "the hold-as-built mate did not return the body to the declared place: off by {d2:.4} mm ({back:?})");
}

/// An angle is measured in the direction the gizmo shows, including on a turned anchor.
///
/// Travel is already covered (the arrow, the drag, the limit). Rotation was not, and turning an anchor reverses
/// the rotation axis too: a positive angle would run the other way, the limit arc would be drawn mirrored, and
/// the body would stop somewhere other than the mark.
#[test]
fn a_positive_angle_turns_the_part_the_way_the_gizmo_ring_points() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // The anchors face each other, so the solver turns the first one itself.
    set_transform(&mut p, b, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Revolute);

    let axis = p.joint_slot_axis(jid, 0, p.root).expect("the rotation axis");
    let zero = p.joint_zero_dir(jid, p.root).expect("the angle zero");
    // The angle of the body about the gizmo axis, measured from the gizmo zero.
    let spin = |p: &Project| {
        let m = p.world_transform(b);
        let o = apply12(&m, [0.0, 0.0, 0.0]);
        let px = apply12(&m, [1.0, 0.0, 0.0]);
        let v = [px[0] - o[0], px[1] - o[1], px[2] - o[2]];
        let cross = [zero[1] * v[2] - zero[2] * v[1], zero[2] * v[0] - zero[0] * v[2], zero[0] * v[1] - zero[1] * v[0]];
        let s = axis[0] * cross[0] + axis[1] * cross[1] + axis[2] * cross[2];
        let c = zero[0] * v[0] + zero[1] * v[1] + zero[2] * v[2];
        s.atan2(c).to_degrees()
    };
    let go = |p: &mut Project, v: f64| {
        if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[0] = Some(v);
        }
        p.solve_joints();
    };
    go(&mut p, 0.0);
    let at0 = spin(&p);
    go(&mut p, 45.0);
    let at1 = spin(&p);
    let d = (at1 - at0 + 540.0) % 360.0 - 180.0;
    assert!((d - 45.0).abs() < 1e-2, "+45 degrees were specified but the body turned {d:.4} degrees about the gizmo axis (from {at0:.3} to {at1:.3}): the angle runs the wrong way");
}

/// The mate frame shows the same axes the mate holds by.
///
/// The frame is used by the glyph drawing and by the popup. While only its origin was taken, disagreeing axes
/// broke nothing — the origin depends on neither the kind nor the anchor turn. But the name promises the axes
/// of the mate, and sooner or later somebody takes them. The promise is pinned here: the main axis of the frame
/// has to match the axis `joint_slot_axis` returns.
#[test]
fn the_joint_frame_shows_the_axes_the_joint_actually_holds_by() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // The anchors face each other, so the solver turns the first one itself.
    set_transform(&mut p, b, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Slider);

    let m = p.joint_frame(jid, p.root).expect("the mate frame");
    let main = [m[2], m[6], m[10]]; // The main axis (Z) of the frame.
    let axis = p.joint_slot_axis(jid, 1, p.root).expect("the travel axis");
    let dot = main[0] * axis[0] + main[1] * axis[1] + main[2] * axis[2];
    assert!(dot > 0.999, "the main axis of the mate frame {main:?} disagrees with the travel axis {axis:?}: agreement {dot:.4}");
}

/// Every degree travels along its own arrow, the second one included, and on a turned anchor too.
///
/// The anchor turn is 180 degrees about X: it leaves X alone and reverses Y and Z. So for a planar mate the
/// first travel (along X) is indifferent to the turn while the second (along Y) is not; for a pin-slot the
/// travel belongs to the second anchor and turning the first does not affect it either. All of this is
/// measured rather than deduced: reasoning about axes has already been right on paper and wrong in the code
/// three times.
#[test]
fn every_slot_travels_along_its_own_arrow_on_a_turned_anchor() {
    let cases: [(JointKind, usize, f64); 3] = [(JointKind::Planar, 1, 10.0), (JointKind::Planar, 2, 10.0), (JointKind::PinSlot, 1, 10.0)];
    let mut bad = Vec::new();
    let mut checked = 0usize;
    for (kind, slot, want) in cases {
        let mut p = Project::default();
        let c = parts(&mut p, 2);
        let (a, b) = (c[0], c[1]);
        p.set_grounded(a, true);
        // The anchors face each other, so the solver turns the first one itself.
        set_transform(&mut p, b, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0]);
        let ca = p.add_connector(a, AnchorRef::Origin);
        let cb = p.add_connector(b, AnchorRef::Origin);
        let jid = p.add_joint(ca, cb, kind);

        let dir = match p.joint_slot_axis(jid, slot, p.root) {
            Some(d) => d,
            None => {
                bad.push(format!("{kind:?} slot {slot}: the degree has no arrow"));
                continue;
            }
        };
        let go = |p: &mut Project, v: f64| {
            if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
                j.drive[slot] = Some(v);
            }
            p.solve_joints();
            apply12(&p.world_transform(b), [0.0, 0.0, 0.0])
        };
        let at0 = go(&mut p, 0.0);
        let at1 = go(&mut p, want);
        let d = [at1[0] - at0[0], at1[1] - at0[1], at1[2] - at0[2]];
        let along = d[0] * dir[0] + d[1] * dir[1] + d[2] * dir[2];
        checked += 1;
        if (along - want).abs() > 1e-3 {
            bad.push(format!("{kind:?} slot {slot}: {want} was specified but {along:.4} was travelled along the arrow (travel {d:?})"));
        }
    }
    assert_eq!(checked, 3, "guard: three degrees were set up but {checked} were checked");
    assert!(bad.is_empty(), "a degree does not travel along its own arrow:\n  {}", bad.join("\n  "));
}

/// The mate field shows exactly what was specified, including on a turned anchor.
///
/// This is the number shown in the mate popup, and the limit and the relation are computed from it. On a real
/// document it was already caught with the opposite sign (an angle of 30 degrees displayed as -30), but there
/// the mate was a single cylindrical one. Here every kind that has something to display is checked, on a
/// deliberately turned anchor.
#[test]
fn the_field_of_a_joint_shows_exactly_what_was_asked_on_a_turned_anchor() {
    let cases: [(JointKind, usize, f64); 4] = [(JointKind::Revolute, 0, 30.0), (JointKind::Slider, 1, 12.0), (JointKind::Cylindrical, 0, -25.0), (JointKind::Planar, 2, 7.0)];
    let mut bad = Vec::new();
    let mut checked = 0usize;
    for (kind, slot, want) in cases {
        let mut p = Project::default();
        let c = parts(&mut p, 2);
        let (a, b) = (c[0], c[1]);
        p.set_grounded(a, true);
        // The anchors face each other, so the solver turns the first one itself.
        set_transform(&mut p, b, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0]);
        let ca = p.add_connector(a, AnchorRef::Origin);
        let cb = p.add_connector(b, AnchorRef::Origin);
        let jid = p.add_joint(ca, cb, kind);
        if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[slot] = Some(want);
        }
        p.solve_joints();
        let shown = p
            .joints
            .iter()
            .find(|x| x.id == jid)
            .map(|j| match slot {
                0 => j.angle,
                1 => j.offset,
                _ => j.offset2,
            })
            .unwrap_or(f64::NAN);
        checked += 1;
        if (shown - want).abs() > 1e-3 {
            bad.push(format!("{kind:?} slot {slot}: {want} was specified but the field shows {shown:.4}"));
        }
    }
    assert_eq!(checked, 4, "guard: four fields were set up but {checked} were checked");
    assert!(bad.is_empty(), "the mate holds one thing while the field says another:\n  {}", bad.join("\n  "));
}

/// A mate re-created on a turned anchor behaves the same way.
///
/// The mating side is not stored in the document as an answer: it is chosen from the current placement. So
/// re-creating a mate from the same anchors has to give the same side, the same field and the same travel.
/// Otherwise deleting a mate and creating it again would change the behaviour.
///
/// What this test holds and what it does not was checked by corruption. Reducing the side choice to a single
/// stored flag does not redden it: both runs use one rule, whatever it is, and therefore agree. So it holds the
/// determinism of re-creation rather than the correctness of the turn. The turn is covered by the neighbouring
/// tests (the mate field across every kind, the arrow, the relation).
#[test]
fn a_remade_joint_on_a_turned_anchor_behaves_the_same() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let home = [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0];
    set_transform(&mut p, b, home);
    let mk = |p: &mut Project| {
        let ca = p.add_connector(a, AnchorRef::Origin);
        let cb = p.add_connector(b, AnchorRef::Origin);
        p.add_joint(ca, cb, JointKind::Slider)
    };
    let run = |p: &mut Project, jid: Id| {
        if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[1] = Some(12.0);
        }
        p.solve_joints();
        let shown = p.joints.iter().find(|x| x.id == jid).map(|j| j.offset).unwrap_or(f64::NAN);
        let dir = p.joint_slot_axis(jid, 1, p.root).expect("the travel axis");
        (shown, dir, apply12(&p.world_transform(b), [0.0, 0.0, 0.0]))
    };

    let j1 = mk(&mut p);
    let (shown1, dir1, at1) = run(&mut p, j1);

    // Create it again from the same anchors, with the body put back.
    p.delete_joint(j1);
    set_transform(&mut p, b, home);
    let j2 = mk(&mut p);
    let (shown2, dir2, at2) = run(&mut p, j2);

    assert!((shown1 - 12.0).abs() < 1e-3 && (shown2 - 12.0).abs() < 1e-3, "the field must show the specified 12: was {shown1:.4}, now {shown2:.4}");
    let dd = (0..3).map(|k| (dir1[k] - dir2[k]).abs()).fold(0.0f64, f64::max);
    assert!(dd < 1e-9, "the arrow points differently after re-creation: was {dir1:?}, now {dir2:?}");
    let d = ((at1[0] - at2[0]).powi(2) + (at1[1] - at2[1]).powi(2) + (at1[2] - at2[2]).powi(2)).sqrt();
    assert!(d < 1e-3, "the re-created mate placed the body elsewhere: was {at1:?}, now {at2:?} (off by {d:.4} mm)");
}

/// A rigid mate turns the body about the joint axis.
///
/// A rigid mate has two parameters: an offset and a rotation. With only the gap along the axis available there
/// is no way to turn a fastened body about the joint axis, and the body itself has to be rotated by guesswork.
/// This is not a missing dialogue field but a missing specifiable parameter: a rigid mate has no freedom, yet
/// it does have parameters, and rotation is one of them.
#[test]
fn a_fastened_joint_can_be_given_a_twist_about_its_axis() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    set_transform(&mut p, b, tr(50.0, 20.0, -10.0));
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Rigid);

    // Without a specified value a rigid mate aligns the anchors with no extra turn.
    p.solve_joints();
    let x0 = apply12(&p.world_transform(b), [1.0, 0.0, 0.0]);
    assert!((x0[0] - 1.0).abs() < 1e-6 && x0[1].abs() < 1e-6, "setup: without a rotation the body X axis must land on world X, but it is {x0:?}");

    // Specify a rotation of 30 degrees about the joint axis.
    if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
        j.drive[0] = Some(30.0);
    }
    p.solve_joints();
    let o = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    let x = apply12(&p.world_transform(b), [1.0, 0.0, 0.0]);
    let v = [x[0] - o[0], x[1] - o[1], x[2] - o[2]];
    let ang = v[1].atan2(v[0]).to_degrees();
    assert!((ang - 30.0).abs() < 1e-3, "a rotation of 30 degrees was specified but the body turned {ang:.4} degrees");
    // The gap still applies alongside the rotation.
    if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
        j.drive[1] = Some(7.0);
    }
    p.solve_joints();
    let o2 = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    assert!((o2[2] - 7.0).abs() < 1e-3, "a gap of 7 mm must apply together with the rotation, but the body is at z = {:.4}", o2[2]);
}

/// A DRIVER IS HELD INSIDE ITS LIMITS BY THE MODEL, not by whoever sets it.
///
/// Four places in the interface used to write `drive[slot]` by hand, and only one of them clamped to the
/// limit. Nothing wrong came of it - the other three happened to pass values already in range - but the
/// rule lived in four memories instead of in the code. `set_joint_drive` is where it lives now, and this
/// is what says so.
#[test]
fn a_driver_cannot_be_set_outside_the_limits() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Revolute);
    {
        let jj = p.joints.iter_mut().find(|x| x.id == j).unwrap();
        jj.limit_min[0] = Some(-30.0);
        jj.limit_max[0] = Some(45.0);
    }

    assert!(p.set_joint_drive(j, 0, Some(90.0)), "the mate is there and the slot exists");
    let got = p.joints.iter().find(|x| x.id == j).unwrap().drive[0];
    assert_eq!(got, Some(45.0), "above the upper limit the driver is held at it, not written through");

    p.set_joint_drive(j, 0, Some(-100.0));
    let got = p.joints.iter().find(|x| x.id == j).unwrap().drive[0];
    assert_eq!(got, Some(-30.0), "below the lower limit likewise");

    p.set_joint_drive(j, 0, Some(10.0));
    let got = p.joints.iter().find(|x| x.id == j).unwrap().drive[0];
    assert_eq!(got, Some(10.0), "a value inside the limits passes through untouched");

    // CLEARING IS NOT CLAMPING. `None` means "the degree of freedom is free again", and a limit has
    // nothing to say about that.
    p.set_joint_drive(j, 0, None);
    let got = p.joints.iter().find(|x| x.id == j).unwrap().drive[0];
    assert_eq!(got, None, "clearing the driver frees the degree of freedom");

    // A MISS IS TOLD APART FROM A CHANGE, so a caller cannot take one for the other.
    assert!(!p.set_joint_drive(j, 7, Some(1.0)), "there is no seventh slot");
    assert!(!p.set_joint_drive(9999, 0, Some(1.0)), "there is no such mate");
}

/// A CONNECTOR IS PLACED IN ONE MOVE, not three.
///
/// The panel used to assign the point, the turn and the offset one after another. Three writes have two
/// moments in between when the record disagrees with itself, and anything reading the connector then -
/// a solve, a redraw, a check - sees half a placement.
#[test]
fn a_connector_is_placed_in_one_move() {
    let mut p = Project::default();
    let c = parts(&mut p, 1);
    let conn = p.add_connector(c[0], AnchorRef::Origin);

    let point = qymcad_core::asm::connector::AttachPoint::End;
    assert!(p.set_connector_placement(conn, point, 45.0, [1.0, 2.0, 3.0]), "the connector is there");
    let got = p.connector(conn).expect("the connector");
    assert_eq!(got.point, point, "the point is set");
    assert_eq!(got.rot_deg, 45.0, "the turn is set");
    assert_eq!(got.offset_xyz, [1.0, 2.0, 3.0], "and the offset with it, in the same call");

    assert!(!p.set_connector_placement(9999, point, 0.0, [0.0; 3]), "a miss is told apart from a change");
}

/// SHOWING AND HIDING A COMPONENT, AND RENAMING IT, ARE OPERATIONS OF THE DOCUMENT.
#[test]
fn a_component_is_shown_hidden_and_renamed_by_name() {
    let mut p = Project::default();
    let c = parts(&mut p, 1);
    assert!(p.set_component_visible(c[0], false), "the component is there");
    assert!(!p.components.iter().find(|x| x.id == c[0]).unwrap().visible, "it is hidden");
    assert!(p.set_component_visible(c[0], true));
    assert!(p.components.iter().find(|x| x.id == c[0]).unwrap().visible, "and shown again");

    assert!(p.rename_component(c[0], "bracket"), "renaming works by id");
    assert_eq!(p.components.iter().find(|x| x.id == c[0]).unwrap().name, "bracket");

    assert!(!p.set_component_visible(9999, true), "a miss is told apart from a change");
    assert!(!p.rename_component(9999, "nowhere"), "and so is a rename of nothing");
}
