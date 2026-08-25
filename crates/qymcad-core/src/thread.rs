//! Generator of threads and augers: geometry from standards rather than from invented coefficients.
//!
//! The rule a professional CAD follows is that the user picks a standard and a size and the system computes the
//! rest — the thread is given by a profile family, a size and a designation such as M10x1.5 together with a fit
//! class, and the geometry comes out of the profile tables (major, pitch and minor diameters for each size and
//! class). Nobody types a "turn depth" by hand. For 3D printing, tables with an enlarged clearance are
//! substituted instead.
//!
//! That is exactly what was missing here: the feature carried a bare angle and a bare depth, and the profile was
//! built from coefficients of the form `0.48·P` and `0.30·P` that were tied to no standard at all.
//!
//! ## What lives here
//!
//! Pure mathematics, with no kernel involved and fully covered by tests: pitch tables, diameter formulas per
//! standard (ISO 68-1 metric, ISO 2901/2904 trapezoidal Tr, ACME, DIN 405 round Rd, buttress) and the axial
//! profile of the groove as exact edges (`ProfEdge`: line segments and arcs), with the crest and the root
//! rounded. The kernel then simply sweeps that profile along a helix.
//!
//! Coordinate system of the profile: `x` runs along the axis, u ∈ [−P/2, P/2], and `y` is radial, zero on the
//! original surface with negative values going into the material. For an internal thread the kernel mirrors the
//! sign.

use crate::geom::{Point2, ProfEdge};

/// The profile family. The angle is the included angle between the flanks in the axial section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ThreadStandard {
    /// ISO 68-1: 60°, H = P·√3/2, crest truncated by H/8, root rounded at R = H/6. The fastening thread and
    /// the most common one.
    MetricIso,
    /// ISO 2901/2904 (Tr): 30°, a power thread and the best profile for 3D printing and lead screws.
    TrapezoidalTr,
    /// ACME, the imperial power thread: 29°, with a flat crest and root.
    Acme,
    /// DIN 405 (Rd), the round thread: 30° with the crest and the root fully rounded. It tolerates dirt,
    /// prints well and has no sharp edges.
    RoundRd,
    /// Buttress: asymmetric, with a 7° working flank and a 45° trailing one.
    Buttress,
    /// A custom profile: the included angle and the depth are entered by hand, for experiments and augers.
    Custom,
}

impl ThreadStandard {
    /// The included angle of the profile, in degrees.
    pub fn angle_deg(self) -> f64 {
        match self {
            ThreadStandard::MetricIso => 60.0,
            ThreadStandard::TrapezoidalTr | ThreadStandard::RoundRd => 30.0,
            ThreadStandard::Acme => 29.0,
            ThreadStandard::Buttress => 52.0, // 7° + 45°
            ThreadStandard::Custom => 60.0,
        }
    }
    /// A catalogue key rather than a word: the core carries no language.
    pub fn label(self) -> &'static str {
        match self {
            ThreadStandard::MetricIso => "thread-std-metric",
            ThreadStandard::TrapezoidalTr => "thread-std-trapezoidal",
            ThreadStandard::Acme => "ACME (29°)",
            ThreadStandard::RoundRd => "thread-std-round",
            ThreadStandard::Buttress => "thread-std-buttress",
            ThreadStandard::Custom => "thread-std-custom",
        }
    }
}

/// The specification of a thread the way a professional CAD takes it: a standard and a size, with the rest
/// computed.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSpec {
    pub standard: ThreadStandard,
    /// Nominal (major) diameter d/D, in mm.
    pub nominal_d: f64,
    /// Pitch P, in mm. Zero means take the standard coarse pitch for this diameter.
    pub pitch: f64,
    /// Number of starts, for a multi-start thread or an auger. The lead is P·starts.
    pub starts: u32,
    /// A left-hand thread.
    pub left: bool,
    /// Internal, in a hole; otherwise external, on a shaft.
    pub internal: bool,
    /// Fit clearance per side, in mm; this is the tolerance class of a professional CAD, and 0.1 to 0.4 works
    /// for 3D printing. An external thread thins by this amount and an internal one thickens, so the pair
    /// screws together.
    pub fit: f64,
    /// Crest rounding radius, in mm. `None` takes the value from the standard.
    pub crest_r: Option<f64>,
    /// Root rounding radius, in mm. `None` takes the value from the standard.
    pub root_r: Option<f64>,
    /// A custom depth, `Custom` only, in mm.
    pub custom_depth: f64,
    /// A custom included angle of the profile, `Custom` only, in degrees. Without it, choosing a custom
    /// profile changed nothing.
    #[serde(default = "default_custom_angle")]
    pub custom_angle: f64,
}

