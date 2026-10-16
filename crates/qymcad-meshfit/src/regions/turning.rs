//! REGIONS THAT TURN ABOUT AN AXIS: coaxial, straight, helical, round.

use super::*;

/// THE AXIS A SURFACE OF REVOLUTION SHARES WITH ITS NEIGHBOURS.
///
/// A band a few triangles wide - the flank of a thread drawn as rings - does not show its own axis: its normals barely
/// part along it, and a sphere through its two edges fits its corners as closely as the true cone does (two coaxial
/// circles always lie on one sphere). Measured on a ring thread from our kernel, ten rings and 43 faces: every crest
/// and root came as one cylinder, and every flank came apart into 117 - 169 regions of cylinders, planes and spheres.
/// So the axes of the cylinders found are taken as axes the regions around them may turn about: neighbours whose
/// corners, seen as (height along the axis, distance from it), lie on one straight line join as a cone - or a
/// cylinder, or a plane - about that axis. A straight line is taken before a curve: with no corner between a band's
/// two edges nothing shows it bends. A sphere or a torus about the axis lies on no straight line and stays as it is.
pub(crate) fn coaxial(g: &Grower, out: &mut [Region], owner: &mut [u32]) {
    for (on, a) in &cylinder_axes(g, out) {
        // THE FAMILY OF THE AXIS, flooded from its cylinders through the regions beside them - across any edge, a
        // sharp one too - that lie on a straight line about it on their own. Measured on the tensioner screw (386 924
        // triangles): trying every region against its neighbours for each of the axes took the split from 10.7 s to
        // 808 s; the family of a thread's axis is the thread.
        let family = axis_family(g, out, owner, on, a, |tris| straight_about(g, tris, on, a).is_some());
        // A MEMBER OF THE FAMILY BENT BY ITS OWN FIT TAKES THE STRAIGHT SURFACE IT LIES ON. A band whose triangles run
        // rim to rim has its corners on two circles about the axis, and any two such circles lie on a sphere as exactly
        // as on a cone: a chamfer of 0.5 at 45 deg on a washer's rim came in as a sphere of 13.79, and across its sharp
        // edges nothing joined it to be refitted. Lying on a straight line about the axis, it is the cone.
        for &r in &family {
            if matches!(out[r].surface, Some(Surface::Sphere { .. } | Surface::Torus { .. })) {
                if let Some(straight) = straight_about(g, &out[r].tris, on, a) {
                    out[r].surface = Some(straight);
                }
            }
        }
        // neighbours of the family across an edge that is not sharp join, the widest first: at once where the one
        // lies on the other's surface already, and otherwise where their union lies on one straight line
        loop {
            let mut joined = false;
            let mut order: Vec<usize> = family.iter().copied().filter(|&r| !out[r].tris.is_empty()).collect();
            order.sort_by_key(|&r| (std::cmp::Reverse(out[r].tris.len()), r));
            for ra in order {
                'more: loop {
                    if out[ra].tris.is_empty() {
                        break;
                    }
                    let mut near: Vec<usize> = Vec::new();
                    for &t in &out[ra].tris {
                        for &u in &g.p.neighbours[t as usize] {
                            if u == NO_NEIGHBOUR || g.sharp(t as usize, u as usize) {
                                continue;
                            }
                            let rb = owner[u as usize] as usize;
                            if rb != ra && family.contains(&rb) && !near.contains(&rb) {
                                near.push(rb);
                            }
                        }
                    }
                    for rb in near {
                        let fits_as_is = out[ra].surface.as_ref().filter(|s| straight_kind(s)).is_some_and(|s| out[rb].tris.iter().all(|&t| g.fits(t as usize, s)));
                        let union: Vec<u32> = out[ra].tris.iter().chain(&out[rb].tris).copied().collect();
                        let s = if fits_as_is { out[ra].surface.clone() } else { straight_about(g, &union, on, a) };
                        let Some(s) = s else { continue };
                        for &t in &out[rb].tris {
                            owner[t as usize] = ra as u32;
                        }
                        out[rb].tris.clear();
                        out[rb].surface = None;
                        out[ra].tris = union;
                        out[ra].surface = Some(s);
                        joined = true;
                        continue 'more;
                    }
                    break;
                }
            }
            if !joined {
                break;
            }
        }
    }
}

