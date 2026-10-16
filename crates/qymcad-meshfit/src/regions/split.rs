//! A MESH SPLIT INTO REGIONS: grown apart, walls freed, mosaics, free forms, merged and absorbed.

use super::*;

/// Split `p` into regions. Every triangle ends in exactly one.
pub fn regions(p: &Prepared, tol: &Tolerance) -> Vec<Region> {
    regions_until(p, tol, &|| false).unwrap_or_default()
}

/// The same, asking `stop` before every seed: `None` once it says yes. Growing the regions is most of a recognition
/// (11 s of 14 s on a 160k-triangle ball in a debug build), and work nobody waits for any more is dropped at once.
pub fn regions_until(p: &Prepared, tol: &Tolerance, stop: &dyn Fn() -> bool) -> Option<Vec<Region>> {
    let n = p.mesh.tris.len();
    let mut sliver = vec![false; n];
    for &s in &p.slivers {
        sliver[s as usize] = true;
    }
    let g = Grower { p, tol, cos_sharp: tol.sharp_deg.to_radians().cos(), cos_angle: tol.angle_deg.to_radians().cos(), sliver: &sliver };
    let area: Vec<f64> = (0..n).map(|t| g.area(t)).collect();
    // THE LARGEST TRIANGLES SEED FIRST: a plane face is often a couple of large triangles, and a surface is guessed
    // best where it is sampled widest
    let mut order: Vec<usize> = (0..n).filter(|&t| !sliver[t]).collect();
    order.sort_by(|&a, &b| area[b].total_cmp(&area[a]).then(a.cmp(&b)));
    let mut owner = vec![NONE; n];
    let mut out: Vec<Region> = Vec::new();
    for seed in order {
        if owner[seed] != NONE {
            continue;
        }
        if stop() {
            return None;
        }
        let mut best: Option<(f64, Vec<u32>, Surface)> = None;
        for kind in [Kind::Plane, Kind::Cylinder, Kind::Cone, Kind::Sphere] {
            let mut widest: Option<(f64, Vec<u32>, Surface)> = None;
            for s in g.guesses(seed, kind, &owner) {
                let Some((tris, s)) = g.grow(seed, s, kind, &owner) else { continue };
                let covered: f64 = tris.iter().map(|&t| area[t as usize]).sum();
                if widest.as_ref().is_none_or(|w| covered > w.0 * (1.0 + 1e-9)) {
                    widest = Some((covered, tris, s));
                }
            }
            // A MORE INVOLVED SURFACE HAS TO EARN ITS PLACE. Corners are all that is checked, and a plane face of
            // two triangles has only four: measured on the rounded bar, a 114 mm cylinder through them and the two
            // tangent strips beside them covered 3% more than the face itself and took it. A rounding's cylinder
            // covers eleven times the plane of one of its strips.
            if let Some(w) = widest {
                if best.as_ref().is_none_or(|b| w.0 > b.0 * SIMPLER_WINS) {
                    best = Some(w);
                }
            }
        }
        let r = out.len() as u32;
        match best {
            Some((_, tris, s)) => {
                for &t in &tris {
                    owner[t as usize] = r;
                }
                out.push(Region { tris, surface: Some(s) });
            }
            None => {
                owner[seed] = r;
                out.push(Region { tris: vec![seed as u32], surface: None });
            }
        }
    }
    if stop() {
        return None;
    }
    merge(&g, &mut out, &mut owner);
    absorb(&g, &mut out, &mut owner);
    coaxial(&g, &mut out, &mut owner);
    // THE PIECES TURNED ABOUT ONE AXIS ARE JOINED BY A FIT OF THEIR UNION, not by the axis they were turned about: that
    // axis is some one region's, and measured under a sphere set 0.1 mm off a cylinder's axis it stood 0.0009 mm off
    // the true one, so the four pieces of the wall took radii from 9.9993 to 10.0007 - further apart than the
    // tolerance of 0.0005 - and none of them lay on another's surface.
    merge(&g, &mut out, &mut owner);
    absorb(&g, &mut out, &mut owner);
    helical(&g, &mut out, &mut owner);
    absorb(&g, &mut out, &mut owner);
    // A SLIVER HAS NO NORMAL TO AGREE WITH: it joins a neighbouring region whose surface its corners lie on, or
    // stands alone, unrecognised
    for &s in &p.slivers {
        let s = s as usize;
        let home = p.neighbours[s].iter().filter(|&&u| u != NO_NEIGHBOUR).map(|&u| owner[u as usize]).find(|&r| r != NONE && out[r as usize].surface.as_ref().is_some_and(|f| g.corners_on(s, f)));
        match home {
            Some(r) => {
                owner[s] = r;
                out[r as usize].tris.push(s as u32);
            }
            None => {
                owner[s] = out.len() as u32;
                out.push(Region { tris: vec![s as u32], surface: None });
            }
        }
    }
    coiled(&g, &mut out, &mut owner);
    free_form(&g, &mut out, &mut owner);
    unwind(&g, &mut out, &mut owner);
    if stop() {
        return None;
    }
    free_walls(&g, &mut out, &mut owner);
    out.retain(|r| !r.tris.is_empty());
    Some(apart(&g, out))
}