fn default_custom_angle() -> f64 {
    60.0
}

impl Default for ThreadSpec {
    fn default() -> Self {
        Self {
            standard: ThreadStandard::MetricIso,
            nominal_d: 10.0,
            pitch: 0.0,
            starts: 1,
            left: false,
            internal: false,
            fit: 0.0,
            crest_r: None,
            root_r: None,
            custom_depth: 0.0,
            custom_angle: 60.0,
        }
    }
}

/// The computed geometry of a thread: diameters, depth, lead and the axial profile of the groove.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadGeom {
    /// Major diameter, over the crests of an external thread or over the roots of an internal one, in mm.
    pub major_d: f64,
    /// Pitch diameter, in mm.
    pub pitch_d: f64,
    /// Minor diameter, at the root of an external thread or the crest of an internal one, in mm.
    pub minor_d: f64,
    /// Radial depth of the thread, in mm.
    pub depth: f64,
    /// Pitch, in mm.
    pub pitch: f64,
    /// Lead per revolution, pitch · starts, in mm.
    pub lead: f64,
    /// Included angle of the profile, in degrees.
    pub angle_deg: f64,
    /// Diameter of the stock the groove is cut from: major for an external thread, minor for an internal one.
    pub stock_d: f64,
    /// The flat left on the crest by the standard, in mm. It is what the pitch has to share with the groove,
    /// so the answer about how much clearance fits is computed from it.
    pub crest_flat: f64,
    /// Axial profile of the groove as exact edges; x runs along the axis and y radially inwards, so y ≤ 0.
    pub groove: Vec<ProfEdge>,
}

/// The standard coarse pitch of a metric thread (ISO 261) for a nominal diameter.
pub fn metric_coarse_pitch(d: f64) -> f64 {
    const TABLE: &[(f64, f64)] = &[
        (1.0, 0.25),
        (1.2, 0.25),
        (1.6, 0.35),
        (2.0, 0.4),
        (2.5, 0.45),
        (3.0, 0.5),
        (4.0, 0.7),
        (5.0, 0.8),
        (6.0, 1.0),
        (8.0, 1.25),
        (10.0, 1.5),
        (12.0, 1.75),
        (16.0, 2.0),
        (20.0, 2.5),
        (24.0, 3.0),
        (30.0, 3.5),
        (36.0, 4.0),
        (42.0, 4.5),
        (48.0, 5.0),
        (56.0, 5.5),
        (64.0, 6.0),
    ];
    let mut best = TABLE[0];
    for &(dd, p) in TABLE {
        if d + 1e-9 >= dd {
            best = (dd, p);
        }
    }
    best.1
}

/// The standard pitch of a trapezoidal Tr thread (ISO 2901, coarse series).
pub fn tr_coarse_pitch(d: f64) -> f64 {
    const TABLE: &[(f64, f64)] = &[(8.0, 1.5), (10.0, 2.0), (12.0, 3.0), (16.0, 4.0), (20.0, 4.0), (24.0, 5.0), (28.0, 5.0), (32.0, 6.0), (40.0, 7.0), (48.0, 8.0), (60.0, 9.0), (80.0, 10.0)];
    let mut best = TABLE[0];
    for &(dd, p) in TABLE {
        if d + 1e-9 >= dd {
            best = (dd, p);
        }
    }
    best.1
}

