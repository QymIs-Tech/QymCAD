//! THE MENUS OF THE PROGRAM: the items that show a window, described as actions.
use crate::contract::fixtures::Fixture;
use crate::contract::{Entry, Flow, Outcome, Tool};

/// The block as the extrusion makes it: 40 x 30 x 10 from the origin.
const BLOCK: Outcome = Outcome::Body { volume: 12000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] };

/// What an item that opens a window leaves out: it is an action on nothing, and the window is not the document.
const SHOWS: &[(u8, &str)] = &[
    (2, "a menu item has no bar of options"),
    (3, "a menu item takes nothing"),
    (4, "a menu item takes nothing, so there is no wrong pick to refuse"),
    (5, "a menu item has no field"),
    (6, "a menu item has no field to refuse a value in"),
    (7, "the window opens at once: there is nothing to show before it"),
    (9, "the window opens at once; closing it is the window's own contract"),
    (12, "a window of the program is not part of the document: saving and opening does not carry it"),
    (13, "a window lays no node in a timeline"),
    (14, "nothing of a timeline stands above a window"),
    (15, "a window stands on nothing of the document"),
    (16, "a window of the program opens the same from any part: there is no second place that differs"),
    (17, "a window always opens: there is nothing the kernel could refuse"),
    (19, "a menu item is not held, so F1 has no tool in hand to open an article of"),
];

/// An item of the menus that opens the window of this title, and lays no step of undo.
macro_rules! shows {
    ($name:ident, $id:literal, $path:expr, $title:literal, $help:literal) => {
        pub static $name: Tool = Tool {
            id: $id,
            flow: Flow::Action,
            title: $title,
            entries: &[Entry::Menu($path)],
            other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
            fixture: Fixture::FirstPart,
            picks: &[],
            pick_trial: &[],
            wrong_picks: &[],
            words: &[],
            fields: &[],
            modes: &[],
            result: Outcome::Window { title: $title },
            node: "",
            undo: $title,
            undo_steps: 0,
            stays: false,
            upstream: None,
            dependency: None,
            contexts: &[],
            refusal: None,
            budget: (10, 2000),
            help: $help,
            not_applicable: SHOWS,
        };
    };
}

shows!(SETTINGS, "menu.settings", &["menu-windows", "menu-settings"], "win-settings", "general/07-settings");
// The table of keys is a section of the settings window, so the keys are reached by the same item.
shows!(KEYBOARD, "settings.keyboard", &["menu-windows", "menu-settings"], "win-settings", "general/10-hotkeys");
shows!(PARTS_LIBRARY, "menu.parts-library", &["menu-windows", "menu-parts-library"], "pl-title", "general/12-library");
shows!(START_SCREEN, "menu.start-screen", &["menu-windows", "win-start"], "start-title", "general/01-window");
shows!(REPORT, "menu.report", &["menu-help", "help-report"], "report-title", "general/13-report");

/// What the items of history leave out: they act on the document as it stands, and take nothing.
const HISTORY: &[(u8, &str)] = &[
    (2, "a menu item has no bar of options"),
    (3, "a menu item takes nothing"),
    (4, "a menu item takes nothing, so there is no wrong pick to refuse"),
    (5, "a menu item has no field"),
    (6, "a menu item has no field to refuse a value in"),
    (7, "the item acts at once: there is nothing to show before it"),
    (9, "the item acts at once: there is nothing to cancel"),
    (11, "stepping through the history is what undo and redo are; a step of their own they do not lay"),
    (13, "an item of the menu lays no node of its own"),
    (14, "nothing of a timeline stands above a step of history"),
    (15, "a step of history stands on nothing of the document"),
    (16, "the history is the document's own: there is no second place that differs"),
    (17, "going back and forth in the history always builds what was built"),
    (19, "a menu item is not held, so F1 has no tool in hand to open an article of"),
];

/// UNDO: Edit -> Undo on the block - the extrusion is taken back, and no body is left.
pub static UNDO: Tool = Tool {
    id: "menu.undo",
    flow: Flow::Action,
    title: "menu-undo",
    entries: &[Entry::Menu(&["menu-edit", "menu-undo"])],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Bodies { count: 0, volume: 0.0, largest: 0.0 },
    node: "",
    undo: "menu-undo",
    undo_steps: 0,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[],
    refusal: None,
    budget: (10, 2000),
    help: "general/03-timeline",
    not_applicable: HISTORY,
};

