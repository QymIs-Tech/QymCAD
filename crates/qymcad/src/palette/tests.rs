//! A SCHEME AS DATA — the checks that keep it honest.
//!
//! The main thing here is not "the light one is lighter than the dark one" (that is plain anyway) but two
//! guards:
//!
//! 1. **The census is complete.** The list of readers is written by hand, and forgetting a field in it is
//!    easy: the code compiles, and the colour simply drops out of the settings and out of every check
//!    below — quietly. The test compares the list against the source itself.
//! 2. **Every colour is legible on its own background.** Not six selected ones but ALL of them —
//!    otherwise the check defends exactly what somebody once remembered to list.
use super::*;

/// The luminance — light is told from dark by it, without being tied to exact numbers.
fn luma(c: [u8; 3]) -> f32 {
    0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32
}

/// A translucent colour over a background — what is really seen on screen.
fn blend(c: [u8; 3], bg: [u8; 3], alpha: u8) -> [u8; 3] {
    let k = alpha as f32 / 255.0;
    let m = |i: usize| (c[i] as f32 * k + bg[i] as f32 * (1.0 - k)).round() as u8;
    [m(0), m(1), m(2)]
}

/// The shading of a face — the same operation the renderer performs.
fn shaded(c: [u8; 3], k: f32) -> [u8; 3] {
    let t = tint(Color32::from_rgb(c[0], c[1], c[2]), k);
    [t.r(), t.g(), t.b()]
}

/// THE CENSUS OF COLOURS IS COMPLETE.
///
/// `entries()` lists the fields by hand (through a macro), and that is the only way for the settings, the
/// editor of one's own scheme and the tests below to walk every colour. A forgotten field would not break
/// the build — it would simply drop out of everything, silently. So the list is checked against the source
/// of the struct.
#[test]
fn every_colour_field_is_listed_in_entries() {
    let src = include_str!("../palette.rs");
    let decl = src
        .split("impl Default for Palette")
        .next()
        .expect("the declaration of the struct comes before impl Default");
    let declared: Vec<&str> = decl
        .lines()
        .filter_map(|l| l.trim().strip_suffix(": [u8; 3],"))
        .filter_map(|l| l.strip_prefix("pub "))
        .collect();
    let listed: Vec<&str> = dark().entries().into_iter().map(|(k, _)| k).collect();

    for f in &declared {
        assert!(listed.contains(f), "the field `{f}` is declared and did not get into entries!() — it will drop out of the settings and out of the tests");
    }
    assert_eq!(declared.len(), listed.len(), "{} colours are declared and {} are listed", declared.len(), listed.len());
}

/// THE MEASURING LINE IS ONE ACROSS THE WHOLE CAD. It used to be amber in 3D and green in a sketch; it
/// was brought to green on request. A guard against the discord returning: one tool has one colour.
#[test]
fn the_measure_line_has_a_single_colour_everywhere() {
    let d = dark();
    assert_eq!(d.measure, [120, 220, 160], "the measuring line is green — that was the decision");
    assert_ne!(d.measure, d.cut_line, "the cutting line is another tool and has a colour of its own");
    let src = include_str!("../gui/sketching.rs");
    assert!(src.contains("self.scheme.pal.measure()"), "a sketch measures in the same colour as 3D");
    assert!(!src.contains("measure_sketch"), "there is no separate colour of the measuring line for a sketch any more");
}

/// THE EDITOR SHOWS EVERY COLOUR AND NOT ONE SPARE.
///
/// The list of sections is the THIRD enumeration of the fields after the struct and the readers, and it is
/// written by hand. A forgotten field would silently not reach the editor (and could not be corrected by a
/// scheme of one's own), while a spare one would point at a colour that does not exist. It is checked in
/// both directions.
#[test]
fn the_editor_lists_every_colour_exactly_once() {
    let known: Vec<&str> = dark().entries().into_iter().map(|(k, _)| k).collect();
    let mut listed: Vec<&str> = Vec::new();
    for (section, rows) in groups() {
        assert!(!rows.is_empty(), "the section \"{section}\" is empty");
        for key in rows {
            assert!(known.contains(&key), "the section \"{section}\" holds a colour `{key}` the scheme does not have");
            assert!(!listed.contains(&key), "the colour `{key}` is shown in the editor twice");
            listed.push(key);
        }
    }
    for k in &known {
        assert!(listed.contains(k), "the colour `{k}` did not get into any section of the editor — it cannot be corrected by a scheme of one's own");
    }
}

