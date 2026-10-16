//! THE FRAMES OF PLANES: a base plane, a face key, a sketch plane, a frame and its tilt.

use super::*;

/// Orthonormal frame of a plane: an origin plus the X and Y axes in world space (the normal is X cross Y).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneFrame {
    pub origin: [f64; 3],
    pub x: [f64; 3],
    pub y: [f64; 3],
}

impl PlaneFrame {
    /// Lift a 2D sketch point into world coordinates.
    pub fn lift(&self, p: Point2) -> Point3 {
        Point3::new(self.origin[0] + self.x[0] * p.x + self.y[0] * p.y, self.origin[1] + self.x[1] * p.x + self.y[1] * p.y, self.origin[2] + self.x[2] * p.x + self.y[2] * p.y)
    }

    /// Project a world point into the local 2D coordinates of the frame, along its X and Y axes: the inverse
    /// of `lift`.
    ///
    /// Used to draw a body inside the 2D sketch editor on any plane (a face or a datum), so the body lines up
    /// with the sketch coordinates instead of being shown from above.
    pub fn project(&self, p: Point3) -> Point2 {
        let d = [p.x - self.origin[0], p.y - self.origin[1], p.z - self.origin[2]];
        Point2::new(d[0] * self.x[0] + d[1] * self.x[1] + d[2] * self.x[2], d[0] * self.y[0] + d[1] * self.y[1] + d[2] * self.y[2])
    }

    /// Depth of a world point along the frame normal, used for ordering and shading in the 2D projection.
    pub fn depth(&self, p: Point3) -> f64 {
        let n = self.normal();
        (p.x - self.origin[0]) * n[0] + (p.y - self.origin[1]) * n[1] + (p.z - self.origin[2]) * n[2]
    }

    /// Normal of the plane (X cross Y).
    pub fn normal(&self) -> [f64; 3] {
        [self.x[1] * self.y[2] - self.x[2] * self.y[1], self.x[2] * self.y[0] - self.x[0] * self.y[2], self.x[0] * self.y[1] - self.x[1] * self.y[0]]
    }

    /// Build an orthonormal frame from an origin, a normal and a rotation of X about it, in degrees.
    ///
    /// The reference up direction is world +Z (or +X when the normal is nearly parallel to Z), so that X lies
    /// in the plane.
    pub fn from_origin_normal(origin: [f64; 3], normal: [f64; 3], rot_deg: f64) -> PlaneFrame {
        let n = norm3(normal).unwrap_or([0.0, 0.0, 1.0]);
        // A normal parallel to Z gives axes equal to world X and Y, the natural frame for an XY datum;
        // otherwise X = Z cross n (which lies in the plane) and Y = n cross X.
        let x0 = if n[2].abs() > 0.999 { [1.0, 0.0, 0.0] } else { norm3(cross3([0.0, 0.0, 1.0], n)).unwrap_or([1.0, 0.0, 0.0]) };
        let y0 = cross3(n, x0);
        // Rotate the X and Y axes about the normal by `rot_deg`.
        let (s, c) = rot_deg.to_radians().sin_cos();
        let x = [x0[0] * c + y0[0] * s, x0[1] * c + y0[1] * s, x0[2] * c + y0[2] * s];
        let y = [y0[0] * c - x0[0] * s, y0[1] * c - x0[1] * s, y0[2] * c - x0[2] * s];
        PlaneFrame { origin, x, y }
    }

    /// Frame from an origin, a main axis (the normal) and an explicit secondary axis.
    ///
    /// The difference from `from_origin_normal` is fundamental: there the secondary axis is derived from
    /// world Z, so it depends on how the body happens to be rotated in the world and comes out differently
    /// for two bodies being mated. Here the caller supplies it, from the geometry of the body itself.
    ///
    /// `x_hint` is orthogonalised against the normal; if it is parallel to it or degenerate, the behaviour
    /// falls back to `from_origin_normal` — a predictable fallback beats a division by zero.
    pub fn from_origin_axes(origin: [f64; 3], normal: [f64; 3], x_hint: [f64; 3]) -> Self {
        let n = norm3(normal).unwrap_or([0.0, 0.0, 1.0]);
        let d = x_hint[0] * n[0] + x_hint[1] * n[1] + x_hint[2] * n[2];
        let proj = [x_hint[0] - n[0] * d, x_hint[1] - n[1] * d, x_hint[2] - n[2] * d];
        match norm3(proj) {
            Some(x) => PlaneFrame { origin, x, y: cross3(n, x) },
            None => Self::from_origin_normal(origin, normal, 0.0),
        }
    }