/// ISO 2901: the axial clearance `ac` of a trapezoidal thread as a function of the pitch.
fn tr_clearance(pitch: f64) -> f64 {
    if pitch <= 1.5 + 1e-9 {
        0.15 // ISO 2901, fine pitches: Tr8×1.5 gives h3 = 0.9 and d3 = 6.2 per the ISO 2904 table
    } else if pitch <= 5.0 + 1e-9 {
        0.25
    } else if pitch <= 12.0 + 1e-9 {
        0.5
    } else {
        1.0
    }
}

impl ThreadSpec {
    /// The effective pitch: the one given, or the standard coarse pitch for this standard and diameter.
    pub fn effective_pitch(&self) -> f64 {
        if self.pitch > 1e-9 {
            return self.pitch;
        }
        match self.standard {
            ThreadStandard::TrapezoidalTr | ThreadStandard::Acme => tr_coarse_pitch(self.nominal_d),
            _ => metric_coarse_pitch(self.nominal_d),
        }
    }

    /// The full geometry: diameters per the standard plus the groove profile as exact edges.
    pub fn geometry(&self) -> ThreadGeom {
        let p = self.effective_pitch().max(1e-6);
        let d = self.nominal_d.max(1e-6);
        let angle = if self.standard == ThreadStandard::Custom && self.custom_angle > 1.0 { self.custom_angle } else { self.standard.angle_deg() };
        // ── diameters and depth per the standard ───────────────────────────────────────────────
        let (depth, pitch_d, minor_d, crest_flat, std_root_r, std_crest_r) = match self.standard {
            ThreadStandard::MetricIso => {
                // ISO 68-1: H = P·√3/2, crest truncation H/8, root rounded at R = H/6, h3 = 0.6134·P
                let h = p * 3f64.sqrt() / 2.0;
                (0.613_434 * p, d - 0.649_519 * p, d - 1.226_869 * p, p / 8.0, h / 6.0, 0.0)
            }
            ThreadStandard::TrapezoidalTr => {
                // ISO 2901: H1 = 0.5P, h3 = 0.5P + ac, d2 = d − 0.5P, d3 = d − 2h3; R1 = 0.5ac, R2 = ac
                let ac = tr_clearance(p);
                let h3 = 0.5 * p + ac;
                (h3, d - 0.5 * p, d - 2.0 * h3, 0.366 * p, ac, 0.5 * ac)
            }
            ThreadStandard::Acme => {
                // ACME: h = 0.5P plus backlash, with crest and root flats of 0.3707P
                let h = 0.5 * p + 0.01 * p;
                (h, d - 0.5 * p, d - 2.0 * h, 0.3707 * p, 0.05 * p, 0.05 * p)
            }
            ThreadStandard::RoundRd => {
                // DIN 405: 30°, crest and root fully rounded at R ≈ 0.24P, h ≈ 0.5P.
                // The crest flat is not specified here — the radius itself sets it, since the arcs of
                // neighbouring grooves have to meet at the crest. Setting it to zero made the groove occupy
                // the whole pitch, left no crest at all, and then squeezed the radius down to 0.03 mm: a round
                // thread came out straight.
                let (h, r) = (0.5 * p, 0.24 * p);
                let (t, k) = (15f64.to_radians().tan(), 1.0 / 15f64.to_radians().cos());
                (h, d - 0.5 * p, d - 2.0 * h, 2.0 * r * (k - t), r, r)
            }
            ThreadStandard::Buttress => {
                let h = 0.75 * p;
                (h, d - 0.75 * p, d - 2.0 * h, 0.25 * p, 0.1 * p, 0.05 * p)
            }
            ThreadStandard::Custom => {
                let h = if self.custom_depth > 1e-9 { self.custom_depth } else { 0.6 * p };
                (h, d - h, d - 2.0 * h, 0.1 * p, 0.05 * p, 0.05 * p)
            }
        };
        let major_d = d;
        // ── fit clearance: an external thread thins and an internal one thickens, so the pair screws
        //    together ──────────────────────────────────────────────────────────────────────────────
        let fit = self.fit.max(0.0);
        let crest_r = self.crest_r.unwrap_or(std_crest_r).max(0.0);
        let root_r = self.root_r.unwrap_or(std_root_r).max(0.0);
        // The fit clearance acts both along the axis and radially. Axial alone is not enough: the crest of
        // the bolt would press against the root of the nut, both sitting at the nominal diameter, and a printed
        // pair would not screw together at all — observed on a Ø20 pair with fits of 0.2 and 0.4 and zero
        // radial clearance. The groove deepens by the same amount, so the pair gains clearance on both sides of
        // the thread.
        let groove = self.groove_profile(p, depth + fit, angle, crest_flat, crest_r, root_r, fit);
        ThreadGeom {
            major_d,
            pitch_d,
            minor_d,
            depth,
            pitch: p,
            lead: p * self.starts.max(1) as f64,
            angle_deg: angle,
            stock_d: if self.internal { minor_d } else { major_d },
            crest_flat,
            groove,
        }
    }

