//! THE COMMAND CATALOGUE - one source for the bars, their "More" lists, the search, the keys and the help.
//!
//! WHY A CATALOGUE AND NOT A LIST INSIDE THE SEARCH. A search that knows its own list will drift from
//! the panel at the very first new feature — exactly as the canvas drifted from the list of joints and
//! the help from the datums. The bar itself used to be such a second list: its buttons were written one by
//! one in the code, and a guard read the source to hold them to the rows. Now the rows are given by the
//! workbenches and the bar is drawn from them: a button IS a row, and there is nothing left to hold together.
//!
//! THE NAME OF A COMMAND IS TAKEN FROM THE TITLE OF ITS HELP ARTICLE. That is not a shortcut but a
//! consequence: every tool already has an article (the `help_map_flow` guard requires it), its title
//! is translated into both languages and written in the same words a person sees in the help. A
//! separate list of names would become a second place of truth and would drift from the first.
//!
//! The exception is commands that share ONE article between several (the six primitives): those need
//! a name of their own, otherwise the search shows six identical rows reading "Primitives".

pub use qymcad_ui_state::catalogue::{Command, Launch};

/// THE WORKBENCHES THAT GIVE COMMANDS, in the order of their bars - the sketch, the part, the assembly.
pub(crate) const BENCHES: [&qymcad_ui_state::catalogue::Offer; 3] = [&qymcad_sketch::OFFER, &qymcad_part::OFFER, &qymcad_assembly::OFFER];

/// EVERY COMMAND: the rows each workbench gives, in the order of their bars.
pub fn all() -> impl Iterator<Item = &'static Command> {
    BENCHES.into_iter().flat_map(|b| b.rows.iter())
}

/// A command by its code.
pub fn by_code(code: &str) -> Option<&'static Command> {
    all().find(|c| c.code == code)
}

/// THE COMMANDS ON THE BAR OF `bench`, in their order.
pub fn of_bench(bench: &str) -> Vec<&'static Command> {
    all().filter(|c| c.on(bench)).collect()
}

/// THE ROW OF THE DATUM PLANE: one row, which the bars of the part and of the assembly both show.
pub const DATUM_PLANE: &str = "part.datum-plane";

/// THE TOOL OF A PART under the code of a row of the catalogue - what a key bound to that code takes.
pub fn part_tool_of(code: &str) -> Option<qymcad_ui_state::PartTool> {
    match by_code(code)?.launch {
        Launch::Feat(t) | Launch::Prim(t) => Some(t),
        _ => None,
    }
}

/// PRESS A COMMAND, through the workbench that gave its row: its button, its row of "More" and the search come here
/// alike. The command in hand pressed again is put down (`catalogue::again`).
pub fn press(cmd: &Command, bc: &mut qymcad_ui_state::BarCtx) {
    if let Some(down) = qymcad_ui_state::catalogue::again(cmd.launch, bc) {
        bc.ask.push(down);
        return;
    }
    if let Some(bench) = BENCHES.into_iter().find(|b| b.rows.iter().any(|r| r.code == cmd.code)) {
        (bench.press)(cmd.launch, bc);
    }
}

/// THE KEY OF THE COMMAND IN HAND PRESSED AGAIN puts it down, as its button pressed again does: what is asked for is in
/// `bc.ask`, and the key is spent. Any other key is left to the keys of its workbench. A letter typed into a field never
/// reaches here (`pressed_chord`).
pub fn key_again(bc: &mut qymcad_ui_state::BarCtx, key: qymcad_ui_state::Chord) -> bool {
    let area = if qymcad_ui_state::edit_si(&*bc.project, &*bc.sketch_ses).is_some() {
        "sketch"
    } else {
        match *bc.workbench {
            qymcad_ui_state::Workbench::Part => "part",
            qymcad_ui_state::Workbench::Assembly => "assembly",
            _ => return false,
        }
    };
    let Some(cmd) = qymcad_ui_state::hotkey_action(&*bc.set, area, key).and_then(command_of_key) else { return false };
    let Some(down) = qymcad_ui_state::catalogue::again(cmd.launch, bc) else { return false };
    bc.ask.push(down);
    true
}

/// THE ROW OF THE CATALOGUE A KEY TAKES: the row under the code of its action, or for the keys whose action is named
/// apart from the row they take, that row. A cut is no command of its own - it is the extrusion with the operation of a
/// cut - and has none.
fn command_of_key(action: &str) -> Option<&'static Command> {
    let code = match action {
        "sketch.corner-fillet" => "sketch.corner",
        "part.sketch-pick" => "part.sketch",
        "assembly.rigid-joint" => "assembly.joint",
        "assembly.datum-plane" => DATUM_PLANE,
        other => other,
    };
    by_code(code)
}