    /// Frame of a plane whose origin is the projection of the world origin onto that plane, rather than an
    /// arbitrary point of a face or a datum. For axis-aligned planes the local XY axes then coincide with the
    /// world ones, so the 2D sketch and the 3D view agree.
    ///
    /// `point` is any point of the plane (a face centre, a datum origin) and `normal` is its normal.
    pub fn world_aligned(point: [f64; 3], normal: [f64; 3], rot_deg: f64) -> PlaneFrame {
        let n = norm3(normal).unwrap_or([0.0, 0.0, 1.0]);
        // Foot of the perpendicular from the origin onto the plane {x : (x - point) . n = 0}.
        let d = point[0] * n[0] + point[1] * n[1] + point[2] * n[2];
        let origin = [d * n[0], d * n[1], d * n[2]];
        // In-plane axes. For an axis-aligned normal the two positive world axes serve as X and Y, ordered so
        // that X cross Y = n and the triple stays right-handed, which keeps the extrude direction correct.
        // This removes the mirrored snapping between opposing faces (+X against -X and so on): every face has
        // its origin in the same corner and the body grows towards +X and +Y. A tilted face keeps X = Z cross
        // n.
        let dom = if n[0].abs() > 0.999 {
            Some(0)
        } else if n[1].abs() > 0.999 {
            Some(1)
        } else if n[2].abs() > 0.999 {
            Some(2)
        } else {
            None
        };
        let (x0, y0) = match dom {
            Some(k) => {
                let e = |i: usize| {
                    let mut v = [0.0; 3];
                    v[i] = 1.0;
                    v
                };
                let (i, j) = match k {
                    0 => (1, 2),
                    1 => (0, 2),
                    _ => (0, 1),
                };
                let (u, v) = (e(i), e(j));
                // u cross v = +e_k: co-directed with n gives (u,v), otherwise they are swapped to keep
                // X cross Y = n.
                if cross3(u, v)[k] * n[k] > 0.0 {
                    (u, v)
                } else {
                    (v, u)
                }
            }
            None => {
                let x = norm3(cross3([0.0, 0.0, 1.0], n)).unwrap_or([1.0, 0.0, 0.0]);
                (x, cross3(n, x))
            }
        };
        // Rotate the X and Y axes about the normal by `rot_deg`.
        let (s, c) = rot_deg.to_radians().sin_cos();
        let x = [x0[0] * c + y0[0] * s, x0[1] * c + y0[1] * s, x0[2] * c + y0[2] * s];
        let y = [y0[0] * c - x0[0] * s, y0[1] * c - x0[1] * s, y0[2] * c - x0[2] * s];
        PlaneFrame { origin, x, y }
    }

    /// Lift a local 3D point (in frame axes) into the world: origin + x*lx + y*ly + n*lz.
    pub fn lift3(&self, p: Point3) -> Point3 {
        let n = self.normal();
        Point3::new(
            self.origin[0] + self.x[0] * p.x + self.y[0] * p.y + n[0] * p.z,
            self.origin[1] + self.x[1] * p.x + self.y[1] * p.y + n[1] * p.z,
            self.origin[2] + self.x[2] * p.x + self.y[2] * p.y + n[2] * p.z,
        )
    }

    /// Rotate a local direction (a face normal, for example) into the world, without translation.
    pub fn rotate_dir(&self, d: [f64; 3]) -> [f64; 3] {
        let n = self.normal();
        [self.x[0] * d[0] + self.y[0] * d[1] + n[0] * d[2], self.x[1] * d[0] + self.y[1] * d[1] + n[1] * d[2], self.x[2] * d[0] + self.y[2] * d[1] + n[2] * d[2]]
    }

    /// Placement as a 3x4 row-major matrix (the X, Y and N axes as columns plus the origin), for a kernel
    /// transform.
    pub fn matrix12(&self) -> [f64; 12] {
        let n = self.normal();
        [self.x[0], self.y[0], n[0], self.origin[0], self.x[1], self.y[1], n[1], self.origin[1], self.x[2], self.y[2], n[2], self.origin[2]]
    }

    /// Whether the frame is the identity (world XY), in which case a body needs no transform.
    pub fn is_identity(&self) -> bool {
        self.origin == [0.0; 3] && self.x == [1.0, 0.0, 0.0] && self.y == [0.0, 1.0, 0.0]
    }

