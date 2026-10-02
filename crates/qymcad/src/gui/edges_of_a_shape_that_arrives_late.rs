//! THE EDGES OF A BODY WHOSE SOLID ARRIVED AFTER THEY WERE ASKED FOR.
//!
//! Reported need: a document of five rectangles - a cross of two bars - with three of them extruded into one
//! another. The list of the elements named TWELVE edges, which is exactly the one rectangle a first pass had
//! built, and the other rectangles had none at all: not in the list, not under the cursor, not in the details.
//!
//! Nothing was wrong with the naming. The live B-rep is prepared in the BACKGROUND, and the list asked for the
//! edges while it was still being built, so it was answered by the half-finished solid. The finished solid
//! then landed - and the answer stayed the one it had been given, because the thing that decides "is this
//! cache still true" is the revision of the GEOMETRY, and a solid arriving from a thread moves no revision at
//! all. The cache was then right for ever about a shape that had stopped existing.

#[cfg(test)]
mod tests {
    use super::super::App;

    /// THE SAME CROSS, EXTRUDED FROM THREE CONTOURS - and the same cross extruded from ONE. Both are built the
    /// way a person builds them, so the two solids are the kernel's own and not a drawing of them.
    fn cross(extrude: usize) -> App {
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
        app.tools.gsel.profiles.clear();
        for c in closed.iter().skip(1).take(extrude) {
            app.tools.gsel.profiles.insert(*c);
        }
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.viewing.mode_3d = true;
        app.apply_feat_cmd();
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
        app
    }

    /// THE ANSWER FOLLOWS THE SOLID IN, rather than standing where the first question found it.
    ///
    /// The shape is not re-read from the document between the two questions - there is no edit, no rebuild and
    /// no revision that moves, which is the whole of what a background job finishing looks like from here. So
    /// a cache keyed on the geometry alone cannot tell the two apart, and the second answer is the first one
    /// again: twelve edges for a body that now has eighteen, and every rectangle but the first missing.
    #[test]
    fn the_edges_follow_the_solid_in_from_the_background() {
        let mut app = cross(3);
        let id = app.project.mesh_id(0).expect("the cross is one body");
        let finished = app.live.shapes.remove(&id).expect("the finished solid");
        let many = finished.edges_full_smooth().1.len();
        assert!(many > 12, "the cross of three rectangles has more edges than one rectangle: {many}");

        // THE HALF-BUILT SOLID, of ONE rectangle: what the list was answered with while the job ran.
        let half = cross(1).live.shapes.values().next().expect("the half-built solid").transformed(&[1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0.]).unwrap();
        assert_eq!(half.edges_full_smooth().1.len(), 12, "one rectangle is twelve edges");

        // THE QUESTION IS ASKED OF THE HALF-BUILT SOLID, and the finished one arrives afterwards.
        app.live.shapes.insert(id, half);
        let early = crate::gui::pick::body_edges_cached(&app.cache, &app.live, &app.regen, id).map(|e| e.ids.len()).unwrap_or(0);
        assert_eq!(early, 12, "the half-built solid is the one that is there, and the list should say so");

        app.live.shapes.insert(id, finished);
        app.live.shapes_rev = app.live.shapes_rev.wrapping_add(1); // as the background job does when it adopts a solid
        let late = crate::gui::pick::body_edges_cached(&app.cache, &app.live, &app.regen, id).map(|e| e.ids.len()).unwrap_or(0);
        assert_eq!(late, many, "the solid that arrived is the one the list must now speak of: {late} edges against the {many} it has");
    }
}