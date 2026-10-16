//! THE SETTINGS OF THE PROGRAM: the gestures of the mouse, the defaults, the snapping.

use super::*;

/// WHAT OCCUPIES THE HAND among the things with no command number - by the key of its hint.
///
/// The conditions here are THE SAME ones by which a button in the bar is shown as pressed (`icon_tool(..., active)`).
/// Otherwise F1 and the highlight of the button would diverge: the button glows and the help is about something else.
///
/// ACTION buttons (create a part, insert a component) are not here and cannot be: they leave no state, and there is
/// nothing to ask about during them. Their rows in the table hold a different promise - that the article is written and
/// will be found through the contents.
pub fn armed_toolbar_hint(pn: &Painting) -> Option<&'static str> {
    if pn.boolean.pick.is_some() {
        return Some("tb-bool-bodies-hint");
    }
    if pn.mirror.in_hand() {
        return Some("tb-mirror-part-hint");
    }
    if pn.picking.fillet_all() {
        return Some("tb-fillet-all-hint");
    }
    // Edit -> Copy, Cut, Insert in a sketch are the copy's article: the base point and the place
    if pn.clip.geom_pending.is_some() || pn.clip.geom_place {
        return Some("tb-copy-hint");
    }
    match pn.armed.move_op() {
        1 => return Some("tb-move-hint"),
        2 => return Some("tb-copy-hint"),
        3 => return Some("tb-rotate-hint"),
        _ => {}
    }
    match pn.armed.pat_op() {
        1 => return Some("tb-lin-array-hint"),
        2 => return Some("tb-circ-array-hint"),
        _ => {}
    }
    if pn.m3.on {
        return Some("tb-measure3d-hint");
    }
    if pn.armed.measuring() {
        return Some("tb-measure-hint");
    }
    if pn.section.pick || pn.section.plane.is_some() {
        return Some("tb-section-hint");
    }
    // IN A SKETCH THE HAND IS NEVER EMPTY: with no tool taken it holds the arrow, and F1 answers about selecting
    if pn.sketch_ses.editing.is_some() && matches!(pn.armed, Armed::None) && pn.sel_sk.constraint.is_none() && pn.sel_sk.modify.is_none() {
        return Some("tb-select-hint");
    }
    None
}

/// ALL THE PROGRAM'S SETTINGS IN ONE RECORD, WHICH IS ALSO THE SOLE OWNER OF THE VALUES.
///
/// The settings used to be held two ways at once: some as fields of `App`, some as a positional tuple in the
/// store, while the list of what gets saved was written by hand separately from the settings window. Forgetting to
/// add a setting to that list was THE NORM rather than an accident: the compiler says nothing, no test goes red,
/// and it is a person who finds out. And so it went - out of the window's thirteen settings, seven survived a
/// restart, and among those forgotten was THE THEME ITSELF.
///
/// A value now lives in exactly one place: here. The settings window edits this record, the store saves it whole,
/// and the program reads from it. So "the setting is in the window but is not saved" stopped being an expressible
/// state: to lose one, a field would have to be left uncreated.
///
/// **ONLY a person's choices go here.** Derived state (the snap hint, the highlight, the caches) does not: it is
/// recomputed and has no business in a settings file.
///
/// `serde(default)` on the record and on the nested ones: a setting added by a newer version is read from an older
/// file with its own default rather than bringing down THE WHOLE settings file.
/// HOW THE CAMERA PROJECTS. Two states with names, not a boolean: the settings window draws it as a pair of
/// buttons, and `selectable_value(&mut flag, false, "Orthographic")` made the reader work out which of the two
/// `false` meant.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Projection {
    /// Parallel: the size on screen does not depend on the distance. What engineering drawing means by a view.
    Ortho,
    /// A vanishing point: nearer is larger. Easier to read a shape by, harder to measure by eye.
    Perspective,
}

