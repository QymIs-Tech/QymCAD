//! A FACE OF A COLOUR OF ITS OWN IS DRAWN IN IT, by both painters: the software raster and the scene handed to the
//! graphics device.
//!
//! The reference is the kernel's `tests/data/assembly.step`: two red plates, the top face of the plate green - a colour
//! the file gives that face alone - and a blue pin. Green is on the picture only where that face is.
#[cfg(test)]
mod tests {
    use crate::gui::a_component_stepped_into_is_not_lit::tests::calm;
    use crate::gui::import_door::tests::{answer, running, settle};
    use crate::gui::App;
    use qymcad_ui_state::Want;

    const GREEN: [u8; 3] = [26, 204, 26];

    /// Green, as the painters give it: shading lifts all three channels (the green face comes out [115, 216, 137] on
    /// the software picture, beside a red plate at [176, 119, 133] and a blue pin at [115, 136, 244]), so what tells it
    /// apart is green standing well above both others.
    fn greenish(r: u8, g: u8, b: u8) -> bool {
        g as i32 > r as i32 + 60 && g as i32 > b as i32 + 60
    }

    /// The reference, come in by the door and rebuilt, seen in 3D from above.
    fn the_reference() -> App {
        let (mut app, ctx) = running();
        answer(&mut app, &ctx, Want::Anything, concat!(env!("CARGO_MANIFEST_DIR"), "/../qymcad-kernel/tests/data/assembly.step"));
        settle(&mut app, &ctx);
        calm(&mut app, &ctx);
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        app
    }

    /// THE DOCUMENT HOLDS THE FACE'S COLOUR where the painters look for it: the top face of the first plate's body,
    /// under the name the rebuild gave it - face 6 of the file, named by where it lies.
    fn the_plate(app: &App) -> u64 {
        let plate = app
            .project
            .components
            .iter()
            .find(|c| app.project.component_bodies(c.id).len() == 1 && app.project.mesh_index(app.project.component_bodies(c.id)[0]).is_some_and(|i| app.project.mesh_color(i) == [204, 26, 26]))
            .map(|c| app.project.component_bodies(c.id)[0])
            .expect("a red plate");
        let top = app.project.regen_faces.get(&plate).and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.99 && (f.centroid.z - 5.0).abs() < 1e-6)).map(|f| f.id).expect("the top face of the plate");
        assert_eq!(app.project.face_color(plate, top), Some(GREEN), "the document does not hold the top face's colour");
        plate
    }

    #[test]
    fn the_software_raster_draws_the_face_in_its_colour() {
        let mut app = the_reference();
        let _ = the_plate(&app);
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(700.0, 600.0));
        crate::gui::fit3d(&mut app.viewing.cam, &app.project, rect);
        let basis = app.viewing.cam.basis();
        let img = qymcad_render::rasterize_3d(&app.painting(), rect, &basis, 1.0, 1.0).expect("a raster");
        let green = img.pixels.iter().filter(|c| greenish(c.r(), c.g(), c.b())).count();
        assert!(green > 200, "the top face is not drawn green on the software picture: {green} green pixels");
    }

    #[test]
    fn the_scene_for_the_device_draws_the_face_in_its_colour() {
        let app = the_reference();
        let _ = the_plate(&app);
        let (verts, looks) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
        let green = verts
            .iter()
            .filter(|v| {
                looks.get(v.body as usize).is_some_and(|l| {
                    let [r, g, b, _] = l.tint.to_le_bytes();
                    greenish(r, g, b)
                })
            })
            .count();
        assert!(green >= 6, "no triangle of the scene names a green row of the look table: {green} vertices");
        let red = verts
            .iter()
            .filter(|v| {
                looks.get(v.body as usize).is_some_and(|l| {
                    let [r, g, b, _] = l.tint.to_le_bytes();
                    r > 140 && g < 120 && b < 120
                })
            })
            .count();
        assert!(red > green, "the rest of the plates is not red any more: {red} red vertices, {green} green");
    }
}