    /// IS THERE ROOM IN THE PITCH FOR THE CLEARANCE ASKED FOR?
    ///
    /// The clearance widens the groove: an external thread thins by it and an internal one thickens, and that
    /// is what gives the pair its play. But the groove's half-width is capped at 0.49 of the pitch - beyond
    /// that neighbouring grooves would meet - and a metric profile already sits at 0.4375 of it with no
    /// clearance at all. About 0.05 of the pitch is left: 0.13 mm at a pitch of 2.5, 0.08 mm at 1.5.
    ///
    /// Asking for more used to be taken in silence. Measured: at M20x2.5 a fit of 0.2 and one of 0.4 produced
    /// bit-identical bodies, both binding against the mating part by 264.3 mm^3 - two different numbers typed
    /// in, one ceiling reached, and nothing said about it.
    ///
    /// Returns `None` when the clearance fits, otherwise (the clearance asked for, the one that fits).
    pub fn fit_overflow(&self) -> Option<(f64, f64)> {
        if self.fit <= 0.0 {
            return None;
        }
        let g = self.geometry();
        let p = g.pitch;
        if p <= 1e-9 {
            return None;
        }
        // The same arithmetic the profile is built from: half-width = (pitch - crest flat)/2 + fit, capped at
        // 0.49 of the pitch. What is left over for the clearance is the difference.
        let bare = (p - g.crest_flat) * 0.5;
        let room = (0.49 * p - bare).max(0.0);
        (self.fit > room + 1e-9).then_some((self.fit, room))
    }

    /// HOW MUCH THE CREST HAS TO COME DOWN so that the clearance asked for is actually given.
    ///
    /// The clearance is taken by widening the groove, and the width is capped by the pitch (see
    /// [`Self::fit_overflow`]). What will not fit there has to be taken RADIALLY, the way a real fit class
    /// does it: the bolt's diameters are reduced by the allowance, the nut's are opened up.
    ///
    /// The two are not interchangeable one for one. A flank stands at the half-angle from the radial, so
    /// widening the groove by `w` gives `w·cos(beta)` of room measured along the flank's normal, while moving
    /// the profile radially by `e` gives `e·sin(beta)`. Matching them gives `e = missing / tan(beta)`.
    ///
    /// Returns 0 when the whole clearance fits inside the groove.
    pub fn radial_relief(&self) -> f64 {
        let Some((asked, given)) = self.fit_overflow() else { return 0.0 };
        let g = self.geometry();
        let t = (g.angle_deg * 0.5).to_radians().tan();
        if t <= 1e-9 {
            return 0.0;
        }
        ((asked - given) / t).max(0.0)
    }