/// The axes of the cylinders found, one per line, the widest cylinder's first: two lie on one line when they are parallel
/// to a thousandth of a radian and a point of one is within ten distances of the other.
pub(crate) fn cylinder_axes(g: &Grower, out: &[Region]) -> Vec<(Vector3<f64>, Vector3<f64>)> {
    let mut axes: Vec<(Vector3<f64>, Vector3<f64>)> = Vec::new();
    let mut cylinders: Vec<&Region> = out.iter().filter(|r| r.tris.len() >= MERGE_FROM && matches!(r.surface, Some(Surface::Cylinder { .. }))).collect();
    cylinders.sort_by_key(|r| std::cmp::Reverse(r.tris.len()));
    for r in cylinders {
        let Some(Surface::Cylinder { point, axis, .. }) = r.surface else { continue };
        let (p, a) = (Vector3::from(point), Vector3::from(axis));
        let known = axes.iter().any(|(q, b)| a.cross(b).norm() < 1e-3 && ((p - q) - b * (p - q).dot(b)).norm() < 10.0 * g.tol.distance);
        if !known {
            axes.push((p, a));
        }
    }
    axes
}

/// The regions flooded from the cylinders on the axis through `on` along `a`, across any edge, through the regions that
/// `belongs` takes.
pub(crate) fn axis_family(g: &Grower, out: &[Region], owner: &[u32], on: &Vector3<f64>, a: &Vector3<f64>, belongs: impl Fn(&[u32]) -> bool) -> HashSet<usize> {
    let on_axis = |r: &Region| match r.surface {
        Some(Surface::Cylinder { point, axis, .. }) => {
            let (p, b) = (Vector3::from(point), Vector3::from(axis));
            b.cross(a).norm() < 1e-3 && ((p - on) - a * (p - on).dot(a)).norm() < 10.0 * g.tol.distance
        }
        _ => false,
    };
    let mut family: HashSet<usize> = (0..out.len()).filter(|&r| out[r].tris.len() >= MERGE_FROM && on_axis(&out[r])).collect();
    let mut looked: HashSet<usize> = family.clone();
    let mut queue: VecDeque<usize> = family.iter().copied().collect();
    while let Some(r) = queue.pop_front() {
        let mut next: Vec<usize> = Vec::new();
        for &t in &out[r].tris {
            for &u in &g.p.neighbours[t as usize] {
                if u != NO_NEIGHBOUR && !looked.contains(&(owner[u as usize] as usize)) && !next.contains(&(owner[u as usize] as usize)) {
                    next.push(owner[u as usize] as usize);
                }
            }
        }
        for rb in next {
            looked.insert(rb);
            if belongs(&out[rb].tris) {
                family.insert(rb);
                queue.push_back(rb);
            }
        }
    }
    family
}

/// Whether the corners of `tris` go more than half way round the axis through `on` along `a`: no gap between their angles
/// round it is as wide as half a turn.
pub(crate) fn goes_round(g: &Grower, tris: &[u32], on: &Vector3<f64>, a: &Vector3<f64>) -> bool {
    let (x, y) = across(a);
    let mut angles: Vec<f64> = g.corners_of(tris).iter().map(|c| (c - on).dot(&y).atan2((c - on).dot(&x))).collect();
    angles.sort_by(f64::total_cmp);
    let (Some(&first), Some(&last)) = (angles.first(), angles.last()) else { return false };
    let widest = angles.windows(2).map(|w| w[1] - w[0]).fold(first + 2.0 * std::f64::consts::PI - last, f64::max);
    widest < std::f64::consts::PI
}

/// Whether `s` is what a straight line about an axis gives: a cone, a cylinder or a plane.
pub(crate) fn straight_kind(s: &Surface) -> bool {
    matches!(s, Surface::Cone { .. } | Surface::Cylinder { .. } | Surface::Plane { .. })
}

