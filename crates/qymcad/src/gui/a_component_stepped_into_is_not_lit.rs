//! A COMPONENT ONE STEPS INTO IS DRAWN AS IT IS, NOT LIGHTER.
//!
//! Reported behaviour: after stepping into a subassembly or a part by a double click in the tree, its bodies stay
//! drawn lighter in the 3D view, as if selected; neither Esc nor a click on the view clears it, only opening a tool
//! does, and not every time.
//!
//! The selection was never the cause - stepping in clears it. The card draws a body by the number its vertices
//! carry, a row of the look table, and that number is the body's place in the list of what is shown: it changes
//! with the context. The scene keeps every body's vertices in a block and takes a block ready-made when its shape
//! and place did not change - with the number it was built under, a row that now belongs to another body or to
//! none. A tool changes the geometry revision, every block is built again, and the look comes right.
#[cfg(test)]
pub(crate) mod tests {
    use crate::gui::import_door::tests::{answer, click, frame, running, settle, spot};
    use crate::gui::App;
    use qymcad_core::model::Id;
    use qymcad_ui_state::Want;

    fn reference() -> String {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../qymcad-kernel/tests/data/assembly.step").to_string()
    }

    /// The bodies the 3D view draws highlighted.
    fn lit(app: &App) -> Vec<usize> {
        qymcad_ui_state::visible_mesh_items(&app.painting()).iter().filter(|m| m.hot).map(|m| m.index).collect()
    }

    /// The pieces of the scene whose vertices name a row of the look table outside their own body's rows - its own and
    /// those of the colours of its faces.
    fn misnumbered(app: &App) -> Vec<(u32, u32)> {
        let scene = crate::gui::render_scene::gpu_scene(&app.painting());
        scene.pieces.iter().filter_map(|p| p.verts.iter().find(|v| v.body < p.body || v.body >= p.body + p.rows).map(|v| (p.body, v.body))).collect()
    }

    /// Frames until nothing is busy for several in a row: the rebuild after a reading or an edit holds the window as
    /// well, and swallows clicks. Under the load of the whole run a rebuild that takes no time alone takes several
    /// frames, and a check that clicked through it failed there and passed on its own.
    pub(crate) fn calm(app: &mut App, ctx: &egui::Context) {
        let mut calm = 0;
        crate::gui::session::waiting::until("five frames in a row with nothing busy", || {
            let _ = frame(app, ctx, Vec::new());
            calm = if app.regen.busy.is_none() { calm + 1 } else { 0 };
            calm >= 5
        });
    }

    /// The file at `path` brought in by the door, its rebuild waited out. Answers the component it landed as: the one
    /// the import added under a component that stood there before - found by the tree, not by its name, which the window
    /// numbers apart when a row of the document already reads it.
    pub(crate) fn brought_in(app: &mut App, ctx: &egui::Context, path: &str) -> Id {
        let before: Vec<Id> = app.project.components.iter().map(|c| c.id).collect();
        answer(app, ctx, Want::Anything, path);
        settle(app, ctx);
        calm(app, ctx);
        let tops: Vec<Id> = app.project.components.iter().filter(|c| !before.contains(&c.id) && c.parent.is_some_and(|p| before.contains(&p))).map(|c| c.id).collect();
        assert_eq!(tops.len(), 1, "the file did not land as one component; the tree: {:?}", app.project.components.iter().map(|c| &c.name).collect::<Vec<_>>());
        tops[0]
    }

    /// `path` in through the door, with the program calm after it.
    fn landed(path: &str) -> (App, egui::Context) {
        let (mut app, ctx) = running();
        answer(&mut app, &ctx, Want::Anything, path);
        settle(&mut app, &ctx);
        calm(&mut app, &ctx);
        assert!(app.win.import_scale.is_none(), "{path} was asked about its scale");
        (app, ctx)
    }

    /// A HAND'S DOUBLE CLICK on the tree row `name`: a pause first, then the move, each press and each release in a
    /// frame of its own. The pause is not decoration: egui counts a release within 0.6 s of the one before last as a
    /// TRIPLE click, and frames here are 1/60 s apart.
    fn double_click(app: &mut App, ctx: &egui::Context, name: &str) {
        for _ in 0..40 {
            let _ = frame(app, ctx, Vec::new());
        }
        let texts = frame(app, ctx, Vec::new());
        let at = spot(&texts, name).unwrap_or_else(|| panic!("no row {name:?}"));
        double_click_at(app, ctx, at);
    }

