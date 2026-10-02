//! EVERY PRISM OF A FUSE IS NAMED FOR ITSELF, and not after the first one.
//!
//! Reported need: a cross of five rectangles - two bars laid across one another - with three of them extruded
//! together. The list of the elements named twelve edges, which is exactly the twelve of ONE rectangle, and the
//! other rectangles had none at all: not in the list, not under the cursor, not in the details.
//!
//! Nothing was wrong with the geometry and nothing was wrong with the naming scheme either. Each prism is
//! numbered 1..n over its own edges, which is right and stable for a part extruded from one sketch; but the fuse
//! of several profiles took that numbering from each prism as it was and put them side by side WITHOUT MOVING ON.
//! Three prisms were all numbered 1..12, so the body had twelve names for thirty-six edges. Two edges of one
//! body under one name cannot be told apart by anything: `Sel::Edge(body, id)` does not, the drawn piece does
//! not, the cursor does not - and with the repeats taken out of the list, the half of the body whose edges were
//! repeated had none to show.

#[cfg(test)]
mod tests {
    use super::super::App;

    /// A PART EXTRUDED FROM `n` SEPARATE RECTANGLES, the way a person makes it: the sketch, the command, the
    /// height. Every rectangle is far from the others, so nothing is fused into anything and the body holds
    /// `n` whole solids - the plainest possible case of "several prisms, one body".
    fn apart(n: usize) -> App {
        let mut app = App::default();
        app.project.new_document();
        let root = app.project.root;
        app.project.set_active_component(Some(root));
        let part = app.project.add_part("Apart");
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        for k in 0..n {
            let x = 50.0 * k as f64;
            app.project.add_rect_entity(si, x, 0.0, x + 10.0, 10.0, qymcad_core::feature::Purpose::Real);
        }
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.chosen.sel = qymcad_ui_state::Sel::Sketch(si);
        app.start_feat_cmd(1);
        let closed = qymcad_ui_state::sketch_closed_contours(&app.project, si);
        app.tools.gsel.profiles.clear();
        for c in closed.iter().take(n) {
            app.tools.gsel.profiles.insert(*c);
        }
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.viewing.mode_3d = true;
        app.apply_feat_cmd();
        built(&mut app);
        app
    }

    /// THE CROSS OF THE REPORT: five rectangles, the middle one and one on each side, and the three outer ones
    /// extruded together.
    fn cross() -> App {
        let mut app = App::default();
        app.project.new_document();
        let root = app.project.root;
        app.project.set_active_component(Some(root));
        let part = app.project.add_part("Cross");
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 10.0, qymcad_core::feature::Purpose::Real);
        app.project.add_rect_entity(si, 15.0, -15.0, 25.0, 25.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.chosen.sel = qymcad_ui_state::Sel::Sketch(si);
        app.start_feat_cmd(1);
        let closed = qymcad_ui_state::sketch_closed_contours(&app.project, si);
        // THE CENTRE IS THE SMALLEST RECTANGLE, and it is the one left alone; the other four are the sides, and
        // three of them are extruded.
        let mut by_area: Vec<(f64, qymcad_core::model::Id)> = closed
            .iter()
            .map(|c| {
                let pts = &app.project.contours[app.project.contour_index(*c).unwrap()].points;
                let (mut lo, mut hi, mut bottom, mut top) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
                for p in pts {
                    lo = lo.min(p.x);
                    hi = hi.max(p.x);
                    bottom = bottom.min(p.y);
                    top = top.max(p.y);
                }
                ((hi - lo) * (top - bottom), *c)
            })
            .collect();
        by_area.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        app.tools.gsel.profiles.clear();
        for (_, c) in by_area.iter().skip(1).take(3) {
            app.tools.gsel.profiles.insert(*c);
        }
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.viewing.mode_3d = true;
        app.apply_feat_cmd();
        built(&mut app);
        app
    }

    fn built(app: &mut App) {
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
    }

    /// WHAT THE LIST AND THE KERNEL SAY ABOUT ONE BODY: how many solids it is, how many edges the kernel has,
    /// how many different names those edges carry, and how many rows the list will show.
    fn said_about(app: &App) -> (usize, usize, usize, usize) {
        let mut out = (0, 0, 0, 0);
        for mi in 0..app.project.bodies.len() {
            let id = app.project.mesh_id(mi).expect("a body of the document");
            let Some(shape) = app.live.shapes.get(&id) else { continue };
            let ids = shape.edges_full_smooth().1;
            let mut seen = std::collections::HashSet::new();
            out.0 += shape.solids_info().len();
            out.1 += ids.len();
            out.2 += ids.iter().filter(|i| seen.insert(**i)).count();
            out.3 += crate::gui::pick::body_edges_cached(&app.cache, &app.live, &app.regen, id).map(|e| e.ids.len()).unwrap_or(0);
        }
        out
    }

    /// EVERY RECTANGLE KEEPS ITS OWN TWELVE EDGES AND ITS OWN NAMES, and the list shows them all.
    ///
    /// The count alone is not the check - it passed before the fix as well, because the kernel always had every
    /// edge. What is checked is that NO EDGE SHARES A NAME with another, because a name that repeats is the
    /// whole of the fault: the body simply does not have words for the rest of itself, and the list cannot show
    /// an edge it cannot name.
    #[test]
    fn every_rectangle_of_a_fuse_is_named_for_itself() {
        for n in 1..=4 {
            let app = apart(n);
            let (solids, edges, distinct, listed) = said_about(&app);
            assert_eq!(solids, n, "the {n} rectangles are not {n} solids of one body");
            assert_eq!(edges, n * 12, "a rectangle is twelve edges");
            assert_eq!(distinct, edges, "{n} rectangles: {edges} edges but only {distinct} different names, so the body cannot name the rest of itself");
            assert_eq!(listed, edges, "the list shows {listed} of the {edges} edges");
        }
    }

    /// THE REPORTED CROSS, and the twelve it used to answer with.
    #[test]
    fn the_cross_names_every_rectangle_it_was_given() {
        let app = cross();
        let (solids, edges, distinct, listed) = said_about(&app);
        assert_eq!(solids, 3, "the three outer rectangles are three solids");
        assert_eq!(edges, 36, "three rectangles are thirty-six edges");
        assert_eq!(distinct, 36, "thirty-six edges and thirty-six names: not the twelve the report saw");
        assert_eq!(listed, 36, "the list names all thirty-six");
    }
}