/// The cone, cylinder or plane about the axis through `on` along `a` that the corners of `tris` lie on: their (height,
/// distance) points on one straight line - the principal direction of their scatter - each within the distance of it,
/// which is the corner's distance from the surface itself; and every triangle's normal agreeing. `None` otherwise.
pub(crate) fn straight_about(g: &Grower, tris: &[u32], on: &Vector3<f64>, a: &Vector3<f64>) -> Option<Surface> {
    let zr = profile(&g.corners_of(tris), a, on);
    let n = zr.len() as f64;
    if zr.len() < 3 {
        return None;
    }
    let (mz, mr) = (zr.iter().map(|x| x.0).sum::<f64>() / n, zr.iter().map(|x| x.1).sum::<f64>() / n);
    let (szz, szr, srr) = zr.iter().fold((0.0, 0.0, 0.0), |(zz, zrr, rr), &(z, r)| (zz + (z - mz).powi(2), zrr + (z - mz) * (r - mr), rr + (r - mr).powi(2)));
    let theta = 0.5 * (2.0 * szr).atan2(szz - srr);
    let (dz, dr) = (theta.cos(), theta.sin());
    if zr.iter().any(|&(z, r)| ((z - mz) * dr - (r - mr) * dz).abs() > g.tol.distance) {
        return None;
    }
    // A SLOPE THE REGION CANNOT SHOW IS NO SLOPE. The line of a cylinder's wall came out at 1e-6 rather than zero, and
    // the wall - measured under a sphere set 0.1 mm off its axis - stood as four cones opening from 1e7 mm below it, each
    // a cone of its own that no neighbour joined; across the region's height its radius changed by a thousandth of the
    // distance tolerance.
    let zs = zr.iter().fold(0.0f64, |zs, &(z, _)| zs.max((z - mz).abs()));
    let s = if dr.abs() * zs <= g.tol.distance * dz.abs() {
        Surface::Cylinder { point: (*on).into(), axis: (*a).into(), radius: mr }
    } else if dz.abs() <= 1e-9 * dr.abs() {
        Surface::Plane { point: (on + a * mz).into(), normal: (*a).into() }
    } else {
        // the apex where the line meets the axis, the cone opening from it towards the corners
        let z0 = mz - mr * dz / dr;
        let into = if mz > z0 { *a } else { -a };
        Surface::Cone { apex: (on + a * z0).into(), axis: into.into(), half_angle: (dr / dz).abs().atan() }
    };
    tris.iter().all(|&t| g.fits(t as usize, &s)).then_some(s)
}

