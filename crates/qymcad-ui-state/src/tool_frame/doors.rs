//! THE DOORS OF A TOOL: what Apply, Cancel and a step back do, given by the tool, gone through by the frame. Each tool
//! matched the answer of its frame by itself before - twelve times over the part and the assembly - and said a refusal
//! to apply where it chose: the status line, in the tone it picked. Now the frame goes through the door the answer
//! names, and a refusal to apply is said by the frame, where the tool speaks, in the tone of a refusal or of an error.

use super::*;

/// WHY A TOOL DID NOT APPLY, in words.
#[derive(Clone, Debug, PartialEq)]
pub enum NotApplied {
    /// What the tool holds will not do: a pick gone, a value the operation cannot take, a plane that cuts nothing.
    Refused(String),
    /// The operation was tried and failed.
    Failed(String),
}

impl NotApplied {
    /// The words, whatever their tone.
    pub fn words(&self) -> &str {
        match self {
            NotApplied::Refused(w) | NotApplied::Failed(w) => w,
        }
    }

    /// The words as they are said, in their tone.
    pub fn said(&self) -> Notice {
        match self {
            NotApplied::Refused(w) => Notice::refusal(w.clone()),
            NotApplied::Failed(w) => Notice::error(w.clone()),
        }
    }
}

/// WHAT A STEP BACK CAME TO.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stepped {
    /// The tool let go of something - a pick being made, a choice - and stays in hand.
    Back,
    /// The tool had nothing to let go of: the frame puts it down, as Cancel does.
    NothingLeft,
}

/// THE THREE DOORS OF A TOOL, over the narrow context it works in.
pub trait Doors {
    /// Apply what the tool holds; why not, in words.
    fn apply(&mut self) -> Result<(), NotApplied>;
    /// Put the tool down, what it made on the way taken away.
    fn cancel(&mut self);
    /// Esc with no field holding the keyboard: let go of one thing, or say there is nothing left to let go of - a tool
    /// that picks nothing along the way has nothing, and Esc puts it down.
    fn back(&mut self) -> Stepped {
        Stepped::NothingLeft
    }
    /// Enter was refused with `words`: the tool may say them where its workbench speaks besides the plate of the frame.
    fn refused(&mut self, _words: &Notice) {}
}

/// GO THROUGH THE DOOR THE ANSWER OF THE FRAME NAMES: Apply applies, and a refusal is said where the tool speaks, while
/// `gate` - the gate the tool stood at - stands; Cancel puts the tool down; Esc steps back, and with nothing left to let
/// go of puts the tool down.
pub fn go_through(ctx: &egui::Context, gate: &Gate, verdict: Verdict, doors: &mut dyn Doors) {
    match verdict {
        Verdict::Stay => {}
        // a refused Enter is said by the frame as it answers, and told to the tool
        Verdict::Refused(words) => doors.refused(&words),
        Verdict::Apply => {
            if let Err(why) = doors.apply() {
                let said = why.said();
                remember_refusal(ctx, gate, &said);
                doors.refused(&said);
            }
        }
        Verdict::Cancel => doors.cancel(),
        Verdict::Back => {
            if doors.back() == Stepped::NothingLeft {
                doors.cancel();
            }
        }
    }
}