    /// Carry the frame through a 3x4 transform: the origin as a point and the axes as directions. This is how
    /// a local component frame is taken into world space (`world_transform`).
    pub fn transformed(&self, m: &[f64; 12]) -> PlaneFrame {
        PlaneFrame { origin: apply12(m, self.origin), x: apply12_dir(m, self.x), y: apply12_dir(m, self.y) }
    }

    /// Move a body (mesh plus faces) built in the local axes of the frame into world space.
    pub fn place_body(&self, mut mesh: Mesh, mut faces: Vec<MeshFace>) -> Built {
        if self.is_identity() {
            return Built { mesh, faces };
        }
        for v in &mut mesh.verts {
            *v = self.lift3(*v);
        }
        for n in &mut mesh.norms {
            if let Some(d) = crate::geom::unpack_direction(*n) {
                *n = crate::geom::pack_direction(self.rotate_dir(d));
            }
        }
        for f in &mut faces {
            f.centroid = self.lift3(f.centroid);
            f.normal = self.rotate_dir(f.normal);
        }
        Built { mesh, faces }
    }
}

/// THE TILT OF A DATUM PLANE, in degrees: about the X and about the Y axis of the plane before the tilt, through the
/// point the plane stands at. The X and Y are the ones a sketch on the plane before the tilt takes
/// (`PlaneFrame::world_aligned`); the turn about the normal is the plane's own `rot_deg`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlaneTilt {
    pub about_x: f64,
    pub about_y: f64,
}

impl PlaneTilt {
    /// The axes the tilt turns about, of a plane whose normal before the tilt is `normal`: the frame's X and Y.
    pub fn axes(normal: [f64; 3]) -> PlaneFrame {
        PlaneFrame::world_aligned([0.0; 3], normal, 0.0)
    }

    /// The normal `normal` tilted: about the plane's X first, then about its Y, both axes of the plane before the tilt.
    pub fn apply(self, normal: [f64; 3]) -> [f64; 3] {
        let f = Self::axes(normal);
        let n = norm3(normal).unwrap_or([0.0, 0.0, 1.0]);
        turn_about(turn_about(n, f.x, self.about_x), f.y, self.about_y)
    }
}

/// `v` turned about the unit axis `k` by `deg` degrees, counter-clockwise looking down `k` (Rodrigues).
pub fn turn_about(v: [f64; 3], k: [f64; 3], deg: f64) -> [f64; 3] {
    let (s, c) = deg.to_radians().sin_cos();
    let kxv = cross3(k, v);
    let kv = k[0] * v[0] + k[1] * v[1] + k[2] * v[2];
    [v[0] * c + kxv[0] * s + k[0] * kv * (1.0 - c), v[1] * c + kxv[1] * s + k[1] * kv * (1.0 - c), v[2] * c + kxv[2] * s + k[2] * kv * (1.0 - c)]
}

pub(crate) fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub(crate) fn norm3(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-12 {
        None
    } else {
        Some([v[0] / l, v[1] / l, v[2] / l])
    }
}

impl BasePlane {
    /// Frame of a base plane at the origin.
    ///
    /// XY: X = +x, Y = +y (normal +z). XZ: X = +x, Y = +z (normal -y). YZ: X = +y, Y = +z (normal +x).
    pub fn frame(self) -> PlaneFrame {
        match self {
            BasePlane::XY => PlaneFrame { origin: [0.0; 3], x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0] },
            BasePlane::XZ => PlaneFrame { origin: [0.0; 3], x: [1.0, 0.0, 0.0], y: [0.0, 0.0, 1.0] },
            BasePlane::YZ => PlaneFrame { origin: [0.0; 3], x: [0.0, 1.0, 0.0], y: [0.0, 0.0, 1.0] },
        }
    }
    /// THE SIDE A PLANE OFFSET FROM THIS ONE GOES TO: the positive axis across it - +Z from XY, +Y from XZ, +X from YZ.
    /// Not the normal of `frame`, which for XZ is X x Z = -Y: offset along it, a plane 100 from XZ was previewed at
    /// +100 and made at -100. The preview and the plane made both take it from here.
    pub fn offset_direction(self) -> [f64; 3] {
        match self {
            BasePlane::XY => [0.0, 0.0, 1.0],
            BasePlane::XZ => [0.0, 1.0, 0.0],
            BasePlane::YZ => [1.0, 0.0, 0.0],
        }
    }
}

impl SketchPlane {
    /// Frame without project context, available for `World` only. `Datum` and `Face` need a `Project` to
    /// resolve the plane or the face.
    pub fn world_frame(&self) -> Option<PlaneFrame> {
        match self {
            SketchPlane::World(b) => Some(b.frame()),
            _ => None,
        }
    }
}