/// THE FLANK OF A THREAD IS ONE HELIX, NOT A MOSAIC. A profile screwed about an axis is no surface of revolution, and its
/// flank came apart into tiles each within the tolerance of a small cylinder, plane or sphere: measured on an M10 x 1.5
/// cut by our kernel along 6 mm, 1 737 regions and 2 340 faces. Every face of such a thread is a straight piece of the
/// profile screwed along - the crest and the root cylinders about the axis the flanks turn round. So the axes of the
/// cylinders found are taken again, and the regions around them that lie on a helix about one are joined across edges
/// that are not sharp while their union lies on one helix, every triangle on it.
pub(crate) fn helical(g: &Grower, out: &mut [Region], owner: &mut [u32]) {
    // A THREAD'S AXIS IS SEEN ROUND: its crest or root goes all round it. An organic mesh's small cylinders give axes
    // of their own by the thousand - 8 003 on a 225 154-triangle one, and trying each took its split from 9.3 s to 21.3 s.
    let round: Vec<(Vector3<f64>, Vector3<f64>)> = cylinder_axes(g, out)
        .into_iter()
        .filter(|(on, a)| {
            out.iter().any(|r| match r.surface {
                Some(Surface::Cylinder { point, axis, .. }) if r.tris.len() >= MERGE_FROM => {
                    let (p, b) = (Vector3::from(point), Vector3::from(axis));
                    b.cross(a).norm() < 1e-3 && ((p - on) - a * (p - on).dot(a)).norm() < 10.0 * g.tol.distance && goes_round(g, &r.tris, on, a)
                }
                _ => false,
            })
        })
        .collect();
    for (on, a) in &round {
        // a region too small to show a helix of its own lets the flood through and is joined by its neighbour's
        let family = axis_family(g, out, owner, on, a, |tris| tris.len() <= 2 || straight_about(g, tris, on, a).is_some() || helix_about(g, tris, on, a, None).is_some());
        // the joins are made on a copy, each region keeping which regions it took
        let (mut work, mut whose) = (out.to_vec(), owner.to_vec());
        let mut took: Vec<Vec<usize>> = (0..out.len()).map(|r| vec![r]).collect();
        loop {
            let mut joined = false;
            let mut order: Vec<usize> = family.iter().copied().filter(|&r| work[r].tris.len() > 2).collect();
            order.sort_by_key(|&r| (std::cmp::Reverse(work[r].tris.len()), r));
            for ra in order {
                // A NEIGHBOUR TURNED DOWN GETS ITS SECOND CHANCE IN THE NEXT PASS, not after every join: tried again each
                // time - and a circle refined for each - an auger's split took 28 s, once a pass 1.8 s
                let mut tried: HashSet<usize> = HashSet::new();
                'more: loop {
                    if work[ra].tris.is_empty() {
                        break;
                    }
                    let own = match work[ra].surface {
                        Some(Surface::Helix { .. } | Surface::RoundHelix { .. }) => work[ra].surface.clone(),
                        _ => helix_about(g, &work[ra].tris, on, a, None),
                    };
                    let Some(own) = own else { break };
                    let mut near: Vec<usize> = Vec::new();
                    for &t in &work[ra].tris {
                        for &u in &g.p.neighbours[t as usize] {
                            if u == NO_NEIGHBOUR || g.sharp(t as usize, u as usize) {
                                continue;
                            }
                            let rb = whose[u as usize] as usize;
                            if rb != ra && family.contains(&rb) && !near.contains(&rb) && !tried.contains(&rb) {
                                near.push(rb);
                            }
                        }
                    }
                    for rb in near {
                        tried.insert(rb);
                        let union: Vec<u32> = work[ra].tris.iter().chain(&work[rb].tris).copied().collect();
                        let s = if work[rb].tris.iter().all(|&t| g.fits(t as usize, &own)) { Some(own.clone()) } else { helix_about(g, &union, on, a, Some(&own)) };
                        let Some(s) = s else { continue };
                        for &t in &work[rb].tris {
                            whose[t as usize] = ra as u32;
                        }
                        work[rb].tris.clear();
                        work[rb].surface = None;
                        work[ra].tris = union;
                        work[ra].surface = Some(s);
                        let moved = std::mem::take(&mut took[rb]);
                        took[ra].extend(moved);
                        joined = true;
                        continue 'more;
                    }
                    break;
                }
            }
            if !joined {
                break;
            }
        }
        // A SCREW IS SEEN BY TURNING: a helix is taken where it goes a whole turn round its axis at the least. A smooth
        // loft's tiles lie on helices too, about the axes of small cylinders among them: measured through squares of 10,
        // 6 and 10, 32 such helices of 11 - 88 triangles, each spanning 0.03 - 0.13 rad, and the walls no longer came
        // together as free forms; on an airplane's mesh one of half a turn took tiles of its free forms. A thread's flank
        // spans all its turns - 25 rad along 6 mm of an M10 x 1.5.
        for r in 0..work.len() {
            let Some(h @ (Surface::Helix { .. } | Surface::RoundHelix { .. })) = &work[r].surface else { continue };
            let (lo, hi) = g.corners_of(&work[r].tris).iter().filter_map(|c| h.turn_at([c.x, c.y, c.z])).fold((f64::MAX, f64::MIN), |(lo, hi), (t, _)| (lo.min(t), hi.max(t)));
            if hi - lo < 2.0 * std::f64::consts::PI {
                continue;
            }
            for &m in &took[r] {
                if m != r {
                    out[m].tris.clear();
                    out[m].surface = None;
                }
            }
            for &t in &work[r].tris {
                owner[t as usize] = r as u32;
            }
            out[r] = work[r].clone();
        }
    }
}

