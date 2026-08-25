//! ONE LIST OF THE ASSEMBLY TOOLS — AND EVERYBODY READS IT.
//!
//! WHY THIS EXISTS. In one sitting three troubles in a row turned out to be ONE illness: somewhere in
//! the code the modes are listed by hand, and the new tool was forgotten there.
//!
//! * the highlight stayed silent while pointing at a secondary axis — the mode was not named in the
//!   drawing condition;
//! * the highlight stayed silent for FOUR more (connector, group, width, tangent) — the same list;
//! * Esc did not release FIVE tools — a list of its own in `on_escape` that knew two out of nine.
//!
//! Fixing each list separately is pointless: the next tool will be forgotten in the next one. So
//! there is ONE list, and the compiler watches over its completeness: the `match` on `AssemblyTool`
//! is exhaustive, and a new kind simply will not build until it has been said whether it is taken up
//! and how it is put down.
use super::App;

/// THE TOOLS OF THE ASSEMBLY WORKBENCH — everything that can be "taken up".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AssemblyTool {
    /// Assembling a mate: two anchors pointed at.
    Mate,
    /// A standalone connector: one pointing, no joint created.
    Anchor,
    /// A group: clicks on the parts, then Enter.
    Group,
    /// A width: two walls and a tab between them.
    Width,
    /// A tangent: two surfaces, no connectors.
    Tangent,
    /// A relation between degrees: clicks on the JOINTS rather than on geometry.
    Relation,
    /// Ground: a click on a part fixes it.
    Ground,
    /// Pointing at the secondary axis of a connector (the second pick).
    Axis,
    /// Re-picking an anchor of a finished joint.
    Repick,
}

impl AssemblyTool {
    /// EVERY KIND. The single list the checks walk over.
    pub(crate) const ALL: [AssemblyTool; 9] = [
        AssemblyTool::Mate,
        AssemblyTool::Anchor,
        AssemblyTool::Group,
        AssemblyTool::Width,
        AssemblyTool::Tangent,
        AssemblyTool::Relation,
        AssemblyTool::Ground,
        AssemblyTool::Axis,
        AssemblyTool::Repick,
    ];

    /// DOES IT ASK FOR GEOMETRY TO BE POINTED AT IN THE FRAME.
    ///
    /// A relation is the only one that does not: the degrees are pointed at by clicking joints in the
    /// list, and they cannot be collected in the viewport.
    pub(crate) fn wants_geometry(self) -> bool {
        !matches!(self, AssemblyTool::Relation)
    }

    /// THE MODE FOR THE HELP: F1 finds the article by it (`help_map::ASSEMBLY`).
    ///
    /// With a DOT rather than a hyphen: the keys of the language catalogue are written with hyphens,
    /// and the guard against an internal name reaching the screen catches any literal of that shape.
    pub(crate) fn help_mode(self) -> &'static str {
        match self {
            AssemblyTool::Mate => "asm.joint",
            AssemblyTool::Anchor => "asm.anchor",
            AssemblyTool::Group => "asm.group",
            AssemblyTool::Width => "asm.width",
            AssemblyTool::Tangent => "asm.tangent",
            AssemblyTool::Relation => "asm.relation",
            AssemblyTool::Ground => "asm.ground",
            AssemblyTool::Axis => "asm.axis",
            AssemblyTool::Repick => "asm.repick",
        }
    }
}

impl App {
    /// IS THIS PARTICULAR TOOL TAKEN UP. One answer for everyone who asks.
    pub(crate) fn assembly_tool_armed(&self, t: AssemblyTool) -> bool {
        match t {
            AssemblyTool::Mate => self.joint.pick_faces,
            AssemblyTool::Anchor => self.joint.conn_pick,
            AssemblyTool::Group => self.joint.group_pick.is_some(),
            AssemblyTool::Width => self.joint.width_pick.is_some(),
            AssemblyTool::Tangent => self.joint.tangent_pick.is_some(),
            AssemblyTool::Relation => self.joint.relation_pick.is_some(),
            AssemblyTool::Ground => self.joint.ground_pick,
            AssemblyTool::Axis => self.joint.axis_pick.is_some(),
            AssemblyTool::Repick => self.joint.edit_repick.is_some(),
        }
    }

    /// WHICH ASSEMBLY TOOLS ARE TAKEN UP RIGHT NOW.
    pub(crate) fn armed_assembly_tools(&self) -> Vec<AssemblyTool> {
        AssemblyTool::ALL.into_iter().filter(|&t| self.assembly_tool_armed(t)).collect()
    }

    /// IS THE WORKBENCH WAITING FOR GEOMETRY — the question the highlight and the click pass ask.
    pub(crate) fn assembly_wants_geometry(&self) -> bool {
        self.armed_assembly_tools().into_iter().any(AssemblyTool::wants_geometry)
    }

    /// RELEASE EVERY ASSEMBLY TOOL. Esc looks here too.
    ///
    /// An unfinished selection goes with the tool: half of what was pointed at is not a document but
    /// an intention, and it must not survive a cancellation.
    pub(crate) fn drop_assembly_tools(&mut self) {
        for t in AssemblyTool::ALL {
            match t {
                AssemblyTool::Mate => {
                    self.joint.pick_faces = false;
                    self.joint.pick_first = None;
                }
                AssemblyTool::Anchor => self.joint.conn_pick = false,
                AssemblyTool::Group => self.joint.group_pick = None,
                AssemblyTool::Width => self.joint.width_pick = None,
                AssemblyTool::Tangent => self.joint.tangent_pick = None,
                AssemblyTool::Relation => self.joint.relation_pick = None,
                AssemblyTool::Ground => self.joint.ground_pick = false,
                AssemblyTool::Axis => self.joint.axis_pick = None,
                AssemblyTool::Repick => self.joint.edit_repick = None,
            }
        }
    }
}
