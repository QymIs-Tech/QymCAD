//! TOOL TO ARTICLE: what F1 opens the right thing by.
//!
//! HELP ROTS SILENTLY. A tool gets renamed, split in two, removed — and the article stays and goes on
//! lying confidently. The only thing that saves from that is a link the build checks: every tool in
//! the dispatcher must have a row here, and every row must have a tool that exists.
//!
//! ONE ARTICLE FOR SEVERAL TOOLS IS NORMAL. The six primitives (box, cylinder, sphere, cone, torus,
//! prism) differ in their fields rather than in their meaning: six nearly identical articles are worse
//! to read than one. The reverse is forbidden: a tool without an article is an F1 that does not
//! answer.
//!
//! THE KEYS HERE ARE THOSE OF THE CODE - the codes of the commands of the catalogue: the
//! table is compared against the source of the panels, so "added a tool, forgot the article" reddens
//! the build.

/// The commands of the Part: the code of the command -> the article.
pub const PART: &[Article] = &[
    Article { code: "part.extrude", article: "part/01-extrude" },
    Article { code: "part.revolve", article: "part/02-revolve" },
    Article { code: "part.sweep", article: "part/03-sweep" },
    Article { code: "part.loft", article: "part/04-loft" },
    Article { code: "part.fillet", article: "part/05-fillet" },
    Article { code: "part.chamfer", article: "part/06-chamfer" },
    Article { code: "part.shell", article: "part/07-shell" },
    Article { code: "part.hole", article: "part/08-hole" },
    Article { code: "part.draft", article: "part/09-draft" },
    Article { code: "part.thread", article: "part/10-thread" },
    Article { code: "part.push-face", article: "part/11-push-face" },
    Article { code: "part.remove-face", article: "part/12-remove-face" },
    Article { code: "part.split-body", article: "part/13-split-body" },
    Article { code: "part.split-face", article: "part/14-split-face" },
    Article { code: "part.thicken", article: "part/15-thicken" },
    Article { code: "part.face-copy", article: "part/20-face-copy" },
    Article { code: "part.offset-surface", article: "part/27-offset-surface" },
    Article { code: "part.surface-replace", article: "part/21-surface-replace" },
    Article { code: "part.patch", article: "part/22-patch" },
    Article { code: "part.stitch", article: "part/23-stitch" },
    Article { code: "part.trim", article: "part/24-trim" },
    Article { code: "part.recognise", article: "part/26-recognise" },
    Article { code: "part.mirror", article: "part/16-mirror" },
    Article { code: "part.array-linear", article: "part/17-linear-array" },
    Article { code: "part.array-circular", article: "part/18-circular-array" },
    // DATUMS are commands too, and are shared by the Part and the Assembly: their buttons live in the
    // common creation panel rather than in the workbench panel. One article for three: a plane, an axis
    // and a point answer one question — what to build from — and differ only in what they set.
    Article { code: "part.datum-plane", article: "general/04-datums" },
    Article { code: "part.datum-point", article: "general/04-datums" },
    Article { code: "part.datum-axis", article: "general/04-datums" },
    // six primitives, one article: they differ in their fields rather than in their meaning
    Article { code: "part.box", article: "part/19-primitives" },
    Article { code: "part.cylinder", article: "part/19-primitives" },
    Article { code: "part.sphere", article: "part/19-primitives" },
    Article { code: "part.cone", article: "part/19-primitives" },
    Article { code: "part.torus", article: "part/19-primitives" },
    Article { code: "part.prism", article: "part/19-primitives" },
];

/// ONE ROW OF A TABLE OF THE ARTICLES: the code of a command of the catalogue and its article.
pub struct Article {
    pub code: &'static str,
    pub article: &'static str,
}

