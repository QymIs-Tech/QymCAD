//! Exporting a sketch to SVG, writing exact primitives — segments, arcs and circles — rather than a
//! tessellation.
//!
//! CAD coordinates run Y upwards while SVG runs Y downwards, so Y is inverted for the view to match.
use qymcad_core::geom::ProfEdge;
use std::io::Write;

/// Write the curves of a sketch to SVG. An error means there was nothing to export, or the write failed.
pub fn export_svg(edges: &[ProfEdge], path: &str) -> Result<(), String> {
    if edges.is_empty() {
        return Err("io-svg-empty-sketch".into());
    }
    // the extents for the viewBox, accounting for the full radius of circles and arcs
    let (mut xmin, mut ymin, mut xmax, mut ymax) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut grow = |x: f64, y: f64| {
        xmin = xmin.min(x);
        ymin = ymin.min(y);
        xmax = xmax.max(x);
        ymax = ymax.max(y);
    };
    for e in edges {
        match *e {
            ProfEdge::Line { a, b } => {
                grow(a.x, a.y);
                grow(b.x, b.y);
            }
            ProfEdge::Arc { a, b, center, .. } => {
                let r = ((a.x - center.x).powi(2) + (a.y - center.y).powi(2)).sqrt();
                grow(center.x - r, center.y - r);
                grow(center.x + r, center.y + r);
                grow(b.x, b.y);
            }
            ProfEdge::Circle { center, r } => {
                grow(center.x - r, center.y - r);
                grow(center.x + r, center.y + r);
            }
            ProfEdge::Oval { .. } | ProfEdge::Spline(_) => {
                for k in 0..=48 {
                    if let Some(p) = e.curve_at(k as f64 / 48.0) {
                        grow(p.x, p.y);
                    }
                }
            }
        }
    }
    let pad = ((xmax - xmin).max(ymax - ymin) * 0.05).max(1.0);
    let (w, h) = (xmax - xmin + 2.0 * pad, ymax - ymin + 2.0 * pad);
    // the mapping from CAD to SVG: sx = x − xmin + pad, sy = ymax − y + pad, inverting Y
    let sx = |x: f64| x - xmin + pad;
    let sy = |y: f64| ymax - y + pad;
    let stroke = (w.max(h) * 0.003).max(0.1);

    let mut s = String::new();
    s.push_str(&format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.3}mm\" height=\"{h:.3}mm\" viewBox=\"0 0 {w:.3} {h:.3}\">\n"));
    s.push_str(&format!("  <g fill=\"none\" stroke=\"black\" stroke-width=\"{stroke:.3}\">\n"));
    for e in edges {
        match *e {
            ProfEdge::Line { a, b } => {
                s.push_str(&format!("    <line x1=\"{:.4}\" y1=\"{:.4}\" x2=\"{:.4}\" y2=\"{:.4}\"/>\n", sx(a.x), sy(a.y), sx(b.x), sy(b.y)));
            }
            ProfEdge::Circle { center, r } => {
                s.push_str(&format!("    <circle cx=\"{:.4}\" cy=\"{:.4}\" r=\"{:.4}\"/>\n", sx(center.x), sy(center.y), r));
            }
            ProfEdge::Arc { a, b, center, ccw } => {
                let r = ((a.x - center.x).powi(2) + (a.y - center.y).powi(2)).sqrt();
                let ang_a = (a.y - center.y).atan2(a.x - center.x);
                let ang_b = (b.y - center.y).atan2(b.x - center.x);
                let two_pi = std::f64::consts::TAU;
                // the central angle in the direction of traversal
                let mut sweep = if ccw { ang_b - ang_a } else { ang_a - ang_b };
                sweep = sweep.rem_euclid(two_pi);
                let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
                // inverting Y reverses the on-screen direction, so a counter-clockwise CAD arc becomes
                // clockwise on screen and the sweep flag is 1
                let sflag = if ccw { 1 } else { 0 };
                s.push_str(&format!("    <path d=\"M {:.4} {:.4} A {:.4} {:.4} 0 {} {} {:.4} {:.4}\"/>\n", sx(a.x), sy(a.y), r, r, large, sflag, sx(b.x), sy(b.y)));
            }
            // a spline is a path of the cubic Beziers of SVG itself, one a span
            ProfEdge::Spline(ref b) => {
                let pieces = b.beziers();
                let Some(first) = pieces.first() else { continue };
                let mut d = format!("M {:.4} {:.4}", sx(first[0].x), sy(first[0].y));
                for [_, c1, c2, end] in &pieces {
                    d.push_str(&format!(" C {:.4} {:.4} {:.4} {:.4} {:.4} {:.4}", sx(c1.x), sy(c1.y), sx(c2.x), sy(c2.y), sx(end.x), sy(end.y)));
                }
                s.push_str(&format!("    <path d=\"{d}\"/>\n"));
            }
            // an ellipse as the elliptical arcs of SVG itself, a whole one as two halves: an arc of SVG from a point to the
            // same point draws nothing
            ProfEdge::Oval { centre, major, minor, from, sweep } => {
                let (rx, ry) = (major.x.hypot(major.y), minor.x.hypot(minor.y));
                // inverting Y turns the axis the other way round
                let tilt = -major.y.atan2(major.x).to_degrees();
                // the angle runs counter-clockwise when the minor axis is a quarter counter-clockwise of the major and the
                // sweep positive, or both the other way; counter-clockwise in CAD is clockwise on screen, sweep flag 1
                let ccw = (major.x * minor.y - major.y * minor.x > 0.0) == (sweep > 0.0);
                let at = |t: f64| (centre.x + major.x * t.cos() + minor.x * t.sin(), centre.y + major.y * t.cos() + minor.y * t.sin());
                let halves = if sweep.abs() >= std::f64::consts::TAU - 1e-12 { 2 } else { 1 };
                let (x0, y0) = at(from);
                let mut d = format!("M {:.4} {:.4}", sx(x0), sy(y0));
                for k in 1..=halves {
                    let part = sweep / halves as f64;
                    let (x, y) = at(from + part * k as f64);
                    let large = if part.abs() > std::f64::consts::PI { 1 } else { 0 };
                    d.push_str(&format!(" A {rx:.4} {ry:.4} {tilt:.4} {large} {} {:.4} {:.4}", if ccw { 1 } else { 0 }, sx(x), sy(y)));
                }
                s.push_str(&format!("    <path d=\"{d}\"/>\n"));
            }
        }
    }
    s.push_str("  </g>\n</svg>\n");
    std::fs::File::create(path).and_then(|mut f| f.write_all(s.as_bytes())).map_err(|e| format!("io-svg-write-failed#{e}"))
}