/// EVERY COLOUR AND EVERY SECTION HAS A CAPTION IN EVERY LANGUAGE.
///
/// The screen of the scheme was first written with text typed straight into the code, at a time when the
/// language catalogue was already in place. That was spotted at once. The test closes not that case but
/// the class: a caption missing from some language would show up there as a key such as
/// `scheme-color-sketch_line`.
#[test]
fn every_colour_and_section_is_translated_in_every_language() {
    let prev = crate::i18n::language();
    let mut holes: Vec<String> = Vec::new();
    for (code, _name) in crate::i18n::available() {
        crate::i18n::set_language(&code);
        for (section, rows) in groups() {
            for (key, kind) in std::iter::once((format!("scheme-group-{section}"), "the section")).chain(rows.iter().map(|k| (format!("scheme-color-{k}"), "the colour"))) {
                let text = crate::i18n::tr(&key);
                if text == key || text.trim().is_empty() {
                    holes.push(format!("{code}: {kind} `{key}` has no translation"));
                }
            }
        }
    }
    crate::i18n::set_language(&prev);
    assert!(holes.is_empty(), "the screen of the scheme would show keys instead of words:\n{}", holes.join("\n"));
}

/// AND THE BUTTONS OF THE SCREEN COME FROM THE CATALOGUE TOO, along with the substitutions in the
/// messages.
#[test]
fn the_scheme_editor_buttons_are_translated_in_every_language() {
    const KEYS: &[&str] = &[
        "scheme-edit", "scheme-duplicate", "scheme-delete", "scheme-save", "scheme-name", "scheme-rename",
        "scheme-name-taken", "scheme-is-light", "scheme-shading",
        "scheme-shade-body", "scheme-shade-body-hint", "scheme-shade-mesh", "scheme-shade-mesh-hint",
        "scheme-shade-viewcube", "scheme-shade-viewcube-hint", "scheme-body-lighten", "scheme-body-lighten-hint",
        "scheme-body-saturate", "scheme-body-saturate-hint",
    ];
    let prev = crate::i18n::language();
    for (code, _) in crate::i18n::available() {
        crate::i18n::set_language(&code);
        for k in KEYS {
            let text = crate::i18n::tr(k);
            assert_ne!(&text, k, "language {code} has no string {k}");
            assert!(!text.trim().is_empty(), "an empty string {k} in language {code}");
        }
        // messages with substitutions: the value must be substituted rather than left as `{$...}`
        for (k, t) in [
            ("scheme-created", crate::i18n::tr2("scheme-created", "name", "X", "path", "/p")),
            ("scheme-saved", crate::i18n::tr1("scheme-saved", "path", "/p")),
            ("scheme-deleted", crate::i18n::tr1("scheme-deleted", "name", "X")),
            ("scheme-create-failed", crate::i18n::tr1("scheme-create-failed", "error", "e")),
            ("scheme-save-failed", crate::i18n::tr1("scheme-save-failed", "error", "e")),
            ("scheme-delete-failed", crate::i18n::tr1("scheme-delete-failed", "error", "e")),
            ("scheme-load-failed", crate::i18n::tr1("scheme-load-failed", "error", "e")),
        ] {
            assert_ne!(t, k, "language {code} has no string {k}");
            assert!(!t.contains("{$"), "in {code}/{k} the substitution was not filled in: {t}");
        }
    }
    crate::i18n::set_language(&prev);
}

/// A GUARD AGAINST A RETURN: no text typed straight into the code is left in the screen of the scheme.
#[test]
fn the_scheme_screen_takes_its_words_from_the_catalogue() {
    let panels = crate::gui::panels_source::PANELS;
    let from = panels.find("fn scheme_section").expect("the section of the scheme is in place");
    let to = panels.find("fn tools_window").expect("the next method");
    let screen = &panels[from..to];
    let cyrillic: Vec<&str> = screen
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .filter(|l| l.contains('"') && l.chars().any(|c| ('а'..='я').contains(&c) || ('А'..='Я').contains(&c)))
        .collect();
    assert!(cyrillic.is_empty(), "text typed straight into the code has appeared in the screen of the scheme again:\n{}", cyrillic.join("\n"));
}

