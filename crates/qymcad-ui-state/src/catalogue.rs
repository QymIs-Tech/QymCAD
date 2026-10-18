//! THE COMMAND CATALOGUE - one row a command, and every place that shows a command reads it: the bar of a workbench,
//! its "More" list, the search, the help, the keys and the settings of the bar.
//!
//! WHY THE ROWS LIVE HERE. The bars of the workbenches and the window both read them, and two lists of one set of
//! commands drift at the first new command: the bar used to write its buttons one after another in the code while the
//! search read a catalogue of its own, and a guard had to hold the two together by reading the source. Now a button IS
//! a row: what it shows (icon, hint, group) and what it starts (`Launch`) are said once. Each workbench gives its own
//! rows; the type of a row is shared, so the window knows no workbench by name to draw a bar.

use crate::IconId;

/// WHAT A COMMAND STARTS. Exactly what its button does; the search starts a command through the same door.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Launch {
    /// A command of the feature timeline.
    Feat(u8),
    /// A primitive.
    Prim(u8),
    /// A drawing tool of a sketch.
    SkTool(u8),
    /// A dimension of a sketch.
    Dim(u8),
    /// A tool of a sketch acting on a click: trim, extend, break, the corner and its chamfer, the projection.
    ClickOp(u8),
    /// Editing the selection of a sketch: delete, mirror, offset.
    Modify(crate::EditTool),
    /// The arrow of a sketch: every tool put down.
    SketchSelect,
    /// Construction geometry: what is selected is turned, or what is drawn next.
    Construction,
    /// A pattern of the selection of a sketch, linear or circular.
    SkPattern(u8),
    /// Move, copy or rotate the selection of a sketch.
    SkMove(u8),
    /// Round every corner of a contour.
    FilletAll,
    /// Measure on the sheet of a sketch.
    Measure2d,
    /// A constraint of a sketch, by its code.
    Constraint(u8),
    /// A new sketch: the pick of its plane.
    SketchPick,
    /// The section of the view.
    Section,
    /// Measure in 3D.
    Measure3d,
    /// A boolean of two bodies.
    Boolean,
    /// A new part, or a new subassembly, in the assembly open.
    NewPart,
    NewSubassembly,
    /// A component brought in from a file.
    InsertComponent,
    /// A pattern of components, linear or circular.
    CompArray(u8),
    /// A mirrored copy of a part or a subassembly.
    MirrorPart,
    /// The mates: a joint, grounding, a group, a width, a tangent, a relation.
    Joint,
    Ground,
    Group,
    Width,
    Tangent,
    Relation,
    /// One of the several bodies a part shows, made a part of its own. Reached from the search and the body's
    /// menu; it has no button.
    PieceToPart,
}

/// WHERE A COMMAND IS OFFERED besides the search.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shelf {
    /// A button of the bar of its workbench, or a row of its "More" list.
    Bar,
    /// The search and the menu of what it acts on, no button.
    SearchOnly,
}

/// ONE COMMAND.
#[derive(Debug)]
pub struct Command {
    /// A stable code, `area.name`, as the keys have. With a dot rather than a hyphen: the guard of the words catches
    /// any literal shaped like a key of the catalogue, and the code is never shown.
    pub code: &'static str,
    /// The workbenches whose bar shows it, the one it belongs to first.
    pub benches: &'static [&'static str],
    pub launch: Launch,
    /// A name of its own, a key of the words; empty when the name is the title of its help article.
    pub name_key: &'static str,
    pub icon: IconId,
    /// The key of the hint of its button.
    pub hint_key: &'static str,
    /// The key of the name of its group on the bar; neighbours of one group stand together, a line between groups.
    pub group: &'static str,
    pub shelf: Shelf,
}

impl Command {
    /// The workbench the command belongs to.
    pub fn home(&self) -> &'static str {
        self.benches.first().copied().unwrap_or("")
    }

    /// Is the command on the bar of `bench`?
    pub fn on(&self, bench: &str) -> bool {
        self.shelf == Shelf::Bar && self.benches.contains(&bench)
    }
}

