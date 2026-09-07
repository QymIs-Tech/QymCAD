//! THE SINGLE EXPRESSION FIELD AND ITS LIST OF DRIVERS.
//!
//! The main law:
//!
//! > EDITING TEXT IS NOT EDITING THE MODEL.
//!
//! While a person types, the document does not change: the text lives in the buffer of the field and
//! travels into the model ONCE, on commit (Enter, Tab, losing focus). Escape throws the buffer away.
//!
//! That is how grown-up CAD is built, and not for reasons of taste. The name of a driver used to be
//! written into the model on every letter pressed — and that produced three troubles at once: a rebuild
//! of the whole project per letter, formulas destroyed halfway (`w` -> `wi` -> `wid`: the name `w` did
//! not exist for a second) and the erasure of what had been typed whenever the model refused to accept
//! an unfinished name.
//!
//! The list of drivers lives here as well, because the keyboard is shared: while the list is open, the
//! arrows and Enter belong to it and not to the field.
pub(crate) use qymcad_ui_state::{expr_field, name_field, LIST_OPEN};
use crate::gui::{App};



/// WAS A DRIVER LIST OPEN WHEN THE KEY WAS PRESSED — asked once, and the answer is taken away.
///
/// The frame's keys are handled BEFORE anything is drawn, so a field cannot answer for a frame that has not
/// been painted yet: the answer is about the frame the person was looking at when they pressed. It is taken
/// away rather than read, so that a field which has gone (the popup closed) cannot leave a stale "open"
/// behind and swallow somebody else's Escape.
pub(super) fn take_list_open(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| {
        let id = egui::Id::new(LIST_OPEN);
        let was = d.get_temp::<bool>(id).unwrap_or(false);
        d.insert_temp(id, false);
        was
    })
}

impl App {
}
