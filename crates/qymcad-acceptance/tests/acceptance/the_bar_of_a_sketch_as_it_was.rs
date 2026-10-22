//! THE BAR OF A SKETCH AS IT ALWAYS WAS, in the small things the frame of a tool must not lose: the count of what is
//! selected at the end of the bar, a way chosen again starting a drawing anew, Esc with a modifier held, and Enter of
//! Extend with the pointer anywhere.
use qymcad::{Key, Modifiers};
use qymcad_acceptance::contract::fixtures::Fixture;
use qymcad_acceptance::probe;

probe! {
    /// THE COUNT OF WHAT IS SELECTED STANDS AT THE END OF THE BAR, right of its hints, as the bar of a sketch told it.
    fn the_count_of_the_selected_stands_at_the_end_of_the_bar() {
        let mut s = Fixture::SketchOnXy.start();
        let count = s.word_with("opt-selected-n", qymcad::Arg { name: "n", value: "0" });
        let at = s.find(&count, qymcad::pos2(0.0, 0.0)).unwrap_or_else(|| panic!("the bar counts nothing selected; on screen: {:?}", s.words()));
        let across = s.screen().x;
        assert!(at.max.x > across * 0.8, "the count of the selected stands at x {} of {across}, not at the end of the bar", at.max.x);
    }
}

probe! {
    /// THE WAY ALREADY CHOSEN, CLICKED AGAIN, STARTS THE DRAWING ANEW: the first corner of a rectangle clicked, "2 corners"
    /// pressed again, and the next click is a first corner again - nothing is drawn by it.
    fn the_way_chosen_again_starts_the_drawing_anew() {
        let mut s = Fixture::SketchOnXy.start();
        let hint = s.word("tb-rect-hint");
        s.press_hint(&hint);
        s.click_on_sketch(0.0, 0.0);
        let way = s.word("opt-rect-2corners");
        s.press_word_near(&way, qymcad::pos2(0.0, 0.0));
        s.click_on_sketch(40.0, 30.0);
        assert_eq!(s.document().sketches[0].lines, 0, "the way chosen again kept the first corner: the next click drew a rectangle");
    }
}

probe! {
    /// Esc WITH SHIFT HELD PUTS THE TOOL DOWN as Esc does.
    fn esc_with_shift_held_puts_the_tool_down() {
        let mut s = Fixture::SketchOnXy.start();
        let bar = s.in_hand();
        let hint = s.word("tb-line-hint");
        s.press_hint(&hint);
        assert_ne!(s.in_hand(), bar, "setup: the line is not in hand");
        s.chord(Modifiers::SHIFT, Key::Escape);
        assert_eq!(s.in_hand(), bar, "Shift+Esc left the line in hand: the bar says {:?}", s.in_hand());
    }
}

probe! {
    /// ENTER OF EXTEND MAKES THE EXTENSION SHOWN WITH THE POINTER ANYWHERE: the line picked by its end, the extension shown
    /// up to the line across, the pointer moved off the view over the properties, Enter - the line reaches the one across.
    fn enter_of_extend_makes_the_extension_with_the_pointer_off_the_view() {
        let mut s = Fixture::LineShortOfLine.start();
        let hint = s.word("tb-extend-hint");
        s.press_hint(&hint);
        s.click_on_sketch(18.0, 0.0);
        let shown = s.on_sketch(25.0, 3.0);
        s.move_to(shown);
        let canvas = s.canvas();
        s.move_to(qymcad::pos2(canvas.max.x + 40.0, canvas.center().y));
        s.key(Key::Enter);
        let reach = s.document().sketches[0].max[0];
        assert!((reach - 30.0).abs() < 1e-6, "Enter with the pointer off the view: the sketch reaches x {reach}, the line across stands at 30");
    }
}
