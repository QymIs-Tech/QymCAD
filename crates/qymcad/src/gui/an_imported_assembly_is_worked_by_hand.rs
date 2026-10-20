//! AN ASSEMBLY THAT CAME IN FROM A FILE IS WORKED BY HAND like one built here: a subassembly hidden by its tick in the
//! tree, a part renamed by F2, a mate put between parts of two different subassemblies. Every step goes through a real
//! frame, from the click, as a person takes it.
//!
//! The reference is the kernel's `tests/data/assembly.step`: an assembly holding a plate twice (at x 0 and at x 30) and
//! a subassembly at (5, 10, 0) holding a pin at z 5. A plate is a 10 x 20 x 5 box, the pin a cylinder of radius 4 and
//! height 12 standing on its base.
#[cfg(test)]
mod tests {
    use crate::gui::a_component_stepped_into_is_not_lit::tests::{brought_in, calm, double_click_at};
    use crate::gui::hand::Hand;
    use crate::gui::import_door::tests::{answer, click, frame, key, running, settle, spot};
    use crate::gui::{App, Sel};
    use qymcad_core::feature::JointKind;
    use qymcad_core::model::Id;
    use qymcad_ui_state::Want;

    /// The names the reference was written with: the assembly, the plate, the subassembly, the pin.
    const NAMES: &str = include_str!("../../../qymcad-kernel/tests/data/assembly.names");

