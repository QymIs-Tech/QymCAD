//! THE VIEW CUBE — a navigation cube, as in grown-up CAD.
//!
//! What there was: 36 pixels, six flat faces, snapping ONLY to six normals and no highlight under the
//! cursor. That is, the commonest action — "set the isometric view" — could not be done with the cube at
//! all, and hitting a face had to be guessed blind.
//!
//! What there is now: **26 zones** — 6 faces, 12 edges, 8 corners. A click on a corner gives the
//! isometric view, on an edge a view at 45 degrees. It is for the sake of those zones that the cube is
//! drawn TRUNCATED (with chamfers along the edges and corners): a chamfer is not decoration but the very
//! zone that has to be hit. A flat cube could not show where to aim.
//!
//! The geometry is a truncated cube on 48 vertices: every sign and permutation of `(H, T, S)` with
//! `H > T > S`. The zones are derived from that rather than enumerated by hand:
//!
//! - a **face** is an octagon of the vertices whose chosen coordinate equals `H`;
//! - an **edge** is a quadrilateral of the vertices whose two coordinates equal `H` and `T`;
//! - a **corner** is a polygon of the vertices around a corner.
//!
//! That way the set of zones cannot diverge from the picture: THE SAME THING is drawn and picked.
use super::App;
use egui::{Color32, Pos2, Rect, Stroke};

/// The half-size of the cube, and the vertex of the chamfer along the middle axis and the small one.
/// `H > T > S`, otherwise there is no truncation.
const H: f64 = 1.0;
const T: f64 = 0.72;
const S: f64 = 0.46;

/// The kind of a zone — it decides both the shape and how large its caption should be.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ZoneKind {
    /// a face: an octagon, and it carries the caption of the view
    Face,
    /// an edge: a view at 45 degrees between two faces
    Edge,
    /// a corner: the isometric view
    Corner,
}

/// One zone of the cube: where to look on a click, and what it is drawn as.
pub(crate) struct Zone {
    /// the direction FROM the model TO the camera (the normal of the zone). The yaw and pitch are
    /// computed from it.
    pub dir: [f64; 3],
    pub kind: ZoneKind,
    /// the vertices of the polygon in the local coordinates of the cube
    pub poly: Vec<[f64; 3]>,
    /// the localisation key of the caption (faces only)
    pub label: Option<&'static str>,
}

/// The key of the caption for the normal of a face. The captions ARE TRANSLATABLE: a build in one
/// language with a cube in another would look unfinished, and the cube is the first thing seen in the
/// window.
fn face_label(d: [f64; 3]) -> Option<&'static str> {
    match (d[0] as i32, d[1] as i32, d[2] as i32) {
        (0, -1, 0) => Some("view-front"),
        (0, 1, 0) => Some("view-back"),
        (0, 0, 1) => Some("view-top"),
        (0, 0, -1) => Some("view-bottom"),
        (-1, 0, 0) => Some("view-left"),
        (1, 0, 0) => Some("view-right"),
        _ => None,
    }
}

