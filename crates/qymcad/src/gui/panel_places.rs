//! WHERE THE PANELS GO: the places the window registers with the shell, and what fills them.

use super::*;

/// WHERE THE PANELS GO. The sizes and the framing that used to be written inside each panel are said here
/// instead - they describe the container, not what is drawn in it.
///
/// Built afresh each frame rather than kept in a field: it is a dozen pushes, and a stored layout is a
/// second copy of this list that would drift from it. What the person MOVED does live on, in the settings,
/// and is laid over the registration.
///
/// FREE, not a method: it reads the settings and nothing else, and `App` is large enough already.
pub(crate) fn shell(set: &Settings) -> qymcad_shell::Shell {
    use qymcad_shell::{Place, Slot};
    let mut s = qymcad_shell::Shell::default();
    s.put(Place::new("menubar", Slot::Menu));
    s.put(Place::new("toolbar", Slot::Top));
    s.put(Place::new("command_bar", Slot::Top).framed().never_hidden());
    // THE STATUS LINE STAYS AT THE BOTTOM AND IN SIGHT, as the menu stays at the top: a person looks for it there, and
    // it is where the program says what it waits for
    s.put(Place::new("status", Slot::Bottom).pinned().never_hidden());
    // the bar of tools is as thick as its lines of buttons, and its captions where they stand across it, with an even
    // margin of BAR_MARGIN round them
    let look = set.bar.look();
    let content = qymcad_shell::ContentSize { side: look.across(qymcad_shell::Edge::Left), flat: look.across(qymcad_shell::Edge::Top) };
    // THE BAR OF TOOLS SHOWS THE MODEL THROUGH as much as the person set; every other place is solid
    let opacity = f32::from(set.bar.opacity) / 100.0;
    s.put(Place::new("wbtools", Slot::Left).content(content).margin(crate::gui::panels_bars::BAR_MARGIN).opacity(opacity).bare());
    s.put(Place::new("tree", Slot::Left).sized(260.0, true));
    s.put(Place::new("props", Slot::Right).sized(290.0, true));
    s.put(Place::new("viewport", Slot::Centre));
    s.restore(set.layout.clone());
    s
}

/// KEEP A SIZE the person dragged a panel to, in the layout of the settings.
pub(crate) fn keep_panel_size(set: &mut Settings, key: &str, size: qymcad_shell::Resize) {
    let mut sh = shell(set);
    sh.resize(key, size);
    set.layout = sh.saved();
}

/// THE BARS OF WHAT IS IN HAND, the first live one shown in the one place of the command bar. A command taken puts
/// down the one held before, so two of them are not in hand at once; the bar of the view's section is the last, as
/// the section stays on under any command.
const COMMAND_BARS: [&str; 7] = ["feat_cmd_bar", "bool_tool_bar", "comp_array_bar", "joint_edit_bar", "joint_tool_bar", "sk_tool_opts", "section_bar"];

impl qymcad_shell::Fills for App {
    /// IS THE PLACE WANTED THIS FRAME. Each of these used to be a `return` at the top of the panel - which
    /// worked only while the panel opened its own container. Now the container is opened first, and an empty
    /// one still eats a strip of the window, so the question is asked before opening rather than inside.
    fn live(&self, key: &'static str) -> bool {
        match key {
            "command_bar" => COMMAND_BARS.iter().any(|k| self.live(k)),
            "section_bar" => {
                self.side.section.plane.is_some()
                    || self.side.section.pick
                    || self.params.mirror.in_hand()
                    || self.tools.picking.is_sketch_plane()
                    || self.tools.picking.sketch_for().is_some()
                    || self.side.m3.on
            }
            "comp_array_bar" => self.side.carr.mode != 0,
            "feat_cmd_bar" => self.tools.armed.commanding(),
            "sk_tool_opts" => edit_si(&self.project, &self.sketch_ses).is_some(),
            "bool_tool_bar" => self.params.boolean.pick.is_some() || self.params.boolean.edit.is_some(),
            // THE ASSEMBLY BARS: each is one of several by what is being picked, and the shell now owns
            // their places. The conditions used to live at the top of each bar as an early return - which
            // meant the place existed even when nothing was drawn in it.
            "joint_tool_bar" => matches!(self.workbench, Workbench::Assembly) && self.viewing.mode_3d && qymcad_assembly::joint_picking(&self.side.joint),
            "joint_edit_bar" => self.side.joint.edit.is_some() && matches!(self.workbench, Workbench::Assembly) && self.viewing.mode_3d,
            // while a sketch is being edited the tree on the left is not needed - only the tools
            "tree" => edit_si(&self.project, &self.sketch_ses).is_none(),
            _ => true,
        }
    }

