//! THE BAR OF THE TOOL IN HAND: its name, its options as the tool describes them, Apply and Cancel at the end, and the
//! keys - Enter applies through the gate, Esc steps back once no field holds the keyboard.

use super::*;

/// A TOOL AS IT DESCRIBES ITSELF for a frame: its name, the options of its bar, its gate and what it says.
pub struct Described {
    pub head: Head,
    pub bar: Vec<Opt>,
    pub gate: Gate,
    pub says: Vec<Notice>,
    /// Its box at the geometry, when it has one now.
    pub boxed: Option<BoxSpec>,
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
    /// What the fields of a box did besides their values; a bar has no such fields.
    pub fields: Vec<FieldDid>,
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
        did = draw_opts(ui, pal, &tool.bar);
        // the ends outermost, the count of what is selected beside them, as the bar always told it
        pressed = end_of_bar(ui, |ui| {
            let pressed = the_ends(pal, ui, tool.head.ends, &tool.gate);
            for o in &tool.bar {
                if let Opt::Tally(words) = o {
                    ui.label(egui::RichText::new(words.as_str()).weak());
                }
            }
            pressed
        })
        .inner;
    });
    let enter = !typing && ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.command);
    // Esc with Shift, Ctrl or Alt held is Esc all the same, as it always was in a sketch
    let esc = !typing && ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let keys = Keys { enter: if enter { Enter::Pressed } else { Enter::Not }, esc: if esc { Esc::Pressed } else { Esc::Not } };
    let gate = tool.gate.clone();
    let verdict = verdict(&ctx, pressed, keys, &gate);
    // A VALUE REFUSED, A PICK THAT WILL NOT DO OR A TRIAL STILL RUNNING is said while it stands, not only when Enter is
    // pressed: the field marks itself, the words say which and why; the answer to a refused Enter follows, unless it says
    // the same. Reported behaviour: the trial of a fillet ran for minutes, Apply grey, and the words that it checks came
    // only with Enter.
    let standing = match &gate {
        Gate::Refused(_) | Gate::Wrong(_) | Gate::Checking => gate.answer(),
        _ => None,
    };
    let mut words: Vec<Notice> = standing.clone().into_iter().collect();
    words.extend(remembered_refusal(&ctx, &gate).filter(|r| Some(r) != standing.as_ref()));
    words.extend(tool.says.iter().cloned());
    lay_words(&ctx, look.canvas, pal, &words);
    Answer { verdict, did, fields: Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A TRIAL STILL RUNNING IS SAID IN THE CORNER OF THE VIEW AT ONCE, with no Enter pressed. Reported behaviour: the trial of a fillet ran for minutes, Apply grey, and the words that
    /// it checks came only with Enter.
    #[test]
    fn a_trial_running_is_said_before_enter() {
        let ctx = egui::Context::default();
        let (scheme, project) = (SchemeUi::default(), Project::default());
        let tool =
            Described { head: Head { icon: "", name: "Fillet".into(), ends: Ends::ApplyCancel, after: AfterApply::Select }, bar: Vec::new(), gate: Gate::Checking, says: Vec::new(), boxed: None };
        let checking = qymcad_i18n::tr("cmd-trial-checking");
        for frame in 0..3 {
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                let mut look = BarLook { scheme: &scheme, project: &project, bar_exprs: None, canvas: egui::Rect::NOTHING };
                let _ = tool_bar(ui, &mut look, &tool);
            });
            let said = words_laid(&ctx);
            assert!(said.iter().any(|n| n.words == checking), "frame {frame}: the corner says {said:?}, not that the values are being checked");
        }
    }
}
