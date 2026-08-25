//! THE COLOUR SCHEME - ONE SOURCE OF COLOUR INSTEAD OF LITERALS ON THE SPOT.
//!
//! Reported: the light theme did not repaint the viewport and the sketcher - they stayed black; nor did it
//! repaint the horizontal bar of the active tool. The causes were specific, and both structural:
//!
//! - the viewport background was filled with `Color32::from_gray(26)` - it never asked about the theme;
//! - the tool bar was built by a function **without `&self`**, so it could not look at the theme even if
//!   it had wanted to.
//!
//! But mending those two lines would have been mending a symptom. The interface layer holds **266 uses of
//! `Color32` and 108 distinct literals**: while colour stays a number at the point of use, the theme will
//! always lag behind - every new tool brings a colour of its own and drifts away from the scheme again.
//!
//! So the colours here are named BY MEANING (`sketch_line`, `error`, `preview`) rather than by appearance
//! ("yellow"): the code asks for a meaning, the scheme answers with a colour. A scheme is data, not code,
//! hence:
//!
//! 1. **The dark scheme is A TRANSCRIPTION OF WHAT EXISTS**, literal for literal. The present look was
//!    said to be liked; so this is no place for taste, this is a place for precision.
//! 2. **The light one is DERIVED BY A RULE rather than by inversion** (see [`toward_light`]), plus manual
//!    exceptions where the rule is blind. The rule matters more than the list: it extends to colours that
//!    do not exist yet - otherwise every new tool would mend the light scheme by hand all over again.
//! 3. **A scheme of your own is a file**, by the same means as the languages: copy it, edit it, drop it
//!    alongside.
//!
//! # Why there are so many fields
//!
//! There are about seventy, and that is not sprawl but a census of what exists: that is how many DIFFERENT
//! roles colour really has in this interface. Squeezing them into "a pretty dozen" would mean changing a
//! look that is liked as it is.
//!
//! # What the name decides
//!
//! **The role, not the value.** One colour shared by two different roles is no reason to merge the names:
//! `sketch_axis_idle` and `panel_border` are both grey today, but in another scheme they will part ways,
//! and there would be nowhere to call a panel border "a sketch axis" from.
//!
//! The other way round is harder. The literals had drifted: `255,200,90`, `250,200,90`, `255,200,80`,
//! `240,200,90` - one and the same amber, typed four times from memory. One role, different numbers.
//! Those are brought under one name: the disagreement is exactly the disease.
//!
//! Where the difference within one role was VISIBLE, the names stayed separate at first: merging them
//! would have changed the look, and the look was transcribed as it is. Such places went into the plan as
//! questions - and so the measuring line, amber in 3D and green in the sketch, became one green line by
//! decision. The cut line, which had shared its name by an accidental match of colour, got one of its own:
//! they are different tools.
use egui::Color32;
use serde::{Deserialize, Serialize};