    fn fill(&mut self, key: &'static str, ui: &mut egui::Ui, edge: Option<qymcad_shell::Edge>) {
        // WHAT THE BARS ASKED FOR IS DONE AFTER THE DRAWING. A button that acted mid-frame would start a
        // command while the bar of that command was still being laid out.
        let (mut bar_asks, mut win_asks) = (Vec::new(), Vec::new());
        match key {
            "command_bar" => {
                if let Some(k) = COMMAND_BARS.into_iter().find(|k| self.live(k)) {
                    // ONE ID FOR THE PLACE, whatever bar stands in it: one bar of a command giving way to another lays its
                    // widgets where the last one's stood, and under the key of each bar egui marked the whole strip with a
                    // red frame on every change of the command in hand
                    qymcad_shell::keyed(ui, "the bar of the command in hand", |ui| self.fill(k, ui, edge));
                }
            }
            "menubar" => crate::gui::panels_bars::menu_bar(&mut self.bar_ctx(&mut bar_asks), ui),
            "toolbar" => crate::gui::panels_bars::toolbar(&mut self.bar_ctx(&mut bar_asks), ui),
            "section_bar" => crate::gui::panels_bars::section_bar(
                &mut qymcad_part::PlaneBarCtx {
                    regen: &mut self.regen,
                    section: &mut self.side.section,
                    mirror: &mut self.params.mirror,
                    picking: &mut self.tools.picking,
                    project: &mut self.project,
                    edits: &mut self.disk.edits,
                    active_path: &self.active_path,
                    status: &mut self.status,
                    m3: &mut self.side.m3,
                },
                ui,
            ),
            "comp_array_bar" => crate::gui::panels_bars::comp_array_bar(&mut self.part_ctx(), ui),
            "feat_cmd_bar" => self.feat_command_bar(ui),
            "sk_tool_opts" => crate::gui::panels_bars::tool_options_bar(&mut self.bar_ctx(&mut bar_asks), ui),
            "bool_tool_bar" => crate::gui::commands::bool_tool_bar(&mut self.part_ctx(), ui),
            "joint_tool_bar" => qymcad_assembly::joint_tool_bar(&mut self.joint_ctx(), ui),
            "joint_edit_bar" => qymcad_assembly::joint_edit_bar(&mut self.joint_ctx(), ui),
            "status" => crate::gui::panels_bars::status_bar(
                &mut qymcad_ui_state::StatusCtx {
                    cache: &self.cache,
                    cursor: self.cursor,
                    project: &self.project,
                    scheme: &self.scheme,
                    set: &mut self.set,
                    sketch_ses: &self.sketch_ses,
                    line: qymcad_ui_state::StatusLine { said: &mut self.status, rebuild: &mut self.regen.line },
                    win: &mut self.win,
                    par_search: &mut self.par_search,
                    ask: &mut win_asks,
                },
                ui,
            ),
            "wbtools" => crate::gui::panels_bars::wb_toolbar(&mut self.bar_ctx(&mut bar_asks), ui, edge),
            "tree" => self.tree_panel(ui),
            "props" => self.properties_panel(ui),
            "viewport" => self.viewport(ui),
            _ => {}
        }
        self.do_bar_asks(bar_asks, &ui.ctx().clone());
        self.do_win_asks(win_asks, &ui.ctx().clone()); // a click on a fault in the status line goes where a window's link goes
    }

    fn bar_frame(&self) -> egui::Frame {
        qymcad_ui_state::tool_bar_frame(&self.scheme)
    }

    fn resized(&mut self, key: &'static str, size: qymcad_shell::Resize) {
        keep_panel_size(&mut self.set, key, size);
    }
}