/// HOW A FACE IS LIT. Same reason as `Projection`: a pair of named buttons rather than a boolean.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Shading {
    /// From the smoothed vertex normals (Gouraud): a curved surface looks curved.
    Smooth,
    /// From the face's own normal: every facet is visible, which is what one wants when checking a mesh.
    Flat,
}

/// A NAVIGATION GESTURE: which buttons are held and which modifiers, to move the view.
///
/// Written as data rather than as a branch per style. With eleven layouts a condition written out at each
/// one is eleven places to get wrong, and the differences between them are exactly this: which buttons,
/// which modifiers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Gesture {
    /// Every one of these must be held. EMPTY means no button at all - a touchpad layout moves the view on
    /// a modifier plus a movement.
    pub buttons: &'static [egui::PointerButton],
    /// WHICHEVER BUTTON IS DRAGGING will do. Only ours works this way, and it is why the right button has
    /// always turned the camera here: a drag is a drag. Naming the left button instead would have taken
    /// that away silently.
    pub any_button: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Gesture {
    pub(crate) const fn of(buttons: &'static [egui::PointerButton]) -> Self {
        Gesture { buttons, any_button: false, shift: false, ctrl: false, alt: false }
    }
    /// Whichever button is dragging.
    pub(crate) const fn any() -> Self {
        Gesture { buttons: &[], any_button: true, shift: false, ctrl: false, alt: false }
    }
    /// MAY THIS GESTURE MOVE THE SHEET OF A SKETCH? The left button is the sketch's own there - it draws, grabs and, with
    /// Ctrl or Shift, adds to the selection - so a gesture holding it moves the sheet only as a chord of two buttons, the
    /// left with the right or the middle one (decided 28.09).
    pub fn sheet_may_take(&self) -> bool {
        !self.takes_a_bare_left_drag() && (!self.buttons.contains(&egui::PointerButton::Primary) || self.buttons.len() >= 2)
    }

    /// Is the left button held together with the right or the middle one - a chord of the layout's movements rather than
    /// the left button's own drag?
    pub fn chord_held(ctx: &egui::Context) -> bool {
        ctx.input(|i| i.pointer.primary_down() && (i.pointer.secondary_down() || i.pointer.middle_down()))
    }

    /// Does a plain left drag, with no modifier, make this gesture?
    pub fn takes_a_bare_left_drag(&self) -> bool {
        !self.shift && !self.ctrl && !self.alt && (self.any_button || self.buttons == [egui::PointerButton::Primary])
    }
    pub(crate) const fn shift(mut self) -> Self {
        self.shift = true;
        self
    }
    pub(crate) const fn ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }
    pub(crate) const fn alt(mut self) -> Self {
        self.alt = true;
        self
    }
    /// Nothing at all: the layout does not offer this movement.
    pub const NONE: Gesture = Gesture { buttons: &[], any_button: false, shift: false, ctrl: false, alt: false };

    /// Is this gesture being made right now over `resp`?
    ///
    /// THE MODIFIERS ARE MATCHED EXACTLY, not merely "held". Otherwise a layout whose pan is Shift and
    /// whose rotate is bare would pan and rotate at once the moment Shift went down, and the two layouts
    /// that differ only by a modifier would be the same layout.
    pub fn active(&self, ctx: &egui::Context, resp: &egui::Response) -> bool {
        if *self == Gesture::NONE {
            return false;
        }
        let m = ctx.input(|i| i.modifiers);
        if m.shift != self.shift || (m.ctrl || m.command) != self.ctrl || m.alt != self.alt {
            return false;
        }
        if self.any_button {
            return resp.dragged();
        }
        if self.buttons.is_empty() {
            // no button: the pointer must simply be over the canvas and moving
            return resp.hovered() && ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        }
        // AT LEAST ONE OF THEM MUST BE A DRAG ON THIS CANVAS. `button_down` alone says only that a button is
        // held SOMEWHERE - dragging a window by its title bar holds one too, and the camera turned along
        // with the window. The drag anchors the gesture to the viewport; the rest of the buttons are then
        // merely required to be down, because egui reports the drag for one of them only.
        //
        // AND NO OTHER BUTTON IS HELD: the buttons are matched exactly, as the modifiers are. A pan on the middle
        // button and a rotate on middle + left otherwise both answered the chord, the pan was asked first, and the
        // chord moved the view by the drag without ever turning it.
        let extra = [LEFT, RIGHT, MIDDLE].into_iter().filter(|b| !self.buttons.contains(b)).any(|b| ctx.input(|i| i.pointer.button_down(b)));
        !extra && self.buttons.iter().any(|b| resp.dragged_by(*b)) && self.buttons.iter().all(|b| resp.dragged_by(*b) || ctx.input(|i| i.pointer.button_down(*b)))
    }
}

