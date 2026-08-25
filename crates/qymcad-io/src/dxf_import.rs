//! Importing DXF into exact primitive curves — segments, arcs and circles — rather than a tessellation.
//!
//! Supported: LINE, CIRCLE, ARC and polylines, including bulge arcs from fillets. A circle stays a circle and an
//! arc stays an arc; they are not broken into segments, or importing a drawing would yield thousands of them.
//! The sketch recovers connectivity and closure from shared endpoints, so nothing needs stitching here.

use dxf::entities::EntityType;
use dxf::Drawing;
use qymcad_core::geom::{Point2, ProfEdge};

use crate::ImportedSketch;

/// Import a DXF file into a set of exact curves.
pub fn import_dxf(path: &str) -> Result<ImportedSketch, String> {
    let drawing = Drawing::load_file(path).map_err(|e| format!("DXF load: {e}"))?;

    let mut curves: Vec<ProfEdge> = Vec::new();

    for entity in drawing.entities() {
        match &entity.specific {
            EntityType::Line(line) => {
                curves.push(ProfEdge::Line { a: Point2::new(line.p1.x, line.p1.y), b: Point2::new(line.p2.x, line.p2.y) });
            }
            EntityType::Circle(c) => {
                curves.push(ProfEdge::Circle { center: Point2::new(c.center.x, c.center.y), r: c.radius });
            }
            EntityType::Arc(a) => {
                curves.push(arc_from_angles(Point2::new(a.center.x, a.center.y), a.radius, a.start_angle.to_radians(), a.end_angle.to_radians()));
            }
            EntityType::LwPolyline(p) => {
                let verts: Vec<(Point2, f64)> = p.vertices.iter().map(|v| (Point2::new(v.x, v.y), v.bulge)).collect();
                polyline_curves(&verts, is_closed_flag(p.flags), &mut curves);
            }
            EntityType::Polyline(p) => {
                let verts: Vec<(Point2, f64)> = p.vertices().map(|v| (Point2::new(v.location.x, v.location.y), v.bulge)).collect();
                polyline_curves(&verts, p.is_closed(), &mut curves);
            }
            _ => {}
        }
    }

    Ok(ImportedSketch { curves })
}

fn is_closed_flag(flags: i32) -> bool {
    flags & 1 != 0
}

/// An arc from a centre, a radius and two angles; DXF runs counter-clockwise from start to end. The endpoints
/// lie on the circle.
fn arc_from_angles(center: Point2, r: f64, start: f64, end: f64) -> ProfEdge {
    let a = Point2::new(center.x + r * start.cos(), center.y + r * start.sin());
    let b = Point2::new(center.x + r * end.cos(), center.y + r * end.sin());
    ProfEdge::Arc { a, b, center, ccw: true }
}

/// A polyline becomes segments and arcs between adjacent vertices, a non-zero bulge giving an arc. `closed`
/// adds the closing edge from the last vertex to the first.
fn polyline_curves(verts: &[(Point2, f64)], closed: bool, out: &mut Vec<ProfEdge>) {
    let n = verts.len();
    if n < 2 {
        return;
    }
    let last = if closed { n } else { n - 1 };
    for i in 0..last {
        let (p1, bulge) = verts[i];
        let p2 = verts[(i + 1) % n].0;
        if p1.dist(p2) < 1e-9 {
            continue; // a degenerate edge
        }
        out.push(if bulge.abs() > 1e-9 { bulge_arc(p1, p2, bulge) } else { ProfEdge::Line { a: p1, b: p2 } });
    }
}

/// A DXF bulge arc, where `bulge = tan(θ/4)` with θ the inscribed angle and the sign giving the direction. The
/// centre comes from the cotangent formula, and a positive bulge means counter-clockwise.
fn bulge_arc(p1: Point2, p2: Point2, bulge: f64) -> ProfEdge {
    let cot = (1.0 / bulge - bulge) / 2.0;
    let cx = (p1.x + p2.x) / 2.0 - cot * (p2.y - p1.y) / 2.0;
    let cy = (p1.y + p2.y) / 2.0 + cot * (p2.x - p1.x) / 2.0;
    ProfEdge::Arc { a: p1, b: p2, center: Point2::new(cx, cy), ccw: bulge > 0.0 }
}