/// IS THE COMMAND IN HAND - its button lit. Read off the state its press sets, the same for every workbench.
///
/// A command of the timeline is in hand also while it waits for the sketch it needs. Reported behaviour: "press the
/// tool and it stays highlighted, with the status asking for a sketch" - it did not: the wait lives in the picking,
/// and the button lit from the command alone, so a tool that WAS waiting looked untouched.
pub fn pressed(launch: Launch, bc: &crate::BarCtx) -> bool {
    match launch {
        Launch::SketchSelect => crate::in_select_mode(bc.armed),
        Launch::SkTool(n) => bc.armed.draw_kind() == n,
        Launch::Construction => bc.tool.construction,
        Launch::Modify(tool) => bc.sel_sk.modify == Some(tool),
        Launch::SkPattern(n) => bc.armed.pat_op() == n,
        Launch::ClickOp(n) => bc.armed.click_op() == n,
        Launch::FilletAll => bc.picking.fillet_all() || bc.corner.at.is_some_and(|(_, pid, _)| pid == 0),
        Launch::SkMove(n) => bc.armed.move_op() == n,
        Launch::Dim(n) => bc.armed.dim_kind() == n,
        Launch::Measure2d => bc.armed.measuring(),
        Launch::Constraint(n) => bc.sel_sk.constraint == Some(n),
        Launch::SketchPick => bc.picking.is_sketch_plane(),
        Launch::Feat(n) => bc.armed.cmd_kind() == n || bc.picking.sketch_for() == Some(n),
        Launch::Prim(n) => bc.armed.cmd_kind() == n,
        Launch::Section => bc.section.pick || bc.section.plane.is_some(),
        Launch::Measure3d => bc.m3.on,
        Launch::Boolean => bc.boolean.pick.is_some(),
        Launch::CompArray(n) => bc.carr.mode == n,
        Launch::MirrorPart => bc.mirror.in_hand(),
        Launch::Joint => bc.joint.pick_faces,
        Launch::Ground => bc.joint.ground_pick,
        Launch::Group => bc.joint.group_pick.is_some(),
        Launch::Width => bc.joint.width_pick.is_some(),
        Launch::Tangent => bc.joint.tangent_pick.is_some(),
        Launch::Relation => bc.joint.relation_pick.is_some(),
        Launch::NewPart | Launch::NewSubassembly | Launch::InsertComponent | Launch::PieceToPart => false,
    }
}

/// THE HINT OF A COMMAND'S BUTTON. The joint names the kind it will make, which is chosen on its bar: one button
/// instead of one a kind.
pub fn hint(cmd: &Command, bc: &crate::BarCtx) -> String {
    match cmd.launch {
        Launch::Joint => qymcad_i18n::tr1(cmd.hint_key, "kind", &qymcad_i18n::tr(bc.joint.new_kind.label())),
        _ => qymcad_i18n::tr(cmd.hint_key),
    }
}

/// THE BUTTONS OF A BAR AS THE PERSON SET THEM: on the bar, and in its "More" list.
pub struct Arranged {
    pub on_bar: Vec<&'static Command>,
    pub in_more: Vec<&'static Command>,
}

