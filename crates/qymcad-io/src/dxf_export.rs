//! Exporting a sketch to DXF: a minimal ASCII R12 file with an ENTITIES section of LINE, CIRCLE and ARC.
//!
//! DXF runs Y upwards as CAD does, so no inversion is needed. Arcs in DXF are always counter-clockwise, so a
//! clockwise edge has its angles swapped.
use qymcad_core::geom::ProfEdge;
use std::fmt::Write as _;
use std::io::Write as _;

/// Write the curves of a sketch to DXF. An error means there was nothing to export, or the write failed.
pub fn export_dxf(edges: &[ProfEdge], path: &str) -> Result<(), String> {
    if edges.is_empty() {
        return Err("io-dxf-empty-sketch".into());
    }
    let mut s = String::new();
    // a minimal preamble and the start of ENTITIES
    s.push_str("0\nSECTION\n2\nENTITIES\n");
    // helpers for the code and value pairs
    let ln = |code: i32, val: &str, out: &mut String| {
        let _ = write!(out, "{code}\n{val}\n");
    };
    for e in edges {
        match *e {
            ProfEdge::Line { a, b } => {
                ln(0, "LINE", &mut s);
                ln(8, "0", &mut s); // layer 0
                ln(10, &format!("{:.6}", a.x), &mut s);
                ln(20, &format!("{:.6}", a.y), &mut s);
                ln(30, "0.0", &mut s);
                ln(11, &format!("{:.6}", b.x), &mut s);
                ln(21, &format!("{:.6}", b.y), &mut s);
                ln(31, "0.0", &mut s);
            }
            ProfEdge::Circle { center, r } => {
                ln(0, "CIRCLE", &mut s);
                ln(8, "0", &mut s);
                ln(10, &format!("{:.6}", center.x), &mut s);
                ln(20, &format!("{:.6}", center.y), &mut s);
                ln(30, "0.0", &mut s);
                ln(40, &format!("{:.6}", r), &mut s);
            }
            ProfEdge::Arc { a, b, center, ccw } => {
                let r = ((a.x - center.x).powi(2) + (a.y - center.y).powi(2)).sqrt();
                let deg = |p_x: f64, p_y: f64| (p_y - center.y).atan2(p_x - center.x).to_degrees().rem_euclid(360.0);
                // DXF draws counter-clockwise from start to end; for a clockwise edge the same arc results
                // from swapping them
                let (start, end) = if ccw { (deg(a.x, a.y), deg(b.x, b.y)) } else { (deg(b.x, b.y), deg(a.x, a.y)) };
                ln(0, "ARC", &mut s);
                ln(8, "0", &mut s);
                ln(10, &format!("{:.6}", center.x), &mut s);
                ln(20, &format!("{:.6}", center.y), &mut s);
                ln(30, "0.0", &mut s);
                ln(40, &format!("{:.6}", r), &mut s);
                ln(50, &format!("{:.6}", start), &mut s);
                ln(51, &format!("{:.6}", end), &mut s);
            }
        }
    }
    s.push_str("0\nENDSEC\n0\nEOF\n");
    std::fs::File::create(path)
        .and_then(|mut f| f.write_all(s.as_bytes()))
        .map_err(|e| format!("io-dxf-write-failed#{e}"))
}
