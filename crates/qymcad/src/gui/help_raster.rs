//! A RASTERISER OF AN `egui` FRAME — so that not only bodies can be captured but THE PLANE as well: a
//! sketch with its constraints, dimensions and the colour of definedness.
//!
//! Pictures of bodies come from `rasterize_3d`, and that was enough for the Part section. It is of no
//! help to a sketch: what is drawn there is not a body but lines, arcs, constraint glyphs and
//! dimension captions — all of that lives in `egui` rather than in the geometry. And the Sketch
//! section is a newcomer's first hour, the most expensive thing of all to explain in words.
//!
//! WHAT HAPPENS HERE. `Context::tessellate` already turns the shapes of a frame into triangles with
//! colour in the vertices and coordinates in the font atlas. So one single piece is needed: fill those
//! triangles into a picture, taking the TRANSPARENCY from the atlas. That is how the real backend
//! draws too — which is why the text, the lines and the glyphs come out the same as on screen rather
//! than merely similar.
//!
//! As a side effect this opens up captures of the WHOLE WINDOW (panels, menus, bars), should they be
//! needed.
#![cfg(test)]

use egui::{Color32, ColorImage};

/// Capture an `egui` frame into a picture.
///
/// `draw` paints exactly what must land in the frame; the background is given separately, because a
/// transparent background is meaningless for A CAPTURE OF THE INTERFACE — text is drawn with
/// semi-transparent antialiasing and spreads into mud over transparency.
pub(super) fn shot_ui(size: [usize; 2], bg: Color32, mut draw: impl FnMut(&egui::Context)) -> ColorImage {
    const SS: usize = 2;
    let (w, h) = (size[0] * SS, size[1] * SS);
    let ctx = egui::Context::default();
    super::install_fonts(&ctx);
    // SUPERSAMPLING THROUGH PIXEL DENSITY RATHER THAN CANVAS SIZE. The first edition simply took a
    // canvas twice as large — and the dimension captions and constraint glyphs came out half the size:
    // they are given in POINTS and do not react to a larger canvas. `egui` can do exactly what is
    // needed: the same points, twice as many pixels per point.
    ctx.set_pixels_per_point(SS as f32);
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(size[0] as f32, size[1] as f32));
    let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    // WARM-UP FRAMES WITH TIME, NOT ONE AT ZERO.
    //
    // Two reasons, and the second cost a long search. The first: `egui` defers layout by a frame, and
    // on the first one the panels do not yet know their sizes. The second: A WINDOW FADES IN by
    // animation — at zero time it is drawn semi-transparent. The capture came out twice as dim as the
    // program (reported as "as if under a shade"), while the panels — the ones drawn without animation
    // — were exact. So frames are run with growing time until the fade-in has finished.
    //
    // THE TEXTURES ARE COLLECTED FROM EVERY FRAME rather than from the first and the last. The picture
    // of the viewport (the software raster gives it as a separate texture) is loaded on some frame in
    // the middle, and the moment two frames became eight it disappeared from the assembly: the body
    // was drawn SOLID WHITE — without a texture the sampling honestly returns white.
    let mut texes: std::collections::HashMap<egui::TextureId, Tex> = std::collections::HashMap::new();
    let mut out = ctx.run(input.clone(), &mut draw);
    for (id, delta) in &out.textures_delta.set {
        apply_delta(&mut texes, *id, delta);
    }
    for i in 1..8 {
        let mut inp = input.clone();
        inp.time = Some(i as f64 * 0.25);
        out = ctx.run(inp, &mut draw);
        for (id, delta) in &out.textures_delta.set {
            apply_delta(&mut texes, *id, delta);
        }
    }

    // THE TEXTURES OF THE FRAME RATHER THAN ONE FONT ATLAS. The first edition slipped the atlas to
    // every triangle — and the picture of the viewport (the software rasteriser gives it as a SEPARATE
    // texture) turned into white noise: the letters read, the body did not. Everything the frame loaded
    // is collected, by the frame's own identifiers.
    let prims = ctx.tessellate(out.shapes, SS as f32);
    let mut img = ColorImage::new([w, h], bg);
    for p in &prims {
        if let egui::epaint::Primitive::Mesh(mesh) = &p.primitive {
            let tex = texes.get(&mesh.texture_id);
            for tri in mesh.indices.chunks_exact(3) {
                let v = [&mesh.vertices[tri[0] as usize], &mesh.vertices[tri[1] as usize], &mesh.vertices[tri[2] as usize]];
                fill_triangle(&mut img, tex, v, p.clip_rect, SS as f32);
            }
        }
    }
    downscale(&img, SS)
}

