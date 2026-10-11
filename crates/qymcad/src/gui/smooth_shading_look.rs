//! A SMOOTH SURFACE IS SHADED AS SMOOTH: by its own normal, with no seam on a closed surface and no step between a
//! rounding and the faces it is tangent to.
//!
//! Reported behaviour: blotches and streaks along every rounding, dark tongues where three roundings meet, a line along
//! a sphere, a hard step in the shadow between neighbouring faces - all of it since the release of 1 October.
//!
//! The normals are read off the scene as it goes to the card (`gpu_scene`), after their packing - what the shader
//! takes. Two numbers: how far a normal is from the surface's own (on a sphere, where the surface's normal is known
//! by formula), and how far apart the normals are at one point of space where the surface is smooth (a seam, the
//! border of a rounding). The pictures are left in `target/look/` to be looked at.
#[cfg(test)]
pub(crate) mod tests {
    use crate::gui::App;

    /// The angle between two directions, in degrees.
    fn angle(a: [f64; 3], b: [f64; 3]) -> f64 {
        let la = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        let lb = (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt();
        let c = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / (la * lb).max(1e-12);
        c.clamp(-1.0, 1.0).acos().to_degrees()
    }

    /// The normal a vertex of the card carries, unpacked as the shader unpacks it.
    fn unpacked(nrm: u32) -> [f64; 3] {
        qymcad_core::geom::unpack_direction(nrm).unwrap_or([0.0; 3])
    }

    /// Every vertex of the scene as the card gets it: where it is and the normal it is shaded with.
    pub(crate) fn shaded(app: &App) -> Vec<([f64; 3], [f64; 3])> {
        let (verts, _) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
        verts.iter().map(|v| ([v.pos[0] as f64, v.pos[1] as f64, v.pos[2] as f64], unpacked(v.nrm))).collect()
    }

    /// THE WORST STEP OF THE LIGHT WHERE THE SURFACE IS SMOOTH: of the vertices at one point of space, the pairs whose
    /// normals turn by less than `crease` degrees (a sharp edge turns by more and is meant to be sharp), the widest.
    pub(crate) fn worst_step(v: &[([f64; 3], [f64; 3])], crease: f64) -> f64 {
        let mut at: std::collections::HashMap<[i64; 3], Vec<[f64; 3]>> = std::collections::HashMap::new();
        for (p, n) in v {
            at.entry(p.map(|c| (c * 1e4).round() as i64)).or_default().push(*n);
        }
        let mut worst = 0.0f64;
        for ns in at.values() {
            for i in 0..ns.len() {
                for j in i + 1..ns.len() {
                    let a = angle(ns[i], ns[j]);
                    if a < crease {
                        worst = worst.max(a);
                    }
                }
            }
        }
        worst
    }

    /// THE WORST NORMAL OF A SPHERE OFF THE SPHERE'S OWN, in degrees. The centre is the middle of its box: an average
    /// of the vertices leans to where they lie thicker.
    fn off_the_sphere(v: &[([f64; 3], [f64; 3])]) -> f64 {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for (p, _) in v {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        v.iter().map(|(p, n)| angle(*n, [p[0] - c[0], p[1] - c[1], p[2] - c[2]])).fold(0.0f64, f64::max)
    }

    /// A SPHERE IS LIT BY ITS OWN NORMAL, with no seam. Measured before: the normal of the light up to 6.43 deg off the
    /// sphere's own, and the two sides of the seam 12.75 deg apart - the line along a sphere of the report. Now 0.07
    /// deg, the vertices themselves rounded to single precision, and no step at all.
    #[test]
    fn a_sphere_is_lit_by_its_own_normal_with_no_seam() {
        let v = shaded(&sphere(20.0));
        let (off, step) = (off_the_sphere(&v), worst_step(&v, 30.0));
        assert!(off <= 0.2, "the light of a sphere stands {off:.2} deg off the sphere's own normal");
        assert!(step <= 0.1, "the light of a sphere steps by {step:.2} deg at one point - a seam");
    }

    /// A ROUNDED BOX HAS NO STEP IN ITS LIGHT where a rounding meets the face it is tangent to, and where three
    /// roundings meet at a corner. Measured before: 6.30 deg - the hard step in the shadow of the report.
    #[test]
    fn a_rounded_box_has_no_step_in_its_light() {
        let step = worst_step(&shaded(&rounded_box(3.0)), 30.0);
        assert!(step <= 0.1, "the light of a rounded box steps by {step:.2} deg where the surface is smooth");
    }

    /// THE NORMALS OF THE SURFACE ARE KEPT IN THE FILE: an opened document is not rebuilt, and without them its light
    /// would be guessed from the triangles again.
    #[test]
    fn the_normals_of_the_surface_are_kept_in_the_file() {
        let app = sphere(20.0);
        let path = std::env::temp_dir().join(format!("qymcad-normals-{}.qcad", std::process::id()));
        let path = path.to_string_lossy().into_owned();
        qymcad_io::save_project(&app.project, &path).expect("the document is saved");
        let opened = qymcad_io::load_project(&path).expect("the document opens");
        let _ = std::fs::remove_file(&path);
        let (before, after) = (&app.project.bodies[0].mesh.norms, &opened.bodies[0].mesh.norms);
        assert!(!before.is_empty() && before == after, "the normals of the surface are not kept: {} before, {} after", before.len(), after.len());
    }

    /// The part to work in, empty.
    fn part() -> App {
        let mut app = App::default();
        app.set.shading = qymcad_ui_state::Shading::Smooth;
        let root = app.project.root;
        app.project.set_active_component(Some(root));
        let part = app.project.add_part("Look");
        app.enter_component(part);
        app
    }

    /// A box with every edge rounded: twelve roundings and eight corners where three of them meet.
    pub(crate) fn rounded_box(radius: f64) -> App {
        let mut app = part();
        let body = app.project.add_box(40.0, 30.0, 10.0);
        qymcad_ui_state::regenerate_now(&mut app.rebuild_ctx());
        let edges: Vec<u32> = app.project.regen_edges.get(&body).map(|es| es.iter().map(|e| e.id).collect()).unwrap_or_default();
        assert_eq!(edges.len(), 12, "setup: a box has twelve edges");
        app.project.add_fillet(body, radius, edges);
        qymcad_ui_state::regenerate_now(&mut app.rebuild_ctx());
        app
    }

    /// A CUP: a cylinder R30 x 20 with its bottom edge rounded R5, shelled 3 mm with the top face open - a rounding
    /// inside and out, as in a turned or moulded part.
    pub(crate) fn cup() -> App {
        let mut app = part();
        let body = app.project.add_cylinder(30.0, 20.0);
        qymcad_ui_state::regenerate_now(&mut app.rebuild_ctx());
        let edges = app.project.regen_edges.get(&body).cloned().unwrap_or_default();
        let bottom = edges.iter().filter(|e| e.dir[2].abs() < 0.5).min_by(|a, b| a.mid[2].total_cmp(&b.mid[2])).map(|e| e.id).expect("setup: the bottom edge");
        let rounded = app.project.add_fillet(body, 5.0, vec![bottom]);
        qymcad_ui_state::regenerate_now(&mut app.rebuild_ctx());
        let solid = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).unwrap_or(rounded);
        let top = app.project.bodies.iter().find(|b| b.id == solid).and_then(|b| b.faces.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).map(|f| f.id)).expect("setup: the top face");
        app.project.add_shell_mode(solid, 3.0, vec![top], qymcad_core::feature::ShellSide::Inward);
        qymcad_ui_state::regenerate_now(&mut app.rebuild_ctx());
        app
    }

