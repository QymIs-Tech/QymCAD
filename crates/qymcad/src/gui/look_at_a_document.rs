//! LOOKING AT THE EXTRUDED TEXT BY EYE - a picture, not a claim.
//!
//! Reported behaviour: "the faces fell apart" on a document with extruded text, without any import being
//! opened. A check that says "it did not crash" cannot see that; the only way is to draw the scene and look
//! at it.
//!
//! `QYM_LOOK_DOC` points it at any document - the scenario one by default. The pictures go to `target/look/`:
//! the raster and the card, so the two can be held side by side.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::apply12;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 950.0))
    }

    fn fit(app: &mut App) {
        let basis = app.viewing.cam.basis();
        let (right, up) = (basis.0, basis.1);
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        let mut pts: Vec<[f64; 3]> = Vec::new();
        for (i, b) in app.project.bodies.iter().enumerate() {
            let Some(id) = app.project.mesh_id(i) else { continue };
            if !b.visible {
                continue;
            }
            let wt = app.project.body_world_transform(id);
            for v in &b.mesh.verts {
                let q = apply12(&wt, [v.x, v.y, v.z]);
                for k in 0..3 {
                    lo[k] = lo[k].min(q[k]);
                    hi[k] = hi[k].max(q[k]);
                }
                pts.push(q);
            }
        }
        if pts.is_empty() {
            return;
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
    }

    /// The same frame, but drawn BY THE CARD through the real pipeline.
    fn gpu_shot(app: &mut App, name: &str) {
        match crate::gui::gpu_shot::eyes::shot(&app.painting(), rect()) {
            Some(img) => {
                let png = crate::gui::color_image_to_png(&img).expect("PNG");
                let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look");
                std::fs::create_dir_all(&dir).ok();
                std::fs::write(dir.join(name), png).expect("writing");
                eprintln!("SNAPSHOT {name} (the card)");
            }
            None => eprintln!("SNAPSHOT {name}: there is no graphics device here"),
        }
    }

    fn shot(app: &mut App, name: &str) {
        let basis = app.viewing.cam.basis();
        let Some(img) = qymcad_render::rasterize_3d(&app.painting(), rect(), &basis, 1.0, 1.0) else {
            eprintln!("SNAPSHOT {name}: the rasteriser came back empty");
            return;
        };
        let png = crate::gui::color_image_to_png(&img).expect("PNG");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look");
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(dir.join(name), png).expect("writing");
        eprintln!("SNAPSHOT {name}");
    }

    /// ONE SOLID FROM A FILE OF THE KERNEL'S OWN (`QYM_LOOK_BREP`), tessellated afresh and drawn both ways.
    ///
    /// A document carries the MESH it was saved with, so a change in how bodies are tessellated shows nothing
    /// there until they are rebuilt. Here the mesh is made on the spot.
    #[test]
    #[ignore = "an eye, not a check: writes pictures"]
    fn look_at_one_solid() {
        let Ok(path) = std::env::var("QYM_LOOK_BREP") else {
            eprintln!("QYM_LOOK_BREP is not set - nothing to look at");
            return;
        };
        let bytes = std::fs::read(&path).expect("the solid file");
        let shape = qymcad_kernel::specimen::read(&bytes).expect("the kernel reads it");
        let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(&shape, 0.5).into_iter().next().expect("it tessellates");
        eprintln!("LOOKING AT {path}: {} vertices, {} triangles", mesh.verts.len(), mesh.tris.len());

        // ONE CONNECTED PIECE OF THE SHELL ON ITS OWN (`QYM_ONLY_PIECE`): where a band of surface seems to be
        // missing, the question is whether its triangles exist at all and where they stand.
        let mesh = match std::env::var("QYM_ONLY_PIECE").ok().and_then(|s| s.parse::<usize>().ok()) {
            None => mesh,
            Some(want) => {
                let grid = 1.0e-4;
                let key = |v: qymcad_core::geom::Point3| [(v.x / grid).round() as i64, (v.y / grid).round() as i64, (v.z / grid).round() as i64];
                let mut at: std::collections::HashMap<[i64; 3], u32> = std::collections::HashMap::new();
                let mut weld: Vec<u32> = Vec::with_capacity(mesh.verts.len());
                for v in &mesh.verts {
                    let n = at.len() as u32;
                    weld.push(*at.entry(key(*v)).or_insert(n));
                }
                let mut on_edge: std::collections::HashMap<(u32, u32), Vec<u32>> = std::collections::HashMap::new();
                for (i, t) in mesh.tris.iter().enumerate() {
                    for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                        let (a, b) = (weld[a as usize], weld[b as usize]);
                        on_edge.entry((a.min(b), a.max(b))).or_default().push(i as u32);
                    }
                }
                let mut seen = vec![false; mesh.tris.len()];
                let mut pieces: Vec<Vec<u32>> = Vec::new();
                for start in 0..mesh.tris.len() {
                    if seen[start] {
                        continue;
                    }
                    let mut piece = Vec::new();
                    let mut queue = std::collections::VecDeque::from([start as u32]);
                    seen[start] = true;
                    while let Some(i) = queue.pop_front() {
                        piece.push(i);
                        let t = mesh.tris[i as usize];
                        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                            let (a, b) = (weld[a as usize], weld[b as usize]);
                            for &j in on_edge.get(&(a.min(b), a.max(b))).map(|v| v.as_slice()).unwrap_or(&[]) {
                                if !seen[j as usize] {
                                    seen[j as usize] = true;
                                    queue.push_back(j);
                                }
                            }
                        }
                    }
                    pieces.push(piece);
                }
                pieces.sort_by_key(|p| std::cmp::Reverse(p.len()));
                eprintln!("  pieces: {:?}, drawing number {want}", pieces.iter().map(|p| p.len()).collect::<Vec<_>>());
                let keep = pieces.get(want).cloned().unwrap_or_default();
                qymcad_core::geom::Mesh { verts: mesh.verts.clone(), tris: keep.iter().map(|&i| mesh.tris[i as usize]).collect(), ..Default::default() }
            }
        };
        // IN THE ROOT, with no part around it: a body with no owner is drawn in the root assembly and nowhere
        // else, and here there is nothing to look at but the body.
        let mut app = App::default();
        app.project.add_mesh(mesh);
        app.viewing.mode_3d = true;
        fit(&mut app);
        shot(&mut app, "solid-raster.png");
        gpu_shot(&mut app, "solid-card.png");
    }

    /// A NEIGHBOUR IN CONTEXT, translucent: `samples/project.qcad`, its second part entered, "In context" on - the part with
    /// the hexagonal hole shown as a ghost. Reported behaviour: the walls of the hole and the faces behind them heaped up
    /// darker than the rest, a mess of layers.
    #[test]
    #[ignore = "an eye, not a check: writes pictures"]
    fn look_at_a_ghost_in_context() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/project.qcad").to_string();
        let project = qymcad_io::load_project(&path).expect("the document opens");
        let mut app = App::default();
        app.finish_project_load(path.clone(), project, Vec::new());
        app.viewing.mode_3d = true;
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let part = app.project.components.iter().filter(|c| app.project.ctx_holds_bodies(c.id)).nth(1).map(|c| c.id).expect("the second part of the document");
        app.enter_component(part);
        app.win.context = true;
        fit(&mut app);
        shot(&mut app, "ghost-in-context.png");
        gpu_shot(&mut app, "ghost-in-context-card.png");
    }

    #[test]
    #[ignore = "an eye, not a check: writes pictures"]
    fn look_at_the_document() {
        let path = std::env::var("QYM_LOOK_DOC").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/user-case.qcad").to_string());
        let project = qymcad_io::load_project(&path).expect("the document opens");
        let mut app = App::default();
        app.finish_project_load(path.clone(), project, Vec::new());
        app.viewing.mode_3d = true;
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());

        // EVERYTHING THAT IS IN THE DOCUMENT, whatever the ticks say: a document saved with the assembly
        // hidden shows one body, and the question is usually about all of them. `QYM_LOOK_ALL=1` ticks them on.
        if std::env::var("QYM_LOOK_ALL").is_ok() {
            for b in app.project.bodies.iter_mut() {
                b.visible = true;
            }
            for c in app.project.components.iter_mut() {
                c.visible = true;
            }
        }
        // ONE BODY ON ITS OWN: a hole is looked for in the part, not in the assembly around it.
        if let Ok(n) = std::env::var("QYM_ONLY_BODY").map(|s| s.parse::<usize>().unwrap_or(0)) {
            for (i, b) in app.project.bodies.iter_mut().enumerate() {
                b.visible = i == n;
            }
        }
        eprintln!("LOOKING AT {path}");
        for (i, b) in app.project.bodies.iter().enumerate().take(3) {
            let deg = (0..b.mesh.tris.len())
                .filter(|&t| {
                    let p = b.mesh.triangle(t);
                    let (u, v) = ([p[1].x - p[0].x, p[1].y - p[0].y, p[1].z - p[0].z], [p[2].x - p[0].x, p[2].y - p[0].y, p[2].z - p[0].z]);
                    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                    (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() < 1e-12
                })
                .count();
            eprintln!("  body {i}: {} vertices, {} triangles, {} faces, {deg} degenerate, visible {}", b.mesh.verts.len(), b.mesh.tris.len(), b.faces.len(), b.visible);
        }

        fit(&mut app);
        shot(&mut app, "doc-iso.png");
        gpu_shot(&mut app, "doc-iso-card.png");
        // HOW MANY BODIES ARE DRAWN AS GHOSTS: a body of a foreign context is drawn translucent and without
        // writing depth, so if that verdict goes wrong the whole picture turns see-through.
        // WHOSE BODY IS IT, asked of the FIRST node that names it and of the LAST: where the two answers part
        // company, a pass that takes the wrong one puts the body in a foreign component - and then everything
        // inside a context is drawn as a ghost, see-through and without depth.
        {
            let (mut first, mut last) = (std::collections::HashMap::new(), std::collections::HashMap::new());
            for n in &app.project.timeline {
                for b in n.kind.bodies() {
                    first.entry(b).or_insert(n.parent);
                    last.insert(b, n.parent);
                }
            }
            let apart = first.iter().filter(|(b, p)| last.get(b) != Some(p)).count();
            eprintln!("  bodies whose first and last owning nodes differ: {apart} of {}", first.len());
        }
        // IS THE SHELL CLOSED. An edge of a closed mesh belongs to exactly two triangles; an edge with one
        // is a hole - you see the inside of the part through it. Reported with a screenshot: a gap in a pipe
        // bracket where the shell should be.
        {
            let mut open_bodies: Vec<(usize, usize, usize, f64)> = Vec::new();
            for (i, b) in app.project.bodies.iter().enumerate() {
                if !b.visible || b.mesh.tris.is_empty() {
                    continue;
                }
                // the vertices are welded by position: a mesh from the kernel carries a vertex per face
                // corner, so comparing indices would call every part open
                // HOW CLOSE TWO POINTS COUNT AS ONE. A seam between two faces of one solid carries the same
                // points; a model sewn with a loose tolerance may carry them a hundredth apart. The grid says
                // which of the two we are looking at, and it is asked from outside (`QYM_WELD`, mm).
                let grid: f64 = std::env::var("QYM_WELD").ok().and_then(|s| s.parse().ok()).unwrap_or(1e-4);
                let key = |v: qymcad_core::geom::Point3| [(v.x / grid).round() as i64, (v.y / grid).round() as i64, (v.z / grid).round() as i64];
                let mut at: std::collections::HashMap<[i64; 3], u32> = std::collections::HashMap::new();
                let mut weld: Vec<u32> = Vec::with_capacity(b.mesh.verts.len());
                for v in &b.mesh.verts {
                    let n = at.len() as u32;
                    weld.push(*at.entry(key(*v)).or_insert(n));
                }
                let mut edges: std::collections::HashMap<(u32, u32), i32> = std::collections::HashMap::new();
                for t in &b.mesh.tris {
                    for (a, c) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                        let (a, c) = (weld[a as usize], weld[c as usize]);
                        *edges.entry((a.min(c), a.max(c))).or_insert(0) += 1;
                    }
                }
                let open: Vec<(u32, u32)> = edges.iter().filter(|(_, &n)| n != 2).map(|(e, _)| *e).collect();
                if !open.is_empty() {
                    // the length of the open edges tells a missing FACE (millimetres) from a seam that merely
                    // failed to weld (microns)
                    let pos: Vec<qymcad_core::geom::Point3> = {
                        let mut v = vec![qymcad_core::geom::Point3::new(0.0, 0.0, 0.0); at.len()];
                        for (k, p) in b.mesh.verts.iter().enumerate() {
                            v[weld[k] as usize] = *p;
                        }
                        v
                    };
                    let len = |e: &(u32, u32)| {
                        let (a, c) = (pos[e.0 as usize], pos[e.1 as usize]);
                        ((a.x - c.x).powi(2) + (a.y - c.y).powi(2) + (a.z - c.z).powi(2)).sqrt()
                    };
                    let longest = open.iter().map(len).fold(0.0_f64, f64::max);
                    open_bodies.push((i, open.len(), edges.len(), longest));
                }
            }
            open_bodies.sort_by(|a, b| b.3.total_cmp(&a.3));
            eprintln!("  bodies with an unclosed shell: {} of {}", open_bodies.len(), app.project.bodies.iter().filter(|b| b.visible && !b.mesh.tris.is_empty()).count());
            for (i, open, all, longest) in open_bodies.iter().take(8) {
                eprintln!("    body {i}: {open} edges of {all} belong to one triangle, the longest {longest:.3} mm ({})", app.project.mesh_name(*i));
            }
        }
        // A BODY WITH A HOLE, SAVED ASIDE AS A FIXTURE: experiments over one solid take a second, over the
        // whole document a minute and a half. `QYM_SAVE_BODY` names the mesh index.
        if let Ok(n) = std::env::var("QYM_SAVE_BODY").map(|s| s.parse::<usize>().unwrap_or(0)) {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look");
            std::fs::create_dir_all(&dir).ok();
            // WHICH SOLID OF THE SOURCE FILE this body came from: an import node names the source and the
            // index of the solid inside it.
            let id = app.project.mesh_id(n).expect("the body has an id");
            let found = app.project.timeline.iter().find_map(|node| match node.kind {
                qymcad_core::feature::FeatureKind::Import { body, source, solid, .. } if body == id => Some((source, solid)),
                _ => None,
            });
            match found {
                Some((source, solid)) => {
                    let src = app.project.sources.iter().find(|s| s.id == source).expect("the embedded source file");
                    let tmp = dir.join(format!("source-{source}.{}", if src.ext.is_empty() { "step" } else { src.ext.as_str() }));
                    std::fs::write(&tmp, &src.data).expect("writing the source out");
                    let solids = qymcad_kernel::step_solids(tmp.to_str().expect("the path")).expect("the source parses");
                    eprintln!("  the source holds {} solids, this body is number {solid}", solids.len());
                    match solids.get(solid as usize).and_then(|sh| sh.to_brep_bytes()) {
                        Some(bytes) => {
                            let out = dir.join(format!("body-{n}.brep"));
                            std::fs::write(&out, &bytes).expect("writing the fixture");
                            eprintln!("  body {n} saved as {} ({} KB)", out.display(), bytes.len() / 1024);
                        }
                        None => eprintln!("  solid {solid} could not be written out"),
                    }
                    std::fs::remove_file(&tmp).ok();
                }
                None => eprintln!("  body {n} is not an import - there is no source to take it from"),
            }
        }
        let looks = crate::gui::render_scene::scene_looks(&app.painting());
        let ghosts = looks.iter().filter(|l| l.state & qymcad_ui_state::LOOK_GHOST != 0).count();
        eprintln!("  drawn {} bodies, of them ghosts {ghosts}", looks.len());
        app.viewing.cam.yaw += std::f64::consts::FRAC_PI_2;
        fit(&mut app);
        shot(&mut app, "doc-turned.png");
        app.viewing.cam.pitch = 1.2;
        fit(&mut app);
        shot(&mut app, "doc-top.png");
    }
}