/// A texture of the frame: either coverage (the font atlas) or colour (the viewport picture, icons).
pub(super) enum Tex {
    Cover { size: [usize; 2], px: Vec<f32> },
    Color { size: [usize; 2], px: Vec<Color32> },
}

/// Apply a texture change sent by the frame. `pos` = Some means a PIECE was updated (the font atlas
/// is appended to as new letters appear), otherwise it was loaded whole.
fn apply_delta(texes: &mut std::collections::HashMap<egui::TextureId, Tex>, id: egui::TextureId, d: &egui::epaint::ImageDelta) {
    match &d.image {
        egui::ImageData::Font(f) => {
            let e = texes.entry(id).or_insert_with(|| Tex::Cover { size: f.size, px: vec![0.0; f.size[0] * f.size[1]] });
            if let Tex::Cover { size, px } = e {
                match d.pos {
                    None => {
                        *size = f.size;
                        *px = f.pixels.clone();
                    }
                    Some([ox, oy]) => {
                        for y in 0..f.size[1] {
                            for x in 0..f.size[0] {
                                if let Some(dst) = px.get_mut((oy + y) * size[0] + ox + x) {
                                    *dst = f.pixels[y * f.size[0] + x];
                                }
                            }
                        }
                    }
                }
            }
        }
        egui::ImageData::Color(c) => {
            texes.insert(id, Tex::Color { size: c.size, px: c.pixels.clone() });
        }
    }
}

/// Fill one triangle with interpolation of colour and texture coordinates.
///
/// Barycentrically, as it should be: in `egui` the colour and the transparency are given IN THE
/// VERTICES, and without interpolation both the antialiasing (made of a semi-transparent fringe) and
/// the gradients would be lost.
fn fill_triangle(img: &mut ColorImage, tex: Option<&Tex>, v: [&egui::epaint::Vertex; 3], clip: egui::Rect, k: f32) {
    let (w, h) = (img.size[0] as f32, img.size[1] as f32);
    // POINTS TO PIXELS: `egui` gives the vertices and the clip in points, and the backend accounts
    // for the density.
    let clip = egui::Rect::from_min_max(egui::pos2(clip.min.x * k, clip.min.y * k), egui::pos2(clip.max.x * k, clip.max.y * k));
    let xs = [v[0].pos.x * k, v[1].pos.x * k, v[2].pos.x * k];
    let ys = [v[0].pos.y * k, v[1].pos.y * k, v[2].pos.y * k];
    let x0 = xs.iter().cloned().fold(f32::MAX, f32::min).max(clip.min.x).max(0.0).floor() as i32;
    let x1 = xs.iter().cloned().fold(f32::MIN, f32::max).min(clip.max.x).min(w - 1.0).ceil() as i32;
    let y0 = ys.iter().cloned().fold(f32::MAX, f32::min).max(clip.min.y).max(0.0).floor() as i32;
    let y1 = ys.iter().cloned().fold(f32::MIN, f32::max).min(clip.max.y).min(h - 1.0).ceil() as i32;
    let area = (xs[1] - xs[0]) * (ys[2] - ys[0]) - (xs[2] - xs[0]) * (ys[1] - ys[0]);
    if area.abs() < 1e-9 {
        return; // a degenerate triangle: its area cannot be divided by, and there is nothing to fill
    }
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((xs[1] - px) * (ys[2] - py) - (xs[2] - px) * (ys[1] - py)) / area;
            let w1 = ((xs[2] - px) * (ys[0] - py) - (xs[0] - px) * (ys[2] - py)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let uv = egui::pos2(w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x, w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y);
            let (tc, cov) = sample(tex, uv);
            let col = [
                (blend_channel(v, |c| c.r(), w0, w1, w2) as u32 * tc.r() as u32 / 255) as u8,
                (blend_channel(v, |c| c.g(), w0, w1, w2) as u32 * tc.g() as u32 / 255) as u8,
                (blend_channel(v, |c| c.b(), w0, w1, w2) as u32 * tc.b() as u32 / 255) as u8,
                (blend_channel(v, |c| c.a(), w0, w1, w2) as u32 * tc.a() as u32 / 255) as u8,
            ];
            let a = col[3] as f32 / 255.0 * cov;
            if a <= 0.0 {
                continue;
            }
            let dst = img.pixels[y as usize * img.size[0] + x as usize];
            // BLENDED IN LINEAR SPACE RATHER THAN IN sRGB. The first edition added the bytes as they
            // were — and opaque panels came out exact while semi-transparent WINDOWS came out
            // noticeably darker than in the program: "as if under a shade", reported from captures of
            // the settings and the keys. The real `egui` backend converts the colour into linear space,
            // blends and converts back; at alpha=1 there is no difference at all — which is why the
            // discrepancy only showed on windows.
            let mix = |s: u8, d: u8| to_srgb(from_srgb(s) * a + from_srgb(d) * (1.0 - a));
            img.pixels[y as usize * img.size[0] + x as usize] = Color32::from_rgb(mix(col[0], dst.r()), mix(col[1], dst.g()), mix(col[2], dst.b()));
        }
    }
}

