//! THE COMMAND CATALOGUE — one source for the search, the panel, the keys and the help.
//!
//! WHY A CATALOGUE AND NOT A LIST INSIDE THE SEARCH. A search that knows its own list will drift from
//! the panel at the very first new feature — exactly as the canvas drifted from the list of joints and
//! the help from the datums. Both times it was fixed the same way: one rule, and a guard that holds
//! the link. The same here: a row of the catalogue must have a button in the panel, and a button must
//! have a row in the catalogue.
//!
//! THE NAME OF A COMMAND IS TAKEN FROM THE TITLE OF ITS HELP ARTICLE. That is not a shortcut but a
//! consequence: every tool already has an article (the `help_map_flow` guard requires it), its title
//! is translated into both languages and written in the same words a person sees in the help. A
//! separate list of names would become a second place of truth and would drift from the first.
//!
//! The exception is commands that share ONE article between several (the six primitives): those need
//! a name of their own, otherwise the search shows six identical rows reading "Primitives".

/// WHAT LAUNCHES A COMMAND. Exactly the same calls the panel button makes: the search has no right to
/// introduce a launch path of its own — otherwise it starts doing what the button does not.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Launch {
    /// A command of the feature timeline (`start_feat_cmd`).
    Feat(u8),
    /// A primitive (`start_prim_cmd`).
    Prim(u8),
    /// A sketch drawing tool (`set_sk_tool`).
    SkTool(u8),
    /// A sketch dimension (`set_dim_tool`).
    Dim(u8),
    /// A click operation of a sketch: trim, extend, project (`set_click_op`).
    ClickOp(u8),
    /// Editing the selection in a sketch: delete, mirror, offset (`modify_button`).
    Modify(u8),
    /// An assembly or view action with a launch of its own (mate, ground, measure).
    Action(&'static str),
}

/// One command of the catalogue.
pub struct Command {
    /// A stable code: `area.name`, as with the hotkeys. With a dot rather than a hyphen — the
    /// localisation guard catches ANY literal shaped like a catalogue key, and the code is never shown
    /// on screen.
    pub code: &'static str,
    /// The workbench the command lives in: `sketch` / `part` / `assembly`.
    pub workbench: &'static str,
    /// What launches it.
    pub launch: Launch,
    /// A name OF ITS OWN (a key of the language catalogue) — only where one article is shared by
    /// several commands. Empty means the name comes from the title of the help article.
    pub name_key: &'static str,
}

