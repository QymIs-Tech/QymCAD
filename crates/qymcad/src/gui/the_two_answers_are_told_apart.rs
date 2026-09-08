//! THE TWO ANSWERS OF A COMMAND ARE TOLD APART WITHOUT READING THEM.
//!
//! Reported behaviour: "highlight the Enter (apply) and Esc (cancel) buttons in the tool popups with
//! different colours, green and red say, so that it is clear."
//!
//! They used to differ by a tick glyph and a bold face. That is a difference one READS, and at the moment
//! of pressing nobody is reading: the hand goes where the eye has already decided, and both buttons were
//! the same grey patch.
//!
//! CHECKED ON THE PICTURE, not in the source. A guard reading the code would pass for a colour that never
//! reaches the screen - painted under something, clipped away, or drawn in a disabled state that greys it
//! out. The frame is rendered and its pixels are counted.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// How far a pixel may sit from the colour and still be that colour: the text is drawn with
    /// antialiasing, so its edges are blends of the colour and the background.
    const NEAR: i32 = 40;

    fn near(px: egui::Color32, want: egui::Color32) -> bool {
        let d = (px.r() as i32 - want.r() as i32).abs() + (px.g() as i32 - want.g() as i32).abs() + (px.b() as i32 - want.b() as i32).abs();
        d <= NEAR
    }

    fn count(img: &egui::ColorImage, want: egui::Color32) -> usize {
        img.pixels.iter().filter(|p| near(**p, want)).count()
    }

    /// A part with an open command, and the command bar drawn as a person sees it.
    fn shot_of_the_command_bar(app: &mut App) -> egui::ColorImage {
        let bg = app.scheme.pal.viewport_bg();
        super::super::help_raster::shot_ui([900, 90], bg, |ui| {
            let ctx = &ui.ctx().clone();
            qymcad_ui_state::apply_theme(&mut app.scheme, &app.set, ctx);
            app.feat_command_bar(ui);
        })
    }

    fn a_part_with_an_open_command() -> App {
        let mut app = App::default();
        app.set.gpu_viewport = false;
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.chosen.sel = super::super::Sel::Sketch(si);
        app.start_feat_cmd(1); // extrude
        app
    }

    /// BOTH COLOURS ARE ON THE SCREEN, AND THEY ARE NOT THE SAME COLOUR.
    ///
    /// Two claims in one check on purpose: "the apply button is green" is worth nothing if the cancel one
    /// is the same green, and the complaint is about telling them apart rather than about either colour.
    #[test]
    fn the_apply_and_the_cancel_button_are_different_colours() {
        let mut app = a_part_with_an_open_command();
        let (confirm, refuse) = (app.scheme.pal.confirm(), app.scheme.pal.refuse());
        assert!(!near(confirm, refuse), "GUARD: the scheme gives the two answers the same colour, so the picture could not tell them apart either");

        let img = shot_of_the_command_bar(&mut app);
        let (c, r) = (count(&img, confirm), count(&img, refuse));

        assert!(c > 20, "the apply button is not drawn in the scheme's confirm colour ({c} pixels of it on the screen)");
        assert!(r > 20, "the cancel button is not drawn in the scheme's refuse colour ({r} pixels of it on the screen)");
    }

    /// AND IN THE LIGHT SCHEME TOO.
    ///
    /// A colour tuned against a dark canvas can vanish on a light one, and the scheme's own legibility
    /// check measures a colour against the PANEL, not against the button it lands on.
    #[test]
    fn the_two_answers_are_told_apart_in_the_light_scheme_as_well() {
        let mut app = a_part_with_an_open_command();
        app.set.scheme = "light".into();
        qymcad_ui_state::apply_theme(&mut app.scheme, &app.set, &egui::Context::default());
        let (confirm, refuse) = (app.scheme.pal.confirm(), app.scheme.pal.refuse());

        let img = shot_of_the_command_bar(&mut app);
        assert!(count(&img, confirm) > 20, "the apply button loses its colour in the light scheme");
        assert!(count(&img, refuse) > 20, "the cancel button loses its colour in the light scheme");
    }
}
