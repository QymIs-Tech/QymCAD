//! THE CARD AND THE SOFTWARE RASTER MUST DRAW THE SAME BODY.
//!
//! Reported behaviour: extruded text lost its walls on screen - "the faces fell apart" - while the software
//! raster drew the same document correctly. Everything that happens below the vertices is invisible to a check
//! that reads numbers: the culling, the shading, the look table, the indices. None of it crashes when wrong;
//! it draws the body wrong.
//!
//! So both renderers draw the same scene and the pictures are compared by SILHOUETTE - which pixels the body
//! covers. The colours differ by design (the raster shades per vertex, the card per fragment), the coverage
//! must not.
#[cfg(test)]
mod tests {
    use crate::gui::App;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(700.0, 500.0))
    }

    /// The font shipped in this repository - a `.ttf`, the format the text defect lives in.
    const FONT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/LiberationSans-Bold.ttf");

    /// A part with the word "Text" extruded out of a sketch, in a font from a file.
    fn extruded_text(app: &mut App) {
        let font = std::fs::read(FONT).expect("the font shipped with the repository");
        let glyphs: Vec<Vec<qymcad_core::geom::Point2>> = qymcad_core::text::text_outline_contours(&font, 0, "Text", 30.0, 0.0, 0.0).into_iter().map(|c| c.points).collect();
        // A PART TO PUT IT IN: a body belongs to a component, and without one the node has no owner and is
        // never built.
        let root = app.project.root;
        app.project.set_active_component(Some(root));
        let part = app.project.add_part("Text");
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_sketch_text(
            si,
            qymcad_core::model::TextSpec {
                at: qymcad_core::geom::Point2::new(0.0, 0.0),
                height: 30.0,
                angle: 0.0,
                text: "Text".into(),
                glyphs,
                font: qymcad_core::model::FontRef { family: "Liberation Sans".into(), path: FONT.into(), index: 0 },
            },
            qymcad_core::feature::Purpose::Real,
        );
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        assert!(!app.project.sketches[si].contour_ids.is_empty(), "setup: the text gave no contours to extrude");
        // THE EXTRUDE GOES THROUGH THE COMMAND, the way a person does it: pick the sketch, take the tool, set
        // the height, press Enter.
        let sid = app.project.sketches[si].id;
        let profiles: Vec<qymcad_core::model::Id> = app.project.sketches[si].contour_ids.clone();
        app.project.add_extrude_multi(sid, profiles, 8.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());
        qymcad_ui_state::regenerate_now(&mut app.rebuild_ctx());
        assert!(!app.project.bodies.is_empty(), "setup: the text did not extrude");
    }

    /// Point the camera at everything there is, from the usual three-quarter view.
    fn fit(app: &mut App) {
        let basis = app.viewing.cam.basis();
        let (right, up) = (basis.0, basis.1);
        let mut pts: Vec<[f64; 3]> = Vec::new();
        for (i, b) in app.project.bodies.iter().enumerate() {
            let Some(id) = app.project.mesh_id(i) else { continue };
            let wt = app.project.body_world_transform(id);
            for v in &b.mesh.verts {
                pts.push(qymcad_core::feature::apply12(&wt, [v.x, v.y, v.z]));
            }
        }
        assert!(!pts.is_empty(), "setup: there is nothing to look at");
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for q in &pts {
            for k in 0..3 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let (mut sx, mut sy) = (0.0f64, 0.0f64);
        for q in &pts {
            let rel = [q[0] - mid[0], q[1] - mid[1], q[2] - mid[2]];
            sx = sx.max((rel[0] * right[0] + rel[1] * right[1] + rel[2] * right[2]).abs());
            sy = sy.max((rel[0] * up[0] + rel[1] * up[1] + rel[2] * up[2]).abs());
        }
        app.viewing.cam.target = mid;
        let r = rect();
        app.viewing.cam.scale = ((r.width() as f64 / 2.0 / sx.max(1e-6)).min(r.height() as f64 / 2.0 / sy.max(1e-6)) * 0.85) as f32;
        app.viewing.cam.init = true;
        app.viewing.mode_3d = true;
    }

    /// Which pixels the body covers, and where the two pictures disagree about it.
    fn compare(app: &mut App, name: &str) -> Option<(usize, usize)> {
        let basis = app.viewing.cam.basis();
        let cpu: egui::ColorImage = qymcad_render::rasterize_3d(&app.painting(), rect(), &basis, 1.0, 1.0).expect("the raster drew nothing");
        let gpu = crate::gui::gpu_shot::eyes::shot(&app.painting(), rect())?;
        assert_eq!(cpu.size, gpu.size, "the two pictures are of different sizes");
        // THE COLOUR, NOT ONLY THE SILHOUETTE. A lost top face leaves the outline where it was - the inner
        // wall shows through it - so coverage alone calls the two pictures equal while a face is missing.
        let (mut covered, mut differ) = (0usize, 0usize);
        for (a, b) in cpu.pixels.iter().zip(&gpu.pixels) {
            let (ca, cb) = (a.a() > 128, b.a() > 128);
            if ca || cb {
                covered += 1;
            }
            let far = ca != cb || (a.r() as i32 - b.r() as i32).abs().max((a.g() as i32 - b.g() as i32).abs()).max((a.b() as i32 - b.b() as i32).abs()) > 24;
            if far {
                differ += 1;
            }
        }
        {
            let mut bins = [0usize; 5];
            for (a, b) in cpu.pixels.iter().zip(&gpu.pixels) {
                if !(a.a() > 128 || b.a() > 128) {
                    continue;
                }
                let d = (a.r() as i32 - b.r() as i32).abs().max((a.g() as i32 - b.g() as i32).abs()).max((a.b() as i32 - b.b() as i32).abs());
                for (k, lim) in [8, 24, 48, 96, 160].into_iter().enumerate() {
                    if d > lim {
                        bins[k] += 1;
                    }
                }
            }
            eprintln!("  by how far: >8 {}, >24 {}, >48 {}, >96 {}, >160 {}", bins[0], bins[1], bins[2], bins[3], bins[4]);
        }
        // the pictures are kept beside the numbers: when this goes red, the eye has to see what happened
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look");
        std::fs::create_dir_all(&dir).ok();
        for (img, tail) in [(&cpu, "raster"), (&gpu, "card")] {
            if let Some(png) = crate::gui::color_image_to_png(img) {
                std::fs::write(dir.join(format!("{name}-{tail}.png")), png).ok();
            }
        }
        Some((covered, differ))
    }

    /// A cylinder standing on the origin, as a part of its own.
    fn cylinder(app: &mut App) {
        let root = app.project.root;
        app.project.set_active_component(Some(root));
        let part = app.project.add_part("Cylinder");
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_circle_entity(si, 0.0, 0.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        let sid = app.project.sketches[si].id;
        let profiles: Vec<qymcad_core::model::Id> = app.project.sketches[si].contour_ids.clone();
        app.project.add_extrude_multi(sid, profiles, 80.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());
        qymcad_ui_state::regenerate_now(&mut app.rebuild_ctx());
        assert!(!app.project.bodies.is_empty(), "setup: the cylinder was not built");
    }

    /// A DEVICE REACHED THROUGH OPENGL DRAWS THE BODY TOO. Reported behaviour, a virtual machine under Windows (Mesa
    /// GL 4.3 on an emulated adapter): the program closed at start with exit code 101 and no window - the look table
    /// asked for a storage buffer in the fragment stage, where that device allows none. The limits of the weakest
    /// device wgpu names (the WebGL2 level) stand in for it here, on the card of this machine.
    #[test]
    fn a_device_without_storage_buffers_draws_the_body() {
        let mut app = App::default();
        cylinder(&mut app);
        fit(&mut app);
        let weakest = |a: &eframe::wgpu::Adapter| eframe::wgpu::Limits::downlevel_webgl2_defaults().using_resolution(a.limits());
        let Some(img) = crate::gui::gpu_shot::eyes::shot_within(&app.painting(), rect(), weakest) else { return };
        let covered = img.pixels.iter().filter(|p| p.a() > 128).count();
        assert!(covered > 1000, "the weakest device drew {covered} pixels of the cylinder");
    }

    /// HOW MANY TIMES THE BRIGHTNESS CHANGES along the middle row of the body in the picture.
    ///
    /// Flat shading paints a whole facet in one colour, so the row changes once a facet; smooth shading blends across
    /// the triangle and the row changes every few pixels. The number tells the two apart without knowing anything
    /// about the geometry. The widest run of one brightness, measured first, tells the lights apart instead: lit from
    /// in front, the middle of a cylinder is a plateau 20 px wide whatever the normals do.
    fn tone_changes(img: &egui::ColorImage) -> usize {
        let w = img.size[0];
        let row = img.size[1] / 2;
        let lum = |x: usize| {
            let p = img.pixels[row * w + x];
            (p.a() >= 128).then(|| (p.r() as u32 * 2 + p.g() as u32 * 3 + p.b() as u32) / 6)
        };
        (1..w).filter(|&x| matches!((lum(x - 1), lum(x)), (Some(a), Some(b)) if a != b)).count()
    }

    /// SMOOTH SHADING MUST ACTUALLY BE SMOOTH ON THE CARD.
    ///
    /// Reported on an imported engine: pipes and fan blades drawn as polygons, the housing visibly made of
    /// planes - while the software raster drew the same bodies smooth. The vertex normal was carried to the
    /// fragment FLAT, so every triangle took one vertex's normal and the smooth mode shaded facets.
    ///
    /// The check compares the two modes on one body: smooth must change its tone far oftener than flat. Comparing
    /// against a number of pixels alone would pass the moment both modes became flat.
    #[test]
    fn smooth_shading_on_the_card_is_smoother_than_flat() {
        let mut app = App::default();
        cylinder(&mut app);
        fit(&mut app);

        app.set.shading = qymcad_ui_state::Shading::Flat;
        let Some(flat) = crate::gui::gpu_shot::eyes::shot(&app.painting(), rect()) else {
            eprintln!("there is no graphics device here - the card cannot be asked");
            return;
        };
        app.set.shading = qymcad_ui_state::Shading::Smooth;
        let smooth = crate::gui::gpu_shot::eyes::shot(&app.painting(), rect()).expect("the card drew the flat picture but not the smooth one");
        let (fc, sc) = (tone_changes(&flat), tone_changes(&smooth));
        eprintln!("MEASURED the changes of brightness along the row: flat {fc}, smooth {sc}");
        assert!(fc > 4, "even in flat shading the picture came out without steps ({fc} changes) - the check has nothing to compare against");
        assert!(sc > fc * 2, "smooth shading changes its tone {sc} times against {fc} in flat: the vertex normal is not being interpolated");
    }

    /// A CYLINDER IN A WIDE PERSPECTIVE, where the culling is hardest: the ray of sight is its own at every
    /// point of the frame, and near the edges it diverges from the camera axis the most. Two reported
    /// screenshots were about exactly this - holes in a body, a ring broken into ribbons.
    #[test]
    fn the_card_draws_a_cylinder_in_perspective_as_the_raster_does() {
        let mut app = App::default();
        cylinder(&mut app);
        fit(&mut app);
        app.set.projection = qymcad_ui_state::Projection::Perspective;
        app.set.persp_fov_deg = 80.0; // as wide as the setting allows: the divergence is at its largest
        let Some((covered, differ)) = compare(&mut app, "cylinder") else { return };
        let part = differ as f64 / covered.max(1) as f64;
        assert!(part < 0.05, "in perspective the card and the raster disagree on {differ} pixels out of {covered} ({:.1} %) - see target/look/cylinder-*.png", part * 100.0);
    }

    /// EXTRUDED TEXT LOOKS THE SAME ON BOTH PATHS.
    #[test]
    fn the_card_draws_extruded_text_as_the_raster_does() {
        let mut app = App::default();
        extruded_text(&mut app);
        fit(&mut app);
        let Some((covered, differ)) = compare(&mut app, "text") else {
            eprintln!("there is no graphics device here - the card cannot be asked");
            return;
        };
        // THE EDGES ARE ALLOWED TO DIFFER: the card resolves 4 samples per pixel, the raster draws one. On a
        // word of four letters the outline is about 2 % of the covered area; a lost wall is tens of per cent.
        let part = differ as f64 / covered.max(1) as f64;
        eprintln!("MEASURED: {differ} pixels differ out of {covered} covered ({:.1} %)", part * 100.0);
        assert!(part < 0.05, "the card and the raster disagree on {differ} pixels out of {covered} ({:.1} %) - see target/look/text-*.png", part * 100.0);
    }
}