/// EVERY REGION IN ONE PIECE: a region whose triangles fall apart - the pieces of one cylinder joined across a slot, the
/// parts of a wall on either side of a hole - becomes one region per piece, each on the same surface. A face has one
/// outer loop, and one built on the loops of several pieces was refused: measured on a simplified handle, 10 walls and
/// 3 cylinders of it, up to 10 pieces side by side, and the body not sound until mended.
///
/// A CRUMB FALLEN OFF - under `CRUMB` triangles, not its region's largest piece or closed round by one piece - joins
/// the piece beside it that shares most of its sides, where that one's surface holds its corners: standing alone, 84 such bits of
/// cylinders of the handle made no face and went back as 468 triangles of mesh.
pub(crate) fn apart(g: &Grower, out: Vec<Region>) -> Vec<Region> {
    let p = g.p;
    let mut region_of = vec![u32::MAX; p.mesh.tris.len()];
    for (r, region) in out.iter().enumerate() {
        for &t in &region.tris {
            region_of[t as usize] = r as u32;
        }
    }
    let mut seen = vec![false; p.mesh.tris.len()];
    let mut pieces: Vec<Region> = Vec::with_capacity(out.len());
    let mut largest: Vec<bool> = Vec::with_capacity(out.len());
    for (r, region) in out.into_iter().enumerate() {
        let first = pieces.len();
        for &s in &region.tris {
            if seen[s as usize] {
                continue;
            }
            seen[s as usize] = true;
            let (mut piece, mut queue) = (vec![s], VecDeque::from([s]));
            while let Some(t) = queue.pop_front() {
                for &u in &p.neighbours[t as usize] {
                    if u != NO_NEIGHBOUR && region_of[u as usize] == r as u32 && !seen[u as usize] {
                        seen[u as usize] = true;
                        piece.push(u);
                        queue.push_back(u);
                    }
                }
            }
            pieces.push(Region { tris: piece, surface: region.surface.clone() });
            largest.push(false);
        }
        if let Some(big) = (first..pieces.len()).max_by_key(|&k| pieces[k].tris.len()) {
            largest[big] = true;
        }
    }
    // the crumbs to their neighbours
    let mut piece_of = vec![u32::MAX; p.mesh.tris.len()];
    for (k, piece) in pieces.iter().enumerate() {
        for &t in &piece.tris {
            piece_of[t as usize] = k as u32;
        }
    }
    for k in 0..pieces.len() {
        if pieces[k].tris.len() >= CRUMB {
            continue;
        }
        let mut sides: HashMap<u32, usize> = HashMap::new();
        for &t in &pieces[k].tris {
            for &u in &p.neighbours[t as usize] {
                if u != NO_NEIGHBOUR && piece_of[u as usize] != k as u32 && piece_of[u as usize] != u32::MAX {
                    *sides.entry(piece_of[u as usize]).or_default() += 1;
                }
            }
        }
        // a whole region of a few triangles stays itself, unless one piece closes round it: a hole of a sliver in a wall
        // is a loop of no area, and the wall's face was refused - two walls of 240 triangles of the handle went back
        // to the mesh
        if largest[k] && sides.len() != 1 {
            continue;
        }
        let mut by: Vec<(u32, usize)> = sides.into_iter().collect();
        by.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        // on a fitted wall, within its flattening and near it: past the square it was fitted over the surface runs off
        // wild - a crumb joined to a wall beyond it put a corner 3.9 of the wall's widths away, where the surface stood
        // 1 155 mm off, and the wall made no face
        let on_wall = |t: u32, w: &crate::bspline::BSpline| {
            (0..3).all(|c| {
                let q = g.corner(t as usize, c);
                let (u, v) = w.uv_of([q.x, q.y, q.z]);
                let at = w.at(u, v);
                (-0.02..=1.02).contains(&u) && (-0.02..=1.02).contains(&v) && ((at[0] - q.x).powi(2) + (at[1] - q.y).powi(2) + (at[2] - q.z).powi(2)).sqrt() <= 10.0 * g.tol.distance
            })
        };
        let home = by.into_iter().map(|(h, _)| h as usize).find(|&h| match &pieces[h].surface {
            Some(Surface::Spline(w)) => pieces[k].tris.iter().all(|&t| on_wall(t, w)),
            Some(s) => pieces[k].tris.iter().all(|&t| g.corners_on(t as usize, s)),
            None => false,
        });
        if let Some(h) = home {
            let moved = std::mem::take(&mut pieces[k].tris);
            for &t in &moved {
                piece_of[t as usize] = h as u32;
            }
            pieces[h].tris.extend(moved);
        }
    }
    pieces.retain(|r| !r.tris.is_empty());
    pieces
}

/// Under how many triangles a piece fallen off its region is a crumb.
pub(crate) const CRUMB: usize = 6;

/// How few triangles make a wall worth its own surface.
pub(crate) const WALL_FROM: usize = 30;

/// A SMOOTH WALL IS ONE SURFACE, NOT A MOSAIC. Where no plane, cylinder, cone, sphere or torus lies on a smooth wall, the
/// seeds grow each of them over a strip of it, as far as it stays within the tolerance: measured on a 178 032-triangle
/// mesh of a smooth handle, 1 887 strips of cylinders and 1 061 regions of none, the body built of 54 825 faces in 530 s.
/// So the tiles of such a wall - curved regions, small planes and triangles no surface took - joined across edges that
/// are not sharp, are cut into pieces that face one way, and on each piece one B-spline surface is fitted to its corners;
/// a piece it holds within the tolerance, replacing two tiles or more, becomes one region on it. A piece it does not hold
/// is halved until each part holds.
pub(crate) fn free_walls(g: &Grower, out: &mut Vec<Region>, owner: &mut [u32]) {
    let tile = |r: &Region| match &r.surface {
        None => true,
        Some(Surface::Plane { .. }) => r.tris.len() <= TILE_UP_TO,
        Some(Surface::Cylinder { .. } | Surface::Cone { .. } | Surface::Sphere { .. } | Surface::Torus { .. }) => true,
        Some(_) => false,
    };
    let n = g.p.mesh.tris.len();
    let in_wall: Vec<bool> = (0..n).map(|t| owner[t] != NONE && tile(&out[owner[t] as usize]) && !g.sliver[t]).collect();
    let mut taken = vec![false; n];
    let mut order: Vec<usize> = (0..n).filter(|&t| in_wall[t]).collect();
    order.sort_by(|&a, &b| g.area(b).total_cmp(&g.area(a)).then(a.cmp(&b)));
    let sag = crate::prep::chord_deflection(g.p, g.tol.sharp_deg);
    let cos_turn = WALL_TURN_DEG.to_radians().cos();
    for seed in order {
        if taken[seed] {
            continue;
        }
        // PIECES OF A WALL THAT FACE ONE WAY, grown from its biggest triangles while each faces within `WALL_TURN_DEG` of
        // the mean so far. Taken whole instead, a smooth handle's walls fitted as they stood, and the faces of the
        // cylinders beside them came out of the mending with copies of their edges that nothing else shared: 33 sides
        // left for the sewing, and the body refused.
        let piece = grow(g, seed, &in_wall, &mut taken, cos_turn);
        // A PIECE NO ONE SURFACE HOLDS IS HALVED, and each half fitted again, until every part holds or is too small to be
        // worth a surface of its own.
        let mut work = vec![piece];
        while let Some(piece) = work.pop() {
            if into_wall(g, out, owner, &piece, sag) == Some(false) && piece.len() >= 2 * WALL_FROM {
                for half in halves(g, &piece) {
                    work.extend(connected(g, &half));
                }
            }
        }
    }
}

