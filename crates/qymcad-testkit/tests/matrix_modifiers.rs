//! MATRIX: revolve x axes/angles; fillet/chamfer x edges/radii (including a radius that is far too
//! large — an honest error, not garbage); shell. Real OCCT kernel, exact volumes, failures accumulate.

use qymcad_core::model::Project;

const PI: f64 = std::f64::consts::PI;

fn fail_check(fails: &mut Vec<String>, label: &str, v_got: f64, v_exp: f64, tol: f64) {
    if v_exp == 0.0 {
        if v_got.abs() > 1e-6 {
            fails.push(format!("{label}: expected NOTHING, got V={v_got:.1}"));
        }
        return;
    }
    if ((v_got - v_exp) / v_exp).abs() > tol {
        fails.push(format!("{label}: V={v_got:.1}, expected {v_exp:.1}"));
    }
}

/// Revolving the rectangle (10..20)x(0..30) around the sketch Y axis: angles 90/180/270/360.
/// A ring (tube): V = angle/360 * pi*(R^2-r^2)*h = f * pi*(400-100)*30.
#[test]
fn matrix_revolve_angles() {
    let mut fails = Vec::new();
    for angle in [90.0_f64, 180.0, 270.0, 360.0] {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("s");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "s");
        p.add_rect_entity(si, 10.0, 0.0, 20.0, 30.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let body = p.add_revolve_axis(sid, vec![cid], 1, angle, 0, 0); // axis 1 = sketch Y
        let last = p.finish_base_body(body, 1);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        let label = format!("revolve {angle} degrees");
        for (id, e) in &report.errors {
            fails.push(format!("{label}: ERROR {id}: {e}"));
        }
        let v = shapes.get(&last).map(|s| s.volume()).unwrap_or(0.0);
        fail_check(&mut fails, &label, v, angle / 360.0 * PI * 300.0 * 30.0, 0.02);
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// A 20x20x20 box: fillet/chamfer on ONE edge and on ALL edges of the top face; a deliberately
/// OVERSIZED radius — the node must give an HONEST error, not a garbage body.
#[test]
fn matrix_fillet_chamfer() {
    let mut fails = Vec::new();
    let mk_cube = || -> (Project, u64) {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("s");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "s");
        p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let e = p.add_extrude_multi(sid, vec![cid], 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let cube = p.finish_base_body(e, 1);
        (p, cube)
    };
    // fillet on one vertical edge r=5: V = 8000 - (25 - pi*25/4)*20 = 8000 - 107.3
    {
        let (mut p, cube) = mk_cube();
        let _ = qymcad_testkit::regenerate(&mut p);
        // the edge to fillet: the kernel's persistent edges live in regen_edges (vertical at corner (0,0))
        let edges = p.regen_edges.get(&cube).cloned().unwrap_or_default();
        let vert = edges.iter().find(|e| {
            let (a, b) = (e.a, e.b);
            (a[0]).abs() < 1e-6 && (a[1]).abs() < 1e-6 && (b[0]).abs() < 1e-6 && (b[1]).abs() < 1e-6 && (a[2] - b[2]).abs() > 10.0
        });
        match vert {
            None => fails.push("fillet: the vertical edge (0,0) was not found in the kernel".into()),
            Some(ed) => {
                let f = p.add_fillet(cube, 5.0, vec![ed.id]);
                let (report, shapes) = qymcad_testkit::regenerate(&mut p);
                for (id, e) in &report.errors {
                    fails.push(format!("fillet r5: ERROR {id}: {e}"));
                }
                let v = shapes.get(&f).map(|s| s.volume()).unwrap_or(0.0);
                fail_check(&mut fails, "fillet r5 on one edge", v, 8000.0 - (25.0 - PI * 25.0 / 4.0) * 20.0, 0.01);
            }
        }
    }
    // chamfer on one edge d=4: V = 8000 - (16/2)*20 = 7840
    {
        let (mut p, cube) = mk_cube();
        let _ = qymcad_testkit::regenerate(&mut p);
        let edges = p.regen_edges.get(&cube).cloned().unwrap_or_default();
        let vert = edges.iter().find(|e| {
            let (a, b) = (e.a, e.b);
            (a[0]).abs() < 1e-6 && (a[1]).abs() < 1e-6 && (b[0]).abs() < 1e-6 && (b[1]).abs() < 1e-6 && (a[2] - b[2]).abs() > 10.0
        });
        match vert {
            None => fails.push("chamfer: the edge was not found".into()),
            Some(ed) => {
                let f = p.add_chamfer(cube, 4.0, vec![ed.id]);
                let (report, shapes) = qymcad_testkit::regenerate(&mut p);
                for (id, e) in &report.errors {
                    fails.push(format!("chamfer d4: ERROR {id}: {e}"));
                }
                let v = shapes.get(&f).map(|s| s.volume()).unwrap_or(0.0);
                fail_check(&mut fails, "chamfer d4 on one edge", v, 8000.0 - 8.0 * 20.0, 0.01);
            }
        }
    }
    // A KNOWINGLY impossible fillet r=15 (more than half the face of a 20 box): expect an HONEST node
    // error (report.errors) AND no garbage body with a volume that has slid.
    {
        let (mut p, cube) = mk_cube();
        let _ = qymcad_testkit::regenerate(&mut p);
        let edges = p.regen_edges.get(&cube).cloned().unwrap_or_default();
        let vert = edges.iter().find(|e| {
            let (a, b) = (e.a, e.b);
            (a[0]).abs() < 1e-6 && (a[1]).abs() < 1e-6 && (a[2] - b[2]).abs() > 10.0
        });
        if let Some(ed) = vert {
            let f = p.add_fillet(cube, 15.0, vec![ed.id]);
            let (report, shapes) = qymcad_testkit::regenerate(&mut p);
            let errored = report.errors.iter().any(|(id, _)| *id == f);
            let v = shapes.get(&f).map(|s| s.volume());
            // acceptable: a node error (fallback pass-through V=8000) OR OCCT honestly built it (V<8000, valid)
            match v {
                Some(vv) if !errored && (vv > 8000.0 + 1.0 || vv < 4000.0) => {
                    fails.push(format!("fillet r15 (impossible): no error and a garbage volume {vv:.0}"));
                }
                None if !errored => fails.push("fillet r15: no body and no error (a silent failure)".into()),
                _ => {}
            }
        }
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Shelling a 20^3 box, wall 2, top face open: V = 8000 - 16*16*18 = 3392.
#[test]
fn matrix_shell() {
    let mut fails = Vec::new();
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("s");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
    let e = p.add_extrude_multi(sid, vec![cid], 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let cube = p.finish_base_body(e, 1);
    let _ = qymcad_testkit::regenerate(&mut p);
    // the top face: centre (10,10,20), normal +Z — looked up in the model's regen_faces
    let top = p.regen_faces.get(&cube).and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.9 && (f.centroid.z - 20.0).abs() < 1e-3)).map(|f| f.id);
    match top {
        None => fails.push("shell: the top face of the box was not found".into()),
        Some(fid) => {
            let sh = p.add_shell(cube, 2.0, vec![fid], false);
            let (report, shapes) = qymcad_testkit::regenerate(&mut p);
            for (id, er) in &report.errors {
                fails.push(format!("shell: ERROR {id}: {er}"));
            }
            let v = shapes.get(&sh).map(|s| s.volume()).unwrap_or(0.0);
            fail_check(&mut fails, "shell t2 with the top open", v, 8000.0 - 16.0 * 16.0 * 18.0, 0.02);
        }
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// A 4x4x4 box: filleting ALL 4 vertical edges at r=2 yields a d4 CYLINDER (pi*4*4); r=1.8 is
/// "almost a cylinder". Not even 1.8 used to succeed.
#[test]
fn matrix_fillet_cube_to_cylinder() {
    let mut fails: Vec<String> = Vec::new();
    for r in [1.0_f64, 1.8, 1.99] {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("s");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "s");
        p.add_rect_entity(si, 0.0, 0.0, 4.0, 4.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let e = p.add_extrude_multi(sid, vec![cid], 4.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let cube = p.finish_base_body(e, 1);
        let _ = qymcad_testkit::regenerate(&mut p);
        let verts: Vec<u32> = p
            .regen_edges
            .get(&cube)
            .map(|es| es.iter().filter(|ed| (ed.a[2] - ed.b[2]).abs() > 3.0).map(|ed| ed.id).collect())
            .unwrap_or_default();
        if verts.len() != 4 {
            fails.push(format!("r={r}: {} vertical edges (expected 4)", verts.len()));
            continue;
        }
        let f = p.add_fillet(cube, r, verts);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        for (id, er) in &report.errors {
            fails.push(format!("r={r}: ERROR {id}: {er}"));
        }
        let v = shapes.get(&f).map(|s| s.volume()).unwrap_or(0.0);
        let exp = (16.0 - (4.0 - PI) * r * r) * 4.0;
        fail_check(&mut fails, &format!("box 4^3 r={r}"), v, exp, 0.02);
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// EXACTLY the degeneracy boundary — a 10^3 box, fillet r=5 (half the face) yields a cylinder;
/// chamfer d=10 (the whole face) yields a wedge. OCCT fails right at the limit, so the kernel backs
/// off by about 2e-8 automatically.
#[test]
fn matrix_degenerate_boundary_fillet_chamfer() {
    let mut fails: Vec<String> = Vec::new();
    let mk = |p: &mut Project| -> u64 {
        p.new_document();
        let si = p.new_sketch("s");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "s");
        p.add_rect_entity(si, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let e = p.add_extrude_multi(sid, vec![cid], 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        p.finish_base_body(e, 1)
    };
    // filleting all 4 vertical edges at EXACTLY r=5 yields a cylinder pi*25*10
    {
        let mut p = Project::default();
        let cube = mk(&mut p);
        let _ = qymcad_testkit::regenerate(&mut p);
        let verts: Vec<u32> = p.regen_edges.get(&cube).map(|es| es.iter().filter(|e| (e.a[2] - e.b[2]).abs() > 5.0).map(|e| e.id).collect()).unwrap_or_default();
        let f = p.add_fillet(cube, 5.0, verts);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        for (id, er) in &report.errors {
            fails.push(format!("fillet r=5 EXACTLY: ERROR {id}: {er}"));
        }
        let v = shapes.get(&f).map(|s| s.volume()).unwrap_or(0.0);
        fail_check(&mut fails, "box 10^3 r=5 (the boundary)", v, PI * 25.0 * 10.0, 0.02);
    }
    // chamfering ONE vertical edge at EXACTLY d=10 (the whole face) cuts half the box off as a wedge: V=500
    {
        let mut p = Project::default();
        let cube = mk(&mut p);
        let _ = qymcad_testkit::regenerate(&mut p);
        let eid = p.regen_edges.get(&cube).and_then(|es| es.iter().find(|e| e.a[0].abs() < 1e-6 && e.a[1].abs() < 1e-6 && (e.a[2] - e.b[2]).abs() > 5.0)).map(|e| e.id).unwrap();
        let f = p.add_chamfer(cube, 10.0, vec![eid]);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        for (id, er) in &report.errors {
            fails.push(format!("chamfer d=10 EXACTLY: ERROR {id}: {er}"));
        }
        let v = shapes.get(&f).map(|s| s.volume()).unwrap_or(0.0);
        fail_check(&mut fails, "box 10^3 chamfer d=10 (the boundary)", v, 500.0, 0.02);
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// A chamfer of EXACTLY d=2 on the edges of a 4^3 box — this needs the ladder of back-offs (~1e-5).
#[test]
fn matrix_chamfer_exact_on_small_cube() {
    let mut fails: Vec<String> = Vec::new();
    let mk = |p: &mut Project| -> u64 {
        p.new_document();
        let si = p.new_sketch("s");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "s");
        p.add_rect_entity(si, 0.0, 0.0, 4.0, 4.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let e = p.add_extrude_multi(sid, vec![cid], 4.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        p.finish_base_body(e, 1)
    };
    for (label, nedges, exp) in [("2 edges", 2usize, 64.0 - 16.0), ("all 4", 4, 64.0 - 32.0)] {
        let mut p = Project::default();
        let cube = mk(&mut p);
        let _ = qymcad_testkit::regenerate(&mut p);
        let verts: Vec<u32> = p
            .regen_edges
            .get(&cube)
            .map(|es| es.iter().filter(|e| (e.a[2] - e.b[2]).abs() > 2.0).map(|e| e.id).take(nedges).collect())
            .unwrap_or_default();
        if verts.len() != nedges {
            fails.push(format!("{label}: {} edges (expected {nedges})", verts.len()));
            continue;
        }
        let f = p.add_chamfer(cube, 2.0, verts);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        for (id, er) in &report.errors {
            fails.push(format!("{label}: ERROR {id}: {er}"));
        }
        let v = shapes.get(&f).map(|s| s.volume()).unwrap_or(0.0);
        fail_check(&mut fails, &format!("box 4^3 chamfer d=2 {label}"), v, exp, 0.02);
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// A 2 mm plate; ONE sketch holding a rectangle inside a rectangle; the outer one cut 1 mm deep (a
/// pocket), the inner one cut ALL THE WAY THROUGH. The step edges are 1 mm tall: a fillet of a
/// sensible radius (< 1 mm) MUST build; a radius larger than the geometry is an honest error, not
/// garbage.
#[test]
fn matrix_fillet_short_step_edges() {
    let mut fails: Vec<String> = Vec::new();
    let build = || -> (Project, u64) {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("base");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "base");
        p.add_rect_entity(si, 0.0, 0.0, 30.0, 30.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let e = p.add_extrude_multi(sid, vec![cid], 2.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let plate = p.finish_base_body(e, 1);
        // ONE sketch: a rectangle inside a rectangle
        let s2 = p.new_sketch("cuts");
        let sid2 = p.sketches[s2].id;
        p.add_sketch_node(sid2, "cuts");
        p.add_rect_entity(s2, 10.0, 10.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.add_rect_entity(s2, 13.0, 13.0, 17.0, 17.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(s2);
        let mut cc: Vec<(u64, f64)> = p.sketches[s2].contour_ids.iter().copied().filter_map(|c| Some((c, p.contours[p.contour_index(c)?].area()))).collect();
        cc.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let (inner, outer) = (cc[0].0, cc[1].0);
        // the outer one (the step ring) is a 1 mm pocket; the inner one goes all the way through
        // the sketch is on world XY (z=0 is the bottom of the plate), so the tool grows UP (flip=false)
        let cut1 = p.add_combine_multi_op(plate, sid2, vec![outer], 1.0, 0, qymcad_core::feature::Extent::default(), 0.0, vec![]);
        let cut2 = p.add_combine_multi_op(cut1, sid2, vec![inner], 2.0, 0, qymcad_core::feature::Extent { through: true, ..Default::default() }, 0.0, vec![]);
        (p, cut2)
    };
    // did the geometry build at all?
    {
        let (mut p, body) = build();
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        for (id, e) in &report.errors {
            fails.push(format!("build: ERROR {id}: {e}"));
        }
        let v = shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
        let exp = 30.0 * 30.0 * 2.0 - (100.0 - 16.0) * 1.0 - 16.0 * 2.0; // 1800 - 84 - 32
        fail_check(&mut fails, "stepped cut", v, exp, 0.02);
    }
    // filleting the short (1 mm) step edges. NOTE: the edges are CONCAVE (an inside corner of the
    // opening), so the fillet ADDS material (it fills the corner by about (4-pi)/4*r^2*len); the
    // limit here is HALF THE WIDTH of the opening (4 mm, so r<=2, the boundary reached via the eps
    // ladder), not the length of the edge. r=5 is impossible and must be an honest error.
    for (r, must_build) in [(0.4_f64, true), (0.8, true), (2.0, true), (5.0, false)] {
        let (mut p, body) = build();
        let _ = qymcad_testkit::regenerate(&mut p);
        // the vertical step edges: 1 mm long (z from 1 to 2), on the contour of the inner pocket
        let short: Vec<u32> = p
            .regen_edges
            .get(&body)
            .map(|es| es.iter().filter(|e| ((e.a[2] - e.b[2]).abs() - 1.0).abs() < 1e-3 && e.a[2].min(e.b[2]) > 0.5).map(|e| e.id).collect())
            .unwrap_or_default();
        if short.is_empty() {
            fails.push(format!("r={r}: the short step edges were not found"));
            continue;
        }
        let f = p.add_fillet(body, r, vec![short[0]]);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        let errored = report.errors.iter().any(|(id, _)| *id == f);
        let v = shapes.get(&f).map(|s| s.volume()).unwrap_or(0.0);
        let exp = 1684.0;
        if must_build {
            let fill = (4.0 - PI) / 4.0 * r * r; // filling the concave corner over a length of 1 mm
            if errored || (v - (exp + fill)).abs() > 1.5 {
                fails.push(format!("r={r} on a 1 mm edge MUST build (+{fill:.2}): err={errored}, V={v:.1}: {:?}", report.errors));
            }
        } else if !errored && (v > exp + 25.0 || v < 800.0) {
            fails.push(format!("r={r} (larger than the geometry): neither an honest error nor a valid body (V={v:.1})"));
        }
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// The direction of a revolve angle, and SYMMETRY. Profile x in [10,20] around Y, 180 degrees: by
/// default the material lands at z<=0 (+X goes to -Z, right-hand rule); flipped, at z>=0; symmetric,
/// evenly on both sides.
#[test]
fn matrix_revolve_direction_and_symmetry() {
    let mut fails: Vec<String> = Vec::new();
    // probe: material in the column z in [2..28], x in [-25..25], y in [0..30] on the side `sign`
    let probe = |s: &qymcad_kernel::Shape, sign: f64| -> f64 {
        let outer = qymcad_core::geom::Contour::closed(vec![
            qymcad_core::geom::Point2::new(-25.0, 0.0),
            qymcad_core::geom::Point2::new(25.0, 0.0),
            qymcad_core::geom::Point2::new(25.0, 30.0),
            qymcad_core::geom::Point2::new(-25.0, 30.0),
        ]);
        let prof = qymcad_core::geom::encode_profile(&outer, &[]);
        let bx = qymcad_kernel::Shape::extrude_profile(&prof, 26.0).unwrap();
        let mut place = qymcad_core::feature::PLACE_IDENTITY;
        // the profile lies in XY and is extruded along +Z, so it is shifted to z=2..28 on the
        // required side; the solid of revolution lives in coordinates (x, sketch y -> the Y axis, z)
        // and the probe is built in those same world axes
        place[11] = if sign > 0.0 { 2.0 } else { -28.0 };
        let bx = bx.transformed(&place).unwrap();
        s.boolean(&bx, 2).map(|c| c.volume()).unwrap_or(0.0)
    };
    use qymcad_core::feature::Reach;
    for (label, reach, want_neg, want_pos) in [
        ("default (-Z)", Reach::Forward, true, false),
        ("flipped (+Z)", Reach::Backward, false, true),
        ("both ways", Reach::BothWays, true, true),
    ] {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("s");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "s");
        p.add_rect_entity(si, 10.0, 0.0, 20.0, 30.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let body = p.add_revolve_axis_ex(sid, vec![cid], 1, 180.0, 0, 0, reach);
        let last = p.finish_base_body(body, 1);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        for (id, e) in &report.errors {
            fails.push(format!("{label}: ERROR {id}: {e}"));
        }
        let Some(s) = shapes.get(&last) else {
            fails.push(format!("{label}: no body"));
            continue;
        };
        let v = s.volume();
        let expect_v = std::f64::consts::PI * 300.0 * 30.0 / 2.0;
        if ((v - expect_v) / expect_v).abs() > 0.02 {
            fails.push(format!("{label}: V={v:.0} != {expect_v:.0}"));
        }
        let (vn, vp) = (probe(s, -1.0), probe(s, 1.0));
        let full = |x: f64| x > 3000.0;
        let empty = |x: f64| x < 50.0;
        if want_neg != full(vn) || want_pos != full(vp) || (!want_neg && !empty(vn)) || (!want_pos && !empty(vp)) {
            fails.push(format!("{label}: material -Z={vn:.0} +Z={vp:.0} (expected neg={want_neg} pos={want_pos})"));
        }
        if reach == Reach::BothWays && (vn - vp).abs() / vn.max(1.0) > 0.05 {
            fails.push(format!("{label}: not symmetric: {vn:.0} vs {vp:.0}"));
        }
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}