/// The tools of the Sketch: the code of the command -> the article. Keyed by the code of the command rather than by a
/// number of the tool: a number belongs to its kind of tool, a line and a trim were both 1, and a key glued of a kind
/// and a number was a second name for what the code already names.
pub const SKETCH: &[Article] = &[
    Article { code: "sketch.line", article: "sketch/01-line" },
    Article { code: "sketch.rect", article: "sketch/02-rect" },
    Article { code: "sketch.circle", article: "sketch/03-circle" },
    // a circle by three points is the same tool set a different way: one article for both
    Article { code: "sketch.arc", article: "sketch/04-arc" },
    Article { code: "sketch.point", article: "sketch/05-point" },
    Article { code: "sketch.polygon", article: "sketch/06-polygon" },
    Article { code: "sketch.slot", article: "sketch/07-slot" },
    Article { code: "sketch.ellipse", article: "sketch/08-ellipse" },
    Article { code: "sketch.spline", article: "sketch/09-spline" },
    Article { code: "sketch.text", article: "sketch/10-text" },
    Article { code: "sketch.dim", article: "sketch/11-dimensions" },
    Article { code: "sketch.dim-angle", article: "sketch/11-dimensions" },
    Article { code: "sketch.dim-radius", article: "sketch/11-dimensions" },
    Article { code: "sketch.trim", article: "sketch/12-trim" },
    Article { code: "sketch.extend", article: "sketch/13-extend" },
    Article { code: "sketch.break", article: "sketch/14-break" },
    // the rounding of a corner and its chamfer share an article; the projection is one tool, of edges and of a body alike
    Article { code: "sketch.corner", article: "sketch/16-corner" },
    Article { code: "sketch.corner-chamfer", article: "sketch/16-corner" },
    Article { code: "sketch.project", article: "sketch/15-project" },
    Article { code: "sketch.delete", article: "sketch/18-delete" },
    Article { code: "sketch.mirror", article: "sketch/19-mirror" },
    Article { code: "sketch.offset", article: "sketch/20-offset" },
];

/// THE MODES OF THE ASSEMBLY -> the article. The tools of the Assembly are not timeline commands but
/// modes: picking faces for a mate, laying out a pattern of components. So the key here is a string
/// rather than a command number.
///
/// WITH A DOT, as with the action codes of the hotkeys: the keys of the language catalogue are written
/// with hyphens, and the guard against a key reaching the screen untranslated catches any literal of
/// that shape. It caught the first edition too — `comp-array` was indistinguishable from a catalogue
/// key.
///
/// EVERY ASSEMBLY TOOL MUST HAVE A ROW HERE (`gui::assembly_tools::AssemblyTool`), not just the two
/// somebody got round to: an F1 that answers with a table of contents is an F1 that did not answer.
pub const ASSEMBLY: &[(&str, &str)] = &[
    ("asm.joint", "assembly/02-joints"),
    ("asm.anchor", "assembly/02-joints"),
    ("asm.group", "assembly/02-joints"),
    ("asm.width", "assembly/02-joints"),
    ("asm.tangent", "assembly/02-joints"),
    ("asm.relation", "assembly/07-relations"),
    ("asm.ground", "assembly/02-joints"),
    ("asm.axis", "assembly/02-joints"),
    ("asm.repick", "assembly/02-joints"),
    ("asm.comp-array", "assembly/04-arrays"),
    ("asm.interference", "assembly/05-interference"),
];

/// The article of an Assembly mode.
pub fn assembly_article(mode: &str) -> Option<&'static str> {
    ASSEMBLY.iter().find(|(m, _)| *m == mode).map(|(_, a)| *a)
}