/// ALL 26 ZONES. They are built from one geometry, so the pick and the drawing cannot diverge.
pub(crate) fn zones() -> Vec<Zone> {
    let mut out = Vec::with_capacity(26);
    // 6 FACES: an octagon in the plane where the absolute coordinate equals H
    for axis in 0..3 {
        for sign in [1.0_f64, -1.0] {
            let mut dir = [0.0; 3];
            dir[axis] = sign;
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            // eight vertices: (+-T, +-S) and (+-S, +-T) in the axes of the face — those are the
            // chamfered corners
            let mut poly = Vec::with_capacity(8);
            for (a, b) in [(T, S), (S, T), (-S, T), (-T, S), (-T, -S), (-S, -T), (S, -T), (T, -S)] {
                let mut p = [0.0; 3];
                p[axis] = sign * H;
                p[u] = a;
                p[v] = b;
                poly.push(p);
            }
            // at sign<0 the winding reverses — otherwise the polygon is inside out and the fill comes
            // out full of holes
            if sign < 0.0 {
                poly.reverse();
            }
            out.push(Zone { dir, kind: ZoneKind::Face, poly, label: face_label(dir) });
        }
    }
    // 12 EDGES: a strip between two faces. A click gives a view at 45 degrees — what fillets are looked
    // at with.
    for a in 0..3 {
        for b in (a + 1)..3 {
            for sa in [1.0_f64, -1.0] {
                for sb in [1.0_f64, -1.0] {
                    let c = 3 - a - b; // the third axis
                    let mut dir = [0.0; 3];
                    dir[a] = sa;
                    dir[b] = sb;
                    let mk = |ha: f64, hb: f64, hc: f64| {
                        let mut p = [0.0; 3];
                        p[a] = sa * ha;
                        p[b] = sb * hb;
                        p[c] = hc;
                        p
                    };
                    let poly = vec![mk(H, T, S), mk(T, H, S), mk(T, H, -S), mk(H, T, -S)];
                    out.push(Zone { dir: norm(dir), kind: ZoneKind::Edge, poly, label: None });
                }
            }
        }
    }
    // 8 CORNERS: A HEXAGON. A click gives THE ISOMETRIC VIEW — the commonest view, which the cube used to
    // lack entirely.
    //
    // Six vertices and not three. At a corner of a truncated cube THREE faces and THREE edges meet, that
    // is, all six permutations of (H,T,S) with these signs lie next to each other. A triangle through
    // three of them left three HOLES between it and the neighbouring edge strips — visible as gaps at the
    // corners.
    for sx in [1.0_f64, -1.0] {
        for sy in [1.0_f64, -1.0] {
            for sz in [1.0_f64, -1.0] {
                let dir = norm([sx, sy, sz]);
                let mut poly = vec![
                    [sx * H, sy * T, sz * S],
                    [sx * H, sy * S, sz * T],
                    [sx * T, sy * H, sz * S],
                    [sx * S, sy * H, sz * T],
                    [sx * T, sy * S, sz * H],
                    [sx * S, sy * T, sz * H],
                ];
                sort_around(&mut poly, dir);
                out.push(Zone { dir, kind: ZoneKind::Corner, poly, label: None });
            }
        }
    }
    out
}

/// Order the vertices AROUND the normal. A polygon given "as it happened" is drawn as a
/// self-intersecting star and picked wrongly; there has to be an order, and it must be derived rather
/// than written out by hand — with six vertices it is easier to get the order wrong than to notice it by
/// eye.
fn sort_around(poly: &mut [[f64; 3]], n: [f64; 3]) {
    let c = poly.iter().fold([0.0; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]]);
    let k = poly.len() as f64;
    let c = [c[0] / k, c[1] / k, c[2] / k];
    // a unit vector in the plane of the zone: the world axis least aligned with the normal is taken
    let seed = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let u = norm(cross(n, seed));
    let v = cross(n, u);
    poly.sort_by(|a, b| {
        let f = |p: &[f64; 3]| {
            let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
            (d[0] * v[0] + d[1] * v[1] + d[2] * v[2]).atan2(d[0] * u[0] + d[1] * u[1] + d[2] * u[2])
        };
        f(a).total_cmp(&f(b))
    });
}

fn norm(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
    [v[0] / l, v[1] / l, v[2] / l]
}

/// The direction of a zone becomes the angles of the camera. As a function of its own, because the same
/// conversion is used by the animation of the transition and by the tests: two copies of the formula
/// would diverge in a sign and give a cube that looks the wrong way.
pub(crate) fn dir_to_angles(d: [f64; 3]) -> (f64, f64) {
    let n = norm(d);
    (n[1].atan2(n[0]), n[2].clamp(-1.0, 1.0).asin())
}

impl App {
    /// THE SIZE OF THE CUBE in pixels — from the settings. On a 4K screen the former 36 px were
    /// unreadable in principle, while "make it bigger for everybody" would get in the way on a small
    /// screen: this is a choice for the person using it.
    pub(super) fn viewcube_size(&self) -> f32 {
        // The sizes were reduced by 30% after a check on a 2K screen: the middle one was too big and the
        // large one took up a noticeable part of the viewport. The cube is a pointer, not a piece of
        // composition.
        match self.set.viewcube_size {
            0 => 32.0,
            2 => 67.0,
            _ => 48.0,
        }
    }