/// THE DARK SCHEME IS A TRANSFER, NOT A REDESIGN.
///
/// The exact values of the places that were reported, plus a few reference ones: should anybody one day
/// "improve" a shade along with a refactoring, the test will call that a change rather than let it
/// through.
#[test]
fn the_dark_scheme_is_a_transfer_not_a_redesign() {
    let d = dark();
    assert!(!d.light);
    assert_eq!(d.viewport_bg, [26, 26, 26], "the background of the viewport was from_gray(26)");
    assert_eq!(d.toolbar_bg, [34, 40, 46], "the tool bar was from_rgb(34, 40, 46)");
    assert_eq!(d.sketch_line, [250, 230, 120], "the sketch line");
    assert_eq!(d.selected, [255, 170, 60], "the selection");
    assert_eq!(d.error, [255, 80, 70], "a conflict");
    assert_eq!(d.constraint_ok, [46, 150, 74], "a satisfied constraint");
}

/// EVERY COLOUR IS LEGIBLE ON ITS OWN BACKGROUND — all seventy, not a chosen handful.
///
/// The threshold differs by meaning: lines and text must be noticeably more contrasting than background
/// surfaces (the grid, the backings, the table of the machine), which are supposed to be quiet. The
/// exceptions are listed by name — they are backgrounds and auxiliary backings for which contrast with the
/// canvas is not wanted by design.
#[test]
fn every_colour_is_legible_on_its_own_background() {
    // the backgrounds, and what is drawn NOT on the canvas but on a backing of its own
    const GROUNDS: &[&str] = &[
        "viewport_bg", "toolbar_bg", "panel_bg", "splash_bg", "thumbnail_bg", "scrim", "glyph_text", "glyph_backing", "plate_text",
        // the view cube is a surface of its own: its faces are separated from the canvas by their own
        // edge rather than by luminance. What LIES on them is checked by a test of its own below, against
        // their native background rather than the canvas
        "viewcube_face", "viewcube_edge",
        // a ghost fades TOWARDS this colour rather than being drawn in it: it is supposed to be close to
        // the canvas
        "ghost_target",
        // the colour of a body is not a mark on the canvas but the surface itself: it reads by its shading
        // and its silhouette. Demanding contrast with the background from it is wrong — the check of the
        // shading below answers for it
        "body_face", "body_clash",
    ];
    // the quiet surfaces: they are supposed to be barely noticeable
    const QUIET: &[&str] = &["grid", "grid_minor", "sketch_axis_idle", "panel_border", "cam_table", "cam_table_grid", "dim_helper_ring", "body_ghost"];

    for p in builtin() {
        let bg = luma(p.viewport_bg);
        for (key, v) in p.entries() {
            if GROUNDS.contains(&key) {
                continue;
            }
            // THE COLOURS OF THE INTERFACE ITSELF HAVE NEVER SEEN THE CANVAS: a button lies on a panel and
            // not on the scene. There is no point measuring them against the background of the viewport —
            // they have a guard of their own below.
            if key.starts_with("ui_") {
                continue;
            }
            let diff = (luma(v) - bg).abs();
            let need = if QUIET.contains(&key) { 12.0 } else { 45.0 };
            assert!(diff >= need, "in the scheme \"{}\" the colour `{key}` = {v:?} merges into the background (a luminance difference of {diff:.0}, {need} is needed)", p.name);
        }
    }
}

/// WHAT DOES NOT LIE ON THE CANVAS IS LEGIBLE ON ITS OWN BACKGROUND.
///
/// The check above measures everything against the canvas, and for a mark inside a coloured plate that is
/// meaningless: it has never seen the canvas. Here the pairs of "what lies on what" are listed — the only
/// way to catch, for instance, a white view cube on a white canvas (and it was caught).
#[test]
fn marks_are_legible_against_the_surface_they_lie_on() {
    for p in builtin() {
        for (what, mark, ground) in [
            ("the edge of the view cube", p.viewcube_edge, p.viewcube_face),
            ("the badge of a constraint", p.glyph_text, p.constraint_ok),
            ("the badge of a conflict", p.glyph_text, p.error),
            ("the caption on a face of the cube", p.plate_text, p.viewcube_face),
            ("the text of a measurement label", p.plate_text, p.measure),
            ("the text on the plate of a cut", p.plate_text, p.cut_line),
            ("the text on a card", p.text_strong, p.panel_bg),
            ("the explanation on a card", p.text_dim, p.panel_bg),
        ] {
            let diff = (luma(mark) - luma(ground)).abs();
            assert!(diff > 45.0, "in the scheme \"{}\" {what} {mark:?} merges into its own background {ground:?} (a difference of {diff:.0})", p.name);
        }
    }
}