pub(crate) const LEFT: egui::PointerButton = egui::PointerButton::Primary;
pub(crate) const RIGHT: egui::PointerButton = egui::PointerButton::Secondary;
pub(crate) const MIDDLE: egui::PointerButton = egui::PointerButton::Middle;

/// HOW THE MOUSE MOVES THE VIEW - a layout picked from a list, as other CADs offer.
///
/// Reported behaviour: "add a way to choose how the mouse behaves; other CADs have a dropdown of mouse
/// layouts for the popular ones. Make the same list, with our current layout in it as QymCAD, and let it
/// be the default."
///
/// The ten borrowed layouts are transcribed from the documentation of the CAD they are named after rather
/// than invented here: somebody choosing "Blender" wants Blender's bindings, and a layout that only
/// resembles them is worse than none - it is the habit they came with, failing in a way they cannot name.
///
/// IN A SKETCH the layouts agree wherever they can: a flat sheet has nothing to rotate, so what is left is
/// pan and zoom.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum MouseNav {
    /// OURS, and the factory choice: a drag turns the model, Shift and a drag moves it.
    QymCad,
    Cad,
    Blender,
    Gesture,
    MayaGesture,
    OpenCascade,
    OpenInventor,
    OpenScad,
    Revit,
    TinkerCad,
    Touchpad,
    /// A PEN's: the left button turns the model, the middle one moves the view, the two together zoom - right or up
    /// nearer, left or down farther. Asked for by a person working with a pen; named as they asked.
    Den,
}

impl MouseNav {
    /// EVERY LAYOUT. The one list the settings window, the checks and the catalogue walk over.
    pub const ALL: [MouseNav; 12] = [
        MouseNav::QymCad,
        MouseNav::Cad,
        MouseNav::Blender,
        MouseNav::Gesture,
        MouseNav::MayaGesture,
        MouseNav::OpenCascade,
        MouseNav::OpenInventor,
        MouseNav::OpenScad,
        MouseNav::Revit,
        MouseNav::TinkerCad,
        MouseNav::Touchpad,
        MouseNav::Den,
    ];