    /// DOES THE PROFILE FIT INSIDE THE PITCH?
    ///
    /// A groove wider than the pitch cannot be cut: the next pass runs into the previous turn and takes it
    /// away. Nothing is left between the turns but a thin plate, and the result mates with nothing. On a
    /// lathe this is impossible - the tool would destroy the thread it had just cut - and the program must
    /// not pretend otherwise: parameters like these were accepted in silence and produced rubbish.
    ///
    /// Returns `None` when the profile fits. Otherwise (the groove's width, the greatest depth that fits at
    /// this angle and pitch, the smallest pitch that fits at this depth) - three numbers, so a person can be
    /// told what to change rather than merely that something is wrong.
    pub fn profile_overflow(&self) -> Option<(f64, f64, f64)> {
        let g = self.geometry();
        let t = (g.angle_deg * 0.5).to_radians().tan();
        if t <= 1e-9 || g.pitch <= 1e-9 {
            return None;
        }
        // THE GROOVE'S HALF-WIDTH AT THE SURFACE, by the same arithmetic the profile is built from: what the
        // pitch has left over after the crest's flat, plus the clearance, and capped so that neighbouring
        // grooves keep a web between them.
        //
        // The first version of this compared the V's own width at full depth against the pitch. That is not
        // what the builder does, and it refused ordinary threads: a real part's fillet lost all 36 of its edge
        // names because a legitimate thread beside it stopped building.
        let wt = ((g.pitch - g.crest_flat) * 0.5 + self.fit).clamp(0.02 * g.pitch, 0.49 * g.pitch);
        // The flanks close to a point at this depth, so a deeper groove than this simply cannot be cut - what
        // comes out is a shallower V and a sliver of land between the turns.
        let max_depth = wt / t;
        (g.depth > max_depth + 1e-9).then(|| {
            // The pitch that would hold this depth: the cap is what binds, so 0.49 of the pitch has to reach
            // the depth's own half-width.
            let min_pitch = g.depth * t / 0.49;
            (2.0 * g.depth * t, max_depth, min_pitch)
        })
    }

    /// THE COUNTERPART THREAD — the one that has to be cut on the mating part.
    ///
    /// A thread is only ever half of a pair, and the other half is not a matter of taste: the standard, the
    /// nominal diameter, the pitch, the number of starts, the hand and the fit all have to be the same, and
    /// only the side flips. The fit is deliberately carried over unchanged — it thins an external thread and
    /// thickens an internal one by the same amount, so one value gives the pair its clearance.
    pub fn mating(&self) -> Self {
        Self { internal: !self.internal, ..self.clone() }
    }

    /// WHAT THE BLANK MUST BE for this thread and for its counterpart, in millimetres.
    ///
    /// Returns (the diameter this thread is cut from, the diameter the counterpart is cut from). For an
    /// external thread that is the shaft; for an internal one it is the drilled hole - the tap drill, the
    /// number a person otherwise looks up in a table.
    pub fn blank_diameters(&self) -> (f64, f64) {
        (self.geometry().stock_d, self.mating().geometry().stock_d)
    }

