//! A SWEEP ACROSS THE INTERFACE: capture everything in a row and LOOK at it.
//!
//! The tests see the interface as a list of strings: "a catalogue key did not arrive", "a CAM word did
//! not show", "a button is drawn". What they do not see is how it looks: clipped captions, columns
//! that have slid, empty panels, a colour that cannot be told apart. That is exactly where three
//! defects lived that were found by eye in a day: the quotation marks in the language catalogue, the
//! orange glyphs on a perfectly normal slot, the lost texture of the viewport.
//!
//! The run: `cargo test -p qymcad -- --ignored --nocapture ui_sweep`. It puts PNGs into
//! `target/ui-sweep/` — they do not go into the repository: this is not a product but a way to
//! look.
#[cfg(test)]
mod tests {
    use crate::gui::WinKind;
    use super::super::App;
    use egui::ColorImage;

    fn out_dir() -> std::path::PathBuf {
        let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-sweep");
        std::fs::create_dir_all(&d).expect("the directory of the run");
        d
    }

    fn save(name: &str, img: &ColorImage) {
        let png = crate::gui::color_image_to_png(img).expect("PNG");
        std::fs::write(out_dir().join(format!("{name}.png")), png).expect("the write");
    }

    /// A capture of a surface with the theme of the program applied.
    fn shot(app: &mut App, w: usize, h: usize, draw: impl Fn(&mut App, &mut egui::Ui)) -> ColorImage {
        let bg = app.scheme.pal.viewport_bg();
        super::super::help_raster::shot_ui([w, h], bg, |ui| {
            let ctx = &ui.ctx().clone();
            crate::gui::apply_theme(&mut app.scheme, &app.set, ctx);
            draw(app, ui);
        })
    }

    /// THE WHOLE WINDOW — as a person sees it.
    ///
    /// THROUGH THE SHELL, exactly as the frame does. It used to stack the bars, the tree, the properties
    /// and the viewport into ONE `Ui` by hand, and that stopped being a picture of the program the day the
    /// panels gave their containers up to the shell: a panel now FILLS the place it is handed, so handing
    /// all of them the same top-level `Ui` drew them one over another. The captures showed the tree's text
    /// on top of the toolbar's - letters over letters, which reads as a broken font rather than as an
    /// overlap, so the sweep looked like a rasteriser fault and was believed for its silence.
    ///
    /// It was also a hand-written list beside a full one: `Slot::ORDER` names the six kinds of place and
    /// the order they must open in (the middle last, which is egui's rule). Walking it means a panel added
    /// to the shell appears here without anyone remembering to add it.
    fn whole(app: &mut App, name: &str) {
        let draw = |a: &mut App, ui: &mut egui::Ui| {
            let shell = crate::gui::shell(&a.set);
            for slot in qymcad_shell::Slot::ORDER {
                shell.run_slot(slot, ui, a);
            }
            a.command_search_window(ui.ctx()); // over everything, as in the program
        };
        // TWO PASSES, AND THE FIRST ONE IS FOR THE CANVAS RECTANGLE.
        //
        // The program frames a body itself: entering a part clears `cam.init`, and the viewport fits the
        // body to its own rectangle. In a capture that fitting lands on the FIRST frame, when the side
        // panels have not yet claimed their strips - egui settles a layout one frame late - so it fits the
        // body to a canvas far wider than the one it ends up drawn on. Measured on the 60x40x14 plate:
        // scale 39.7 instead of 5.6, seven times too close, and four captures of the Part came out as a
        // flat wash of one colour with nothing of the fillet, the selection or a command's preview in them.
        //
        // So the first pass is thrown away and only teaches `view_rect`; the fit is then done against that
        // settled rectangle and locked with `init`, and the second pass is the picture.
        let _ = shot(app, 1280, 800, draw);
        // BOTH VIEWS, because both fit themselves the same way and both get it wrong for the same reason:
        // `viewport_3d` fits when `cam.init` is false and `viewport_2d` when `view.initialized` is false,
        // and in a capture that is always the FIRST frame. Measured on the flat sketch of the light scheme:
        // the canvas showed nothing but the grid while the properties panel counted three points and four
        // constraints of a rectangle that was simply off-screen.
        if app.viewing.view_rect.width() > 1.0 {
            if app.viewing.mode_3d {
                crate::gui::fit3d(&mut app.viewing.cam, &app.project, app.viewing.view_rect);
            } else {
                qymcad_ui_state::fit(&app.project, &mut app.viewing.view, app.viewing.view_rect);
            }
        }
        // A TEXTURE BELONGS TO THE CONTEXT THAT MADE IT, and each capture runs its own. The view cube keeps
        // its face captions as textures in a cache on the application (rasterising a font every frame would
        // be wasteful), so a handle made in the first pass names nothing in the second: the captions came
        // out as BLACK RECTANGLES on the cube - a texture that is not there samples as nothing.
        //
        // The application never meets this, having one context for its whole life; only a capture does. So
        // the cache is emptied between passes rather than the cube being taught about it.
        app.cache.label_tex.borrow_mut().clear();
        let img = shot(app, 1280, 800, draw);
        // THE CAPTURE CHECKS ITSELF, because a sweep nobody looks at every day fails silently and stays
        // green. Both defects this instrument had were the same shape on the picture - A FLAT WASH OF ONE
        // COLOUR where the model should be: once because the panels were stacked into a single `Ui` and
        // drew over one another, once because the camera fitted itself to a canvas the body was not drawn
        // on and ended up inside it. Counting the distinct colours of the canvas catches both, and costs a
        // pass over the pixels that are already in hand.
        if !app.project.bodies.is_empty() {
            let r = app.viewing.view_rect;
            let mut seen = std::collections::HashSet::new();
            for y in (r.top().max(0.0) as usize..(r.bottom() as usize).min(img.size[1])).step_by(4) {
                for x in (r.left().max(0.0) as usize..(r.right() as usize).min(img.size[0])).step_by(4) {
                    seen.insert(img.pixels[y * img.size[0] + x].to_array());
                }
            }
            assert!(
                seen.len() > 32,
                "the capture \"{name}\" shows {} colours on the canvas: a body is in the document but the picture is a flat wash, \
                 so this sweep is showing nothing and would go on saying so quietly",
                seen.len()
            );
        }
        save(name, &img);
    }