    /// The centre of the cube on screen: the top right corner of the viewport with a margin.
    fn viewcube_center(&self, rect: Rect) -> Pos2 {
        let s = self.viewcube_size();
        Pos2::new(rect.right() - s - 18.0, rect.top() + s + 18.0)
    }

    /// A facade for the tests over the projection of a point of the cube.
    #[cfg(test)]
    pub(crate) fn viewcube_project_pub(&self, p: [f64; 3], rect: Rect) -> Pos2 {
        self.viewcube_project(p, rect).0
    }

    /// Project a point of the cube onto the screen, plus the depth (for sorting and for cutting away the
    /// back zones).
    fn viewcube_project(&self, p: [f64; 3], rect: Rect) -> (Pos2, f64) {
        let (right, up, fwd) = self.cam.basis();
        let c = self.viewcube_center(rect);
        let s = self.viewcube_size();
        let d = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        (Pos2::new(c.x + d(p, right) as f32 * s, c.y - d(p, up) as f32 * s), d(p, fwd))
    }

    /// Facades for the tests: the screen width of a zone and of the quadrilateral of its caption — they
    /// are what proves that the caption is deformed TOGETHER with the face rather than living a life of
    /// its own.
    #[cfg(test)]
    pub(crate) fn zone_screen_width_pub(&self, rect: Rect, i: usize) -> f32 {
        let pts: Vec<Pos2> = zones()[i].poly.iter().map(|p| self.viewcube_project(*p, rect).0).collect();
        let (mn, mx) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
        mx - mn
    }

    #[cfg(test)]
    pub(crate) fn label_quad_width_pub(&self, rect: Rect, i: usize) -> f32 {
        let z = &zones()[i];
        let (r, up) = label_frame(z.dir);
        let half = T * 0.80;
        let corner = |sr: f64, su: f64| {
            let mut p = [0.0; 3];
            for k in 0..3 {
                p[k] = z.dir[k] * H + r[k] * sr * half + up[k] * su * half;
            }
            self.viewcube_project(p, rect).0
        };
        let pts = [corner(1.0, 1.0), corner(-1.0, 1.0), corner(-1.0, -1.0), corner(1.0, -1.0)];
        let (mn, mx) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
        mx - mn
    }

    /// The screen directions of "right" and "up" for the caption on a face — a test uses them to prove
    /// that the text has not stood up vertically or turned upside down.
    #[cfg(test)]
    pub(crate) fn label_screen_dirs_pub(&self, rect: Rect, i: usize) -> (egui::Vec2, egui::Vec2) {
        let z = &zones()[i];
        let (r, up) = label_frame(z.dir);
        let o = self.viewcube_project([z.dir[0] * H, z.dir[1] * H, z.dir[2] * H], rect).0;
        let at = |d: [f64; 3]| {
            let p = [z.dir[0] * H + d[0] * 0.5, z.dir[1] * H + d[1] * 0.5, z.dir[2] * H + d[2] * 0.5];
            self.viewcube_project(p, rect).0 - o
        };
        (at(r), at(up))
    }