/// A BODY STAYS A READABLE OBJECT — an end-to-end check of the whole shading pipeline.
///
/// The report came twice: first that the parts look darker than they should, then that they had not become
/// any lighter. The first attempt raised only THE FLOOR, and that did not help: shading is a
/// multiplication, it can only darken, so the ceiling of a part stayed its own colour (about 160 in
/// luminance against a canvas of 240). The part remained a dark object on a light field.
///
/// So what is checked is not a single number but what comes out ON SCREEN from the real colours of the
/// parts: the brightest face, the darkest one, and the spread between them.
#[test]
fn a_body_stays_a_readable_object_in_every_scheme() {
    for p in builtin() {
        let bg = luma(p.viewport_bg);
        for i in 0..8 {
            let c = qymcad_core::model::default_part_color(i);
            let lifted = brighten(c, p.body_lighten, p.body_saturate);
            let dim = luma(shaded(lifted, lit(p.shade_floor_body, 0.0)));
            let bright = luma(shaded(lifted, lit(p.shade_floor_body, 1.0)));

            assert!(bright - dim >= 30.0, "in the scheme \"{}\" the part {c:?} is flat: the spread of the shading is only {:.0}", p.name, bright - dim);
            assert!((bright - bg).abs() >= 15.0, "in the scheme \"{}\" the lit face of the part {c:?} is indistinguishable from the canvas ({bright:.0} against {bg:.0})", p.name);
            if p.light {
                assert!(dim > 130.0, "in a light scheme the part {c:?} sinks into a silhouette: its darkest face is {dim:.0} against a canvas of {bg:.0}");
            } else {
                assert!(bright > bg + 60.0, "in a dark scheme the part {c:?} must be lighter than the canvas: {bright:.0} against {bg:.0}");
            }
        }
    }
}

/// A BODY BECOMES BRIGHTER, NOT WHITER. Of the first attempt it was said that the parts had simply become
/// whiter rather than lighter.
///
/// And so they had: mixing in white raises the luminance and KNOCKS DOWN the saturation, while the colours
/// of the parts are muted as it is. What is checked is the substance: after the lift the saturation does
/// not fall and the hue does not drift.
#[test]
fn brightening_a_body_keeps_its_colour_instead_of_washing_it_out() {
    for p in builtin() {
        for i in 0..8 {
            let c = qymcad_core::model::default_part_color(i);
            let out = brighten(c, p.body_lighten, p.body_saturate);
            let (h0, s0, l0) = to_hsl(c);
            let (h1, s1, l1) = to_hsl(out);
            assert!(s1 >= s0 - 0.01, "in the scheme \"{}\" the part {c:?} -> {out:?} turned white: the saturation went {s0:.2} -> {s1:.2}", p.name);
            assert!(l1 >= l0 - 0.01, "in the scheme \"{}\" the part {c:?} darkened instead of lightening", p.name);
            let dh = (h0 - h1).abs().min(360.0 - (h0 - h1).abs());
            assert!(dh < 6.0, "in the scheme \"{}\" the hue of the part {c:?} drifted: {h0:.0} deg -> {h1:.0} deg", p.name);
        }
    }
}

/// THE DARK SCHEME DOES NOT TOUCH THE COLOUR OF A PART AT ALL — byte for byte, with no trip through HSL.
#[test]
fn the_dark_scheme_leaves_body_colours_untouched() {
    let d = dark();
    for i in 0..8 {
        let c = qymcad_core::model::default_part_color(i);
        assert_eq!(brighten(c, d.body_lighten, d.body_saturate), c, "the dark scheme must give the colour of a part back as it is");
    }
}

