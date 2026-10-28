//! WHERE THE PANELS GO: the places the window registers with the shell, and what fills them.

use super::*;

/// The AI assistant, served in the frame of the status place.
mod assistant;

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
    // THE BAR OF THE COMMAND STANDS ALWAYS, AT ONE HEIGHT: empty with nothing in hand, so taking a tool or putting it
    // down moves neither the view nor the panels
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
/// down the one held before, so two of them are not in hand at once; a step typed at a gizmo comes before the bar of the
/// view's section, which is the last, as the section stays on under any command.
const COMMAND_BARS: [&str; 5] = ["part_tool", "assembly_tool", "sk_tool_opts", "gizmo_number", "section_bar"];

/// THE BARS THAT STAND FOR A TOOL IN HAND - every bar of the command place but the sketch's own, which stands while a
/// sketch is open, and the section's, which stands while a section is on.
const TOOL_BARS: [&str; 2] = ["part_tool", "assembly_tool"];

/// DOES THE BAR OF THE SECTION STAND FOR A TOOL IN HAND - a section being placed, or one of the tools that share its
/// place: a mirrored copy of a part, a pencil or a command waiting for its plane or sketch, the measure.
fn section_bar_holds_a_tool(app: &App) -> bool {
    app.side.section.pick || app.params.mirror.in_hand() || app.tools.picking.is_sketch_plane() || app.tools.picking.sketch_for().is_some() || app.side.m3.on
}

/// IS A TOOL OF THE PART OR THE ASSEMBLY IN HAND: what a click on the model is given to, rather than to the selection.
pub(crate) fn tool_in_hand(app: &App) -> qymcad_ui_state::InHand {
    use qymcad_shell::Fills;
    if TOOL_BARS.iter().any(|k| app.live(k)) || section_bar_holds_a_tool(app) {
        qymcad_ui_state::InHand::Tool
    } else {
        qymcad_ui_state::InHand::Nothing
    }
}

/// THE TOOL OF A PART IN HAND, through the one entry of the frame: its bar here, its box over the view.
pub(crate) fn part_tool(app: &mut App, ui: &mut egui::Ui) {
    let canvas = app.viewing.view_rect;
    qymcad_ui_state::tool_in_hand(ui, canvas, qymcad_part::in_hand(&mut app.part_ctx()));
}

/// THE TOOL OF THE SKETCH OPEN FOR EDITING - with nothing in hand, the selection - through the one entry of the frame.
pub(crate) fn sketch_tool(app: &mut App, ui: &mut egui::Ui) {
    let canvas = app.viewing.view_rect;
    qymcad_ui_state::tool_in_hand(ui, canvas, qymcad_sketch::in_hand(&mut app.sketch_ctx()));
}

/// THE TOOL OF AN ASSEMBLY IN HAND - a joint made or reopened, or a tool that picks - through the one entry of the frame.
pub(crate) fn assembly_tool(app: &mut App, ui: &mut egui::Ui) {
    let canvas = app.viewing.view_rect;
    qymcad_ui_state::tool_in_hand(ui, canvas, qymcad_assembly::in_hand(&mut app.joint_ctx()));
}

/// THE TOOL OF THE PLACE OF THE SECTION IN HAND - the measure, a command waiting for its sketch, the pencil, the mirrored
/// copy of a part, the section - through the one entry of the frame.
pub(crate) fn plane_tool(app: &mut App, ui: &mut egui::Ui) {
    let canvas = app.viewing.view_rect;
    let pb = &mut qymcad_part::PlaneBarCtx {
        regen: &mut app.regen,
        section: &mut app.side.section,
        mirror: &mut app.params.mirror,
        picking: &mut app.tools.picking,
        project: &mut app.project,
        edits: &mut app.disk.edits,
        active_path: &app.active_path,
        status: &mut app.status,
        m3: &mut app.side.m3,
        scheme: &app.scheme,
        cam: &app.viewing.cam,
        set: &app.set,
        canvas,
    };
    qymcad_ui_state::tool_in_hand(ui, canvas, qymcad_part::plane_in_hand(pb));
}

/// DOES THE FRAME OF A TOOL TAKE Esc: a tool in hand, or the numeric entry at a gizmo - its box answers the key itself.
pub(crate) fn frame_takes_esc(app: &App) -> bool {
    tool_in_hand(app) == qymcad_ui_state::InHand::Tool || app.dragged.body_giz.num.is_some() || edit_si(&app.project, &app.sketch_ses).is_some()
}

impl qymcad_shell::Fills for App {
    /// IS THE PLACE WANTED THIS FRAME. Each of these used to be a `return` at the top of the panel - which
    /// worked only while the panel opened its own container. Now the container is opened first, and an empty
    /// one still eats a strip of the window, so the question is asked before opening rather than inside.
    fn live(&self, key: &'static str) -> bool {
        match key {
            "command_bar" => true,
            "section_bar" => self.side.section.plane.is_some() || section_bar_holds_a_tool(self),
            "part_tool" => self.tools.armed.commanding() || self.params.boolean.pick.is_some() || self.params.boolean.edit.is_some() || self.side.carr.mode.is_some(),
            "gizmo_number" => self.dragged.body_giz.num.is_some(),
            "sk_tool_opts" => edit_si(&self.project, &self.sketch_ses).is_some(),
            // THE ASSEMBLY BARS: each is one of several by what is being picked, and the shell now owns
            // their places. The conditions used to live at the top of each bar as an early return - which
            // meant the place existed even when nothing was drawn in it.
            "assembly_tool" => matches!(self.workbench, Workbench::Assembly) && self.viewing.mode_3d && (self.side.joint.edit.is_some() || qymcad_assembly::joint_picking(&self.side.joint)),
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
                // ONE ROW TALL AT LEAST, empty or not, in every workbench, so taking a tool or putting it down moves
                // neither the view nor the panels; a sketch's own bar grows past it by its rows. Without it the
                // outlined Apply and Cancel of a pattern of a sketch stood the bar 1 point taller than the arrow did.
                ui.set_min_height(qymcad_ui_state::command_bar_row(ui));
                if let Some(k) = COMMAND_BARS.into_iter().find(|k| self.live(k)) {
                    // ONE ID FOR THE PLACE, whatever bar stands in it: one bar of a command giving way to another lays its
                    // widgets where the last one's stood, and under the key of each bar egui marked the whole strip with a
                    // red frame on every change of the command in hand
                    qymcad_shell::keyed(ui, "the bar of the command in hand", |ui| self.fill(k, ui, edge));
                }
            }
            "menubar" => crate::gui::panels_bars::menu_bar(&mut self.bar_ctx(&mut bar_asks), ui),
            "toolbar" => qymcad_ui_state::path_bar(&mut self.bar_ctx(&mut bar_asks), ui, &crate::command_catalog::BENCHES),
            "section_bar" => plane_tool(self, ui),
            "part_tool" | "gizmo_number" => part_tool(self, ui),
            "sk_tool_opts" => sketch_tool(self, ui),
            "assembly_tool" => assembly_tool(self, ui),
            "status" => {
                // THE ASSISTANT'S STEP OF THIS FRAME, before the line that says what it did: the status place is in
                // every layout and never hidden, so a call is never left waiting for a panel the person closed
                assistant::serve(&mut self.part_ctx(), &ui.ctx().clone());
                crate::gui::panels_bars::status_bar(
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
                )
            }
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