/// EVERY COMMAND. The order is that of the workbench panel: from "draw" to "fix".
pub const COMMANDS: &[Command] = &[
    // ── Sketch ──────────────────────────────────────────────────────────────────────────────
    Command { code: "sketch.line", workbench: "sketch", launch: Launch::SkTool(1), name_key: "" },
    Command { code: "sketch.rect", workbench: "sketch", launch: Launch::SkTool(2), name_key: "" },
    Command { code: "sketch.circle", workbench: "sketch", launch: Launch::SkTool(3), name_key: "" },
    Command { code: "sketch.circle3", workbench: "sketch", launch: Launch::SkTool(10), name_key: "cmdname-circle-3pt" },
    Command { code: "sketch.arc", workbench: "sketch", launch: Launch::SkTool(4), name_key: "" },
    Command { code: "sketch.point", workbench: "sketch", launch: Launch::SkTool(5), name_key: "" },
    Command { code: "sketch.polygon", workbench: "sketch", launch: Launch::SkTool(6), name_key: "" },
    Command { code: "sketch.slot", workbench: "sketch", launch: Launch::SkTool(7), name_key: "" },
    Command { code: "sketch.ellipse", workbench: "sketch", launch: Launch::SkTool(8), name_key: "" },
    Command { code: "sketch.spline", workbench: "sketch", launch: Launch::SkTool(9), name_key: "" },
    Command { code: "sketch.text", workbench: "sketch", launch: Launch::SkTool(11), name_key: "" },
    Command { code: "sketch.dim", workbench: "sketch", launch: Launch::Dim(1), name_key: "" },
    Command { code: "sketch.dim-angle", workbench: "sketch", launch: Launch::Dim(2), name_key: "cmdname-dim-angle" },
    Command { code: "sketch.dim-radius", workbench: "sketch", launch: Launch::Dim(3), name_key: "cmdname-dim-radius" },
    Command { code: "sketch.trim", workbench: "sketch", launch: Launch::ClickOp(1), name_key: "" },
    Command { code: "sketch.extend", workbench: "sketch", launch: Launch::ClickOp(2), name_key: "" },
    Command { code: "sketch.break", workbench: "sketch", launch: Launch::ClickOp(3), name_key: "" },
    Command { code: "sketch.project", workbench: "sketch", launch: Launch::ClickOp(4), name_key: "" },
    Command { code: "sketch.corner", workbench: "sketch", launch: Launch::ClickOp(5), name_key: "" },
    Command { code: "sketch.project-body", workbench: "sketch", launch: Launch::ClickOp(6), name_key: "" },
    Command { code: "sketch.delete", workbench: "sketch", launch: Launch::Modify(0), name_key: "" },
    Command { code: "sketch.mirror", workbench: "sketch", launch: Launch::Modify(1), name_key: "" },
    Command { code: "sketch.offset", workbench: "sketch", launch: Launch::Modify(6), name_key: "" },
    // ── Part ────────────────────────────────────────────────────────────────────────────────
    Command { code: "part.extrude", workbench: "part", launch: Launch::Feat(1), name_key: "" },
    Command { code: "part.revolve", workbench: "part", launch: Launch::Feat(3), name_key: "" },
    Command { code: "part.sweep", workbench: "part", launch: Launch::Feat(8), name_key: "" },
    Command { code: "part.loft", workbench: "part", launch: Launch::Feat(9), name_key: "" },
    Command { code: "part.fillet", workbench: "part", launch: Launch::Feat(4), name_key: "" },
    Command { code: "part.chamfer", workbench: "part", launch: Launch::Feat(5), name_key: "" },
    Command { code: "part.shell", workbench: "part", launch: Launch::Feat(6), name_key: "" },
    Command { code: "part.hole", workbench: "part", launch: Launch::Feat(7), name_key: "" },
    Command { code: "part.draft", workbench: "part", launch: Launch::Feat(23), name_key: "" },
    Command { code: "part.thread", workbench: "part", launch: Launch::Feat(24), name_key: "" },
    Command { code: "part.push-face", workbench: "part", launch: Launch::Feat(25), name_key: "" },
    Command { code: "part.remove-face", workbench: "part", launch: Launch::Feat(26), name_key: "" },
    Command { code: "part.split-body", workbench: "part", launch: Launch::Feat(27), name_key: "" },
    Command { code: "part.split-face", workbench: "part", launch: Launch::Feat(29), name_key: "" },
    Command { code: "part.thicken", workbench: "part", launch: Launch::Feat(28), name_key: "" },
    // the bridge from the parametric side into the design layer — the name comes from the article
    // title, as with the neighbours
    Command { code: "part.face-copy", workbench: "part", launch: Launch::Feat(30), name_key: "" },
    Command { code: "part.surface-replace", workbench: "part", launch: Launch::Feat(31), name_key: "" },
    Command { code: "part.patch", workbench: "part", launch: Launch::Feat(32), name_key: "" },
    Command { code: "part.stitch", workbench: "part", launch: Launch::Feat(33), name_key: "" },
    Command { code: "part.trim", workbench: "part", launch: Launch::Feat(34), name_key: "" },
    Command { code: "part.mirror", workbench: "part", launch: Launch::Feat(16), name_key: "" },
    Command { code: "part.array-linear", workbench: "part", launch: Launch::Feat(17), name_key: "" },
    Command { code: "part.array-circular", workbench: "part", launch: Launch::Feat(18), name_key: "" },
    Command { code: "part.datum-plane", workbench: "part", launch: Launch::Feat(20), name_key: "cmdname-datum-plane" },
    Command { code: "part.datum-axis", workbench: "part", launch: Launch::Feat(21), name_key: "cmdname-datum-axis" },
    Command { code: "part.datum-point", workbench: "part", launch: Launch::Feat(22), name_key: "cmdname-datum-point" },
    // THE SIX PRIMITIVES — a caption of its own for each: they share ONE article, and without names
    // the search would show six identical rows reading "Primitives".
    Command { code: "part.box", workbench: "part", launch: Launch::Prim(10), name_key: "cmdname-box" },
    Command { code: "part.cylinder", workbench: "part", launch: Launch::Prim(11), name_key: "cmdname-cylinder" },
    Command { code: "part.sphere", workbench: "part", launch: Launch::Prim(12), name_key: "cmdname-sphere" },
    Command { code: "part.cone", workbench: "part", launch: Launch::Prim(13), name_key: "cmdname-cone" },
    Command { code: "part.torus", workbench: "part", launch: Launch::Prim(14), name_key: "cmdname-torus" },
    Command { code: "part.prism", workbench: "part", launch: Launch::Prim(15), name_key: "cmdname-prism" },
    // ── Assembly ────────────────────────────────────────────────────────────────────────────
    Command { code: "assembly.joint", workbench: "assembly", launch: Launch::Action("joint"), name_key: "" },
    Command { code: "assembly.ground", workbench: "assembly", launch: Launch::Action("ground"), name_key: "cmdname-ground" },
];

/// A command by its code.
pub fn by_code(code: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.code == code)
}

impl Command {
    /// THE NAME FOR A PERSON: its own if one is set, otherwise the title of the help article.
    ///
    /// The title comes from the article itself, so it is translated and written in the same words a
    /// person has already read. If there is no article the code is returned: an empty row in the
    /// search is worse than an ugly one.
    pub fn name(&self) -> String {
        if !self.name_key.is_empty() {
            return crate::i18n::tr(self.name_key);
        }
        match self.help_article() {
            Some(a) => crate::help::title(a),
            None => self.code.to_string(),
        }
    }

    /// THE NAME IN A PARTICULAR LANGUAGE — for searching in the other language.
    ///
    /// The first edition searched the second language only for commands with a key OF THEIR OWN, while
    /// for the rest the name comes from the article title — and `fillet` was not found in English. The
    /// hole was visible on a snapshot exactly: a two-letter prefix found one row instead of three.
    pub fn name_in(&self, lang: &str) -> String {
        if !self.name_key.is_empty() {
            return crate::i18n::tr_in(lang, self.name_key).unwrap_or_default();
        }
        self.help_article().and_then(|a| crate::help::title_in(lang, a)).unwrap_or_default()
    }

    /// The help article of this command — through the same table F1 uses.
    pub fn help_article(&self) -> Option<&'static str> {
        match self.launch {
            Launch::Feat(n) | Launch::Prim(n) => crate::help_map::part_article(n),
            Launch::SkTool(n) => crate::help_map::sketch_article("sk", n),
            Launch::Dim(n) => crate::help_map::sketch_article("dim", n),
            Launch::ClickOp(n) => crate::help_map::sketch_article("click", n),
            Launch::Modify(n) => crate::help_map::sketch_article("mod", n),
            Launch::Action("joint") => crate::help_map::assembly_article("asm.joint"),
            Launch::Action(_) => None,
        }
    }
}
