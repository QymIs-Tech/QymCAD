//! THE PARTS WITH THE SAME SET OF TOOLS IN A DIFFERENT ORDER: whichever part is entered, all of the operations are
//! there while the sequence differs. That is how order dependencies are caught - the ones a single scenario never
//! shows.
use qymcad_ui_state::PartTool;

/// ONE PART OF THE SCENARIO: its name and the tools applied to it, in this order.
pub(super) struct Order {
    pub(super) name: &'static str,
    pub(super) tools: [PartTool; 17],
}

/// THE ORDERS. The trim comes AFTER the split: only then does the part hold something to cut with. Removing a face
/// right after a chamfer: a fresh chamfer can be taken off, a wall of a finished part cannot. The thread comes AFTER
/// the shell: its grooves are finer than the wall, and a shell does not build over them. The mirror comes BEFORE the
/// shell: the core cannot mirror a hollow part. The body split goes last: after it a part legitimately holds several
/// bodies. The draft comes BEFORE the shell: after it the wall is 2 mm and there is nothing to tilt.
pub(super) const ORDERS: [Order; 5] = [
    Order {
        name: "Order A",
        tools: [
            PartTool::Hole,
            PartTool::Fillet,
            PartTool::Mirror,
            PartTool::Chamfer,
            PartTool::RemoveFace,
            PartTool::SplitFace,
            PartTool::CircularPattern,
            PartTool::Shell,
            PartTool::PushFace,
            PartTool::Draft,
            PartTool::FaceCopy,
            PartTool::Thicken,
            PartTool::Stitch,
            PartTool::Patch,
            PartTool::Thread,
            PartTool::SplitBody,
            PartTool::Trim,
        ],
    },
    Order {
        name: "Order B",
        tools: [
            PartTool::Draft,
            PartTool::Patch,
            PartTool::SplitFace,
            PartTool::Stitch,
            PartTool::FaceCopy,
            PartTool::Thicken,
            PartTool::Mirror,
            PartTool::PushFace,
            PartTool::Chamfer,
            PartTool::RemoveFace,
            PartTool::Fillet,
            PartTool::Hole,
            PartTool::CircularPattern,
            PartTool::Shell,
            PartTool::Thread,
            PartTool::SplitBody,
            PartTool::Trim,
        ],
    },
    Order {
        name: "Order C",
        tools: [
            PartTool::Stitch,
            PartTool::Mirror,
            PartTool::FaceCopy,
            PartTool::Patch,
            PartTool::Thicken,
            PartTool::Chamfer,
            PartTool::RemoveFace,
            PartTool::SplitFace,
            PartTool::Draft,
            PartTool::Hole,
            PartTool::CircularPattern,
            PartTool::Shell,
            PartTool::Fillet,
            PartTool::PushFace,
            PartTool::Thread,
            PartTool::SplitBody,
            PartTool::Trim,
        ],
    },
    Order {
        name: "Order D",
        tools: [
            PartTool::Fillet,
            PartTool::Hole,
            PartTool::PushFace,
            PartTool::Mirror,
            PartTool::SplitFace,
            PartTool::RemoveFace,
            PartTool::Chamfer,
            PartTool::CircularPattern,
            PartTool::FaceCopy,
            PartTool::Stitch,
            PartTool::Patch,
            PartTool::Thicken,
            PartTool::Shell,
            PartTool::Draft,
            PartTool::Thread,
            PartTool::SplitBody,
            PartTool::Trim,
        ],
    },
    Order {
        name: "Order E",
        tools: [
            PartTool::SplitFace,
            PartTool::Chamfer,
            PartTool::FaceCopy,
            PartTool::Patch,
            PartTool::Mirror,
            PartTool::Fillet,
            PartTool::RemoveFace,
            PartTool::Hole,
            PartTool::PushFace,
            PartTool::Stitch,
            PartTool::Thicken,
            PartTool::CircularPattern,
            PartTool::Draft,
            PartTool::Shell,
            PartTool::Thread,
            PartTool::SplitBody,
            PartTool::Trim,
        ],
    },
];
