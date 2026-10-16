//! THE SURFACES A REGION IS FITTED TO: their kinds, helices and coils, the tolerance.

use super::*;

/// A surface a region lies on.
#[derive(Clone, Debug, PartialEq)]
pub enum Surface {
    Plane {
        point: [f64; 3],
        normal: [f64; 3],
    },
    /// `point` lies on the axis, `axis` is a unit vector.
    Cylinder {
        point: [f64; 3],
        axis: [f64; 3],
        radius: f64,
    },
    Sphere {
        center: [f64; 3],
        radius: f64,
    },
    /// `apex` is the tip, `axis` a unit vector from the tip into the cone, `half_angle` in radians.
    Cone {
        apex: [f64; 3],
        axis: [f64; 3],
        half_angle: f64,
    },
    /// `axis` is a unit vector square to the torus's middle plane through `center`; `major` is the radius of the
    /// tube's centre line, `minor` the tube's.
    Torus {
        center: [f64; 3],
        axis: [f64; 3],
        major: f64,
        minor: f64,
    },
    /// A SMOOTH WALL FITTED BY ONE B-SPLINE SURFACE through the corners of the whole region - a free form known to the
    /// last pole, where `Free` leaves it to the kernel to fill the border.
    Spline(Box<crate::bspline::BSpline>),
    /// No surface of the kinds above: a smooth free form, its triangles all facing within a right angle of `normal`, the
    /// mean of them, which points out. What it is exactly the kernel finds, filling the face's own border.
    Free {
        normal: [f64; 3],
    },
    /// A straight profile screwed about an axis: the flank of a thread, an auger's flight. `point` lies on the axis;
    /// `axis` and `reference` are unit vectors square to each other. A point at distance r from the axis, height z along
    /// it from `point` and angle t from `reference`, turning right-handed about `axis`, lies on it where
    /// `radial * r + axial * (z - rise * t) = offset` with t taken some whole number of turns on: `rise` is the height
    /// gained per radian, negative for a left-hand screw, and (`radial`, `axial`) the unit normal of the profile's line
    /// in the cut through the axis, `axial` positive. Its own normal points the way (`radial`, `axial`) does.
    Helix {
        point: [f64; 3],
        axis: [f64; 3],
        reference: [f64; 3],
        rise: f64,
        radial: f64,
        axial: f64,
        offset: f64,
    },
    /// A circle screwed about an axis: a rounded edge of an auger's flight, the root of a thread drawn round. As `Helix`,
    /// but a point lies on it where, taken the whole turns that bring z - rise * t nearest `height`, its distance from the
    /// axis and z - rise * t lie `round` from (`middle`, `height`). Its own normal points away from the circle's middle.
    RoundHelix {
        point: [f64; 3],
        axis: [f64; 3],
        reference: [f64; 3],
        rise: f64,
        middle: f64,
        height: f64,
        round: f64,
    },
    /// A round wire wound about an axis: a spring's coil. `point` lies on the axis; `axis` and `reference` are unit
    /// vectors square to each other. The wire's middle runs `radius` from the axis at angle t from `reference`, turning
    /// right-handed about `axis`, `lift + rise * t` along it; the surface is every point `wire` from that line. Its own
    /// normal points away from the line.
    Coil {
        point: [f64; 3],
        axis: [f64; 3],
        reference: [f64; 3],
        rise: f64,
        radius: f64,
        lift: f64,
        wire: f64,
    },
}

impl Surface {
    /// How far `v` lies from the surface, in mm.
    pub fn distance(&self, v: [f64; 3]) -> f64 {
        distance(self, &Vector3::from(v))
    }

    /// How far `v` lies off a helix, a round helix or a coil, the side its own normal points to positive; zero on any other
    /// surface.
    pub(crate) fn signed_off(&self, v: &Vector3<f64>) -> f64 {
        match self {
            Surface::Coil { .. } => coil_at(self, v).map_or(0.0, |c| c.off),
            _ => helix_at(self, v).map_or(0.0, |h| h.off),
        }
    }