/// The helix about the axis through `on` along `a` that every triangle of `tris` lies on, or `None`.
///
/// Its line in (r, z, t) - distance, height and angle round the axis - is fitted to the corners by total least squares:
/// radial * r + axial * z - axial * rise * t = offset with (radial, axial) of unit length. Each corner's angle takes the
/// whole turns that bring it onto `guide`, the helix of a part of the union - a flank runs many turns. With no guide the
/// corners are taken within half a turn of the first: the pieces tried so are a few triangles. A helix whose rise the
/// corners cannot show across their angles is a cone or a plane.
pub(crate) fn helix_about(g: &Grower, tris: &[u32], on: &Vector3<f64>, a: &Vector3<f64>, guide: Option<&Surface>) -> Option<Surface> {
    let (x, y) = across(a);
    let pts = g.corners_of(tris);
    if pts.len() < 4 {
        return None;
    }
    let tau = 2.0 * std::f64::consts::PI;
    let mut rzt: Vec<(f64, f64, f64)> = pts
        .iter()
        .map(|p| {
            let d = p - on;
            let z = d.dot(a);
            let e = d - a * z;
            (e.norm(), z, e.dot(&y).atan2(e.dot(&x)))
        })
        .collect();
    match guide {
        Some(h @ (Surface::Helix { .. } | Surface::RoundHelix { .. })) => {
            for (c, p) in rzt.iter_mut().zip(&pts) {
                c.2 = helix_at(h, p)?.turn;
            }
        }
        _ => {
            let t0 = rzt[0].2;
            for c in &mut rzt {
                c.2 = t0 + (c.2 - t0 + std::f64::consts::PI).rem_euclid(tau) - std::f64::consts::PI;
            }
        }
    }
    let n = rzt.len() as f64;
    let (mr, mz, mt) = rzt.iter().fold((0.0, 0.0, 0.0), |m, c| (m.0 + c.0 / n, m.1 + c.1 / n, m.2 + c.2 / n));
    let (mut srr, mut srz, mut szz, mut srt, mut szt, mut stt) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for &(r, z, t) in &rzt {
        let (r, z, t) = (r - mr, z - mz, t - mt);
        srr += r * r;
        srz += r * z;
        szz += z * z;
        srt += r * t;
        szt += z * t;
        stt += t * t;
    }
    // the angle the corners span shows the screw; along a hair of it any rise fits
    if stt < n * 1e-6 {
        return None;
    }
    // the rise taken out, what is left of the scatter of (r, z): its least direction is the profile's normal
    let m = nalgebra::Matrix2::new(srr - srt * srt / stt, srz - srt * szt / stt, srz - srt * szt / stt, szz - szt * szt / stt);
    let e = nalgebra::SymmetricEigen::new(m);
    let least = if e.eigenvalues[0] <= e.eigenvalues[1] { 0 } else { 1 };
    let (mut radial, mut axial) = (e.eigenvectors[(0, least)], e.eigenvectors[(1, least)]);
    if axial < 0.0 {
        (radial, axial) = (-radial, -axial);
    }
    let (lo, hi) = rzt.iter().fold((f64::MAX, f64::MIN), |(lo, hi), c| (lo.min(c.2), hi.max(c.2)));
    if axial > 1e-9 {
        let turned = (radial * srt + axial * szt) / stt;
        let rise = turned / axial;
        // A HELIX WHOSE TURNS LIE ON ONE ANOTHER IS A CYLINDER: its turns stand 2 pi x axial x rise apart across it, and
        // a loft's wall came as one with axial 9e-9 and a rise of 354 - turns 2e-5 mm apart, and 716 rad to a few triangles
        if (rise * (hi - lo)).abs() > g.tol.distance && tau * (axial * rise).abs() > 10.0 * g.tol.distance {
            let s = Surface::Helix { point: (*on).into(), axis: (*a).into(), reference: x.into(), rise, radial, axial, offset: radial * mr + axial * mz - turned * mt };
            if tris.iter().all(|&t| g.fits(t as usize, &s)) {
                return Some(s);
            }
        }
    }
    // a union grown on a straight helix stays straight: refining a circle for every neighbour turned down took an auger's
    // split from 1.8 s to 2.2 s
    if matches!(guide, Some(Surface::Helix { .. })) {
        return None;
    }
    round_about(g, tris, &pts, &rzt, on, a, guide)
}