    /// THE ZONE UNDER THE CURSOR. Only the FRONT zones: the back side of the cube is not clickable —
    /// otherwise a click on the top would land in the invisible bottom behind it.
    pub(super) fn viewcube_zone_at(&self, rect: Rect, pos: Pos2) -> Option<usize> {
        let (_, _, fwd) = self.cam.basis();
        let zs = zones();
        let mut best: Option<(f64, usize)> = None;
        for (i, z) in zs.iter().enumerate() {
            // the zone looks AWAY from the camera, so it is on the far side
            if z.dir[0] * fwd[0] + z.dir[1] * fwd[1] + z.dir[2] * fwd[2] > -0.05 {
                continue;
            }
            let pts: Vec<Pos2> = z.poly.iter().map(|p| self.viewcube_project(*p, rect).0).collect();
            if !point_in_poly(pos, &pts) {
                continue;
            }
            let depth: f64 = z.poly.iter().map(|p| self.viewcube_project(*p, rect).1).sum::<f64>() / z.poly.len() as f64;
            if best.is_none_or(|(bd, _)| depth < bd) {
                best = Some((depth, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// A click on the cube: turn the view towards the zone. `false` means a miss (the click travels on).
    pub(super) fn viewcube_click(&mut self, rect: Rect, pos: Pos2) -> bool {
        // THE HOME BUTTON sits next to the cube, as in grown-up CAD: it returns the isometric view in
        // one press
        if self.viewcube_home_rect(rect).contains(pos) {
            self.animate_view_to(-0.7, 0.6);
            self.status = crate::i18n::tr("view-home-done");
            return true;
        }
        let Some(i) = self.viewcube_zone_at(rect, pos) else { return false };
        let z = &zones()[i];
        let (yaw, pitch) = dir_to_angles(z.dir);
        self.animate_view_to(yaw, pitch);
        if let Some(key) = z.label {
            self.status = crate::i18n::tr(key);
        }
        true
    }

    /// The rectangle of the home button — under the cube.
    pub(super) fn viewcube_home_rect(&self, rect: Rect) -> Rect {
        let c = self.viewcube_center(rect);
        let s = self.viewcube_size();
        Rect::from_center_size(Pos2::new(c.x, c.y + s + 14.0), egui::vec2(26.0, 18.0))
    }
}

/// A point inside a convex polygon (wound either way).
fn point_in_poly(p: Pos2, poly: &[Pos2]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut pos = false;
    let mut neg = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let cr = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if cr > 1e-4 {
            pos = true;
        } else if cr < -1e-4 {
            neg = true;
        }
        if pos && neg {
            return false;
        }
    }
    true
}

/// A CAPTION BAKED INTO A TEXTURE — so that it lies ON the face rather than living next to the cube.
///
/// Three approaches before it would not do, and each broke in its own way:
///
/// 1. Horizontal text in the centre of a face — the text and the faces live separately: the cube turns
///    and the caption stands still.
/// 2. Text rotated by the angle of the face (`TextShape::with_angle`) — it turns but is NOT distorted: at
///    a grazing angle the face is squeezed into a narrow strip while the letters keep their normal width.
/// 3. A visibility threshold — the caption simply vanished.
///
/// The right answer, and the one that was asked for: **bake it into a texture and stretch it over the
/// face**. Then the caption is distorted exactly as the face is, because it is a drawing on it.
///
/// The texture is prepared ONCE per caption and size and lives in a cache: rasterising a font every frame
/// is thousands of glyphs a second for nothing.
fn label_texture(ctx: &egui::Context, cache: &mut std::collections::HashMap<String, egui::TextureHandle>, text: &str, px: usize) -> egui::TextureHandle {
    let key = format!("{text}@{px}");
    if let Some(t) = cache.get(&key) {
        return t.clone();
    }
    use ab_glyph::{Font, ScaleFont};
    static BOLD: &[u8] = include_bytes!("../../../../assets/fonts/LiberationSans-Bold.ttf");
    let font = ab_glyph::FontRef::try_from_slice(BOLD).expect("the baked-in bold font parses");
    let scaled = font.as_scaled(px as f32);

    // THE WIDTH COMES FROM THE GLYPHS THEMSELVES rather than from the number of letters: captions differ
    // in length, and a texture of fixed width would stretch one and squeeze another.
    let glyphs: Vec<_> = text.chars().map(|c| font.glyph_id(c)).collect();
    let advance: f32 = glyphs.iter().map(|g| scaled.h_advance(*g)).sum();
    let pad = px as f32 * 0.25;
    let w = (advance + pad * 2.0).ceil().max(1.0) as usize;
    let h = (scaled.height() + pad).ceil().max(1.0) as usize;
    let mut alpha = vec![0u8; w * h];
    let mut pen = pad;
    let baseline = pad * 0.5 + scaled.ascent();
    for g in &glyphs {
        let q = g.with_scale_and_position(px as f32, ab_glyph::point(pen, baseline));
        if let Some(outline) = font.outline_glyph(q) {
            let bb = outline.px_bounds();
            outline.draw(|gx, gy, c| {
                let (x, y) = (bb.min.x as i32 + gx as i32, bb.min.y as i32 + gy as i32);
                if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                    let i = y as usize * w + x as usize;
                    // THE MAXIMUM is taken rather than the sum: neighbouring glyphs overlap, and adding
                    // gives dirty dark patches at the joins
                    alpha[i] = alpha[i].max((c * 255.0) as u8);
                }
            });
        }
        pen += scaled.h_advance(*g);
    }
    // THE COLOUR COMES FROM THE VERTICES OF THE MESH (multiplied by the texture), so only the alpha is
    // here: one texture serves a dark caption on a light face and the other way round alike.
    let pixels: Vec<Color32> = alpha.iter().map(|a| Color32::from_white_alpha(*a)).collect();
    let img = egui::ColorImage { size: [w, h], pixels };
    let tex = ctx.load_texture(&key, img, egui::TextureOptions::LINEAR);
    cache.insert(key, tex.clone());
    tex
}

/// WHERE "RIGHT" AND "UP" OF A CAPTION POINT ON EACH FACE — as a table, not as a formula.
///
/// The formula "take the next axes in a cycle" (`u = (a+1)%3`, `v = (a+2)%3`) looks tidy and lies on half
/// the faces: on +-Y it gives "right" = Z and the caption stands UP VERTICALLY, and on -X it turns the
/// text upside down. There are exactly six right answers here, and each is a convention about how a
/// person looks at that face rather than a consequence of the order of the axes.
///
/// The convention is the usual one in CAD: on every side face "up" is world +Z and "right" is the axis
/// that runs to the right when looking AT that face. The top and the bottom have no world "up", and there
/// "up" is +Y.
fn label_frame(d: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    match (d[0] as i32, d[1] as i32, d[2] as i32) {
        // the front (-Y): looking along +Y, right runs to +X
        (0, -1, 0) => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        // the back (+Y): looking along -Y, right runs to -X
        (0, 1, 0) => ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        // the right (+X): looking along -X, right runs to +Y
        (1, 0, 0) => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        // the left (-X): looking along +X, right runs to -Y
        (-1, 0, 0) => ([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
        // the top (+Z): there is no world "up", so +Y is taken, as the camera does at a pole
        (0, 0, 1) => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        // the bottom (-Z): looking FROM BELOW UPWARDS, world +X runs to the LEFT on screen — so "right"
        // for the caption is -X. The same mirror reversal as on any face looked at from its far side; the
        // top face has none, which is why the pair looks asymmetric, and yet it is right.
        _ => ([-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    }
}

impl App {
    /// A facade for the tests over the drawing: a test must run THE SAME code a real frame does.
    #[cfg(test)]
    pub(crate) fn draw_viewcube_pub(&self, painter: &egui::Painter, rect: Rect) {
        self.draw_viewcube(painter, rect);
    }

    /// DRAWING THE CUBE: the zones from the far ones to the near, the captions on the faces, the
    /// highlight under the cursor, the triad of axes and the home button.
    pub(super) fn draw_viewcube(&self, painter: &egui::Painter, rect: Rect) {
        let hover = painter.ctx().pointer_hover_pos().and_then(|p| self.viewcube_zone_at(rect, p));
        let (_, _, fwd) = self.cam.basis();
        let zs = zones();
        // sorting by depth: without it the front zones are painted over by the back ones and the cube
        // looks inside out
        let mut order: Vec<(usize, f64)> = zs
            .iter()
            .enumerate()
            .map(|(i, z)| {
                let d: f64 = z.poly.iter().map(|p| self.viewcube_project(*p, rect).1).sum::<f64>() / z.poly.len() as f64;
                (i, d)
            })
            .collect();
        order.sort_by(|a, b| b.1.total_cmp(&a.1));

        for (i, _) in order {
            let z = &zs[i];
            let toward = -(z.dir[0] * fwd[0] + z.dir[1] * fwd[1] + z.dir[2] * fwd[2]);
            if toward <= 0.02 {
                continue; // the back side is not drawn — nor is it clickable
            }
            let pts: Vec<Pos2> = z.poly.iter().map(|p| self.viewcube_project(*p, rect).0).collect();
            let base = match z.kind {
                ZoneKind::Face => 96.0,
                ZoneKind::Edge => 78.0,
                ZoneKind::Corner => 66.0,
            };
            // how far the face is turned towards the viewer, as a fraction: 0 is the furthest corner, 1
            // is the face head on. The depth of the shadow comes from the scheme: on a light background a
            // dark cube looks like a dirty patch.
            let t = ((base - 66.0) + toward * 70.0) / 169.0;
            let fill = if hover == Some(i) {
                self.scheme.pal.highlight()
            } else {
                crate::palette::tint(self.scheme.pal.viewcube_face(), crate::palette::lit(self.scheme.pal.shade_floor_viewcube, t as f32))
            };
            painter.add(egui::Shape::convex_polygon(pts.clone(), fill, Stroke::new(1.0, self.scheme.pal.viewcube_edge())));

            // THE CAPTION LIES ON THE FACE AS A DRAWING: the texture is stretched over a quadrilateral
            // IN THE PLANE of the face, so it turns and is distorted exactly with it. The text has no
            // separate life any more — it is part of the face.
            //
            // A SMALL CUBE HAS NO CAPTIONS: 32 px per face give letters of 6 px — mush, not text. And the
            // small size is chosen precisely so that the cube does not get in the way.
            if let (Some(key), true) = (z.label, self.set.viewcube_size > 0) {
                let text = crate::i18n::tr(key);
                let px = (self.viewcube_size() * 0.9).clamp(24.0, 96.0) as usize;
                let tex = {
                    let mut cache = self.cache.label_tex.borrow_mut();
                    label_texture(painter.ctx(), &mut cache, &text, px)
                };
                let [tw, th] = tex.size();
                // THE QUADRILATERAL FOR THE CAPTION — in the axes of the face, with the proportions of
                // the texture: otherwise a wide caption and a narrow one would stretch differently and
                // look like different fonts.
                let (r, up) = label_frame(z.dir);
                let half_w = T * 0.80;
                let half_h = half_w * (th as f64 / tw as f64);
                let corner = |sr: f64, su: f64| {
                    let mut p = [0.0; 3];
                    for k in 0..3 {
                        p[k] = z.dir[k] * H + r[k] * sr * half_w + up[k] * su * half_h;
                    }
                    self.viewcube_project(p, rect).0
                };
                // the winding: top left -> top right -> bottom right -> bottom left. Nothing needs
                // mirroring — the axes are already chosen for THIS face rather than derived from the order
                // of the coordinates.
                let quad = [corner(-1.0, 1.0), corner(1.0, 1.0), corner(1.0, -1.0), corner(-1.0, -1.0)];
                let uv = [egui::pos2(0.0, 0.0), egui::pos2(1.0, 0.0), egui::pos2(1.0, 1.0), egui::pos2(0.0, 1.0)];
                let ink = self.scheme.pal.plate_text();
                // THE VERTICES ARE PUSHED DIRECTLY: `colored_vertex` is for a mesh WITHOUT a texture,
                // it checks that with an assertion and brings the program down on the very first frame. A
                // textured vertex needs all three fields at once: the position, the uv and the colour.
                let mut mesh = egui::Mesh::with_texture(tex.id());
                for k in 0..4 {
                    mesh.vertices.push(egui::epaint::Vertex { pos: quad[k], uv: uv[k], color: ink });
                }
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                painter.add(egui::Shape::mesh(mesh));
            }
        }
        self.draw_axis_triad(painter, rect);
        self.draw_viewcube_home(painter, rect);
    }

    /// THE TRIAD OF AXES — arrows with heads and bold captions, as in grown-up CAD.
    ///
    /// The cube answers "where are we looking from", the triad answers "which axis is where". Those are
    /// different questions, and one widget does not answer both: a caption saying "front" tells nothing
    /// about which way X grows.
    ///
    /// The origin of the triad sits at the bottom left corner of the cube, as is customary: that way it
    /// reads as a continuation of the cube rather than a separate icon hanging beside it.
    fn draw_axis_triad(&self, painter: &egui::Painter, rect: Rect) {
        let (right, up, fwd) = self.cam.basis();
        let s = self.viewcube_size();
        let c = self.viewcube_center(rect);
        let origin = Pos2::new(c.x - s * 1.15, c.y + s * 0.95);
        let len = s * 0.95;
        let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let names = ["X", "Y", "Z"];
        // the far axes are drawn first — the near ones land on top
        let mut idx: Vec<usize> = (0..3).collect();
        idx.sort_by(|a, b| {
            let d = |i: usize| axes[i][0] * fwd[0] + axes[i][1] * fwd[1] + axes[i][2] * fwd[2];
            d(*b).total_cmp(&d(*a))
        });
        for i in idx {
            let a = axes[i];
            let d = |b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let (dx, dy) = (d(right) as f32, -(d(up) as f32));
            let l = (dx * dx + dy * dy).sqrt();
            // AN AXIS ALMOST ALONG THE VIEW degenerates into a point: drawing an "arrow" one pixel long
            // is pointless — only the caption at the origin is shown, so that the axis does not disappear
            // altogether.
            let away = d(fwd) > 0.0;
            let col = if away { self.scheme.pal.axis(i).gamma_multiply(0.5) } else { self.scheme.pal.axis(i) };
            if l < 0.12 {
                painter.circle_filled(origin, 3.0, col);
                continue;
            }
            let tip = Pos2::new(origin.x + dx * len, origin.y + dy * len);
            let w = if away { 1.8 } else { 3.0 };
            painter.line_segment([origin, tip], Stroke::new(w, col));
            // THE HEAD is a triangle at the tip: without it an axis cannot be told from a line of the
            // grid
            let (ux, uy) = (dx / l, dy / l);
            let (px, py) = (-uy, ux);
            let hl = (s * 0.22).max(6.0);
            let hw = hl * 0.42;
            let base = Pos2::new(tip.x - ux * hl, tip.y - uy * hl);
            painter.add(egui::Shape::convex_polygon(
                vec![tip, Pos2::new(base.x + px * hw, base.y + py * hw), Pos2::new(base.x - px * hw, base.y - py * hw)],
                col,
                Stroke::NONE,
            ));
            // THE CAPTION GOES BEYOND THE HEAD and is bold — a thin letter beside a bright arrow gets
            // lost
            let lp = Pos2::new(tip.x + ux * (hl * 0.75), tip.y + uy * (hl * 0.75));
            painter.text(lp, egui::Align2::CENTER_CENTER, names[i], super::bold((s * 0.30).clamp(11.0, 17.0)), col);
        }
        painter.circle_filled(origin, 3.2, self.scheme.pal.text_faint());
    }

    /// The home button: return the isometric view.
    fn draw_viewcube_home(&self, painter: &egui::Painter, rect: Rect) {
        let r = self.viewcube_home_rect(rect);
        let hot = painter.ctx().pointer_hover_pos().is_some_and(|p| r.contains(p));
        let bg = if hot { self.scheme.pal.highlight() } else { crate::palette::a(self.scheme.pal.viewcube_edge(), 220) };
        let fg = if hot { self.scheme.pal.plate_text() } else { self.scheme.pal.text_strong() };
        painter.rect_filled(r, 4.0, bg);
        painter.text(r.center(), egui::Align2::CENTER_CENTER, egui_phosphor::regular::HOUSE, egui::FontId::proportional(12.0), fg);
    }
}