/// REDO: Edit -> Redo after the extrusion was undone - the block comes back as it was.
pub static REDO: Tool = Tool {
    id: "menu.redo",
    flow: Flow::Action,
    title: "menu-redo",
    entries: &[Entry::Menu(&["menu-edit", "menu-redo"])],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::BlockUndone,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: BLOCK,
    node: "",
    undo: "menu-redo",
    undo_steps: 0,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[],
    refusal: None,
    budget: (10, 2000),
    help: "general/03-timeline",
    not_applicable: HISTORY,
};

/// REBUILD: Edit -> Rebuild everything on the block - the timeline recomputed, the block the same as before.
pub static REBUILD: Tool = Tool {
    id: "menu.rebuild",
    flow: Flow::Action,
    title: "menu-rebuild",
    entries: &[Entry::Menu(&["menu-edit", "menu-rebuild"])],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: BLOCK,
    node: "",
    undo: "menu-rebuild",
    undo_steps: 0,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[],
    refusal: None,
    budget: (30, 2000),
    help: "general/03-timeline",
    not_applicable: HISTORY,
};

/// NEW_PROJECT: File -> New project on the unsaved block - first the question about the changes not saved, as the
/// professional systems ask it; the answer is the dialog's own matter.
pub static NEW_PROJECT: Tool = Tool {
    id: "menu.new-project",
    flow: Flow::Action,
    title: "file-new",
    entries: &[Entry::Menu(&["menu-file", "file-new"])],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Window { title: "nav-unsaved-title" },
    node: "",
    undo: "file-new",
    undo_steps: 0,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[],
    refusal: None,
    budget: (10, 2000),
    help: "general/08-documents",
    not_applicable: HISTORY,
};

/// An item of the menus that puts up the chooser of files - to write one, or to pick one to read - on the block.
macro_rules! chooses {
    ($name:ident, $id:literal, $path:expr, $save:literal, $help:literal) => {
        pub static $name: Tool = Tool {
            id: $id,
            flow: Flow::Action,
            title: "menu-file",
            entries: &[Entry::Menu($path)],
            other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
            fixture: Fixture::Block,
            picks: &[],
            pick_trial: &[],
            wrong_picks: &[],
            words: &[],
            fields: &[],
            modes: &[],
            result: Outcome::Chooser { save: $save },
            node: "",
            undo: "menu-file",
            undo_steps: 0,
            stays: false,
            upstream: None,
            dependency: None,
            contexts: &[],
            refusal: None,
            budget: (10, 2000),
            help: $help,
            not_applicable: CHOOSES,
        };
    };
}

/// What an item that puts up a chooser leaves out, besides what every menu item does: the file it writes or reads is
/// the matter of the formats' own round trips.
const CHOOSES: &[(u8, &str)] = &[
    (2, "a menu item has no bar of options"),
    (3, "a menu item takes nothing"),
    (4, "a menu item takes nothing, so there is no wrong pick to refuse"),
    (5, "a menu item has no field; the name of the file is typed in the chooser"),
    (6, "a menu item has no field to refuse a value in"),
    (7, "the chooser comes up at once: there is nothing to show before it"),
    (9, "closing the chooser without a file is the chooser's own matter"),
    (11, "putting up a chooser lays no step of undo"),
    (12, "the chooser waits for an answer; the file it writes or reads is the matter of the formats' round trips"),
    (13, "a menu item lays no node"),
    (14, "nothing of a timeline stands above a chooser"),
    (15, "a chooser stands on nothing of the document"),
    (16, "the chooser comes up the same from any part"),
    (17, "a chooser always comes up"),
    (19, "a menu item is not held, so F1 has no tool in hand to open an article of"),
];

/// An export to a mesh: first the window that asks its quality ("Export to STL - quality"), before any chooser.
macro_rules! shows_first {
    ($name:ident, $id:literal, $path:expr) => {
        pub static $name: Tool = Tool {
            id: $id,
            flow: Flow::Action,
            title: "menu-file",
            entries: &[Entry::Menu($path)],
            other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
            fixture: Fixture::Block,
            picks: &[],
            pick_trial: &[],
            wrong_picks: &[],
            words: &[],
            fields: &[],
            modes: &[],
            // the title is "Export to { $format } - quality": its words for the quality are what it is known by
            result: Outcome::Window { title: "quality" },
            node: "",
            undo: "menu-file",
            undo_steps: 0,
            stays: false,
            upstream: None,
            dependency: None,
            contexts: &[],
            refusal: None,
            budget: (10, 2000),
            help: "general/06-import-export",
            not_applicable: CHOOSES,
        };
    };
}

