//! THE AI ASSISTANT IN THE WINDOW: the calls of `qymcad-mcp` made as steps of this window, one at a time - each drawn,
//! written on the status line and undoable as one step - and pictures of the window handed back for the model to look
//! at.
//!
//! A call is taken only when the window is at rest: no operation open, no rebuild asked for or running. Its answer
//! waits for the same: the window rebuilds in the background, and an answer read right after the step would report the
//! bodies as they were before it. A picture is asked for once the step is drawn, and comes back as an event of a later
//! frame.
//!
//! Called from the frame of the status place, which stands in every layout and is never hidden: it adds nothing to the
//! application but this call.
use std::sync::{Arc, Mutex};

use qymcad_core::model::{Id, Project};
use qymcad_mcp::link;
use qymcad_mcp::tools::{self, Done};
use qymcad_mcp::workshop::{View, Workshop};
use qymcad_ui_state::{AssistantAccess, Notice, PartCtx};

/// Frames a step is given at the least before it is answered: one for the scheduler to start the rebuild it asked for,
/// one for the view to draw the result a picture will show.
const SETTLE_FRAMES: u32 = 2;

/// Frames a picture asked for is waited for before the call is refused: the window hands it back within one or two,
/// so four seconds at 60 frames a second is a window that will not.
const PICTURE_FRAMES: u32 = 240;

/// A picture wider than this is made smaller before it goes: a model looks at about this many pixels across, and the
/// rest would cost it the reading of a picture it cannot see.
const PICTURE_WIDTH: u32 = 1568;

/// Where a call for a picture stands.
enum Shot {
    NotAsked,
    Asked { frames: u32 },
}

/// A call taken and not yet answered.
struct Awaiting {
    request: link::Request,
    done: Done,
    frames: u32,
    shot: Shot,
}

static AWAITING: Mutex<Option<Awaiting>> = Mutex::new(None);

#[cfg(test)]
mod tests;

/// THE ASSISTANT'S PART OF THIS FRAME: listen while the person lets it, take the next call when the window is at rest,
/// answer the one under way once it is built.
pub(super) fn serve(pc: &mut PartCtx, ctx: &egui::Context) {
    if pc.set.assistant != AssistantAccess::Welcome {
        if link::listening() {
            link::stop();
            refuse_waiting("the person has stopped letting an assistant work in the window");
        }
        return;
    }
    if !link::listening() {
        let wake = ctx.clone();
        if let Err(why) = link::listen(Arc::new(move || wake.request_repaint()), link::door_file()) {
            *pc.status = Notice::error(qymcad_i18n::tr1("assistant-cannot-listen", "why", &why));
            return;
        }
    }
    let Ok(mut held) = AWAITING.lock() else { return };
    let Some(w) = held.as_mut() else {
        if !at_rest(pc) {
            return;
        }
        let Some(request) = link::next() else { return };
        let mut ws = Live { pc, doing: String::new() };
        match tools::run(&mut ws, &request.tool, &request.args) {
            Err(why) => request.answer(Err(why)),
            Ok(done) => *held = Some(Awaiting { request, done, frames: 0, shot: Shot::NotAsked }),
        }
        ctx.request_repaint();
        return;
    };
    ctx.request_repaint();
    w.frames += 1;
    if w.frames < SETTLE_FRAMES || !at_rest(pc) {
        return;
    }
    if matches!(w.done, Done::Picture) {
        let frames = match w.shot {
            Shot::NotAsked => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                w.shot = Shot::Asked { frames: 0 };
                return;
            }
            Shot::Asked { frames } => frames,
        };
        let outcome = match picture(ctx) {
            Some(answer) => Ok(answer),
            None if frames < PICTURE_FRAMES => {
                w.shot = Shot::Asked { frames: frames + 1 };
                return;
            }
            None => Err("the window did not hand its picture back".to_string()),
        };
        if let Some(w) = held.take() {
            w.request.answer(outcome);
        }
        return;
    }
    if let Some(w) = held.take() {
        let answer = tools::settle(&Live { pc, doing: String::new() }, w.done);
        w.request.answer(Ok(answer));
    }
}

