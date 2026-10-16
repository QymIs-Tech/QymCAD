//! THE STEPS OF THE CONTRACT, the same for every tool: each point takes a description and checks it through the
//! window. A point that does not hold fails with what a person would have seen.
use std::time::{Duration, Instant};

use qymcad::{pos2, Document, Key, Modifiers, Pos2, Session, Solid};

use super::{Class, Context, Entry, Field, Finish, Flow, Node, Outcome, Pick, Tool, Under, Upstream, When, POINTS};
use crate::build;

/// RUN POINT `n` OF THE CONTRACT OF `tool`. A point the description marks as not applying passes when it says why.
pub fn point(tool: &Tool, n: u8) {
    if let Some((_, why)) = tool.not_applicable.iter().find(|(p, _)| *p == n) {
        assert!(!why.trim().is_empty(), "{}: point {n} is marked as not applying without a reason", tool.id);
        return;
    }
    match n {
        1 => entry(tool),
        2 => bar(tool),
        3 => picks(tool),
        4 => wrong_picks(tool),
        5 => valid_values(tool),
        6 => invalid_values(tool),
        7 => preview(tool),
        8 => apply(tool),
        9 => cancel(tool),
        10 => result(tool),
        11 => undo(tool),
        12 => save_open(tool),
        13 => reopen(tool),
        14 => upstream(tool),
        15 => dependency(tool),
        16 => contexts(tool),
        17 => kernel_refusal(tool),
        18 => budget(tool),
        19 => help(tool),
        _ => panic!("the contract has no point {n}; it has {:?}", POINTS),
    }
}

/// The top left corner of the window, near which the bars stand.
const TOP: Pos2 = pos2(0.0, 0.0);

mod steps;
pub(crate) use steps::*;
mod outcome;
pub(crate) use outcome::*;
mod taking;
pub(crate) use taking::*;
mod refusing;
pub(crate) use refusing::*;
mod showing;
pub(crate) use showing::*;
mod keeping;
pub(crate) use keeping::*;
mod bounds;
pub(crate) use bounds::*;
