//! THE COMMANDS OF THE ASSEMBLY: their rows of the catalogue, in the order of the bar, and what a press of each does -
//! the same for its button, its row of "More" and the search. The datums of its bar are rows of the part.

use super::*;
use qymcad_ui_state::catalogue::{Command, Launch, Shelf};

const ASSEMBLY: &[&str] = &["assembly"];
const CREATE: &str = "tb-group-create";
const MATES: &str = "tb-group-joint";

/// THE COMMANDS OF AN ASSEMBLY, in the order of its bar: the components, then the mates.
pub const COMMANDS: &[Command] = &[
    // A NEW PART OR SUBASSEMBLY IS ONE STEP OF UNDO of its own name
    Command {
        code: "assembly.new-part",
        benches: ASSEMBLY,
        launch: Launch::NewPart,
        name_key: "cmdname-new-part",
        icon: IconId::AssemblyNewPart,
        hint_key: "tb-new-part-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "assembly.new-subassembly",
        benches: ASSEMBLY,
        launch: Launch::NewSubassembly,
        name_key: "cmdname-new-subassembly",
        icon: IconId::AssemblyNewSubassembly,
        hint_key: "tb-new-subassembly-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "assembly.insert",
        benches: ASSEMBLY,
        launch: Launch::InsertComponent,
        name_key: "cmdname-insert-component",
        icon: IconId::AssemblyInsertComponent,
        hint_key: "tb-insert-component-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    // COMPONENT PATTERNS ARE AN ASSEMBLY TOOL: in a part there are no components to repeat
    Command {
        code: "assembly.pattern-linear",
        benches: ASSEMBLY,
        launch: Launch::CompArray(1),
        name_key: "cmdname-comp-pattern-linear",
        icon: IconId::AssemblyArrayLinear,
        hint_key: "tb-comp-lin-array-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "assembly.pattern-circular",
        benches: ASSEMBLY,
        launch: Launch::CompArray(2),
        name_key: "cmdname-comp-pattern-circular",
        icon: IconId::AssemblyArrayCircular,
        hint_key: "tb-comp-circ-array-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "assembly.mirror",
        benches: ASSEMBLY,
        launch: Launch::MirrorPart,
        name_key: "cmdname-mirror-part",
        icon: IconId::AssemblyMirror,
        hint_key: "tb-mirror-part-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "assembly.section",
        benches: ASSEMBLY,
        launch: Launch::Section,
        name_key: "cmdname-section",
        icon: IconId::AssemblySection,
        hint_key: "tb-section-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    // ONE BUTTON FOR A JOINT, its kind chosen on its bar: seven buttons, one a kind, made the choice twice
    Command { code: "assembly.joint", benches: ASSEMBLY, launch: Launch::Joint, name_key: "", icon: IconId::AssemblyJoint, hint_key: "jt-joint-tip", group: MATES, shelf: Shelf::Bar },
    Command {
        code: "assembly.ground",
        benches: ASSEMBLY,
        launch: Launch::Ground,
        name_key: "cmdname-ground",
        icon: IconId::AssemblyGround,
        hint_key: "tb-ground-hint",
        group: MATES,
        shelf: Shelf::Bar,
    },
    Command { code: "assembly.group", benches: ASSEMBLY, launch: Launch::Group, name_key: "cmdname-group", icon: IconId::AssemblyGroup, hint_key: "j-group-tip", group: MATES, shelf: Shelf::Bar },
    Command { code: "assembly.width", benches: ASSEMBLY, launch: Launch::Width, name_key: "cmdname-width", icon: IconId::AssemblyWidth, hint_key: "j-width-tip", group: MATES, shelf: Shelf::Bar },
    Command {
        code: "assembly.tangent",
        benches: ASSEMBLY,
        launch: Launch::Tangent,
        name_key: "cmdname-tangent",
        icon: IconId::AssemblyTangent,
        hint_key: "j-tangent-tip",
        group: MATES,
        shelf: Shelf::Bar,
    },
    Command {
        code: "assembly.relation",
        benches: ASSEMBLY,
        launch: Launch::Relation,
        name_key: "cmdname-relation",
        icon: IconId::AssemblyRelation,
        hint_key: "j-relation-tip",
        group: MATES,
        shelf: Shelf::Bar,
    },
];

/// PRESS A COMMAND OF THE ASSEMBLY: what its button did, through the doors its button used.
pub fn press(launch: Launch, bc: &mut BarCtx) {
    match launch {
        Launch::NewPart => {
            begin_edit(bc.edits, bc.project, qymcad_i18n::tr("hotkey-assembly-n"));
            let id = bc.project.add_part(bc.project.free_part_name());
            close_edit(bc.edits, bc.project);
            bc.ask.push(BarAsk::EnterComponent(id));
        }
        Launch::NewSubassembly => {
            begin_edit(bc.edits, bc.project, qymcad_i18n::tr("hotkey-assembly-u"));
            let id = bc.project.add_assembly(qymcad_i18n::tr1("node-assembly-n", "n", &bc.project.components.len().to_string()));
            close_edit(bc.edits, bc.project);
            bc.ask.push(BarAsk::EnterComponent(id));
        }
        Launch::InsertComponent => bc.ask.push(BarAsk::Import(Want::Part)),
        Launch::CompArray(n) => bc.ask.push(take_or_drop(bc.carr.mode == n, BarAsk::CompArray(n))),
        Launch::MirrorPart => {
            // both a PART and a SUBASSEMBLY are accepted (the whole subtree is mirrored); with the tool in hand the
            // press goes to the door as it is, which puts it down
            let src = bc.mirror.part.or_else(|| match *bc.sel {
                Sel::Component(ci) => bc.project.components.get(ci).map(|c| c.id),
                _ => selected_body(&*bc.project, &*bc.sel).and_then(|b| bc.project.body_owner(b)),
            });
            // THROUGH A DOOR, in the order that works: release the others, THEN take this one. Pushing the release
            // and arming straight after put them the other way round - the release runs after the frame - and the
            // button did nothing. With nothing picked before it the tool waits for a click on a part.
            bc.ask.push(BarAsk::MirrorPart(src));
        }
        // ONE DOOR for the section: a copy of it here could not release the other tools
        Launch::Section => bc.ask.push(BarAsk::ToggleSection),
        Launch::Joint => bc.ask.push(take_or_drop(bc.joint.pick_faces, BarAsk::JointPick)),
        // a click on a part fixes or releases it
        Launch::Ground => bc.ask.push(BarAsk::GroundPick),
        // a set of parts fastened to one another without pairwise joints
        Launch::Group => bc.ask.push(BarAsk::GroupPick),
        // a part midway between two walls
        Launch::Width => bc.ask.push(BarAsk::WidthPick),
        // a cylinder laid onto a plane
        Launch::Tangent => bc.ask.push(BarAsk::TangentPick),
        // the degrees of freedom of two mates tied together
        Launch::Relation => bc.ask.push(BarAsk::RelationPick),
        // the commands of the sketch and of the part, the datums of this bar among them, are pressed by the part
        _ => {}
    }
}