    /// The stable name used in the settings file and in the catalogue key.
    pub fn code(self) -> &'static str {
        match self {
            MouseNav::QymCad => "qymcad",
            MouseNav::Cad => "cad",
            MouseNav::Blender => "blender",
            MouseNav::Gesture => "gesture",
            MouseNav::MayaGesture => "maya",
            MouseNav::OpenCascade => "opencascade",
            MouseNav::OpenInventor => "openinventor",
            MouseNav::OpenScad => "openscad",
            MouseNav::Revit => "revit",
            MouseNav::TinkerCad => "tinkercad",
            MouseNav::Touchpad => "touchpad",
            MouseNav::Den => "den",
        }
    }

    /// TURNING THE MODEL: every gesture of the layout that does it, as the program the layout is named after has them,
    /// read from that program's handling of the events rather than from its help lines, which lag behind it.
    pub fn rotates(self) -> &'static [Gesture] {
        match self {
            MouseNav::QymCad => const { &[Gesture::any()] },
            MouseNav::Cad => const { &[Gesture::of(&[MIDDLE, LEFT]), Gesture::of(&[MIDDLE, RIGHT]), Gesture::of(&[RIGHT]).shift()] },
            MouseNav::Blender => const { &[Gesture::of(&[MIDDLE])] },
            MouseNav::Gesture | MouseNav::OpenScad | MouseNav::OpenInventor | MouseNav::Den => const { &[Gesture::of(&[LEFT])] },
            MouseNav::MayaGesture => const { &[Gesture::of(&[LEFT]).alt()] },
            MouseNav::OpenCascade => const { &[Gesture::of(&[RIGHT]).ctrl()] },
            MouseNav::Revit => const { &[Gesture::of(&[MIDDLE]).shift()] },
            MouseNav::TinkerCad => const { &[Gesture::of(&[RIGHT])] },
            MouseNav::Touchpad => const { &[Gesture::of(&[]).alt()] },
        }
    }

    /// MOVING THE VIEW SIDEWAYS: every gesture of the layout that does it.
    pub fn pans(self) -> &'static [Gesture] {
        match self {
            MouseNav::QymCad => const { &[Gesture::any().shift()] },
            MouseNav::Cad => const { &[Gesture::of(&[MIDDLE]), Gesture::of(&[RIGHT]).ctrl()] },
            MouseNav::Blender => const { &[Gesture::of(&[MIDDLE]).shift(), Gesture::of(&[LEFT, RIGHT])] },
            MouseNav::Revit => const { &[Gesture::of(&[MIDDLE]), Gesture::of(&[LEFT, RIGHT])] },
            MouseNav::OpenCascade => const { &[Gesture::of(&[MIDDLE]), Gesture::of(&[MIDDLE]).ctrl()] },
            MouseNav::OpenInventor => const { &[Gesture::of(&[MIDDLE]), Gesture::of(&[LEFT]).ctrl().shift(), Gesture::of(&[]).ctrl().shift()] },
            MouseNav::TinkerCad | MouseNav::Den => const { &[Gesture::of(&[MIDDLE])] },
            MouseNav::Gesture | MouseNav::OpenScad => const { &[Gesture::of(&[RIGHT])] },
            MouseNav::MayaGesture => const { &[Gesture::of(&[MIDDLE]).alt()] },
            MouseNav::Touchpad => const { &[Gesture::of(&[]).shift()] },
        }
    }

    /// ZOOMING BY A MOVEMENT, up and down, besides the wheel: every gesture of the layout that does it.
    pub fn zooms(self) -> &'static [Gesture] {
        match self {
            MouseNav::QymCad | MouseNav::Gesture | MouseNav::TinkerCad => &[],
            MouseNav::Cad => const { &[Gesture::of(&[RIGHT]).ctrl().shift()] },
            MouseNav::Blender | MouseNav::Revit => const { &[Gesture::of(&[MIDDLE]).ctrl(), Gesture::of(&[RIGHT]).ctrl().shift()] },
            MouseNav::OpenInventor => const { &[Gesture::of(&[LEFT, MIDDLE]), Gesture::of(&[MIDDLE]).ctrl(), Gesture::of(&[RIGHT]).ctrl().shift()] },
            MouseNav::OpenScad => const { &[Gesture::of(&[MIDDLE]), Gesture::of(&[RIGHT]).shift(), Gesture::of(&[MIDDLE]).shift()] },
            MouseNav::MayaGesture => const { &[Gesture::of(&[RIGHT]).alt()] },
            MouseNav::OpenCascade => const { &[Gesture::of(&[LEFT]).ctrl()] },
            MouseNav::Touchpad => const { &[Gesture::of(&[]).ctrl().shift()] },
            MouseNav::Den => const { &[Gesture::of(&[MIDDLE, LEFT])] },
        }
    }

    /// DOES A MOVEMENT SIDEWAYS ZOOM TOO? Under the pen's layout right brings the view nearer and left takes it away, as
    /// up and down do; every other layout zooms by up and down alone, as its program does.
    pub fn zooms_sideways(self) -> bool {
        self == MouseNav::Den
    }

    /// TILTING THE VIEW about the line of sight: the left and right buttons together under Gesture, as its program has
    /// it; no other layout tilts.
    pub fn tilts(self) -> &'static [Gesture] {
        match self {
            MouseNav::Gesture => const { &[Gesture::of(&[LEFT, RIGHT])] },
            _ => &[],
        }
    }

    /// The first way of turning the model - the one a layout is known by.
    pub fn rotate(self) -> Gesture {
        self.rotates()[0]
    }

    /// The first way of moving the view - the one a layout is known by.
    pub fn pan(self) -> Gesture {
        self.pans()[0]
    }

    /// TAKING A PART WITHOUT ITS GIZMO: ours alone - Shift and the left button, the drag begun on the part; begun on
    /// empty space it is still ours to move the view. Every other layout is the one of its own program and does what
    /// that program does, which has no such gesture. Reported behaviour: the middle and the right button begun on a
    /// part carried the part instead of the view. Decided 28.09: the view keeps every button, and a part is taken
    /// with Shift and the left one.
    pub fn take_a_part(self) -> Gesture {
        match self {
            MouseNav::QymCad => Gesture::of(&[LEFT]).shift(),
            _ => Gesture::NONE,
        }
    }

    /// Whether the wheel zooms under this layout.
    pub fn wheel_zooms(self) -> bool {
        self != MouseNav::Touchpad
    }

    /// THE FRAME OF SELECTION: the gesture that draws it, and whether it may start on the model (`true`) or from empty
    /// space only. The layouts of other programs draw it as their programs do - a bare left drag from anywhere, or Shift
    /// and the left one where the bare left turns the model; two draw none, their left drag turns the model wherever it
    /// starts. Ours draws it with the bare left from empty space: begun on the model, the left
    /// drag turns it.
    pub fn frames(self) -> Option<(Gesture, bool)> {
        match self {
            MouseNav::QymCad => Some((Gesture::of(&[LEFT]), false)),
            MouseNav::Cad | MouseNav::Blender | MouseNav::Revit | MouseNav::TinkerCad | MouseNav::Touchpad | MouseNav::OpenCascade => Some((Gesture::of(&[LEFT]), true)),
            MouseNav::Gesture | MouseNav::MayaGesture | MouseNav::Den => Some((Gesture::of(&[LEFT]).shift(), true)),
            MouseNav::OpenInventor | MouseNav::OpenScad => None,
        }
    }

    /// A SHORT CLICK OF THE MIDDLE BUTTON LOOKS AT THE POINT UNDER IT - the view turns about it from then on, and it
    /// comes to the middle of the view - under the layouts whose programs do so; ours, OpenCascade, OpenSCAD, TinkerCAD
    /// and the touchpad give the middle click nothing.
    pub fn middle_click_looks(self) -> bool {
        matches!(self, MouseNav::Cad | MouseNav::Blender | MouseNav::Revit | MouseNav::OpenInventor | MouseNav::MayaGesture | MouseNav::Gesture)
    }

    /// DOES A CLICK IN THE 3D VIEW TAKE WHAT IS UNDER IT, with the modifiers held now? One layout turns the model on
    /// the bare left button, and its program selects with Ctrl or Shift and the left button; a bare click there
    /// takes nothing. In a sketch the click is the sketch's own, as it is in that program's edit mode.
    pub fn click_takes(self, ctx: &egui::Context) -> bool {
        self != MouseNav::OpenInventor || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command || i.modifiers.shift)
    }

    /// The catalogue key holding this layout's name.
    pub fn key(self) -> String {
        format!("settings-mouse-{}", self.code())
    }

    /// The catalogue key of the line describing what the layout does.
    pub fn hint_key(self) -> String {
        format!("settings-mouse-{}-hint", self.code())
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    /// WHERE THE PANELS STAND, but only what the person MOVED. The defaults come from the code: writing them
    /// into the file would freeze them, and a panel added later would never appear for anyone who had ever
    /// saved. An empty list therefore means "as the workbenches registered them", not "no panels".
    pub layout: Vec<(String, qymcad_shell::Slot)>,
    /// THE INTERFACE LANGUAGE (a code from the `i18n/` catalogue). Empty means nothing has been chosen yet, so it
    /// follows the system locale, and an undetermined locale gives English. An explicit choice overrides both.
    pub language: String,
    /// THE COLOUR SCHEME by its STABLE identifier (`dark`, `light`, or one of the user's). Not by its label: a
    /// built-in scheme's label is translated, and a settings record has no right to depend on the language.
    pub scheme: String,
    /// the ViewCube's size: 0 small, 1 medium, 2 large. On a 4K screen the former 36 px were unreadable, while
    /// "make it larger for everyone" would get in the way on a small screen - this is a person's choice.
    pub viewcube_size: u8,
    /// HOW OFTEN TO WRITE AN AUTOSAVE, in seconds. `0` means never.
    ///
    /// It used to be three minutes as a constant. On a heavy assembly the write is noticeable, and some want
    /// fifteen minutes while others want thirty seconds: the cost of losing work and the cost of a pause differ
    /// from person to person.
    pub autosave_secs: u64,
    /// HOW MANY UNDO STEPS TO KEEP. Memory against the history's length is a person's choice, not ours: a snapshot
    /// of an assembly weighs tens of megabytes, and some people roll back fifty steps.
    pub undo_cap: usize,
    /// THE GHOSTS' OPACITY (0 to 255): a part outside the context, an operation's preview. Some find it in the way
    /// and others cannot see it at all - a matter of taste rather than truth.
    pub ghost_alpha: u8,
    /// HOW MANY CORES THE KERNEL MAY TAKE for a rebuild. Zero means all but one; ONE means single-threaded.
    ///
    /// A number rather than a number plus a tick: "one core" is the off switch, and a separate flag beside it
    /// would allow the pair "off, eight cores", which means nothing. Measured on the reference documents: on all
    /// cores the scenario document rebuilds in 17.2 s against 20.3 s
    /// on one. All but one by default, so the machine stays usable while a heavy rebuild runs - and so that a
    /// person whose computer is busy with something else can give the kernel less.
    pub kernel_threads: i32,
    /// THE PERSPECTIVE FIELD OF VIEW, in degrees (the full vertical angle). Everyone is used to their own.
    pub persp_fov_deg: f64,
    /// GPU ANTIALIASING (the MSAA sample count): 1, 2, 4 or 8.
    ///
    /// IT TAKES EFFECT ON A RESTART, and the window says so. The sample count is baked into the wgpu pipelines when
    /// the renderer is created; changing it on the fly would mean rebuilding the pipelines and the render targets.
    /// Silently not applying it would be a lie, so the setting states its price plainly.
    pub msaa: u32,
    /// THE REASSIGNED KEYS: an action's code mapped to a key. ONLY the differences from the factory ones.
    ///
    /// Storing the full layout would be a mistake: a new tool added to the program would never appear for anyone
    /// who had ever touched the keys - their action simply would not be in the record. The differences, by
    /// contrast, survive any growth of the table.
    pub hotkeys: std::collections::BTreeMap<String, String>,
    /// THE HELP'S LANGUAGE, SEPARATE FROM THE INTERFACE'S. Empty means whatever the interface uses.
    ///
    /// Not a whim: CAD terminology is English, and someone working in a translated interface may well want to read
    /// `sweep` and `loft` rather than their translations, so that it matches what they see in manuals and on
    /// forums. The reverse holds too.
    pub help_lang: String,
    /// OPEN THE HELP IN A BROWSER rather than in the program's own window.
    ///
    /// The own window is the default: engineering software must explain itself without the internet. But some
    /// people have a second monitor, and keeping an article beside the program is easier in a browser.
    pub help_external: bool,
    /// the GPU viewport (otherwise the CPU raster)
    pub gpu_viewport: bool,
    /// The camera's projection.
    pub projection: Projection,
    /// How a face is lit.
    pub shading: Shading,
    /// THE SHARED TOGGLE FOR SKETCH OUTLINES - FOR THE ASSEMBLY ONLY.
    ///
    /// Inside a Part there is none and there must be none: every sketch has a visibility checkbox of its own there
    /// and a shared one would duplicate it. But in an ASSEMBLY the sketches number in the dozens across all the
    /// components, and hiding them one by one is impractical - there a shared one is needed.
    pub show_contours: bool,
    /// the mate glyphs in an assembly
    pub show_joints: bool,
    /// the interference check in an assembly (expensive, so off by default)
    pub show_interference: bool,
    /// the cursor snap: whether it is on, the grid step, the angle step
    pub snap: Snapping,
    /// the automatic constraints while drawing
    pub auto_constrain: bool,
    /// THE NUMBERS OF THE POINTS of a sketch beside them: off by default - to a person drawing they mean nothing and
    /// clutter the sheet; on for looking into a sketch (`draw_sketch_points`)
    pub show_point_numbers: bool,
    /// WHAT THE LABEL OF A SKETCH DIMENSION SAYS beside its value: the name of a driver (`w = 110`) and the formula it
    /// is set by (`2*w+10 = 110`); both on reads `w = 2*w+10 = 110`
    pub dim_show_name: bool,
    pub dim_show_formula: bool,
    /// the size of the labels of sketch dimensions, px
    pub dim_font: f32,
    /// how the text of a linear dimension is turned: level whatever the line, or along its dimension line
    pub dim_text: DimTextTurn,
    /// the values the commands open with
    pub defaults: Defaults,
    /// THE RECENT FILES, the newest first. They live in the settings record because they are saved by the same
    /// mechanism and survive a restart in exactly the same way; a separate store would have to be created, saved
    /// and tested all over again.
    #[serde(default)]
    pub recent: Vec<String>,
    /// HOW MANY FILES TO REMEMBER. A screen-long list is useless: it is searched by eye.
    #[serde(default = "default_recent_limit")]
    pub recent_limit: usize,
    /// THE INTERFACE SCALE. A multiplier for the size of everything `egui` draws: the fonts, the buttons, the
    /// margins. On a 4K screen the factory value gives an unreadably small interface, while on a small screen a
    /// large one eats the viewport - this is a person's choice, not ours.
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    /// POINTING PRECISION: 0 precise, 1 ordinary, 2 coarse. It scales the grab radii (see `grab.rs`).
    /// On a 4K or a touch screen the former pixel radii are small, while with a mouse large ones make it hard to
    /// aim in tight geometry - this is a person's choice, not ours.
    #[serde(default = "default_pick_precision")]
    pub pick_precision: u8,

    // --- WHAT THE PROGRAM OPENS WITH. Two independent answers, not one setting with three states. ---
    /// REOPEN THE PROJECT OF THE PREVIOUS SESSION.
    ///
    /// On by default, decided 25.09 on a report: the project open when the program closed is open when it starts.
    /// Somebody who opens the CAD to try something turns it off in the settings; with no previous project the start
    /// screen comes up.
    #[serde(default = "default_true")]
    pub open_last: bool,
    /// LET THE START SCREEN COME UP BY ITSELF on an empty document.
    ///
    /// Separate from `open_last` because the two answer different questions: one is about which document,
    /// the other about whether to be greeted. With it off an empty document simply opens, and the screen
    /// stays reachable from the menu.
    #[serde(default = "default_true")]
    pub show_start_screen: bool,
    /// WHICH BUTTON MOVES THE VIEW. See `MouseNav`.
    #[serde(default = "default_mouse_nav")]
    pub mouse_nav: MouseNav,
    /// Where the view zooms from: the cursor, or the middle of the viewport.
    #[serde(default = "default_zoom_at")]
    pub zoom_at: ZoomAt,
    /// The exception to that while a command is open.
    #[serde(default = "default_zoom_editing")]
    pub zoom_editing: ZoomWhileEditing,
    /// What the view turns about: the middle of the view, or the point under the pointer (see `OrbitAbout`).
    #[serde(default = "default_orbit_about")]
    pub orbit_about: OrbitAbout,
    /// How often to ask whether a newer version exists (see `UpdateCheck`).
    #[serde(default = "default_update_check")]
    pub update_check: UpdateCheck,
    /// When it was last asked, in seconds since the epoch. `0` means never.
    ///
    /// Kept beside the setting rather than worked out from anything: without it "once a day" would mean
    /// "at every start", since nothing else in the program remembers that a start happened.
    #[serde(default)]
    pub update_last_checked: u64,
    /// ASK FOR THE UNITS AND THE SCALE ON EVERY IMPORT. Off, the window comes up only for a file without units and
    /// for a model under 1 mm or over 10 m.
    #[serde(default)]
    pub import_ask_always: bool,
    /// Active icon theme packs in descending order of priority.
    #[serde(default)]
    pub active_icon_packs: Vec<String>,
    /// Whether dev watch mode (live auto-reload) is enabled for unpacked icon packs.
    #[serde(default)]
    pub icon_dev_watch: bool,
    /// Installed but disabled icon theme packs.
    #[serde(default)]
    pub inactive_icon_packs: Vec<String>,
    /// THE UNIT LAST CHOSEN FOR A FORMAT WITHOUT UNITS, by the format's name: the next file of it comes in the same.
    #[serde(default)]
    pub import_units: std::collections::BTreeMap<String, String>,
    /// Packs with live file watching enabled (specifically for directory packs).
    #[serde(default)]
    pub watched_icon_packs: Vec<String>,
}

