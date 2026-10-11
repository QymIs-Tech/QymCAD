//! A MESH'S SHARP EDGE STAYS SHARP ON BOTH PAINTERS. A mesh from a file shares its vertices across its edges, and
//! light smoothed over every shared vertex made a cube bulge like a cushion: a gradient over every face on the
//! software picture, eight vertices lit along the diagonals in the device's scene.
//!
//! Reported behaviour, measured: the owner's print head from PLY showed streaks along the edges and fillets of the
//! housing, 0.79 % of the picture apart from the head from STEP, where the same triangles with vertices of their own
//! came 0.03 % apart.
#[cfg(test)]
mod tests {
    use crate::gui::import_door::tests::{answer, cube_stl, frame, key, running, settle};
    use crate::gui::App;
    use qymcad_ui_state::Want;

    /// A cube from a text STL through the door, seen in 3D: its corners welded, every one shared by three faces.
    fn the_cube() -> App {
        let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/import-door"));
        std::fs::create_dir_all(&dir).expect("a folder for the check");
        let p = dir.join("sharp.stl");
        std::fs::write(&p, cube_stl(10.0)).expect("written");
        let (mut app, ctx) = running();
        answer(&mut app, &ctx, Want::Anything, &p.to_string_lossy());
        settle(&mut app, &ctx);
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter)); // the window about the unit, as the file has it
        let _ = frame(&mut app, &ctx, Vec::new());
        assert_eq!(app.project.bodies.iter().map(|b| b.mesh.verts.len()).collect::<Vec<_>>(), [8], "the cube's corners are not welded");
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        app
    }

    /// THE DEVICE'S SCENE: every corner of a face lit by the face - the 36 corners of the cube's 12 triangles as 24
    /// points with a normal of their own (a corner of the cube once for each of its three faces), none along a diagonal.
    #[test]
    fn the_scene_for_the_device_keeps_a_cubes_edges_sharp() {
        let app = the_cube();
        let (verts, _) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
        // how many axes the normal leans along: one for a face of the cube, more for a corner lit along a diagonal
        let axes = |v: &qymcad_ui_state::GpuVert| qymcad_core::geom::unpack_direction(v.nrm).map_or(0, |n| n.iter().filter(|c| c.abs() > 0.16).count());
        let diagonal = verts.iter().filter(|v| axes(v) > 1).count();
        let lit: std::collections::HashSet<([u32; 3], u32)> = verts.iter().map(|v| (v.pos.map(f32::to_bits), v.nrm)).collect();
        assert_eq!((verts.len(), lit.len(), diagonal), (36, 24, 0), "the cube's {} corners go to the device as {} lit points, {diagonal} corners lit along a diagonal", verts.len(), lit.len());
    }

    /// THE SOFTWARE PICTURE: a face in one shade, not a gradient.
    #[test]
    fn the_software_raster_paints_a_cubes_faces_flat() {
        let mut app = the_cube();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(700.0, 600.0));
        crate::gui::fit3d(&mut app.viewing.cam, &app.project, rect);
        let basis = app.viewing.cam.basis();
        let img = qymcad_render::rasterize_3d(&app.painting(), rect, &basis, 1.0, 1.0).expect("a raster");
        let shades: std::collections::HashSet<[u8; 3]> = img.pixels.iter().filter(|c| c.a() == 255).map(|c| [c.r(), c.g(), c.b()]).collect();
        let lit = img.pixels.iter().filter(|c| c.a() == 255).count();
        eprintln!("the cube: {lit} pixels in {} shades", shades.len());
        assert!(lit > 1000 && shades.len() <= 12, "the cube's {lit} pixels are painted in {} shades", shades.len());
    }
}
