//! Turning a cage into NURBS patches.
//!
//! A cage is what a person edits; the kernel needs real geometry. The conversion rests on one fact:
//!
//! **A regular face of a quadrilateral cage is exactly a bicubic patch.** If all four corners of a face have
//! valence four and none lies on a border, the Catmull-Clark limit surface over that face coincides with the
//! uniform bicubic B-spline patch built from the ring of sixteen vertices around it. Not approximates —
//! coincides; it is a property of the scheme.
//!
//! The patch is then converted into Bezier form, the same 4×4 points under different weights, which is what the
//! kernel accepts without argument as a `Geom_BezierSurface`.
//!
//! # What to do about extraordinary vertices
//!
//! Vertices whose valence is not four stay extraordinary forever, however far the cage is subdivided, as a
//! guard in `subdiv/tests.rs` verifies. The faces touching them have no exact patch.
//!
//! The remedy is known: subdivide the cage two or three times. Each step leaves the number of extraordinary
//! vertices as it is and quadruples the number of regular faces, so the share of bad faces falls fourfold per
//! step while they shrink towards a point. Three steps leave them at tenths of a per cent of the area.
//!
//! What to do with the remainder belongs to a later stage. Here they are counted honestly and returned
//! separately rather than swept aside: an exploration has to name its price, not present a pretty picture.

use super::Cage;

/// A 4×4 Bezier patch, which the kernel accepts without conversion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BezierPatch {
    /// The control grid, indexed as `[row][column]`.
    pub cps: [[[f64; 3]; 4]; 4],
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// One row of a B-spline becomes one row of a Bezier.
///
/// A segment of a uniform cubic B-spline over the points `p0..p3` is the Bezier curve over
/// `(p0+4p1+p2)/6`, `(2p1+p2)/3`, `(p1+2p2)/3`, `(p1+4p2+p3)/6`. An identity, not an approximation.
fn bspline_row_to_bezier(p: [[f64; 3]; 4]) -> [[f64; 3]; 4] {
    [
        scale(add(add(p[0], scale(p[1], 4.0)), p[2]), 1.0 / 6.0),
        scale(add(scale(p[1], 2.0), p[2]), 1.0 / 3.0),
        scale(add(p[1], scale(p[2], 2.0)), 1.0 / 3.0),
        scale(add(add(p[1], scale(p[2], 4.0)), p[3]), 1.0 / 6.0),
    ]
}

impl BezierPatch {
    /// From a 4×4 B-spline control grid: rows first, then columns.
    pub fn from_bspline(net: [[[f64; 3]; 4]; 4]) -> BezierPatch {
        let mut rows = [[[0.0; 3]; 4]; 4];
        for (i, r) in net.iter().enumerate() {
            rows[i] = bspline_row_to_bezier(*r);
        }
        let mut cps = [[[0.0; 3]; 4]; 4];
        for j in 0..4 {
            let col = [rows[0][j], rows[1][j], rows[2][j], rows[3][j]];
            let b = bspline_row_to_bezier(col);
            for (i, v) in b.iter().enumerate() {
                cps[i][j] = *v;
            }
        }
        BezierPatch { cps }
    }

    /// The point of the patch at parameters `u` and `v` in [0, 1].
    pub fn eval(&self, u: f64, v: f64) -> [f64; 3] {
        let bern = |t: f64| {
            let s = 1.0 - t;
            [s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t]
        };
        let (bu, bv) = (bern(u), bern(v));
        let mut out = [0.0; 3];
        for i in 0..4 {
            for j in 0..4 {
                let w = bu[i] * bv[j];
                for k in 0..3 {
                    out[k] += self.cps[i][j][k] * w;
                }
            }
        }
        out
    }
}

/// The result of the conversion: the patches, and an honest count of what was not converted.
#[derive(Clone, Debug, Default)]
pub struct PatchSet {
    pub patches: Vec<BezierPatch>,
    /// Faces touching an extraordinary vertex or a border, which have no exact patch.
    pub irregular: usize,
    /// Their share of the total face count: the price of the approximation, which has to be stated aloud.
    pub irregular_share: f64,
}

impl Cage {
    /// The face adjacent across the edge `(a, b)`, other than `except`.
    fn face_across(&self, a: u32, b: u32, except: usize) -> Option<usize> {
        self.faces.iter().enumerate().find_map(|(fi, f)| {
            if fi == except || f.len() != 4 {
                return None;
            }
            let has = |x: u32| f.contains(&x);
            (has(a) && has(b)).then_some(fi)
        })
    }