    fn name(k: usize) -> &'static str {
        NAMES.lines().nth(k).expect("a name of the reference")
    }

    fn named(app: &App, name: &str) -> Vec<Id> {
        app.project.components.iter().filter(|c| c.name == name).map(|c| c.id).collect()
    }

    /// The reference come in by the door, and the tree standing inside it, entered by a double click on its row.
    fn inside_the_reference() -> (App, egui::Context) {
        let (mut app, ctx) = running();
        let asm = brought_in(&mut app, &ctx, concat!(env!("CARGO_MANIFEST_DIR"), "/../qymcad-kernel/tests/data/assembly.step"));
        // THE ROW AS IT READS: in a Russian window the root of the document reads the very name of the reference's
        // assembly, and the assembly is numbered apart from it - a double click on the bare name went into the root
        let reads = app.project.components.iter().find(|c| c.id == asm).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
        let texts = frame(&mut app, &ctx, Vec::new());
        let row = spot(&texts, &reads).unwrap_or_else(|| panic!("no row {reads:?} in the tree; the screen shows {:?}", texts.iter().map(|(t, _)| t).collect::<Vec<_>>()));
        double_click_at(&mut app, &ctx, row);
        let _ = frame(&mut app, &ctx, Vec::new());
        let _ = frame(&mut app, &ctx, Vec::new());
        assert_eq!(qymcad_ui_state::current_ctx_id(&app.active_path, &app.project), asm, "the double click did not go into the assembly");
        (app, ctx)
    }

    /// The row of the tree showing `name`: the rectangle of its words.
    fn row(texts: &[(String, egui::Rect)], name: &str) -> egui::Rect {
        texts.iter().find(|(t, _)| t == name).map(|(_, r)| *r).unwrap_or_else(|| panic!("no row {name:?}; the screen shows {:?}", texts.iter().map(|(t, _)| t).collect::<Vec<_>>()))
    }

    fn drawn(app: &App, body: Id) -> bool {
        app.project.mesh_index(body).is_some_and(|mi| qymcad_ui_state::body_shown(app.painting().body_view(), mi))
    }

    /// Where the topmost face of `body` has its centre in the world, and which way it looks.
    fn top_face(app: &App, body: Id) -> ([f64; 3], [f64; 3]) {
        let wt = app.project.body_world_transform(body);
        let f = app.project.regen_faces.get(&body).and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z))).expect("the body has faces");
        (qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z]), qymcad_core::feature::apply12_dir(&wt, f.normal))
    }

    /// A SUBASSEMBLY IS HIDDEN BY ITS TICK: a click on the tick of its row, and what it holds is drawn no longer.
    #[test]
    fn a_subassembly_is_hidden_by_its_tick() {
        let (mut app, ctx) = inside_the_reference();
        let unit = named(&app, name(2))[0];
        let pin = app.project.component_bodies(named(&app, name(3))[0])[0];
        let plate = app.project.component_bodies(named(&app, name(1))[0])[0];
        assert!(drawn(&app, pin), "GUARD: the pin is drawn before the tick");
        let texts = frame(&mut app, &ctx, Vec::new());
        let r = row(&texts, name(2));
        // the tick stands before the row's label: the label's padding, the gap, half the tick
        let sp = ctx.style_of(ctx.theme()).spacing.clone();
        let tick = egui::pos2(r.left() - sp.button_padding.x - sp.item_spacing.x - sp.interact_size.y / 2.0, r.center().y);
        let _ = frame(&mut app, &ctx, click(tick));
        let _ = frame(&mut app, &ctx, Vec::new());
        assert!(app.project.components.iter().find(|c| c.id == unit).is_some_and(|c| !c.visible), "the tick did not hide the subassembly");
        assert!(!drawn(&app, pin), "the pin inside the hidden subassembly is still drawn");
        assert!(drawn(&app, plate), "the plate beside it went out of sight too");
    }

    /// A PART IS RENAMED BY F2: a click on its row, F2, the new name typed over the old one, Enter - one step of undo.
    #[test]
    fn a_part_is_renamed_by_f2() {
        let (mut app, ctx) = inside_the_reference();
        let plates = named(&app, name(1));
        let texts = frame(&mut app, &ctx, Vec::new());
        let at = spot(&texts, name(1)).expect("the plate's row");
        calm(&mut app, &ctx);
        let _ = frame(&mut app, &ctx, click(at));
        let _ = frame(&mut app, &ctx, Vec::new());
        assert!(matches!(app.chosen.sel, Sel::Component(ci) if app.project.components[ci].id == plates[0]), "the click did not select the first plate");
        let steps = app.disk.edits.undo.len();
        let _ = frame(&mut app, &ctx, key(egui::Key::F2));
        let _ = frame(&mut app, &ctx, Vec::new());
        let all = egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND };
        let _ = frame(&mut app, &ctx, vec![all, egui::Event::Text("Base plate".into())]);
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
        let _ = frame(&mut app, &ctx, Vec::new());
        let now = |id: Id| app.project.components.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_default();
        assert_eq!(now(plates[0]), "Base plate", "F2, the typing and Enter did not rename the plate");
        assert_eq!(app.disk.edits.undo.len(), steps + 1, "the rename is not one step of undo");
    }

    /// A MATE JOINS PARTS OF TWO SUBASSEMBLIES: the second plate grounded by its tool, then a rigid mate from the top of
    /// the pin - inside the subassembly - to the top of that plate. The mate lives in the assembly holding both, and
    /// what moves is the subassembly as a whole: the pin keeps its place in it, the grounded plate keeps its own.
    #[test]
    fn a_mate_joins_parts_of_two_subassemblies() {
        let (mut app, ctx) = inside_the_reference();
        let (asm, plates, unit, pin) = (qymcad_ui_state::current_ctx_id(&app.active_path, &app.project), named(&app, name(1)), named(&app, name(2))[0], named(&app, name(3))[0]);
        let (plate_body, pin_body) = (app.project.component_bodies(plates[1])[0], app.project.component_bodies(pin)[0]);
        let transform = |app: &App, id: Id| app.project.components.iter().find(|c| c.id == id).map(|c| c.transform).expect("a component");
        let (unit_was, pin_was, first_was) = (transform(&app, unit), transform(&app, pin), app.project.world_transform(plates[0]));
        app.start_ground_pick(); // the button of the bar
        let (plate_top, _) = top_face(&app, plate_body);
        Hand::new(&mut app).look_at([20.0, 10.0, 8.0], 5.0).click(plate_top);
        assert!(app.project.is_grounded(plates[1]), "the click of the grounding tool did not ground the plate; the status: {}", app.status);
        let plate_at = app.project.world_transform(plates[1]);
        let (pin_top, _) = top_face(&app, pin_body);
        Hand::new(&mut app).look_at([20.0, 10.0, 8.0], 5.0).mate(JointKind::Rigid).anchor(0).click(pin_top).click(plate_top).key(egui::Key::Enter).key(egui::Key::Escape);
        calm(&mut app, &ctx); // the frames run on, as they do before a person, until the rebuild has landed
        assert_eq!(app.project.joints.len(), 1, "two clicks did not make a mate; the status: {}", app.status);
        assert_eq!(app.project.joint_home(&app.project.joints[0]), Some(asm), "the mate does not live in the assembly holding both parts");
        assert_eq!(app.project.joint_faults(), Vec::new(), "the mate does not solve");
        let near = |a: &[f64], b: &[f64], tol: f64| a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol);
        assert!(near(&transform(&app, pin), &pin_was, 1e-9), "the pin tore away from its subassembly");
        assert!(!near(&transform(&app, unit), &unit_was, 1e-6), "the subassembly did not move");
        assert!(near(&app.project.world_transform(plates[1]), &plate_at, 1e-9), "the grounded plate moved");
        assert!(near(&app.project.world_transform(plates[0]), &first_was, 1e-9), "the plate the mate does not touch moved");
        let (pin_face, pin_normal) = top_face_of_pin_now(&app, pin_body);
        let (plate_face, plate_normal) = top_face(&app, plate_body);
        assert!(near(&pin_face, &plate_face, 1e-6), "the faces do not meet: the pin's at {pin_face:?}, the plate's at {plate_face:?}");
        let dot: f64 = pin_normal.iter().zip(&plate_normal).map(|(a, b)| a * b).sum();
        assert!((dot.abs() - 1.0).abs() < 1e-6, "the faces are not in one plane: the normals {pin_normal:?} and {plate_normal:?}");
    }

    /// Every part under `id`, however deep.
    fn parts(app: &App, id: Id) -> Vec<Id> {
        let mut out = Vec::new();
        for k in app.project.component_children(id) {
            match app.project.components.iter().find(|c| c.id == k).map(|c| c.kind) {
                Some(qymcad_core::feature::ComponentKind::Part) => out.push(k),
                _ => out.extend(parts(app, k)),
            }
        }
        out
    }

    /// The tick of the row showing `name`: it stands before the row's label - the label's padding, the gap, half the tick.
    fn tick(ctx: &egui::Context, texts: &[(String, egui::Rect)], name: &str) -> egui::Pos2 {
        let r = row(texts, name);
        let sp = ctx.style_of(ctx.theme()).spacing.clone();
        egui::pos2(r.left() - sp.button_padding.x - sp.item_spacing.x - sp.interact_size.y / 2.0, r.center().y)
    }

    /// The view through the graphics device into `target/look/<file>`, the camera fitted to what is shown.
    fn shot(app: &mut App, file: &str) {
        let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/look"));
        std::fs::create_dir_all(&dir).expect("a folder for the pictures");
        crate::gui::fit3d(&mut app.viewing.cam, &app.project, app.viewing.view_rect);
        let view = app.viewing.view_rect;
        if let Some(img) = crate::gui::gpu_shot::eyes::shot(&app.painting(), egui::Rect::from_min_size(egui::pos2(0.0, 0.0), view.size())) {
            std::fs::write(dir.join(file), crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
        }
    }

    /// THE PRINT HEAD OF THE REPORT WORKED BY HAND, where its file is at hand (`QYM_CONDOR`): gone into by a double
    /// click, its first level under the names the other CAD shows, the carriage hidden by its tick and shown again, a
    /// mate put between parts of the carriage and the extruder, a part of the carriage renamed by F2. The views through the
    /// graphics device go into `target/look/c8-*.png`.
    #[test]
    #[ignore = "the owner's file"]
    fn the_print_head_is_worked_by_hand() {
        let Ok(path) = std::env::var("QYM_CONDOR") else { return };
        let first_level: Vec<&str> = include_str!("../../../qymcad-kernel/tests/data/condor-first-level.names").lines().collect();
        let (mut app, ctx) = running();
        answer(&mut app, &ctx, Want::Anything, &path);
        settle(&mut app, &ctx);
        calm(&mut app, &ctx);
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter)); // the window about the scale, where it came up, as the file has it
        let _ = frame(&mut app, &ctx, Vec::new());
        let head = app.project.components.iter().find(|c| c.parent == Some(app.project.root) && c.kind == qymcad_core::feature::ComponentKind::Assembly).map(|c| c.id).expect("the head came in");
        let head_name = app.project.components.iter().find(|c| c.id == head).map(|c| c.name.clone()).expect("a name");
        let texts = frame(&mut app, &ctx, Vec::new());
        let at = spot(&texts, &head_name).unwrap_or_else(|| panic!("no row {head_name:?} in the tree"));
        double_click_at(&mut app, &ctx, at);
        let _ = frame(&mut app, &ctx, Vec::new());
        let _ = frame(&mut app, &ctx, Vec::new());
        assert_eq!(qymcad_ui_state::current_ctx_id(&app.active_path, &app.project), head, "the double click did not go into the head");
        // THE FIRST LEVEL AS THE OTHER CAD SHOWS IT
        let name_of = |app: &App, id: Id| app.project.components.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_default();
        let first: Vec<String> = app.project.component_children(head).iter().map(|&c| name_of(&app, c)).collect();
        assert_eq!(first, first_level, "the first level is not the one the other CAD shows");
        shot(&mut app, "c8-head.png");
        let child = |app: &App, name: &str| app.project.component_children(head).into_iter().find(|&c| name_of(app, c) == name).expect("a subassembly of the first level");
        let (carriage, extruder) = (child(&app, first_level[0]), child(&app, first_level[1]));
        // the carriage HIDDEN BY ITS TICK, then shown again
        let texts = frame(&mut app, &ctx, Vec::new());
        let _ = frame(&mut app, &ctx, click(tick(&ctx, &texts, first_level[0])));
        let _ = frame(&mut app, &ctx, Vec::new());
        let carriage_bodies: Vec<Id> = parts(&app, carriage).into_iter().flat_map(|p| app.project.component_bodies(p)).collect();
        assert!(app.project.components.iter().find(|c| c.id == carriage).is_some_and(|c| !c.visible), "the tick did not hide the carriage");
        assert!(carriage_bodies.iter().all(|&b| !drawn(&app, b)), "a part of the hidden carriage is still drawn");
        shot(&mut app, "c8-hidden.png");
        let texts = frame(&mut app, &ctx, Vec::new());
        let _ = frame(&mut app, &ctx, click(tick(&ctx, &texts, first_level[0])));
        let _ = frame(&mut app, &ctx, Vec::new());
        assert!(carriage_bodies.iter().all(|&b| drawn(&app, b)), "the carriage did not come back into sight");
        // THE REST OUT OF THE WAY, as a person clears the view before picking faces
        for name in &first_level[2..] {
            let texts = frame(&mut app, &ctx, Vec::new());
            let _ = frame(&mut app, &ctx, click(tick(&ctx, &texts, name)));
            let _ = frame(&mut app, &ctx, Vec::new());
        }
        // THE MATE: the extruder grounded by its tool, then the topmost face of a part of the carriage onto the topmost
        // face of a part of the extruder
        let topmost = |app: &App, sub: Id| parts(app, sub).into_iter().map(|p| (p, top_face(app, app.project.component_bodies(p)[0]))).max_by(|a, b| a.1 .0[2].total_cmp(&b.1 .0[2])).expect("a part");
        let (carriage_part, (carriage_top, _)) = topmost(&app, carriage);
        let (extruder_part, (extruder_top, _)) = topmost(&app, extruder);
        let middle = [0, 1, 2].map(|k| (carriage_top[k] + extruder_top[k]) / 2.0);
        let inside_was: Vec<[f64; 12]> = parts(&app, carriage).iter().map(|&p| app.project.components.iter().find(|c| c.id == p).map(|c| c.transform).expect("a part")).collect();
        let (carriage_was, extruder_at) = (app.project.world_transform(carriage), app.project.world_transform(extruder));
        app.start_ground_pick(); // the button of the bar
        Hand::new(&mut app).look_at(middle, 1.5).click(extruder_top);
        assert!(app.project.is_grounded(extruder), "the grounding tool did not ground the extruder; the status: {}", app.status);
        Hand::new(&mut app).look_at(middle, 1.5).mate(JointKind::Rigid).anchor(0).click(carriage_top).click(extruder_top).key(egui::Key::Enter).key(egui::Key::Escape);
        calm(&mut app, &ctx);
        assert_eq!(app.project.joints.len(), 1, "two clicks did not make a mate; the status: {}", app.status);
        let j = app.project.joints[0].clone();
        let owners: Vec<Id> = [j.a, j.b].iter().filter_map(|&c| app.project.connector(c)).map(|c| c.owner).collect();
        assert_eq!(owners, [carriage, extruder], "the mate is not between the carriage and the extruder: {:?}", owners.iter().map(|&o| name_of(&app, o)).collect::<Vec<_>>());
        assert_eq!(app.project.joint_home(&j), Some(head), "the mate does not live in the head");
        assert_eq!(app.project.joint_faults(), Vec::new(), "the mate does not solve");
        let near = |a: &[f64], b: &[f64], tol: f64| a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol);
        let inside_now: Vec<[f64; 12]> = parts(&app, carriage).iter().map(|&p| app.project.components.iter().find(|c| c.id == p).map(|c| c.transform).expect("a part")).collect();
        assert!(inside_now.iter().zip(&inside_was).all(|(a, b)| near(a, b, 1e-9)), "a part tore away from the carriage");
        assert!(!near(&app.project.world_transform(carriage), &carriage_was, 1e-6), "the carriage did not move");
        assert!(near(&app.project.world_transform(extruder), &extruder_at, 1e-9), "the grounded extruder moved");
        let (c_now, _) = top_face_of_pin_now(&app, app.project.component_bodies(carriage_part)[0]);
        let (e_now, _) = top_face(&app, app.project.component_bodies(extruder_part)[0]);
        eprintln!("the carriage's face now at {c_now:?}, the extruder's at {e_now:?}; the carriage stands at {:?}", app.project.world_transform(carriage));
        shot(&mut app, "c8-mated.png");
        // A PART OF THE CARRIAGE RENAMED BY F2, from inside the carriage
        let texts = frame(&mut app, &ctx, Vec::new());
        double_click_at(&mut app, &ctx, spot(&texts, first_level[0]).expect("the carriage's row"));
        let _ = frame(&mut app, &ctx, Vec::new());
        assert_eq!(qymcad_ui_state::current_ctx_id(&app.active_path, &app.project), carriage, "the double click did not go into the carriage");
        let part = app.project.component_children(carriage)[0];
        let texts = frame(&mut app, &ctx, Vec::new());
        let at = spot(&texts, &name_of(&app, part)).expect("the part's row");
        let _ = frame(&mut app, &ctx, click(at));
        let _ = frame(&mut app, &ctx, key(egui::Key::F2));
        let _ = frame(&mut app, &ctx, Vec::new());
        let all = egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND };
        let _ = frame(&mut app, &ctx, vec![all, egui::Event::Text("Bearing A".into())]);
        let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
        let _ = frame(&mut app, &ctx, Vec::new());
        assert_eq!(name_of(&app, part), "Bearing A", "F2, the typing and Enter did not rename the part");
    }

    /// The pin's face that was its top before the mate, wherever the mate turned it: the one whose normal was +Z in
    /// the pin's own coordinates.
    fn top_face_of_pin_now(app: &App, body: Id) -> ([f64; 3], [f64; 3]) {
        let wt = app.project.body_world_transform(body);
        let f = app.project.regen_faces.get(&body).and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z))).expect("the body has faces");
        (qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z]), qymcad_core::feature::apply12_dir(&wt, f.normal))
    }
}