/// An sRGB byte to a linear value.
fn from_srgb(v: u8) -> f32 {
    let s = v as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// A linear value to an sRGB byte.
fn to_srgb(l: f32) -> u8 {
    let l = l.clamp(0.0, 1.0);
    let s = if l <= 0.0031308 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

fn blend_channel(v: [&egui::epaint::Vertex; 3], get: impl Fn(Color32) -> u8, w0: f32, w1: f32, w2: f32) -> u8 {
    (w0 * get(v[0].color) as f32 + w1 * get(v[1].color) as f32 + w2 * get(v[2].color) as f32).round().clamp(0.0, 255.0) as u8
}

/// TAKE FROM A TEXTURE: the colour and the coverage.
///
/// The font atlas is a map of COVERAGE (one value per pixel): the colour of a letter is set by the
/// vertex. An ordinary picture is the other way round: the colour is its own and the vertex only tints
/// it. Solid shapes look into the white pixel of the atlas and get one.
fn sample(tex: Option<&Tex>, uv: egui::Pos2) -> (Color32, f32) {
    // WHITE_UV MEANS "NO TEXTURE" rather than being a coordinate. Solid fills are marked with exactly
    // that, and a pixel of the atlas must not be taken by it: that is what was done at first, missing
    // the white texel for the neighbouring one, and THE WHOLE solid fill came out darker — a window
    // background of 21 instead of 27. That is noticeable only next to a panel drawn in the same
    // colour: "the windows look as if under a shade".
    if uv == egui::epaint::WHITE_UV {
        return (Color32::WHITE, 1.0);
    }
    let at = |size: [usize; 2]| {
        let x = (uv.x * size[0] as f32).round().clamp(0.0, size[0] as f32 - 1.0) as usize;
        let y = (uv.y * size[1] as f32).round().clamp(0.0, size[1] as f32 - 1.0) as usize;
        y * size[0] + x
    };
    match tex {
        // A COVERAGE GAMMA OF 0.55 is not decoration but what `egui` itself does when loading the
        // atlas (`FontImage::srgba_pixels`). Without it the text comes out TWICE as dim: a white meant
        // to be 140 landed as 68, and every capture of a window read "as if under a shade". The thin
        // strokes of letters cover a pixel halfway, and without the lightening they simply
        // disappear.
        Some(Tex::Cover { size, px }) => (Color32::WHITE, px.get(at(*size)).copied().unwrap_or(1.0).powf(0.55)),
        Some(Tex::Color { size, px }) => (px.get(at(*size)).copied().unwrap_or(Color32::WHITE), 1.0),
        None => (Color32::WHITE, 1.0),
    }
}

/// Average `k` by `k` pixels. The background is opaque, so there is nothing to premultiply.
fn downscale(img: &ColorImage, k: usize) -> ColorImage {
    let (w, h) = (img.size[0] / k, img.size[1] / k);
    let mut out = ColorImage::new([w, h], Color32::BLACK);
    let n = (k * k) as u32;
    for y in 0..h {
        for x in 0..w {
            // THE AVERAGING IS IN LINEAR SPACE TOO: the mean of two sRGB bytes is darker than the mean
            // of the light, and on the antialiased edges of letters that shows as thickening.
            let (mut r, mut g, mut b) = (0.0f32, 0.0f32, 0.0f32);
            for dy in 0..k {
                for dx in 0..k {
                    let p = img.pixels[(y * k + dy) * img.size[0] + x * k + dx];
                    r += from_srgb(p.r());
                    g += from_srgb(p.g());
                    b += from_srgb(p.b());
                }
            }
            let n = n as f32;
            out.pixels[y * w + x] = Color32::from_rgb(to_srgb(r / n), to_srgb(g / n), to_srgb(b / n));
        }
    }
    out
}
