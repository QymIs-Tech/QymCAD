//! THE SCENE: the outlines of holes, drafts and blends, the bodies as the GPU takes them, their looks.

use super::*;

pub fn v_norm(a: [f64; 3]) -> [f64; 3] {
    let l = v_dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

pub fn v_cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn v_dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// THE OUTLINE OF A HOLE BEFORE ENTER, in the frame the rebuild drills it in (`pl`: its Z the outward normal of the
/// face, the hole going down it): the rims of the bore at the face and at the bottom with four lines between, and the
/// recess over it - a counterbore's step (kind 1) or a countersink's cone (kind 2) from `dia2` down `depth2`. It follows
/// the fields: a diameter of 6 draws rims 6 across, 12 draws them 12 across.
pub fn hole_outline(pl: &[f64; 12], tool: qymcad_core::model::HoleTool) -> Vec<Vec<[f64; 3]>> {
    let at = |r: f64, a: f64, z: f64| qymcad_core::feature::apply12(pl, [r * a.cos(), r * a.sin(), z]);
    let rim = |r: f64, z: f64| (0..=32).map(|i| at(r, std::f64::consts::TAU * i as f64 / 32.0, z)).collect::<Vec<_>>();
    let quarters = |r0: f64, z0: f64, r1: f64, z1: f64| (0..4).map(move |i| std::f64::consts::FRAC_PI_2 * i as f64).map(move |a| vec![at(r0, a, z0), at(r1, a, z1)]);
    let (r, depth) = (tool.diameter / 2.0, tool.depth.abs());
    let mut out = vec![rim(r, 0.0), rim(r, -depth)];
    out.extend(quarters(r, 0.0, r, -depth));
    if tool.kind != 0 && tool.dia2 > tool.diameter && tool.depth2 > 0.0 {
        let r2 = tool.dia2 / 2.0;
        out.push(rim(r2, 0.0));
        if tool.kind == 1 {
            out.push(rim(r2, -tool.depth2));
            out.extend(quarters(r2, 0.0, r2, -tool.depth2));
        } else {
            out.extend(quarters(r2, 0.0, r, -tool.depth2));
        }
    }
    out
}

/// THE DRAFTED FACE BEFORE ENTER: the border of the face as it will lean, each point moved across the pull by its
/// height over the neutral plane times tan(angle) - out below the neutral face, in above it, as the rebuild tilts it
/// (a side 10 high drafted 3 deg leans out 10 * tan 3 deg = 0.52 at the bottom). The border is the sides of the face's
/// triangles that no other triangle of the face shares. A face square to the pull (a cap) has nothing to lean and
/// draws nothing.
pub fn draft_outline(mesh: &qymcad_core::geom::Mesh, face: &qymcad_core::geom::MeshFace, origin: [f64; 3], pull: [f64; 3], angle_deg: f64) -> Vec<Vec<[f64; 3]>> {
    let pull = v_norm(pull);
    let across = v_sub(face.normal, [pull[0] * v_dot(face.normal, pull), pull[1] * v_dot(face.normal, pull), pull[2] * v_dot(face.normal, pull)]);
    if v_dot(across, across).sqrt() < 1e-6 {
        return Vec::new();
    }
    let across = v_norm(across);
    let tan = angle_deg.to_radians().tan();
    let mut sides: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
    for &t in &face.triangles {
        let tri = mesh.tris[t as usize];
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            *sides.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let lean = |i: u32| {
        let v = mesh.verts[i as usize];
        let p = [v.x, v.y, v.z];
        let h = v_dot(v_sub(p, origin), pull);
        let k = -h * tan;
        [p[0] + across[0] * k, p[1] + across[1] * k, p[2] + across[2] * k]
    };
    let mut out: Vec<Vec<[f64; 3]>> = sides.into_iter().filter(|(_, n)| *n == 1).map(|((a, b), _)| vec![lean(a), lean(b)]).collect();
    out.sort_by(|x, y| x[0].partial_cmp(&y[0]).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// A REGULAR POLYGON FROM ITS TWO CLICKS: (centre, a vertex), by the mode of the bar. The second click lies on the
/// polygon in every mode, as it does in the professional systems:
/// - 0 inscribed - the centre and a vertex;
/// - 1 circumscribed - the centre and the middle of an edge: the vertices stand r / cos(pi/n) out, turned by pi/n;
/// - 2 by edge - the two ends of an edge, the polygon to the left of the first -> second.
pub fn polygon_from_clicks(a: Point2, b: Point2, n: u32, mode: u8) -> (Point2, Point2) {
    let half = std::f64::consts::PI / n.max(3) as f64;
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
    match mode {
        1 => {
            let (r, ang) = (len / half.cos(), dy.atan2(dx) + half);
            (a, Point2::new(a.x + r * ang.cos(), a.y + r * ang.sin()))
        }
        2 => {
            let apothem = len / (2.0 * half.tan());
            let c = Point2::new((a.x + b.x) / 2.0 - dy / len * apothem, (a.y + b.y) / 2.0 + dx / len * apothem);
            (c, b)
        }
        _ => (a, b),
    }
}

/// What is made along an edge: a fillet of a radius, or a chamfer of a leg on either face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Blend {
    Round(f64),
    /// A chamfer: the first leg, the second, and the normal of the reference face the first leg lies on, when one is
    /// named - the kernel lays the first leg there; with none named the legs take the faces in the order met.
    Cut(f64, f64, Option<[f64; 3]>),
}

/// The point of triangle `t` nearest to `p` (by the regions of the triangle's plane).
pub fn closest_on_triangle(p: [f64; 3], t: &[[f64; 3]; 3]) -> [f64; 3] {
    let [a, b, c] = *t;
    let (ab, ac, ap) = (v_sub(b, a), v_sub(c, a), v_sub(p, a));
    let at = |u: f64, v: f64| [a[0] + ab[0] * u + ac[0] * v, a[1] + ab[1] * u + ac[1] * v, a[2] + ab[2] * u + ac[2] * v];
    let (d1, d2) = (v_dot(ab, ap), v_dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = v_sub(p, b);
    let (d3, d4) = (v_dot(ab, bp), v_dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return at(d1 / (d1 - d3), 0.0);
    }
    let cp = v_sub(p, c);
    let (d5, d6) = (v_dot(ab, cp), v_dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return at(0.0, d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return [b[0] + (c[0] - b[0]) * w, b[1] + (c[1] - b[1]) * w, b[2] + (c[2] - b[2]) * w];
    }
    let denom = 1.0 / (va + vb + vc);
    at(vb * denom, vc * denom)
}

/// A side of a mesh triangle, from `a` to `b`, with the triangle's normal and its third corner `far`.
pub(crate) struct TriSide {
    pub(crate) a: [f64; 3],
    pub(crate) b: [f64; 3],
    pub(crate) normal: [f64; 3],
    pub(crate) far: [f64; 3],
}

/// A face met along a piece of an edge: its normal, and the unit direction it runs on in, away from the edge.
#[derive(Clone, Copy)]
pub(crate) struct FaceRun {
    pub(crate) normal: [f64; 3],
    pub(crate) away: [f64; 3],
}

/// The two faces either side of a piece of an edge.
#[derive(Clone, Copy)]
pub(crate) struct FacePair {
    pub(crate) one: FaceRun,
    pub(crate) other: FaceRun,
}

/// At a vertex of the edge: where the blend meets each face, and the centre of a fillet's arc.
#[derive(Clone, Copy)]
pub(crate) struct Rail {
    pub(crate) q1: [f64; 3],
    pub(crate) q2: [f64; 3],
    pub(crate) centre: [f64; 3],
}

/// THE OUTLINE OF A BLEND ALONG AN EDGE, before Enter: the two lines where a fillet (or a chamfer) meets the faces
/// either side of the edge, then its section at both ends - an arc for a fillet, a straight cut for a chamfer. It
/// follows the value typed: a radius of 2 on a square edge sets the lines 2 back from it, a radius of 4 sets them 4.
///
/// The faces are told by the triangles of the body's mesh with a side along a piece of the edge: the edge's polyline
/// is cut finer than the mesh (24 pieces against one side of a block's face), so the pieces are matched to the sides
/// by where they lie, not by their vertices. A piece with only one
/// face found, or two faces within 2.5 deg of each other, has nothing to set back from and draws nothing. The
/// setback of a fillet is r * tan(bend / 2): r on a square edge, less on an obtuse one.
pub fn edge_blend_outline(mesh: &qymcad_core::geom::Mesh, poly: &[[f32; 3]], blend: Blend) -> Vec<Vec<[f64; 3]>> {
    let n = poly.len();
    if n < 2 {
        return Vec::new();
    }
    let pt = |p: [f32; 3]| [p[0] as f64, p[1] as f64, p[2] as f64];
    let add = |p: [f64; 3], d: [f64; 3], k: f64| [p[0] + d[0] * k, p[1] + d[1] * k, p[2] + d[2] * k];
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in poly {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i] as f64 - 0.01);
            hi[i] = hi[i].max(p[i] as f64 + 0.01);
        }
    }
    // the triangles within the reach of the blend around the edge: a point set back along a curved face is laid back on
    // it - along the tangent it leaves a cylinder of R5 by 2.1 mm at a setback of 8.6
    let reach = match blend {
        Blend::Round(r) => r * 2.0,
        Blend::Cut(a, b, _) => a.max(b) * 2.0,
    } + 0.1;
    let near: Vec<[[f64; 3]; 3]> = (0..mesh.tris.len())
        .map(|t| mesh.triangle(t))
        .map(|[a, b, c]| [[a.x, a.y, a.z], [b.x, b.y, b.z], [c.x, c.y, c.z]])
        .filter(|v| !(0..3).any(|i| v.iter().all(|p| p[i] < lo[i] - reach) || v.iter().all(|p| p[i] > hi[i] + reach)))
        .collect();
    // only a curved face lays the point back: where the nearest triangle turns away from the face at the edge (by more
    // than 0.5 deg); a flat face keeps the point on its plane, even past the triangles a mesh happens to have there
    let on_surface = |q: [f64; 3], n: [f64; 3]| -> [f64; 3] {
        let best = near.iter().map(|t| (closest_on_triangle(q, t), t)).min_by(|a, b| v_dot(v_sub(a.0, q), v_sub(a.0, q)).total_cmp(&v_dot(v_sub(b.0, q), v_sub(b.0, q))));
        match best {
            Some((c, t)) if v_dot(v_norm(v_cross(v_sub(t[1], t[0]), v_sub(t[2], t[0]))), n).abs() < 0.99996 => c,
            _ => q,
        }
    };
    // the sides of the triangles that touch the edge's box, with the triangle's normal and its third corner: few,
    // whatever the size of the mesh, and each piece of the edge is looked up among them only. The third corner says
    // which way the face runs on from the edge - the one thing the two normals cannot tell an inner corner from an
    // outer one by
    let mut sides: Vec<TriSide> = Vec::new();
    for t in 0..mesh.tris.len() {
        let [a, b, c] = mesh.triangle(t);
        let v = [[a.x, a.y, a.z], [b.x, b.y, b.z], [c.x, c.y, c.z]];
        if (0..3).any(|i| v.iter().all(|p| p[i] < lo[i]) || v.iter().all(|p| p[i] > hi[i])) {
            continue;
        }
        let nrm = v_norm(v_cross(v_sub(v[1], v[0]), v_sub(v[2], v[0])));
        for k in 0..3 {
            sides.push(TriSide { a: v[k], b: v[(k + 1) % 3], normal: nrm, far: v[(k + 2) % 3] });
        }
    }
    // per piece of the edge: the faces met, each as its normal and the way it runs on from the edge, across it
    let mut normals: Vec<Vec<FaceRun>> = vec![Vec::new(); n - 1];
    for (seg, ns) in normals.iter_mut().enumerate() {
        let (p, q) = (pt(poly[seg]), pt(poly[seg + 1]));
        let (m, d) = ([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0, (p[2] + q[2]) / 2.0], v_norm(v_sub(q, p)));
        for TriSide { a, b, normal: nrm, far } in &sides {
            let ab = v_sub(*b, *a);
            let len = v_dot(ab, ab).sqrt();
            if len < 1e-9 || v_dot(ab, d).abs() < 0.98 * len {
                continue;
            }
            // the middle of the piece lies on the side, to 0.001 mm plus 2 % of the side for a curved edge's chord
            let t = (v_dot(v_sub(m, *a), ab) / (len * len)).clamp(0.0, 1.0);
            let off = v_sub(m, [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t]);
            if v_dot(off, off).sqrt() < 1e-3 + 0.02 * len {
                let to = v_sub(*far, m);
                ns.push(FaceRun { normal: *nrm, away: v_norm(v_sub(to, add([0.0; 3], d, v_dot(to, d)))) });
            }
        }
    }
    // the two faces of each piece, kept in step along the edge so that a line does not jump from one face to the other
    let mut faces: Vec<Option<FacePair>> = Vec::with_capacity(n - 1);
    let mut prev: Option<FacePair> = None;
    for ns in &normals {
        let pair = ns.first().and_then(|n1| ns.iter().find(|n2| v_dot(n1.normal, n2.normal) < 0.999).map(|n2| FacePair { one: *n1, other: *n2 }));
        let pair = match (pair, prev) {
            (Some(p), Some(q)) if v_dot(p.one.normal, q.one.normal) < v_dot(p.one.normal, q.other.normal) => Some(FacePair { one: p.other, other: p.one }),
            (p, _) => p,
        };
        if pair.is_some() {
            prev = pair;
        }
        faces.push(pair);
    }
    // per vertex of the edge: the two points on the faces and, for a fillet, the centre of its arc
    let mut rails: Vec<Rail> = Vec::with_capacity(n);
    for (k, p) in poly.iter().enumerate() {
        let Some(FacePair { one: FaceRun { normal: n1, away: t1 }, other: FaceRun { normal: n2, away: t2 } }) = faces[k.min(n - 2)].or_else(|| faces[k.saturating_sub(1)]) else { continue };
        let p = pt(*p);
        // t1, t2: in each face, away from the edge. An outer edge has the faces run on away from each other's normal,
        // an inner one towards it; the fillet's centre is inside the part for the first and in the air for the second
        let inner = v_dot(t1, n2) > 0.0;
        let (q1, q2, c) = match blend {
            Blend::Round(r) => {
                let bend = v_dot(n1, n2).clamp(-1.0, 1.0).acos();
                let back = r * (bend / 2.0).tan();
                let q1 = add(p, t1, back);
                (q1, add(p, t2, back), add(q1, n1, if inner { r } else { -r }))
            }
            Blend::Cut(a, b, reference) => {
                // the two faces of an edge are never parallel, so the reference is the one its normal lies along
                let swap = reference.is_some_and(|r| v_dot(n2, r).abs() > v_dot(n1, r).abs());
                let (a, b) = if swap { (b, a) } else { (a, b) };
                (add(p, t1, a), add(p, t2, b), p)
            }
        };
        let (q1, q2) = (on_surface(q1, n1), on_surface(q2, n2));
        rails.push(Rail { q1, q2, centre: c });
    }
    if rails.len() < 2 {
        return Vec::new();
    }
    let section = |Rail { q1, q2, centre: c }: Rail| -> Vec<[f64; 3]> {
        match blend {
            Blend::Round(r) => (0..=8)
                .map(|i| {
                    let t = i as f64 / 8.0;
                    let (u, w) = (v_sub(q1, c), v_sub(q2, c));
                    add(c, v_norm([u[0] + (w[0] - u[0]) * t, u[1] + (w[1] - u[1]) * t, u[2] + (w[2] - u[2]) * t]), r)
                })
                .collect(),
            Blend::Cut(..) => vec![q1, q2],
        }
    };
    vec![rails.iter().map(|r| r.q1).collect(), rails.iter().map(|r| r.q2).collect(), section(rails[0]), section(rails[rails.len() - 1])]
}

/// A vertex of a body for the GPU. The colour and the normal do NOT depend on the camera (the light is
/// of the world), so the buffer is re-uploaded only when the scene changes and not when it is rotated.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVert {
    /// The position in world coordinates (with the transform of the owning component already applied).
    pub pos: [f32; 3],
    /// WHOSE VERTEX THIS IS - the number of the body in the scene's own list, not its colour.
    ///
    /// The colour used to be baked in here, and with it the highlight and the ghosting. That made the
    /// appearance part of the geometry: hovering over a part, or stepping into a subassembly, changed the
    /// vertices themselves - all 23 million of them on the reference engine - and the whole scene went to the
    /// card again. Reported as "cannot enter the assembly, it hangs".
    ///
    /// Now the vertex says only WHOSE it is, and what that body looks like right now lives in a small table
    /// beside it (`BodyLook`), which is rewritten in full on every frame and costs bytes rather than megabytes.
    pub body: u32,
    /// THE NORMAL, PACKED INTO FOUR BYTES - one signed byte per axis, which is 1/127 of accuracy and more
    /// than shading needs.
    ///
    /// It is here for SMOOTH shading only: there the normal belongs to the vertex, not to the triangle, and
    /// no derivative can recover it. Flat shading leaves it zero and the fragment takes the normal from the
    /// derivatives of the world position, as before.
    ///
    /// The size is an admitted trade: 20 bytes against the 16 a flat-only vertex would need. Splitting the
    /// two into separate formats and pipelines would bring those four bytes back; it is written down in the
    /// plan rather than done here, because the freeze on entering an assembly is worth more than 20 % of the
    /// scene's size.
    pub nrm: u32,
}

/// THE LIGHTS OF THE 3D VIEW: a key light and a weaker fill, both carried with the camera.
///
/// One light taken by its absolute value lit a face and the face opposite it alike, and every face at one angle to
/// it alike: on a block chamfered round the top the right chamfer came out 2 levels from the top it leads into.
/// Reported behaviour: "add a second light, so the models and their structure look more solid". The key comes from
/// above and to the left of the eye, the fill from below and to the right at 0.37 of its strength, each lights only the
/// side turned to it, and the two together give exactly 1 to a face looking straight at the eye. Searched over the
/// faces and 45 deg chamfers of a block seen from the front, from behind and from below: two faces that share an edge
/// stay at least 12 levels apart, against 2 with the one light, and the top of a block in the usual view takes 0.70,
/// its front 0.56 and its side 0.48 - lit from above, as a person expects a part on a table to be.
///
/// FIXED TO THE CAMERA, as a CAD lights a part: the side a person turns to themselves is the lit side, whichever it
/// is. Fixed to the world, a part turned over showed its underside in the fill alone.
#[derive(Clone, Copy, Debug)]
pub struct SceneLights {
    /// Where the key light comes from: a unit vector in the world, pointing at the light.
    pub key: [f64; 3],
    /// Where the fill comes from, the same way.
    pub fill: [f64; 3],
    /// What each light gives a face turned straight to it.
    pub key_share: f64,
    pub fill_share: f64,
}

/// The two lights in the frame of the camera: x to the right, y up, z towards the eye.
const KEY_FROM_EYE: [f64; 3] = [-0.21, 0.38, 0.9];
const FILL_FROM_EYE: [f64; 3] = [0.17, -0.47, 0.87];
/// The fill's strength against the key's.
const FILL_TO_KEY: f64 = 0.37;

/// The lights for a camera whose right, up and forward are `basis`.
pub fn scene_lights(basis: &([f64; 3], [f64; 3], [f64; 3])) -> SceneLights {
    let (right, up, fwd) = basis;
    let place = |v: [f64; 3]| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        std::array::from_fn(|a| (right[a] * v[0] + up[a] * v[1] - fwd[a] * v[2]) / l)
    };
    let (key, fill) = (place(KEY_FROM_EYE), place(FILL_FROM_EYE));
    // a face looking straight at the eye gets exactly 1: the key's part and the fill's part of it add up to that
    let head_on = -v_dot(key, *fwd) + FILL_TO_KEY * -v_dot(fill, *fwd);
    SceneLights { key, fill, key_share: 1.0 / head_on, fill_share: FILL_TO_KEY / head_on }
}