/// NAMED COLOURS. The fields are named BY MEANING: the code asks for "the sketch line", not "yellow" -
/// otherwise the light scheme would send you hunting for where yellow meant a selection and where it meant
/// a warning.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    /// THE STABLE IDENTIFIER of a scheme: it lives in the settings and in the file name, and does not
    /// depend on the language. The built-in ones are `dark`/`light`; a custom one carries what was typed.
    pub id: String,
    /// THE CAPTION FOR A PERSON. Empty for the built-in ones: their caption comes from the language
    /// catalogue, otherwise one language would show a name written in another. For a custom scheme it is
    /// what was typed: that word is not subject to translation.
    pub name: String,
    /// whether the scheme is light - this picks the `egui` look and derives the shades that follow
    pub light: bool,
    /// WHETHER THE SCHEME PAINTS THE INTERFACE ITSELF (panels, buttons, fields) and not just the canvas.
    ///
    /// Off means the interface takes the stock `egui` look, light or dark, exactly as before. The built-in
    /// dark and light schemes are like that: the present look was said to be liked, and repainting it
    /// "while we are at it" would be a rework nobody asked for. Schemes in the vein of Dracula turn this
    /// on and paint the whole program - the `ui_*` fields exist for their sake.
    #[serde(default)]
    pub ui_on: bool,

    // --- window, panels, text ---
    /// the background of the 3D viewport and of the sketch canvas
    pub viewport_bg: [u8; 3],
    /// the background of the bar of the active tool
    pub toolbar_bg: [u8; 3],
    /// the background of a card over the canvas (the modal entry of a dimension)
    pub panel_bg: [u8; 3],
    /// the border of such a card
    pub panel_border: [u8; 3],
    /// the background of the splash screen at startup
    pub splash_bg: [u8; 3],
    /// the background of a component thumbnail in the tree
    pub thumbnail_bg: [u8; 3],
    /// the dimming under a modal window (used with an alpha)
    pub scrim: [u8; 3],
    /// the main text over the canvas
    pub text_strong: [u8; 3],
    /// explanatory text over the canvas
    pub text_dim: [u8; 3],
    /// the secondary matter: a reference dimension, a hint
    pub text_faint: [u8; 3],
    /// the brightest of all - whatever is under the cursor right now
    pub emphasis: [u8; 3],
    /// the mark inside a coloured plate (a constraint glyph)
    pub glyph_text: [u8; 3],
    /// THE DISC BEHIND A GLYPH. Its job is to be THE OPPOSITE of the glyph rather than "the dark one": in
    /// a light scheme the glyph itself turns dark, and a dark disc behind it turns the glyph into mush.
    /// Reported: every glyph in the light theme looked as if it were under milk.
    pub glyph_backing: [u8; 3],

    // --- grid and axes ---
    /// the grid on the canvas (the major lines)
    pub grid: [u8; 3],
    /// the minor grid lines - they are meant to be quieter than the major ones
    pub grid_minor: [u8; 3],
    /// the axes of the move gizmo (bright, interactive)
    pub axis_x: [u8; 3],
    pub axis_y: [u8; 3],
    pub axis_z: [u8; 3],
    /// the axes of the world grid under the model (muted - this is background, not a tool)
    pub grid_axis_x: [u8; 3],
    pub grid_axis_y: [u8; 3],
    pub grid_axis_z: [u8; 3],
    /// the axes of the sketch canvas
    pub sketch_axis_x: [u8; 3],
    pub sketch_axis_y: [u8; 3],
    /// the other guides of the sketch canvas
    pub sketch_axis_idle: [u8; 3],

    // --- sketch geometry ---
    /// an ordinary sketch line
    pub sketch_line: [u8; 3],
    /// construction geometry
    pub sketch_construction: [u8; 3],
    /// a live projection of outside geometry into the sketch
    pub sketch_driven: [u8; 3],
    /// the edges of the face the sketch lies on
    pub sketch_face_edge: [u8; 3],
    /// a sketch shown in 3D, on its own, not selected
    pub sketch_edge_3d: [u8; 3],

    // --- dimensions ---
    /// a dimension that drives the geometry
    pub dimension: [u8; 3],
    /// a driven (reference) dimension - it drives nothing, and that must be visible
    pub dimension_driven: [u8; 3],
    /// the helper leader of a dimension
    pub dim_helper: [u8; 3],
    /// the ring around the helper so it reads on a light face
    pub dim_helper_ring: [u8; 3],

    // --- selection and hover ---
    /// what is selected
    pub selected: [u8; 3],
    /// ATTENTION IS HERE: under the cursor, a selected datum, the copies in an array preview. One role,
    /// one colour; splitting it into "hover" and "selected datum" would give one appearance two names
    pub highlight: [u8; 3],
    /// "working right now": a handle being dragged, a joint being turned, a path being led
    pub active: [u8; 3],
    /// a handle of a command that can be dragged
    pub handle: [u8; 3],
    /// an arrow handle on the face itself (it has a shade of its own: it lies on a body, not in the void)
    pub handle_face: [u8; 3],
    /// the snap point under the cursor
    pub snap_point: [u8; 3],
    /// an axis that is a candidate while an axis is being picked
    pub axis_pick_idle: [u8; 3],
    /// the cut-away part of a body (the section preview)
    pub clip: [u8; 3],
    /// text on a bright plate - it must stay dark in any scheme
    pub plate_text: [u8; 3],
    /// the centre of a circular array, or the mark of a mirror
    pub pattern_center: [u8; 3],
    /// a sketch point on its own
    pub sketch_point: [u8; 3],
    /// a note written on the canvas
    pub annotation: [u8; 3],
    /// the "this is not an error" remark next to a diagnostic
    pub note: [u8; 3],
    /// a hint in a panel: what has been selected so far
    pub hint: [u8; 3],
    /// a hint that asks for something: pick a face, two points are needed
    pub hint_action: [u8; 3],
    /// the selected node of the tree
    pub tree_selected: [u8; 3],
    /// a joint connector in the list
    pub connector: [u8; 3],
    /// the rollback bar in the build history
    pub rollback: [u8; 3],
    /// a window selection box (left to right - only what is wholly inside)
    pub select_window: [u8; 3],
    /// a crossing selection box (right to left - everything that is touched)
    pub select_cross: [u8; 3],
    /// the rubber line from the last point to the cursor
    pub rubber_band: [u8; 3],

    // --- cursor snaps: the colour TELLS THE KIND of snap apart, it is not decoration ---
    /// a vertex, a midpoint, a centre
    pub snap_marker: [u8; 3],
    /// an intersection
    pub snap_intersection: [u8; 3],
    /// a point on an edge
    pub snap_edge: [u8; 3],
    /// an axis
    pub snap_axis: [u8; 3],
    /// a grid node
    pub snap_grid: [u8; 3],
    /// an edge of a body with nothing marked on it
    pub edge_idle: [u8; 3],

    // --- bodies ---
    /// the colour of a body on its best-lit face (deeper means proportionally darker)
    pub body_face: [u8; 3],
    /// the same for a ghost (a translucent body outside the context)
    pub body_ghost: [u8; 3],
    /// the same for a body that has run into something
    pub body_clash: [u8; 3],

    // --- the roles of faces inside an operation ---
    /// material will be added (a pad, a positive offset)
    pub add: [u8; 3],
    /// material will be taken away (a shell)
    pub remove: [u8; 3],
    /// the face will change (a draft, a cut, an offset)
    pub modify: [u8; 3],
    /// the face is taken as a reference and does not change itself (the neutral face of a draft)
    pub reference: [u8; 3],
    /// an offset into the body
    pub offset_in: [u8; 3],

    // --- planes and datums ---
    /// a plane that can be picked
    pub plane_face: [u8; 3],
    /// its fill
    pub plane_fill: [u8; 3],
    /// a datum plane, not selected
    pub plane_idle: [u8; 3],
    /// the normal of a datum plane
    pub plane_normal: [u8; 3],
    /// a datum point
    pub datum_point: [u8; 3],
    /// a datum axis
    pub datum_axis: [u8; 3],

    // --- tool previews ---
    /// the general preview of the coming result
    pub preview: [u8; 3],
    /// the preview of a primitive (a box, a cylinder)
    pub preview_prim: [u8; 3],
    /// the preview of an array of copies
    pub preview_array: [u8; 3],
    /// an axis or plane of symmetry: a mirror, a thread, a revolve
    pub preview_axis: [u8; 3],
    /// helper geometry inside a preview: a datum, a mirror plane, the grid of copies
    pub preview_datum: [u8; 3],
    /// THE MEASURING LINE - ONE FOR THE WHOLE PROGRAM. It used to be amber in 3D and green in a sketch:
    /// one tool, two colours, which is not a design but a disagreement. It was merged into the green one
    /// by decision, so that one colour sets it everywhere.
    pub measure: [u8; 3],
    /// the cut line (the preview of splitting a body). Separate from the measuring line: they are
    /// different tools, and they used to share one name only because their colours happened to match
    pub cut_line: [u8; 3],
    /// the caption at a gizmo (the value currently being dragged)
    pub gizmo_label: [u8; 3],

    // --- sketch constraints ---
    /// the constraint is satisfied
    pub constraint_ok: [u8; 3],
    /// the constraint is selected in the list
    pub constraint_selected: [u8; 3],
    /// the constraint is under the cursor
    pub constraint_hover: [u8; 3],

    // --- assembly ---
    /// a joint on its own
    pub joint_idle: [u8; 3],
    /// a joint under the cursor
    pub joint_hover: [u8; 3],
    /// the first connector being picked for a joint
    pub joint_pick_a: [u8; 3],
    /// the second one
    pub joint_pick_b: [u8; 3],
    /// the component is grounded
    pub grounded: [u8; 3],

    // --- contours ---
    /// a contour on its own
    pub contour_idle: [u8; 3],
    /// a contour under the cursor
    pub contour_hover: [u8; 3],
    /// a contour picked as the profile of an operation
    pub contour_profile: [u8; 3],

    // --- states ---
    /// success, fully defined
    pub ok: [u8; 3],
    /// defined, but with a caveat (there are reference dimensions)
    pub ok_soft: [u8; 3],
    /// not defined yet - degrees of freedom remain
    pub underdefined: [u8; 3],
    /// a warning (a redundant constraint)
    pub warning: [u8; 3],
    /// an error, a conflict
    pub error: [u8; 3],
    /// a milder trouble: a bad expression, an over-defining constraint - red, but not shouting
    pub error_mild: [u8; 3],

    // --- THE DEPTH OF SHADING: how dark the scheme lets the most shaded face become ---
    //
    // This is NOT a colour, but it is a decision of the scheme, and keeping it as a number in the painter
    // is not allowed. The depth was tuned for a DARK background: there a silhouette shaded down to a
    // quarter of the brightness reads perfectly. On a light canvas the same shading turns a part into a
    // dark blot - reported as parts and assemblies looking darker than they should, with a very dark
    // navigation cube.
    //
    // There are three thresholds rather than one because there are three painters and their ranges DIFFER.
    // Collapsing them into one number would change the dark scheme in two places out of three - and its
    // look is transcribed as it is. In the light scheme all three are raised.
    /// bodies in the 3D viewport
    pub shade_floor_body: f32,
    /// the mesh preview in the sketcher
    pub shade_floor_mesh: f32,
    /// the view cube
    pub shade_floor_viewcube: f32,
    /// HOW MUCH THE SCHEME RAISES THE LIGHTNESS OF A PART before shading it.
    ///
    /// Shading is a multiplication, and it can only DARKEN. So the best-lit face is no lighter than the
    /// part's own colour, and the colours of parts (steel, brass, olive) sit around 160 in brightness. On
    /// a dark canvas that is a light object on a dark field; on a light canvas (240) it is a dark object,
    /// and raising the floor alone is useless: the ceiling stays where it was. In the dark scheme this is
    /// zero, that is, exactly the previous behaviour.
    pub body_lighten: f32,
    /// HOW MUCH TO RAISE ITS SATURATION ALONG WITH IT. Without this a part that is being lightened turns
    /// white: the lighter a colour, the closer to white it is by construction. Of the first attempt it was
    /// said that the parts had merely become whiter rather than lighter or brighter - this is exactly the
    /// parameter that was missing then.
    pub body_saturate: f32,
    /// where a ghost (a part outside the context) is drawn towards: into darkness in the dark scheme,
    /// towards the canvas in the light one. Muting a ghost by darkening works only where the background
    /// is dark
    pub ghost_target: [u8; 3],

    // --- the view cube ---
    /// a face of the view cube at its brightest turn (further away means proportionally darker)
    pub viewcube_face: [u8; 3],
    /// the edges of the view cube and the background of the home button
    pub viewcube_edge: [u8; 3],

    // --- machining ---
    /// plunging
    pub cam_plunge: [u8; 3],
    /// a rapid move
    pub cam_rapid: [u8; 3],
    /// the machine table
    pub cam_table: [u8; 3],
    /// the markings on the table
    pub cam_table_grid: [u8; 3],
    /// the stock
    pub cam_stock: [u8; 3],
    /// the stock, not selected
    pub cam_stock_idle: [u8; 3],
    /// the toolpaths of operations: the colour TELLS the operations apart, hence six of them, none close
    /// to another
    pub cam_op1: [u8; 3],
    pub cam_op2: [u8; 3],
    pub cam_op3: [u8; 3],
    pub cam_op4: [u8; 3],
    pub cam_op5: [u8; 3],
    pub cam_op6: [u8; 3],

    // --- the interface itself (in effect only when `ui_on`) ---
    //
    // WHY SEPARATE NAMES RATHER THAN `panel_bg`/`text_dim`. Those describe A CARD OVER THE CANVAS - the
    // modal entry of a dimension - and that is a different role: today the colours would coincide, in
    // another scheme they will part ways, and there would be nowhere to call a panel background "the
    // background of a card over the canvas" from. The rule of this module: the name decides the role, not
    // the value.
    /// the background of the program's windows and panels
    pub ui_window: [u8; 3],
    /// the background of an input field (the "deepest" surface)
    pub ui_field: [u8; 3],
    /// the backing of an alternating row in a table
    pub ui_stripe: [u8; 3],
    /// the outlines of windows and widgets
    pub ui_outline: [u8; 3],
    /// the background of a button at rest
    pub ui_control: [u8; 3],
    /// the same under the cursor
    pub ui_control_hover: [u8; 3],
    /// the same pressed or expanded
    pub ui_control_active: [u8; 3],
    /// what is selected: a row of a list, a selection inside a field
    pub ui_accent: [u8; 3],
    /// the main text of the interface
    pub ui_text: [u8; 3],
    /// muted text: captions, explanations
    pub ui_text_dim: [u8; 3],
    /// text under the cursor and on something pressed
    pub ui_text_strong: [u8; 3],
    /// a link
    pub ui_link: [u8; 3],
}

