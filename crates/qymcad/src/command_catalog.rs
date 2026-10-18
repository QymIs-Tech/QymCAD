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

/// EVERY COMMAND: the rows each workbench gives, in the order of their bars - the sketch, the part, the assembly.
pub fn all() -> impl Iterator<Item = &'static Command> {
    qymcad_part::SKETCH_COMMANDS.iter().chain(qymcad_part::PART_COMMANDS).chain(qymcad_assembly::COMMANDS)
}

/// A command by its code.
pub fn by_code(code: &str) -> Option<&'static Command> {
    all().find(|c| c.code == code)
}

/// THE COMMANDS ON THE BAR OF `bench`, in their order.
pub fn of_bench(bench: &str) -> Vec<&'static Command> {
    all().filter(|c| c.on(bench)).collect()
}

/// PRESS A COMMAND, through the workbench it belongs to: its button, its row of "More" and the search come here
/// alike.
pub fn press(cmd: &Command, bc: &mut qymcad_ui_state::BarCtx) {
    if cmd.home() == "assembly" {
        qymcad_assembly::press(cmd.launch, bc);
    } else {
        qymcad_part::press(cmd.launch, bc);
    }
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
            Launch::Feat(n) | Launch::Prim(n) => crate::help_map::part_article(n),
            Launch::SkTool(n) => crate::help_map::sketch_article("sk", n),
            Launch::Dim(n) => crate::help_map::sketch_article("dim", n),
            Launch::ClickOp(n) => crate::help_map::sketch_article("click", n),
            Launch::Modify(tool) => crate::help_map::sketch_article("mod", tool.code()),
            Launch::Joint => crate::help_map::assembly_article("asm.joint"),
            Launch::PieceToPart => Some("part/13-split-body"),
            _ => None,
        };
        by_launch.or_else(|| crate::help_map::toolbar_article(self.hint_key))
    }
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
    /// and "Datum point" the axis - the catalogue had 21 for the axis and 22 for the point, the window the other way.
    /// Now the button is the row, and the row is checked against the kinds the bar of the command is built for.
    #[test]
    fn a_command_found_by_name_launches_the_kind_of_its_own_button() {
        for (code, kind, hint) in [("part.datum-axis", 22, "g-datum-axis-hint"), ("part.datum-point", 21, "g-datum-point-hint")] {
            let c = by_code(code).unwrap_or_else(|| panic!("{code} is not in the catalogue"));
            assert_eq!(c.launch, Launch::Feat(kind), "{code} launches {:?}", c.launch);
            assert_eq!(c.hint_key, hint, "{code} shows the hint of another button");
        }
    }
}
