//! AN ARC LENGTH IS DRAWN AS A DIMENSION: a dimension arc about the centre of the arc, outside it, extension lines from
//! the ends of the arc out to it, and the number over its middle with an arc mark over the number - all outside the
//! arc it measures, for arcs turning either way.
//!
//! Reported behaviour: the arc length was a caption "L1.0" beside the middle of the arc, crooked and saying nothing of
//! what it measured.
use qymcad_core::feature::{Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, EntityKind, Project};
use qymcad_ui_state::{arc_length_dim_geom, Settings, Sheet, View2d};

/// A quarter arc of radius 10 about (0, 0) turning `winding`, with its length dimensioned. Answers the project, the
/// sketch, the index of the dimension and the centre.
fn an_arc_with_its_length(winding: Winding) -> (Project, usize, usize) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let (a, b) = match winding {
        Winding::Ccw => (Point2::new(10.0, 0.0), Point2::new(0.0, 10.0)),
        Winding::Cw => (Point2::new(0.0, 10.0), Point2::new(10.0, 0.0)),
    };
    p.add_arc_entity(si, Point2::new(0.0, 0.0), a, b, winding, Purpose::Real);
    let s = &p.sketches[si];
    let Some((c, a, b, ccw)) = s.entities.iter().find_map(|e| match e.kind {
        EntityKind::Arc { center, a, b, ccw } => Some((center, a, b, ccw)),
        _ => None,
    }) else {
        panic!("the arc")
    };
    let len = 10.0 * std::f64::consts::FRAC_PI_2;
    p.sketches[si].constraints.push(Constraint::ArcLength { dimension: Default::default(), c, a, b, ccw, len, off: 0.0, expr: String::new(), driven: false });
    let ci = p.sketches[si].constraints.len() - 1;
    (p, si, ci)
}

#[test]
fn the_dimension_of_an_arc_length_stands_outside_the_arc() {
    let sheet = Sheet { view: View2d { scale: 10.0, ..View2d::default() }, rect: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0)) };
    let centre = sheet.at(Point2::new(0.0, 0.0));
    let rim = 10.0 * 10.0; // the radius on screen, px
    for winding in [Winding::Ccw, Winding::Cw] {
        let (p, si, ci) = an_arc_with_its_length(winding);
        let g = arc_length_dim_geom(&p, si, ci, &sheet, &Settings::default()).expect("the geometry of the arc length");
        let radii: Vec<f32> = g.arc.iter().map(|q| q.distance(centre)).collect();
        assert!(radii.iter().all(|r| (r - radii[0]).abs() < 0.5 && *r > rim + 5.0), "{winding:?}: the dimension arc is not about the centre outside the arc: {radii:?}");
        for e in g.ext {
            assert!((e[0].distance(centre) - rim).abs() < 0.5 && e[1].distance(centre) > radii[0], "{winding:?}: an extension line does not run from the arc out past the dimension arc: {e:?}");
        }
        let mid_angle = |q: egui::Pos2| (q - centre).angle();
        let text_angle = mid_angle(g.text);
        let mid = mid_angle(g.arc[g.arc.len() / 2]);
        assert!(g.text.distance(centre) > radii[0] && (text_angle - mid).abs() < 0.2, "{winding:?}: the number does not stand outside over the middle of the dimension arc: {:?}", g.text);
        assert!(g.mark.iter().all(|q| q.distance(centre) > g.text.distance(centre)), "{winding:?}: the arc mark is not over the number");
    }
}
