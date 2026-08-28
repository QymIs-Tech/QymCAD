//! A SECOND SWEEP — OVER BEHAVIOUR RATHER THAN APPEARANCE.
//!
//! The first sweep took pictures and therefore could not, in principle, catch "the command does not
//! start", "rubbish is left after Esc", "Enter does not apply". That is a different class of defect,
//! and it costs more than cosmetics: a crooked caption is irritating, a tool that cannot be switched
//! off takes work away.
//!
//! WHY OVER THE CATALOGUE RATHER THAN A LIST OF FAVOURITE COMMANDS. `audit.rs` already runs live
//! scenarios, but invented ones — which means it checks what somebody suspected. Here EVERY command
//! from `COMMANDS` is checked, including the ones nobody would have remembered; a new command enters
//! the run on the day it is written into the catalogue, not on the day its author remembered to add a
//! test.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};
    use crate::command_catalog::{Launch, COMMANDS};

    /// A part with a body inside an assembly: the context a person actually works in.
    fn part_app() -> App {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 12.0;
            p.txt = "12".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let part = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component_for_test(part);
        app
    }

    /// A scene in which a command of this workbench makes any sense at all.
    ///
    /// THE SCENE IS PART OF THE CHECK rather than a prop. A command started in emptiness legitimately
    /// "does not work": there is nothing to extrude, nothing to round. It would be a mistake to check
    /// it where it has nothing to do and then declare the silence a defect.
    fn stage(workbench: &str) -> App {
        match workbench {
            "sketch" => {
                let mut app = part_app();
                let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
                app.project.add_rect_entity(si, 0.0, 0.0, 30.0, 20.0, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si);
                app.enter_sketch_edit(si);
                app
            }
            "assembly" => {
                let mut app = part_app();
                app.exit_context();
                // A SECOND PART: a mate needs two, and grounding a single part means nothing
                let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
                app.project.add_rect_entity(si, 80.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si);
                app.finish_sketch_edit();
                app.sel = Sel::Sketch(si);
                app.start_feat_cmd(1);
                app.apply_feat_cmd();
                app.rebuild_if_dirty();
                app
            }
            _ => part_app(),
        }
    }

    /// WHAT "THE COMMAND STARTED" MEANS — by the same state a person sees it by.
    fn started(app: &App, launch: Launch) -> bool {
        match launch {
            Launch::Feat(_) | Launch::Prim(_) => app.cmd.active(),
            Launch::SkTool(n) => app.tool.kind == n,
            Launch::Dim(n) => app.dim.kind == n,
            Launch::ClickOp(n) => app.tool.click_op == n,
            Launch::Modify(_) => true, // editing the selection fires at once and starts no mode
            Launch::Action("joint") => app.joint.pick_faces,
            Launch::Action("ground") => app.joint.ground_pick,
            Launch::Action(_) => true,
        }
    }

    /// WHAT STAYED SWITCHED ON. An empty list means the program came back to selection mode.
    fn tail(app: &App) -> Vec<&'static str> {
        let mut t = Vec::new();
        if app.cmd.active() {
            t.push("a feature command");
        }
        if app.tool.kind != 0 {
            t.push("a drawing tool");
        }
        if app.tool.click_op != 0 {
            t.push("a click operation");
        }
        if app.tool.modify != 0 {
            t.push("editing the selection");
        }
        if app.dim.kind != 0 {
            t.push("a dimension");
        }
        if app.joint.pick_faces || app.joint.ground_pick {
            t.push("assembling a mate");
        }
        if app.measure.on {
            t.push("measuring");
        }
        if app.pat.op != 0 {
            t.push("a pattern");
        }
        t
    }

    /// EVERY COMMAND EITHER STARTS OR SAYS WHY NOT.
    ///
    /// The first edition demanded simply "it started" and went red on the extrude, the revolve, the
    /// sweep and the loft. The analysis showed that is NOT a defect: with no sketch selected there is
    /// nothing to extrude, and the program honestly says to select a sketch first. The requirement had
    /// been stated wrongly — a scene without a selection is legitimate for those commands.
    ///
    /// Silence, however, is a defect in any case: a person pressed something and does not know whether
    /// anything happened or whether the program did not hear them. So the real invariant is checked:
    /// SOMETHING happened — either a mode switched on, or an explanation appeared in the status
    /// line.
    #[test]
    fn every_command_either_starts_or_says_why_not() {
        let mut mute: Vec<String> = Vec::new();
        for c in COMMANDS {
            let mut app = stage(c.workbench);
            app.status.clear();
            app.run_command(c.code);
            if !started(&app, c.launch) && app.status.trim().is_empty() {
                mute.push(format!("{} ({})", c.code, c.workbench));
            }
        }
        assert!(mute.is_empty(), "the command did not switch on AND stayed silent — a person does not know whether they were heard ({}):\n{}", mute.len(), mute.join("\n"));
    }

    /// ESC SWITCHES OFF ANY COMMAND.
    ///
    /// A tool that cannot be left by a key is one a person will put out with the mouse through the
    /// panel — and that is the very small thing that makes a program feel awkward while every function
    /// works.
    #[test]
    fn escape_switches_off_whatever_was_started() {
        let mut stuck: Vec<String> = Vec::new();
        for c in COMMANDS {
            let mut app = stage(c.workbench);
            app.run_command(c.code);
            app.on_escape();
            let t = tail(&app);
            if !t.is_empty() {
                stuck.push(format!("{}: {} is left after Esc", c.code, t.join(", ")));
            }
        }
        assert!(stuck.is_empty(), "Esc must put out the command ({}):\n{}", stuck.len(), stuck.join("\n"));
    }

    /// A CANCELLED COMMAND LEAVES NO TRACE IN THE DOCUMENT.
    ///
    /// Started it, changed one's mind, pressed Esc — the file must stay as it was. Otherwise the "save?"
    /// question on exit is asked about something nobody did, and rubbish takes root in the feature
    /// timeline.
    #[test]
    fn a_command_dropped_by_escape_leaves_the_document_alone() {
        let mut dirty: Vec<String> = Vec::new();
        for c in COMMANDS {
            // EDITING THE SELECTION IS NOT INCLUDED HERE: delete, mirror and offset fire straight on
            // the press — they have nothing to cancel on Esc, and changing the document is their job.
            if matches!(c.launch, Launch::Modify(_)) {
                continue;
            }
            let mut app = stage(c.workbench);
            let before = app.edit_key();
            app.run_command(c.code);
            app.on_escape();
            if app.edit_key() != before {
                dirty.push(c.code.to_string());
            }
        }
        assert!(dirty.is_empty(), "a command dropped by Esc changed the document ({}):\n{}", dirty.len(), dirty.join("\n"));
    }

    /// ENTER FROM A FIELD APPLIES THE COMMAND — a reported complaint that no guard had closed until
    /// now.
    ///
    /// It used to be: pressing Enter a second time does not apply the command until the tick button is
    /// pressed. The cause is the same as with the unreachable `U`: the key handler of the frame began
    /// with a refusal if a field held the keyboard — Enter never reached the command at all, and one
    /// had to aim at the tick with the mouse.
    ///
    /// Now ONE Enter is enough: `egui` releases a single-line field by itself, and the command receives
    /// it in the same frame. The value has already been accepted by then — the field writes into its own
    /// variable before the program gets to the key.
    ///
    /// The frames are real, with a real field: a fake "as if there were focus" would check somebody's
    /// own invention, while the question is exactly what `egui` does.
    #[test]
    fn enter_from_a_focused_field_applies_the_command() {
        let mut app = stage("part");
        let si = app.project.sketches.len() - 1;
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        assert!(app.cmd.active(), "the scene did not open the command — there is nothing to check");
        let feats_before = app.project.timeline.len();

        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let step = |app: &mut App, enter: bool, grab: bool| {
            let mut input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            if enter {
                input.events.push(egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
            }
            let mut buf = String::from("12");
            let _ = ctx.run_ui(input, |c| {
                egui::Area::new(egui::Id::new("probe_field")).show(c, |ui| {
                    let r = ui.text_edit_singleline(&mut buf);
                    if grab {
                        r.request_focus();
                    }
                });
                app.handle_key_commands(c);
            });
        };

        step(&mut app, false, true);
        assert!(ctx.egui_wants_keyboard_input(), "the field did not take the focus — this is not the scene the complaint was about");
        assert!(app.cmd.active(), "the command closed by itself, without a single key");

        step(&mut app, true, false);
        assert!(!ctx.egui_wants_keyboard_input(), "Enter must release the field");
        assert!(!app.cmd.active(), "Enter from a field did not apply the command — one will have to aim at the tick with the mouse");
        assert!(app.project.timeline.len() > feats_before, "the command \"applied\" and no feature appeared in the timeline");
    }
}