/// IS THE WINDOW AT REST: no operation open - the person's or a step of the assistant's - no rebuild asked for or
/// running, no import or export holding the window.
fn at_rest(pc: &PartCtx) -> bool {
    pc.edits.open.is_none() && !pc.regen.wanted && !pc.regen.pending && pc.regen.busy.is_none()
}

/// Refuse the call under way, when the window stops taking an assistant in the middle of it.
fn refuse_waiting(why: &str) {
    if let Some(w) = AWAITING.lock().ok().and_then(|mut held| held.take()) {
        w.request.answer(Err(why.to_string()));
    }
}

/// The picture of the window this frame handed back, as an answer; `None` while it has not come.
fn picture(ctx: &egui::Context) -> Option<qymcad_mcp::Value> {
    let image = ctx.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    })?;
    let shown = smaller(&image)?;
    let png = crate::gui::color_image_to_png(&shown)?;
    Some(qymcad_mcp::picture::answer(&png, qymcad_mcp::picture::Size { width: shown.size[0], height: shown.size[1] }))
}

/// The picture no wider than `PICTURE_WIDTH`, its proportions kept.
fn smaller(image: &egui::ColorImage) -> Option<egui::ColorImage> {
    let (w, h) = (image.size[0] as u32, image.size[1] as u32);
    if w <= PICTURE_WIDTH {
        return Some(image.clone());
    }
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
    let full = image::RgbaImage::from_raw(w, h, rgba)?;
    let height = (h * PICTURE_WIDTH / w).max(1);
    let small = image::imageops::resize(&full, PICTURE_WIDTH, height, image::imageops::FilterType::Triangle);
    Some(egui::ColorImage::from_rgba_premultiplied([PICTURE_WIDTH as usize, height as usize], small.as_raw()))
}

/// THE DOCUMENT OF THIS WINDOW as the tools see it: a step opens an undo step named for the person, and closes through
/// the window's own door - the caches brought into line, the rebuild handed to the scheduler, the step put on the stack.
struct Live<'p, 'a> {
    pc: &'p mut PartCtx<'a>,
    /// The step open now, by the tool's name, for the status line when it closes.
    doing: String,
}

impl Workshop for Live<'_, '_> {
    fn project(&self) -> &Project {
        self.pc.project
    }

    fn project_mut(&mut self) -> &mut Project {
        self.pc.project
    }

    fn begin(&mut self, what: &str) {
        self.doing = what.to_string();
        qymcad_ui_state::begin_edit(&mut *self.pc.edits, &*self.pc.project, qymcad_i18n::tr1("assistant-step", "what", what));
    }

    fn finish(&mut self) {
        qymcad_part::resync_after_topology_change(self.pc);
        qymcad_ui_state::commit_edit(&mut self.pc.rebuild());
        *self.pc.status = Notice::info(qymcad_i18n::tr1("assistant-did", "what", &self.doing));
    }

    fn shape(&self, body: Id) -> Option<&qymcad_kernel::Shape> {
        self.pc.live.shapes.get(&body)
    }

    fn place(&self) -> &'static str {
        "window"
    }

    /// The window keeps the file it is saved to; a call saves a copy where it is told.
    fn path(&self) -> Option<String> {
        None
    }

    fn new_document(&mut self) -> Result<(), String> {
        Err("the window holds the person's document: ask the person to start a new one in the window (File -> New), then go on".into())
    }

    fn open(&mut self, _path: &str) -> Result<(), String> {
        Err("the window holds the person's document: ask the person to open the file in the window (File -> Open), then go on".into())
    }

    fn saved_as(&mut self, _path: &str) {}

    /// The step taken back through the window's own history, as Ctrl+Z takes it; the window's path follows the
    /// document on the next frame, as after any change of context.
    fn undo(&mut self) -> Result<String, String> {
        let Some(step) = self.pc.edits.undo.last().map(|s| s.name.clone()) else { return Err("there is no step to take back".into()) };
        let _ = qymcad_ui_state::step_through_history(&mut self.pc.rebuild(), false);
        Ok(step)
    }

    /// The camera turned to the view and told to frame the model on the next frame it draws.
    fn look(&mut self, view: View) -> Result<(), String> {
        let eye = view.eye();
        self.pc.cam.yaw = eye.turn;
        self.pc.cam.pitch = eye.rise;
        self.pc.cam.roll = 0.0;
        self.pc.cam.init = false;
        Ok(())
    }
}
