//! A PROJECTED ELLIPSE OR SPLINE IS ONE CURVE OF THE SKETCH: the rim of a cylinder seen at an angle comes into a sketch as
//! one ellipse of the semi-axes the angle gives, the rim of a body extruded from a sketch ellipse as that ellipse, and the
//! rim of a body extruded from a closed spline as one spline for each of its edges, each on the edge it came from.
//!
//! Reported behaviour: everything that was neither a line nor a circle seen square came into the sketch as a polyline of
//! 24 segments an edge - a hole on an inclined face took no tangency and no point on it, and a dimension took a segment.
use qymcad_core::feature::{Ends, Purpose, SketchPlane};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{Id, ProjSource, Project, WorkPlane};
use qymcad_kernel::OcctKernel;
use std::collections::HashMap;

/// A live kernel with the shapes the bodies were built into, as the application holds them.
struct Live {
    p: Project,
    shapes: HashMap<Id, qymcad_kernel::Shape>,
}

impl Live {
    fn rebuild(&mut self) {
        let (_, sh) = qymcad_testkit::regenerate_with_shapes(&mut self.p, std::mem::take(&mut self.shapes));
        self.shapes = sh;
    }

    fn project_into(&mut self, si: usize, body: Id, src: ProjSource) -> Id {
        let k = OcctKernel { shapes: std::cell::RefCell::new(std::mem::take(&mut self.shapes)), ..Default::default() };
        let id = self.p.add_sketch_projection(si, body, src, &k);
        self.shapes = k.shapes.into_inner();
        id
    }

    /// The curves the projections of sketch `si` laid.
    fn projected(&self, si: usize) -> Vec<Curve> {
        let s = &self.p.sketches[si];
        let ids: Vec<Id> = s.projections.iter().flat_map(|x| x.entities.clone()).collect();
        s.entities.iter().filter(|e| ids.contains(&e.id)).filter_map(|e| Curve::of(s, e)).collect()
    }
}

/// The persistent id of the top face of a body.
fn top_face_id(p: &Project, body: Id) -> u32 {
    p.regen_faces.get(&body).and_then(|fs| fs.iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z))).map(|f| f.id).expect("there is a top face")
}

/// A new sketch with a timeline node, as the application makes one.
fn sketch(p: &mut Project, name: &str) -> usize {
    let si = p.new_sketch(name);
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, name);
    si
}

/// A PART BUILT LIVE, and the body in it.
struct Built {
    live: Live,
    body: Id,
}

/// A part, and in it the body `make` builds.
fn part(make: impl FnOnce(&mut Project) -> Id) -> Built {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let part = p.add_part("part");
    p.set_active_component(Some(part));
    let body = make(&mut p);
    let mut live = Live { p, shapes: HashMap::new() };
    live.rebuild();
    Built { live, body }
}

/// The body extruded 10 from a sketch on XY drawn by `draw`.
fn extruded(p: &mut Project, draw: fn(&mut Project, usize)) -> Id {
    let si = sketch(p, "profile");
    draw(p, si);
    p.regen_sketch(si);
    let body = p.add_extrude(p.sketches[si].id, 10.0);
    p.finish_base_body(body, 1);
    body
}

/// The semi-axes of an ellipse, the greater first.
fn semi_axes(c: &Curve) -> Option<[f64; 2]> {
    let Curve::Oval { major, minor, .. } = c else { return None };
    let (a, b) = (major.x.hypot(major.y), minor.x.hypot(minor.y));
    Some([a.max(b), a.min(b)])
}

#[test]
fn the_rim_of_a_cylinder_seen_at_30_deg_is_one_ellipse() {
    let Built { mut live, body } = part(|p| p.add_cylinder(8.0, 20.0));
    let face = top_face_id(&live.p, body);
    let tilt = 30f64.to_radians();
    let plane =
        live.p.add_plane(WorkPlane { id: 0, name: "tilted".into(), origin: [0.0, 0.0, 0.0], normal: [0.0, tilt.sin(), tilt.cos()], rot_deg: 0.0, tilt: Default::default(), def: Default::default() });
    let si = sketch(&mut live.p, "on the tilted plane");
    live.p.sketches[si].plane = SketchPlane::Datum(plane);
    live.rebuild();
    live.project_into(si, body, ProjSource::Face(face));
    let curves = live.projected(si);
    let axes: Vec<Option<[f64; 2]>> = curves.iter().map(semi_axes).collect();
    assert!(
        matches!(axes[..], [Some([a, b])] if (a - 8.0).abs() < 1e-4 && (b - 8.0 * tilt.cos()).abs() < 1e-4),
        "the rim seen at 30 deg is {} curves, semi-axes {axes:?}, not one ellipse of 8 and {:.4}",
        curves.len(),
        8.0 * tilt.cos()
    );
}

#[test]
fn the_rim_of_an_extruded_ellipse_is_that_ellipse() {
    let Built { mut live, body } = part(|p| {
        extruded(p, |p, si| {
            p.add_ellipse_entity(si, Point2::new(3.0, 2.0), 10.0, 5.0, 0.3, Purpose::Real);
        })
    });
    let face = top_face_id(&live.p, body);
    let si = sketch(&mut live.p, "above");
    live.rebuild();
    live.project_into(si, body, ProjSource::Face(face));
    let curves = live.projected(si);
    let axes: Vec<Option<[f64; 2]>> = curves.iter().map(semi_axes).collect();
    assert!(matches!(axes[..], [Some([a, b])] if (a - 10.0).abs() < 1e-4 && (b - 5.0).abs() < 1e-4), "the rim of the ellipse 10 x 5 is {} curves, semi-axes {axes:?}", curves.len());
}

#[test]
fn the_rim_of_an_extruded_spline_is_one_spline_an_edge() {
    let nodes = [(12.0, 0.0), (0.0, 7.0), (-10.0, 0.0), (0.0, -6.0)].map(|(x, y)| Point2::new(x, y));
    let Built { mut live, body } = part(|p| {
        extruded(p, |p, si| {
            let nodes = [(12.0, 0.0), (0.0, 7.0), (-10.0, 0.0), (0.0, -6.0)].map(|(x, y)| Point2::new(x, y));
            p.add_spline(si, nodes.to_vec(), Ends::Closed, Purpose::Real);
        })
    });
    let face = top_face_id(&live.p, body);
    let mut source = Project::default();
    let ssi = source.new_sketch("source");
    source.add_spline(ssi, nodes.to_vec(), Ends::Closed, Purpose::Real);
    let s = &source.sketches[ssi];
    let spline = Curve::of(s, &s.entities[0]).expect("the source spline");
    let si = sketch(&mut live.p, "above");
    live.rebuild();
    live.project_into(si, body, ProjSource::Face(face));
    let curves = live.projected(si);
    let splines = curves.iter().filter(|c| matches!(c, Curve::Spline { .. })).count();
    let off = curves.iter().flat_map(|c| (0..=50).map(move |k| c.at(k as f64 / 50.0))).map(|q| spline.nearest(q).off).fold(0.0, f64::max);
    assert!(curves.len() == 1 && splines == 1 && off < 0.01, "the rim of a closed spline of four spans is {} curves, {splines} of them splines, {off:.4} off the spline at most", curves.len());
}