/// How far a piece of a wall may turn from where it faces on the whole: a fitted surface lies the wall flat on the plane
/// square to its mean normal, and the fit holds a turn of 75 deg; the piece is cut short of it.
pub(crate) const WALL_TURN_DEG: f64 = 60.0;

/// The triangles joined to `seed` over sides that are not sharp, among those `within` and not `taken` yet (all of them
/// taken on the way), while each faces within `cos_turn` of the mean so far.
pub(crate) fn grow(g: &Grower, seed: usize, within: &[bool], taken: &mut [bool], cos_turn: f64) -> Vec<usize> {
    let mut piece = vec![seed];
    taken[seed] = true;
    let mut sum = g.normal(seed) * g.area(seed);
    let mut queue = VecDeque::from([seed]);
    while let Some(t) = queue.pop_front() {
        for &u in &g.p.neighbours[t] {
            let u = u as usize;
            if u as u32 == NO_NEIGHBOUR || taken[u] || !within[u] || g.sharp(t, u) {
                continue;
            }
            let mean = sum.try_normalize(1e-300).unwrap_or_else(|| g.normal(seed));
            if g.normal(u).dot(&mean) < cos_turn {
                continue;
            }
            taken[u] = true;
            sum += g.normal(u) * g.area(u);
            piece.push(u);
            queue.push_back(u);
        }
    }
    piece
}

/// `piece` made one region on one fitted wall, where it is a mosaic of two tiles or more and one surface holds it; the
/// tiles give its triangles up. `None` where it is no mosaic - its tiles are the part's faces and it is left alone, its
/// parts too: halved, parts of such pieces passed for mosaics, and on a smooth handle 126 of its 264 cylinders went into
/// walls and the body was refused - and otherwise whether one surface held it.
pub(crate) fn into_wall(g: &Grower, out: &mut Vec<Region>, owner: &mut [u32], piece: &[usize], sag: f64) -> Option<bool> {
    let tiles: HashSet<u32> = piece.iter().map(|&t| owner[t]).collect();
    if piece.len() < WALL_FROM || tiles.len() < 2 || !(bends_many_ways(out, &tiles) || a_mosaic(g, out, piece, owner, &tiles)) {
        return None;
    }
    let Some(surface) = wall_surface(g.p, piece, g.tol.distance, sag) else { return Some(false) };
    let r = out.len() as u32;
    let leaving: HashSet<u32> = piece.iter().map(|&t| t as u32).collect();
    for &old in &tiles {
        out[old as usize].tris.retain(|x| !leaving.contains(x));
    }
    for &t in piece {
        owner[t] = r;
    }
    out.push(Region { tris: piece.iter().map(|&t| t as u32).collect(), surface: Some(Surface::Spline(Box::new(surface))) });
    Some(true)
}

/// How many tiles make a mosaic of strips.
pub(crate) const MOSAIC_FROM: usize = 8;
/// How far the normals of a curved tile may spread for it to be a strip: a strip of a free form turns through 10 to 20
/// deg of its cylinder; a rounding through its quarter turn.
pub(crate) const STRIP_UP_TO_DEG: f64 = 60.0;

/// A MOSAIC OF STRIPS, however alike they bend: many tiles, three quarters of the piece on curved tiles each turning
/// through a narrow arc. A free form whose bend changes slowly is cut into strips of cylinders of radii within a quarter
/// of each other, which the ways of bending take for a rounding - measured on a smooth handle, 81 004 triangles of walls
/// were left as strips so. A rounding's tiles each turn through a wide arc: the quarter turn of a fillet and of the
/// sphere at its corner - a rounded bar's rim is eight such tiles and no mosaic. Flat tiles do not count either way.
pub(crate) fn a_mosaic(g: &Grower, out: &[Region], piece: &[usize], owner: &[u32], tiles: &HashSet<u32>) -> bool {
    if tiles.len() < MOSAIC_FROM {
        return false;
    }
    let cos_half = (STRIP_UP_TO_DEG / 2.0).to_radians().cos();
    let strip = |r: u32| {
        let region = &out[r as usize];
        if !matches!(region.surface, Some(Surface::Cylinder { .. } | Surface::Cone { .. } | Surface::Sphere { .. } | Surface::Torus { .. })) {
            return false;
        }
        let Some(mean) = region.tris.iter().fold(Vector3::zeros(), |m, &t| m + g.normal(t as usize) * g.area(t as usize)).try_normalize(1e-300) else { return false };
        region.tris.iter().all(|&t| g.normal(t as usize).dot(&mean) >= cos_half)
    };
    let strips: HashSet<u32> = tiles.iter().copied().filter(|&r| strip(r)).collect();
    2 * piece.iter().filter(|&&t| strips.contains(&owner[t])).count() >= piece.len()
}