    /// The same double click on a spot rather than a row by its name - for rows that share one.
    pub(crate) fn double_click_at(app: &mut App, ctx: &egui::Context, at: egui::Pos2) {
        calm(app, ctx); // a person waits for the rebuild's window before clicking
        for _ in 0..40 {
            let _ = frame(app, ctx, Vec::new());
        }
        let button = |pressed| vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }];
        for events in [vec![egui::Event::PointerMoved(at)], button(true), button(false), Vec::new(), button(true), button(false)] {
            let _ = frame(app, ctx, events);
        }
        for _ in 0..4 {
            let _ = frame(app, ctx, Vec::new());
        }
    }

    fn inside(app: &App) -> String {
        let here = qymcad_ui_state::current_ctx_id(&app.active_path, &app.project);
        app.project.components.iter().find(|c| c.id == here).map(|c| c.name.clone()).unwrap_or_default()
    }

    /// The head of the reference: its assembly, named in the file.
    fn head(app: &App) -> String {
        app.project.components.iter().find(|c| c.parent == Some(app.project.root) && c.kind == qymcad_core::feature::ComponentKind::Assembly).map(|c| c.name.clone()).expect("the assembly came in")
    }

    /// EVERY BODY KEEPS ITS OWN LOOK AFTER STEPPING IN: the pin is the third body shown at the top and the first
    /// inside its subassembly, and the block of its vertices, taken ready-made, must say so.
    #[test]
    fn every_body_keeps_its_own_look_after_stepping_in() {
        let (mut app, ctx) = landed(&reference());
        assert!(misnumbered(&app).is_empty(), "at the top: {:?}", misnumbered(&app)); // the blocks are built here
        let head = head(&app);
        double_click(&mut app, &ctx, &head);
        let unit = app
            .project
            .components
            .iter()
            .find(|c| c.kind == qymcad_core::feature::ComponentKind::Assembly && app.project.components.iter().any(|p| Some(p.id) == c.parent && p.name == head))
            .map(|c| c.name.clone())
            .expect("the subassembly of the reference");
        double_click(&mut app, &ctx, &unit);
        assert_eq!(inside(&app), unit, "the double click did not step into {unit:?}");
        assert!(misnumbered(&app).is_empty(), "stepped in, and the card is told to draw these pieces (their body, the row their vertices name) by another body's look: {:?}", misnumbered(&app));
    }

    #[test]
    fn a_double_click_into_a_subassembly_leaves_nothing_lit() {
        let (mut app, ctx) = landed(&reference());
        let head = head(&app);
        double_click(&mut app, &ctx, &head);
        assert_eq!(inside(&app), head, "the double click did not step in");
        assert!(lit(&app).is_empty(), "stepped in, and the view still draws {:?} highlighted", lit(&app));
    }

    #[test]
    fn stepping_in_from_the_properties_leaves_nothing_lit() {
        let (mut app, ctx) = landed(&reference());
        let head = head(&app);
        let texts = frame(&mut app, &ctx, Vec::new());
        let row = spot(&texts, &head).unwrap_or_else(|| panic!("no row {head:?}"));
        let _ = frame(&mut app, &ctx, click(row));
        let texts = frame(&mut app, &ctx, Vec::new());
        let button = spot(&texts, &crate::i18n::tr("props-enter-component")).expect("the button to step in");
        let _ = frame(&mut app, &ctx, click(button));
        let _ = frame(&mut app, &ctx, Vec::new());
        assert_eq!(inside(&app), head, "the button did not step in");
        assert!(lit(&app).is_empty(), "stepped in, and the view still draws {:?} highlighted", lit(&app));
    }

    #[test]
    fn a_double_click_into_a_part_leaves_nothing_lit() {
        let (mut app, ctx) = landed(&reference());
        let head = head(&app);
        double_click(&mut app, &ctx, &head);
        let plate = app
            .project
            .components
            .iter()
            .find(|c| c.kind == qymcad_core::feature::ComponentKind::Part && app.project.components.iter().any(|p| Some(p.id) == c.parent && p.name == head))
            .map(|c| c.name.clone())
            .expect("a part of the head");
        double_click(&mut app, &ctx, &plate);
        assert_eq!(inside(&app), plate, "the double click did not step into the part");
        assert!(lit(&app).is_empty(), "inside the part, the view still draws {:?} highlighted", lit(&app));
        assert!(misnumbered(&app).is_empty(), "inside the part: {:?}", misnumbered(&app));
    }

    /// THE REPORT'S OWN PATH, where the owner's file is at hand (`QYM_CONDOR`): into the print head, the extruder and
    /// its mount by double clicks in the tree, the view through the graphics device into `target/look/lit-<step>.png`.
    #[test]
    #[ignore = "the owner's file"]
    fn the_reported_path_into_the_extruder_mount() {
        let Ok(path) = std::env::var("QYM_CONDOR") else { return };
        let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/look"));
        std::fs::create_dir_all(&dir).expect("a folder for the pictures");
        let (mut app, ctx) = landed(&path);
        let shoot = |app: &mut App, tag: &str| {
            let view = app.viewing.view_rect;
            crate::gui::fit3d(&mut app.viewing.cam, &app.project, view);
            if let Some(img) = crate::gui::gpu_shot::eyes::shot(&app.painting(), egui::Rect::from_min_size(egui::pos2(0.0, 0.0), view.size())) {
                std::fs::write(dir.join(format!("lit-{tag}.png")), crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
            }
        };
        shoot(&mut app, "top");
        let names = include_str!("../../../qymcad-kernel/tests/data/condor-first-level.names");
        let extruder = names.lines().nth(1).expect("the extruder's name").to_string();
        // the extruder's mount: its subassembly that names the extruder
        let mount = app
            .project
            .components
            .iter()
            .find(|c| {
                app.project.components.iter().any(|p| Some(p.id) == c.parent && p.name == extruder)
                    && c.kind == qymcad_core::feature::ComponentKind::Assembly
                    && c.name.to_lowercase().contains(&extruder.to_lowercase())
            })
            .map(|c| c.name.clone());
        for (k, name) in ["Condor v108".to_string(), extruder.clone()].into_iter().chain(mount).enumerate() {
            double_click(&mut app, &ctx, &name);
            eprintln!("into {name}: inside {:?}, lit {:?}, misnumbered {:?}", inside(&app), lit(&app), misnumbered(&app));
            shoot(&mut app, &format!("{k}"));
        }
    }
}