/// THE PROFILE MAY BE A CIRCLE. An auger's flight rounded by 0.8 our kernel draws in five straight pieces, but the mesh's
/// corners lie on the circle through them, and its pieces of a few triangles each took two of them: measured, they
/// missed every straight helix by 0.023 mm. The rise is the guide's, or the one the triangles' normals show - a screwed
/// surface's normal stands square to the screw's motion, a x (v - on) + rise a; the corners less the rise are fitted a
/// circle in (r, z - rise * t) algebraically, and all four numbers refined on the corners.
pub(crate) fn round_about(g: &Grower, tris: &[u32], pts: &[Vector3<f64>], rzt: &[(f64, f64, f64)], on: &Vector3<f64>, a: &Vector3<f64>, guide: Option<&Surface>) -> Option<Surface> {
    if pts.len() < 6 {
        return None;
    }
    let (lo, hi) = rzt.iter().fold((f64::MAX, f64::MIN), |(lo, hi), c| (lo.min(c.2), hi.max(c.2)));
    let span = hi - lo;
    let tau = 2.0 * std::f64::consts::PI;
    let rise = match guide {
        Some(Surface::Helix { rise, .. } | Surface::RoundHelix { rise, .. }) => *rise,
        _ => {
            let (mut turned, mut along) = (0.0, 0.0);
            for &t in tris {
                let t = t as usize;
                if g.sliver[t] {
                    continue;
                }
                let (n, w) = (g.normal(t), g.area(t));
                turned += w * n.dot(a) * n.dot(&a.cross(&(g.centroid(t) - on)));
                along += w * n.dot(a) * n.dot(a);
            }
            if along < 1e-12 {
                return None;
            }
            -turned / along
        }
    };
    let pitch = tau * rise;
    if (rise * span).abs() <= g.tol.distance || pitch.abs() <= 10.0 * g.tol.distance {
        return None;
    }
    let first = rzt[0].1 - rise * rzt[0].2;
    let profile: Vec<(f64, f64)> = rzt
        .iter()
        .map(|&(r, z, t)| {
            let lifted = z - rise * t;
            (r, lifted - pitch * ((lifted - first) / pitch).round())
        })
        .collect();
    let (mut m, mut v) = (Matrix3::<f64>::zeros(), Vector3::<f64>::zeros());
    for &(r, q) in &profile {
        let row = Vector3::new(r, q, 1.0);
        m += row * row.transpose();
        v -= row * (r * r + q * q);
    }
    let k = m.lu().solve(&v)?;
    let (middle, height) = (-k.x / 2.0, -k.y / 2.0);
    let squared = middle * middle + height * height - k.z;
    if squared.is_nan() || squared <= 0.0 {
        return None;
    }

    let x = across(a).0;
    let make = |q: &[f64; 4]| Surface::RoundHelix { point: (*on).into(), axis: (*a).into(), reference: x.into(), rise: q[0], middle: q[1], height: q[2], round: q[3] };
    // the miss and its derivatives written out: off = (apart - round) / |normal|, apart the distance from the circle's
    // middle in (r, z - rise * t); the slow change of |normal| with the numbers is left out, which Gauss-Newton bears.
    // By small steps each miss cost nine, and a mesh of hearts and gears took 1.20 s to split against 0.84 s.
    let miss = |q: &[f64; 4], v: &Vector3<f64>| {
        let Some(h) = helix_at(&make(q), v) else { return (0.0, [0.0; 4]) };
        let d = v - on;
        let z = d.dot(a);
        let far = (d - a * z).norm();
        let (dr, dz) = (far - q[1], z - q[0] * h.turn - q[2]);
        let apart = dr.hypot(dz).max(1e-300);
        let scale = 1.0 / h.normal.norm().max(1e-300);
        (h.off, [-dz / apart * h.turn * scale, -dr / apart * scale, -dz / apart * scale, -scale])
    };
    let mut q = [rise, middle, height, squared.sqrt()];
    refine(&mut q, pts, &miss, 20, 600, None, g.tol.distance * 0.1);
    // a circle within the pitch and inside its distance from the axis, the tolerance at the least
    if !(q[3] > g.tol.distance && 2.0 * q[3] < (tau * q[0]).abs() && q[3] < q[1]) {
        return None;
    }
    let s = make(&q);
    tris.iter().all(|&t| g.fits(t as usize, &s)).then_some(s)
}

/// How few triangles a region may have and still be joined. Measured on a 225 154-triangle organic mesh: most of its
/// 28 000 regions are of three to five, none of them lies on a torus or a cone with a neighbour, and trying them all
/// took the joins to 95 s; a band of a torus from a print program is some forty.
pub(crate) const MERGE_FROM: usize = 8;

/// Whether two surfaces bend alike: the radii of two bands of one torus - a cylinder's and a sphere's - differ by a
/// hundredth, of an organic mesh's neighbours by anything. A cone bends along its length, so it is always tried.
pub(crate) fn alike(a: &Option<Surface>, b: &Option<Surface>) -> bool {
    let bend = |s: &Option<Surface>| match s {
        Some(Surface::Cylinder { radius, .. }) | Some(Surface::Sphere { radius, .. }) => Some(*radius),
        Some(Surface::Torus { minor, .. }) => Some(*minor),
        _ => None,
    };
    match (bend(a), bend(b)) {
        (Some(x), Some(y)) => x.max(y) <= 1.25 * x.min(y),
        _ => true,
    }
}