/// ARRANGE THE ROWS OF A BAR by the person's set: the rows they ordered in their order, a row they never placed after
/// the one the catalogue puts before it, and the ones they put into "More" in its list. No set: the catalogue's order,
/// everything on the bar.
pub fn arrange(rows: &[&'static Command], set: Option<&crate::BarSet>) -> Arranged {
    let Some(set) = set else { return Arranged { on_bar: rows.to_vec(), in_more: Vec::new() } };
    let all = ordered(rows, Some(set));
    let put_away = |c: &&'static Command| set.in_more.iter().any(|m| m == c.code);
    Arranged { on_bar: all.iter().filter(|c| !put_away(c)).copied().collect(), in_more: all.iter().filter(|c| put_away(c)).copied().collect() }
}

/// THE ROWS OF A BAR IN THE PERSON'S ORDER, the bar and "More" together: the rows they ordered in their order, a row
/// they never placed after the one the catalogue puts before it.
pub fn ordered(rows: &[&'static Command], set: Option<&crate::BarSet>) -> Vec<&'static Command> {
    let Some(set) = set else { return rows.to_vec() };
    let mut out: Vec<&'static Command> = Vec::new();
    for code in &set.order {
        if let Some(c) = rows.iter().find(|c| c.code == code.as_str()) {
            if !out.iter().any(|o| o.code == c.code) {
                out.push(c);
            }
        }
    }
    for (i, c) in rows.iter().enumerate() {
        if out.iter().any(|o| o.code == c.code) {
            continue;
        }
        let after = rows[..i].iter().rev().find_map(|p| out.iter().position(|o| o.code == p.code));
        out.insert(after.map_or(0, |a| a + 1), c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BENCH: &[&str] = &["bench"];

    /// Four rows of one bench, in the catalogue's order a, b, c, d.
    static ROWS: [Command; 4] = [
        Command { code: "x.a", benches: BENCH, launch: Launch::SkTool(1), name_key: "", icon: IconId::SketchLine, hint_key: "", group: "g", shelf: Shelf::Bar },
        Command { code: "x.b", benches: BENCH, launch: Launch::SkTool(2), name_key: "", icon: IconId::SketchRect, hint_key: "", group: "g", shelf: Shelf::Bar },
        Command { code: "x.c", benches: BENCH, launch: Launch::SkTool(3), name_key: "", icon: IconId::SketchCircle, hint_key: "", group: "g", shelf: Shelf::Bar },
        Command { code: "x.d", benches: BENCH, launch: Launch::SkTool(4), name_key: "", icon: IconId::SketchArc, hint_key: "", group: "g", shelf: Shelf::Bar },
    ];

    fn rows() -> Vec<&'static Command> {
        ROWS.iter().collect()
    }

    fn codes(v: &[&'static Command]) -> Vec<&'static str> {
        v.iter().map(|c| c.code).collect()
    }

    /// NO SET: the catalogue's order, everything on the bar.
    #[test]
    fn without_a_set_the_bar_is_the_catalogue() {
        let a = arrange(&rows(), None);
        assert_eq!(codes(&a.on_bar), ["x.a", "x.b", "x.c", "x.d"]);
        assert!(a.in_more.is_empty());
    }

    /// THE PERSON'S ORDER FIRST, a row never placed after the one the catalogue puts before it, and the rows put away in
    /// "More" off the bar - in the same order.
    #[test]
    fn a_set_orders_the_bar_and_puts_rows_into_more() {
        let set = crate::BarSet { order: vec!["x.c".into(), "x.a".into()], in_more: vec!["x.b".into()] };
        assert_eq!(codes(&ordered(&rows(), Some(&set))), ["x.c", "x.d", "x.a", "x.b"], "d follows c, its neighbour in the catalogue; b follows a");
        let a = arrange(&rows(), Some(&set));
        assert_eq!(codes(&a.on_bar), ["x.c", "x.d", "x.a"]);
        assert_eq!(codes(&a.in_more), ["x.b"]);
    }

    /// A CODE THE CATALOGUE NO LONGER HAS is passed over: a command removed since the set was kept leaves no hole.
    #[test]
    fn a_code_that_is_gone_is_passed_over() {
        let set = crate::BarSet { order: vec!["x.gone".into(), "x.b".into(), "x.b".into()], in_more: vec!["x.gone".into()] };
        assert_eq!(codes(&ordered(&rows(), Some(&set))), ["x.a", "x.b", "x.c", "x.d"], "b once, as kept; a has nothing before it in the catalogue and stands first");
    }
}