/// THE VIEW CUBE is an object too, and on a light canvas it has no right to be a grey blob.
#[test]
fn the_view_cube_is_not_a_grey_blob_on_a_light_canvas() {
    for p in builtin() {
        let bg = luma(p.viewport_bg);
        let dim = luma(shaded(p.viewcube_face, lit(p.shade_floor_viewcube, 0.0)));
        let bright = luma(shaded(p.viewcube_face, lit(p.shade_floor_viewcube, 1.0)));
        assert!(bright - dim >= 25.0, "in the scheme \"{}\" the faces of the cube are indistinguishable: the spread is {:.0}", p.name, bright - dim);
        if p.light {
            assert!(dim > bg - 40.0, "in a light scheme the cube is dark: its darkest face is {dim:.0} against a canvas of {bg:.0}");
        }
    }
}

/// THE DARK SCHEME SHADES EXACTLY AS IT DID BEFORE. The thresholds were moved into the scheme, and the
/// look must not change because of it.
///
/// Every number is checked against the formula that stood in the renderer before the move. Should anybody
/// one day correct the floor by eye, the test will call that a change to the dark scheme rather than let it
/// through silently.
#[test]
fn the_dark_scheme_shades_exactly_as_it_did_before() {
    let d = dark();
    for t in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        let was_body = 0.4 + 0.6 * t; // gui.rs: lit = 0.4 + 0.6 * |n . light|
        assert!((lit(d.shade_floor_body, t) - was_body).abs() < 1e-6, "the body at t={t}");
        let was_mesh = (45.0 + t * 150.0) / 195.0; // render.rs, draw_mesh
        assert!((lit(d.shade_floor_mesh, t) - was_mesh).abs() < 1e-6, "the mesh at t={t}");
        let was_cube = (66.0 + t * 169.0) / 235.0; // viewcube.rs: from the furthest corner to a face head on
        assert!((lit(d.shade_floor_viewcube, t) - was_cube).abs() < 1e-6, "the cube at t={t}");
    }
}

/// A GHOST IS MUTED, NOT PAINTED BLACK. On a light background, fading an inactive part into darkness makes
/// it the most noticeable thing on screen — exactly the opposite of what was intended.
#[test]
fn a_ghost_fades_toward_the_canvas_not_away_from_it() {
    for p in builtin() {
        let bg = luma(p.viewport_bg);
        let t = luma(p.ghost_target);
        let body = luma([160, 160, 160]); // an ordinary grey part
        assert!((t - bg).abs() < (body - bg).abs(), "in the scheme \"{}\" a ghost fades AWAY from the canvas ({t:.0} against a background of {bg:.0}) when it should fade towards it", p.name);
    }
}

/// A GLYPH IS LEGIBLE ON ITS BACKING — EVERY glyph, and with the TRANSPARENCY of the backing taken into
/// account.
///
/// Reported behaviour: in the light theme every glyph, selected or not, and the anchor glyph too, look as
/// if they were under milk. And so they did: the backing is drawn translucent while the check compared the
/// opaque colour, and against one glyph out of four at that. In fact an ordinary joint came out at -8
/// against ITS OWN backing: the glyph and the ground under it are of one luminance, that is, there is no
/// glyph at all.
///
/// So what is taken here is exactly what is seen on screen: the backing BLENDED WITH THE CANVAS by its own
/// alpha, against every glyph drawn on it.
#[test]
fn every_glyph_is_legible_on_its_translucent_backing() {
    const DISC_ALPHA: u8 = 225; // as in draw_joints and draw_grounded_glyphs
    for p in builtin() {
        let disc = blend(p.glyph_backing, p.viewport_bg, DISC_ALPHA);
        for (what, mark) in [
            ("the anchor", p.grounded),
            ("a selected joint", p.active),
            ("a joint under the cursor", p.joint_hover),
            ("an ordinary joint", p.joint_idle),
        ] {
            let diff = (luma(mark) - luma(disc)).abs();
            assert!(diff > 60.0, "in the scheme \"{}\" the glyph of {what} {mark:?} merges into its backing {disc:?} (a difference of {diff:.0})", p.name);
        }
    }
}