/// A WALL BENDS MANY WAYS; a rounding does not. The strips a free form is cut into lie on cylinders and spheres of radii
/// that drift from strip to strip, while a rounding's cylinders and its corner spheres share one radius and are faces of
/// the part as they stand - measured on a bar rounded R2, a wall taken across them lost its faces.
pub(crate) fn bends_many_ways(out: &[Region], tiles: &HashSet<u32>) -> bool {
    let mut bends: Vec<f64> = tiles
        .iter()
        .filter_map(|&r| match out[r as usize].surface {
            Some(Surface::Cylinder { radius, .. }) | Some(Surface::Sphere { radius, .. }) => Some(radius),
            Some(Surface::Torus { minor, .. }) => Some(minor),
            Some(Surface::Cone { half_angle, .. }) => Some(1.0 / half_angle.tan().abs().max(1e-9)),
            _ => None,
        })
        .collect();
    bends.sort_by(f64::total_cmp);
    bends.windows(2).filter(|w| w[1] > 1.25 * w[0]).count() + usize::from(!bends.is_empty()) >= BENDS_FROM
}

/// The one B-spline surface holding `piece` within `tol`: through the corners of its triangles, and through their middles
/// too - a big flat triangle has corners only at its ends, and a surface through the corners alone bulged between them
/// (measured on a cube with a dent in its top, 37 mm^3 of a flat face gone); a middle stands off a curved wall by the
/// mesh's own sag, `sag`.
pub(crate) fn wall_surface(p: &Prepared, piece: &[usize], tol: f64, sag: f64) -> Option<crate::bspline::BSpline> {
    let corner = |t: usize, k: usize| {
        let v = p.mesh.verts[p.mesh.tris[t][k] as usize];
        Vector3::new(v.x, v.y, v.z)
    };
    let normal = |t: usize| Vector3::from(p.normals[t]);
    let area = |t: usize| (corner(t, 1) - corner(t, 0)).cross(&(corner(t, 2) - corner(t, 0))).norm() * 0.5;
    let mut at: HashMap<u32, Vector3<f64>> = HashMap::new();
    for &t in piece {
        for &v in &p.mesh.tris[t] {
            *at.entry(v).or_insert_with(Vector3::zeros) += normal(t) * area(t);
        }
    }
    let mut corners: Vec<u32> = at.keys().copied().collect();
    corners.sort_unstable();
    let mut pts: Vec<[f64; 3]> = corners
        .iter()
        .map(|&v| {
            let q = p.mesh.verts[v as usize];
            [q.x, q.y, q.z]
        })
        .collect();
    let mut nrm: Vec<[f64; 3]> = corners.iter().map(|v| at[v].try_normalize(1e-300).map(Into::into).unwrap_or([0.0, 0.0, 1.0])).collect();
    // A BIG TRIANGLE IS SAMPLED ACROSS, not at its middle alone: a surface held at its corners and middle still bowed
    // between them - measured on a cube with a dent in its top, 69 mm^3 of a flat face gone. Points every fortieth of
    // the piece's extent, at most 50 000 of them.
    let (lo, hi) = pts.iter().fold(([f64::MAX; 3], [f64::MIN; 3]), |(lo, hi), q| ([lo[0].min(q[0]), lo[1].min(q[1]), lo[2].min(q[2])], [hi[0].max(q[0]), hi[1].max(q[1]), hi[2].max(q[2])]));
    let extent = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
    let step = (extent / 40.0).max(1e-9);
    for &t in piece {
        let (a, b, c) = (corner(t, 0), corner(t, 1), corner(t, 2));
        let longest = (b - a).norm().max((c - b).norm()).max((a - c).norm());
        let k = ((longest / step).ceil() as usize).clamp(1, 16);
        for i in 0..=k {
            for j in 0..=(k - i) {
                let (u, v) = (i as f64 / k as f64, j as f64 / k as f64);
                if k > 1 && (i + j == 0 || i == k || j == k) {
                    continue; // the corners are in already
                }
                if k == 1 && !(i == 0 && j == 0) {
                    continue;
                }
                let q = if k == 1 { (a + b + c) / 3.0 } else { a + (b - a) * u + (c - a) * v };
                pts.push(q.into());
                nrm.push(normal(t).into());
            }
        }
        if pts.len() > 50_000 {
            break;
        }
    }
    // the corners within the tolerance and the mesh's sag, a point amid a triangle within twice the sag: it stands off
    // the wall by the triangle's own sag besides
    crate::bspline::fit_held(&pts, &nrm, corners.len(), tol + sag, tol + 2.0 * sag).map(|f| f.surface)
}

/// THE FREE FORM OF A REGION WHOSE PRIMITIVE MADE NO FACE: one B-spline surface over its triangles, held within the
/// distance tolerance plus the mesh's own sag (`sag`, from `chord_deflection`); `None` where one surface does not hold
/// them. A face on a primitive fails where its strip of the primitive is trimmed by curves that leave the surface; a
/// surface laid over the region's own points carries the region's edges by construction.
pub fn free_form_of(p: &Prepared, tris: &[u32], tol: &Tolerance, sag: f64) -> Option<Surface> {
    let piece: Vec<usize> = tris.iter().map(|&t| t as usize).collect();
    wall_surface(p, &piece, tol.distance, sag).map(|b| Surface::Spline(Box::new(b)))
}

/// The pieces of `tris` that hang together across smooth sides: a half cut off by a straight line can fall apart, and
/// a face built on the loops of a region in two pieces took the wrong one (0.09 mm^2 of a region of 13).
pub(crate) fn connected(g: &Grower, tris: &[usize]) -> Vec<Vec<usize>> {
    let inside: HashSet<usize> = tris.iter().copied().collect();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut out = Vec::new();
    for &s in tris {
        if !seen.insert(s) {
            continue;
        }
        let (mut part, mut queue) = (vec![s], VecDeque::from([s]));
        while let Some(t) = queue.pop_front() {
            for &u in &g.p.neighbours[t] {
                let u = u as usize;
                if u as u32 != NO_NEIGHBOUR && inside.contains(&u) && !g.sharp(t, u) && seen.insert(u) {
                    part.push(u);
                    queue.push_back(u);
                }
            }
        }
        out.push(part);
    }
    out
}