    /// Where `v` stands on a helix: its angle round the axis with the whole turns that put it on the helix, and its
    /// distance from the axis - on a round helix its angle round the profile's circle. `None` on any other surface.
    pub fn turn_at(&self, v: [f64; 3]) -> Option<(f64, f64)> {
        match self {
            Surface::Coil { .. } => coil_at(self, &Vector3::from(v)).map(|c| (c.turn, 0.0)),
            _ => helix_at(self, &Vector3::from(v)).map(|h| (h.turn, h.across)),
        }
    }

    /// The unit normal of the surface at the point nearest `v`, or `None` where it has none there.
    pub fn normal_at(&self, v: [f64; 3]) -> Option<[f64; 3]> {
        normal_at(self, &Vector3::from(v)).map(Into::into)
    }
}

/// Where a point stands about a helix.
pub(crate) struct OnHelix {
    /// The angle round the axis, with the whole turns that bring the point onto the helix.
    pub(crate) turn: f64,
    /// Its distance from the axis, or on a round helix its angle round the profile's circle from the way out.
    pub(crate) across: f64,
    /// The helix's own normal there, not of unit length.
    pub(crate) normal: Vector3<f64>,
    /// How far off the helix the point is, the side the normal points positive: the miss within half a turn, divided by
    /// how steeply it grows.
    pub(crate) off: f64,
}

pub(crate) fn helix_at(s: &Surface, v: &Vector3<f64>) -> Option<OnHelix> {
    if let Surface::RoundHelix { point, axis, reference, rise, middle, height, round } = *s {
        let (a, x) = (Vector3::from(axis), Vector3::from(reference));
        let d = v - Vector3::from(point);
        let z = d.dot(&a);
        let e = d - a * z;
        let far = e.norm();
        let out = e.try_normalize(1e-12)?;
        let seen = out.dot(&a.cross(&x)).atan2(out.dot(&x));
        let pitch = 2.0 * std::f64::consts::PI * rise;
        let lifted = z - rise * seen;
        let turns = if pitch.abs() > 1e-300 { ((lifted - height) / pitch).round() } else { 0.0 };
        let (dr, dz) = (far - middle, lifted - pitch * turns - height);
        let apart = dr.hypot(dz).max(1e-300);
        let normal = out * (dr / apart) + (a - a.cross(&out) * (rise / far)) * (dz / apart);
        return Some(OnHelix { turn: seen + 2.0 * std::f64::consts::PI * turns, across: dz.atan2(dr), off: (apart - round) / normal.norm(), normal });
    }
    let Surface::Helix { point, axis, reference, rise, radial, axial, offset } = *s else { return None };
    let (a, x) = (Vector3::from(axis), Vector3::from(reference));
    let d = v - Vector3::from(point);
    let z = d.dot(&a);
    let e = d - a * z;
    let far = e.norm();
    let out = e.try_normalize(1e-12)?;
    let round = a.cross(&out);
    let seen = out.dot(&a.cross(&x)).atan2(out.dot(&x));
    let period = 2.0 * std::f64::consts::PI * axial * rise;
    let raw = radial * far + axial * (z - rise * seen) - offset;
    let turns = if period.abs() > 1e-300 { (raw / period).round() } else { 0.0 };
    let normal = out * radial + (a - round * (rise / far)) * axial;
    Some(OnHelix { turn: seen + 2.0 * std::f64::consts::PI * turns, across: far, off: (raw - period * turns) / normal.norm(), normal })
}

/// Where a point stands about a coil.
pub(crate) struct OnCoil {
    /// The angle round the axis of the nearest point of the wire's middle, with its whole turns.
    pub(crate) turn: f64,
    /// The way from that point out to `v`, not of unit length.
    pub(crate) out: Vector3<f64>,
    /// How far off the coil `v` is, outside positive.
    pub(crate) off: f64,
}