/// THE LIGHT SCHEME IS REALLY LIGHT — exactly the report all this started from.
#[test]
fn the_light_scheme_actually_lightens_the_canvas() {
    let (d, l) = (dark(), light());
    assert!(l.light && !d.light);
    assert!(luma(l.viewport_bg) > 200.0, "the background of the viewport must be LIGHT, and its luminance is {}", luma(l.viewport_bg));
    assert!(luma(l.toolbar_bg) > 180.0, "the tool bar must be light");
    assert!(luma(d.viewport_bg) < 60.0, "a dark background must stay dark, otherwise the test means nothing");
}

/// THE LIGHT SCHEME IS DERIVED BY A RULE RATHER THAN BY INVERSION: the hue is kept.
///
/// An inversion would make the yellow sketch line blue and the green "satisfied" pink: the scheme would
/// stop being the same scheme. It is checked on colours whose hue is unambiguous.
#[test]
fn the_light_scheme_keeps_the_hue_it_inherited() {
    for (what, dc, lc) in [
        ("the sketch line", dark().sketch_line, light().sketch_line),
        ("an error", dark().error, light().error),
        ("a success", dark().ok, light().ok),
        ("a preview", dark().preview, light().preview),
    ] {
        let (hd, _, _) = to_hsl(dc);
        let (hl, _, _) = to_hsl(lc);
        let delta = (hd - hl).abs().min(360.0 - (hd - hl).abs());
        assert!(delta < 12.0, "{what}: the hue drifted from {hd:.0} deg to {hl:.0} deg — that is a redesign rather than a move onto a light background");
    }
}

/// THE RULE HANDLES A COLOUR NOBODY HAS ADDED YET.
///
/// That is what the rule was written for: the next tool will bring a colour of its own into the dark
/// scheme, and the light one must cope with it by itself, without seventy lines being edited by hand.
#[test]
fn the_rule_handles_a_colour_nobody_has_added_yet() {
    let bg = luma(light().viewport_bg);
    for c in [[255, 0, 255], [0, 255, 0], [255, 255, 0], [10, 10, 10], [250, 250, 250], [0, 128, 255]] {
        let out = toward_light(c);
        assert!((luma(out) - bg).abs() > 45.0, "the rule did not cope with {c:?} -> {out:?} (a luminance of {:.0} against a background of {bg:.0})", luma(out));
    }
}

/// THE NEUTRALS ARE MIRRORED RATHER THAN TINTED, and the order among them is kept.
///
/// Text must stay text rather than acquire a tint; and if in the dark scheme one grey was more noticeable
/// than another, in the light one it must stay more noticeable — otherwise the main and the secondary text
/// would swap roles. An exact mirror is not required: a near-black is barred from the top by a ceiling,
/// otherwise it would dissolve into the light canvas.
#[test]
fn neutrals_stay_neutral_and_keep_their_order() {
    let greys = [20u8, 60, 135, 170, 230];
    let out: Vec<[u8; 3]> = greys.iter().map(|&g| toward_light([g, g, g])).collect();
    for (g, o) in greys.iter().zip(&out) {
        let spread = o.iter().max().unwrap() - o.iter().min().unwrap();
        assert!(spread <= 2, "the grey {g} became coloured: {o:?}");
    }
    for w in out.windows(2) {
        assert!(w[0][0] > w[1][0], "the order of the greys must invert as a whole: {:?} -> {:?}", w[0], w[1]);
    }
    assert!(out[0][0] <= 190, "a near-black has no right to become a near-white: {:?}", out[0]);
    assert!(out[4][0] <= 40, "a near-white must become dark: {:?}", out[4]);
}

/// THE EDITOR OF ONE'S OWN SCHEME: a colour is changed by name, and an unknown name is refused rather
/// than written silently past the target.
#[test]
fn a_colour_can_be_changed_by_name_and_a_typo_is_refused() {
    let mut p = dark();
    assert!(p.set("sketch_line", [1, 2, 3]), "the name exists, so the value is accepted");
    assert_eq!(p.sketch_line, [1, 2, 3]);
    assert!(!p.set("sketch_lien", [9, 9, 9]), "a typo must be refused rather than swallowed");
}

