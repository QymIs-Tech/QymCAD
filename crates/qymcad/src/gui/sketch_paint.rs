//! THE SKETCHER PAINTS THE GEOMETRY — written from a report.
//!
//! Reported behaviour: sketch geometry is no longer visible in the sketcher at all — in the dark
//! scheme and in the light one alike. The points and the constraint glyphs are there, the lines are
//! not.
//!
//! Not one of the earlier tests could catch that: they checked the colours, the geometry and the
//! sources, but NOT ONE built a sketcher frame and looked at what ended up in it. Exactly the same
//! mistake as with the view cube: "the shape adds up" is not the same as "it is drawn".
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    /// A rectangle sketch with the edit FINISHED — the ordinary state: the sketch is selected but not
    /// open.
    ///
    /// Leaving the edit is stated explicitly: `create_sketch_on` enters it by itself, and relying on
    /// that silently means writing tests about a mode you think you are not in (that has already
    /// caught people out).
    fn editing_rect() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.sketch_ses.editing = None;
        app.sel = Sel::Sketch(si);
        app.view.scale = 6.0;
        (app, si)
    }

    /// One frame: `draw` paints over an ordinary panel, and ALL the shapes of the frame are returned.
    fn frame(app: &App, draw: bool) -> Vec<egui::epaint::ClippedShape> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let painter = ui.painter().clone();
                if draw {
                    app.draw_contours(&painter, rect());
                }
            });
        })
        .shapes
    }

    /// How many lines THE SKETCHER ITSELF added. Counted as the DIFFERENCE from an empty frame: an
    /// egui panel draws a frame of its own, and the absolute number is always above zero — a check for
    /// "it drew something at all" would be green even when there is no geometry in the frame. That is
    /// what caught the first version of this test out.
    fn painted_lines(app: &App) -> usize {
        line_shapes(&frame(app, true)).saturating_sub(line_shapes(&frame(app, false)))
    }

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0))
    }

    /// How many segments and polylines ended up in the frame. LINES are what is counted: fills, texts
    /// and circles are of no interest here — it was the geometry that went missing.
    fn line_shapes(shapes: &[egui::epaint::ClippedShape]) -> usize {
        fn count(s: &egui::Shape) -> usize {
            match s {
                egui::Shape::LineSegment { .. } => 1,
                egui::Shape::Path(p) => usize::from(p.points.len() >= 2),
                egui::Shape::Vec(v) => v.iter().map(count).sum(),
                _ => 0,
            }
        }
        shapes.iter().map(|c| count(&c.shape)).sum()
    }

    /// THE SKETCH LINES REACH THE FRAME. The report one for one.
    #[test]
    fn the_sketcher_paints_the_geometry_it_was_given() {
        let (app, _si) = editing_rect();
        assert!(!app.project.contours.is_empty(), "a rectangle must give a contour — otherwise there is nothing to draw");

        assert!(painted_lines(&app) > 0, "the sketcher drew not one line — exactly what was reported");
    }

    /// ONE THING GOVERNS THE VISIBILITY OF A SKETCH — ITS OWN CHECKBOX.
    ///
    /// There used to be a general switch for sketch contours as well. It duplicated the checkbox of
    /// the sketch and could hide the very thing a person had just asked to be shown: with the switch
    /// off, picking contours in the right column highlighted nothing, and there was no way to tell
    /// which contour to choose. On top of that it put out the sketch editor itself. The switch was
    /// removed, and one truth was left.
    #[test]
    fn only_the_sketchs_own_checkbox_hides_it() {
        let (mut app, si) = editing_rect();
        let sid = app.project.sketches[si].id;
        assert!(painted_lines(&app) > 0, "by default the sketch is visible");

        app.sketch_hidden.insert(sid);
        assert_eq!(painted_lines(&app), 0, "an unticked sketch checkbox hides its contours");

        // but a sketch open for editing is ALWAYS visible: that is what it was opened for
        app.sketch_ses.editing = Some(sid);
        assert!(painted_lines(&app) > 0, "a sketch being edited must not be hidden — otherwise the editor is empty");
        app.sketch_ses.editing = None;

        app.sketch_hidden.remove(&sid);
        assert!(painted_lines(&app) > 0, "the checkbox came back and so did the contours");
    }

    /// THE GENERAL SWITCH LIVES ONLY IN THE ASSEMBLY.
    ///
    /// It must not be in a Part: there every sketch has a checkbox of its own, and a second, general
    /// one would duplicate it. But an ASSEMBLY holds dozens of sketches across all the components, and
    /// putting them out one at a time is impractical — there it is needed. Removing it everywhere (as
    /// was done at first) takes away the only way to clear that mess off the assembly screen.
    #[test]
    fn the_global_switch_works_in_the_assembly_and_is_absent_in_a_part() {
        let (mut app, _si) = editing_rect();

        app.workbench = super::super::Workbench::Part;
        app.set.show_contours = false;
        assert!(painted_lines(&app) > 0, "in a Part the general switch has no effect — the checkbox of the sketch decides there");

        app.workbench = super::super::Workbench::Assembly;
        assert_eq!(painted_lines(&app), 0, "in an Assembly the general switch must put out the contours");
        app.set.show_contours = true;
        assert!(painted_lines(&app) > 0, "switched on and the contours came back");
    }

    /// AND THE CHECKBOX IN THE INTERFACE STANDS WHERE IT ACTS — next to the joints, under the
    /// assembly condition.
    #[test]
    fn the_switch_is_offered_only_where_it_does_something() {
        let panels = crate::gui::panels_source::PANELS;
        let at = panels.find("show_contours").expect("the contours checkbox is there");
        // backwards by LINES rather than by bytes: a slice in the middle of a multibyte letter fails
        // the test with a panic
        let before: String = panels[..at].lines().rev().take(8).collect::<Vec<_>>().join("\n");
        assert!(
            before.contains("Workbench::Assembly"),
            "the contours switch must stand under the \"this is an assembly\" condition — it must not be in a Part"
        );
    }

    /// THE GEOMETRY IS VISIBLE IN BOTH SCHEMES: the colour of a line does not match the canvas
    /// background.
    ///
    /// The palette check measures the contrast by colour names. Here it goes by what the lines in the
    /// frame are ACTUALLY painted with: the wrong name may have been chosen, and the palette will
    /// never learn of it.
    ///
    /// The schemes are named BY IDENTIFIER: `apply_theme` looks them up by `Palette::id`, and a
    /// display caption would find nothing and quietly fall back to the dark one — the light scheme
    /// would then never be checked at all.
    #[test]
    fn the_painted_lines_differ_from_the_canvas_in_every_scheme() {
        for scheme in ["dark", "light"] {
            let (mut app, _) = editing_rect();
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            app.set.scheme = scheme.into();
            app.apply_theme(&ctx);
            let bg = app.palette_pub().viewport_bg();

            let shapes = frame(&app, true);
            let mut seen = 0usize;
            for c in &shapes {
                if let egui::Shape::Path(p) = &c.shape {
                    if p.points.len() < 2 {
                        continue;
                    }
                    seen += 1;
                    let egui::epaint::ColorMode::Solid(col) = p.stroke.color else {
                        panic!("in the \"{scheme}\" scheme the stroke of a sketch line is not a solid colour");
                    };
                    let d = |a: u8, b: u8| (a as i32 - b as i32).abs();
                    let diff = d(col.r(), bg.r()) + d(col.g(), bg.g()) + d(col.b(), bg.b());
                    assert!(col.a() > 0, "in the \"{scheme}\" scheme a sketch line is transparent");
                    assert!(diff > 60, "in the \"{scheme}\" scheme the sketch line {col:?} merges with the background {bg:?}");
                }
            }
            assert!(seen > 0, "in the \"{scheme}\" scheme there were no lines in the frame at all");
        }
    }
}
