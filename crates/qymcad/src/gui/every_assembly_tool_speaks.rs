//! EVERY ASSEMBLY TOOL SAYS WHAT IT IS WAITING FOR, AND DOES NOT COMPLAIN IN THE FRAME.
//!
//! Four troubles surfaced by hand that 771 checks had not caught: the highlight did not work in
//! "point at the axis", the axis tool flooded the whole assembly green, the popup complained in red,
//! and "point at the axis" on a slider did not change the travel. All four have one thing in common:
//! the FRAME was what had to be looked at, and the checks were looking at numbers.
//!
//! Hence a guard for the WHOLE CLASS: every tool of the workbench is taken in turn, a real screen is
//! drawn, and the least thing without which a tool is useless is checked — it was taken up, it SAID
//! what it is waiting for, and the screen does not complain about itself while it does. A tool that
//! silently waits for who knows what is a tool a person abandons.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1100.0, 800.0))
    }

    /// AN ASSEMBLY OF FOUR PARTS — on an empty document the tools rightly refuse.
    fn a_small_assembly(app: &mut App) -> Vec<Id> {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        for x in [0.0, 60.0, 120.0, 180.0] {
            super::super::joint_flow::tests::add_part_at(app, x);
        }
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 4, "setup: there should be four bodies of our own, and there are {}", mine.len());
        mine
    }

    /// Draw the whole assembly screen and return its words.
    fn screen_words(app: &mut App) -> Vec<String> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        app.workbench = super::super::Workbench::Assembly;
        app.mode_3d = true;
        let mut texts: Vec<String> = Vec::new();
        for _ in 0..2 {
            let out = ctx.run(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                egui::SidePanel::left("tree").show(c, |ui| app.build_tree_for_test(ui));
                egui::SidePanel::right("props").show(c, |ui| app.joints_panel_for_test(ui));
                app.joint_tool_bar_for_test(c);
                app.joint_popup_for_test(c, viewport());
            });
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }
        texts
    }

    #[test]
    fn every_tool_says_what_it_waits_for_and_the_screen_stays_quiet() {
        // EVERY TOOL OF THE WORKBENCH, each by its own door — the same one the button and the key use.
        let tools: [(&str, fn(&mut App)); 7] = [
            ("mate", |a: &mut App| a.arm_joint_pick_for_test()),
            ("connector", |a: &mut App| a.start_conn_pick()),
            ("group", |a: &mut App| a.start_group_pick()),
            ("width", |a: &mut App| a.start_width_pick()),
            ("tangent", |a: &mut App| a.start_tangent_pick()),
            ("relation", |a: &mut App| a.start_relation_pick()),
            ("ground", |a: &mut App| a.start_ground_pick()),
        ];
        let mut mute: Vec<String> = Vec::new();
        let mut noisy: Vec<String> = Vec::new();
        for (name, arm) in tools {
            let mut app = App::default();
            let _ = a_small_assembly(&mut app);
            app.workbench = super::super::Workbench::Assembly;
            app.mode_3d = true;
            arm(&mut app);

            // 1. THE TOOL SAID WHAT IT WAITS FOR. An empty status line means "work it out yourself".
            if app.status.trim().is_empty() {
                mute.push(format!("\"{name}\": taken up in silence — the person is not told what is expected of them"));
            }
            // 2. AND THE SCREEN DOES NOT COMPLAIN ABOUT ITSELF.
            let words = screen_words(&mut app);
            let complaints: Vec<&String> = words.iter().filter(|t| t.contains("widget ID") || t.contains("Widget ID")).collect();
            if !complaints.is_empty() {
                noisy.push(format!("\"{name}\": the screen complains in the frame — {complaints:?}"));
            }
            // 3. TRAP GUARD: the screen really was drawn, otherwise the silence means nothing.
            assert!(words.len() > 3, "GUARD: with the \"{name}\" tool the screen drew {} lines — there is nothing to look at", words.len());
        }
        assert!(mute.is_empty(), "tools are taken up in silence:\n{}", mute.join("\n"));
        assert!(noisy.is_empty(), "the screen complains about itself:\n{}", noisy.join("\n"));
    }
}
