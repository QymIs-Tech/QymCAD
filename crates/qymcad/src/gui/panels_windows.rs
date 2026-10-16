//! THE WINDOWS AND DIALOGUES: the parts library, the properties of a component and of a datum, the
//! parameters, the machine, the settings, the tools, the modal confirmations.

use super::*;

impl App {}

mod dialogs;
pub(crate) use dialogs::*;
mod schemes;
pub(crate) use schemes::*;
mod settings;
pub(crate) use settings::*;
mod params;
pub(crate) use params::*;
mod faults;
pub(crate) use faults::*;
mod library;
pub(crate) use library::*;
