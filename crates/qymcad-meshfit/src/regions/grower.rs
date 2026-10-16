//! A REGION GROWN from a seed: its samples, corners and fits.

use super::*;

pub(crate) fn kind_of(s: &Surface) -> Kind {
    match s {
        Surface::Plane { .. } => Kind::Plane,
        Surface::Cylinder { .. } => Kind::Cylinder,
        Surface::Cone { .. } => Kind::Cone,
        Surface::Sphere { .. } => Kind::Sphere,
        Surface::Torus { .. } => Kind::Torus,
        // a free form and a helix are never refitted by kind; they stand where a plane would be asked for nothing
        Surface::Free { .. } | Surface::Spline(_) | Surface::Helix { .. } | Surface::RoundHelix { .. } | Surface::Coil { .. } => Kind::Plane,
    }
}

/// What every fit of a set of triangles reads, gathered once: the distinct corners, the triangles' normals, and - only
/// when a cone or a torus asks - each corner with its normal averaged round it. A join tries up to four kinds on the
/// same union, and each gathering its own cost a HashSet, a HashMap and their vectors every time.
pub(crate) struct Sample<'t> {
    pub(crate) tris: &'t [u32],
    pub(crate) pts: Vec<Vector3<f64>>,
    pub(crate) normals: Vec<Vector3<f64>>,
    pub(crate) corners: std::cell::OnceCell<CornerNormals>,
}

/// Each corner of a set of triangles with the normal averaged round it, by area.
pub(crate) struct CornerNormals {
    pub(crate) feet: Vec<Vector3<f64>>,
    pub(crate) normals: Vec<Vector3<f64>>,
}

impl<'t> Sample<'t> {
    pub(crate) fn of(g: &Grower, tris: &'t [u32]) -> Sample<'t> {
        let normals = tris.iter().filter(|&&t| !g.sliver[t as usize]).map(|&t| g.normal(t as usize)).collect();
        Sample { tris, pts: g.corners_of(tris), normals, corners: std::cell::OnceCell::new() }
    }

    pub(crate) fn corners(&self, g: &Grower) -> &CornerNormals {
        self.corners.get_or_init(|| g.corner_normals(self.tris))
    }
}

pub(crate) struct Grower<'a> {
    pub(crate) p: &'a Prepared,
    pub(crate) tol: &'a Tolerance,
    pub(crate) cos_sharp: f64,
    pub(crate) cos_angle: f64,
    pub(crate) sliver: &'a [bool],
}

