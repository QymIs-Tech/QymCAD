//! THE PICTURES OF THE HELP: drawn BY THE PROGRAM ITSELF rather than captured from the screen.
//!
//! WHY NOT HAND-TAKEN SCREENSHOTS. A picture taken by hand lives a life of its own: the interface moves,
//! the material changes, a button gets renamed - and the help still carries last year's capture, which only
//! the reader ever notices. Here the pictures are assembled by the same software rasteriser that draws the
//! viewport (`rasterize_3d`), out of REAL geometry built by the same commands. Redrawing all of them is one
//! command, and the difference shows up in `git diff`.
//!
//! To redraw: `cargo test -p qymcad -- --ignored --nocapture help_images`.
#[cfg(test)]
mod tests {
    use super::super::App;
    use egui::{Color32, ColorImage, Rect};

    /// Where they go. The directory is SHARED across languages: a drawing is not translated.
    fn img_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/help/img")
    }

    /// TAKE A VIEW OF A BODY: the camera fits itself to the bounds, the background is TRANSPARENT, and the
    /// edges are smoothed.
    ///
    /// A TRANSPARENT BACKGROUND RATHER THAN THE VIEWPORT COLOUR, because there are two schemes. A picture
    /// with a dark backing looks like a hole in the page of a light theme, and the other way round; a
    /// transparent one lies on either.
    ///
    /// SMOOTHING BY SUPERSAMPLING: the rasteriser cuts triangles by pixels, and on a slanted edge that shows
    /// as a staircase. The shot is taken at twice the size and averaged - four times the work, but it reads
    /// as a drawing rather than as a screenshot from the nineties.
    ///
    /// The camera is fitted here rather than by a number in every scene: bodies differ in size, and a scale
    /// picked by eye lies at the first edit - the part drifts off the edge, and nobody sees it until the
    /// article is opened.
    fn shot(app: &mut App, w: usize, h: usize) -> ColorImage {
        let rect = frame_rect(w, h);
        fit_camera(app, rect);
        shot_as_is(app, w, h)
    }

    /// The frame rectangle including the supersampling.
    fn frame_rect(w: usize, h: usize) -> Rect {
        Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2((w * SS) as f32, (h * SS) as f32))
    }

    /// The supersampling factor.
    const SS: usize = 2;

    /// TAKE A SHOT WITH THE CAMERA ALREADY SET - for the animations.
    fn shot_as_is(app: &mut App, w: usize, h: usize) -> ColorImage {
        let rect = frame_rect(w, h);
        let basis = app.cam.basis();
        let big = app.rasterize_3d(rect, &basis, 1.0, 1.0).expect("the rasteriser returned nothing - there are no visible bodies in the scene");
        downscale(&big, SS)
    }

    /// Average `k` by `k` pixels into one. The colours are mixed PREMULTIPLIED: otherwise the transparent
    /// pixels of the background would drag their own (zero) colour into the edge of the body, and a dark
    /// fringe would appear along the outline.
    fn downscale(img: &ColorImage, k: usize) -> ColorImage {
        let (w, h) = (img.size[0] / k, img.size[1] / k);
        let mut out = ColorImage::new([w, h], Color32::TRANSPARENT);
        for y in 0..h {
            for x in 0..w {
                let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
                for dy in 0..k {
                    for dx in 0..k {
                        let p = img.pixels[(y * k + dy) * img.size[0] + x * k + dx];
                        let pa = p.a() as u32;
                        r += p.r() as u32 * pa;
                        g += p.g() as u32 * pa;
                        b += p.b() as u32 * pa;
                        a += pa;
                    }
                }
                out.pixels[y * w + x] = if a == 0 {
                    Color32::TRANSPARENT
                } else {
                    let n = (k * k) as u32;
                    Color32::from_rgba_unmultiplied((r / a) as u8, (g / a) as u8, (b / a) as u8, (a / n) as u8)
                };
            }
        }
        out
    }

    /// FIT THE CAMERA TO THE BOUNDS OF THE SCENE - in the screen units of the view rather than in world
    /// units.
    ///
    /// What is measured is the spread of the PROJECTIONS of the vertices onto the view axes: for a rotated
    /// part the world bounds and the screen bounds differ, and fitting by the world ones left margins of
    /// either a third of the frame or none at all.
    fn fit_camera(app: &mut App, rect: Rect) {
        use super::super::{v_dot, v_sub};
        let (right, up, _) = app.cam.basis();
        // EXACTLY WHAT WILL BE DRAWN. `project.bodies` also holds CONSUMED bodies - the original plate
        // under a fillet, the original under an array. Fitting by those would stretch the frame around what
        // is invisible, and the part would come out half the size it should be.
        let items: Vec<([f64; 3], [f64; 12])> = app
            .visible_mesh_items()
            .iter()
            .flat_map(|(_, _, _, _, mesh, wt)| mesh.verts.iter().map(|v| ([v.x, v.y, v.z], *wt)).collect::<Vec<_>>())
            .collect();
        assert!(!items.is_empty(), "there is not a single visible vertex in the scene - nothing to shoot");
        let world: Vec<[f64; 3]> = items.iter().map(|(p, wt)| if qymcad_core::feature::is_identity12(wt) { *p } else { qymcad_core::feature::apply12(wt, *p) }).collect();
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for p in &world {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let (mut sx, mut sy) = (0.0f64, 0.0f64);
        for p in &world {
            let rel = v_sub(*p, mid);
            sx = sx.max(v_dot(rel, right).abs());
            sy = sy.max(v_dot(rel, up).abs());
        }
        app.cam.target = mid;
        app.cam.scale = ((rect.width() as f64 / 2.0 / sx.max(1e-6)).min(rect.height() as f64 / 2.0 / sy.max(1e-6)) * 0.86) as f32;
        app.cam.init = true;
    }

    /// Write the PNG. The directory is created here - otherwise the very first redraw on a clean checkout
    /// fails.
    fn save(name: &str, img: &ColorImage) {
        let dir = img_dir();
        std::fs::create_dir_all(&dir).expect("the picture directory");
        let png = App::color_image_to_png(img).expect("encoding the PNG");
        std::fs::write(dir.join(name), png).expect("writing the picture");
    }

    /// AN EMPTY SCENE INSIDE ONE PART.
    ///
    /// Without this the nodes scatter across different components: the first sketch creates a part for
    /// itself, while the ones that follow - a datum, a second sketch, the feature itself - are created in
    /// the root of the assembly or in a NEW part. A regen honestly answers that a reference between
    /// components is not allowed, and the scene comes out empty. That is why the sweep and the loft failed
    /// to build on the first attempt: the commands had nothing to do with it, the fault was in how the
    /// scene was assembled.
    fn in_one_part() -> App {
        let mut app = App::default();
        let part = app.project.add_component("part");
        app.enter_component_for_test(part);
        app
    }

    /// A 40x30x10 plate - the basis of half the scenes.
    fn plate(h: f64) -> App {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = super::super::Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = h;
            p.txt = format!("{h}");
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        app.mode_3d = true;
        app
    }

    /// The body of the last feature.
    fn body_of(app: &App) -> qymcad_core::model::Id {
        app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("there is no body in the scene")
    }

    /// ALL the edges of a body - for fillets and chamfers all the way round.
    fn all_edges(app: &App, body: qymcad_core::model::Id) -> Vec<u32> {
        app.project.regen_edges.get(&body).map(|es| es.iter().map(|e| e.id).collect()).unwrap_or_default()
    }

    /// The top face of a body - the one whose normal points along +Z. That is how it gets picked by mouse
    /// as well.
    fn top_face(app: &App, body: qymcad_core::model::Id) -> qymcad_core::feature::FaceKey {
        let fs = app.project.regen_faces.get(&body).expect("the faces of the body");
        let f = fs.iter().max_by(|a, b| (a.normal[2], a.centroid.z).partial_cmp(&(b.normal[2], b.centroid.z)).expect("comparing faces")).expect("the top face");
        qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
    }

    /// THE NEUTRAL FACE (the bottom) and THE SIDES for a draft.
    ///
    /// A draft tilts the sides while leaving the neutral face still. The bottom as the neutral one is how a
    /// part is taken out of a mould: the bottom lies in the tooling and the walls spread upwards.
    fn draft_faces(app: &App, body: qymcad_core::model::Id) -> (u32, Vec<u32>) {
        let fs = app.project.regen_faces.get(&body).expect("the faces of the body");
        let bottom = fs.iter().min_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).expect("comparing")).expect("the bottom face").id;
        let sides = fs.iter().filter(|f| f.normal[2].abs() < 0.2).map(|f| f.id).collect();
        (bottom, sides)
    }

    /// AN ANIMATION: every frame with ONE camera.
    ///
    /// That is not a matter of presentation but the whole point. Fitting the camera frame by frame means
    /// showing three pictures of the same size in which only the proportions change: an extrusion of 12 mm
    /// looks exactly like one of 1.5 mm. The camera is taken from THE LARGEST frame and does not move -
    /// then what the command actually does is visible: the part grows.
    fn anim(dir: &str, mut apps: Vec<App>) {
        std::fs::create_dir_all(img_dir().join(dir)).expect("the frame directory");
        let rect = frame_rect(640, 400);
        let (mut target, mut scale) = ([0.0; 3], f32::MAX);
        for a in apps.iter_mut() {
            fit_camera(a, rect);
            if a.cam.scale < scale {
                scale = a.cam.scale;
                target = a.cam.target;
            }
        }
        for (i, a) in apps.iter_mut().enumerate() {
            a.cam.scale = scale;
            a.cam.target = target;
            let img = shot_as_is(a, 640, 400);
            save(&format!("{dir}/{i:02}.png"), &img);
        }
    }

    /// A SHOT WITH THE COMMAND OVERLAY: the body is rasterised, and the edges and the tool preview are
    /// drawn on top.
    ///
    /// `rasterize_3d` draws BODIES ONLY. Everything that says what is currently selected - the highlight of
    /// the edges, the preview of the tool - lives in an overlay above the raster, and it never made it into
    /// the shot at all. Because of that the frame showing the boundary edges selected showed the same box
    /// as the frame showing the opening.
    fn shot_cmd(app: &mut App, w: usize, h: usize) -> ColorImage {
        // THE RASTER IS TAKEN AT THE LOGICAL size of the frame, with no supersampling of its own: `shot_ui`
        // already supersamples everything through the pixel density. A private SS broke the alignment of the
        // layers - the overlay was computed for a 640x400 frame while the body was computed for 1280x800,
        // and the picture gained a copy of the box at twice the size (those "lines of foreign geometry").
        let body = {
            let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w as f32, h as f32));
            let basis = app.cam.basis();
            app.rasterize_3d(rect, &basis, 1.0, 1.0).expect("the rasteriser returned nothing")
        };
        // THE BACKGROUND IS THE VIEWPORT COLOUR FROM THE SCHEME. Pure white is not allowed: a guard uses it
        // to catch frames with a lost texture, and filling with white would blind it. A colour of one's own
        // as a number is not allowed either - colours come only from the scheme. Transparent does not work
        // here: the layers are combined by the overlay rasteriser, and over a transparent canvas it leaves
        // black. White on a help page is indistinguishable from the transparency of the neighbouring
        // pictures - while a black frame among light ones is spotted at once.
        let a = &*app;
        super::super::help_raster::shot_ui([w, h], a.scheme.pal.viewport_bg(), |ctx| {
            let tex = ctx.load_texture("body", egui::ImageData::Color(std::sync::Arc::new(body.clone())), egui::TextureOptions::LINEAR);
            egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
                let painter = ui.painter().clone();
                let r = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w as f32, h as f32));
                painter.image(tex.id(), r, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                a.draw_body_edges(&painter, r);
                a.draw_feat_cmd_preview(&painter, r);
            });
        })
    }

    /// Step-by-step frames WITH THE COMMAND OVERLAY - for the tools where WHAT is selected matters.
    fn anim_cmd(dir: &str, mut apps: Vec<App>) {
        std::fs::create_dir_all(img_dir().join(dir)).expect("the frame directory");
        // THE CAMERA IS FITTED TO THE LOGICAL frame - the same one the raster is taken in and the overlay is
        // drawn in. Fitting to the supersampled frame gave a body twice the size of the frame.
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(640.0, 400.0));
        let (mut target, mut scale) = ([0.0; 3], f32::MAX);
        for a in apps.iter_mut() {
            fit_camera(a, rect);
            if a.cam.scale < scale {
                scale = a.cam.scale;
                target = a.cam.target;
            }
        }
        for (i, a) in apps.iter_mut().enumerate() {
            a.cam.scale = scale;
            a.cam.target = target;
            let img = shot_cmd(a, 640, 400);
            save(&format!("{dir}/{i:02}.png"), &img);
        }
    }

    /// A single picture.
    fn still(app: &mut App, name: &str) {
        let img = shot(app, 640, 400);
        save(&format!("{name}.png"), &img);
    }

    /// A SHOT OF A WINDOW OR A PANEL against the canvas.
    ///
    /// The background is the colour of the panels rather than transparency: an `egui` window has no
    /// translucent edges, but the text in it is antialiased, and over a transparent background the fringe of
    /// the letters would smear into dirt.
    fn shot_panel(app: &mut App, w: usize, h: usize, draw: impl Fn(&mut App, &egui::Context)) -> ColorImage {
        let bg = app.scheme.pal.viewport_bg();
        super::super::help_raster::shot_ui([w, h], bg, |ctx| {
            // THE PROGRAM'S THEME GOES TO THE CONTEXT. Without it the windows are drawn in the STOCK `egui`
            // style: the panels our code paints came out exactly right while the windows came out dull, as
            // if under a dimming (reported from the shots of the settings and the keys). A scheme sets not
            // only the palette of the canvas but the look of `egui` itself - and a shot needs it exactly as
            // much as the program does.
            app.apply_theme(ctx);
            draw(app, ctx);
        })
    }

    /// A SKETCH AT A SCALE SET IN ADVANCE - for the animations.
    ///
    /// Fitting the frame to the bounds does not do here: after a trim the geometry gets smaller and the
    /// frame closes in - to the eye it looks as if the line did not shorten but came nearer.
    fn shot_sketch_fixed(app: &mut App, si: usize, scale: f32) -> ColorImage {
        app.mode_3d = false;
        app.project.regen_sketch(si);
        app.view.scale = scale;
        app.view.center = super::super::Vec2::new(0.0, 0.0);
        app.view.initialized = true;
        let bg = app.scheme.pal.viewport_bg();
        let a = &*app;
        super::super::help_raster::shot_ui([640, 400], bg, |ctx| {
            ctx.set_visuals(if a.scheme.pal.light { egui::Visuals::light() } else { egui::Visuals::dark() });
            egui::CentralPanel::default().show(ctx, |ui| {
                let painter = ui.painter().clone();
                let r = ui.available_rect_before_wrap();
                painter.rect_filled(r, 0.0, bg);
                a.draw_contours(&painter, r);
                a.draw_sketch_constraints(&painter, r, si);
                a.draw_sketch_dims(&painter, r, si);
            });
        })
    }

    /// An empty sketch in a part of its own - the basis of the scenes in the sketch section.
    fn empty_sketch() -> (App, usize) {
        let mut app = in_one_part();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        (app, si)
    }

    /// A SHOT OF A SKETCH: the geometry, the constraint glyphs and the dimension captions - by the same
    /// code that draws the sketcher on screen (`draw_contours` plus `draw_sketch_constraints` plus
    /// `draw_sketch_dims`).
    ///
    /// The background is OPAQUE, unlike the pictures of bodies: the canvas of the sketcher is part of the
    /// picture, and without it the page would carry bare lines in a colour meant for a dark canvas.
    fn shot_sketch(app: &mut App, si: usize, w: usize, h: usize) -> ColorImage {
        app.mode_3d = false;
        app.project.regen_sketch(si);
        app.fit(Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w as f32, h as f32)));
        let bg = app.scheme.pal.viewport_bg();
        let a = &*app;
        super::super::help_raster::shot_ui([w, h], bg, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let painter = ui.painter().clone();
                let r = ui.available_rect_before_wrap();
                painter.rect_filled(r, 0.0, bg);
                a.draw_contours(&painter, r);
                a.draw_sketch_constraints(&painter, r, si);
                a.draw_sketch_dims(&painter, r, si);
            });
        })
    }

    /// One frame of a sketch animation (sketches fit their own camera - the scenes are all the same size).
    fn sketch_frame(app: &mut App, si: usize, dir: &str, i: usize) {
        std::fs::create_dir_all(img_dir().join(dir)).expect("the frame directory");
        let img = shot_sketch(app, si, 640, 400);
        save(&format!("{dir}/{i:02}.png"), &img);
    }

    /// A single picture of a sketch.
    fn sketch_still(app: &mut App, si: usize, name: &str) {
        let img = shot_sketch(app, si, 640, 400);
        save(&format!("{name}.png"), &img);
    }

    /// REDRAW THE PICTURES OF THE HELP. Run by hand - a test must not write into the repository on every
    /// run: it would then not be checking anything but changing it.
    ///
    /// THE SCENES ARE BUILT BY REAL COMMANDS rather than drawn as lookalike boxes: a picture must show what
    /// the program actually does. If a fillet starts building differently tomorrow, the redrawn picture
    /// shows it while a hand-drawn one keeps quiet.
    #[test]
    #[ignore = "draws the pictures of the help into docs/help/img"]
    fn help_images() {
        // NOBODY'S HOME PATH GETS INTO A PICTURE. The settings window honestly shows where the settings file
        // lives - and in the shot that was `/home/<name>/...`, that is, a person's name travelling into the
        // repository and onto the website. The data directory is substituted with a neutral one: the shot
        // stays real and carries nothing personal.
        //
        // The variable is process-wide, and that is safe precisely because the test is marked `#[ignore]`
        // and run by a separate command - nothing else runs beside it.
        std::env::set_var("XDG_DATA_HOME", "/home/user/.local/share");
        // EXTRUDE - three frames: the contour rises to its height. The animation shows exactly what the
        // command does and needs not a single word of caption.
        anim("part-extrude", [1.5, 6.0, 12.0].into_iter().map(plate).collect());

        // FILLET - the radius grows. The main point is visible too: a fillet eats material while the
        // overall size stays.
        anim(
            "part-fillet",
            [0.8, 2.5, 5.0]
                .into_iter()
                .map(|r| {
                    let mut app = plate(12.0);
                    let body = body_of(&app);
                    let edges = all_edges(&app, body);
                    app.project.add_fillet(body, r, edges);
                    app.rebuild_if_dirty();
                    app
                })
                .collect(),
        );

        // CHAMFER - next to the fillet, so the difference reads at a glance.
        let mut app = plate(12.0);
        let body = body_of(&app);
        let edges = all_edges(&app, body);
        app.project.add_chamfer(body, 3.0, edges);
        app.rebuild_if_dirty();
        still(&mut app, "part-chamfer");

        // HOLE - a through hole in the top face.
        let mut app = plate(12.0);
        let body = body_of(&app);
        let face = top_face(&app, body);
        app.project.add_hole(body, face, 10.0, 20.0);
        app.rebuild_if_dirty();
        still(&mut app, "part-hole");

        // SHELL - seen from the open side: otherwise the picture shows the same plate and the point of the
        // command is not visible at all. So the camera is taken up and over.
        let mut app = plate(14.0);
        let body = body_of(&app);
        let face = top_face(&app, body);
        let fid = app.project.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| f.id == face.id)).map(|f| f.id).expect("the face of the shell");
        app.project.add_shell(body, 2.0, vec![fid], false);
        app.rebuild_if_dirty();
        app.cam.pitch = 1.05; // looking from above - the walls and the floor are visible
        still(&mut app, "part-shell");

        // LINEAR ARRAY - copies along two directions.
        let mut app = plate(8.0);
        let body = body_of(&app);
        app.project.add_linear_array_grid(body, 55.0, 0.0, 0.0, 3, 0.0, 45.0, 0.0, 2);
        app.rebuild_if_dirty();
        still(&mut app, "part-array-linear");

        // CIRCULAR ARRAY - about the Z axis, so the plate is moved off centre.
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 26.0, -5.0, 14.0, 10.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = super::super::Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 6.0;
            p.txt = "6".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        app.mode_3d = true;
        let body = body_of(&app);
        app.project.add_circular_array(body, 8, 360.0);
        app.rebuild_if_dirty();
        still(&mut app, "part-array-circular");

        // REVOLVE - a profile set ASIDE from the axis gives a ring rather than a disc: that is exactly what
        // the words explain (a profile on the axis gives a solid body, one set aside gives a ring), and the
        // picture must show the case that is needed more often.
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::World(qymcad_core::feature::BasePlane::XZ));
        app.project.add_rect_entity(si, -9.0, 7.0, 18.0, 13.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = super::super::Sel::Sketch(si);
        app.start_feat_cmd(3); // revolve, through the same command a person uses
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "angle") {
            p.val = 270.0;
            p.txt = "270".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        app.mode_3d = true;
        still(&mut app, "part-revolve");

        // PRIMITIVES - a box and a cylinder side by side: one article covers six of them, and the picture
        // must show that these are ready-made bodies rather than the result of a sketch.
        let mut app = App::default();
        app.project.add_box(30.0, 30.0, 20.0);
        app.project.add_cylinder(12.0, 26.0);
        app.rebuild_if_dirty();
        app.mode_3d = true;
        still(&mut app, "part-primitives");

        // DRAFT - three frames: 0, 8 and 16 deg. In a single picture a draft does not read at all, the eye
        // has nothing to compare it with; in motion it shows at once.
        anim(
            "part-draft",
            [0.0, 8.0, 16.0]
                .into_iter()
                .map(|ang| {
                    let mut app = plate(20.0);
                    let body = body_of(&app);
                    let (neutral, sides) = draft_faces(&app, body);
                    if ang > 0.0 {
                        app.project.add_draft(body, sides, neutral, ang, false);
                        app.rebuild_if_dirty();
                    }
                    app
                })
                .collect(),
        );

        // MIRROR - half a part and its reflection. The shape is ASYMMETRIC and stands ASIDE from the plane:
        // a symmetric blank would reflect into itself, and the picture would show exactly the same body,
        // that is, nothing.
        let mut app = plate(10.0);
        let body = body_of(&app);
        let face = top_face(&app, body);
        app.project.add_hole(body, face, 9.0, 20.0);
        app.rebuild_if_dirty();
        let body = body_of(&app);
        app.project.add_mirror(body, 2, true, 0);
        app.rebuild_if_dirty();
        still(&mut app, "part-mirror");




        // PUSH A FACE - three frames: offsets of 0, 6 and 12. A single picture is pointless here: the whole
        // command is that the face MOVES while its neighbours stretch after it.
        anim(
            "part-push-face",
            [0.0, 6.0, 12.0]
                .into_iter()
                .map(|d| {
                    let mut app = plate(10.0);
                    if d > 0.0 {
                        let body = body_of(&app);
                        let face = top_face(&app, body);
                        app.project.add_push_face(body, face, d);
                        app.rebuild_if_dirty();
                    }
                    app
                })
                .collect(),
        );

        // LOFT - a transition between sections: a square at the bottom, a circle at the top. Exactly the
        // case the command exists for, and exactly what words cannot explain.
        let mut app = in_one_part();
        let s0 = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        // THE SQUARE IS CENTRED ON THE ORIGIN rather than starting from it: `add_rect_entity` takes A
        // CORNER, and the circle of the second section (which sits at zero) ended up over a corner of the
        // square rather than over its centre. The loft came out as a wedge - an honest shape, but it
        // explained the command wrongly.
        app.project.add_rect_entity(s0, -15.0, -15.0, 30.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(s0);
        app.finish_sketch_edit();
        let pl = app.project.add_offset_plane(qymcad_core::feature::BasePlane::XY, 34.0);
        let s1 = app.create_sketch_on(qymcad_core::feature::SketchPlane::Datum(pl));
        app.project.add_circle_entity(s1, 0.0, 0.0, 11.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(s1);
        app.finish_sketch_edit();
        let (id0, id1) = (app.project.sketches[s0].id, app.project.sketches[s1].id);
        app.project.add_loft(vec![id0, id1], vec![0, 0], false, 0, 0, false);
        app.rebuild_if_dirty();
        app.mode_3d = true;
        // SLIGHTLY BELOW ISOMETRIC: the whole point of the command is that there is a square BELOW and a
        // circle ABOVE, and both sections must read in one frame.
        app.cam.pitch = 0.5;
        still(&mut app, "part-loft");

        // SWEEP - a section is led along a path. Of all the commands this is the hardest to picture from
        // words: "the profile travels along the path" sounds clear right up to the first attempt.
        let mut app = in_one_part();
        let prof = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_circle_entity(prof, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(prof);
        app.finish_sketch_edit();
        let pid = app.project.sketches[prof].id;
        let path_i = app.create_sketch_on(qymcad_core::feature::SketchPlane::World(qymcad_core::feature::BasePlane::XZ));
        app.project.add_line_entity(path_i, 0.0, 0.0, 0.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.add_line_entity(path_i, 0.0, 30.0, 26.0, 44.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(path_i);
        app.finish_sketch_edit();
        let path_id = app.project.sketches[path_i].id;
        app.project.add_sweep(pid, Vec::new(), path_id, 0);
        app.rebuild_if_dirty();
        app.mode_3d = true;
        still(&mut app, "part-sweep");

        // SKETCHES. What is shot here is not a body but the plane itself: the colour of definedness, the
        // constraint glyphs, the dimension captions. The sketch section is a beginner's first hour, and it
        // is the most expensive thing to convey in words.

        // DIMENSIONS - two frames: the contour without them, and the same contour with dimensions and an
        // anchor.
        //
        // The intent was different - to show the COLOUR change from undefined to defined - but the colour of
        // a contour in the sketcher does not depend on definedness at all (`contour_idle` is one for all of
        // them), and the degrees of freedom live in the status line. The caption states what IS in the
        // picture rather than what was meant to be shot.
        {
            let mut app = App::default();
            let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
            app.project.add_rect_entity(si, -25.0, -18.0, 50.0, 36.0, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(si);
            sketch_frame(&mut app, si, "sketch-dimensions", 0);
            // THE POINTS ARE TAKEN BY COORDINATES rather than by their order in the list. The first version
            // took consecutive points - and the vertical 36 landed on the bottom side, where dy = 0: the
            // dimension became a conflict, the side went red, and the picture taught the exact opposite of
            // what was intended.
            let corner = |app: &App, want: (bool, bool)| -> qymcad_core::model::Id {
                let ps = &app.project.sketches[si].points;
                ps.iter()
                    .min_by(|a, b| {
                        let d = |p: &qymcad_core::model::SketchPoint| (if want.0 { p.x } else { -p.x }) + (if want.1 { p.y } else { -p.y });
                        d(a).partial_cmp(&d(b)).expect("comparing points")
                    })
                    .expect("the corner point")
                    .id
            };
            let (bl, br, tl) = (corner(&app, (true, true)), corner(&app, (false, true)), corner(&app, (true, false)));
            app.project.sketches[si].constraints.push(qymcad_core::model::Constraint::Fixed { p: bl });
            app.project.sketches[si].constraints.push(qymcad_core::model::Constraint::Distance { a: bl, b: br, d: 50.0, off: 0.0, expr: String::new(), driven: false, axis: 1 });
            app.project.sketches[si].constraints.push(qymcad_core::model::Constraint::Distance { a: bl, b: tl, d: 36.0, off: 0.0, expr: String::new(), driven: false, axis: 2 });
            sketch_frame(&mut app, si, "sketch-dimensions", 1);
        }

        // CONSTRAINTS AND DIMENSIONS - a rectangle with the horizontal and vertical glyphs and a circle with
        // a diameter: both are drawn by the sketcher itself rather than by a lookalike picture.
        {
            let mut app = App::default();
            let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
            app.project.add_rect_entity(si, -26.0, -18.0, 52.0, 36.0, qymcad_core::feature::Purpose::Real);
            app.project.add_circle_entity(si, 0.0, 0.0, 9.0, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(si);
            sketch_still(&mut app, si, "sketch-constraints");
        }

        // CONSTRUCTION GEOMETRY - a line that holds the intent but does not go into the body.
        {
            let mut app = App::default();
            let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
            app.project.add_circle_entity(si, -18.0, 0.0, 8.0, qymcad_core::feature::Purpose::Real);
            app.project.add_circle_entity(si, 18.0, 0.0, 8.0, qymcad_core::feature::Purpose::Real);
            app.project.add_line_entity(si, -18.0, 0.0, 18.0, 0.0, qymcad_core::feature::Purpose::Construction); // a centre line: construction
            app.project.regen_sketch(si);
            sketch_still(&mut app, si, "sketch-construction");
        }

        // --- THE SKETCH TOOLS ONE BY ONE ---
        // Each is drawn by the same call the panel button makes; the shape is exactly what comes out by
        // hand rather than something drawn to look similar.

        let (mut app, si) = empty_sketch();
        app.project.add_line_entity(si, -30.0, -14.0, -6.0, 16.0, qymcad_core::feature::Purpose::Real);
        app.project.add_line_entity(si, -6.0, 16.0, 16.0, -6.0, qymcad_core::feature::Purpose::Real);
        app.project.add_line_entity(si, 16.0, -6.0, 32.0, 12.0, qymcad_core::feature::Purpose::Real);
        sketch_still(&mut app, si, "sketch-line");

        let (mut app, si) = empty_sketch();
        app.project.add_rect_entity(si, -28.0, -18.0, 56.0, 36.0, qymcad_core::feature::Purpose::Real);
        sketch_still(&mut app, si, "sketch-rect");

        let (mut app, si) = empty_sketch();
        app.project.add_circle_entity(si, 0.0, 0.0, 20.0, qymcad_core::feature::Purpose::Real);
        sketch_still(&mut app, si, "sketch-circle");

        let (mut app, si) = empty_sketch();
        app.project.add_arc_entity(si, 0.0, -8.0, -22.0, -8.0, 22.0, -8.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real);
        sketch_still(&mut app, si, "sketch-arc");

        let (mut app, si) = empty_sketch();
        app.project.add_spline(
            si,
            vec![
                qymcad_core::geom::Point2::new(-30.0, -6.0),
                qymcad_core::geom::Point2::new(-12.0, 16.0),
                qymcad_core::geom::Point2::new(8.0, -14.0),
                qymcad_core::geom::Point2::new(30.0, 8.0),
            ],
            qymcad_core::feature::Ends::Open,
            qymcad_core::feature::Purpose::Real,
        );
        sketch_still(&mut app, si, "sketch-spline");

        // TRIM - two frames: crossing lines, and the same lines after the cut. The scale is FIXED: after the
        // cut the bounds are smaller, and fitting the frame would read as the line coming nearer rather than
        // getting shorter.
        {
            let (mut app, si) = empty_sketch();
            app.project.add_line_entity(si, -30.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
            let cut = app.project.add_line_entity(si, 10.0, -20.0, 10.0, 20.0, qymcad_core::feature::Purpose::Real);
            let _ = cut;
            std::fs::create_dir_all(img_dir().join("sketch-trim")).expect("the frame directory");
            save("sketch-trim/00.png", &shot_sketch_fixed(&mut app, si, 6.0));
            let ids: Vec<qymcad_core::model::Id> = app.project.sketches[si].entities.iter().map(|e| e.id).collect();
            if let Some(first) = ids.first() {
                app.project.trim_line(si, *first, 25.0, 0.0); // clicked on the tail to the right of the crossing
            }
            save("sketch-trim/01.png", &shot_sketch_fixed(&mut app, si, 6.0));
        }

        // OFFSET - a contour and its copy at a distance. Two frames again: otherwise it is not visible what
        // appeared and what was already there.
        {
            let (mut app, si) = empty_sketch();
            app.project.add_rect_entity(si, -24.0, -15.0, 48.0, 30.0, qymcad_core::feature::Purpose::Real);
            std::fs::create_dir_all(img_dir().join("sketch-offset")).expect("the frame directory");
            save("sketch-offset/00.png", &shot_sketch_fixed(&mut app, si, 5.5));
            let ids: Vec<qymcad_core::model::Id> = app.project.sketches[si].entities.iter().map(|e| e.id).collect();
            app.project.offset_entities(si, &ids, 6.0);
            save("sketch-offset/01.png", &shot_sketch_fixed(&mut app, si, 5.5));
        }

        // MIRROR - what is shot here is THE RESULT rather than the working of the command: the command
        // itself is interactive (pick the entities, then the axis). The geometry is the same as what comes
        // out by hand, and that is honest; but if the command changes, this picture will not say so by
        // itself, unlike the rest.
        {
            let (mut app, si) = empty_sketch();
            for s in [-1.0, 1.0] {
                app.project.add_line_entity(si, s * 8.0, -16.0, s * 8.0, 6.0, qymcad_core::feature::Purpose::Real);
                app.project.add_line_entity(si, s * 8.0, 6.0, s * 26.0, 16.0, qymcad_core::feature::Purpose::Real);
            }
            app.project.add_line_entity(si, 0.0, -22.0, 0.0, 22.0, qymcad_core::feature::Purpose::Construction); // the axis of symmetry is construction geometry
            sketch_still(&mut app, si, "sketch-mirror");
        }

        // --- THE REMAINING PART COMMANDS ---

        // THREAD - a real one, cut by the kernel, not a drawn helix. It is built along the same path the
        // program takes: a cylinder as a primitive, the round rim as an edge, the thread command.
        {
            let mut app = App::default();
            app.start_prim_cmd(11);
            for (k, v) in [("r", 15.0), ("h", 60.0)] {
                if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == k) {
                    p.val = v;
                    p.txt = format!("{v}");
                }
            }
            app.apply_feat_cmd();
            let consumed = app.consumed_bodies();
            let body = *app.live.shapes.keys().find(|b| !consumed.contains(b)).expect("the body of the shaft");
            let eid = app.project.regen_edges.get(&body).and_then(|es| es.iter().find(|e| (e.radius - 15.0).abs() < 0.05).map(|e| e.id)).expect("the round rim of the shaft");
            app.select_body(body);
            app.start_thread_cmd();
            app.set_thread_params();
            app.thread.src = Some(body);
            app.thread.edge = eid;
            app.thread.radius = 15.0;
            app.thread.axis = ([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
            for (k, v) in [("nominal", 30.0), ("pitch", 3.5), ("length", 40.0), ("fit", 0.0), ("lead_in", 0.0), ("lead_out", 0.0)] {
                if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == k) {
                    p.val = v;
                    p.txt = format!("{v}");
                }
            }
            app.apply_feat_cmd();
            app.rebuild_if_dirty();
            app.mode_3d = true;
            still(&mut app, "part-thread");
        }


        // A PATCH STEP BY STEP: an open box -> the selected edges -> the surface stretched over them.
        //
        // The frames are made by the same machinery as the rest of the help: a picture must be rebuilt along
        // with the code, or sooner or later it shows something the program no longer has.
        {
            let mut frames: Vec<App> = Vec::new();
            let open_box = || -> App {
                let mut app = plate(20.0);
                let body = body_of(&app);
                let top: Vec<u32> = app.project.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
                app.project.add_shell_mode(body, 2.0, top, qymcad_core::feature::ShellSide::Inward);
                app.rebuild_if_dirty();
                app
            };
            let rim = |app: &App| -> Vec<u32> {
                let b = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the shell");
                let zmax = app.project.regen_edges[&b].iter().flat_map(|e| [e.a[2], e.b[2]]).fold(f64::MIN, f64::max);
                let (mut x0, mut x1) = (f64::MAX, f64::MIN);
                for e in &app.project.regen_edges[&b] {
                    for q in [e.a, e.b] {
                        x0 = x0.min(q[0]);
                        x1 = x1.max(q[0]);
                    }
                }
                app.project.regen_edges[&b]
                    .iter()
                    .filter(|e| (e.a[2] - zmax).abs() < 1e-6 && (e.b[2] - zmax).abs() < 1e-6)
                    .filter(|e| [e.a, e.b].iter().all(|q| (q[0] - x0).abs() > 1e-6 && (q[0] - x1).abs() > 1e-6))
                    .map(|e| e.id)
                    .collect()
            };
            // 1. the opening
            frames.push(open_box());
            // 2. the boundary edges selected - what is on screen just before Enter
            let mut a2 = open_box();
            a2.mode_3d = true;
            // THE BODY IS SELECTED EXPLICITLY: the edges are prepared only for the body being worked on, and
            // outside a live window there is nowhere to get it from - without this the "what is selected"
            // frame would show the same box.
            if let Some(mi) = a2.project.timeline.iter().rev().find_map(|n| n.kind.body()).and_then(|b| a2.project.mesh_index(b)) {
                a2.sel = super::super::Sel::Mesh(mi);
            }
            a2.start_feat_cmd(32);
            a2.refresh_edges();
            for id in rim(&a2) {
                a2.gsel.edges.insert(id);
            }
            frames.push(a2);
            // 3. Enter - the surface is stretched over
            let mut a3 = open_box();
            let b3 = a3.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the shell");
            let picks = rim(&a3);
            a3.project.add_patch(b3, qymcad_core::refs::Ref::picks(&picks), false);
            a3.rebuild_if_dirty();
            frames.push(a3);
            anim_cmd("part-patch", frames);
        }

        // WHAT A PATCH IS FOR - THE WHOLE PATH: an open box -> a patch -> thickness -> a union.
        //
        // A patch on its own is useless, and that is visible: a surface can be neither added to a part nor
        // printed. The point appears on the third frame, when it has thickness, and on the fourth, when it
        // has become part of the body. Showing the tool apart from this path means showing half of it.
        {
            let build = |steps: usize| -> App {
                let mut app = plate(20.0);
                let body = body_of(&app);
                let top: Vec<u32> = app.project.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
                let shell = app.project.add_shell_mode(body, 2.0, top, qymcad_core::feature::ShellSide::Inward);
                app.rebuild_if_dirty();
                if steps == 0 {
                    return app;
                }
                // the edges of the opening, along the inner contour
                let zmax = app.project.regen_edges[&shell].iter().flat_map(|e| [e.a[2], e.b[2]]).fold(f64::MIN, f64::max);
                let (mut x0, mut x1) = (f64::MAX, f64::MIN);
                for e in &app.project.regen_edges[&shell] {
                    for q in [e.a, e.b] {
                        x0 = x0.min(q[0]);
                        x1 = x1.max(q[0]);
                    }
                }
                let rim: Vec<u32> = app.project.regen_edges[&shell]
                    .iter()
                    .filter(|e| (e.a[2] - zmax).abs() < 1e-6 && (e.b[2] - zmax).abs() < 1e-6)
                    .filter(|e| [e.a, e.b].iter().all(|q| (q[0] - x0).abs() > 1e-6 && (q[0] - x1).abs() > 1e-6))
                    .map(|e| e.id)
                    .collect();
                let patch = app.project.add_patch(shell, qymcad_core::refs::Ref::picks(&rim), false);
                app.rebuild_if_dirty();
                if steps == 1 {
                    return app;
                }
                let face = app.project.regen_faces[&patch].first().map(|f| f.id).expect("the face of the patch");
                let lid = app.project.add_thicken(patch, face, 2.0);
                app.rebuild_if_dirty();
                if steps == 2 {
                    return app;
                }
                app.project.add_body_boolean(shell, lid, 1); // 1 = union
                app.rebuild_if_dirty();
                app
            };
            anim_cmd("part-surface-flow", (0..4).map(build).collect());
        }

        // THICKEN - a face is turned into a body of a given thickness.
        {
            let mut app = plate(6.0);
            let body = body_of(&app);
            let face = top_face(&app, body);
            app.project.add_thicken(body, face.id, 10.0);
            app.rebuild_if_dirty();
            still(&mut app, "part-thicken");
        }

        // SPLIT A BODY - in two by a plane, with one half moved aside.
        //
        // Moving it aside IS NECESSARY: after the cut the halves butt together and take one colour, and the
        // shot shows the same bar. The shift is not part of the command but a way to show its result; in the
        // document it lives as a separate Move feature, exactly as it would by hand.
        //
        // On the first attempt Move looked as if it were moving the wrong thing. It was moving exactly the
        // right thing: a shift of 16 mm with a piece 15 wide simply lays one half over the other almost
        // entirely. The lesson is not about the API: "it did not work" must be checked with numbers rather
        // than by eye on a single picture.
        {
            let mut app = plate(18.0);
            let body = body_of(&app);
            // the cut goes through THE MIDDLE: the plate spans 0 to 30 in Y, and a cut at zero would run
            // along its edge
            let pieces = app.project.add_split_body(body, 1, 0, 15.0, 2);
            app.rebuild_if_dirty();
            if let Some(&half) = pieces.first() {
                app.project.add_move(half, [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 40.0, 0.0, 0.0, 1.0, 0.0]);
                app.rebuild_if_dirty();
            }
            still(&mut app, "part-split-body");
        }

        // REMOVE A FACE - TWO FRAMES: a plate with a hole, and the same plate without it.
        //
        // A single picture is pointless here: it shows just a flat plate, and there is no way to tell that a
        // face WAS there. The frames are taken with one camera - the body does not change size, and fitting
        // the frame would lie with the scale.
        //
        // On the first attempt the shot came out WITH the hole: the face was selected by `|x| < 12`, while
        // the plate spans 0 to 40 and its middle is at 20. The filter found nothing, the command received an
        // empty list and honestly did nothing. Now an empty list fails the scene rather than producing a
        // quiet "nothing happened" picture.
        {
            std::fs::create_dir_all(img_dir().join("part-remove-face")).expect("the frame directory");
            let mut app = plate(12.0);
            let body = body_of(&app);
            let face = top_face(&app, body);
            app.project.add_hole(body, face, 14.0, 20.0);
            app.rebuild_if_dirty();
            let rect = frame_rect(640, 400);
            fit_camera(&mut app, rect);
            let (scale, target) = (app.cam.scale, app.cam.target);
            save("part-remove-face/00.png", &shot_as_is(&mut app, 640, 400));

            let body = body_of(&app);
            let cyl: Vec<qymcad_core::feature::FaceKey> = app
                .project
                .regen_faces
                .get(&body)
                .map(|fs| {
                    fs.iter()
                        .filter(|f| f.normal[2].abs() < 0.2 && (f.centroid.x - 20.0).abs() < 10.0 && (f.centroid.y - 15.0).abs() < 10.0)
                        .map(|f| qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
                        .collect()
                })
                .unwrap_or_default();
            assert!(!cyl.is_empty(), "the face of the hole was not found - the scene would show a plate WITH a hole");
            app.project.add_remove_face(body, cyl);
            app.rebuild_if_dirty();
            app.cam.scale = scale;
            app.cam.target = target;
            save("part-remove-face/01.png", &shot_as_is(&mut app, 640, 400));
        }


        // --- THE WHOLE PROGRAM WINDOW ---
        //
        // The picture a beginner needs most, and the one the frame rasteriser was written for: "tools on the
        // left, the path on top, properties on the right, the state below" takes twice as long to read as to
        // see. It is drawn with THE REAL panels of the program, in their own order.
        //
        // The viewport uses the software rasteriser (`gpu_viewport = false`): the GPU path draws through a
        // paint callback, which does not and cannot exist in a test run.
        {
            let mut app = plate(12.0);
            let body = body_of(&app);
            let edges = all_edges(&app, body);
            app.project.add_fillet(body, 3.0, edges.into_iter().take(4).collect());
            app.rebuild_if_dirty();
            app.set.gpu_viewport = false;
            app.mode_3d = true;
            app.cam.init = true;
            app.cam.scale = 7.0;
            app.cam.target = [20.0, 15.0, 6.0];
            let bg = app.scheme.pal.viewport_bg();
            let img = {
                let a = &mut app;
                super::super::help_raster::shot_ui([1100, 690], bg, |ctx| {
                    a.apply_theme(ctx);
                    a.menu_bar(ctx);
                    a.toolbar(ctx);
                    a.wb_toolbar(ctx);
                    a.tree_panel(ctx);
                    a.properties_panel(ctx);
                    a.viewport(ctx);
                })
            };
            save("window.png", &img);
        }

        // --- ASSEMBLIES ---
        //
        // Built the way it is built by hand: a root assembly holding two PARTS, each with a body of its own,
        // both placed by their own matrices. The components take different colours - and that is the main
        // thing to see: an assembly is not one body of a complicated shape but several parts side by side.

        /// A part component holding a plate of a given size, placed where it belongs.
        fn part_with_plate(app: &mut App, name: &str, w: f64, d: f64, h: f64, at: [f64; 3]) {
            let cid = app.project.add_component(name);
            app.enter_component_for_test(cid);
            let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
            app.project.add_rect_entity(si, 0.0, 0.0, w, d, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(si);
            app.finish_sketch_edit();
            app.sel = super::super::Sel::Sketch(si);
            app.start_feat_cmd(1);
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = h;
                p.txt = format!("{h}");
            }
            app.apply_feat_cmd();
            app.rebuild_if_dirty();
            app.project.set_component_transform(cid, [1.0, 0.0, 0.0, at[0], 0.0, 1.0, 0.0, at[1], 0.0, 0.0, 1.0, at[2]]);
            app.exit_context();
        }

        {
            let mut app = App::default();
            part_with_plate(&mut app, "base", 70.0, 50.0, 10.0, [0.0, 0.0, 0.0]);
            part_with_plate(&mut app, "post", 18.0, 18.0, 40.0, [26.0, 16.0, 10.0]);
            app.rebuild_if_dirty();
            app.mode_3d = true;
            still(&mut app, "assembly-components");
        }

        // A COMPONENT ARRAY - not copies made by hand but one feature: four posts at a 22 mm step.
        {
            let mut app = App::default();
            part_with_plate(&mut app, "base", 100.0, 40.0, 8.0, [0.0, 0.0, 0.0]);
            let post = app.project.components.len();
            part_with_plate(&mut app, "post", 14.0, 14.0, 30.0, [10.0, 13.0, 8.0]);
            let pid = app.project.components.get(post).map(|c| c.id).expect("the post component");
            app.project.add_comp_pattern(pid, qymcad_core::model::CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 22.0, count: 4 });
            app.rebuild_if_dirty();
            app.mode_3d = true;
            still(&mut app, "assembly-array");
        }

        // THE INTERFERENCE CHECK HAS NO PICTURE, for the same reason as splitting a body: the interference
        // ITSELF IS NOT VISIBLE. A post sunk 8 mm into a plate looks exactly like a post standing on a
        // plate - the whole point is that the eye does not catch it. What has to be shown here is the report
        // of the check, that is, a shot of the WINDOW with the panel open; that is a separate piece of work.


        // A POLYGON, A SLOT AND AN ELLIPSE - FLAT.
        //
        // At first they were shot EXTRUDED: there was no 2D shot back then, and the reasoning was that a
        // flat contour in a picture is indistinguishable from a bunch of lines. That was corrected: in the
        // sketch section a picture must show a sketch. The excuse fell away along with the arrival of the
        // plane shot - a line, a circle and an arc read perfectly well on it.
        let (mut app, si) = empty_sketch();
        app.project.add_polygon_entity(si, 0.0, 0.0, 22.0, 0.0, 6, qymcad_core::feature::Purpose::Real);
        sketch_still(&mut app, si, "sketch-polygon");

        let (mut app, si) = empty_sketch();
        app.project.add_slot_entity(si, -16.0, 0.0, 16.0, 0.0, 9.0, qymcad_core::feature::Purpose::Real);
        sketch_still(&mut app, si, "sketch-slot");

        let (mut app, si) = empty_sketch();
        app.project.add_ellipse_entity(si, 0.0, 0.0, 26.0, 14.0, 0.0, qymcad_core::feature::Purpose::Real);
        sketch_still(&mut app, si, "sketch-ellipse");

        // EXTEND - two frames: a line that stops short of its neighbour, and the same line reaching it.
        {
            let (mut app, si) = empty_sketch();
            app.project.add_line_entity(si, -6.0, -20.0, -6.0, 20.0, qymcad_core::feature::Purpose::Real);
            let short = app.project.add_line_entity(si, -30.0, 0.0, -16.0, 0.0, qymcad_core::feature::Purpose::Real);
            std::fs::create_dir_all(img_dir().join("sketch-extend")).expect("the frame directory");
            save("sketch-extend/00.png", &shot_sketch_fixed(&mut app, si, 6.5));
            app.project.extend_line(si, short, -16.0, 0.0); // pulled by the right-hand end
            save("sketch-extend/01.png", &shot_sketch_fixed(&mut app, si, 6.5));
        }

        // BREAK HAS NO PICTURE. It is not visible by itself: the line stays exactly where it was, it merely
        // becomes two. Pulling the halves apart was tried - they stay tied at the break point (removing the
        // coincidences is not enough), and the shot came out as a tent made of one polyline: the picture
        // would show a bend rather than a break. To come back here, first work out what exactly ties the
        // ends together after the cut.


        // --- THE GENERAL SECTION: shots of the real panels and windows ---
        //
        // What is needed here is not geometry but the interface: the tree, the parameters window, the
        // settings, the key reference. Words explain this most slowly of all, while drawing it takes the
        // same `egui` frame.

        // THE BUILD TREE with a real timeline: a sketch, an extrude, a fillet, a hole.
        {
            let mut app = plate(14.0);
            let body = body_of(&app);
            let edges = all_edges(&app, body);
            app.project.add_fillet(body, 3.0, edges.into_iter().take(4).collect());
            app.rebuild_if_dirty();
            let body = body_of(&app);
            let face = top_face(&app, body);
            app.project.add_hole(body, face, 10.0, 20.0);
            app.rebuild_if_dirty();
            // INSIDE THE PART: from outside, the tree shows the makeup of the assembly (the origin, the
            // components, the first part), while the build timeline lives INSIDE a part. The article is
            // about the timeline, so the shot must come from where the timeline is visible.
            let part = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
            app.enter_component_for_test(part);
            let img = shot_panel(&mut app, 300, 420, |a, ctx| a.tree_panel(ctx));
            save("tree.png", &img);
        }

        // PARAMETERS AND FORMULAS - with one parameter referring to another: the very thing they exist for.
        {
            let mut app = plate(10.0);
            app.project.parameters = vec![
                qymcad_core::model::Param { name: "w".into(), expr: "60".into(), value: 60.0 },
                qymcad_core::model::Param { name: "h".into(), expr: "w/2".into(), value: 30.0 },
                qymcad_core::model::Param { name: "wall".into(), expr: "3".into(), value: 3.0 },
                qymcad_core::model::Param { name: "d".into(), expr: "w/6 + wall".into(), value: 13.0 },
            ];
            app.win.params = true;
            let img = shot_panel(&mut app, 560, 320, |a, ctx| a.params_window(ctx));
            save("params.png", &img);
        }

        // THE SETTINGS - with the sections on the left and the search above them.
        {
            let mut app = App::default();
            app.win.settings = true;
            let img = shot_panel(&mut app, 900, 560, |a, ctx| a.settings_window(ctx));
            save("settings.png", &img);
        }

        // A SCHEME THAT PAINTS THE WHOLE PROGRAM - a shot of THE WHOLE WINDOW rather than of a panel.
        //
        // A panel would show that the scheme paints a panel, and nothing more: the point of such a scheme is
        // that ALL the surfaces come together at once - the scene, the tree, the properties, the buttons.
        // One panel cannot tell that.
        for id in ["dracula", "alucard"] {
            let mut app = App::default();
            app.set.scheme = id.into();
            app.set.gpu_viewport = false;
            let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
            app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(si);
            app.finish_sketch_edit();
            app.sel = super::super::Sel::Sketch(si);
            app.start_feat_cmd(1);
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = 14.0;
                p.txt = "14".into();
            }
            app.apply_feat_cmd();
            app.rebuild_if_dirty();
            app.mode_3d = true;
            // THE CAMERA IS NOT FITTED: `fit_camera` computes against the rectangle of THE FRAME, while here
            // the frame is the whole window, of which the viewport gets only the strip between the panels.
            // The first version did fit it, and the part came out flush against the edges, cropped. The
            // stock camera shows it whole - and that is exactly how it looks on opening the program.
            let bg = app.scheme.pal.viewport_bg();
            let img = {
                let a = &mut app;
                super::super::help_raster::shot_ui([1200, 700], bg, |ctx| {
                    a.apply_theme(ctx);
                    a.menu_bar(ctx);
                    a.toolbar(ctx);
                    a.wb_toolbar(ctx);
                    a.tree_panel(ctx);
                    a.properties_panel(ctx);
                    a.viewport(ctx);
                })
            };
            save(&format!("scheme-{id}.png"), &img);
        }

        // THE HOTKEYS - the whole reference, the same one assembled from a single source.
        {
            let mut app = App::default();
            app.win.hotkeys = true;
            let img = shot_panel(&mut app, 720, 640, |a, ctx| a.hotkeys_window(ctx));
            save("hotkeys.png", &img);
        }

        // THE DOCUMENT PROPERTIES - what travels along with the file.
        {
            let mut app = plate(10.0);
            app.win.doc_props = true;
            let img = shot_panel(&mut app, 640, 460, |a, ctx| a.doc_props_window(ctx));
            save("doc-props.png", &img);
        }

        // THE WHOLE VIEWPORT - the view cube, the axes, the grid. The article about the viewport explains
        // exactly those.
        //
        // It was meant to be "a datum plane above a part", but the plane did not make it into the frame: it
        // is not always drawn, and working out under what conditions is a separate piece of work. The shot
        // is left as what it IS, and the article about datums is left without a picture.
        {
            let mut app = plate(12.0);
            app.set.gpu_viewport = false;
            app.mode_3d = true;
            app.cam.init = true;
            app.cam.scale = 5.0;
            app.cam.target = [20.0, 15.0, 6.0];
            let bg = app.scheme.pal.viewport_bg();
            let img = {
                let a = &mut app;
                super::super::help_raster::shot_ui([640, 420], bg, |ctx| {
                    a.apply_theme(ctx);
                    a.viewport(ctx);
                })
            };
            save("viewport.png", &img);
        }

        // A JOINT - the joint glyphs on the bodies. Built the way the program builds it: connectors on the
        // components, then the joint itself; the plate is grounded, the post is driven by the joint.
        {
            let mut app = App::default();
            part_with_plate(&mut app, "base", 70.0, 50.0, 10.0, [0.0, 0.0, 0.0]);
            part_with_plate(&mut app, "post", 16.0, 16.0, 36.0, [27.0, 17.0, 10.0]);
            app.rebuild_if_dirty();
            let comps: Vec<qymcad_core::model::Id> = app.project.components.iter().filter(|c| c.parent.is_some() && c.parent != Some(app.project.root)).map(|c| c.id).collect();
            let parts: Vec<qymcad_core::model::Id> = app.project.components.iter().filter(|c| c.name == "base" || c.name == "post").map(|c| c.id).collect();
            let _ = comps;
            if parts.len() >= 2 {
                app.project.set_grounded(parts[0], true);
                let ca = app.project.add_connector(parts[0], qymcad_core::feature::AnchorRef::Origin);
                let cb = app.project.add_connector(parts[1], qymcad_core::feature::AnchorRef::Origin);
                app.project.add_joint(ca, cb, qymcad_core::feature::JointKind::Revolute);
                app.project.solve_joints();
            }
            app.set.show_joints = true;
            app.rebuild_if_dirty();
            app.set.gpu_viewport = false;
            app.mode_3d = true;
            app.cam.init = true;
            app.cam.scale = 5.5;
            app.cam.target = [35.0, 25.0, 14.0];
            let bg = app.scheme.pal.viewport_bg();
            let img = {
                let a = &mut app;
                super::super::help_raster::shot_ui([640, 420], bg, |ctx| {
                    a.apply_theme(ctx);
                    a.viewport(ctx);
                })
            };
            save("assembly-joint.png", &img);
        }

        // DRIVING A JOINT - three positions of ONE mechanism. The point of a joint is that a part MOVES by a
        // rule rather than standing still; a single picture will never show that.
        //
        // One camera serves every frame and is set by hand: fitting it to the bounds is not possible here -
        // the arm changes the bounds of the scene as it turns, and the frame would breathe after it.
        //
        // THE ANCHOR IS A FACE, NOT AN ORIGIN. The first version took `AnchorRef::Origin` on both parts, and
        // the joint honestly mated their ZEROES: the zero of the plate lies in its bottom corner, and the
        // arm ended up INSIDE the base - on the animation the parts passed through one another. That was
        // reported as looking broken, and rightly: the picture taught the wrong thing.
        //
        // The joint itself was working correctly - the scenario was lying. Nobody works that way by hand:
        // the TOP face of the base and the BOTTOM face of the arm are taken, exactly as here.
        {
            use qymcad_core::feature::{AnchorRef, FaceKey};
            /// The face of the body of part `comp` whose normal points along `dir` - the very one that would
            /// be clicked by mouse. The key is assembled from the live face, so resolving finds exactly it.
            fn face_towards(p: &qymcad_core::model::Project, comp: qymcad_core::model::Id, dir: [f64; 3]) -> Option<(qymcad_core::model::Id, FaceKey)> {
                let body = p.bodies.iter().find(|b| p.body_owner(b.id) == Some(comp))?;
                let (i, f) = body
                    .faces
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| {
                        let dot = |f: &qymcad_core::geom::MeshFace| f.normal[0] * dir[0] + f.normal[1] * dir[1] + f.normal[2] * dir[2];
                        dot(a).partial_cmp(&dot(b)).unwrap_or(std::cmp::Ordering::Equal)
                    })?;
                Some((body.id, FaceKey { index: i as u32, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }))
            }
            /// The bounds of a body IN THE WORLD - the only way to check IN NUMBERS whether the parts sit
            /// inside one another. By eye, on a single picture, that is exactly what was missed last time.
            fn world_aabb(p: &qymcad_core::model::Project, comp: qymcad_core::model::Id) -> Option<([f64; 3], [f64; 3])> {
                let body = p.bodies.iter().find(|b| p.body_owner(b.id) == Some(comp))?;
                let m = p.body_world_transform(body.id);
                let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
                for v in &body.mesh.verts {
                    let w = qymcad_core::feature::apply12(&m, [v.x, v.y, v.z]);
                    for k in 0..3 {
                        lo[k] = lo[k].min(w[k]);
                        hi[k] = hi[k].max(w[k]);
                    }
                }
                Some((lo, hi))
            }

            std::fs::create_dir_all(img_dir().join("assembly-drive")).expect("the frame directory");
            let mut tips: Vec<[f64; 3]> = Vec::new();
            for (i, ang) in [0.0, 40.0, 80.0].into_iter().enumerate() {
                let mut app = App::default();
                // THE BASE IS SMALL, THE ARM IS LONG. The first version took a 70x50 plate - it covered the
                // arm with itself, and a turn of 80 deg read as nothing having changed.
                //
                // The base is RECTANGULAR rather than square: a square top face has no long side, and the
                // secondary axis of the joint would be derived from a degenerate direction.
                part_with_plate(&mut app, "base", 26.0, 20.0, 8.0, [-13.0, -10.0, -8.0]);
                part_with_plate(&mut app, "arm", 62.0, 10.0, 6.0, [0.0, -5.0, 0.0]);
                app.rebuild_if_dirty();
                let parts: Vec<qymcad_core::model::Id> = app.project.components.iter().filter(|c| c.name == "base" || c.name == "arm").map(|c| c.id).collect();
                assert!(parts.len() >= 2, "the drive scene did not assemble: {} parts", parts.len());
                app.project.set_grounded(parts[0], true);
                let top = face_towards(&app.project, parts[0], [0.0, 0.0, 1.0]).expect("the top face of the base");
                let bottom = face_towards(&app.project, parts[1], [0.0, 0.0, -1.0]).expect("the bottom face of the arm");
                let ca = app.project.add_connector(parts[0], AnchorRef::FaceCenter(top.0, top.1));
                let cb = app.project.add_connector(parts[1], AnchorRef::FaceCenter(bottom.0, bottom.1));
                // FACE TO FACE - the normals meet, so the arm turns over: without this it would lie body
                // DOWN, back inside the base.
                if let Some(c) = app.project.connectors.iter_mut().find(|c| c.id == cb) {
                    c.flip = true;
                }
                let jid = app.project.add_joint(ca, cb, qymcad_core::feature::JointKind::Revolute);
                if let Some(j) = app.project.joints.iter_mut().find(|j| j.id == jid) {
                    j.drive[0] = Some(ang); // a driver on the angle slot: hold exactly this
                }
                app.project.solve_joints();

                // CHECKED IN NUMBERS, NOT BY EYE. Exactly the defect that made it into the help: the parts
                // intersected, and on a single frame that read as if it were intended.
                let (blo, bhi) = world_aabb(&app.project, parts[0]).expect("the bounds of the base");
                let (alo, ahi) = world_aabb(&app.project, parts[1]).expect("the bounds of the arm");
                let over = (0..3).all(|k| alo[k] < bhi[k] - 1e-6 && blo[k] < ahi[k] - 1e-6);
                assert!(!over, "at {ang} deg the arm sits inside the base: arm {alo:?}..{ahi:?}, base {blo:?}..{bhi:?}");
                tips.push(ahi);

                app.set.show_joints = true;
                app.rebuild_if_dirty();
                app.set.gpu_viewport = false;
                app.mode_3d = true;
                app.cam.init = true;
                app.cam.scale = 5.2;
                app.cam.target = [0.0, 0.0, 0.0];
                let bg = app.scheme.pal.viewport_bg();
                let img = {
                    let a = &mut app;
                    super::super::help_raster::shot_ui([640, 420], bg, |ctx| {
                        a.apply_theme(ctx);
                        a.viewport(ctx);
                    })
                };
                save(&format!("assembly-drive/{i:02}.png"), &img);
            }
            // AND THE ARM REALLY MOVES. Three identical frames are not an animation but a deception: the
            // point of a driver is that the part stands where the number says, and the frame must show it.
            for k in 1..tips.len() {
                let d = (0..3).map(|c| (tips[k][c] - tips[k - 1][c]).powi(2)).sum::<f64>().sqrt();
                assert!(d > 5.0, "frames {} and {k} are nearly identical (a shift of {d:.1} mm) - the animation shows nothing", k - 1);
            }
        }

        // THE INTERFERENCE CHECK - a shot of THE TREE rather than of the scene. The interference itself is
        // not visible: a post sunk 6 mm into a plate looks like a post standing on a plate - and that is the
        // whole point of the command. What has to be shown is what it SAYS: the red line counting the
        // interferences.
        {
            let mut app = App::default();
            part_with_plate(&mut app, "base", 60.0, 40.0, 12.0, [0.0, 0.0, 0.0]);
            part_with_plate(&mut app, "post", 16.0, 16.0, 34.0, [22.0, 12.0, 6.0]);
            app.rebuild_if_dirty();
            app.set.show_interference = true;
            app.refresh_interference();
            let img = shot_panel(&mut app, 320, 300, |a, ctx| a.tree_panel(ctx));
            save("interference.png", &img);
        }

        // A CORNER FILLET IN A SKETCH - two frames: a sharp corner, and the same corner rounded.
        //
        // A single picture would show merely a rounded rectangle, and what the command did could not be read
        // out of it. The scale is fixed: the bounds do not change, and fitting the frame would add motion
        // between the frames for nothing.
        {
            let (mut app, si) = empty_sketch();
            app.project.add_rect_entity(si, -26.0, -17.0, 52.0, 34.0, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(si);
            std::fs::create_dir_all(img_dir().join("sketch-corner")).expect("the frame directory");
            save("sketch-corner/00.png", &shot_sketch_fixed(&mut app, si, 4.2));
            // ALL four corners are rounded, exactly as it is done by hand
            let corners: Vec<qymcad_core::model::Id> = app.project.sketches[si].points.iter().map(|p| p.id).collect();
            let mut done = 0;
            for pid in corners {
                if app.project.fillet_at_vertex(si, pid, 8.0) {
                    done += 1;
                }
            }
            assert!(done > 0, "not a single corner was rounded - the second frame would repeat the first");
            app.project.regen_sketch(si);
            save("sketch-corner/01.png", &shot_sketch_fixed(&mut app, si, 4.2));
        }

        // DELETE - two frames again, and for the same reason: the "after" shot shows a contour missing one
        // line, but that it was removed is clear only next to the "before".
        {
            let (mut app, si) = empty_sketch();
            app.project.add_rect_entity(si, -26.0, -17.0, 52.0, 34.0, qymcad_core::feature::Purpose::Real);
            app.project.add_circle_entity(si, 0.0, 0.0, 9.0, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(si);
            std::fs::create_dir_all(img_dir().join("sketch-delete")).expect("the frame directory");
            save("sketch-delete/00.png", &shot_sketch_fixed(&mut app, si, 4.2));
            let circle = app.project.sketches[si]
                .entities
                .iter()
                .find(|e| matches!(e.kind, qymcad_core::model::EntityKind::Circle { .. }))
                .map(|e| e.id)
                .expect("the circle in the sketch");
            app.project.delete_entities(si, &[circle]);
            app.project.regen_sketch(si);
            save("sketch-delete/01.png", &shot_sketch_fixed(&mut app, si, 4.2));
        }

        // TEXT IN A SKETCH - real glyph outlines rather than a caption. They are baked by the same call the
        // tool makes: `bake_text_glyphs` takes the outlines from the interface font.
        {
            let (mut app, si) = empty_sketch();
            let glyphs = app.bake_text_glyphs(-34.0, -8.0, 22.0, "QYM CAD");
            assert!(!glyphs.is_empty(), "the glyphs did not bake - the shot would be empty");
            app.project.add_sketch_text(si, -34.0, -8.0, 22.0, 0.0, "QYM CAD".to_string(), qymcad_core::feature::Purpose::Real, glyphs);
            app.project.regen_sketch(si);
            sketch_still(&mut app, si, "sketch-text");
        }

        // --- SCENES ADDED ALONGSIDE THE ARTICLES (the gaps in the help beyond the guard's reach) ---

        // A POINT IN A SKETCH - by itself it is invisible without neighbours, so the frame shows WHAT IT IS
        // FOR: a circle whose centre is held by a point, with construction marks around it.
        {
            let (mut app, si) = empty_sketch();
            app.project.add_rect_entity(si, -28.0, -18.0, 56.0, 36.0, qymcad_core::feature::Purpose::Real);
            for (x, y) in [(-18.0, -10.0), (18.0, -10.0), (18.0, 10.0), (-18.0, 10.0)] {
                app.project.sketch_point_at(si, x, y, 1e-6);
                app.project.add_circle_entity(si, x, y, 3.0, qymcad_core::feature::Purpose::Real);
            }
            sketch_still(&mut app, si, "sketch-point");
        }

        // SPLIT - two frames: a whole segment, and the same segment cut at the crossing point.
        {
            let (mut app, si) = empty_sketch();
            let line = app.project.add_line_entity(si, -30.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
            app.project.add_line_entity(si, 0.0, -18.0, 0.0, 18.0, qymcad_core::feature::Purpose::Real);
            std::fs::create_dir_all(img_dir().join("sketch-break")).expect("the frame directory");
            save("sketch-break/00.png", &shot_sketch_fixed(&mut app, si, 5.5));
            assert!(app.project.break_line(si, line, 0.0, 0.0), "the segment must split - otherwise the frames are identical");
            save("sketch-break/01.png", &shot_sketch_fixed(&mut app, si, 5.5));
        }

        // COPY - two frames: before and after. Moving and rotating are of the same nature, and one article
        // covers them.
        {
            let (mut app, si) = empty_sketch();
            let ids = app.project.add_rect_entity(si, -30.0, -6.0, 16.0, 12.0, qymcad_core::feature::Purpose::Real);
            std::fs::create_dir_all(img_dir().join("sketch-copy")).expect("the frame directory");
            save("sketch-copy/00.png", &shot_sketch_fixed(&mut app, si, 5.5));
            let made = app.project.copy_entities(si, &ids, 30.0, 0.0);
            assert!(!made.is_empty(), "no copy appeared - the second frame would repeat the first");
            save("sketch-copy/01.png", &shot_sketch_fixed(&mut app, si, 5.5));
        }

        // AN ARRAY IN A SKETCH - one frame per number of copies: that shows the main thing, that an array is
        // A PARAMETER rather than six separately drawn circles. The scale is fixed, or the frames breathe.
        {
            std::fs::create_dir_all(img_dir().join("sketch-array")).expect("the frame directory");
            for (i, n) in [2u32, 4, 6].into_iter().enumerate() {
                let (mut app, si) = empty_sketch();
                let c = app.project.add_circle_entity(si, -25.0, 0.0, 4.0, qymcad_core::feature::Purpose::Real);
                app.project.add_pattern(si, &[c], qymcad_core::model::PatternKind::Linear { dx: 10.0, dy: 0.0, count: n, dx2: 0.0, dy2: 0.0, count2: 0 });
                save(&format!("sketch-array/{i:02}.png"), &shot_sketch_fixed(&mut app, si, 5.5));
            }
        }

        // A BOOLEAN BETWEEN BODIES - TWO FRAMES, because one cannot show the point of the command: the
        // "after" shows a plate with a hole, and there is no telling from it that the hole came from A
        // CYLINDER rather than from a sketch. The first frame shows the two bodies overlapping, the second
        // the result of the cut.
        {
            let two_bodies = || {
                let mut app = in_one_part();
                let a = app.project.add_box(60.0, 40.0, 14.0);
                app.project.finish_base_body(a, 1);
                let b = app.project.add_cylinder(9.0, 40.0);
                app.project.finish_base_body(b, 1);
                app.rebuild_if_dirty();
                app
            };
            let before = two_bodies();
            let mut after = two_bodies();
            let bodies: Vec<qymcad_core::model::Id> = after.project.bodies.iter().map(|x| x.id).collect();
            let (a, b) = (bodies[bodies.len() - 2], bodies[bodies.len() - 1]);
            after.project.add_body_boolean(a, b, 0); // 0 = subtract
            after.rebuild_if_dirty();
            // THE TOOL BODY DOES NOT DISAPPEAR FROM THE LIST but is marked CONSUMED - and it is the selection
            // of visible bodies that keeps it out of the frame. The first version of this guard counted
            // bodies and went red on a perfectly good scene.
            assert!(after.project.consumed_bodies().contains(&b), "the boolean did not consume the tool body - the frames would come out identical");
            anim("part-boolean", vec![before, after]);
        }

        // SPLIT A FACE - THERE IS NO SCENE, AND THAT IS HONESTER THAN AN EMPTY PICTURE.
        //
        // The command does not change THE SHAPE: the body stays the same, it merely gains an edge. The help
        // raster draws bodies, and the edge overlay does not come up outside a live window for this scene -
        // four ways were tried (selecting the body, a live command, selecting the dividing edge itself) and
        // the frames came out byte for byte identical. Showing two identical frames as "before and after"
        // means lying with a picture, so the article goes without one for now, and that is recorded in the
        // note on `every_tool_article_shows_a_picture`.

        // A FACE COPY and a SURFACE REPLACE - one path over three frames, because apart they are pointless:
        // a face is taken off, edited on its own, and returned to the body. The copy taken off LIES EXACTLY
        // ON THE BODY and is indistinguishable from it in a frame - so on the second frame it is moved aside
        // by a Move: that shows it is a surface of its own rather than a face of the part.
        {
            let build = |steps: usize| -> App {
                let mut app = plate(12.0);
                let body = body_of(&app);
                let top: Vec<u32> = app.project.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
                if steps == 0 {
                    return app;
                }
                let surf = app.project.add_face_copy(body, qymcad_core::refs::Ref::picks(&top));
                app.rebuild_if_dirty();
                if steps == 1 {
                    // move the copy upwards - otherwise it merges with the face it was taken from
                    let up = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 16.0];
                    app.project.add_move(surf, up);
                    app.rebuild_if_dirty();
                    return app;
                }
                let faces: Vec<u32> = app.project.regen_faces.get(&surf).map(|fs| fs.iter().map(|f| f.id).collect()).unwrap_or_default();
                let _ = faces;
                app.project.add_surface_replace(body, qymcad_core::refs::Ref::picks(&top), surf);
                app.rebuild_if_dirty();
                app
            };
            anim("part-face-copy", (0..3).map(build).collect());
        }

        // STITCH and TRIM - two operations of the same order (working with surfaces), and both are shown by
        // the same means: a "before" frame and an "after" frame.
        {
            // STITCHING: two faces taken off become ONE surface, and that surface can then be worked with.
            // Three frames, because two are not enough: before and after a stitch look the same - the
            // surfaces already lie flush. So on the first frame they are SET APART (it shows there are two of
            // them), on the second they are stitched, and on the third the stitched one has gained thickness
            // - which is what stitching is done for.
            let stitch = |step: usize| -> App {
                let mut app = plate(12.0);
                let body = body_of(&app);
                let faces = app.project.regen_faces[&body].clone();
                let top: Vec<u32> = faces.iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
                let side: Vec<u32> = faces.iter().filter(|f| f.normal[2].abs() < 0.1 && f.normal[0] > 0.9).map(|f| f.id).collect();
                let a = app.project.add_face_copy(body, qymcad_core::refs::Ref::picks(&top));
                let b = app.project.add_face_copy(body, qymcad_core::refs::Ref::picks(&side));
                app.rebuild_if_dirty();
                if step == 0 {
                    let aside = [1.0, 0.0, 0.0, 18.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 10.0];
                    app.project.add_move(b, aside);
                    app.rebuild_if_dirty();
                    return app;
                }
                let sewn = app.project.add_stitch(vec![a, b], 1e-3);
                app.rebuild_if_dirty();
                if step == 1 {
                    return app;
                }
                if let Some(f) = app.project.regen_faces.get(&sewn).and_then(|fs| fs.first()).map(|f| f.id) {
                    app.project.add_thicken(sewn, f, 2.0);
                    app.rebuild_if_dirty();
                }
                app
            };
            anim("part-stitch", (0..3).map(stitch).collect());
        }

        // A RELATION BETWEEN JOINTS - three frames, and the whole point is in them: the driving arm turns
        // 20 deg while the driven one turns 40, because a gear relation with a ratio of 2 stands between
        // their joints. One frame cannot show this at all - a relation IS NOT VISIBLE in the geometry, it is
        // visible only in motion.
        //
        // TWO SEPARATE POSTS RATHER THAN ONE, AND THAT IS NOT DECORATION. The first version sat both arms on
        // ONE base - their axes coincided, the bodies climbed into one another, and instead of a mechanism
        // there was a mess. It was spotted at a glance. The mistake happened precisely because the drive
        // scene was copied and the numeric check "the parts do not sit inside one another" WAS NOT CARRIED
        // OVER from it - the very check that was added there after exactly the same case. It is restored
        // below.
        {
            use qymcad_core::feature::AnchorRef;
            fn face_towards2(p: &qymcad_core::model::Project, comp: qymcad_core::model::Id, dir: [f64; 3]) -> Option<(qymcad_core::model::Id, qymcad_core::feature::FaceKey)> {
                use qymcad_core::feature::FaceKey;
                let body = p.bodies.iter().find(|b| p.body_owner(b.id) == Some(comp))?;
                let (i, f) = body
                    .faces
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| {
                        let dot = |f: &qymcad_core::geom::MeshFace| f.normal[0] * dir[0] + f.normal[1] * dir[1] + f.normal[2] * dir[2];
                        dot(a).partial_cmp(&dot(b)).unwrap_or(std::cmp::Ordering::Equal)
                    })?;
                Some((body.id, FaceKey { index: i as u32, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }))
            }
            fn aabb(p: &qymcad_core::model::Project, comp: qymcad_core::model::Id) -> Option<([f64; 3], [f64; 3])> {
                let body = p.bodies.iter().find(|b| p.body_owner(b.id) == Some(comp))?;
                let m = p.body_world_transform(body.id);
                let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
                for v in &body.mesh.verts {
                    let w = qymcad_core::feature::apply12(&m, [v.x, v.y, v.z]);
                    for k in 0..3 {
                        lo[k] = lo[k].min(w[k]);
                        hi[k] = hi[k].max(w[k]);
                    }
                }
                Some((lo, hi))
            }
            std::fs::create_dir_all(img_dir().join("assembly-relation")).expect("the frame directory");
            let mut driven: Vec<f64> = Vec::new();
            for (i, lead) in [0.0f64, 20.0, 40.0].into_iter().enumerate() {
                let mut app = App::default();
                // TWO MECHANISMS SIDE BY SIDE: each with its own post and its own arm, with a gap between.
                part_with_plate(&mut app, "post-a", 20.0, 18.0, 8.0, [-46.0, -9.0, -8.0]);
                part_with_plate(&mut app, "arm-a", 34.0, 8.0, 5.0, [-36.0, -4.0, 0.0]);
                part_with_plate(&mut app, "post-b", 20.0, 18.0, 8.0, [10.0, -9.0, -8.0]);
                part_with_plate(&mut app, "arm-b", 34.0, 8.0, 5.0, [20.0, -4.0, 0.0]);
                app.rebuild_if_dirty();
                let by = |app: &App, n: &str| app.project.components.iter().find(|c| c.name == n).map(|c| c.id).expect("a part of the scene");
                let (pa, aa, pb, ab) = (by(&app, "post-a"), by(&app, "arm-a"), by(&app, "post-b"), by(&app, "arm-b"));
                app.project.set_grounded(pa, true);
                app.project.set_grounded(pb, true);
                let mut joints = Vec::new();
                for (post, arm) in [(pa, aa), (pb, ab)] {
                    let top = face_towards2(&app.project, post, [0.0, 0.0, 1.0]).expect("the top of the post");
                    let bottom = face_towards2(&app.project, arm, [0.0, 0.0, -1.0]).expect("the bottom of the arm");
                    let ca = app.project.add_connector(post, AnchorRef::FaceCenter(top.0, top.1));
                    let cb = app.project.add_connector(arm, AnchorRef::FaceCenter(bottom.0, bottom.1));
                    if let Some(c) = app.project.connectors.iter_mut().find(|c| c.id == cb) {
                        c.flip = true;
                    }
                    joints.push(app.project.add_joint(ca, cb, qymcad_core::feature::JointKind::Revolute));
                }
                // THE GEAR RELATION: the angle of the driven joint is twice the angle of the driving one.
                app.project.add_relation(qymcad_core::feature::RelationKind::Gear, joints[0], 0, joints[1], 0, 2.0);
                if let Some(j) = app.project.joints.iter_mut().find(|j| j.id == joints[0]) {
                    j.drive[0] = Some(lead);
                }
                app.project.solve_joints();
                driven.push(app.project.joints.iter().find(|j| j.id == joints[1]).map(|j| j.angle).unwrap_or(0.0));

                // NO PAIR THAT IS NOT MEANT TO TOUCH CLIMBS INTO THE OTHER - checked in numbers.
                for (x, y, what) in [(aa, ab, "the arms"), (aa, pb, "arm A and post B"), (ab, pa, "arm B and post A")] {
                    let (xl, xh) = aabb(&app.project, x).expect("the bounds");
                    let (yl, yh) = aabb(&app.project, y).expect("the bounds");
                    let over = (0..3).all(|k| xl[k] < yh[k] - 1e-6 && yl[k] < xh[k] - 1e-6);
                    assert!(!over, "at {lead} deg {what} sit inside one another: {xl:?}..{xh:?} and {yl:?}..{yh:?}");
                }

                app.set.show_joints = true;
                app.rebuild_if_dirty();
                app.set.gpu_viewport = false;
                app.mode_3d = true;
                app.cam.init = true;
                app.cam.scale = 3.4;
                app.cam.target = [0.0, 6.0, 0.0];
                let bg = app.scheme.pal.viewport_bg();
                let img = {
                    let a = &mut app;
                    super::super::help_raster::shot_ui([640, 420], bg, |ctx| {
                        a.apply_theme(ctx);
                        a.viewport(ctx);
                    })
                };
                save(&format!("assembly-relation/{i:02}.png"), &img);
            }
            // THE DRIVEN ONE REALLY DOES GO TWICE AS FAR - otherwise the picture would show a relation that
            // is not there.
            for k in 1..driven.len() {
                let d = (driven[k] - driven[k - 1]).abs();
                assert!(d > 25.0, "the driven joint travelled {d:.1} deg instead of about 40 - the relation did not act, the frames are meaningless");
            }
        }

        // TRIMMING A SURFACE - THERE IS NO SCENE, AND THAT IS A DECISION RATHER THAN FORGETFULNESS.
        //
        // To trim a sheet it has to be taken from somewhere (a donor plate) and cut with something (a bar),
        // while Move leaves the original body in the document as well. The frame came out with three bodies
        // instead of two - a mess in which there is no reading what was trimmed by what. Hiding the extra
        // one was tried four times, including after every rebuild (a rebuild recreates bodies with
        // `visible = true`), and no readable picture came out.
        //
        // A mess must not go into the help: exactly such a one was caught in a neighbouring scene, and
        // rightly. The article stays without a picture, and that is recorded in the note on
        // `every_tool_article_shows_a_picture`.

        // THE PARTS LIBRARY - a shot of the window itself: the categories, the search, the list. There is
        // nothing to show here but the window - the window IS the tool.
        {
            let mut app = App::default();
            app.win.parts_library = true;
            let img = shot_panel(&mut app, 760, 520, |a, ctx| a.parts_library_window(ctx));
            save("library.png", &img);
        }

        // A DATUM PLANE - what is built from when there is no face yet.
        //
        // On the first attempt it was not in the frame, and the conclusion drawn was that it "is not always
        // drawn". The rule turned out to be different and sensible: a datum belongs to A PART and is visible
        // only from inside it - in an assembly other people's datums do not get in the way. The shot was
        // taken from the root, which is why there was no plane. So we go inside, exactly as a person does.
        {
            let mut app = plate(12.0);
            let part = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
            app.enter_component_for_test(part);
            app.project.add_offset_plane(qymcad_core::feature::BasePlane::XY, 22.0);
            app.rebuild_if_dirty();
            app.set.gpu_viewport = false;
            app.mode_3d = true;
            app.cam.init = true;
            app.cam.scale = 4.2;
            app.cam.target = [20.0, 15.0, 12.0];
            let bg = app.scheme.pal.viewport_bg();
            let img = {
                let a = &mut app;
                super::super::help_raster::shot_ui([640, 440], bg, |ctx| {
                    a.apply_theme(ctx);
                    a.viewport(ctx);
                })
            };
            save("datum-plane.png", &img);
        }

        // THE HELP CONTENTS - a shot for the help itself: it shows in what order to read.
        {
            let mut app = App::default();
            app.open_help("index");
            let img = shot_panel(&mut app, 900, 620, |a, ctx| a.help_window(ctx));
            save("help-window.png", &img);
        }

        eprintln!("the pictures were written to {}", img_dir().display());
    }

    /// EVERY PICTURE MENTIONED IN AN ARTICLE EXISTS.
    ///
    /// A broken picture looks like a fault in the program rather than like a forgotten file. And there is no
    /// other way to notice it than by opening that very article in that very language.
    #[test]
    fn every_image_in_the_articles_exists() {
        let mut missing = Vec::new();
        for l in crate::help::languages() {
            for a in crate::help::articles(&l) {
                let md = crate::help::article(&a).expect("the article");
                for line in md.lines() {
                    let t = line.trim();
                    let Some(rest) = t.strip_prefix("![") else { continue };
                    let Some((_, p)) = rest.split_once("](") else { continue };
                    let Some(path) = p.strip_suffix(')') else { continue };
                    let ok = if path.ends_with('/') { !crate::help::frames(path).is_empty() } else { crate::help::image(path).is_some() };
                    if !ok {
                        missing.push(format!("{l}/{a} -> {path}"));
                    }
                }
            }
        }
        assert!(missing.is_empty(), "an article refers to a picture that does not exist ({}):\n{}", missing.len(), missing.join("\n"));
    }

    /// AND EVERY PICTURE IS A REAL PNG OF A SENSIBLE SIZE.
    ///
    /// "The file exists" is half a check: a truncated write, an empty file, an accidentally committed
    /// placeholder all pass straight through it, while the window shows an empty space.
    #[test]
    fn every_image_decodes_and_is_big_enough() {
        let mut all: Vec<String> = Vec::new();
        collect_paths(&mut all);
        assert!(all.len() > 10, "suspiciously few pictures in the help: {}", all.len());
        for p in all {
            let bytes = crate::help::image(&p).unwrap_or_else(|| panic!("no file {p}"));
            let img = image::load_from_memory(bytes).unwrap_or_else(|e| panic!("{p} does not read as a picture: {e}"));
            let (w, h) = (image::GenericImageView::dimensions(&img).0, image::GenericImageView::dimensions(&img).1);
            // BY AREA AND BY EACH SIDE rather than "no narrower than 320". The rule was written for scenes
            // with a body - those are wide; a shot of a side panel is tall and narrow (300x420), and the old
            // measure rejected it although everything in it can be made out perfectly well.
            assert!(w >= 240 && h >= 200 && w * h >= 96_000, "picture {p} is too small ({w}x{h}) - it cannot be made out in an article");
        }
    }

    /// Every picture path (the frames of an animation are expanded by name).
    fn collect_paths(out: &mut Vec<String>) {
        for l in crate::help::languages() {
            for a in crate::help::articles(&l) {
                let md = crate::help::article(&a).expect("the article");
                for line in md.lines() {
                    let t = line.trim();
                    let Some(rest) = t.strip_prefix("![") else { continue };
                    let Some((_, p)) = rest.split_once("](") else { continue };
                    let Some(path) = p.strip_suffix(')') else { continue };
                    if path.ends_with('/') {
                        out.extend(crate::help::frames(path));
                    } else {
                        out.push(path.to_string());
                    }
                }
            }
        }
        out.sort();
        out.dedup();
    }


    /// AND THERE IS SOMETHING IN THE PICTURE: it is not one colour and not one flat blot.
    ///
    /// Exactly the fault that was caught by eye and only by chance: the rasteriser stopped assembling the
    /// viewport texture, and the body was drawn SOLID WHITE. The file was in place, the size was right, the
    /// PNG decoded - every earlier guard stayed green. The colours are counted: a real scene has hundreds of
    /// them (light, antialiasing, shadows), a flat blot has a handful.
    #[test]
    fn every_image_actually_shows_something() {
        let mut all: Vec<String> = Vec::new();
        collect_paths(&mut all);
        for p in all {
            let bytes = crate::help::image(&p).unwrap_or_else(|| panic!("no file {p}"));
            let img = image::load_from_memory(bytes).expect("the picture decodes").to_rgb8();
            let total = img.pixels().len();
            let mut seen: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
            for px in img.pixels() {
                *seen.entry(px.0).or_default() += 1;
            }
            let top = seen.values().copied().max().unwrap_or(total);
            // THE THRESHOLDS ON FILL ARE DELIBERATELY WEAK, and that is not laziness. The pictures differ
            // enormously: a scene with a body has hundreds of colours, while a frame of two lines in a sketch
            // has six, and 99 % of the area is background. A strict threshold would reject correct pictures,
            // and a guard that complains about the correct gets switched off altogether. 24 colours and 97 %
            // were tried - the array of posts and the line extension fell out.
            assert!(seen.len() >= 3, "{p}: only {} colours - the picture is empty", seen.len());
            assert!(top * 1000 / total <= 998, "{p}: {} per mille of the area is one colour - there is nothing in the picture", top * 1000 / total);
            // PURE WHITE NEVER OCCURS HERE: the scheme is dark and the background of the scenes is
            // transparent. A lot of white is exactly the case where the rasteriser lost the texture and
            // filled the body with it.
            let white = seen.get(&[255, 255, 255]).copied().unwrap_or(0);
            assert!(white * 100 / total <= 15, "{p}: {}% pure white - the texture seems to have been lost", white * 100 / total);
        }
    }

    /// A PICTURE REACHES THE SCREEN while its markup does not.
    ///
    /// The parser can be mended and then left unwired from the drawing; the window would then show either
    /// emptiness or a raw `![...](...)` - and both read as a fault in the program.
    #[test]
    fn the_window_draws_an_image_and_its_caption() {
        // THE LANGUAGE IS PINNED. The frame is drawn in the current language while the caption is taken
        // from the article - and between those two steps a neighbouring test managed to switch the language:
        // one language on screen, a word of another in the expectation. The test went red not from a fault
        // but from another run alongside it.
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        crate::help::set_lang("ru");
        let mut app = App::default();
        app.open_help("part/08-hole");
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.help_window(c));
        assert!(!texts.iter().any(|t| t.contains("![") || t.contains("img/")), "the markup of a picture reached the screen raw: {:?}", texts.iter().filter(|t| t.contains("img/")).collect::<Vec<_>>());
        // THE CAPTION IS TAKEN FROM THE ARTICLE ITSELF rather than written out as a word: the language of the
        // help is shared across the run and neighbouring tests switch it - a string nailed down in one
        // language would go red every other time.
        let md = crate::help::article("part/08-hole").expect("the article");
        let caption = md.lines().find_map(|l| l.trim().strip_prefix("![").and_then(|r| r.split_once("](")).map(|(c, _)| c.to_string())).expect("the caption of the picture in the article");
        let word = caption.split_whitespace().max_by_key(|w| w.chars().count()).expect("a word of the caption");
        let found = texts.iter().any(|t| t.contains(word));
        crate::i18n::set_language(&prev);
        crate::help::set_lang("");
        assert!(found, "there is no caption under the picture (looked for \"{word}\"): {texts:?}");
    }



}
