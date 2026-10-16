//! THE STANDARD FILES OF EVERY FORMAT, OPENED THROUGH THE DOOR A PERSON USES, AND LOOKED AT.
//!
//! Reported behaviour: the import of each format was checked by its module's output and by files built for the
//! check, never by a file from the world opened in the window - and a real IGES assembly then hung the import.
//! This opens every file under `QYM_SAMPLES` (a folder) the way the File menu does, waits for it to land, frames
//! the camera on it and draws it through the real graphics pipeline into `<folder>/shots/<file>.png`, printing
//! what came in: the status line, the bodies, the triangles, the box.
#[cfg(test)]
mod tests {
    use crate::gui::import_door::tests::{answer, click, frame, key, running, settle, spot};
    use crate::gui::App;
    use qymcad_core::feature::apply12;
    use qymcad_ui_state::Want;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 950.0))
    }

    /// The camera on everything that came in, as `look_at_a_document` frames a document.
    fn fit(app: &mut App) {
        let basis = app.viewing.cam.basis();
        let (right, up) = (basis.0, basis.1);
        let mut pts: Vec<[f64; 3]> = Vec::new();
        for (i, b) in app.project.bodies.iter().enumerate() {
            let Some(id) = app.project.mesh_id(i) else { continue };
            let wt = app.project.body_world_transform(id);
            pts.extend(b.mesh.verts.iter().map(|v| apply12(&wt, [v.x, v.y, v.z])));
        }
        if pts.is_empty() {
            return;
        }
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
        app.viewing.cam.scale = ((r.width() as f64 / 2.0 / sx.max(1e-6)).min(r.height() as f64 / 2.0 / sy.max(1e-6)) * 0.9) as f32;
        app.viewing.cam.init = true;
        eprintln!("      box {:.3?} .. {:.3?}", lo, hi);
    }

    /// The window about units and scale, where it came up, answered as the file has it - Enter, as a person would.
    fn as_it_is(app: &mut App, ctx: &egui::Context) {
        let _ = frame(app, ctx, Vec::new());
        let _ = frame(app, ctx, key(egui::Key::Enter));
        let _ = frame(app, ctx, Vec::new());
    }

    /// Both sides of the camera, into `<shots>/<name><tag>.png` and `<name><tag>.back.png`.
    fn both_sides(app: &mut App, shots: &std::path::Path, name: &str, tag: &str) {
        for (side, turn) in [("", 0.0), (".back", std::f64::consts::PI)] {
            app.viewing.cam.yaw += turn;
            fit(app);
            match crate::gui::gpu_shot::eyes::shot(&app.painting(), rect()) {
                Some(img) => {
                    let png = crate::gui::color_image_to_png(&img).expect("PNG");
                    std::fs::write(shots.join(format!("{name}{tag}{side}.png")), png).expect("writing");
                }
                None => eprintln!("      no graphics device here"),
            }
        }
    }

    /// THE WINDOW ABOUT UNITS AND SCALE, SEEN OVER THE MODEL IT ASKS ABOUT: the whole program as a person sees it,
    /// into `<folder>/shots/<file>.window.png`, for the files whose window comes up - one without units, and two
    /// IGES that say millimetres and come in at 0.1 mm and 38.9 m.
    #[test]
    #[ignore = "files on this machine"]
    fn the_scale_window_is_looked_at() {
        let Ok(dir) = std::env::var("QYM_SAMPLES") else { return };
        let dir = std::path::PathBuf::from(dir);
        let shots = dir.join("shots");
        std::fs::create_dir_all(&shots).expect("a folder for the pictures");
        for name in ["cube-ascii.stl", "bearing.iges", "hammer.iges"] {
            let (mut app, ctx) = running();
            answer(&mut app, &ctx, Want::Anything, &dir.join(name).to_string_lossy());
            settle(&mut app, &ctx);
            let asking = app.win.import_scale.as_ref().map(|a| (a.factor, a.span));
            eprintln!("WINDOW {name}: {asking:?}");
            window_and_view(&mut app, &shots, name);
            if name == "hammer.iges" {
                // the factor pressed by hand, as a person answers "over 10 m": 0.001 lands the 38.4 m hammer at 38.4 mm
                let texts = frame(&mut app, &ctx, Vec::new());
                let at = spot(&texts, "0.001").expect("the factor 0.001 in the window");
                let _ = frame(&mut app, &ctx, click(at));
                settle(&mut app, &ctx);
                eprintln!("WINDOW {name} at 0.001: {:?}", app.win.import_scale.as_ref().map(|a| (a.factor, a.applied)));
                window_and_view(&mut app, &shots, &format!("{name}.x0.001"));
            }
        }
    }

    /// The whole program with the window about the scale over it, into `<shots>/<tag>.window.png`, and the view itself
    /// through the graphics device at the camera the program set, into `<tag>.view.png`.
    fn window_and_view(app: &mut App, shots: &std::path::Path, tag: &str) {
        let draw = |a: &mut App, ui: &mut egui::Ui| {
            let shell = crate::gui::shell(&a.set);
            for slot in qymcad_shell::Slot::ORDER {
                shell.run_slot(slot, ui, a);
            }
            crate::gui::import_scale::import_scale_window(&mut a.win_ctx(&mut Vec::new()), ui.ctx());
        };
        let bg = app.scheme.pal.viewport_bg();
        let pass = |app: &mut App| {
            crate::gui::help_raster::shot_ui([1280, 800], bg, |ui| {
                let ctx = &ui.ctx().clone();
                crate::gui::apply_theme(&mut app.scheme, &app.set, ctx);
                draw(app, ui);
            })
        };
        let _ = pass(app); // the first pass only settles the canvas rectangle
        if app.viewing.view_rect.width() > 1.0 {
            crate::gui::fit3d(&mut app.viewing.cam, &app.project, app.viewing.view_rect);
            qymcad_ui_state::fit(&app.project, &mut app.viewing.view, app.viewing.view_rect);
            // a drawing in its sketch, framed whole
        }
        app.cache.label_tex.borrow_mut().clear();
        let img = pass(app);
        std::fs::write(shots.join(format!("{tag}.window.png")), crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
        let view = app.viewing.view_rect;
        eprintln!("VIEW {tag}: 3D {}, framed at {} px/mm, the wheel goes {:?}", app.viewing.mode_3d, app.viewing.cam.scale, qymcad_ui_state::zoom_limits(app.viewing.cam.fit));
        if let Some(img) = crate::gui::gpu_shot::eyes::shot(&app.painting(), egui::Rect::from_min_size(egui::pos2(0.0, 0.0), view.size())) {
            std::fs::write(shots.join(format!("{tag}.view.png")), crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
        }
    }

    /// THE REGIONS EACH MESH SPLITS INTO, SEEN: the 24 widest in colours of their own, the rest by what they lie on
    /// - planes light grey, cylinders mid grey, spheres dark grey - and a region of one triangle red.
    ///   `<folder>/shots/<file>.regions.png`.
    #[test]
    #[ignore = "files on this machine"]
    fn every_mesh_shows_its_regions() {
        use qymcad_core::geom::{Mesh, Point3};
        use qymcad_meshfit::{prepare, regions, weld_tolerance, Surface, Tolerance};
        const PALETTE: [[u8; 3]; 24] = [
            [230, 25, 75],
            [60, 180, 75],
            [255, 225, 25],
            [0, 130, 200],
            [245, 130, 48],
            [145, 30, 180],
            [70, 240, 240],
            [240, 50, 230],
            [210, 245, 60],
            [250, 190, 212],
            [0, 128, 128],
            [220, 190, 255],
            [170, 110, 40],
            [255, 250, 200],
            [128, 0, 0],
            [170, 255, 195],
            [128, 128, 0],
            [255, 215, 180],
            [0, 0, 128],
            [255, 160, 0],
            [100, 60, 255],
            [0, 255, 120],
            [255, 90, 160],
            [90, 200, 255],
        ];
        let Ok(dir) = std::env::var("QYM_SAMPLES") else { return };
        let dir = std::path::PathBuf::from(dir);
        let shots = dir.join("shots");
        std::fs::create_dir_all(&shots).expect("a folder for the pictures");
        let only = std::env::var("QYM_SAMPLE").ok();
        let meshy = ["stl", "obj", "ply", "glb", "gltf", "3mf", "amf"];
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .expect("the folder reads")
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| meshy.contains(&e.to_string_lossy().to_lowercase().as_str())))
            .collect();
        files.sort();
        for f in files {
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let (mut app, ctx) = running();
            let before = app.project.bodies.len();
            answer(&mut app, &ctx, Want::Anything, &f.to_string_lossy());
            settle(&mut app, &ctx);
            as_it_is(&mut app, &ctx);
            let came = app.project.bodies.len();
            let mut said = Vec::new();
            for i in before..came {
                let Some(id) = app.project.mesh_id(i) else { continue };
                let wt = app.project.body_world_transform(id);
                let p = prepare(&app.project.bodies[i].mesh, weld_tolerance(&app.project.bodies[i].mesh));
                // `QYM_TOL_FACTOR` widens the distance a corner may lie from its surface, as the measure of the owner's
                // head does, to see where a mesh stops splitting
                let mut tol = Tolerance::for_mesh(&p);
                tol.distance *= std::env::var("QYM_TOL_FACTOR").ok().and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0);
                let found = regions(&p, &tol);
                let area = |t: u32| {
                    let [a, b, c] = p.mesh.triangle(t as usize);
                    let (u, v) = ([b.x - a.x, b.y - a.y, b.z - a.z], [c.x - a.x, c.y - a.y, c.z - a.z]);
                    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                    (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0
                };
                let mut order: Vec<usize> = (0..found.len()).collect();
                let widths: Vec<f64> = found.iter().map(|r| r.tris.iter().map(|&t| area(t)).sum()).collect();
                order.sort_by(|&a, &b| widths[b].total_cmp(&widths[a]));
                let mut groups: std::collections::BTreeMap<[u8; 3], Vec<[u32; 3]>> = std::collections::BTreeMap::new();
                for (rank, &r) in order.iter().enumerate() {
                    let region = &found[r];
                    let colour = if rank < PALETTE.len() {
                        PALETTE[rank]
                    } else if region.tris.len() == 1 {
                        [220, 30, 30]
                    } else {
                        match region.surface {
                            Some(Surface::Plane { .. }) => [205, 205, 205],
                            Some(Surface::Cylinder { .. }) => [140, 140, 140],
                            Some(Surface::Sphere { .. }) => [85, 85, 85],
                            Some(_) => [60, 60, 60],
                            None => [220, 30, 30],
                        }
                    };
                    groups.entry(colour).or_default().extend(region.tris.iter().map(|&t| p.mesh.tris[t as usize]));
                }
                let verts: Vec<Point3> = p
                    .mesh
                    .verts
                    .iter()
                    .map(|v| {
                        let w = apply12(&wt, [v.x, v.y, v.z]);
                        Point3::new(w[0], w[1], w[2])
                    })
                    .collect();
                app.project.bodies[i].visible = false;
                for (colour, tris) in groups {
                    app.project.add_mesh(Mesh { verts: verts.clone(), tris, ..Default::default() });
                    app.project.set_mesh_color(app.project.bodies.len() - 1, colour);
                }
                // WHERE THE REGIONS MEET, in black: every edge a thin square tube along its points, a thousandth of the
                // diagonal across
                let b = qymcad_meshfit::boundaries(&p, &found);
                let across = p.mesh.bounds().map(|bb| ((bb.max.x - bb.min.x).powi(2) + (bb.max.y - bb.min.y).powi(2) + (bb.max.z - bb.min.z).powi(2)).sqrt() * 1e-3).unwrap_or(0.01);
                let mut tube = Mesh { verts: Vec::new(), tris: Vec::new(), ..Default::default() };
                let cross = |x: [f64; 3], y: [f64; 3]| [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
                for e in &b.edges {
                    let mut pts = e.points.clone();
                    if let (None, Some(&first)) = (e.ends, pts.first()) {
                        pts.push(first);
                    }
                    for w in pts.windows(2) {
                        let (a, c) = (apply12(&wt, w[0]), apply12(&wt, w[1]));
                        let d = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                        if l < 1e-12 {
                            continue;
                        }
                        let d = [d[0] / l, d[1] / l, d[2] / l];
                        let u = cross(d, if d[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] });
                        let ul = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
                        let u = [u[0] / ul * across, u[1] / ul * across, u[2] / ul * across];
                        let v = cross(d, u);
                        let base = tube.verts.len() as u32;
                        for q in [a, c] {
                            for (s, t) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
                                tube.verts.push(Point3::new(q[0] + s * u[0] + t * v[0], q[1] + s * u[1] + t * v[1], q[2] + s * u[2] + t * v[2]));
                            }
                        }
                        for i in 0..4u32 {
                            let j = (i + 1) % 4;
                            tube.tris.push([base + i, base + j, base + 4 + j]);
                            tube.tris.push([base + i, base + 4 + j, base + 4 + i]);
                        }
                    }
                }
                if !tube.tris.is_empty() {
                    app.project.add_mesh(tube);
                    app.project.set_mesh_color(app.project.bodies.len() - 1, [15, 15, 15]);
                }
                let single = found.iter().filter(|r| r.tris.len() == 1).count();
                said.push(format!("{} tris -> {} regions ({single} of one triangle), {} corners, {} edges", p.mesh.tris.len(), found.len(), b.vertices.len(), b.edges.len()));
            }
            eprintln!("REGIONS {name}: {}", said.join("; "));
            both_sides(&mut app, &shots, &name, ".regions");
        }
    }

    /// EVERY MESH RECOGNISED INTO A BODY, SEEN: the regions' surfaces built into faces and sewn by the kernel, each face
    /// in one of twelve colours; a region whose face could not be built leaves a hole. `<folder>/shots/<file>.body.png`.
    #[test]
    #[ignore = "files on this machine"]
    fn every_mesh_is_seen_recognised_into_a_body() {
        use qymcad_core::geom::{Mesh, Point3};
        use qymcad_kernel::recognise::recognise as solid;
        use qymcad_meshfit::{boundaries, curves, prepare, regions, weld_tolerance, Tolerance};
        const COLOURS: [[u8; 3]; 12] = [
            [230, 25, 75],
            [60, 180, 75],
            [255, 225, 25],
            [0, 130, 200],
            [245, 130, 48],
            [145, 30, 180],
            [70, 240, 240],
            [240, 50, 230],
            [210, 245, 60],
            [250, 190, 212],
            [0, 128, 128],
            [170, 110, 40],
        ];
        let Ok(dir) = std::env::var("QYM_SAMPLES") else { return };
        let dir = std::path::PathBuf::from(dir);
        let shots = dir.join("shots");
        std::fs::create_dir_all(&shots).expect("a folder for the pictures");
        let only = std::env::var("QYM_SAMPLE").ok();
        let meshy = ["stl", "obj", "ply", "glb", "gltf", "3mf", "amf"];
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .expect("the folder reads")
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| meshy.contains(&e.to_string_lossy().to_lowercase().as_str())))
            .collect();
        files.sort();
        for f in files {
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let (mut app, ctx) = running();
            let before = app.project.bodies.len();
            answer(&mut app, &ctx, Want::Anything, &f.to_string_lossy());
            settle(&mut app, &ctx);
            as_it_is(&mut app, &ctx);
            let came = app.project.bodies.len();
            let mut said = Vec::new();
            for i in before..came {
                let Some(id) = app.project.mesh_id(i) else { continue };
                let wt = app.project.body_world_transform(id);
                let p = prepare(&app.project.bodies[i].mesh, weld_tolerance(&app.project.bodies[i].mesh));
                let tol = Tolerance::for_mesh(&p);
                let found = regions(&p, &tol);
                let b = boundaries(&p, &found);
                let c = curves(&b, &found, &tol);
                let Some(made) = solid(&p, &found, &b, &c, &tol) else {
                    said.push(format!("{} regions -> no body: {:?}", found.len(), qymcad_kernel::last_kernel_refusal()));
                    continue;
                };
                let (body, built, free) = (made.shape, made.areas.iter().flatten().count(), made.free_edges);
                app.project.bodies[i].visible = false;
                for qymcad_core::geom::Built { mesh, faces } in qymcad_kernel::specimen::mesh(&body, body.bbox_diag() * 2e-4) {
                    let verts: Vec<Point3> = mesh
                        .verts
                        .iter()
                        .map(|v| {
                            let w = apply12(&wt, [v.x, v.y, v.z]);
                            Point3::new(w[0], w[1], w[2])
                        })
                        .collect();
                    let mut groups: std::collections::BTreeMap<usize, Vec<[u32; 3]>> = std::collections::BTreeMap::new();
                    for (k, face) in faces.iter().enumerate() {
                        groups.entry(k % COLOURS.len()).or_default().extend(face.triangles.iter().map(|&t| mesh.tris[t as usize]));
                    }
                    for (k, tris) in groups {
                        app.project.add_mesh(Mesh { verts: verts.clone(), tris, ..Default::default() });
                        app.project.set_mesh_color(app.project.bodies.len() - 1, COLOURS[k]);
                    }
                }
                said.push(format!(
                    "{} regions -> {built} faces, {}, free sides {free}, volume {:.3} against the mesh's {:.3}",
                    found.len(),
                    if body.is_sheet() { "a shell" } else { "a solid" },
                    body.volume(),
                    p.mesh.volume()
                ));
            }
            eprintln!("BODY {name}: {}", said.join("; "));
            both_sides(&mut app, &shots, &name, ".body");
        }
    }

    /// WHERE EACH MESH IS TORN, SEEN. Every mesh that came in is prepared for recognition; the triangles
    /// preparation could not join on every side - a border, an edge three triangles share, a flat triangle closing
    /// a seam - move into a red body of their own, the rest stay grey. `<folder>/shots/<file>.prep.png`.
    #[test]
    #[ignore = "files on this machine"]
    fn every_mesh_shows_where_it_is_torn() {
        use qymcad_core::geom::{Mesh, Point3};
        use qymcad_meshfit::{prepare, weld_tolerance, NO_NEIGHBOUR};
        let Ok(dir) = std::env::var("QYM_SAMPLES") else { return };
        let dir = std::path::PathBuf::from(dir);
        let shots = dir.join("shots");
        std::fs::create_dir_all(&shots).expect("a folder for the pictures");
        let only = std::env::var("QYM_SAMPLE").ok();
        let meshy = ["stl", "obj", "ply", "glb", "gltf", "3mf", "amf"];
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .expect("the folder reads")
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| meshy.contains(&e.to_string_lossy().to_lowercase().as_str())))
            .collect();
        files.sort();
        for f in files {
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let (mut app, ctx) = running();
            let before = app.project.bodies.len();
            answer(&mut app, &ctx, Want::Anything, &f.to_string_lossy());
            settle(&mut app, &ctx);
            as_it_is(&mut app, &ctx);
            let came = app.project.bodies.len();
            let (mut torn_all, mut all) = (0, 0);
            for i in before..came {
                let Some(id) = app.project.mesh_id(i) else { continue };
                let p = prepare(&app.project.bodies[i].mesh, weld_tolerance(&app.project.bodies[i].mesh));
                let torn: Vec<bool> = (0..p.mesh.tris.len()).map(|t| p.neighbours[t].contains(&NO_NEIGHBOUR) || p.slivers.contains(&(t as u32))).collect();
                let pick = |want: bool| Mesh { verts: p.mesh.verts.clone(), tris: p.mesh.tris.iter().zip(&torn).filter(|(_, b)| **b == want).map(|(t, _)| *t).collect(), ..Default::default() };
                torn_all += torn.iter().filter(|b| **b).count();
                all += torn.len();
                let wt = app.project.body_world_transform(id);
                let mut red = pick(true);
                for v in &mut red.verts {
                    let w = apply12(&wt, [v.x, v.y, v.z]);
                    *v = Point3::new(w[0], w[1], w[2]);
                }
                app.project.bodies[i].mesh = pick(false);
                app.project.bodies[i].faces.clear();
                app.project.set_mesh_color(i, [170, 170, 170]);
                if !red.tris.is_empty() {
                    app.project.add_mesh(red);
                    app.project.set_mesh_color(app.project.bodies.len() - 1, [220, 30, 30]);
                }
            }
            eprintln!("PREP {name}: {torn_all} of {all} triangles not joined on every side");
            both_sides(&mut app, &shots, &name, ".prep");
        }
    }

    #[test]
    #[ignore = "files on this machine"]
    fn every_standard_file_opens_and_is_looked_at() {
        let Ok(dir) = std::env::var("QYM_SAMPLES") else { return };
        let dir = std::path::PathBuf::from(dir);
        let shots = dir.join("shots");
        std::fs::create_dir_all(&shots).expect("a folder for the pictures");
        let only = std::env::var("QYM_SAMPLE").ok();
        let mut files: Vec<std::path::PathBuf> =
            std::fs::read_dir(&dir).expect("the folder reads").flatten().map(|e| e.path()).filter(|p| p.is_file() && qymcad_io::Format::of_path(&p.to_string_lossy()).is_some()).collect();
        if let Ok(sub) = std::fs::read_dir(&dir) {
            for e in sub.flatten().filter(|e| e.path().is_dir() && e.file_name() != "shots") {
                files.extend(std::fs::read_dir(e.path()).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| qymcad_io::Format::of_path(&p.to_string_lossy()).is_some()));
            }
        }
        files.sort();
        for f in files {
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let (mut app, ctx) = running();
            let before = app.project.bodies.len();
            let t = std::time::Instant::now();
            answer(&mut app, &ctx, Want::Anything, &f.to_string_lossy());
            settle(&mut app, &ctx);
            as_it_is(&mut app, &ctx);
            let came: Vec<(String, usize)> = app.project.bodies.iter().skip(before).map(|b| (b.name.clone(), b.mesh.tris.len())).collect();
            let tris: usize = came.iter().map(|(_, n)| n).sum();
            let pending = app.tools.pending_import.curves.as_ref().map(|(c, _, _)| c.len());
            // a drawing waits for its plane: it is laid on XY, as a click on that plane would lay it
            if pending.is_some() {
                app.place_pending_import(qymcad_core::feature::SketchPlane::World(qymcad_core::feature::BasePlane::XY));
            }
            eprintln!("SAMPLE {name}: {:.1} s, {} bodies, {tris} triangles, curves waiting {:?}\n      status: {}", t.elapsed().as_secs_f64(), came.len(), pending, app.status);
            for (name, n) in came.iter().take(4) {
                eprintln!("      body {name:?}: {n} triangles");
            }
            // A DRAWING IS SEEN IN THE WINDOW: a sketch is drawn by the window over the scene, and a shot through the
            // graphics device alone shows an empty view
            if pending.is_some() {
                window_and_view(&mut app, &shots, &name);
                continue;
            }
            // FROM BOTH SIDES: a picture from one side can look wrong only because the file's own picture was taken
            // from the other (a bevel gear shows its teeth converging from the front and a flat back from behind)
            both_sides(&mut app, &shots, &name, "");
        }
    }
}