chooses!(SAVE, "menu.save", &["menu-file", "file-save"], true, "general/08-documents");
chooses!(SAVE_AS, "menu.save-as", &["menu-file", "file-save-as"], true, "general/08-documents");
chooses!(IMPORT, "menu.import", &["menu-file", "file-import"], false, "general/06-import-export");
shows_first!(EXPORT_STL, "menu.export-stl", &["menu-file", "file-export", "STL\u{2026}"]);
chooses!(EXPORT_STEP, "menu.export-step", &["menu-file", "file-export", "STEP\u{2026}"], true, "general/06-import-export");
shows_first!(EXPORT_PLY, "menu.export-ply", &["menu-file", "file-export", "PLY\u{2026}"]);
shows_first!(EXPORT_OBJ, "menu.export-obj", &["menu-file", "file-export", "OBJ\u{2026}"]);
chooses!(EXPORT_IGES, "menu.export-iges", &["menu-file", "file-export", "IGES\u{2026}"], true, "general/06-import-export");
shows_first!(EXPORT_AMF, "menu.export-amf", &["menu-file", "file-export", "AMF\u{2026}"]);
shows_first!(EXPORT_3MF, "menu.export-3mf", &["menu-file", "file-export", "3MF\u{2026}"]);
shows_first!(EXPORT_GLTF, "menu.export-gltf", &["menu-file", "file-export", "glTF\u{2026}"]);

/// An item of the File menu that first shows a window asking something - the changes not saved, the name of a
/// template - on the unsaved block.
macro_rules! asks {
    ($name:ident, $id:literal, $item:literal, $window:literal) => {
        pub static $name: Tool = Tool {
            id: $id,
            flow: Flow::Action,
            title: $item,
            entries: &[Entry::Menu(&["menu-file", $item])],
            other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
            fixture: Fixture::Block,
            picks: &[],
            pick_trial: &[],
            wrong_picks: &[],
            words: &[],
            fields: &[],
            modes: &[],
            result: Outcome::Window { title: $window },
            node: "",
            undo: $item,
            undo_steps: 0,
            stays: false,
            upstream: None,
            dependency: None,
            contexts: &[],
            refusal: None,
            budget: (10, 2000),
            help: "general/08-documents",
            not_applicable: HISTORY,
        };
    };
}

asks!(OPEN_PROJECT, "menu.open-project", "file-open", "nav-unsaved-title");
// a new document from a template replaces the one open, as opening does: unsaved work is asked about first, then the
// chooser of the template comes up
asks!(NEW_FROM_TEMPLATE, "menu.new-from-template", "file-new-from-template", "nav-unsaved-title");
asks!(SAVE_AS_TEMPLATE, "menu.save-as-template", "file-save-as-template", "file-save-as-template");

shows!(DOCUMENT_PROPERTIES, "menu.document-properties", &["menu-file", "file-doc-props"], "doc-props-title", "general/08-documents");
shows!(ABOUT, "menu.about", &["menu-help", "help-about"], "win-about", "general/14-updates");
asks!(QUIT, "menu.quit", "file-quit", "nav-unsaved-title");

/// An item of View that switches the colours of the window: dark or light, whatever the part holds.
macro_rules! paints {
    ($name:ident, $id:literal, $item:literal, $fixture:expr, $dark:literal) => {
        pub static $name: Tool = Tool {
            id: $id,
            flow: Flow::Action,
            title: $item,
            entries: &[Entry::Menu(&["menu-view", $item])],
            other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
            fixture: $fixture,
            picks: &[],
            pick_trial: &[],
            wrong_picks: &[],
            words: &[],
            fields: &[],
            modes: &[],
            result: Outcome::Shade { dark: $dark },
            node: "",
            undo: $item,
            undo_steps: 0,
            stays: false,
            upstream: None,
            dependency: None,
            contexts: &[],
            refusal: None,
            budget: (10, 2000),
            help: "general/07-settings",
            not_applicable: SHOWS,
        };
    };
}