/// THE BAR BUTTONS THAT DO NOT LAUNCH A NUMBERED COMMAND -> the article.
///
/// WHY A SEPARATE TABLE. The guard looked for tools BY NUMBER (`start_feat_cmd(N)`,
/// `set_sk_tool(N)`), and only 55 of the 91 buttons in the panels are like that: the rest call a
/// handle of their own (`arm_body_boolean`, `start_move_tool`, `start_pattern` and so on). The rule
/// "a tool has an article" did not touch them, and the holes piled up silently — a measurement found
/// six: the boolean of bodies, move/copy/rotate in a sketch, the sketch pattern, measuring, the parts
/// library and the auger.
///
/// The key is the tooltip of the button (`tb-...`): the button already has one, it is visible in the
/// source of the panel and it was not invented specially for the guard. A button is created together
/// with its tooltip — so a row here comes with it.
pub const TOOLBAR: &[(&str, &str)] = &[
    ("tb-bool-bodies-hint", "part/25-boolean"),
    ("tb-copy-hint", "sketch/21-move-copy-rotate"),
    ("tb-move-hint", "sketch/21-move-copy-rotate"),
    ("tb-rotate-hint", "sketch/21-move-copy-rotate"),
    ("tb-lin-array-hint", "sketch/22-array"),
    ("tb-circ-array-hint", "sketch/22-array"),
    ("tb-measure-hint", "general/11-measure"),
    ("tb-measure3d-hint", "general/11-measure"),
    ("tb-insert-component-hint", "general/12-library"),
    // rounding ALL the corners of a contour is the same corner tool, only all at once
    ("tb-fillet-all-hint", "sketch/16-corner"),
    // construction geometry and the selection arrow are described in the sketch section rather than
    // by articles of their own
    ("tb-construction-hint", "sketch/23-construction"),
    ("tb-select-hint", "sketch/24-select"),
    ("con-coincident-hint", "sketch/17-constraints"),
    ("con-horizontal-hint", "sketch/17-constraints"),
    ("con-vertical-hint", "sketch/17-constraints"),
    ("con-parallel-hint", "sketch/17-constraints"),
    ("con-perpendicular-hint", "sketch/17-constraints"),
    ("con-equal", "sketch/17-constraints"),
    ("con-collinear-hint", "sketch/17-constraints"),
    ("con-concentric-hint", "sketch/17-constraints"),
    ("con-tangent-hint", "sketch/17-constraints"),
    ("con-symmetric-hint", "sketch/17-constraints"),
    ("con-midpoint-hint", "sketch/17-constraints"),
    ("con-fix", "sketch/17-constraints"),
    // the assembly buttons: grounding and connectors are in the mates article, the contents in the
    // components one
    ("tb-ground-hint", "assembly/02-joints"),
    // the mates without a joint are in the article about mates, the relation of two of them in its own
    ("j-group-tip", "assembly/02-joints"),
    ("j-width-tip", "assembly/02-joints"),
    ("j-tangent-tip", "assembly/02-joints"),
    ("j-relation-tip", "assembly/07-relations"),
    // a new sketch starts on the page of the sketch: where its plane is picked and what follows
    ("g-sketch-pick-hint", "sketch/index"),
    ("tb-new-part-hint", "assembly/01-components"),
    ("tb-new-subassembly-hint", "assembly/01-components"),
    ("tb-mirror-part-hint", "assembly/04-arrays"),
    ("tb-comp-lin-array-hint", "assembly/04-arrays"),
    ("tb-comp-circ-array-hint", "assembly/04-arrays"),
    // GROUNDING A PART: the buttons appeared to the guard only when the assembly became a crate of its
    // own and its source came into the list the guard reads. The article about joints explains what a
    // grounded part is and what to do when one will not move.
    ("tb-ground-pick", "assembly/02-joints"),
    ("tb-ground-off", "assembly/02-joints"),
    // a section of the view is part of the article about the viewport
    ("tb-section-hint", "general/09-viewport"),
    ("tb-section-hint-bar", "general/09-viewport"),
    ("tb-section-pick", "general/09-viewport"),
    ("tb-section-off", "general/09-viewport"),
];

/// THE TOOLTIPS THAT ARE NOT A TOOL BUTTON.
///
/// The list exists so that "not a tool" is SAID rather than assumed: otherwise the guard is walked
/// round by adding a button and saying nothing. The reason for each group is in the comment above
/// it.
pub const NOT_A_TOOL: &[&str] = &[
    // the headings of button groups and the captions of dropdowns are not tools
    "tb-group-create",
    "tb-group-sketch3d",
    "tb-group-prim",
    "tb-group-edit",
    "tb-group-dim",
    "tb-group-constraints",
    "tb-group-joint",
    "tb-type",
    "tb-body",
    // status lines and refusals of a command already begun: the help belongs to the command itself
    // rather than to its step
    "tb-bool-pick-b",
    "tb-pick-body-a-first",
    "tb-pick-part-first",
    "tb-mirror-pick-plane",
    "tb-mirror-pick-part",
];

/// The article of a bar button (one that does not launch a numbered command).
pub fn toolbar_article(hint: &str) -> Option<&'static str> {
    TOOLBAR.iter().find(|(h, _)| *h == hint).map(|(_, a)| *a)
}

/// The article of the SECTION for a workbench — where F1 leads when no command is active.
pub fn workbench_article(wb: &str) -> &'static str {
    match wb {
        "sketch" => "sketch/index",
        "part" => "part/index",
        "assembly" => "assembly/index",
        // THE MACHINING WORKBENCH EXISTS ONLY WITH THE MODULE SWITCHED ON, so the article is reached
        // only from there: with the module off it is not visible in the table of contents
        // (`help::visible`).
        "cam" => "cam/index",
        _ => "index",
    }
}

/// The article of the active Part command.
pub fn part_article(code: &str) -> Option<&'static str> {
    PART.iter().find(|r| r.code == code).map(|r| r.article)
}

/// The article of the active Sketch tool.
pub fn sketch_article(code: &str) -> Option<&'static str> {
    SKETCH.iter().find(|r| r.code == code).map(|r| r.article)
}

/// Every article promised by the table (without repeats).
pub fn promised() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = PART.iter().map(|r| r.article).chain(SKETCH.iter().map(|r| r.article)).chain(ASSEMBLY.iter().map(|(_, a)| *a)).chain(TOOLBAR.iter().map(|(_, a)| *a)).collect();
    v.sort();
    v.dedup();
    v
}