impl SceneLights {
    /// How much light a face with the unit normal `n` turned to the eye takes, 0..1: each light lights only the side
    /// turned to it.
    pub fn on(&self, n: [f64; 3]) -> f64 {
        (self.key_share * v_dot(n, self.key).max(0.0) + self.fill_share * v_dot(n, self.fill).max(0.0)).clamp(0.0, 1.0)
    }
}

/// ONE PIECE OF THE SCENE handed to the card: the vertices of one body (or of the section caps) and the row
/// of the look table they belong to.
pub struct ScenePiece {
    pub verts: std::sync::Arc<Vec<GpuVert>>,
    /// The triangles as indices into `verts`.
    pub idx: std::sync::Arc<Vec<u32>>,
    pub body: u32,
    /// How many rows of the look table from `body` on the vertices name: the body's own, and one per colour of its faces.
    pub rows: u32,
}

/// THE WHOLE SCENE as it goes to the card: the pieces in a stable order, and the table saying what each body
/// looks like at this moment.
///
/// The two travel apart on purpose. The pieces change when a body changes SHAPE or place; the table changes
/// when a body is highlighted, ghosted or recoloured - which is every time the pointer moves over the model
/// or a person steps into a subassembly. Keeping them together meant re-uploading the geometry for a change
/// of colour: 739 MB on the reference engine, and a frozen window.
pub struct GpuScene {
    pub pieces: Vec<ScenePiece>,
    pub looks: Vec<BodyLook>,
}