/// A SCHEME IS DATA: it survived being written to a file and read back whole.
#[test]
fn a_scheme_survives_a_round_trip_through_a_file() {
    let mut p = light();
    p.id = "mine".into();
    p.name = "Mine".into();
    p.sketch_line = [7, 8, 9];
    let text = ron::ser::to_string(&p).expect("written");
    let back: Palette = ron::from_str(&text).expect("read");
    assert_eq!(back.name, "Mine");
    assert_eq!(back.sketch_line, [7, 8, 9]);
    assert_eq!(back.entries(), p.entries(), "not one colour was lost along the way");
}

/// A SCHEME FROM A FUTURE VERSION, with more fields, still reads — the missing ones come from the dark
/// scheme.
#[test]
fn an_incomplete_scheme_file_still_loads() {
    let back: Palette = ron::from_str("(name:\"Stub\",light:false,sketch_line:(1,2,3))").expect("it reads");
    assert_eq!(back.name, "Stub");
    assert_eq!(back.sketch_line, [1, 2, 3]);
    assert_eq!(back.viewport_bg, dark().viewport_bg, "what is missing comes from the dark scheme");
}

/// THE SHADING OF A FACE is the same multiplication that was in the code: on the brightest face the
/// colour is full, and deeper it is proportionally darker.
#[test]
fn shading_scales_the_body_colour_without_shifting_it() {
    let full = tint(dark().body_face(), 1.0);
    assert_eq!((full.r(), full.g(), full.b()), (97, 195, 214), "full lighting gives the original colour");
    let half = tint(dark().body_face(), 0.5);
    assert_eq!((half.r(), half.g(), half.b()), (48, 97, 107), "twice as dark means half as much in every channel");
}

/// THE INTERFACE IS LEGIBLE AGAINST ITS OWN BACKGROUND.
///
/// A check of "legible on the canvas" is meaningless for a button: it lies on a panel. Here the pairs of
/// "what on what" inside a window are listed — and that is the only thing that catches a scheme where the
/// text matched the background of a field or a selected row of a list became unreadable.
///
/// The threshold for text is stricter than for surfaces: text is read by its letters, a surface as a
/// patch.
#[test]
fn the_interface_colours_are_legible_against_each_other() {
    for p in builtin() {
        let title = if p.id.is_empty() { "?".into() } else { p.id.clone() };
        for (what, mark, ground, need) in [
            ("the text on a panel", p.ui_text, p.ui_window, 60.0),
            ("the muted text on a panel", p.ui_text_dim, p.ui_window, 40.0),
            ("the text in an input field", p.ui_text, p.ui_field, 60.0),
            ("the text on a button", p.ui_text, p.ui_control, 50.0),
            ("the text on a button under the cursor", p.ui_text_strong, p.ui_control_hover, 50.0),
            ("the text on a pressed button", p.ui_text_strong, p.ui_control_active, 50.0),
            ("the text on a selected row", p.ui_text, p.ui_accent, 40.0),
            ("a button on a panel", p.ui_control, p.ui_window, 8.0),
            ("a border on a panel", p.ui_outline, p.ui_window, 8.0),
            ("an input field on a panel", p.ui_field, p.ui_window, 4.0),
        ] {
            let diff = (luma(mark) - luma(ground)).abs();
            assert!(diff >= need, "in the scheme \"{title}\" {what} {mark:?} merges into the background {ground:?} (a difference of {diff:.0}, {need} is needed)");
        }
    }
}

/// A SCHEME WITHOUT INTERFACE COLOURS OF ITS OWN GIVES BACK THE STOCK LOOK BYTE FOR BYTE.
///
/// That is a promise rather than a trifle: the current look is liked, and the arrival of an ability to
/// paint the interface has no right to repaint the dark and the light schemes along the way.
#[test]
fn a_scheme_without_interface_colours_leaves_the_stock_look_alone() {
    for (p, stock) in [(dark(), egui::Visuals::dark()), (light(), egui::Visuals::light())] {
        assert!(!p.ui_on, "the built-in scheme \"{}\" does not paint the interface", p.id);
        let v = visuals(&p);
        assert_eq!(v.panel_fill, stock.panel_fill, "the background of a panel in the scheme \"{}\" diverged from the stock one", p.id);
        assert_eq!(v.widgets.inactive.bg_fill, stock.widgets.inactive.bg_fill, "a button in the scheme \"{}\"", p.id);
        assert_eq!(v.widgets.inactive.fg_stroke, stock.widgets.inactive.fg_stroke, "the text in the scheme \"{}\"", p.id);
        assert_eq!(v.selection.bg_fill, stock.selection.bg_fill, "the selection in the scheme \"{}\"", p.id);
        assert_eq!(v.extreme_bg_color, stock.extreme_bg_color, "an input field in the scheme \"{}\"", p.id);
    }
}