impl Default for Palette {
    fn default() -> Self {
        dark()
    }
}

/// A colour at a given transparency. The alpha is a decision of THE PLACE that draws (a fill against a
/// stroke), not of the scheme: one and the same `preview` goes both as a solid line and as the translucent
/// patch beneath it.
pub fn a(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

/// How lit a face is: `t` in [0, 1] is how far the face is turned towards the light, and `floor` is how
/// much light the scheme leaves to the darkest face. One formula for every painter: the floor plus what
/// remains.
pub fn lit(floor: f32, t: f32) -> f32 {
    floor + t.clamp(0.0, 1.0) * (1.0 - floor)
}

/// MAKE A COLOUR BRIGHTER, NOT WHITER.
///
/// The first attempt mixed in white - and the result was named at once: the parts had merely become whiter
/// rather than lighter or brighter. That is right: mixing in white means knocking down the saturation, and
/// the colours of parts are muted as it is (steel sits at 0.24). What came out was whitewash.
///
/// Here the LIGHTNESS is raised with the hue preserved, while the saturation is RAISED as well - otherwise
/// it would sag by itself: the lighter a colour, the closer to white it is by construction.
///
/// At zero amounts the colour is returned BYTE FOR BYTE, without a round trip through HSL: the dark scheme
/// lightens nothing, and rounding there and back has no right to shift it by a single unit.
pub fn brighten(c: [u8; 3], lighten: f32, saturate: f32) -> [u8; 3] {
    if lighten <= 0.0 && saturate <= 0.0 {
        return c;
    }
    let (h, s, l) = to_hsl(c);
    from_hsl(h, (s * (1.0 + saturate)).min(1.0), l + (1.0 - l) * lighten)
}

/// A colour multiplied by the lighting `k` in [0, 1] - the shading of a face by depth.
pub fn tint(c: Color32, k: f32) -> Color32 {
    let m = |v: u8| (v as f32 * k).clamp(0.0, 255.0) as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// The readers. A separate method per colour rather than `get("sketch.line")`: a typo in a string would
/// be found by the eyes of whoever uses the program, while a typo in a method name does not compile.
macro_rules! readers {
    ($($f:ident),* $(,)?) => {
        // COLOURS WITH NO READER. Some entries of the scheme (`axis_x/y/z`, `axis_pick_idle`,
        // `ghost_target`, `cam_op1..6`) are taken by no code at all: they exist in the scheme and in its
        // window but have no effect on screen. That is A FINDING, not a formality: a person edits a colour
        // and nothing changes. They cannot be deleted (that would break schemes already on disk), so the
        // denial is lifted here in one spot and the trouble itself is written down - it is mended either by
        // wiring them up or by cleaning the scheme out.
        #[allow(dead_code)]
        impl Palette {
            $(pub fn $f(&self) -> Color32 { rgb(self.$f) })*

            /// EVERY COLOUR AS A LIST - for the settings, for saving a scheme of one's own, and for the
            /// guard tests. The order matches the order in which the fields are declared.
            pub fn entries(&self) -> Vec<(&'static str, [u8; 3])> {
                vec![$((stringify!($f), self.$f)),*]
            }

            /// Replace one colour by name (the editor of a custom scheme). `false` means no such name.
            pub fn set(&mut self, key: &str, v: [u8; 3]) -> bool {
                match key {
                    $(stringify!($f) => { self.$f = v; true })*
                    _ => false,
                }
            }
        }
    };
}

readers!(
    viewport_bg,
    toolbar_bg,
    panel_bg,
    panel_border,
    splash_bg,
    thumbnail_bg,
    scrim,
    text_strong,
    text_dim,
    text_faint,
    emphasis,
    glyph_text,
    glyph_backing,
    grid,
    grid_minor,
    axis_x,
    axis_y,
    axis_z,
    grid_axis_x,
    grid_axis_y,
    grid_axis_z,
    sketch_axis_x,
    sketch_axis_y,
    sketch_axis_idle,
    sketch_line,
    sketch_construction,
    sketch_driven,
    sketch_face_edge,
    sketch_edge_3d,
    dimension,
    dimension_driven,
    dim_helper,
    dim_helper_ring,
    selected,
    highlight,
    active,
    handle,
    handle_face,
    snap_point,
    axis_pick_idle,
    clip,
    plate_text,
    pattern_center,
    sketch_point,
    annotation,
    note,
    hint,
    hint_action,
    tree_selected,
    connector,
    rollback,
    select_window,
    select_cross,
    rubber_band,
    snap_marker,
    snap_intersection,
    snap_edge,
    snap_axis,
    snap_grid,
    edge_idle,
    body_face,
    body_ghost,
    body_clash,
    add,
    remove,
    modify,
    reference,
    offset_in,
    plane_face,
    plane_fill,
    plane_idle,
    plane_normal,
    datum_point,
    datum_axis,
    preview,
    preview_prim,
    preview_array,
    preview_axis,
    preview_datum,
    measure,
    cut_line,
    gizmo_label,
    constraint_ok,
    constraint_selected,
    constraint_hover,
    joint_idle,
    joint_hover,
    joint_pick_a,
    joint_pick_b,
    grounded,
    contour_idle,
    contour_hover,
    contour_profile,
    ok,
    ok_soft,
    underdefined,
    warning,
    error,
    error_mild,
    cam_plunge,
    cam_rapid,
    cam_table,
    cam_table_grid,
    cam_stock,
    ghost_target,
    viewcube_face,
    viewcube_edge,
    cam_stock_idle,
    cam_op1,
    cam_op2,
    cam_op3,
    cam_op4,
    cam_op5,
    cam_op6,
    ui_window,
    ui_field,
    ui_stripe,
    ui_outline,
    ui_control,
    ui_control_hover,
    ui_control_active,
    ui_accent,
    ui_text,
    ui_text_dim,
    ui_text_strong,
    ui_link,
);

impl Palette {
    /// The toolpath colour by operation number. Operations are told apart by colour, so the list wraps
    /// around: the seventh operation repeats the first rather than becoming invisible.
    pub fn cam_op(&self, i: usize) -> Color32 {
        rgb(match i % 6 {
            1 => self.cam_op2,
            2 => self.cam_op3,
            3 => self.cam_op4,
            4 => self.cam_op5,
            5 => self.cam_op6,
            _ => self.cam_op1,
        })
    }

    /// AN IMPRINT OF THE SCHEME - for the keys of the image caches.
    ///
    /// The raster of the viewport and the GPU vertex buffer are computed once and reused while their key
    /// stays the same. The colour of the bodies goes into those buffers - so the scheme must go into the
    /// key, otherwise switching the theme leaves the old picture on screen until the next edit to the
    /// geometry. It is computed over ALL the fields rather than over the name of the scheme: the editor of
    /// a custom scheme changes colours without changing the name.
    pub fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut h);
        self.name.hash(&mut h);
        self.light.hash(&mut h);
        for (_, v) in self.entries() {
            v.hash(&mut h);
        }
        for f in [self.shade_floor_body, self.shade_floor_mesh, self.shade_floor_viewcube, self.body_lighten, self.body_saturate] {
            f.to_bits().hash(&mut h);
        }
        h.finish()
    }

    /// The caption of a scheme in the person's language.
    pub fn title(&self) -> String {
        if self.name.is_empty() { crate::i18n::tr(&format!("scheme-{}", self.id)) } else { self.name.clone() }
    }

    /// A gizmo axis by index (0/1/2) - the gizmo walks the axes in a loop rather than by name.
    pub fn axis(&self, i: usize) -> Color32 {
        rgb(match i {
            1 => self.axis_y,
            2 => self.axis_z,
            _ => self.axis_x,
        })
    }
}

