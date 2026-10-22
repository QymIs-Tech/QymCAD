//! THE FONT OF A TEXT ON THE SHEET: taken from the list of the fonts of the machine, or from a file a person points at.
use qymcad::{pos2, Chooser, Session};
use qymcad_acceptance::contract::fixtures::Fixture;
use qymcad_acceptance::probe;

/// A font file of the repository, for the one taken from a file.
const FONT_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/LiberationSans-Bold.ttf");

/// The text tool in hand, with `string` typed into its bar.
fn text_tool(s: &mut Session, string: &str) {
    let hint = s.word("tb-text-hint");
    s.press_hint(&hint);
    let caption = s.word("tool-text");
    s.fill(&caption, string);
}

/// Open the list of fonts from the bar of the text tool.
fn open_the_fonts(s: &mut Session) {
    let named = s.word("opt-font");
    s.press_word_near(&named, pos2(700.0, 0.0));
    let title = s.word("pk-font");
    assert!(s.windows().contains(&title), "the button of the font did not open the list; windows: {:?}", s.windows());
}

probe! {
    /// A FONT IS TAKEN FROM THE LIST OF THE MACHINE'S FONTS, and the text is written in it.
    fn a_font_is_taken_from_the_list() {
        let mut s = Fixture::SketchOnXy.start();
        text_tool(&mut s, "CAD");
        open_the_fonts(&mut s);
        // the rows of the list stand under the button of a file and the count of what was found
        let from_file = s.word("font-from-file");
        let button = s.find(&from_file, pos2(640.0, 400.0)).expect("the list offers a font from a file");
        let first_row = pos2(button.center().x, button.bottom() + 40.0);
        s.click(first_row);
        let title = s.word("pk-font");
        assert!(!s.windows().contains(&title), "the list stayed open after a font was clicked; on screen: {:?}", s.words());
        let named = s.word("opt-font");
        // the button of the row "Font" of the box of the tool names the font taken
        let shown = s.words();
        s.click_on_sketch(0.0, 0.0);
        let fonts = s.document().sketches.first().map(|k| k.text_fonts.clone()).unwrap_or_default();
        assert!(!fonts.is_empty() && fonts.iter().all(|f| *f != named), "the text is written in {fonts:?}: no font was taken from the list");
        assert!(shown.iter().any(|w| fonts.contains(w)), "the box names no font the text is written in ({fonts:?}); on screen: {shown:?}");
    }
}

probe! {
    /// A FONT IS TAKEN FROM A FILE a person points the chooser at, and the text is written in it.
    fn a_font_is_taken_from_a_file() {
        let mut s = Fixture::SketchOnXy.start();
        text_tool(&mut s, "CAD");
        open_the_fonts(&mut s);
        let from_file = s.word("font-from-file");
        s.press_word_near(&from_file, pos2(640.0, 400.0));
        assert_eq!(s.chooser(), Some(Chooser::Open), "the button of a file put up no chooser");
        s.answer_file(FONT_FILE);
        s.click_on_sketch(0.0, 0.0);
        let fonts = s.document().sketches.first().map(|k| k.text_fonts.clone()).unwrap_or_default();
        assert!(fonts.iter().any(|f| f.contains("Liberation")), "the text is written in {fonts:?}, and the file of Liberation Sans Bold was chosen");
    }
}
