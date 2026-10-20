//! A STEP ASSEMBLY LANDS AS ITS AUTHOR BUILT IT, through the door a person opens it by.
//!
//! Reported behaviour: a print head from STEP came in as a flat list of numbered parts in one subassembly, every body
//! baked where it stands, where another CAD laid it out as its author did.
//!
//! The reference is the kernel's `tests/data/assembly.step`: an assembly holding a plate twice (at x 0 and at x 30)
//! and a subassembly at (5, 10, 0) holding a pin at z 5. A plate is a 10 x 20 x 5 box.
#[cfg(test)]
mod tests {
    use crate::gui::a_component_stepped_into_is_not_lit::tests::{brought_in, calm};
    use crate::gui::import_door::tests::{answer, click, frame, key, running, settle, spot};
    use crate::gui::App;
    use qymcad_core::feature::ComponentKind;
    use qymcad_core::model::Id;
    use qymcad_ui_state::Want;

    /// The names the reference was written with: the assembly, the plate, the subassembly, the pin.
    const NAMES: &str = include_str!("../../../qymcad-kernel/tests/data/assembly.names");

    fn name(k: usize) -> &'static str {
        NAMES.lines().nth(k).expect("a name of the reference")
    }

    fn reference() -> String {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../qymcad-kernel/tests/data/assembly.step").to_string()
    }

    fn named(app: &App, name: &str) -> Vec<Id> {
        app.project.components.iter().filter(|c| c.name == name).map(|c| c.id).collect()
    }

    fn under(app: &App, id: Id) -> Vec<(String, ComponentKind)> {
        app.project.components.iter().filter(|c| c.parent == Some(id)).map(|c| (c.name.clone(), c.kind)).collect()
    }

    /// The one body of part `part`, and where the part's zero stands in the world.
    fn body_at(app: &App, part: Id) -> (Id, [f64; 3]) {
        let bodies = app.project.component_bodies(part);
        assert_eq!(bodies.len(), 1, "a part of the import holds {} bodies", bodies.len());
        let w = app.project.body_world_transform(bodies[0]);
        (bodies[0], [w[3], w[7], w[11]])
    }

    fn box_of(app: &App, body: Id) -> [f64; 6] {
        let b = app.project.mesh_index(body).and_then(|i| app.project.bodies[i].mesh.bounds()).expect("a body has a mesh");
        [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z]
    }

    fn near(a: &[f64], b: &[f64], tol: f64) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
    }

    /// The name component `id` carries.
    fn name_of(app: &App, id: Id) -> String {
        app.project.components.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_default()
    }

    /// THE NAME THE REFERENCE'S ASSEMBLY LANDS UNDER: its own, numbered apart when the root of the document reads the
    /// same - which it does in a Russian window, the reference being named in Russian. Measured: in a Russian window the
    /// assembly landed as the name with " (2)", and every check looking it up by the bare name found nothing.
    fn landed_name(app: &App) -> String {
        let root = crate::i18n::name(&name_of(app, app.project.root));
        if root == name(0) {
            format!("{} (2)", name(0))
        } else {
            name(0).to_string()
        }
    }

    #[test]
    fn a_step_assembly_lands_as_its_tree() {
        let (mut app, ctx) = running();
        let steps = app.disk.edits.undo.len();
        let root = brought_in(&mut app, &ctx, &reference());
        assert_eq!(name_of(&app, root), landed_name(&app), "the subassembly does not land under the file's name");
        assert_eq!(under(&app, root), [(name(1).to_string(), ComponentKind::Part), (name(1).to_string(), ComponentKind::Part), (name(2).to_string(), ComponentKind::Assembly)]);
        let unit = named(&app, name(2))[0];
        assert_eq!(under(&app, unit), [(name(3).to_string(), ComponentKind::Part)]);
        // THE SAME PLATE TWICE: the second is a clone of the first, as the file holds the plate once
        let two = named(&app, name(1));
        assert_eq!(app.project.instance_origin(two[1]), two[0], "the second plate is not a clone of the first");
        let plates: Vec<(Id, [f64; 3])> = named(&app, name(1)).into_iter().map(|p| body_at(&app, p)).collect();
        assert!(plates.iter().any(|(_, at)| near(at, &[0.0, 0.0, 0.0], 1e-9)) && plates.iter().any(|(_, at)| near(at, &[30.0, 0.0, 0.0], 1e-9)), "the plates stand at {plates:?}");
        for (body, _) in &plates {
            assert!(near(&box_of(&app, *body), &[0.0, 0.0, 0.0, 10.0, 20.0, 5.0], 1e-6), "a plate's body is baked where it stands: {:?}", box_of(&app, *body));
        }
        let (_, pin) = body_at(&app, named(&app, name(3))[0]);
        assert!(near(&pin, &[5.0, 10.0, 5.0], 1e-9), "the pin stands at {pin:?}, not (5, 10, 5)");
        assert_eq!(app.disk.edits.undo.len(), steps + 1, "the import is not one step of undo");
        // THE AUTHOR'S COLOURS: the plates red, the pin blue
        let colour = |body: Id| app.project.mesh_color(app.project.mesh_index(body).expect("a body"));
        for (body, _) in &plates {
            assert_eq!(colour(*body), [204, 26, 26], "a plate did not come in red");
        }
        let (pin_body, _) = body_at(&app, named(&app, name(3))[0]);
        assert_eq!(colour(pin_body), [26, 51, 230], "the pin did not come in blue");
    }

    /// AND GOES OUT AS IT CAME IN: the subassembly exported from its row's menu is written as its tree - the parts
    /// under their names and colours, each in its place, the plate one product placed twice - and reads back the same.
    #[test]
    fn a_step_assembly_goes_out_as_it_came_in() {
        let (mut app, ctx) = running();
        let root = brought_in(&mut app, &ctx, &reference());
        let (bodies, back) = exported(&mut app, &ctx, root, "door-out.step");
        let roots: Vec<usize> = (0..back.len()).filter(|&i| back[i].parent.is_none()).collect();
        assert_eq!(
            roots.iter().map(|&i| back[i].name.as_str()).collect::<Vec<_>>(),
            [landed_name(&app)],
            "the subassembly does not go out under its name; the file holds {:?}",
            back.iter().map(|n| &n.name).collect::<Vec<_>>()
        );
        let under = |p: usize| back.iter().filter(|n| n.parent == Some(p)).map(|n| n.name.as_str()).collect::<Vec<_>>();
        assert_eq!(under(roots[0]), [name(1), name(1), name(2)], "the tree does not go out as it stands");
        let unit = (0..back.len()).find(|&i| back[i].name == name(2)).expect("the subassembly inside");
        assert_eq!(under(unit), [name(3)]);
        assert_eq!(bodies.len(), 2, "the plate goes out twice, not as one product placed twice");
        let pin = (0..back.len()).find(|&i| back[i].name == name(3)).expect("the pin");
        let near = |c: Option<[f32; 3]>, want: [f32; 3]| c.is_some_and(|c| c.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.01));
        assert!(near(back[pin].color, [0.1, 0.2, 0.9]), "the pin goes out coloured {:?}, not blue", back[pin].color);
        assert!(app.live.shapes.len() >= 3, "the bodies did not come back to the cache after the write");
    }

    /// AN ASSEMBLY NAMED AS THE ROOT OF THE DOCUMENT READS IS NUMBERED APART, and goes out under the name it carries: in
    /// a Russian window the root reads the very name the reference's assembly carries, and two rows reading the same
    /// could not be told apart.
    #[test]
    fn an_assembly_named_as_the_root_reads_is_numbered_apart() {
        let was = qymcad_i18n::language();
        qymcad_i18n::set_language("ru");
        let (mut app, ctx) = running();
        let reads = crate::i18n::name(&name_of(&app, app.project.root));
        let root = brought_in(&mut app, &ctx, &reference());
        let carried = name_of(&app, root);
        let (_, back) = exported(&mut app, &ctx, root, "door-out-numbered.step");
        qymcad_i18n::set_language(&was);
        assert_eq!(reads, name(0), "GUARD: the root of the document reads the name of the reference's assembly");
        assert_eq!(carried, format!("{} (2)", name(0)), "the assembly is not numbered apart from the root reading the same");
        let roots: Vec<&str> = back.iter().filter(|n| n.parent.is_none()).map(|n| n.name.as_str()).collect();
        assert_eq!(roots, [carried.as_str()], "the assembly does not go out under the name it carries");
    }

    /// Component `id` written into `target/step-export/<file>` as its row's menu writes it, and the file read back: its
    /// bodies and its tree. Only the system's chooser is stood in for, answered with the path.
    fn exported(app: &mut App, ctx: &egui::Context, id: Id, file: &str) -> (Vec<qymcad_core::geom::Built>, Vec<qymcad_kernel::ImportNode>) {
        let target = qymcad_ui_state::ExportTarget::Component(id);
        let format = qymcad_kernel::ExactFormat::Step;
        let plan = app.export_plan(target);
        let job = crate::gui::io_jobs::ExportJob { format, tree: crate::gui::io_jobs::export_tree_of(&app.project, format, target, &plan.brep), bodies: plan.brep.clone(), note: plan.note(true) };
        let out = saved(app, ctx, file, move |app, path| crate::gui::io_jobs::write_exact_to(&mut app.live, &mut app.project, &mut app.regen, &mut app.status, &path, &job));
        let text = std::fs::read(&out).unwrap_or_else(|e| panic!("nothing written: {e}; the status: {}", app.status));
        assert!(text.iter().all(|b| b.is_ascii()), "the names go out as raw UTF-8");
        let qymcad_kernel::ExactTree { bodies, nodes: back, .. } =
            qymcad_kernel::read_exact_tree(format, &out.to_string_lossy(), 0.5).unwrap_or_else(|e| panic!("the file written does not read back: {e}"));
        (bodies, back)
    }

    /// The save chooser answered with `target/step-export/<file>` and the write that follows waited out. `then` is what
    /// the menu hands the chooser.
    fn saved(app: &mut App, ctx: &egui::Context, file: &str, then: impl FnOnce(&mut App, std::path::PathBuf) + 'static) -> std::path::PathBuf {
        let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/step-export"));
        std::fs::create_dir_all(&dir).expect("a folder for the check");
        let out = dir.join(file);
        let _ = std::fs::remove_file(&out);
        let (tx, rx) = std::sync::mpsc::channel();
        app.arm_file_ask(rx, then);
        tx.send(Some(out.clone())).expect("the chooser's channel is open");
        let _ = frame(app, ctx, Vec::new());
        settle(app, ctx);
        out
    }

    /// Component `id` written as a mesh file from its row's menu and read back by the door's own reader.
    fn exported_mesh(app: &mut App, ctx: &egui::Context, id: Id, format: qymcad_ui_state::MeshFormat, file: &str) -> Vec<qymcad_io::NamedMesh> {
        let target = qymcad_ui_state::ExportTarget::Component(id);
        let plan = app.export_plan(target);
        let (bodies, note) = (plan.stl_bodies(), plan.note(false));
        let out = saved(app, ctx, file, move |app, path| {
            let job = crate::gui::io_jobs::mesh_job(&app.project, format, target, bodies, note, 0.1);
            crate::gui::io_jobs::write_mesh_to(qymcad_ui_state::editing_of!(app), &mut app.live, &path, &job)
        });
        let p = out.to_string_lossy();
        let back = match format {
            qymcad_ui_state::MeshFormat::ThreeMf => qymcad_io::import_3mf(&p),
            _ => qymcad_io::import_gltf(&p),
        };
        back.unwrap_or_else(|e| panic!("the file written does not read back: {e}; the status: {}", app.status))
    }

    /// The box a piece read back stands in, in the world: its mesh put at its place, then at the places of the groups
    /// of the file it stands in.
    fn world_box(piece: &qymcad_io::NamedMesh) -> [f64; 6] {
        let mut mesh = piece.mesh.clone();
        mesh.transform(&piece.place);
        for place in piece.within.iter().rev().map(|g| &g.place) {
            mesh.transform(place);
        }
        let b = mesh.bounds().expect("a mesh");
        [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z]
    }

    /// AND GOES OUT TO glTF AS ITS TREE: the subassembly exported as GLB from its row's menu reads back a piece a
    /// part, under the part's name, in its colour, where it stands.
    #[test]
    fn a_step_assembly_goes_out_to_gltf_as_its_tree() {
        let (mut app, ctx) = running();
        let root = brought_in(&mut app, &ctx, &reference());
        let back = exported_mesh(&mut app, &ctx, root, qymcad_ui_state::MeshFormat::Glb, "door-out.glb");
        // THE PLATE'S GREEN TOP FACE GOES OUT GREEN, the rest of it red
        let green = back[0].tri_colors.iter().filter(|c| **c == [26, 204, 26]).count();
        let red = back[0].tri_colors.iter().filter(|c| **c == [204, 26, 26]).count();
        assert!(green > 0 && red > green, "the plate goes out with {green} green and {red} red triangles, not its green top face on red");
        assert_eq!(back.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), [name(1), name(1), name(3)], "the parts do not go out under their names");
        assert_eq!(back.iter().map(|p| p.color).collect::<Vec<_>>(), [Some([204, 26, 26]), Some([204, 26, 26]), Some([26, 51, 230])], "the parts do not go out in their colours");
        let bounds = |k: usize| world_box(&back[k]);
        assert!(
            (0..2).any(|k| near(&bounds(k), &[0.0, 0.0, 0.0, 10.0, 20.0, 5.0], 1e-3)) && (0..2).any(|k| near(&bounds(k), &[30.0, 0.0, 0.0, 40.0, 20.0, 5.0], 1e-3)),
            "the plates go out at {:?} and {:?}",
            bounds(0),
            bounds(1)
        );
        assert!(near(&bounds(2), &[1.0, 6.0, 5.0, 9.0, 14.0, 17.0], 0.15), "the pin goes out at {:?}", bounds(2));
        // BACK IN BY THE DOOR as the tree the file holds: the subassembly with its two plates and the unit holding the
        // pin, every part where it stands. The subassembly is found as the last one in, not by its name, which the
        // original shares.
        answer(&mut app, &ctx, Want::Anything, concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/step-export/door-out.glb"));
        settle(&mut app, &ctx);
        calm(&mut app, &ctx);
        let top = app.project.components.iter().filter(|c| c.parent == Some(app.project.root)).map(|c| c.id).next_back().expect("the file came in");
        // brought back beside the same parts, they come in numbered "(2)"; the tree is checked by the names without it
        let bare = |n: &str| n.rsplit_once(" (").filter(|(_, k)| k.ends_with(')') && k[..k.len() - 1].chars().all(|c| c.is_ascii_digit())).map_or(n.to_string(), |(b, _)| b.to_string());
        let tree: Vec<(String, ComponentKind)> = under(&app, top).into_iter().map(|(n, k)| (bare(&n), k)).collect();
        assert_eq!(
            tree,
            [(name(1).to_string(), ComponentKind::Part), (name(1).to_string(), ComponentKind::Part), (name(2).to_string(), ComponentKind::Assembly)],
            "the file comes in as {tree:?} under {:?}",
            app.project.components.iter().find(|c| c.id == top).map(|c| &c.name)
        );
        let unit = app.project.components.iter().filter(|c| c.parent == Some(top)).map(|c| c.id).next_back().expect("the unit");
        let pin: Vec<(String, ComponentKind)> = under(&app, unit).into_iter().map(|(n, k)| (bare(&n), k)).collect();
        assert_eq!(pin, [(name(3).to_string(), ComponentKind::Part)]);
        let plates: Vec<[f64; 3]> = app.project.components.iter().filter(|c| c.parent == Some(top) && c.kind == ComponentKind::Part).map(|c| body_at(&app, c.id).1).collect();
        let pin = app.project.components.iter().find(|c| c.parent == Some(unit)).map(|c| body_at(&app, c.id).1).expect("the pin");
        assert!(near(&plates[0], &[0.0, 0.0, 0.0], 1e-6) && near(&plates[1], &[30.0, 0.0, 0.0], 1e-6) && near(&pin, &[5.0, 10.0, 5.0], 1e-6), "the plates come in at {plates:?}, the pin at {pin:?}");
    }

    /// AND GOES OUT TO 3MF AS ITS TREE: the subassembly exported as 3MF from its row's menu is objects of parts, each
    /// under its name, in its colour, where it stands, and the unit an object of its own - and comes back in by the door
    /// as the same levels: the plates and the unit under the file's name, the pin within the unit, each part at its own
    /// zero and placed by its component, not a mesh baked where it stood.
    #[test]
    fn a_step_assembly_goes_out_to_3mf_as_its_parts() {
        let (mut app, ctx) = running();
        let root = brought_in(&mut app, &ctx, &reference());
        let back = exported_mesh(&mut app, &ctx, root, qymcad_ui_state::MeshFormat::ThreeMf, "door-out.3mf");
        // THE PLATE'S GREEN TOP FACE GOES OUT GREEN, the rest of it red
        let green = back[0].tri_colors.iter().filter(|c| **c == [26, 204, 26]).count();
        let red = back[0].tri_colors.iter().filter(|c| **c == [204, 26, 26]).count();
        assert!(green > 0 && red > green, "the plate goes out with {green} green and {red} red triangles, not its green top face on red");
        assert_eq!(back.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), [name(1), name(1), name(3)], "the parts do not go out under their names");
        assert_eq!(back.iter().map(|p| p.color).collect::<Vec<_>>(), [Some([204, 26, 26]), Some([204, 26, 26]), Some([26, 51, 230])], "the parts do not go out in their colours");
        let bounds = |k: usize| world_box(&back[k]);
        assert!(
            (0..2).any(|k| near(&bounds(k), &[0.0, 0.0, 0.0, 10.0, 20.0, 5.0], 1e-6)) && (0..2).any(|k| near(&bounds(k), &[30.0, 0.0, 0.0, 40.0, 20.0, 5.0], 1e-6)),
            "the plates go out at {:?} and {:?}",
            bounds(0),
            bounds(1)
        );
        assert!(near(&bounds(2), &[1.0, 6.0, 5.0, 9.0, 14.0, 17.0], 0.15), "the pin goes out at {:?}", bounds(2));
        // BACK IN BY THE DOOR: a subassembly under the file's name, its parts where the components place them
        answer(&mut app, &ctx, Want::Anything, concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/step-export/door-out.3mf"));
        settle(&mut app, &ctx);
        calm(&mut app, &ctx);
        let sub =
            *named(&app, "door-out").first().unwrap_or_else(|| panic!("no subassembly under the file's name; the tree: {:?}", app.project.components.iter().map(|c| &c.name).collect::<Vec<_>>()));
        // brought back into the document that holds the same parts, they come in numbered - "Plate (2)" - to be told
        // apart from those already here; the tree is what is checked, under the names without the number
        let bare = |n: &str| n.rsplit_once(" (").filter(|(_, k)| k.ends_with(')') && k[..k.len() - 1].chars().all(|c| c.is_ascii_digit())).map_or(n.to_string(), |(b, _)| b.to_string());
        let unnumbered = |v: Vec<(String, ComponentKind)>| v.into_iter().map(|(n, k)| (bare(&n), k)).collect::<Vec<_>>();
        assert!(under(&app, sub).iter().all(|(n, _)| bare(n) != *n), "the parts brought back beside their namesakes are not numbered apart: {:?}", under(&app, sub));
        assert_eq!(unnumbered(under(&app, sub)), [(name(1).to_string(), ComponentKind::Part), (name(1).to_string(), ComponentKind::Part), (name(2).to_string(), ComponentKind::Assembly)]);
        let unit = app.project.components.iter().find(|c| c.parent == Some(sub) && bare(&c.name) == name(2)).map(|c| c.id).expect("the unit");
        assert_eq!(unnumbered(under(&app, unit)), [(name(3).to_string(), ComponentKind::Part)], "the pin does not come in within the unit");
        let mut parts: Vec<(Id, [f64; 3])> = app.project.components.iter().filter(|c| c.parent == Some(sub) && c.id != unit).map(|c| body_at(&app, c.id)).collect();
        parts.extend(app.project.components.iter().filter(|c| c.parent == Some(unit)).map(|c| body_at(&app, c.id)));
        assert!(near(&parts[0].1, &[0.0, 0.0, 0.0], 1e-9) && near(&parts[1].1, &[30.0, 0.0, 0.0], 1e-9) && near(&parts[2].1, &[5.0, 10.0, 5.0], 1e-9), "the parts come in at {parts:?}");
        assert!(near(&box_of(&app, parts[1].0), &[0.0, 0.0, 0.0, 10.0, 20.0, 5.0], 1e-6), "the second plate is baked where it stands: {:?}", box_of(&app, parts[1].0));
        let colour = |body: Id| app.project.mesh_color(app.project.mesh_index(body).expect("a body"));
        assert_eq!([colour(parts[0].0), colour(parts[2].0)], [[204, 26, 26], [26, 51, 230]], "the parts do not come in in their colours");
    }

    /// AN IGES ASSEMBLY LANDS THE SAME WAY: the reference written as subfigures (308/408), names in Windows-1251 and
    /// colours on faces, the way the print head of the report is written in IGES.
    #[test]
    fn an_iges_assembly_lands_as_its_tree() {
        let (mut app, ctx) = running();
        let root = brought_in(&mut app, &ctx, concat!(env!("CARGO_MANIFEST_DIR"), "/../qymcad-kernel/tests/data/assembly.igs"));
        assert_eq!(name_of(&app, root), landed_name(&app), "the subassembly does not land under the file's name");
        assert_eq!(under(&app, root), [(name(1).to_string(), ComponentKind::Part), (name(1).to_string(), ComponentKind::Part), (name(2).to_string(), ComponentKind::Assembly)]);
        let two = named(&app, name(1));
        assert_eq!(app.project.instance_origin(two[1]), two[0], "the second plate is not a clone of the first");
        let plates: Vec<(Id, [f64; 3])> = two.iter().map(|&p| body_at(&app, p)).collect();
        assert!(plates.iter().any(|(_, at)| near(at, &[30.0, 0.0, 0.0], 1e-9)), "the plates stand at {plates:?}");
        for (body, _) in &plates {
            assert!(near(&box_of(&app, *body), &[0.0, 0.0, 0.0, 10.0, 20.0, 5.0], 1e-6), "a plate's body is baked where it stands: {:?}", box_of(&app, *body));
        }
        let (pin_body, pin) = body_at(&app, named(&app, name(3))[0]);
        assert!(near(&pin, &[5.0, 10.0, 5.0], 1e-9), "the pin stands at {pin:?}, not (5, 10, 5)");
        let colour = |body: Id| app.project.mesh_color(app.project.mesh_index(body).expect("a body"));
        assert_eq!(colour(plates[0].0), [204, 26, 26], "a plate did not come in red from the colours of its faces");
        assert_eq!(colour(pin_body), [26, 51, 230], "the pin did not come in blue");
    }

    /// THE FACTOR SCALES WHERE THE PARTS STAND, not only the parts: the whole assembly grows about the file's zero.
    #[test]
    fn a_scaled_assembly_keeps_its_shape() {
        let (mut app, ctx) = running();
        app.set.import_ask_always = true;
        answer(&mut app, &ctx, Want::Anything, &reference());
        settle(&mut app, &ctx);
        let _ = frame(&mut app, &ctx, Vec::new()); // a window settles on the second pass
        let texts = frame(&mut app, &ctx, Vec::new());
        let ten = spot(&texts, "10").unwrap_or_else(|| panic!("the window does not offer 10; the screen shows: {:?}", texts.iter().map(|(t, _)| t).collect::<Vec<_>>()));
        let _ = frame(&mut app, &ctx, click(ten));
        settle(&mut app, &ctx);
        calm(&mut app, &ctx);
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
        let _ = frame(&mut app, &ctx, Vec::new());
        let plates: Vec<(Id, [f64; 3])> = named(&app, name(1)).into_iter().map(|p| body_at(&app, p)).collect();
        assert!(plates.iter().any(|(_, at)| near(at, &[300.0, 0.0, 0.0], 1e-6)), "the second plate stands at {plates:?}, not at x 300");
        for (body, _) in &plates {
            assert!(near(&box_of(&app, *body), &[0.0, 0.0, 0.0, 100.0, 200.0, 50.0], 1e-3), "a plate is {:?} at ten times", box_of(&app, *body));
        }
        let (_, pin) = body_at(&app, named(&app, name(3))[0]);
        assert!(near(&pin, &[50.0, 100.0, 50.0], 1e-6), "the pin stands at {pin:?}, not (50, 100, 50)");
    }

    /// THE PRINT HEAD OF THE REPORT, SEEN, where its file is at hand: `QYM_CONDOR` names it. The tree is printed with
    /// where every component stands, the whole program goes into `target/look/condor.window.png` and the view
    /// through the graphics device into `condor.view.png`, to lay beside the other CAD's.
    #[test]
    #[ignore = "the owner's file"]
    fn the_print_head_is_looked_at() {
        let Ok(path) = std::env::var("QYM_CONDOR") else { return };
        let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/look"));
        std::fs::create_dir_all(&dir).expect("a folder for the pictures");
        let (mut app, ctx) = running();
        answer(&mut app, &ctx, Want::Anything, &path);
        settle(&mut app, &ctx);
        calm(&mut app, &ctx); // the clones' bodies are built by the rebuild after the reading
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter)); // the window about the scale, where it came up, as the file has it
        let _ = frame(&mut app, &ctx, Vec::new());
        fn print(app: &App, id: Id, depth: usize) {
            let c = app.project.components.iter().find(|c| c.id == id).expect("a component");
            let t = c.transform;
            let clone = if app.project.instance_origin(id) != id { ", a clone" } else { "" };
            eprintln!("{}{} [{:?}, {} bodies{clone}] at ({:.1}, {:.1}, {:.1})", "  ".repeat(depth), c.name, c.kind, app.project.component_bodies(id).len(), t[3], t[7], t[11]);
            for k in app.project.component_children(id) {
                print(app, k, depth + 1);
            }
        }
        print(&app, app.project.root, 0);
        window(&mut app, &dir.join("condor.window.png"));
        let view = app.viewing.view_rect;
        if let Some(img) = crate::gui::gpu_shot::eyes::shot(&app.painting(), egui::Rect::from_min_size(egui::pos2(0.0, 0.0), view.size())) {
            std::fs::write(dir.join("condor.view.png"), crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
        }
        // INTO THE PRINT HEAD by a double click on its row, as a person goes into a subassembly: the tree then shows
        // its first level
        // the head as a subassembly, or as one part where the file holds no tree (a mesh: PLY, STL)
        let head = app
            .project
            .components
            .iter()
            .filter(|c| c.parent == Some(app.project.root))
            .find(|c| c.kind == ComponentKind::Assembly || !app.project.component_bodies(c.id).is_empty())
            .map(|c| c.name.clone())
            .expect("the head came in");
        let texts = frame(&mut app, &ctx, Vec::new());
        let row = spot(&texts, &head).unwrap_or_else(|| panic!("no row {head:?} in the tree"));
        let _ = frame(&mut app, &ctx, click(row));
        let _ = frame(&mut app, &ctx, click(row));
        let _ = frame(&mut app, &ctx, Vec::new());
        let _ = frame(&mut app, &ctx, Vec::new());
        let inside = qymcad_ui_state::current_ctx_id(&app.active_path, &app.project);
        eprintln!("inside: {:?}", app.project.components.iter().find(|c| c.id == inside).map(|c| &c.name));
        window(&mut app, &dir.join("condor.inside.png"));
    }

    /// THE PRINT HEAD GOES OUT AS IT CAME IN, where its file is at hand (`QYM_CONDOR`): written from its row, it reads
    /// back with the same first level under the same names, as many parts, and a product per part that is no clone.
    #[test]
    #[ignore = "the owner's file"]
    fn the_print_head_goes_out_as_it_came_in() {
        let Ok(path) = std::env::var("QYM_CONDOR") else { return };
        let (mut app, ctx, head) = the_head(&path);
        let started = std::time::Instant::now();
        let (bodies, back) = exported(&mut app, &ctx, head, "condor-out.step");
        eprintln!("written and read back in {:.1} s", started.elapsed().as_secs_f64());
        let mut ours = Vec::new();
        parts(&app, head, &mut ours);
        let originals = ours.iter().filter(|&&p| app.project.instance_origin(p) == p).count();
        let root = (0..back.len()).find(|&i| back[i].parent.is_none()).expect("a root in the file");
        let first: Vec<String> = back.iter().filter(|n| n.parent == Some(root)).map(|n| n.name.clone()).collect();
        let want: Vec<String> = under(&app, head).into_iter().map(|(n, _)| n).collect();
        let leaves = back.iter().filter(|n| n.solid.is_some() || n.repeat_of.is_some()).count(); // a part met again repeats the first
        let mut colours: Vec<[u8; 3]> = back.iter().filter_map(|n| n.color).map(|c| c.map(|v| (v * 255.0).round() as u8)).collect();
        colours.sort();
        colours.dedup();
        let mut our_colours: Vec<[u8; 3]> = ours.iter().flat_map(|&p| app.project.component_bodies(p)).filter_map(|b| app.project.mesh_index(b)).map(|i| app.project.mesh_color(i)).collect();
        our_colours.sort();
        our_colours.dedup();
        eprintln!(
            "ours: {} parts, {} of them no clone, colours {our_colours:?}; the file: {} nodes, {} parts, {} bodies, colours {colours:?}; the first level: {:?}",
            ours.len(),
            originals,
            back.len(),
            leaves,
            bodies.len(),
            first
        );
        assert_eq!(first, want, "the first level does not go out as it stands");
        assert_eq!(leaves, ours.len(), "not every part goes out");
        assert_eq!(bodies.len(), originals, "a clone goes out as a product of its own, or a part is missing");
        assert_eq!(colours, our_colours, "the colours do not go out as they stand");
        // THE FACES OF A COLOUR OF THEIR OWN go out as many as the parts hold: a clone repeats its original's product
        let our_faces: usize = ours
            .iter()
            .filter(|&&p| app.project.instance_origin(p) == p)
            .filter_map(|&p| app.project.component_bodies(p).first().copied())
            .map(|b| app.project.face_colors.get(&app.project.lineage_root(b)).map_or(0, Vec::len))
            .sum();
        let their_faces: usize = back.iter().map(|n| n.faces.len()).sum();
        eprintln!("faces of a colour of their own: {our_faces} in the document, {their_faces} back from the file");
        assert_eq!(their_faces, our_faces, "the faces of a colour of their own do not go out as they stand");
        // EVERY PART IN ITS PLACE: the whole placement, matched to the nearest the file holds, not an offset rounded for print
        let head_at = app.project.world_transform(head);
        assert!(head_at.iter().zip(qymcad_core::feature::PLACE_IDENTITY).all(|(g, w)| (g - w).abs() < 1e-9), "the head does not stand at the world's zero: {head_at:?}");
        let world = |i: usize| {
            let (mut m, mut at) = (back[i].place, back[i].parent);
            while let Some(p) = at {
                m = qymcad_core::feature::compose12(&back[p].place, &m);
                at = back[p].parent;
            }
            m
        };
        let mut placed: Vec<[f64; 12]> = (0..back.len()).filter(|&i| back[i].solid.is_some() || back[i].repeat_of.is_some()).map(world).collect();
        let mut worst = 0.0f64;
        for &p in &ours {
            let m = app.project.world_transform(p);
            let (k, d) =
                placed.iter().enumerate().map(|(k, t)| (k, t.iter().zip(&m).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max))).min_by(|a, b| a.1.total_cmp(&b.1)).expect("a part left in the file");
            worst = worst.max(d);
            placed.swap_remove(k);
        }
        eprintln!("the worst placement apart: {worst:e}");
        assert!(worst < 1e-6, "a part goes out {worst} away from where it stands");
    }

    /// Every part under `id`, however deep, in the tree's order.
    fn parts(app: &App, id: Id, out: &mut Vec<Id>) {
        for k in app.project.component_children(id) {
            match app.project.components.iter().find(|c| c.id == k).map(|c| c.kind) {
                Some(ComponentKind::Part) => out.push(k),
                _ => parts(app, k, out),
            }
        }
    }

    /// The print head as it came in (`QYM_CONDOR`), the scale window answered as the file has it; and the head.
    fn the_head(path: &str) -> (App, egui::Context, Id) {
        let (mut app, ctx) = running();
        answer(&mut app, &ctx, Want::Anything, path);
        settle(&mut app, &ctx);
        calm(&mut app, &ctx);
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
        let _ = frame(&mut app, &ctx, Vec::new());
        let head = app.project.components.iter().find(|c| c.parent == Some(app.project.root) && c.kind == ComponentKind::Assembly).map(|c| c.id).expect("the head came in");
        (app, ctx, head)
    }

    /// THE PRINT HEAD GOES OUT TO glTF, where its file is at hand (`QYM_CONDOR`): written from its row as GLB, it reads
    /// back a piece a part, under the same names, in the same colours, every part where it stands. Where is told by
    /// the box of the part's mesh in the world: the file's mesh is cut finer than the document's, so to a millimetre.
    #[test]
    #[ignore = "the owner's file"]
    fn the_print_head_goes_out_to_gltf() {
        the_print_head_goes_out(qymcad_ui_state::MeshFormat::Glb, "condor-out.glb");
    }

    /// THE PRINT HEAD GOES OUT TO 3MF the same way: an object of its parts, each named, coloured, placed.
    #[test]
    #[ignore = "the owner's file"]
    fn the_print_head_goes_out_to_3mf() {
        the_print_head_goes_out(qymcad_ui_state::MeshFormat::ThreeMf, "condor-out.3mf");
    }

    /// The head written as mesh file `file` from its row and read back: a piece a part, under the parts' names, in
    /// their colours, each where the part stands, showing the colours the part shows: its faces' own, its body's where a
    /// face has none.
    fn the_print_head_goes_out(format: qymcad_ui_state::MeshFormat, file: &str) {
        let Ok(path) = std::env::var("QYM_CONDOR") else { return };
        let glb = matches!(format, qymcad_ui_state::MeshFormat::Glb);
        let (mut app, ctx, head) = the_head(&path);
        let back = exported_mesh(&mut app, &ctx, head, format, file);
        let mut ours = Vec::new();
        parts(&app, head, &mut ours);
        let name_of = |p: Id| app.project.components.iter().find(|c| c.id == p).map(|c| c.name.clone()).unwrap_or_default();
        let (mut want, mut got): (Vec<String>, Vec<String>) = (ours.iter().map(|&p| name_of(p)).collect(), back.iter().map(|b| b.name.clone()).collect());
        want.sort();
        got.sort();
        assert_eq!(got, want, "the parts do not go out under their names");
        let part_box = |p: Id| {
            let b = app.project.component_bodies(p)[0];
            let mut mesh = app.project.bodies[app.project.mesh_index(b).expect("a body")].mesh.clone();
            mesh.transform(&app.project.world_transform(p));
            let bb = mesh.bounds().expect("a mesh");
            [bb.min.x, bb.min.y, bb.min.z, bb.max.x, bb.max.y, bb.max.z]
        };
        let colour_of = |p: Id| app.project.mesh_color(app.project.mesh_index(app.project.component_bodies(p)[0]).expect("a body"));
        // the colours a part shows: a face's own where it has one, the body's where not
        let shows = |p: Id| {
            let b = app.project.component_bodies(p)[0];
            let own = app.project.face_colors.get(&app.project.lineage_root(b)).cloned().unwrap_or_default();
            let mut cs: Vec<[u8; 3]> =
                app.project.bodies[app.project.mesh_index(b).expect("a body")].faces.iter().map(|f| own.iter().find(|(id, _)| *id == f.id).map_or(colour_of(p), |(_, c)| *c)).collect();
            cs.sort_unstable();
            cs.dedup();
            cs
        };
        // the colours a piece read back shows: its triangles', or its own where they have none
        let shown = |piece: &qymcad_io::NamedMesh| {
            let mut cs: Vec<[u8; 3]> = if piece.tri_colors.is_empty() { piece.color.into_iter().collect() } else { piece.tri_colors.clone() };
            cs.sort_unstable();
            cs.dedup();
            cs
        };
        eprintln!();
        let mut left: Vec<usize> = (0..back.len()).collect();
        let (mut worst, mut wrong_colour, mut wrong_faces, mut own_tris) = (0.0f64, 0usize, 0usize, 0usize);
        for &p in &ours {
            let want = part_box(p);
            let (k, d) = left
                .iter()
                .enumerate()
                .filter(|(_, &i)| back[i].name == name_of(p))
                .map(|(k, &i)| (k, world_box(&back[i]).iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .expect("a piece of the part's name left");
            worst = worst.max(d);
            let (piece, seen) = (&back[left[k]], shows(p));
            // glTF gives a mesh no colour but its primitives': a part no face of which shows its body's colour comes
            // back in a colour it shows; 3MF keeps the object's colour
            let colour_right = piece.color == Some(colour_of(p)) || (glb && !seen.contains(&colour_of(p)) && piece.color.is_some_and(|c| seen.contains(&c)));
            let faces_right = shown(piece) == seen;
            wrong_colour += usize::from(!colour_right);
            wrong_faces += usize::from(!faces_right);
            if !colour_right || !faces_right {
                eprintln!("  {:?}: its colour {:?}, it shows {seen:?}; back in {:?}, showing {:?}", name_of(p), colour_of(p), piece.color, shown(piece));
            }
            own_tris += piece.tri_colors.iter().filter(|c| Some(**c) != piece.color).count();
            left.swap_remove(k);
        }
        let faced_parts = ours.iter().filter(|&&p| shows(p) != [colour_of(p)]).count();
        eprintln!("{} parts, {} pieces back; the worst box apart: {worst:.3} mm; wrong colours: {wrong_colour}; parts with faces of their own colour: {faced_parts}, {own_tris} triangles of them back; pieces showing colours not their part's: {wrong_faces}", ours.len(), back.len());
        assert!(worst < 1.0, "a part goes out {worst} mm from where it stands");
        assert_eq!(wrong_colour, 0, "parts go out in colours not theirs");
        assert_eq!(wrong_faces, 0, "parts go out showing colours their part does not show");
    }

    /// The whole program into `file`, through the capture without a graphics device: right for the panels, not to be
    /// trusted for the 3D canvas.
    fn window(app: &mut App, file: &std::path::Path) {
        let bg = app.scheme.pal.viewport_bg();
        let pass = |app: &mut App| {
            crate::gui::help_raster::shot_ui([1400, 950], bg, |ui| {
                let ctx = &ui.ctx().clone();
                crate::gui::apply_theme(&mut app.scheme, &app.set, ctx);
                let shell = crate::gui::shell(&app.set);
                for slot in qymcad_shell::Slot::ORDER {
                    shell.run_slot(slot, ui, app);
                }
            })
        };
        let _ = pass(app); // the first pass only settles the canvas rectangle
        if app.viewing.view_rect.width() > 1.0 {
            crate::gui::fit3d(&mut app.viewing.cam, &app.project, app.viewing.view_rect);
        }
        app.cache.label_tex.borrow_mut().clear();
        let img = pass(app);
        std::fs::write(file, crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
    }

    /// AN IGES OF SURFACES LANDS WITH ITS NAMES AND COLOURS: the reference with no solid in it - every face a surface of
    /// its own, the parts as groups (`gen_iges --surfaces`) - comes through the door as the plate and the group holding
    /// the pin, each sewn into its body, in their colours, where the file puts them.
    #[test]
    fn an_iges_of_surfaces_lands_with_its_names_and_colours() {
        let (mut app, ctx) = running();
        answer(&mut app, &ctx, Want::Anything, concat!(env!("CARGO_MANIFEST_DIR"), "/../qymcad-kernel/tests/data/surfaces.igs"));
        settle(&mut app, &ctx);
        calm(&mut app, &ctx);
        let root =
            *named(&app, "surfaces").first().unwrap_or_else(|| panic!("no subassembly under the file's name; the tree: {:?}", app.project.components.iter().map(|c| &c.name).collect::<Vec<_>>()));
        assert_eq!(under(&app, root), [(name(1).to_string(), ComponentKind::Part), (name(2).to_string(), ComponentKind::Assembly)], "the tree under the file");
        assert_eq!(under(&app, named(&app, name(2))[0]), [(name(3).to_string(), ComponentKind::Part)], "the unit does not hold the pin");
        let (plate, _) = body_at(&app, named(&app, name(1))[0]);
        let (pin, _) = body_at(&app, named(&app, name(3))[0]);
        let colour = |body: Id| app.project.mesh_color(app.project.mesh_index(body).expect("a body"));
        assert_eq!([colour(plate), colour(pin)], [[204, 26, 26], [26, 51, 230]], "the parts do not come in in their colours");
        assert!(near(&box_of(&app, plate), &[0.0, 0.0, 0.0, 10.0, 20.0, 5.0], 0.05), "the plate is {:?}", box_of(&app, plate));
        assert!(near(&box_of(&app, pin)[..3], &[1.0, 6.0, 5.0], 0.05), "the pin stands at {:?}, not where the file puts it", box_of(&app, pin));
    }

    /// AN IGES OF SURFACES, SEEN: the whole program and the view through the graphics device, into
    /// `target/look/surfaces.window.png` and `surfaces.view.png`.
    #[test]
    #[ignore = "a picture to look at"]
    fn an_iges_of_surfaces_is_looked_at() {
        let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/look"));
        std::fs::create_dir_all(&dir).expect("a folder for the pictures");
        let (mut app, ctx) = running();
        answer(&mut app, &ctx, Want::Anything, concat!(env!("CARGO_MANIFEST_DIR"), "/../qymcad-kernel/tests/data/surfaces.igs"));
        settle(&mut app, &ctx);
        calm(&mut app, &ctx);
        window(&mut app, &dir.join("surfaces.window.png"));
        let view = app.viewing.view_rect;
        if let Some(img) = crate::gui::gpu_shot::eyes::shot(&app.painting(), egui::Rect::from_min_size(egui::pos2(0.0, 0.0), view.size())) {
            std::fs::write(dir.join("surfaces.view.png"), crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
        }
    }
}