    /// A plate with a fillet and a hole — the common scene for most of the captures.
    fn part() -> App {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.chosen.sel = super::super::Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 14.0;
            p.txt = "14".into();
        }
        app.apply_feat_cmd();
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        let edges: Vec<u32> = app.project.regen_edges.get(&body).map(|es| es.iter().take(4).map(|e| e.id).collect()).unwrap_or_default();
        app.project.add_fillet(body, 3.0, edges);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        app.set.gpu_viewport = false;
        app.viewing.mode_3d = true;
        // The camera is NOT set here: `whole` frames the body against the settled canvas, the way the
        // program does when a person opens a part.
        app
    }

    #[test]
    #[ignore = "a sweep across the interface: the captures land in target/ui-sweep"]
    fn ui_sweep() {
        std::env::set_var("XDG_DATA_HOME", "/home/user/.local/share");

        // 1. AN EMPTY PROGRAM — the first thing a person sees.
        let mut app = App::default();
        app.set.gpu_viewport = false;
        whole(&mut app, "01-empty");

        // 2. THE START SCREEN.
        let mut app = App::default();
        app.win.show_start(true);
        let img = shot(&mut app, 1000, 700, |a, ctx| a.start_screen(ctx));
        save("02-start-screen", &img);

        // 3. A PART WITH A BODY — the working state.
        let mut app = part();
        let p = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component(p);
        whole(&mut app, "03-part");

        // 4. A BODY IS SELECTED — the properties should tell about it.
        let mut app = part();
        let p = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component(p);
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        app.chosen.sel = super::super::Sel::Mesh(app.project.mesh_index(body).expect("the mesh"));
        whole(&mut app, "04-body-selected");

        // 5. AN OPEN COMMAND — the parameter bar and the preview.
        let mut app = part();
        let p = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component(p);
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        app.chosen.sel = super::super::Sel::Mesh(app.project.mesh_index(body).expect("the mesh"));
        app.start_feat_cmd(7); // hole
        whole(&mut app, "05-command-hole");

        let mut app = part();
        let p = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component(p);
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        app.chosen.sel = super::super::Sel::Mesh(app.project.mesh_index(body).expect("the mesh"));
        app.start_feat_cmd(4); // fillet
        whole(&mut app, "06-command-fillet");

        // 7. A SKETCH UNDER EDIT — the whole workbench.
        let mut app = App::default();
        app.set.gpu_viewport = false;
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, -30.0, -20.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.viewing.view.scale = 6.0;
        app.viewing.view.initialized = true;
        whole(&mut app, "07-sketch");

        // 8. A SKETCH WITH A TOOL TAKEN — whether the hint and the tool bar are visible.
        let mut app = App::default();
        app.set.gpu_viewport = false;
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, -30.0, -20.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.set_sk_tool(3); // circle
        app.viewing.view.scale = 6.0;
        app.viewing.view.initialized = true;
        whole(&mut app, "08-sketch-tool");

        // 9. AN ASSEMBLY — two components.
        let mut app = App::default();
        app.set.gpu_viewport = false;
        for (name, w, d, h, at) in [("base", 70.0, 50.0, 10.0, [0.0, 0.0, 0.0]), ("post", 18.0, 18.0, 40.0, [26.0, 16.0, 10.0])] {
            let cid = app.project.add_component(name);
            app.enter_component(cid);
            let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
            app.project.add_rect_entity(si, 0.0, 0.0, w, d, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(si);
            app.finish_sketch_edit();
            app.chosen.sel = super::super::Sel::Sketch(si);
            app.start_feat_cmd(1);
            if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = h;
                p.txt = format!("{h}");
            }
            app.apply_feat_cmd();
            qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
            app.project.set_component_transform(cid, [1.0, 0.0, 0.0, at[0], 0.0, 1.0, 0.0, at[1], 0.0, 0.0, 1.0, at[2]]);
            app.exit_context();
        }
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        app.viewing.cam.scale = 5.0;
        app.viewing.cam.target = [35.0, 25.0, 20.0];
        whole(&mut app, "09-assembly");

        // AN ASSEMBLY WITH THE MATE COMMAND RUNNING — the bar and the choice of anchor, "by origins"
        // included.
        //
        // BEWARE OF REUSING `App` BETWEEN CAPTURES: every capture has an `egui::Context` of its own,
        // while the view cube holds a texture issued by the PREVIOUS one. On the second capture the
        // captions of the cube come out as black rectangles — there is no texture with that number in
        // the new context. In the program there is one context, so this is an artefact of the sweep
        // rather than a defect; but seeing such a thing in a capture, it is easy to spend half a day
        // hunting a defect that does not exist.
        app.start_joint_pick();
        qymcad_assembly::set_joint_anchor_mode_for_test(&mut app.joint_ctx(), 3);
        whole(&mut app, "18-joint-command");

        // 11. THE WINDOWS one by one — each on a scene of its own.
        let mut app = part();
        app.win.open(WinKind::Settings);
        for (i, sec) in super::super::settings_sections::SettingsSection::all().iter().enumerate() {
            app.scheme.section = *sec;
            let img = shot(&mut app, 940, 620, |a, ui| { let mut asks = Vec::new(); crate::gui::panels_windows::settings_window(&mut a.win_ctx(&mut asks), ui.ctx()); a.do_win_asks(asks, ui.ctx()); });
            save(&format!("11-settings-{i}"), &img);
        }

        let mut app = part();
        app.win.open(WinKind::Params);
        app.project.parameters = vec![
            qymcad_core::model::Param { name: "w".into(), expr: "60".into(), value: 60.0 },
            qymcad_core::model::Param { name: "h".into(), expr: "w/2".into(), value: 30.0 },
            qymcad_core::model::Param { name: "bad".into(), expr: "w/".into(), value: 0.0 },
        ];
        let img = shot(&mut app, 700, 400, |a, ui| { let mut asks = Vec::new(); crate::gui::panels_windows::params_window(&mut a.win_ctx(&mut asks), ui.ctx()); a.do_win_asks(asks, ui.ctx()); });
        save("12-params-with-error", &img);

        // A COMMAND FIELD WITH A TYPO — does it say what is wrong and in which field.
        let mut app = part();
        let p = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component(p);
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        app.chosen.sel = super::super::Sel::Mesh(app.project.mesh_index(body).expect("the mesh"));
        app.start_feat_cmd(7);
        if let Some(f) = app.tools.cmd.params.first_mut() {
            f.txt = "10 /".into();
        }
        whole(&mut app, "17-command-bad-expr");

        let mut app = part();
        app.win.open(WinKind::Hotkeys);
        let img = shot(&mut app, 820, 760, |a, ui| a.hotkeys_window(ui.ctx()));
        save("13-hotkeys", &img);

        let mut app = part();
        app.win.open(WinKind::About);
        let img = shot(&mut app, 700, 460, |a, ctx| crate::gui::panels_windows::about_dialog(&mut a.win, &a.scheme, ctx));
        save("14-about", &img);

        let mut app = part();
        app.win.open(WinKind::DocProps);
        let img = shot(&mut app, 700, 520, |a, ui| { let mut asks = Vec::new(); crate::gui::panels_windows::doc_props_window(&mut a.win_ctx(&mut asks), ui.ctx()); a.do_win_asks(asks, ui.ctx()); });
        save("15-doc-props", &img);

        let mut app = part();
        app.win.open(WinKind::PartsLibrary);
        let img = shot(&mut app, 900, 620, |a, ui| { let mut asks = Vec::new(); crate::gui::panels_windows::parts_library_window(&mut a.win_ctx(&mut asks), ui.ctx()); a.do_win_asks(asks, ui.ctx()); });
        save("16-parts-library", &img);

        // THE COMMAND SEARCH — a new window.
        let mut app = part();
        let p = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component(p);
        crate::gui::command_search::toggle_command_search(&mut app.win);
        app.win.cmd_search_query = "fil".to_string();
        whole(&mut app, "19-command-search");

        // THE COLOUR SCHEMES — ALL OF THEM, NOT ONE DARK ONE.
        //
        // The first sweep was entirely in the dark scheme, and the list of what it did not cover said
        // as much: the light one needs a run of its own. With schemes that colour the interface itself
        // the question became sharper: there the colour of a button and the colour of text come from
        // data for the first time, and nothing stops them drifting apart. The guards measure contrast
        // in numbers, but a grey blob instead of a panel is visible only by eye.
        for id in ["light", "dracula", "alucard"] {
            let mut app = part();
            app.set.gpu_viewport = false;
            app.set.scheme = id.into();
            whole(&mut app, &format!("20-scheme-{id}"));

            let mut app = part();
            app.set.scheme = id.into();
            app.win.open(WinKind::Settings);
            app.scheme.section = super::super::settings_sections::SettingsSection::Appearance;
            let img = shot(&mut app, 940, 620, |a, ui| { let mut asks = Vec::new(); crate::gui::panels_windows::settings_window(&mut a.win_ctx(&mut asks), ui.ctx()); a.do_win_asks(asks, ui.ctx()); });
            save(&format!("21-scheme-{id}-settings"), &img);

            // THE SKETCH, THE TREE AND THE PROPERTIES — IN EVERY SCHEME, not only in the dark one.
            //
            // The audit recorded as uncovered: the three schemes are captured in two views only, and a
            // full run over the light one is still needed — there had already been discrepancies
            // there. The contrast is measured in numbers by the palette guards (they walk over ALL the
            // schemes), but a grey blob instead of a panel, and a dimension that merges with a line,
            // are visible only by eye — so the pictures are needed for every scheme too.
            let mut app = part();
            app.set.gpu_viewport = false;
            app.set.scheme = id.into();
            app.viewing.mode_3d = false;
            if let Some(si) = app.project.sketches.iter().position(|_| true) {
                app.chosen.sel = super::super::Sel::Sketch(si);
                app.enter_sketch_edit(si);
            }
            whole(&mut app, &format!("22-scheme-{id}-sketch"));

            let mut app = part();
            app.set.scheme = id.into();
            let img = shot(&mut app, 340, 800, |a, ui| a.tree_panel(ui));
            save(&format!("23-scheme-{id}-tree"), &img);

            let mut app = part();
            app.set.scheme = id.into();
            let img = shot(&mut app, 360, 800, |a, ui| a.properties_panel(ui));
            save(&format!("24-scheme-{id}-properties"), &img);
        }

        eprintln!("captures of the sweep: {}", out_dir().display());
    }

}
