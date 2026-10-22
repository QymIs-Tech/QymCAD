//! THE BAR OF THE TOOL IN HAND: its name, its options as the tool describes them, Apply and Cancel at the end, and the
//! keys - Enter applies through the gate, Esc steps back once no field holds the keyboard.

use super::*;

/// A TOOL AS IT DESCRIBES ITSELF for a frame: its name, the options of its bar, its gate and what it says.
pub struct Described {
    pub head: Head,
    pub bar: Vec<Opt>,
    pub gate: Gate,
    pub says: Vec<Notice>,
}

/// WHAT THE BAR IS DRAWN WITH: the colours, the document a number's formula reads and where its text is kept - a bar
/// with no numbers keeps none - and the view the words of the tool lie in the corner of.
pub struct BarLook<'a> {
    pub scheme: &'a SchemeUi,
    pub project: &'a Project,
    pub bar_exprs: Option<&'a mut std::collections::HashMap<&'static str, String>>,
    pub canvas: egui::Rect,
}

/// WHAT THE PERSON DID IN A FRAME: to the options, and to the tool.
pub struct Answer {
    pub verdict: Verdict,
    pub did: Vec<Did>,
}

/// THE BAR OF THE TOOL IN HAND, drawn from its description, and its words laid in the corner of the view. Enter - on the
/// keyboard with no field holding it, or in a field of the bar - applies through the gate, as the Apply button does; a
/// refusal is said with the tool's words. Esc steps back once no field holds the keyboard: a field takes it first, and
/// gives the keyboard up.
pub fn tool_bar(ui: &mut egui::Ui, look: &mut BarLook, tool: &Described) -> Answer {
    let ctx = ui.ctx().clone();
    // asked before anything of the bar is drawn: a field of the bar takes Esc and gives the keyboard up in this frame
    let typing = ctx.egui_wants_keyboard_input();
    let pal = &look.scheme.pal;
    let mut did = Vec::new();
    let mut pressed = Pressed::Nothing;
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(format!("{} {}", tool.head.icon, tool.head.name)).strong());
        ui.separator();
        let mut numbers = look.bar_exprs.as_deref_mut().map(|bar_exprs| ExprBarCtx { bar_exprs, project: look.project, scheme: look.scheme });
        did = draw_opts(ui, &tool.bar, numbers.as_mut());
        let can = if tool.gate.open() && bar_fields_valid(ui.ctx()) { Can::Yes } else { Can::No };
        pressed = end_of_bar(ui, |ui| the_ends(pal, ui, tool.head.ends, can)).inner;
    });
    let enter = bar_enter_marked(&ctx) || (!typing && ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.command));
    let esc = !typing && ctx.input(|i| i.key_pressed(egui::Key::Escape) && !i.modifiers.any());
    let keys = Keys { enter: if enter { Enter::Pressed } else { Enter::Not }, esc: if esc { Esc::Pressed } else { Esc::Not } };
    // a field of the bar that holds a value it cannot take shuts the gate the tool answered
    let gate = if tool.gate.open() && !bar_fields_valid(&ctx) { Gate::BarField } else { tool.gate.clone() };
    let verdict = verdict(&ctx, pressed, keys, &gate);
    // A VALUE REFUSED OR A PICK THAT WILL NOT DO is said while it stands, not only when Enter is pressed: the field marks
    // itself, the words say which and why; the answer to a refused Enter follows, unless it says the same
    let standing = match &gate {
        Gate::Refused(_) | Gate::Wrong(_) => gate.answer(),
        _ => None,
    };
    let mut words: Vec<Notice> = standing.clone().into_iter().collect();
    words.extend(remembered_refusal(&ctx, &gate).filter(|r| Some(r) != standing.as_ref()));
    words.extend(tool.says.iter().cloned());
    lay_words(&ctx, look.canvas, pal, &words);
    Answer { verdict, did }
}