impl Grower<'_> {
    pub(crate) fn corner(&self, t: usize, k: usize) -> Vector3<f64> {
        let v = self.p.mesh.verts[self.p.mesh.tris[t][k] as usize];
        Vector3::new(v.x, v.y, v.z)
    }

    pub(crate) fn centroid(&self, t: usize) -> Vector3<f64> {
        (self.corner(t, 0) + self.corner(t, 1) + self.corner(t, 2)) / 3.0
    }

    pub(crate) fn normal(&self, t: usize) -> Vector3<f64> {
        let n = self.p.normals[t];
        Vector3::new(n[0], n[1], n[2])
    }

    pub(crate) fn area(&self, t: usize) -> f64 {
        (self.corner(t, 1) - self.corner(t, 0)).cross(&(self.corner(t, 2) - self.corner(t, 0))).norm() * 0.5
    }

    /// Whether the edge between `t` and `u` is sharper than the threshold. A sliver's normal means nothing, so an
    /// edge of a sliver is never taken for sharp.
    pub(crate) fn sharp(&self, t: usize, u: usize) -> bool {
        !self.sliver[t] && !self.sliver[u] && self.normal(t).dot(&self.normal(u)) < self.cos_sharp
    }

    pub(crate) fn corners_on(&self, t: usize, s: &Surface) -> bool {
        (0..3).all(|k| distance(s, &self.corner(t, k)) <= self.tol.distance)
    }

    /// Whether triangle `t` lies on `s`: its corners within the distance, its normal within the angle of the
    /// surface's normal (either way round: a hole's wall faces its axis).
    ///
    /// THE NORMAL IS COMPARED AT THE CENTRE OF THE CIRCLE THROUGH THE CORNERS, not at the centroid. A triangle with
    /// its corners on a sphere has a normal through the sphere's centre only at that point; at the centroid of a
    /// skewed one they part by degrees, and the corner spheres of the rounded bar came apart into single triangles.
    /// On a cylinder's strip that centre falls midway between the two lines, where the normals agree anyway.
    /// Whether triangle `t` is wide enough for its normal to mean something: corners moved within the distance tolerance
    /// turn it by up to atan(2 x tolerance / h), h its least height, and that stays within the angle tolerance.
    pub(crate) fn normal_trusted(&self, t: usize) -> bool {
        let (a, b, c) = (self.corner(t, 0), self.corner(t, 1), self.corner(t, 2));
        let longest = (b - a).norm().max((c - b).norm()).max((a - c).norm());
        let least_height = (b - a).cross(&(c - a)).norm() / longest.max(1e-300);
        least_height * self.tol.angle_deg.to_radians().tan() >= 2.0 * self.tol.distance
    }

    pub(crate) fn fits(&self, t: usize, s: &Surface) -> bool {
        // ...AND AT THE CENTROID, for a long blunt triangle: the centre of its circle stands far aside, and a cylinder's
        // normal there is another's - measured on the wall under a plane cutting a cylinder of 10 at 30 deg, strips from
        // the foot up to the cut were 10.6 deg off at that centre and 0.6 at the centroid, and the wall lay on no cylinder
        let agrees = |q: &Vector3<f64>| normal_at(s, q).is_some_and(|n| n.dot(&self.normal(t)).abs() >= self.cos_angle);
        self.corners_on(t, s) && (self.sliver[t] || agrees(&self.circumcentre(t)) || agrees(&self.centroid(t)))
    }

    pub(crate) fn circumcentre(&self, t: usize) -> Vector3<f64> {
        let (a, b, c) = (self.corner(t, 0), self.corner(t, 1), self.corner(t, 2));
        let (ab, ac) = (b - a, c - a);
        let n = ab.cross(&ac);
        let nn = n.norm_squared();
        if nn < 1e-30 {
            return self.centroid(t);
        }
        a + (n.cross(&ab) * ac.norm_squared() + ac.cross(&n) * ab.norm_squared()) / (2.0 * nn)
    }

    /// The seed and its neighbours `rings` out that belong to no region yet and are not across a sharp edge.
    pub(crate) fn patch(&self, seed: usize, rings: usize, owner: &[u32]) -> Vec<u32> {
        let mut out = vec![seed as u32];
        let mut ring = vec![seed];
        for _ in 0..rings {
            let mut next = Vec::new();
            for &t in &ring {
                for &u in &self.p.neighbours[t] {
                    if u == NO_NEIGHBOUR || owner[u as usize] != NONE || self.sliver[u as usize] || out.contains(&u) || self.sharp(t, u as usize) {
                        continue;
                    }
                    out.push(u);
                    next.push(u as usize);
                }
            }
            ring = next;
        }
        out
    }

    /// The surfaces of `kind` a seed may lie on, each to be grown and the widest kept.
    ///
    /// ONE GUESS FROM A PATCH AROUND THE SEED IS NOT ENOUGH. At the end of a rounding's strip the patch two rings
    /// out reaches into the corner sphere, the axis taken from those mixed normals is askew, and the cylinder died at
    /// its first step: measured on the rounded bar, the roundings came out as 4-triangle "planes". The axis of a
    /// cylinder is square to both normals of any two of its triangles, so every neighbour of the seed gives an axis
    /// of its own - the one across the strip gives the true one - and each is tried.
    pub(crate) fn guesses(&self, seed: usize, kind: Kind, owner: &[u32]) -> Vec<Surface> {
        let mut out = Vec::new();
        match kind {
            Kind::Plane => {
                let (c, n) = (self.centroid(seed), self.normal(seed));
                out.push(Surface::Plane { point: c.into(), normal: n.into() });
            }
            Kind::Cylinder => {
                for &u in &self.p.neighbours[seed] {
                    if u == NO_NEIGHBOUR || owner[u as usize] != NONE || self.sliver[u as usize] || self.sharp(seed, u as usize) {
                        continue;
                    }
                    let Some(axis) = self.normal(seed).cross(&self.normal(u as usize)).try_normalize(1e-9) else { continue };
                    out.extend(circle_about(&self.corners_of(&[seed as u32, u]), &axis));
                }
                out.extend(self.fit(&self.patch(seed, 2, owner), kind));
                // AND FROM THE SMOOTH PART OF THAT PATCH ALONE, the triangles turned from the seed by less than the
                // angle tolerance. Measured on a cylinder of 10 under a sphere set 0.1 mm off its axis: the patch two
                // rings out took in three triangles of the sphere, turned 13 to 17 deg - less than a sharp edge - and
                // the axis from their normals lay across the wall instead of along it.
                let smooth: Vec<u32> = self.patch(seed, 2, owner).into_iter().filter(|&t| self.normal(t as usize).dot(&self.normal(seed)) >= self.cos_angle).collect();
                out.extend(self.fit(&smooth, kind));
            }
            Kind::Sphere => {
                out.extend(self.fit(&self.patch(seed, 1, owner), kind));
                out.extend(self.fit(&self.patch(seed, 2, owner), kind));
            }
            // a cone and a torus need their normals to spread along the axis too, so the patch reaches further
            Kind::Cone | Kind::Torus => {
                out.extend(self.fit(&self.patch(seed, 2, owner), kind));
                out.extend(self.fit(&self.patch(seed, 3, owner), kind));
            }
        }
        out
    }

    /// Every corner of `tris` with its normal: the area-weighted mean of the normals of the triangles of `tris` round it.
    pub(crate) fn corner_normals(&self, tris: &[u32]) -> CornerNormals {
        let mut sum: std::collections::HashMap<u32, Vector3<f64>> = std::collections::HashMap::new();
        for &t in tris {
            let t = t as usize;
            if self.sliver[t] {
                continue;
            }
            let weighted = self.normal(t) * self.area(t);
            for &i in &self.p.mesh.tris[t] {
                *sum.entry(i).or_insert_with(Vector3::zeros) += weighted;
            }
        }
        let mut keys: Vec<u32> = sum.keys().copied().collect();
        keys.sort_unstable();
        let (mut feet, mut normals) = (Vec::with_capacity(keys.len()), Vec::with_capacity(keys.len()));
        for i in keys {
            if let Some(n) = sum[&i].try_normalize(1e-12) {
                let v = self.p.mesh.verts[i as usize];
                feet.push(Vector3::new(v.x, v.y, v.z));
                normals.push(n);
            }
        }
        CornerNormals { feet, normals }
    }

    /// The distinct corners of `tris`.
    pub(crate) fn corners_of(&self, tris: &[u32]) -> Vec<Vector3<f64>> {
        let mut seen = HashSet::new();
        let mut pts = Vec::new();
        for &t in tris {
            for &i in &self.p.mesh.tris[t as usize] {
                if seen.insert(i) {
                    let v = self.p.mesh.verts[i as usize];
                    pts.push(Vector3::new(v.x, v.y, v.z));
                }
            }
        }
        pts
    }

    /// The surface of `kind` fitted to the corners of `tris` by least squares; a cylinder takes its axis from the
    /// normals, which a tessellation gets right at every triangle.
    pub(crate) fn fit(&self, tris: &[u32], kind: Kind) -> Option<Surface> {
        self.fit_sample(&Sample::of(self, tris), kind)
    }

    pub(crate) fn fit_sample(&self, sample: &Sample, kind: Kind) -> Option<Surface> {
        let accept = SHORT_REFINE_TO * self.tol.distance;
        match kind {
            Kind::Plane => fit_plane(&sample.pts),
            Kind::Cylinder => fit_cylinder(&sample.pts, &sample.normals, accept),
            Kind::Cone => {
                let CornerNormals { feet, normals } = sample.corners(self);
                fit_cone(&sample.pts, feet, normals, accept)
            }
            Kind::Sphere => fit_sphere(&sample.pts),
            Kind::Torus => {
                let CornerNormals { feet, normals } = sample.corners(self);
                fit_torus(&sample.pts, feet, normals, accept)
            }
        }
    }

    /// Grow from `seed` over every neighbour that lies on the surface, refitting it to the whole region each time
    /// the region doubles. `None` when the seed itself does not lie on it.
    pub(crate) fn grow(&self, seed: usize, mut s: Surface, kind: Kind, owner: &[u32]) -> Option<(Vec<u32>, Surface)> {
        if !self.fits(seed, &s) {
            return None;
        }
        let mut inside = vec![seed as u32];
        let mut member: HashSet<u32> = HashSet::from([seed as u32]);
        let mut tried: HashSet<u32> = HashSet::from([seed as u32]);
        let mut queue = VecDeque::from([seed]);
        let mut refit_at = 8;
        // GROWN IN ROUNDS: after each the surface is refitted to the whole region, and every neighbour turned down gets
        // another look - it was measured against the surface of the few triangles the region then had. A short arc of a
        // few noisy corners puts a cylinder's axis off: on a cylinder of 10 with its corners moved by 0.02, a guess stood
        // 0.18 mm off its axis, stopped at 11 triangles of the wall's 90, and a sphere through both rims took the wall.
        for _round in 0..4 {
            let before = inside.len();
            while let Some(t) = queue.pop_front() {
                if !member.contains(&(t as u32)) {
                    continue; // left the region at a refit
                }
                for &u in &self.p.neighbours[t] {
                    if u == NO_NEIGHBOUR || owner[u as usize] != NONE || tried.contains(&u) {
                        continue;
                    }
                    let ui = u as usize;
                    if self.sharp(t, ui) || !self.fits(ui, &s) {
                        continue;
                    }
                    tried.insert(u);
                    member.insert(u);
                    inside.push(u);
                    queue.push_back(ui);
                    if inside.len() >= refit_at {
                        s = self.refit(&mut inside, &mut member, seed, kind).unwrap_or(s);
                        refit_at *= 2;
                    }
                }
            }
            s = self.refit(&mut inside, &mut member, seed, kind).unwrap_or(s);
            if inside.len() <= before {
                break;
            }
            queue = inside.iter().map(|&t| t as usize).collect();
        }
        Some((inside, s))
    }

    /// The surface refitted to the whole region. A triangle taken early, on a rougher guess, may not lie on the
    /// better surface: measured on the rounded bar, a rounding took one triangle of each corner sphere, its radius
    /// came out 2.0025 instead of 2, and a refit that every member had to fit was never taken. So the members off
    /// the refit leave - if they are few and the seed is not among them - and the rest is fitted once more.
    pub(crate) fn refit(&self, inside: &mut Vec<u32>, member: &mut HashSet<u32>, seed: usize, kind: Kind) -> Option<Surface> {
        let f = self.fit(inside, kind)?;
        let (stay, leave): (Vec<u32>, Vec<u32>) = inside.iter().partition(|&&t| self.fits(t as usize, &f));
        if leave.is_empty() {
            return Some(f);
        }
        if leave.len() * MAY_LEAVE > inside.len() || leave.contains(&(seed as u32)) {
            return None;
        }
        let g = self.fit(&stay, kind).filter(|g| stay.iter().all(|&t| self.fits(t as usize, g)))?;
        for t in &leave {
            member.remove(t);
        }
        *inside = stay;
        Some(g)
    }
}