pub(crate) fn default_orbit_about() -> OrbitAbout {
    OrbitAbout::ViewCentre // the program's own way: the view turns as it always has
}

/// The factory layout: ours.
pub(crate) fn default_zoom_at() -> ZoomAt {
    ZoomAt::Cursor // what every CAD a person comes from does
}

pub(crate) fn default_update_check() -> UpdateCheck {
    UpdateCheck::Daily
}

pub(crate) fn default_zoom_editing() -> ZoomWhileEditing {
    ZoomWhileEditing::PartCentre
}

pub(crate) fn default_mouse_nav() -> MouseNav {
    MouseNav::QymCad
}

/// A `serde(default)` for a flag whose factory value is "on".
pub(crate) fn default_true() -> bool {
    true
}

/// DEFAULT VALUES — not the state of a command but SETTINGS: a command opens with them, and they
/// outlive its closing. They used to sit among the command's own fields and so looked like state.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Defaults {
    /// the extrusion height the command opens with
    pub extrude_h: f64,
    /// the offset distance for 2D contour edits
    pub offset_2d: f64,
}

impl Default for Defaults {
    fn default() -> Self {
        Self { extrude_h: 10.0, offset_2d: 3.0 }
    }
}

/// SNAPPING WHILE DRAWING — SETTINGS ONLY.
///
/// The hint saying "where it snapped just now" used to live here too, and it is derived from the
/// cursor: it is recomputed every frame and has no place among settings. While it sat inside, this
/// record could not be a setting as a whole — and being a setting as a whole is exactly what makes
/// saving automatic (see `Settings`). The hint moved to `App::snap_hint`.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Snapping {
    pub on: bool,
    /// grid step, mm
    pub grid: f64,
    /// angle snapping step, deg
    pub rot_deg: f64,
}

impl Default for Snapping {
    fn default() -> Self {
        Self { on: true, grid: 1.0, rot_deg: 15.0 }
    }
}
/// The factory interface scale - whatever the system decides.
pub(crate) fn default_ui_scale() -> f32 {
    1.0
}

/// The factory length of the recent-files list.
pub(crate) fn default_recent_limit() -> usize {
    10
}

/// The factory pointing precision - ordinary. As a function of its own, because `serde(default)` on a field wants
/// exactly that and the record's whole `Default` will not do here.
pub(crate) fn default_pick_precision() -> u8 {
    1
}