/// THE DARK SCHEME - WHAT WAS THERE, LITERAL FOR LITERAL.
///
/// Not one decision of taste: the values are copied out of the code as they were. The present look is
/// liked, and any trifle "tidied up while we are at it" would be a rework rather than a transcription -
/// and no rework was asked for. The comment on each line names the place the colour came from, so that the
/// transcription can be checked.
pub fn dark() -> Palette {
    Palette {
        id: "dark".into(),
        name: String::new(),
        light: false,
        ui_on: false, // the interface stays stock: a transcription, not a rework

        viewport_bg: [26, 26, 26],     // gui.rs: from_gray(26)
        toolbar_bg: [34, 40, 46],      // gui.rs: tool_bar_frame
        panel_bg: [28, 28, 28],        // draw_dim_overlay: from_gray(28)
        panel_border: [60, 60, 60],    // draw_dim_overlay: from_gray(60)
        splash_bg: [22, 22, 22],       // draw_splash
        thumbnail_bg: [38, 42, 48],    // render_component_thumbnail
        scrim: [0, 0, 0],              // from_black_alpha(120)
        text_strong: [230, 230, 230],  // draw_splash / draw_dim_overlay (225 merged in here)
        text_dim: [170, 170, 170],     // draw_splash
        text_faint: [135, 135, 135],   // a reference dimension
        emphasis: [255, 255, 255],     // a dimension under the cursor
        glyph_text: [255, 255, 255],   // the mark inside a constraint plate
        glyph_backing: [20, 26, 34],   // the disc behind a joint glyph
        grid: [70, 74, 80],            // the canvas grid (66,72,82 of the major 3D grid merged in here)
        grid_minor: [46, 50, 58],      // a minor line of the 3D grid

        axis_x: [230, 90, 90],       // draw_gizmo_at
        axis_y: [90, 200, 110],      //
        axis_z: [90, 150, 240],      //
        grid_axis_x: [205, 85, 85],  // draw_3d, the world grid
        grid_axis_y: [90, 205, 90],  //
        grid_axis_z: [95, 135, 235], //
        sketch_axis_x: [180, 80, 80], // draw_axes
        sketch_axis_y: [90, 165, 95], //
        sketch_axis_idle: [60, 60, 60], // draw_axes: from_gray(60)

        sketch_line: [250, 230, 120],         // draw_sketch_preview
        sketch_construction: [120, 170, 230], // also the helper leader of a dimension
        sketch_driven: [150, 210, 255],       // a live projection
        sketch_face_edge: [150, 170, 210],    // draw_sketch_face_edges
        sketch_edge_3d: [150, 180, 235],      // a sketch shown in 3D

        dimension: [110, 200, 230],
        dimension_driven: [135, 135, 135],
        dim_helper: [150, 210, 120],
        dim_helper_ring: [40, 70, 30],

        selected: [255, 170, 60],  // an edge, dimension or face is selected (250,170,60 merged in here)
        highlight: [250, 210, 110], // under the cursor, a selected datum, the copies of an array
        active: [250, 200, 90],    // "being dragged now" (255,200,90 / 255,200,80 / 240,200,90 merged)
        handle: [150, 225, 255],   // the arrow handle of a command
        handle_face: [200, 210, 230], // an arrow on a face
        snap_point: [80, 220, 120],
        axis_pick_idle: [150, 190, 150],
        clip: [120, 200, 120],
        plate_text: [20, 22, 26],
        pattern_center: [255, 140, 60],
        sketch_point: [120, 220, 250],
        annotation: [190, 205, 230],
        note: [200, 180, 120],       // (200,190,130 merged in here)
        hint: [120, 200, 255],
        hint_action: [230, 170, 90], // (230,160,90 and 230,180,90 merged in here)
        tree_selected: [120, 200, 255],
        connector: [230, 120, 240],
        rollback: [220, 135, 45],
        select_window: [110, 180, 255],
        select_cross: [110, 220, 130],
        rubber_band: [120, 120, 120],
        snap_marker: [255, 220, 90],
        snap_intersection: [120, 230, 250],
        snap_edge: [150, 220, 150],
        snap_axis: [230, 120, 230],
        snap_grid: [150, 160, 180],
        edge_idle: [150, 155, 170], // an edge of a body on its own

        body_face: [97, 195, 214],  // draw_mesh: (g/2, g, g*1.1) on the lightest face
        body_ghost: [58, 78, 89],   // (g*0.30, g*0.40, g*0.46)
        body_clash: [214, 68, 68],  // (g*1.1, g*0.35, g*0.35)

        add: [120, 220, 160],       // a pad
        remove: [240, 110, 100],    // a shell
        modify: [250, 170, 90],     // a draft, a cut (255,170,90 merged in here)
        reference: [90, 160, 250],  // the neutral face of a draft
        offset_in: [240, 140, 120], // an offset inwards

        plane_face: [110, 185, 255], // a plane available to pick (110,190,255 merged in here)
        plane_fill: [90, 170, 255],
        plane_idle: [110, 130, 110],
        plane_normal: [90, 130, 230],
        datum_point: [180, 200, 230],
        datum_axis: [200, 180, 120],

        preview: [120, 200, 255],       // (130,205,255 / 120,210,255 merged in here)
        preview_prim: [90, 210, 230],
        preview_array: [120, 210, 235],
        preview_axis: [180, 160, 250],
        preview_datum: [150, 200, 255],
        measure: [120, 220, 160], // green, both in 3D and in a sketch
        cut_line: [255, 210, 120],
        gizmo_label: [245, 230, 150],

        constraint_ok: [46, 150, 74],
        constraint_selected: [220, 140, 40],
        constraint_hover: [70, 150, 220],

        joint_idle: [120, 170, 235],
        joint_hover: [150, 220, 255],
        joint_pick_a: [110, 230, 130],
        joint_pick_b: [110, 180, 250],
        grounded: [150, 210, 140],

        contour_idle: [105, 140, 200],
        contour_hover: [220, 230, 255],
        contour_profile: [255, 190, 70],

        ok: [120, 220, 140],  // (120,230,140 merged in here)
        ok_soft: [150, 190, 150],
        underdefined: [250, 210, 100], // (230,200,90 merged in here)
        warning: [210, 160, 40], // a redundant constraint
        error: [255, 80, 70],    // a conflict (240,90,80 and 255,90,80 merged in here)
        error_mild: [230, 120, 120], // (240,130,120 and 230,130,110 merged in here)

        cam_plunge: [225, 95, 95],
        cam_rapid: [115, 115, 115], // from_gray(110) and from_gray(120) merged
        cam_table: [70, 90, 70],
        cam_table_grid: [60, 75, 60],
        cam_stock: [224, 168, 92],   // (230,180,90 of the selected stock merged in here)
        shade_floor_body: 0.40,      // gui.rs shade_tri: lit = 0.4 + 0.6*|n.light|
        shade_floor_mesh: 45.0 / 195.0, // render.rs draw_mesh: g = 45 + shade*150
        shade_floor_viewcube: 66.0 / 235.0, // viewcube.rs: the darkest corner
        body_lighten: 0.0,           // the dark scheme does not touch a part - this is a transcription
        body_saturate: 0.0,
        ghost_target: [29, 32, 40],  // the former addend (22,24,30) is 0.75 of it
        viewcube_face: [235, 235, 249], // at full lighting; the cube is drawn by shading down from it
        viewcube_edge: [60, 66, 74],
        cam_stock_idle: [90, 95, 110],
        cam_op1: [90, 200, 150],
        cam_op2: [120, 170, 240],
        cam_op3: [230, 180, 80],
        cam_op4: [200, 120, 220],
        cam_op5: [120, 220, 220],
        cam_op6: [220, 140, 110],

        // THE INTERFACE - THE NUMBERS OF `egui` ITSELF (`Visuals::dark()`), copied out here.
        //
        // The dark scheme has `ui_on` off and the program never asks for them: the look of the interface
        // stays stock. But a copy of this scheme with the box ticked must start from EXACTLY what was on
        // screen - otherwise "paint the interface" would mean "repaint it at random". A guard checks these
        // twelve against `Visuals::dark()` and goes red if egui changes them.
        ui_window: [27, 27, 27],          // panel_fill / window_fill
        ui_field: [10, 10, 10],           // extreme_bg_color
        ui_stripe: [5, 5, 5],             // faint_bg_color
        ui_outline: [60, 60, 60],         // window_stroke
        ui_control: [60, 60, 60],         // widgets.inactive.bg_fill
        ui_control_hover: [70, 70, 70],   // widgets.hovered.bg_fill
        ui_control_active: [55, 55, 55],  // widgets.active.bg_fill
        ui_accent: [0, 92, 128],          // selection.bg_fill
        ui_text: [180, 180, 180],         // widgets.inactive.fg_stroke
        ui_text_dim: [140, 140, 140],     // widgets.noninteractive.fg_stroke
        ui_text_strong: [240, 240, 240],  // widgets.hovered.fg_stroke
        ui_link: [90, 170, 255],          // hyperlink_color
    }
}

