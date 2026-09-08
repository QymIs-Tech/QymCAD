//! THE HELP SAYS HOW TO MOVE THE VIEW WITH THE MOUSE, IN BOTH VIEWPORTS AND BOTH LANGUAGES.
//!
//! Reported behaviour: "add a basic guide to mouse navigation in the 3D viewport and in the sketcher
//! viewport to the help."
//!
//! The article about the viewport told what the view cube does, what a section is and what can be
//! selected - and not one word about the mouse, which is the first thing anybody touches. The sketcher's
//! flat view was not described at all, and its buttons are NOT the same: the middle one pans there,
//! because the left one draws.
//!
//! WHY A CHECK AND NOT JUST THE TEXT. Two ways to lose this quietly. An article can be rewritten and the
//! section dropped - it is prose, nothing breaks. And the bindings can change in the code while the text
//! stays, which is worse than no text: a person follows it, it does not work, and they conclude the
//! program is broken.
//!
//! So the words that MUST be there are the ones that name a binding, and each of them is a binding the
//! code really has: the wheel zooms, Shift pans in 3D, the middle button pans in a sketch, Ctrl forces a
//! selection box.

fn article(lang: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/help").join(lang).join("general/09-viewport.md");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("the viewport article of \"{lang}\" must be readable: {e}"))
}

/// THE WORDS ARE CHECKED IN ENGLISH ONLY, and the Russian article is held by its neighbour.
///
/// `help_voice::both_languages_tell_the_same_story` already requires the two versions to carry the SAME
/// NUMBER of sections, so a Russian article that lost the mouse section goes red there. Repeating the
/// check here with Russian words would be a second copy of somebody else's guard - and the pair would
/// drift, which is the very shape of trouble this project keeps paying for.
///
/// THE THREE-DIMENSIONAL VIEW: turn, pan, zoom.
#[test]
fn the_viewport_article_names_the_mouse_bindings_of_the_3d_view() {
    let t = article("en");
    assert!(t.contains("wheel"), "the article does not say that the wheel zooms - the first thing anybody tries");
    assert!(t.contains("Shift"), "the article does not say how to PAN, and panning is Shift with a drag");
}

/// AND THE FLAT VIEW OF A SKETCH, whose buttons are different on purpose.
///
/// This is the half that was missing altogether. The middle button is named because in a sketch the left
/// one draws and grabs, so panning had to go somewhere else - and a person coming from the 3D view will
/// try the left one and decide the canvas is stuck.
#[test]
fn the_viewport_article_names_the_mouse_bindings_of_a_sketch() {
    let t = article("en");
    assert!(t.contains("middle button"), "the article does not say that a sketch is panned with the MIDDLE button");
    assert!(t.contains("Ctrl"), "the article does not say that Ctrl draws a selection box over geometry");
}

/// THE TEXT MUST NOT PROMISE WHAT THE CODE NO LONGER DOES.
///
/// The bindings live in `viewport_3d.rs` and `sketching.rs`. This does not read the whole of them - a
/// guard that tried would be reading four thousand lines to check four words - but it does hold the two
/// that a change would break first: the modifier that pans in 3D, and the button that pans in a sketch.
#[test]
fn the_bindings_the_help_promises_are_the_ones_the_code_has() {
    // THE 3D BINDINGS MOVED INTO THE LAYOUTS, so that is where they are asked about now. Reading the
    // viewport for the word `shift` would have gone on passing while saying nothing: the viewport asks the
    // layout, and the layout is the thing that either pans on Shift or does not.
    let ours = qymcad_ui_state::MouseNav::QymCad;
    assert!(ours.pan().shift, "the help says Shift moves the 3D view under our layout, and our layout no longer says so");
    assert!(ours.wheel_zooms(), "the help says the wheel zooms, and our layout no longer puts zoom on the wheel");

    // AND PANNING THE SHEET MOVED OUT OF THE VIEWPORT into the module that owns the view, so the middle
    // button is asked about there. The guard used to read `sketching.rs` for `pointer.middle_down()` and
    // went red the moment that line moved house - correctly: it is what holds the promise, and a guard
    // pointed at the old address would have gone quiet instead of following it.
    let root = qymcad_i18n::ratchet::crates_root();
    let src = std::fs::read_to_string(root.join("qymcad-ui-state/src/lib.rs")).expect("the view module reads");
    let pan = src.split("pub fn pan_sheet_2d").nth(1).expect("panning the sheet is a function of the view module");
    let body = &pan[..pan.find("\n}\n").map(|i| i + 2).unwrap_or(pan.len())];
    assert!(
        body.contains("pointer.middle_down()"),
        "the help says the middle button pans a sketch, and panning the sheet no longer asks about that button"
    );
}

/// AND THE HELP SAYS THE BINDINGS ARE A CHOICE, because they became one.
///
/// The article was written when a drag ALWAYS turned the view. It is a set now, and a text that describes
/// one set as if it were the only one sends the person who picked the other one looking for a fault in the
/// program. Naming the setting is what makes the rest of the section true rather than merely usual.
#[test]
fn the_help_says_which_button_navigates_is_a_setting() {
    let t = article("en");
    assert!(
        t.contains("Mouse navigation"),
        "the bindings are a setting now, and the article still describes one set as the only behaviour"
    );
    let src = std::fs::read_to_string(qymcad_i18n::ratchet::crates_root().join("qymcad/src/gui/viewport_3d.rs")).expect("the 3D viewport reads");
    assert!(
        src.contains("mouse_nav.rotate().active(ctx, resp)") && src.contains("mouse_nav.pan().active(ctx, resp)"),
        "the help promises a choice of navigation and the viewport no longer asks for it"
    );
    // AND THE NAMES IN THE ARTICLE ARE LAYOUTS THAT EXIST. A list of habits that names one the program does
    // not offer sends somebody looking through the settings for a line that is not there.
    let t = article("en");
    for nav in qymcad_ui_state::MouseNav::ALL {
        let name = qymcad_i18n::tr(&nav.key());
        assert!(t.contains(&name), "the article does not name the \"{name}\" layout, so a person cannot tell it is on offer");
    }
}