    /// The axial profile of the groove as exact edges: straight flanks plus rounding arcs at the root and the
    /// crest. `x` runs along the axis and `y` radially, with 0 at the surface of the stock and negative values
    /// going into the material.
    ///
    /// Everything is built strictly by tangency: every arc touches both the flank and the line it adjoins,
    /// whether the root or the surface. An earlier version placed the root arc by eye, putting its centre on
    /// the axis at height `root + r`, which on a round Rd Ø30×3.5 produced an actual radius of 1.43 instead of
    /// 0.79 and dropped the groove below its own depth.
    ///
    /// The governing invariant: the profile fits entirely within half a pitch on either side, |x| ≤ p/2.
    /// Otherwise neighbouring turns of the groove overlap, the swept body self-intersects and the boolean
    /// returns a torn surface. That is exactly what spoiled the metric profile: the overshoot above the surface
    /// continued the flanks outwards, and a half-width of 1.834 at a pitch of 3.5 reached into the neighbouring
    /// turn. The overshoot now runs strictly vertically, which does not affect the cut, being entirely outside
    /// the material, and rules the overlap out.
    #[allow(clippy::too_many_arguments)]
    fn groove_profile(&self, p: f64, depth: f64, angle_deg: f64, crest_flat: f64, crest_r: f64, root_r: f64, fit: f64) -> Vec<ProfEdge> {
        let beta = (angle_deg.to_radians() * 0.5).clamp(1e-3, 1.3); // half-angle of the profile from the radial
        let t = beta.tan();
        let k = 1.0 / beta.cos(); // = √(1+t²), the length of the normal to the flank
        let h = depth.max(1e-6);
        let over = 0.15 * p; // overshoot past the surface, so the boolean cuts without tangent faces
        // Half-width of the groove at the surface and at the root. The fit clearance widens the groove: the
        // thread thins on an external one and the nut thickens on an internal one.
        let wt = ((p - crest_flat) * 0.5 + fit).clamp(0.02 * p, 0.49 * p);
        let wb = (wt - h * t).max(0.0);
        let y_bot = -h;

        // ── crest: an arc tangent to the flank and to the line of the surface ─────────────────────
        // The crest is the material between neighbouring grooves, so it is rounded by truncating the upper
        // corner of the groove. The centre of the arc lies above the groove, at y = −r with x > wt, so the arc
        // bites into the thread. There is one hard limit: the arc must not cross half a pitch, or neighbouring
        // grooves would meet.
        //
        // A web has to remain between neighbouring turns. Exactly half a pitch is not enough: on a round Rd
        // with a fit the crest arcs met precisely, the turns touched, and the resulting body came out
        // self-intersecting — a section then showed seven segment crossings and a torn fill, while a case
        // without contact sectioned cleanly. Two per cent of the pitch is kept.
        let rc = crest_r.max(0.0).min(((p * 0.49 - wt) / (k - t)).max(0.0)).min(h * 0.45);
        let x_out = wt + rc * (k - t); // where the crest arc would touch the surface
        let crest_foot = Point2::new(x_out - rc / k, -rc + rc * t / k); // where the arc meets the flank
        // The crest arc is deliberately not carried all the way to tangency with the surface of the stock. A
        // tangential meeting between the tool and a cylinder is a classic boolean hazard: instead of a clean
        // intersection it yields a band of slivers. Measured on M30×3.5: a sharp crest gives 5 044 triangles,
        // while the same thread with the arc carried to tangency gives 436 862. The arc is cut short at a
        // micro-step, a few per cent of the radius and a handful of microns, and continues vertically: the cut
        // becomes transverse and the step is smaller than the layer of any printer.
        let eps = if rc > 1e-9 { (0.02 * rc).min(0.02 * h) } else { 0.0 };
        let x_wall = if rc > 1e-9 { x_out - (eps * (2.0 * rc - eps)).max(0.0).sqrt() } else { wt };
        let y_wall = -eps;
        let flank_top = if rc > 1e-9 { crest_foot } else { Point2::new(wt, 0.0) };

        // ── root of the groove ────────────────────────────────────────────────────────────────────
        // Two cases, both genuinely tangent:
        //  * the radius fits into the corners, giving two corner arcs with a flat root between them;
        //  * the radius is larger, and the root is rounded entirely by a single arc tangent to both flanks.
        // The boundary between them is `r_full`, at which the corner arcs meet exactly on the axis.
        //
        // The root radius is capped at the value where the root still reaches its own depth: a larger arc sits
        // higher and makes the groove shallower. On a Tr thread of pitch 0.5 the tabulated radius of 0.15 gave
        // a depth of 0.16 instead of 0.40, which is a different thread altogether. Depth outranks rounding: it
        // sets the minor diameter by which the pair screws together.
        let r_full = if k - t > 1e-9 { wb / (k - t) } else { 0.0 };
        let rr = root_r.max(0.0).min(h * 0.45).min(r_full.max(0.0));
        let mut root: Vec<ProfEdge> = Vec::new();
        let flank_bot; // where the right flank meets the root
        if rr <= 1e-9 {
            flank_bot = Point2::new(wb, y_bot);
            root.push(ProfEdge::Line { a: flank_bot, b: Point2::new(-wb, y_bot) });
        } else if rr < r_full - 1e-6 * p {
            // The tolerance is relative: on a metric thread the tabulated root radius (H/6) coincides with
            // the limiting one up to rounding, and an absolute tolerance left a root flat 1e-17 wide, which is
            // degenerate. Such near-coincident cases are treated as full rounding.
            let xc = wb - rr * (k - t); // centre of the corner arc
            let c = Point2::new(xc, y_bot + rr);
            flank_bot = Point2::new(xc + rr / k, y_bot + rr - rr * t / k);
            let touch = Point2::new(xc, y_bot); // tangency with the flat root
            root.push(ProfEdge::Arc { a: flank_bot, b: touch, center: c, ccw: false });
            root.push(ProfEdge::Line { a: touch, b: Point2::new(-touch.x, touch.y) });
            root.push(ProfEdge::Arc { a: Point2::new(-touch.x, touch.y), b: Point2::new(-flank_bot.x, flank_bot.y), center: Point2::new(-c.x, c.y), ccw: false });
        } else {
            // Full rounding of the root: take exactly the limiting radius. Computing from a near-limiting one
            // is not possible — the centre drifts downwards with a lever of k/t and the root drops below its
            // own depth.
            let rr = r_full;
            let yc = y_bot + rr; // the arc touches both flanks and its lowest point is exactly at depth
            flank_bot = Point2::new(rr / k, yc - rr * t / k);
            root.push(ProfEdge::Arc { a: flank_bot, b: Point2::new(-flank_bot.x, flank_bot.y), center: Point2::new(0.0, yc), ccw: false });
        }

        // ── assembly in traversal order: right overshoot down, right flank, root, left flank, left
        //    overshoot ─────────────────────────────────────────────────────────────────────────────
        let mut e = Vec::new();
        e.push(ProfEdge::Line { a: Point2::new(x_wall, over), b: Point2::new(x_wall, y_wall) });
        if rc > 1e-9 {
            // `ccw = true`. The traversal of the crest arc was specified wrongly and the arc took the long
            // way round: 312° instead of 48°. Sweeping nearly a full circle, it tore the geometry of the thread
            // and inflated the face to hundreds of thousands of triangles — on M30×3.5, from five thousand to
            // 437 thousand. That alone was what made the thread look distorted.
            e.push(ProfEdge::Arc { a: Point2::new(x_wall, y_wall), b: crest_foot, center: Point2::new(x_out, -rc), ccw: true });
        }
        e.push(ProfEdge::Line { a: flank_top, b: flank_bot });
        e.extend(root);
        e.push(ProfEdge::Line { a: Point2::new(-flank_bot.x, flank_bot.y), b: Point2::new(-flank_top.x, flank_top.y) });
        if rc > 1e-9 {
            e.push(ProfEdge::Arc { a: Point2::new(-crest_foot.x, crest_foot.y), b: Point2::new(-x_wall, y_wall), center: Point2::new(-x_out, -rc), ccw: true });
        }
        e.push(ProfEdge::Line { a: Point2::new(-x_wall, y_wall), b: Point2::new(-x_wall, over) });
        e.push(ProfEdge::Line { a: Point2::new(-x_wall, over), b: Point2::new(x_wall, over) });
        // Degenerate edges of zero length must not reach the wire: the kernel stumbles on them. They arise
        // legitimately — on a metric thread the root closes to a point, and on Rd the crest flat all but
        // disappears.
        e.retain(|edge| match *edge {
            ProfEdge::Line { a, b } => a.dist(b) > 1e-9,
            _ => true,
        });
        // ── internal thread: the material lies outside the surface of the hole ────────────────────
        // The groove profile is built inwards, y < 0, which is right for a shaft. Inside a hole, inwards means
        // into the void: the groove cut air, only the overshoot reached the body, and flat discs appeared on
        // screen instead of turns — a reproduction with an internal ACME Ø30 P5 removed 1.2 cm³ instead of
        // 26 cm³. Mirroring the profile radially sends the groove into the wall and the overshoot into the
        // hole.
        if self.internal {
            for edge in &mut e {
                *edge = match *edge {
                    ProfEdge::Line { a, b } => ProfEdge::Line { a: Point2::new(a.x, -a.y), b: Point2::new(b.x, -b.y) },
                    ProfEdge::Arc { a, b, center, ccw } => ProfEdge::Arc {
                        a: Point2::new(a.x, -a.y),
                        b: Point2::new(b.x, -b.y),
                        center: Point2::new(center.x, -center.y),
                        ccw: !ccw, // mirroring reverses the traversal direction of an arc
                    },
                    ProfEdge::Circle { center, r } => ProfEdge::Circle { center: Point2::new(center.x, -center.y), r },
                };
            }
        }
        e
    }
}