/// `piece` cut in two across its longest extent, laid flat on the plane square to its mean normal: the triangles whose
/// middles fall below the median go one way, the rest the other.
pub(crate) fn halves(g: &Grower, piece: &[usize]) -> [Vec<usize>; 2] {
    let mean = piece.iter().fold(Vector3::zeros(), |m, &t| m + g.normal(t) * g.area(t)).try_normalize(1e-300).unwrap_or_else(Vector3::z);
    let helper = if mean.x.abs() < 0.9 { Vector3::x() } else { Vector3::y() };
    let e1 = mean.cross(&helper).normalize();
    let e2 = mean.cross(&e1);
    let flat: Vec<(f64, f64)> = piece
        .iter()
        .map(|&t| {
            let c = g.centroid(t);
            (c.dot(&e1), c.dot(&e2))
        })
        .collect();
    let span = |k: fn(&(f64, f64)) -> f64| flat.iter().map(k).fold(f64::MAX, f64::min) - flat.iter().map(k).fold(f64::MIN, f64::max);
    let along: fn(&(f64, f64)) -> f64 = if span(|q| q.0).abs() >= span(|q| q.1).abs() { |q| q.0 } else { |q| q.1 };
    let mut keys: Vec<f64> = flat.iter().map(along).collect();
    let mid = keys.len() / 2;
    let median = *keys.select_nth_unstable_by(mid, f64::total_cmp).1;
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for (&t, q) in piece.iter().zip(&flat) {
        if along(q) < median {
            a.push(t)
        } else {
            b.push(t)
        }
    }
    [a, b]
}

/// A FACE MAY NOT WIND ROUND ITS AXIS. The crest of a thread is its cylinder less a groove wound round it, and a face
/// bounded so is beyond the kernel: measured on an M10 x 1.5 along 6 mm of a rod of 12, the face built on the crest's
/// cylinder took 101 of its 212 mm^2 and the crest went back to the mesh; cutting a whole band of the cylinder by the
/// loop's edges came apart as well. So a region on a surface turned about an axis, one of whose loops follows the axis
/// round a turn more than it ends up turned - a groove's hole comes back where it began, a groove running out through
/// the rim goes round once - is cut by the plane through the axis into its two halves, and each piece of a half is a
/// region of its own on the same surface: no loop within half a turn can wind.
pub(crate) fn unwind(g: &Grower, out: &mut Vec<Region>, owner: &mut [u32]) {
    for r in 0..out.len() {
        let (on, a) = match out[r].surface {
            Some(Surface::Cylinder { point, axis, .. }) | Some(Surface::Torus { center: point, axis, .. }) | Some(Surface::Cone { apex: point, axis, .. }) => {
                (Vector3::from(point), Vector3::from(axis))
            }
            _ => continue,
        };
        let (x, y) = across(&a);
        let angle = |v: &Vector3<f64>| (v - on).dot(&y).atan2((v - on).dot(&x));
        // the region's border, each side from the corner it leaves to the one it reaches, chained into loops
        let mut ahead: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
        for &t in &out[r].tris {
            let corners = g.p.mesh.tris[t as usize];
            for k in 0..3 {
                let u = g.p.neighbours[t as usize][k];
                if u == NO_NEIGHBOUR || owner[u as usize] as usize != r {
                    ahead.entry(corners[k]).or_default().push(corners[(k + 1) % 3]);
                }
            }
        }
        let at = |i: u32| {
            let v = g.p.mesh.verts[i as usize];
            Vector3::new(v.x, v.y, v.z)
        };
        let mut wound = false;
        let mut starts: Vec<u32> = ahead.keys().copied().collect();
        starts.sort_unstable();
        for start in starts {
            while !wound && ahead.get(&start).is_some_and(|n| !n.is_empty()) {
                let (mut here, mut turned, mut lo, mut hi) = (start, 0.0f64, 0.0f64, 0.0f64);
                let mut before = angle(&at(start));
                while let Some(next) = ahead.get_mut(&here).and_then(|n| n.pop()) {
                    let now = angle(&at(next));
                    turned += (now - before + std::f64::consts::PI).rem_euclid(2.0 * std::f64::consts::PI) - std::f64::consts::PI;
                    (lo, hi, before, here) = (lo.min(turned), hi.max(turned), now, next);
                    if here == start {
                        break;
                    }
                }
                wound = hi - lo > turned.abs() + 2.0 * std::f64::consts::PI;
            }
        }
        if !wound {
            continue;
        }
        // the halves, and the pieces of each: triangles joined across a side, their middles on one side of the plane
        let half: std::collections::HashMap<u32, bool> = out[r].tris.iter().map(|&t| (t, angle(&g.centroid(t as usize)) >= 0.0)).collect();
        let mut seen: HashSet<u32> = HashSet::new();
        let mut pieces: Vec<Vec<u32>> = Vec::new();
        for &t in &out[r].tris {
            if !seen.insert(t) {
                continue;
            }
            let (mut piece, mut queue) = (vec![t], VecDeque::from([t]));
            while let Some(s) = queue.pop_front() {
                for &u in &g.p.neighbours[s as usize] {
                    if u != NO_NEIGHBOUR && half.get(&u) == Some(&half[&t]) && seen.insert(u) {
                        piece.push(u);
                        queue.push_back(u);
                    }
                }
            }
            pieces.push(piece);
        }
        let surface = out[r].surface.clone();
        let mut pieces = pieces.into_iter();
        out[r].tris = pieces.next().unwrap_or_default();
        for piece in pieces {
            for &t in &piece {
                owner[t as usize] = out.len() as u32;
            }
            out.push(Region { tris: piece, surface: surface.clone() });
        }
    }
}