    /// The vertex of a quadrilateral opposite `v`.
    fn opposite_in(&self, fi: usize, v: u32) -> Option<u32> {
        let f = &self.faces[fi];
        (f.len() == 4).then(|| f.iter().position(|&x| x == v).map(|k| f[(k + 2) % 4]))?
    }

    /// The wings of the face adjacent across the edge `(a, b)`: the vertices adjacent to `a` and to `b` within
    /// that face, which form the next ring of the control grid beyond the edge itself.
    ///
    /// An earlier version took the opposite vertices instead. The error was quiet: the grid assembled, the
    /// patch built, and the corners drifted 0.1 mm from the limit — too little to catch the eye, and enough for
    /// the kernel to fail to stitch adjacent patches. What caught it was comparing the corners against the
    /// limit points, not a picture.
    fn wing(&self, fi: usize, a: u32, b: u32) -> Option<(u32, u32)> {
        let f = &self.faces[fi];
        if f.len() != 4 {
            return None;
        }
        let nb = |v: u32, not: u32| -> Option<u32> {
            let k = f.iter().position(|&x| x == v)?;
            [f[(k + 1) % 4], f[(k + 3) % 4]].into_iter().find(|&x| x != not)
        };
        Some((nb(a, b)?, nb(b, a)?))
    }

    /// Convert a cage into patches.
    ///
    /// `refine` is how many times to subdivide beforehand. Zero is allowed, but two or three make sense: each
    /// step reduces fourfold the share of faces at extraordinary vertices, for which no exact patch exists.
    pub fn to_bezier_patches(&self, refine: usize) -> PatchSet {
        let c = self.subdivided(refine.max(1)); // at least once: patches are defined only on quadrilaterals
        let mut out = PatchSet::default();
        let total = c.faces.len();
        for fi in 0..c.faces.len() {
            match c.patch_of_face(fi) {
                Some(p) => out.patches.push(p),
                None => out.irregular += 1,
            }
        }
        out.irregular_share = if total == 0 { 0.0 } else { out.irregular as f64 / total as f64 };
        out
    }

    /// For tests: the patch of one face together with the face adjacent across an edge.
    pub fn patch_of_face_for_test(&self, fi: usize) -> Option<BezierPatch> {
        self.patch_of_face(fi)
    }

    /// For tests: the face adjacent across an edge.
    pub fn face_across_for_test(&self, a: u32, b: u32, except: usize) -> Option<usize> {
        self.face_across(a, b, except)
    }

    /// The patch over one face, if that face is regular.
    ///
    /// The 4×4 grid is assembled like this: the face itself sits in the middle as `p11 p12 p22 p21`, the ring
    /// around it comes from the faces adjacent across the edges, and the four corners of the grid come from the
    /// faces that touch a corner by a vertex alone.
    fn patch_of_face(&self, fi: usize) -> Option<BezierPatch> {
        let f = self.faces.get(fi)?;
        if f.len() != 4 {
            return None;
        }
        let (a, b, cc, d) = (f[0], f[1], f[2], f[3]);
        // all four corners have to be interior and of valence four
        for &v in [a, b, cc, d].iter() {
            if self.valence(v as usize) != 4 {
                return None;
            }
        }
        // the neighbours across the edges
        let f_ab = self.face_across(a, b, fi)?;
        let f_bc = self.face_across(b, cc, fi)?;
        let f_cd = self.face_across(cc, d, fi)?;
        let f_da = self.face_across(d, a, fi)?;
        let (a_up, b_up) = self.wing(f_ab, a, b)?;
        let (b_ri, c_ri) = self.wing(f_bc, b, cc)?;
        let (c_dn, d_dn) = self.wing(f_cd, cc, d)?;
        let (d_le, a_le) = self.wing(f_da, d, a)?;

        // the diagonal faces: every vertex has exactly four adjacent faces, and three are already known
        let corner = |v: u32, known: [usize; 3]| -> Option<u32> {
            let fi = (0..self.faces.len()).find(|&k| !known.contains(&k) && self.faces[k].len() == 4 && self.faces[k].contains(&v))?;
            self.opposite_in(fi, v)
        };
        let p00 = corner(a, [fi, f_ab, f_da])?;
        let p03 = corner(b, [fi, f_ab, f_bc])?;
        let p33 = corner(cc, [fi, f_bc, f_cd])?;
        let p30 = corner(d, [fi, f_cd, f_da])?;

        let g = |v: u32| self.verts[v as usize];
        let net = [
            [g(p00), g(a_le), g(d_le), g(p30)],
            [g(a_up), g(a), g(d), g(d_dn)],
            [g(b_up), g(b), g(cc), g(c_dn)],
            [g(p03), g(b_ri), g(c_ri), g(p33)],
        ];
        Some(BezierPatch::from_bspline(net))
    }
}