/// THE LIGHT SCHEME - DERIVED BY A RULE rather than retyped one colour at a time.
///
/// A hand-typed list of seventy values would go stale on the very first new tool: the colour is added to
/// the dark scheme, the light one is forgotten, and it shows a dark blot again. So the light scheme is
/// DERIVED from the dark one by [`toward_light`], and only the places where the rule is blind are set by
/// hand: the backgrounds (the rule does not know how light the canvas should be) and the mark on a
/// coloured plate (it must stay white - the plate beneath it is saturated in any scheme).
pub fn light() -> Palette {
    let d = dark();
    let mut p = Palette { id: "light".into(), name: String::new(), light: true, ..d.clone() };
    for (key, v) in d.entries() {
        p.set(key, toward_light(v));
    }

    // The backgrounds: the rule can only mirror lightness, while how light the canvas should be is a
    // decision about the look. Not pure white: on white the light faces of the model would merge with the
    // void.
    p.viewport_bg = [238, 240, 243];
    p.toolbar_bg = [222, 226, 231];
    p.panel_bg = [246, 247, 249];
    p.splash_bg = [242, 244, 247];
    p.thumbnail_bg = [232, 235, 239];
    p.panel_border = [196, 200, 207];
    p.scrim = [0, 0, 0]; // the dimming under a modal window is dark in the light scheme too

    // The mark inside a coloured plate: the plate is saturated in any scheme, so the mark stays white.
    // By the same reasoning the other way round: text ON a bright plate stays dark.
    p.glyph_text = [255, 255, 255];
    p.plate_text = [20, 22, 26];
    // The disc behind a glyph must be the OPPOSITE OF IT. The rule drove it downwards (the dark blue disc
    // has a saturation of 0.26 - the rule counts it as coloured and takes it into the dark), and the glyph,
    // which the same rule had made dark, landed on dark: the difference in brightness came out at -8.
    p.glyph_backing = [252, 252, 254];

    // A body is read by its shading rather than by contrast with the background, so the rule (which pulls
    // everything towards dark-on-white) would have made the model dirty. The same hue is taken, slightly
    // muted.
    p.body_face = [176, 208, 228];
    p.body_ghost = [150, 160, 168];
    p.body_clash = [200, 95, 95];

    // The view cube is read by the shading of its faces, like a body: the rule would have made it a black cube.
    p.viewcube_face = [250, 251, 254];
    p.viewcube_edge = [140, 147, 158];

    // THE DEPTH OF SHADING ON A LIGHT CANVAS IS SHALLOW. Shading down to a quarter of the brightness reads
    // on a dark background and turns a part into a silhouette on a light one. The form is not lost by this:
    // 30 % of range is enough for the faces to differ from one another while the part stays lighter than
    // the canvas. The part is first made BRIGHTER (lighter and more saturated, not whiter) and only then
    // shaded. Otherwise its ceiling stays below the canvas, and mixed-in white turns the colour into
    // whitewash.
    p.body_lighten = 0.35;
    p.body_saturate = 0.90;
    p.shade_floor_body = 0.78;
    p.shade_floor_mesh = 0.80;
    p.shade_floor_viewcube = 0.86;
    // on a light canvas a ghost is drawn TOWARDS THE CANVAS rather than into darkness - otherwise the
    // "inactive" part turns out to be the most conspicuous thing on screen
    p.ghost_target = [222, 226, 232];

    // THE INTERFACE - THE NUMBERS OF `Visuals::light()`, for the same reason as in the dark scheme: the
    // `toward_light` rule does not fit here, it converts the colours of THE CANVAS, not the surfaces of the
    // windows themselves.
    p.ui_window = [248, 248, 248]; // panel_fill / window_fill
    p.ui_field = [255, 255, 255]; // extreme_bg_color
    p.ui_stripe = [5, 5, 5]; // faint_bg_color
    p.ui_outline = [190, 190, 190]; // window_stroke
    p.ui_control = [230, 230, 230]; // widgets.inactive.bg_fill
    p.ui_control_hover = [220, 220, 220]; // widgets.hovered.bg_fill
    p.ui_control_active = [165, 165, 165]; // widgets.active.bg_fill
    p.ui_accent = [144, 209, 255]; // selection.bg_fill
    p.ui_text = [60, 60, 60]; // widgets.inactive.fg_stroke
    p.ui_text_dim = [80, 80, 80]; // widgets.noninteractive.fg_stroke
    p.ui_text_strong = [0, 0, 0]; // widgets.hovered.fg_stroke
    p.ui_link = [0, 155, 255]; // hyperlink_color

    p
}

