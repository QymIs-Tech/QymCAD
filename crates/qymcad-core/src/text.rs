//! Text contours from a TTF or OTF font, for labels in a sketch and for engraving.
//!
//! Glyph outlines are extracted through `ttf-parser` and the Bezier splines are flattened into polylines. Font
//! coordinates run Y upwards, which matches the sketch coordinate system.

use crate::geom::{Contour, Point2};

/// A collector of glyph outlines: it flattens quadratic and cubic Beziers into segments.
#[derive(Default)]
struct Outline {
    contours: Vec<Vec<(f32, f32)>>,
    cur: Vec<(f32, f32)>,
    last: (f32, f32),
}

impl Outline {
    fn flush(&mut self) {
        if self.cur.len() >= 3 {
            self.contours.push(std::mem::take(&mut self.cur));
        } else {
            self.cur.clear();
        }
    }
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.flush();
        self.cur.push((x, y));
        self.last = (x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.cur.push((x, y));
        self.last = (x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (p0x, p0y) = self.last;
        let n = 8;
        for k in 1..=n {
            let t = k as f32 / n as f32;
            let mt = 1.0 - t;
            let px = mt * mt * p0x + 2.0 * mt * t * x1 + t * t * x;
            let py = mt * mt * p0y + 2.0 * mt * t * y1 + t * t * y;
            self.cur.push((px, py));
        }
        self.last = (x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (p0x, p0y) = self.last;
        let n = 10;
        for k in 1..=n {
            let t = k as f32 / n as f32;
            let mt = 1.0 - t;
            let px = mt * mt * mt * p0x + 3.0 * mt * mt * t * x1 + 3.0 * mt * t * t * x2 + t * t * t * x;
            let py = mt * mt * mt * p0y + 3.0 * mt * mt * t * y1 + 3.0 * mt * t * t * y2 + t * t * t * y;
            self.cur.push((px, py));
        }
        self.last = (x, y);
    }
    fn close(&mut self) {
        self.flush();
    }
}

/// The contours of the string `text` at a height of `height` mm, with its lower-left corner at (ox, oy).
///
/// Every glyph yields its own contours: an outer loop plus the inner loops of letters such as O and A.
pub fn text_outline_contours(font: &[u8], text: &str, height: f64, ox: f64, oy: f64) -> Vec<Contour> {
    let Ok(face) = ttf_parser::Face::parse(font, 0) else {
        return Vec::new();
    };
    let upem = face.units_per_em() as f64;
    if upem <= 0.0 {
        return Vec::new();
    }
    let scale = height / upem;
    let mut out = Vec::new();
    let mut pen = 0.0_f64;
    for ch in text.chars() {
        if ch == ' ' {
            pen += face.glyph_index(' ').and_then(|g| face.glyph_hor_advance(g)).unwrap_or((upem * 0.3) as u16) as f64;
            continue;
        }
        let Some(gid) = face.glyph_index(ch) else { continue };
        let mut b = Outline::default();
        if face.outline_glyph(gid, &mut b).is_some() {
            b.flush();
            for c in &b.contours {
                let pts: Vec<Point2> = c.iter().map(|(x, y)| Point2::new(ox + (pen + *x as f64) * scale, oy + *y as f64 * scale)).collect();
                if pts.len() >= 3 {
                    out.push(Contour::closed(pts));
                }
            }
        }
        pen += face.glyph_hor_advance(gid).unwrap_or(0) as f64;
    }
    out
}