/// A SPRING'S WIRE IS ONE COIL, NOT A MOSAIC. A round wire swept along a helix is no surface of revolution, and it came
/// apart into tiles of tori, spheres and cylinders: measured on three turns of radius 10 rising 4, wire 1, 1 768
/// regions and 4 993 faces. The tiles joined across edges that are not sharp are taken together; a surface carried into
/// itself by a screw motion has every normal square to that motion's velocity, a x (x - p) + h a, and that is linear in
/// the normals' lines - (n, x * n) - so the axis and the rise come from the least eigenvector. The coil about them is
/// fitted to the corners, and taken where every triangle lies on it.
pub(crate) fn coiled(g: &Grower, out: &mut [Region], owner: &mut [u32]) {
    let tile = |r: &Region| match &r.surface {
        None => !r.tris.is_empty(),
        Some(Surface::Free { .. } | Surface::Spline(_) | Surface::Helix { .. } | Surface::RoundHelix { .. } | Surface::Coil { .. }) => false,
        Some(Surface::Plane { .. }) => !r.tris.is_empty() && r.tris.len() <= TILE_UP_TO,
        Some(_) => !r.tris.is_empty(),
    };
    let n = out.len();
    let mut up: Vec<usize> = (0..n).collect();
    fn root(up: &mut [usize], mut x: usize) -> usize {
        while up[x] != x {
            up[x] = up[up[x]];
            x = up[x];
        }
        x
    }
    for r in 0..n {
        if !tile(&out[r]) {
            continue;
        }
        for &t in &out[r].tris {
            for &u in &g.p.neighbours[t as usize] {
                if u == NO_NEIGHBOUR || g.sharp(t as usize, u as usize) {
                    continue;
                }
                let o = owner[u as usize] as usize;
                if o != r && tile(&out[o]) {
                    let (a, b) = (root(&mut up, r), root(&mut up, o));
                    up[a] = b;
                }
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for (r, o) in out.iter().enumerate() {
        if tile(o) {
            let k = root(&mut up, r);
            groups.entry(k).or_default().push(r);
        }
    }
    let mut groups: Vec<Vec<usize>> = groups.into_values().filter(|m| m.len() >= TILES_FROM).collect();
    groups.sort();
    for members in groups {
        let tris: Vec<u32> = members.iter().flat_map(|&r| out[r].tris.iter().copied()).collect();
        let Some(coil) = coil_about(g, &tris) else { continue };
        if !tris.iter().all(|&t| g.fits(t as usize, &coil)) {
            continue;
        }
        let keep = members[0];
        for &r in &members[1..] {
            out[r].tris.clear();
            out[r].surface = None;
        }
        for &t in &tris {
            owner[t as usize] = keep as u32;
        }
        out[keep].tris = tris;
        out[keep].surface = Some(coil);
    }
}

/// The coil the triangles `tris` lie on as nearly as it can be fitted, or `None` where their normals show no screw.
pub(crate) fn coil_about(g: &Grower, tris: &[u32]) -> Option<Surface> {
    // the screw: the least of a . (S_mm - S_mn S_nn^-1 S_nm) a over unit a, then c = -S_nn^-1 S_nm a, rise a . c and the
    // axis through a x c
    let (mut nn, mut nm, mut mm) = (Matrix3::<f64>::zeros(), Matrix3::<f64>::zeros(), Matrix3::<f64>::zeros());
    for &t in tris {
        let t = t as usize;
        if g.sliver[t] {
            continue;
        }
        let (n, w) = (g.normal(t), g.area(t));
        let m = g.centroid(t).cross(&n);
        nn += n * n.transpose() * w;
        nm += n * m.transpose() * w;
        mm += m * m.transpose() * w;
    }
    let solve = nn.try_inverse()?;
    let (_, vec) = eigen_sorted(mm - nm.transpose() * solve * nm);
    let a = vec[0].normalize();
    let c = -(solve * nm * a);
    let rise = a.dot(&c);
    let tau = 2.0 * std::f64::consts::PI;
    if tau * rise.abs() <= 10.0 * g.tol.distance {
        return None;
    }
    let on = a.cross(&c);
    let pts = g.corners_of(tris);
    // the first coil: the wire's middle at the corners' mean distance from the axis, its height from the corners' heights
    // less the rise, averaged round the turn, and the wire the corners' mean distance from that
    let (x, y) = across(&a);
    let pitch = tau * rise;
    let (mut far, mut round) = (0.0, nalgebra::Complex::new(0.0, 0.0));
    for p in &pts {
        let d = p - on;
        let e = d - a * d.dot(&a);
        far += e.norm() / pts.len() as f64;
        let height = d.dot(&a) - rise * e.dot(&y).atan2(e.dot(&x));
        round += nalgebra::Complex::from_polar(1.0, tau * height / pitch);
    }
    let lift = round.arg() * pitch / tau;
    let first = Surface::Coil { point: on.into(), axis: a.into(), reference: x.into(), rise, radius: far, lift, wire: 0.0 };
    let reach: Vec<f64> = pts.iter().filter_map(|p| coil_at(&first, p)).map(|at| at.out.norm()).collect();
    let wire = reach.iter().sum::<f64>() / reach.len().max(1) as f64;
    // A WIRE IS ROUND BEFORE IT IS REFINED: the corners of the spring's mesh stood 0.16 % of the wire apart in their distance
    // from the first middle line, those of every smooth patch of the samples whose normals let a screw through 8 % and
    // more; refining each of those to turn it down took a mesh of hearts and gears from 0.84 s to 2.2 s to split
    let spread = (reach.iter().map(|r| (r - wire).powi(2)).sum::<f64>() / reach.len().max(1) as f64).sqrt();
    if spread > 0.04 * wire {
        return None;
    }
    // refined: the axis tilted and moved across itself, the rise, the radius, the lift and the wire
    let (u, w) = (x, y);
    let make = |q: &[f64; 8]| {
        let axis = (a + u * q[0] + w * q[1]).normalize();
        let reference = (x - axis * axis.dot(&x)).normalize();
        Surface::Coil { point: (on + u * q[2] + w * q[3]).into(), axis: axis.into(), reference: reference.into(), rise: q[4], radius: q[5], lift: q[6], wire: q[7] }
    };
    let miss = |q: &[f64; 8], v: &Vector3<f64>| {
        let off = |q: &[f64; 8]| coil_at(&make(q), v).map_or(0.0, |at| at.off);
        let now = off(q);
        let mut grad = [0.0; 8];
        for k in 0..8 {
            let step = 1e-6 * (1.0 + q[k].abs());
            let (mut up, mut down) = (*q, *q);
            up[k] += step;
            down[k] -= step;
            grad[k] = (off(&up) - off(&down)) / (2.0 * step);
        }
        (now, grad)
    };
    let mut q = [0.0, 0.0, 0.0, 0.0, rise, far, lift, wire];
    refine(&mut q, &pts, &miss, 20, 600, None, g.tol.distance * 0.1);
    (q[5] > q[7] && q[7] > 0.0).then(|| make(&q))
}

/// How many triangles a plane may have and still be a tile of a free form: a flat bit of one, not a face beside it.
pub(crate) const TILE_UP_TO: usize = 40;
/// How few tiles make a free form.
pub(crate) const TILES_FROM: usize = 6;
/// How many kinds of bending the tiles of a free form show at the least: a rounding and its corner spheres bend alike.
pub(crate) const BENDS_FROM: usize = 3;

/// A FREE FORM IS ONE REGION, NOT A MOSAIC. A smooth wall no plane, cylinder, cone, sphere or torus is - a loft, a
/// styled cover - is split into tiles, each within the tolerance of a surface of its own: measured on a smooth loft
/// through squares of 10, 6 and 10, 178 regions and 494 faces for the six the loft has. The tiles - curved regions and
/// small planes joined across edges that are not sharp - become one region of a free form where they are many, bend in
/// at least three different ways (a radius more than 1.25 times another's), all face one side, and no single surface
/// takes them whole. A rounded bar stays itself: its roundings and corner spheres bend alike.
pub(crate) fn free_form(g: &Grower, out: &mut [Region], owner: &mut [u32]) {
    let tile = |r: &Region| match &r.surface {
        None | Some(Surface::Free { .. } | Surface::Spline(_) | Surface::Helix { .. } | Surface::RoundHelix { .. } | Surface::Coil { .. }) => false,
        Some(Surface::Plane { .. }) => !r.tris.is_empty() && r.tris.len() <= TILE_UP_TO,
        Some(_) => !r.tris.is_empty(),
    };
    let n = out.len();
    let mut up: Vec<usize> = (0..n).collect();
    fn root(up: &mut [usize], mut x: usize) -> usize {
        while up[x] != x {
            up[x] = up[up[x]];
            x = up[x];
        }
        x
    }
    for r in 0..n {
        if !tile(&out[r]) {
            continue;
        }
        for &t in &out[r].tris {
            for &u in &g.p.neighbours[t as usize] {
                if u == NO_NEIGHBOUR || g.sharp(t as usize, u as usize) {
                    continue;
                }
                let o = owner[u as usize] as usize;
                if o != r && tile(&out[o]) {
                    let (a, b) = (root(&mut up, r), root(&mut up, o));
                    up[a] = b;
                }
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for (r, o) in out.iter().enumerate() {
        if tile(o) {
            let k = root(&mut up, r);
            groups.entry(k).or_default().push(r);
        }
    }
    let mut groups: Vec<Vec<usize>> = groups.into_values().filter(|m| m.len() >= TILES_FROM).collect();
    groups.sort();
    for members in groups {
        // the ways the tiles bend: radii within 1.25 times of one another are one way
        let mut bends: Vec<f64> = members
            .iter()
            .filter_map(|&r| match out[r].surface {
                Some(Surface::Cylinder { radius, .. }) | Some(Surface::Sphere { radius, .. }) => Some(radius),
                Some(Surface::Torus { minor, .. }) => Some(minor),
                Some(Surface::Cone { half_angle, .. }) => Some(1.0 / half_angle.tan().abs().max(1e-9)),
                _ => None,
            })
            .collect();
        bends.sort_by(f64::total_cmp);
        let ways = bends.windows(2).filter(|w| w[1] > 1.25 * w[0]).count() + usize::from(!bends.is_empty());
        if ways < BENDS_FROM {
            continue;
        }
        let tris: Vec<u32> = members.iter().flat_map(|&r| out[r].tris.iter().copied()).collect();
        let Some(mean) = tris.iter().fold(Vector3::zeros(), |m, &t| m + g.normal(t as usize) * g.area(t as usize)).try_normalize(1e-12) else { continue };
        if tris.iter().any(|&t| !g.sliver[t as usize] && g.normal(t as usize).dot(&mean) <= 0.0) {
            continue;
        }
        let sample = Sample::of(g, &tris);
        if [Kind::Plane, Kind::Cylinder, Kind::Cone, Kind::Sphere, Kind::Torus].into_iter().any(|k| g.fit_sample(&sample, k).is_some_and(|s| tris.iter().all(|&t| g.fits(t as usize, &s)))) {
            continue;
        }
        let keep = members[0];
        for &r in &members[1..] {
            out[r].tris.clear();
            out[r].surface = None;
        }
        for &t in &tris {
            owner[t as usize] = keep as u32;
        }
        out[keep].tris = tris;
        out[keep].surface = Some(Surface::Free { normal: mean.into() });
    }
}

/// REGIONS THAT LIE ON ONE SURFACE TOGETHER BECOME ONE.
///
/// A seed sees a few rings around it, and on a torus that is a narrow band: measured on a torus of radii 20 and 5,
/// the seeds grew bands of cylinders (radius 5.003) and spheres (5.078) and not one torus - the tube's centre points
/// of one band lie on too short an arc to fit a circle to. Two neighbouring bands span twice the arc. So curved
/// neighbours are joined while their union fits one surface - the kind of either, a cone or a torus - with every
/// triangle on it; the widest regions are joined first. An emptied region is dropped at the end.
pub(crate) fn merge(g: &Grower, out: &mut [Region], owner: &mut [u32]) {
    let curved = |r: &Region| r.tris.len() >= MERGE_FROM && matches!(r.surface, Some(Surface::Cylinder { .. } | Surface::Sphere { .. } | Surface::Cone { .. } | Surface::Torus { .. }));
    // A JOIN IS TRIED ONLY WHERE IT CAN BE TRUE. On a 225 154-triangle organic mesh of 28 000 regions the joins took
    // 186 s when every pair was tried in every pass and refined to the end.
    // after the first pass only a pair of which one side has changed can join
    let mut changed: Option<HashSet<usize>> = None;
    loop {
        let mut now: HashSet<usize> = HashSet::new();
        let mut order: Vec<usize> = (0..out.len()).filter(|&r| curved(&out[r])).collect();
        order.sort_by_key(|&r| std::cmp::Reverse(out[r].tris.len()));
        for a in order {
            let mut tried: HashSet<usize> = HashSet::new();
            // the region keeps taking neighbours while one fits, so a torus grows band by band in one pass
            'more: loop {
                if !curved(&out[a]) {
                    break; // emptied by an earlier join
                }
                let mut near: Vec<usize> = Vec::new();
                for &t in &out[a].tris {
                    for &u in &g.p.neighbours[t as usize] {
                        if u == NO_NEIGHBOUR || g.sharp(t as usize, u as usize) {
                            continue;
                        }
                        let b = owner[u as usize] as usize;
                        let fresh = changed.as_ref().is_none_or(|c| c.contains(&a) || c.contains(&b));
                        if b != a && fresh && !near.contains(&b) && !tried.contains(&b) && curved(&out[b]) && alike(&out[a].surface, &out[b].surface) {
                            near.push(b);
                        }
                    }
                }
                for b in near {
                    tried.insert(b);
                    // THE CHEAP WAY FIRST: a neighbour lying on the surface already found joins it as it is. Once a torus
                    // stands from its first bands, the rest of them do, and refitting the union for each one cost 6 ms
                    // a band on a print program's torus.
                    if out[a].surface.as_ref().is_some_and(|s| out[b].tris.iter().all(|&t| g.fits(t as usize, s))) {
                        let taken = std::mem::take(&mut out[b].tris);
                        for &t in &taken {
                            owner[t as usize] = a as u32;
                        }
                        out[a].tris.extend(taken);
                        out[b].surface = None;
                        now.insert(a);
                        continue 'more;
                    }
                    let union: Vec<u32> = out[a].tris.iter().chain(&out[b].tris).copied().collect();
                    let mut kinds: Vec<Kind> = Vec::new();
                    for k in [out[a].surface.as_ref().map(kind_of), out[b].surface.as_ref().map(kind_of), Some(Kind::Cone), Some(Kind::Torus)].into_iter().flatten() {
                        if !kinds.iter().any(|x| std::mem::discriminant(x) == std::mem::discriminant(&k)) {
                            kinds.push(k);
                        }
                    }
                    let sample = Sample::of(g, &union);
                    let Some(s) = kinds.into_iter().find_map(|k| g.fit_sample(&sample, k).filter(|s| union.iter().all(|&t| g.fits(t as usize, s)))) else { continue };
                    for &t in &out[b].tris {
                        owner[t as usize] = a as u32;
                    }
                    out[b].tris.clear();
                    out[b].surface = None;
                    out[a].tris = union;
                    out[a].surface = Some(s);
                    now.insert(a);
                    continue 'more; // a neighbour turned down gets its second chance in the next pass
                }
                break;
            }
        }
        if now.is_empty() {
            break;
        }
        changed = Some(now);
    }
}

/// A REGION LYING WHOLLY ON A LARGER NEIGHBOUR'S SURFACE joins it, the smallest first. Measured on the torus: after
/// the joins two single triangles stood alone as planes, and on a print program's torus a strip of flat pairs along
/// the inner equator - each lying on the torus it was cut from. A plane face beside a rounding does not lie on its
/// cylinder and stays.
///
/// IT IS REPEATED UNTIL NOTHING MORE JOINS. Measured on a cone: a ring of single triangles along the base touched only
/// the cap and regions of three, which joined the cone after the singles had been looked at.
pub(crate) fn absorb(g: &Grower, out: &mut [Region], owner: &mut [u32]) {
    loop {
        if !absorb_once(g, out, owner) {
            break;
        }
    }
}

/// One pass of `absorb`, the smallest regions first; whether anything joined.
pub(crate) fn absorb_once(g: &Grower, out: &mut [Region], owner: &mut [u32]) -> bool {
    let mut joined = false;
    let mut order: Vec<usize> = (0..out.len()).filter(|&r| !out[r].tris.is_empty()).collect();
    order.sort_by_key(|&r| out[r].tris.len());
    for r in order {
        if out[r].tris.is_empty() {
            continue;
        }
        let tris = out[r].tris.clone();
        let mut near: Vec<usize> =
            tris.iter().flat_map(|&t| g.p.neighbours[t as usize]).filter(|&u| u != NO_NEIGHBOUR).map(|u| owner[u as usize] as usize).filter(|&h| h != r && out[h].tris.len() > tris.len()).collect();
        near.sort_unstable();
        near.dedup();
        // A TRIANGLE WHOSE NORMAL THE TOLERANCE CAN TURN PAST THE ANGLE is judged by its corners alone: at the owner's
        // screw at a hundred times the tolerance, 1 677 triangles with sides of 0.01 mm stood alone, turned 33 to 44 deg
        // from neighbours whose surface their corners lay on. Only in joining: a seed still needs a normal to trust.
        let lies = |t: u32, s: &Surface| g.fits(t as usize, s) || (g.corners_on(t as usize, s) && !g.normal_trusted(t as usize));
        let home = near.into_iter().find(|&h| out[h].surface.as_ref().is_some_and(|s| tris.iter().all(|&t| lies(t, s))));
        if let Some(h) = home {
            for &t in &tris {
                owner[t as usize] = h as u32;
            }
            out[h].tris.extend(tris);
            out[r].tris.clear();
            out[r].surface = None;
            joined = true;
        }
    }
    joined
}