/// Where `v` stands about a coil: the nearest point of the wire's middle, found from the angle `v` stands at - with the
/// whole turns its height gives - by Newton's steps on (v - H(t)) . H'(t) = 0.
pub(crate) fn coil_at(s: &Surface, v: &Vector3<f64>) -> Option<OnCoil> {
    let Surface::Coil { point, axis, reference, rise, radius, lift, wire } = *s else { return None };
    let (o, a, x) = (Vector3::from(point), Vector3::from(axis), Vector3::from(reference));
    let y = a.cross(&x);
    let d = v - o;
    let seen = d.dot(&y).atan2(d.dot(&x));
    let tau = 2.0 * std::f64::consts::PI;
    let mut turn = if rise.abs() > 1e-12 { seen + tau * (((d.dot(&a) - lift) / rise - seen) / tau).round() } else { seen };
    for _ in 0..6 {
        let (sin, cos) = turn.sin_cos();
        let at = o + (x * cos + y * sin) * radius + a * (lift + rise * turn);
        let ahead = (y * cos - x * sin) * radius + a * rise;
        let bend = -(x * cos + y * sin) * radius;
        let slope = -ahead.norm_squared() + (v - at).dot(&bend);
        if slope.abs() < 1e-300 {
            break;
        }
        turn -= (v - at).dot(&ahead) / slope;
    }
    let (sin, cos) = turn.sin_cos();
    let out = v - (o + (x * cos + y * sin) * radius + a * (lift + rise * turn));
    Some(OnCoil { turn, off: out.norm() - wire, out })
}

/// The triangles of one region, and the surface they lie on; `None` where no surface was found.
#[derive(Clone, Debug)]
pub struct Region {
    pub tris: Vec<u32>,
    pub surface: Option<Surface>,
}

/// How strictly a region is grown.
#[derive(Clone, Copy, Debug)]
pub struct Tolerance {
    /// How far a corner may lie from the surface, in mm.
    pub distance: f64,
    /// How far a triangle's normal may turn from the surface's, in degrees.
    pub angle_deg: f64,
    /// An edge sharper than this, in degrees, is never crossed.
    pub sharp_deg: f64,
}

impl Tolerance {
    /// A hundred-thousandth of the diagonal: 0.00046 mm on a 46 mm bar. A mesh from CAD puts its corners on the
    /// surface to 1e-10, and a text STL rounds them to six digits - 2e-4 mm on that bar - so this is room enough
    /// for both. It is not wider because tangent neighbours come close: measured on the rounded bar, the corners of
    /// a corner sphere's triangles next to a rounding lie 0.0025 mm off its cylinder, and a ten-thousandth of the
    /// diagonal let them in. The normal may turn 10 deg - a guard against a folded triangle, not what tells faces
    /// apart: the distance does that. At 2 deg the big skewed triangles along a cone's base, lying on the cone to the
    /// last digit, stood apart as 32 regions of one or two.
    pub fn for_mesh(p: &Prepared) -> Tolerance {
        let d = p.mesh.bounds().map(|b| ((b.max.x - b.min.x).powi(2) + (b.max.y - b.min.y).powi(2) + (b.max.z - b.min.z).powi(2)).sqrt()).unwrap_or(1.0);
        Tolerance { distance: (d * 1e-5).max(1e-7), angle_deg: 10.0, sharp_deg: 30.0 }
    }
}

pub(crate) const NONE: u32 = u32::MAX;

/// How much more area a cylinder must cover than a plane, or a sphere than either, to be taken instead.
pub(crate) const SIMPLER_WINS: f64 = 1.5;

/// The share of a region that a refit may leave behind: the triangles that no longer lie on the better surface go
/// back to be taken by their own.
pub(crate) const MAY_LEAVE: usize = 10;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Kind {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
}