/// HOW A BODY LOOKS RIGHT NOW: its own colour, already brightened, and the state it is in.
///
/// One record per body of the scene, handed to the card as a table. Everything that depends on the light -
/// and that is the whole of the shading - is computed in the fragment, because the light depends on the
/// normal and the normal on the triangle.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BodyLook {
    /// The body's own colour after `brighten`, as rgb in the low three bytes.
    pub tint: u32,
    /// THE FLAGS OF THE LOOK, one bit each: `LOOK_HOT`, `LOOK_GHOST`, `LOOK_CAP`.
    ///
    /// Bits rather than a number of three, and that is not taste. Selection and ghostliness are two
    /// different answers: the first decides the COLOUR, the second decides WHICH PASS the triangle is drawn
    /// in - the blended one, with no z-write. A selected neighbour's body is both at once: painted as
    /// selected, and still drawn in the blended pass. Written as one number of three, that case collapsed
    /// into "selected" and a ghost of a neighbouring part jumped into the opaque pass the moment it was
    /// picked.
    pub state: u32,
}

/// The body is selected: it is painted as selected, whatever else it is.
pub const LOOK_HOT: u32 = 1;
/// The body belongs to a neighbouring context: it is drawn in the blended pass, with no z-write.
pub const LOOK_GHOST: u32 = 2;
/// Not a surface of a part but the cap of a section: a fill, and it is not shaded.
pub const LOOK_CAP: u32 = 4;
