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
//! THE NUMBERS HERE ARE THE SAME AS IN THE CODE (`start_feat_cmd`, `set_sk_tool` and the rest): the
//! table is compared against the source of the panels, so "added a tool, forgot the article" reddens
//! the build.

/// The commands of the Part: the number of `start_feat_cmd`/`start_prim_cmd` -> the article.
pub const PART: &[(u8, &str)] = &[
    (1, "part/01-extrude"),
    (3, "part/02-revolve"),
    (8, "part/03-sweep"),
    (9, "part/04-loft"),
    (4, "part/05-fillet"),
    (5, "part/06-chamfer"),
    (6, "part/07-shell"),
    (7, "part/08-hole"),
    (23, "part/09-draft"),
    (24, "part/10-thread"),
    (25, "part/11-push-face"),
    (26, "part/12-remove-face"),
    (27, "part/13-split-body"),
    (29, "part/14-split-face"),
    (28, "part/15-thicken"),
    (30, "part/20-face-copy"),
    (31, "part/21-surface-replace"),
    (32, "part/22-patch"),
    (33, "part/23-stitch"),
    (34, "part/24-trim"),
    (16, "part/16-mirror"),
    (17, "part/17-linear-array"),
    (18, "part/18-circular-array"),
    // DATUMS are commands too, and are shared by the Part and the Assembly: their buttons live in the
    // common creation panel rather than in the workbench panel. One article for three: a plane, an axis
    // and a point answer one question — what to build from — and differ only in what they set.
    (20, "general/04-datums"),
    (21, "general/04-datums"),
    (22, "general/04-datums"),
    // six primitives, one article: they differ in their fields rather than in their meaning
    (10, "part/19-primitives"),
    (11, "part/19-primitives"),
    (12, "part/19-primitives"),
    (13, "part/19-primitives"),
    (14, "part/19-primitives"),
    (15, "part/19-primitives"),
];

/// The tools of the Sketch: (which handle, its number) -> the article.
///
/// The handle is part of the key because the numbers are THEIR OWN: `set_sk_tool(1)` is a line while
/// `set_click_op(1)` is a trim. Glue them into one number and the table starts lying silently.
pub const SKETCH: &[(&str, u8, &str)] = &[
    ("sk", 1, "sketch/01-line"),
    ("sk", 2, "sketch/02-rect"),
    ("sk", 3, "sketch/03-circle"),
    // a circle by three points is the same tool set a different way: one article for both
    ("sk", 10, "sketch/03-circle"),
    ("sk", 4, "sketch/04-arc"),
    ("sk", 5, "sketch/05-point"),
    ("sk", 6, "sketch/06-polygon"),
    ("sk", 7, "sketch/07-slot"),
    ("sk", 8, "sketch/08-ellipse"),
    ("sk", 9, "sketch/09-spline"),
    ("sk", 11, "sketch/10-text"),
    ("dim", 1, "sketch/11-dimensions"),
    ("dim", 2, "sketch/11-dimensions"),
    ("dim", 3, "sketch/11-dimensions"),
    ("click", 1, "sketch/12-trim"),
    ("click", 2, "sketch/13-extend"),
    ("click", 3, "sketch/14-break"),
    ("click", 4, "sketch/15-project"),
    ("click", 5, "sketch/16-corner"),
    ("click", 6, "sketch/17-project-body"),
    ("mod", 0, "sketch/18-delete"),
    ("mod", 1, "sketch/19-mirror"),
    ("mod", 6, "sketch/20-offset"),
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
    ("tb-construction-hint", "sketch/index"),
    ("tb-select-hint", "sketch/index"),
    // the assembly buttons: grounding and connectors are in the mates article, the contents in the
    // components one
    ("tb-ground-hint", "assembly/02-joints"),
    ("tb-new-part-hint", "assembly/01-components"),
    ("tb-new-subassembly-hint", "assembly/01-components"),
    ("tb-mirror-part-hint", "assembly/04-arrays"),
    ("tb-comp-lin-array-hint", "assembly/04-arrays"),
    ("tb-comp-circ-array-hint", "assembly/04-arrays"),
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
#[cfg(test)]
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
    // MACHINING IS DELIBERATELY NOT DESCRIBED: the module is being rewritten, and `cam/index` says so
    // out loud. Writing help for what is about to change is lying with a delay.
    "tb-gcode-generate",
    "tb-gcode-export",
    "tb-simulation",
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
pub fn part_article(kind: u8) -> Option<&'static str> {
    PART.iter().find(|(k, _)| *k == kind).map(|(_, a)| *a)
}

/// The article of the active Sketch tool.
pub fn sketch_article(handle: &str, n: u8) -> Option<&'static str> {
    SKETCH.iter().find(|(h, k, _)| *h == handle && *k == n).map(|(_, _, a)| *a)
}

/// Every article promised by the table (without repeats).
#[cfg(test)]
pub fn promised() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = PART
        .iter()
        .map(|(_, a)| *a)
        .chain(SKETCH.iter().map(|(_, _, a)| *a))
        .chain(ASSEMBLY.iter().map(|(_, a)| *a))
        .chain(TOOLBAR.iter().map(|(_, a)| *a))
        .collect();
    v.sort();
    v.dedup();
    v
}