/// WHAT A PERSON CALLS A COMMAND, and where its help is.
pub trait Named {
    /// THE NAME FOR A PERSON: its own if one is set, otherwise the title of the help article.
    ///
    /// The title comes from the article itself, so it is translated and written in the same words a
    /// person has already read. If there is no article the code is returned: an empty row in the
    /// search is worse than an ugly one.
    fn name(&self) -> String;
    /// THE NAME IN A PARTICULAR LANGUAGE — for searching in the other language.
    ///
    /// The first edition searched the second language only for commands with a key OF THEIR OWN, while
    /// for the rest the name comes from the article title — and `fillet` was not found in English. The
    /// hole was visible on a snapshot exactly: a two-letter prefix found one row instead of three.
    fn name_in(&self, lang: &str) -> String;
    /// The help article of this command: through the same table F1 uses, or the one of its button's hint.
    fn help_article(&self) -> Option<&'static str>;
}

impl Named for Command {
    fn name(&self) -> String {
        if !self.name_key.is_empty() {
            return crate::i18n::tr(self.name_key);
        }
        match self.help_article() {
            Some(a) => crate::help::title(a),
            None => self.code.to_string(),
        }
    }

    fn name_in(&self, lang: &str) -> String {
        if !self.name_key.is_empty() {
            return crate::i18n::tr_in(lang, self.name_key).unwrap_or_default();
        }
        self.help_article().and_then(|a| crate::help::title_in(lang, a)).unwrap_or_default()
    }

    fn help_article(&self) -> Option<&'static str> {
        let by_launch = match self.launch {
            Launch::Feat(_) | Launch::Prim(_) => crate::help_map::part_article(self.code),
            Launch::SkTool(_) | Launch::Dim(_) | Launch::ClickOp(_) | Launch::Modify(_) => crate::help_map::sketch_article(self.code),
            Launch::Joint => crate::help_map::assembly_article("asm.joint"),
            Launch::PieceToPart => Some("part/13-split-body"),
            _ => None,
        };
        by_launch.or_else(|| crate::help_map::toolbar_article(self.hint_key))
    }
}

/// THE ARTICLE OF THE TOOL IN HAND, whichever kind it is - a tool of a part, a drawing tool of a sketch, a dimension, a
/// tool acting on a click, a button editing what is picked: the command whose press put it in hand, and the article of
/// that command.
pub fn article_in_hand(armed: &qymcad_ui_state::Armed) -> Option<&'static str> {
    use qymcad_ui_state::Armed;
    let launch = match *armed {
        Armed::Command(t) if t.is_a_primitive() => Launch::Prim(t),
        Armed::Command(t) => Launch::Feat(t),
        Armed::Draw(s) => Launch::SkTool(s),
        Armed::ClickOp(c) => Launch::ClickOp(c),
        Armed::Dimension(k) => Launch::Dim(k),
        Armed::Modify(t) => Launch::Modify(t),
        _ => return None,
    };
    all().find(|c| c.launch == launch).and_then(|c| c.help_article())
}

/// IS THERE A BUTTON FOR `launch` on the bar of `bench`: a row of the catalogue on its shelf.
#[cfg(test)]
pub(crate) fn has_button(launch: Launch, bench: &str) -> bool {
    all().any(|c| c.launch == launch && c.on(bench))
}

#[cfg(test)]
mod tests {
    use super::{by_code, Launch};

    /// A COMMAND FOUND BY NAME OPENS THE TOOL OF THAT NAME: the row of the datum axis starts the axis and shows the
    /// hint of the axis. Reported behaviour: Ctrl+K "Datum axis" opened the point (the bar "Point", the fields X, Y, Z)
    /// and "Datum point" the axis - the catalogue and the window had the two kinds the other way round.
    /// Now the button is the row, and the row is checked against the kinds the bar of the command is built for.
    #[test]
    fn a_command_found_by_name_launches_the_kind_of_its_own_button() {
        for (code, kind, hint) in [
            ("part.datum-axis", qymcad_ui_state::PartTool::DatumAxis, "g-datum-axis-hint"),
            ("part.datum-point", qymcad_ui_state::PartTool::DatumPoint, "g-datum-point-hint"),
        ] {
            let c = by_code(code).unwrap_or_else(|| panic!("{code} is not in the catalogue"));
            assert_eq!(c.launch, Launch::Feat(kind), "{code} launches {:?}", c.launch);
            assert_eq!(c.hint_key, hint, "{code} shows the hint of another button");
        }
    }
}