    /// A sphere: one closed face with a seam.
    pub(crate) fn sphere(r: f64) -> App {
        let mut app = part();
        app.project.add_sphere(r);
        qymcad_ui_state::regenerate_now(&mut app.rebuild_ctx());
        app
    }

    /// Point the camera at everything there is, from the usual three-quarter view, and leave the picture of the card
    /// in `target/look/<name>.png`.
    fn picture(app: &mut App, name: &str) {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
        let pts: Vec<[f64; 3]> = shaded(app).into_iter().map(|(p, _)| p).collect();
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for q in &pts {
            for k in 0..3 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        let across = (0..3).map(|k| hi[k] - lo[k]).fold(0.0f64, f64::max).max(1e-6);
        app.viewing.cam.target = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        app.viewing.cam.scale = (rect.height() as f64 / across * 0.9) as f32;
        app.viewing.cam.init = true;
        app.viewing.mode_3d = true;
        let Some(img) = crate::gui::gpu_shot::eyes::shot(&app.painting(), rect) else { return };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look");
        let _ = std::fs::create_dir_all(&dir);
        if let Some(png) = crate::gui::color_image_to_png(&img) {
            let _ = std::fs::write(dir.join(format!("{name}.png")), png);
        }
    }

    /// THE BOTTOM OF THE MODEL CLOSE UP: the camera on the lowest front point, `times` nearer than the whole.
    fn close_up(app: &mut App, name: &str, times: f32) {
        picture(app, name);
        let pts: Vec<[f64; 3]> = shaded(app).into_iter().map(|(p, _)| p).collect();
        let low = pts.iter().map(|p| p[2]).fold(f64::MAX, f64::min);
        let fwd = app.viewing.cam.basis().2;
        let near = pts.iter().filter(|p| p[2] < low + 1.0).max_by(|a, b| (-(a[0] * fwd[0] + a[1] * fwd[1])).total_cmp(&-(b[0] * fwd[0] + b[1] * fwd[1]))).copied();
        if let Some(p) = near {
            app.viewing.cam.target = p;
            app.viewing.cam.scale *= times;
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
            if let Some(img) = crate::gui::gpu_shot::eyes::shot(&app.painting(), rect) {
                let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look");
                if let Some(png) = crate::gui::color_image_to_png(&img) {
                    let _ = std::fs::write(dir.join(format!("{name}_close.png")), png);
                }
            }
        }
    }

    /// THE NUMBERS AND THE PICTURES, for the eyes: what the light is made of on a rounded box and on a sphere.
    #[test]
    #[ignore = "a measure for the eyes, run by hand: cargo test -p qymcad --lib smooth_shading_measure -- --ignored --nocapture"]
    fn smooth_shading_measure() {
        let mut boxed = rounded_box(3.0);
        let v = shaded(&boxed);
        eprintln!("rounded box: {} vertices; worst step of the light where it is smooth: {:.2} deg", v.len(), worst_step(&v, 30.0));
        picture(&mut boxed, "rounded_box");

        let mut ball = sphere(20.0);
        let v = shaded(&ball);
        let off = off_the_sphere(&v);
        eprintln!("sphere: {} vertices; worst normal off the surface's own: {:.2} deg; worst step of the light at one point: {:.2} deg", v.len(), off, worst_step(&v, 30.0));
        picture(&mut ball, "sphere");
        // THE SOFTWARE PICTURE of the same frame: it takes the same normals and draws its own way
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
        let basis = ball.viewing.cam.basis();
        if let Some(img) = qymcad_render::rasterize_3d(&ball.painting(), rect, &basis, 1.0, 1.0).and_then(|i| crate::gui::color_image_to_png(&i)) {
            let _ = std::fs::write(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look/sphere_raster.png"), img);
        }

        let mut cup = cup();
        let v = shaded(&cup);
        eprintln!("cup: {} vertices; worst step of the light where it is smooth: {:.2} deg", v.len(), worst_step(&v, 30.0));
        picture(&mut cup, "cup");
        cup.side.section.plane = Some(([0.0; 3], [0.0, 1.0, 0.0]));
        picture(&mut cup, "cup_section");

        // A FILE BROUGHT IN (`QYM_LOOK_IMPORT`) through File -> Import, as a person brings it: its light, and its picture
        if let Ok(path) = std::env::var("QYM_LOOK_IMPORT") {
            let (mut doc, ctx) = crate::gui::import_door::tests::running();
            doc.set.shading = qymcad_ui_state::Shading::Smooth;
            crate::gui::import_door::tests::answer(&mut doc, &ctx, qymcad_ui_state::Want::Anything, &path);
            crate::gui::import_door::tests::settle(&mut doc, &ctx);
            for b in &doc.project.bodies {
                eprintln!("  body {}: {} vertices, {} surface normals", b.id, b.mesh.verts.len(), b.mesh.norms.len());
            }
            let v = shaded(&doc);
            eprintln!("import: worst step of the light where it is smooth: {:.2} deg", worst_step(&v, 20.0));
            close_up(&mut doc, "import", 4.0);
        }
        // THE SAME PART AS SAVED BEFORE (`QYM_LOOK_DOC`): its meshes carry no normals of the surface, the light is the
        // old average - the "before" of the same frame
        if let Ok(path) = std::env::var("QYM_LOOK_DOC") {
            let project = qymcad_io::load_project(&path).expect("the document opens");
            let mut doc = App::default();
            doc.set.shading = qymcad_ui_state::Shading::Smooth;
            doc.finish_project_load(path.clone(), project, Vec::new());
            close_up(&mut doc, "as_saved", 4.0);
        }
    }
}