paints!(THEME_LIGHT, "menu.theme-light", "scheme-light", Fixture::FirstPart, false);
paints!(THEME_ALUCARD, "menu.theme-alucard", "scheme-alucard", Fixture::FirstPart, false);
paints!(THEME_DARK, "menu.theme-dark", "scheme-dark", Fixture::FirstPartLight, true);
paints!(THEME_DRACULA, "menu.theme-dracula", "scheme-dracula", Fixture::FirstPartLight, true);

/// PARAMETERS: the word "fx Parameters" on the bar of the window pressed - the window of named values and formulas
/// opens.
pub static PARAMETERS: Tool = Tool {
    id: "menu.parameters",
    flow: Flow::Action,
    title: "par-title",
    // not a menu but a word on the bar: pressed where it is written
    entries: &[Entry::Label("wb-params")],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::FirstPart,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Window { title: "par-title" },
    node: "",
    undo: "par-title",
    undo_steps: 0,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[],
    refusal: None,
    budget: (10, 2000),
    help: "general/05-parameters",
    not_applicable: SHOWS,
};

/// An item of a menu that opens a submenu: what it shows is the first item of it.
macro_rules! opens {
    ($name:ident, $id:literal, $item:literal, $first:literal) => {
        pub static $name: Tool = Tool {
            id: $id,
            flow: Flow::Action,
            title: $item,
            entries: &[Entry::Menu(&["menu-file", $item])],
            other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
            fixture: Fixture::Block,
            picks: &[],
            pick_trial: &[],
            wrong_picks: &[],
            words: &[],
            fields: &[],
            modes: &[],
            result: Outcome::OnScreen { word: $first },
            node: "",
            undo: $item,
            undo_steps: 0,
            stays: false,
            upstream: None,
            dependency: None,
            contexts: &[],
            refusal: None,
            budget: (10, 2000),
            help: "general/08-documents",
            not_applicable: SUBMENU,
        };
    };
}

/// What an item that opens a submenu leaves out: it shows the items under it, and does nothing else.
const SUBMENU: &[(u8, &str)] = &[
    (2, "a menu item has no bar of options"),
    (3, "a menu item takes nothing"),
    (4, "a menu item takes nothing, so there is no wrong pick to refuse"),
    (5, "a menu item has no field"),
    (6, "a menu item has no field to refuse a value in"),
    (7, "the submenu opens at once: there is nothing to show before it"),
    (9, "a submenu is closed by moving off it; that is the menus' own matter"),
    (10, "a submenu open waits for a choice: its items are the round trips of their own contracts"),
    (11, "opening a submenu lays no step of undo"),
    (12, "opening a submenu changes nothing to save"),
    (13, "a menu item lays no node"),
    (14, "nothing of a timeline stands above a menu"),
    (15, "a menu stands on nothing of the document"),
    (16, "the menus are the same from any part"),
    (17, "a submenu always opens"),
    (19, "a menu item is not held, so F1 has no tool in hand to open an article of"),
];

opens!(EXPORT_PROJECT, "menu.export-project", "file-export", "STL\u{2026}");
opens!(RECENT, "menu.recent", "file-recent", "file-recent-clear");

/// RECENT_CLEAR: File -> Recent -> Clear the list, on a start with no recent projects: the item stands disabled beside
/// the words that the list is empty, and pressing it changes nothing - the block stays as it was.
pub static RECENT_CLEAR: Tool = Tool {
    id: "menu.recent-clear",
    flow: Flow::Action,
    title: "file-recent-clear",
    entries: &[Entry::Menu(&["menu-file", "file-recent", "file-recent-clear"])],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Body { volume: 12000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] },
    node: "",
    undo: "file-recent-clear",
    undo_steps: 0,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[],
    refusal: None,
    budget: (10, 2000),
    help: "general/08-documents",
    not_applicable: SUBMENU,
};

/// FIT_VIEW: View -> Fit the view on the block looked at from too near - the whole block comes back into the canvas.
pub static FIT_VIEW: Tool = Tool {
    id: "menu.fit-view",
    flow: Flow::Action,
    title: "menu-fit-view",
    entries: &[Entry::Menu(&["menu-view", "menu-fit-view"])],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::BlockZoomedIn,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Fitted,
    node: "",
    undo: "menu-fit-view",
    undo_steps: 0,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[],
    refusal: None,
    budget: (10, 2000),
    help: "general/09-viewport",
    not_applicable: SHOWS,
};