/// DRACULA - THE FIRST SCHEME THAT PAINTS THE WHOLE PROGRAM.
///
/// It was asked for by name, and for its sake a scheme learned to paint not only the canvas but the
/// interface itself (`ui_on`).
///
/// The colours are CANONICAL, from the specification of the theme, not merely "similar". That is the whole
/// point of a well-known theme: it is recognised. Taking liberties here would produce "something purple"
/// rather than Dracula.
///
/// Everything else is taken from the dark scheme: the roles the canon says nothing about (the six toolpath
/// colours, the shades of the machine) need not be invented again - they are dark as they are.
pub fn dracula() -> Palette {
    let mut p = Palette { id: "dracula".into(), name: String::new(), light: false, ui_on: true, ..dark() };

    // backgrounds and text
    p.viewport_bg = [33, 34, 44]; // darker than the panels: the canvas reads as a hole in the window
    p.toolbar_bg = [68, 71, 90]; // current line
    p.panel_bg = [40, 42, 54]; // background
    p.panel_border = [68, 71, 90];
    p.splash_bg = [33, 34, 44];
    p.thumbnail_bg = [45, 47, 60];
    p.text_strong = [248, 248, 242]; // foreground
    p.text_dim = [154, 160, 191];
    p.text_faint = [98, 114, 164]; // comment
    p.emphasis = [255, 255, 255];

    // grid and axes
    p.grid = [68, 71, 90];
    p.grid_minor = [52, 54, 70];
    p.axis_x = [255, 85, 85]; // red
    p.axis_y = [80, 250, 123]; // green
    p.axis_z = [139, 233, 253]; // cyan
    p.grid_axis_x = [210, 80, 80];
    p.grid_axis_y = [70, 210, 105];
    p.grid_axis_z = [120, 190, 220];
    p.sketch_axis_x = [190, 70, 70];
    p.sketch_axis_y = [70, 190, 95];
    p.sketch_axis_idle = [80, 84, 105];

    // sketch, dimensions, selection
    p.sketch_line = [241, 250, 140]; // yellow
    p.sketch_construction = [98, 114, 164];
    p.sketch_driven = [189, 147, 249]; // purple
    p.dimension = [139, 233, 253];
    p.dimension_driven = [98, 114, 164];
    p.selected = [255, 121, 198]; // pink
    p.highlight = [255, 184, 108]; // orange
    p.active = [80, 250, 123];
    p.preview = [189, 147, 249];
    p.hint = [154, 160, 191];
    p.hint_action = [255, 184, 108];
    p.tree_selected = [139, 233, 253];

    // states
    p.ok = [80, 250, 123];
    p.ok_soft = [130, 200, 150];
    p.underdefined = [241, 250, 140];
    p.warning = [255, 184, 108];
    p.error = [255, 85, 85];
    p.error_mild = [235, 130, 130];

    p.ghost_target = [40, 42, 54]; // a ghost is drawn TOWARDS THE WINDOW background, not into some other darkness
    p.viewcube_edge = [68, 71, 90];

    // the interface itself
    p.ui_window = [40, 42, 54];
    p.ui_field = [33, 34, 44];
    p.ui_stripe = [45, 47, 60];
    p.ui_outline = [68, 71, 90];
    p.ui_control = [68, 71, 90];
    p.ui_control_hover = [86, 91, 120];
    p.ui_control_active = [98, 114, 164];
    // THE SELECTION IS A DEEP PURPLE rather than the canonical #bd93f9: on pale lilac the foreground text
    // does not read at all (a contrast of 1.9). A row of a list must stay readable - that is about the work,
    // not about fidelity to a palette.
    p.ui_accent = [125, 95, 196];
    p.ui_text = [248, 248, 242];
    p.ui_text_dim = [154, 160, 191];
    p.ui_text_strong = [255, 255, 255];
    p.ui_link = [139, 233, 253];
    p
}