/// An auger, that is a screw conveyor: not a thread but a helical flight added onto a shaft. This was missing
/// entirely, because the tool could only cut a groove.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AugerSpec {
    /// Shaft diameter, the inner one, in mm.
    pub shaft_d: f64,
    /// Outer diameter of the flight, in mm.
    pub outer_d: f64,
    /// Pitch, the lead per revolution, in mm.
    pub pitch: f64,
    /// Thickness of the flight, in mm.
    pub thickness: f64,
    /// Number of starts.
    pub starts: u32,
    /// Left-hand direction.
    pub left: bool,
    /// Rounding of the edges of the flight, in mm; zero leaves them sharp.
    pub edge_r: f64,
}

impl Default for AugerSpec {
    fn default() -> Self {
        Self { shaft_d: 10.0, outer_d: 30.0, pitch: 20.0, thickness: 3.0, starts: 1, left: false, edge_r: 0.0 }
    }
}

impl AugerSpec {
    /// Radial height of the flight, in mm.
    pub fn flight_height(&self) -> f64 {
        ((self.outer_d - self.shaft_d) * 0.5).max(0.0)
    }

    /// The axial section of the flight as exact edges: a rectangle of height by thickness with rounded edges.
    /// The coordinates match those of a thread — `x` along the axis and `y` radial, with 0 at the surface of
    /// the shaft and positive values pointing outwards, since a flight is welded on rather than cut away.
    pub fn flight_profile(&self) -> Vec<ProfEdge> {
        let h = self.flight_height();
        let t = self.thickness.max(1e-6);
        let r = self.edge_r.max(0.0).min(t * 0.49).min(h * 0.49);
        let (x0, x1) = (-t / 2.0, t / 2.0);
        let (y0, y1) = (-0.05 * h, h); // the bottom is sunk into the shaft, giving a clean boolean union
        let mut e = Vec::new();
        if r <= 1e-9 {
            e.push(ProfEdge::Line { a: Point2::new(x0, y0), b: Point2::new(x1, y0) });
            e.push(ProfEdge::Line { a: Point2::new(x1, y0), b: Point2::new(x1, y1) });
            e.push(ProfEdge::Line { a: Point2::new(x1, y1), b: Point2::new(x0, y1) });
            e.push(ProfEdge::Line { a: Point2::new(x0, y1), b: Point2::new(x0, y0) });
            return e;
        }
        // round only the outer edges, the ones in the material flow; the bottom goes into the shaft
        e.push(ProfEdge::Line { a: Point2::new(x0, y0), b: Point2::new(x1, y0) });
        e.push(ProfEdge::Line { a: Point2::new(x1, y0), b: Point2::new(x1, y1 - r) });
        e.push(ProfEdge::Arc { a: Point2::new(x1, y1 - r), b: Point2::new(x1 - r, y1), center: Point2::new(x1 - r, y1 - r), ccw: true });
        e.push(ProfEdge::Line { a: Point2::new(x1 - r, y1), b: Point2::new(x0 + r, y1) });
        e.push(ProfEdge::Arc { a: Point2::new(x0 + r, y1), b: Point2::new(x0, y1 - r), center: Point2::new(x0 + r, y1 - r), ccw: true });
        e.push(ProfEdge::Line { a: Point2::new(x0, y1 - r), b: Point2::new(x0, y0) });
        e
    }

    /// Lead per revolution, accounting for the number of starts, in mm.
    pub fn lead(&self) -> f64 {
        self.pitch * self.starts.max(1) as f64
    }
}

/// Encoding of a groove or flight profile for the kernel (`Shape::helical_profile`), in the same exact-profile
/// format as extrusion uses (`geom::encode_profile`): segments and arcs stay exact, so a finished thread can be
/// chamfered and filleted.
///
/// The provenance of the edges is always zero here: a thread profile is computed from a standard rather than
/// drawn in a sketch.
pub fn encode_edges(edges: &[ProfEdge]) -> Vec<f64> {
    crate::geom::encode_loops(&[edges])
}