/// AND ITS WRITTEN-DOWN INTERFACE COLOURS ARE THE VERY SAME ONES.
///
/// A copy of the dark scheme with the box ticked must start from EXACTLY what was being seen: otherwise
/// "paint the interface" would mean "repaint at random". It is also a guard over upgrades of egui — should
/// the stock numbers move, this is where it turns red.
#[test]
fn the_written_down_interface_colours_match_the_stock_ones() {
    for (base, stock) in [(dark(), egui::Visuals::dark()), (light(), egui::Visuals::light())] {
        let mut p = base.clone();
        p.ui_on = true;
        let v = visuals(&p);
        assert_eq!(v.panel_fill, stock.panel_fill, "the background of a panel, the scheme \"{}\"", p.id);
        assert_eq!(v.window_fill, stock.window_fill, "the background of a window, the scheme \"{}\"", p.id);
        assert_eq!(v.extreme_bg_color, stock.extreme_bg_color, "an input field, the scheme \"{}\"", p.id);
        assert_eq!(v.window_stroke.color, stock.window_stroke.color, "the border of a window, the scheme \"{}\"", p.id);
        assert_eq!(v.hyperlink_color, stock.hyperlink_color, "a link, the scheme \"{}\"", p.id);
        assert_eq!(v.selection.bg_fill, stock.selection.bg_fill, "the selection, the scheme \"{}\"", p.id);
        assert_eq!(v.widgets.inactive.bg_fill, stock.widgets.inactive.bg_fill, "a button, the scheme \"{}\"", p.id);
        assert_eq!(v.widgets.hovered.bg_fill, stock.widgets.hovered.bg_fill, "a button under the cursor, the scheme \"{}\"", p.id);
        assert_eq!(v.widgets.active.bg_fill, stock.widgets.active.bg_fill, "a pressed button, the scheme \"{}\"", p.id);
        assert_eq!(v.widgets.inactive.fg_stroke.color, stock.widgets.inactive.fg_stroke.color, "the text, the scheme \"{}\"", p.id);
        assert_eq!(v.widgets.noninteractive.fg_stroke.color, stock.widgets.noninteractive.fg_stroke.color, "the muted text, the scheme \"{}\"", p.id);
        assert_eq!(v.widgets.hovered.fg_stroke.color, stock.widgets.hovered.fg_stroke.color, "the text under the cursor, the scheme \"{}\"", p.id);
    }
}

/// A SCHEME THAT PAINTS THE INTERFACE REALLY DOES PAINT IT.
///
/// Without this the whole `ui_*` block could stay a dead field in the file: the colours are there, the box
/// is there, and `visuals` does not ask for them — so Dracula would look like the stock dark theme with a
/// violet canvas.
#[test]
fn a_scheme_that_paints_the_interface_actually_paints_it() {
    for p in builtin().into_iter().filter(|p| p.ui_on) {
        let v = visuals(&p);
        let stock = if p.light { egui::Visuals::light() } else { egui::Visuals::dark() };
        assert_eq!(v.panel_fill, rgb(p.ui_window), "the scheme \"{}\" did not paint the panel", p.id);
        assert_eq!(v.widgets.inactive.bg_fill, rgb(p.ui_control), "the scheme \"{}\" did not paint the button", p.id);
        assert_eq!(v.selection.bg_fill, rgb(p.ui_accent), "the scheme \"{}\" did not paint the selection", p.id);
        assert_ne!(v.panel_fill, stock.panel_fill, "the scheme \"{}\" declared itself a painting one and the look stayed stock", p.id);
        // a scheme does not touch widths or roundings: it is about colour, not about shape
        assert_eq!(v.widgets.hovered.fg_stroke.width, stock.widgets.hovered.fg_stroke.width, "the scheme \"{}\" reached into the width of a line", p.id);
    }
    assert_eq!(builtin().iter().filter(|p| p.ui_on).count(), 2, "exactly two built-in schemes paint the interface: Dracula and Alucard");
}