/// ALUCARD - the light twin of Dracula, from the same specification.
///
/// It is derived from the light scheme for the same reason the light one is derived from the dark: what
/// the canon says nothing about must stay readable on a light canvas rather than be invented again.
pub fn alucard() -> Palette {
    let mut p = Palette { id: "alucard".into(), name: String::new(), light: true, ui_on: true, ..light() };

    p.viewport_bg = [247, 242, 223]; // the canvas slightly darker than the panels - as in Dracula, but reversed
    p.toolbar_bg = [240, 235, 214];
    p.panel_bg = [255, 251, 235]; // background
    p.panel_border = [214, 208, 184];
    p.splash_bg = [255, 251, 235];
    p.thumbnail_bg = [245, 240, 222];
    p.text_strong = [31, 31, 31]; // foreground
    p.text_dim = [108, 102, 75]; // comment
    p.text_faint = [140, 134, 110];
    p.emphasis = [0, 0, 0];

    p.grid = [214, 208, 184];
    p.grid_minor = [232, 227, 205];
    p.axis_x = [203, 58, 42]; // red
    p.axis_y = [20, 113, 10]; // green
    p.axis_z = [3, 106, 150]; // cyan
    p.grid_axis_x = [190, 90, 80];
    p.grid_axis_y = [60, 130, 60];
    p.grid_axis_z = [50, 110, 160];
    p.sketch_axis_x = [170, 60, 50];
    p.sketch_axis_y = [40, 120, 40];
    p.sketch_axis_idle = [190, 184, 160];

    p.sketch_line = [132, 110, 21]; // yellow
    p.sketch_construction = [108, 102, 75];
    p.sketch_driven = [100, 74, 201]; // purple
    p.dimension = [3, 106, 150];
    p.dimension_driven = [108, 102, 75];
    p.selected = [163, 20, 77]; // pink
    p.highlight = [163, 77, 20]; // orange
    p.active = [20, 113, 10];
    p.preview = [100, 74, 201];
    p.hint = [108, 102, 75];
    p.hint_action = [163, 77, 20];
    p.tree_selected = [3, 106, 150];

    p.ok = [20, 113, 10];
    p.ok_soft = [90, 130, 85];
    p.underdefined = [132, 110, 21];
    p.warning = [163, 77, 20];
    p.error = [203, 58, 42];
    p.error_mild = [180, 90, 80];

    p.ghost_target = [240, 236, 218];
    p.viewcube_edge = [160, 154, 132];

    // THE PANEL BACKGROUND IS SLIGHTLY DEEPER THAN THE CANON (#fffbeb): an input field must read as a
    // well, and against an almost white background a white field does not differ from it at all - a guard
    // caught a difference of 2 units.
    p.ui_window = [251, 246, 224];
    p.ui_field = [255, 253, 245];
    p.ui_stripe = [245, 240, 220];
    p.ui_outline = [214, 208, 184];
    p.ui_control = [240, 235, 214];
    p.ui_control_hover = [229, 223, 198];
    p.ui_control_active = [207, 207, 222];
    // and here the other way round: the selection is LIGHT, because the text on it is dark
    p.ui_accent = [203, 189, 242];
    p.ui_text = [31, 31, 31];
    p.ui_text_dim = [108, 102, 75];
    p.ui_text_strong = [0, 0, 0];
    p.ui_link = [3, 106, 150];
    p
}

