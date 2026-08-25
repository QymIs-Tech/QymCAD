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
        }
    }
    let pad = ((xmax - xmin).max(ymax - ymin) * 0.05).max(1.0);
    let (w, h) = (xmax - xmin + 2.0 * pad, ymax - ymin + 2.0 * pad);
    // the mapping from CAD to SVG: sx = x − xmin + pad, sy = ymax − y + pad, inverting Y
    let sx = |x: f64| x - xmin + pad;
    let sy = |y: f64| ymax - y + pad;
    let stroke = (w.max(h) * 0.003).max(0.1);

    let mut s = String::new();
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.3}mm\" height=\"{h:.3}mm\" viewBox=\"0 0 {w:.3} {h:.3}\">\n"
    ));
    s.push_str(&format!(
        "  <g fill=\"none\" stroke=\"black\" stroke-width=\"{stroke:.3}\">\n"
    ));
    for e in edges {
        match *e {
            ProfEdge::Line { a, b } => {
                s.push_str(&format!(
                    "    <line x1=\"{:.4}\" y1=\"{:.4}\" x2=\"{:.4}\" y2=\"{:.4}\"/>\n",
                    sx(a.x), sy(a.y), sx(b.x), sy(b.y)
                ));
            }
            ProfEdge::Circle { center, r } => {
                s.push_str(&format!(
                    "    <circle cx=\"{:.4}\" cy=\"{:.4}\" r=\"{:.4}\"/>\n",
                    sx(center.x), sy(center.y), r
                ));
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
                s.push_str(&format!(
                    "    <path d=\"M {:.4} {:.4} A {:.4} {:.4} 0 {} {} {:.4} {:.4}\"/>\n",
                    sx(a.x), sy(a.y), r, r, large, sflag, sx(b.x), sy(b.y)
                ));
            }
        }
    }
    s.push_str("  </g>\n</svg>\n");
    std::fs::File::create(path)
        .and_then(|mut f| f.write_all(s.as_bytes()))
        .map_err(|e| format!("io-svg-write-failed#{e}"))
}