/// THE LOOK OF `egui` ITSELF, TAKEN FROM THE SCHEME.
///
/// With `ui_on` off, the stock look is handed back as it is. That is not a stub but a promise: the dark
/// and light schemes must look EXACTLY as they looked before this code existed. The present look was said
/// to be liked; repainting it as a side effect of a new capability would be a rework.
///
/// With it on, the twelve colours of the scheme are laid out over the places `egui` has. LINE WIDTHS AND
/// ROUNDING ARE LEFT ALONE: a scheme is about colour, not about shape. The scheme does not describe every
/// `egui` field by name (there are four times as many): some are derived - for instance, the text on a
/// pressed button is taken to be the same as the text under the cursor. Nobody would have tuned twenty
/// knobs in the scheme editor.
pub fn visuals(p: &Palette) -> egui::Visuals {
    let mut v = if p.light { egui::Visuals::light() } else { egui::Visuals::dark() };
    if !p.ui_on {
        return v;
    }
    let (window, outline) = (rgb(p.ui_window), rgb(p.ui_outline));
    let (text, dim, strong) = (rgb(p.ui_text), rgb(p.ui_text_dim), rgb(p.ui_text_strong));
    v.panel_fill = window;
    v.window_fill = window;
    v.extreme_bg_color = rgb(p.ui_field);
    v.code_bg_color = rgb(p.ui_field);
    v.faint_bg_color = rgb(p.ui_stripe);
    v.window_stroke.color = outline;
    v.hyperlink_color = rgb(p.ui_link);
    v.selection.bg_fill = rgb(p.ui_accent);
    v.selection.stroke.color = strong;
    // the scheme already has a warning and an error colour - there is no point naming them twice
    v.warn_fg_color = rgb(p.warning);
    v.error_fg_color = rgb(p.error);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = window;
    w.noninteractive.weak_bg_fill = window;
    w.noninteractive.bg_stroke.color = outline;
    w.noninteractive.fg_stroke.color = dim;
    w.inactive.bg_fill = rgb(p.ui_control);
    w.inactive.weak_bg_fill = rgb(p.ui_control);
    w.inactive.fg_stroke.color = text;
    w.hovered.bg_fill = rgb(p.ui_control_hover);
    w.hovered.weak_bg_fill = rgb(p.ui_control_hover);
    w.hovered.bg_stroke.color = outline;
    w.hovered.fg_stroke.color = strong;
    w.active.bg_fill = rgb(p.ui_control_active);
    w.active.weak_bg_fill = rgb(p.ui_control_active);
    w.active.bg_stroke.color = rgb(p.ui_accent);
    w.active.fg_stroke.color = strong;
    w.open.bg_fill = rgb(p.ui_control_active);
    w.open.weak_bg_fill = rgb(p.ui_control_active);
    w.open.bg_stroke.color = outline;
    w.open.fg_stroke.color = strong;
    v
}

/// CONVERTING A COLOUR TO A LIGHT BACKGROUND. Not an inversion: inversion breaks the hue (yellow would
/// turn blue) and makes a scheme a stranger to itself.
///
/// - **Neutral** colours (the greys) are mirrored in lightness: dark grey text on light becomes light grey
///   on dark and the other way round. That is exactly what is expected of them.
/// - **Coloured** ones keep their hue entirely, gain a little saturation (a colour looks paler on white
///   than on black), and have their lightness squeezed into a narrow dark band: any colour derived by this
///   rule reads on a light background, and the differences between them are preserved.
///
/// The greys are not merely mirrored but mirrored WITH COMPRESSION into the band
/// [`GREY_FLOOR`]..[`GREY_CEIL`]. A plain mirror would send near-black to near-white, and the canvas of
/// the light scheme is itself near-white - the colour would disappear. Clamping at the ceiling is not
/// allowed either: the two darkest greys would stick together and the main text would stop differing from
/// the secondary one. Compression preserves the ordering whole. Both traps were found by tests, not by
/// eyes.
pub fn toward_light(c: [u8; 3]) -> [u8; 3] {
    let (h, s, l) = to_hsl(c);
    if s < 0.10 {
        from_hsl(h, s, GREY_FLOOR + (1.0 - l) * (GREY_CEIL - GREY_FLOOR))
    } else {
        from_hsl(h, (s * 1.15).min(1.0), 0.30 + l * 0.18)
    }
}

/// The band of lightness for the greys in the light scheme. The ceiling stays far enough below the
/// lightness of the canvas for the difference to read; the floor keeps "the darkest" from turning into a
/// hole of ink.
const GREY_FLOOR: f32 = 0.06;
const GREY_CEIL: f32 = 0.74;

fn to_hsl(c: [u8; 3]) -> (f32, f32, f32) {
    let (r, g, b) = (c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn from_hsl(h: f32, s: f32, l: f32) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hp as u8 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let q = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    [q(r), q(g), q(b)]
}

/// THE BUILT-IN SCHEMES. The list is the source for the settings; custom ones are added as files (see
/// [`store`]).
pub fn builtin() -> Vec<Palette> {
    vec![dark(), light(), dracula(), alucard()]
}

/// EVERY SCHEME: the built-in ones plus the custom ones from disk. The second value carries the
/// complaints about broken files, which are shown in the status line: a scheme has no right to vanish
/// silently, or it will be hunted for by eye.
pub fn all() -> (Vec<Palette>, Vec<String>) {
    let mut out = builtin();
    let (mine, errs) = store::load_all();
    out.extend(mine);
    (out, errs)
}

mod labels;
pub mod store;
pub use labels::groups;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod store_tests;
