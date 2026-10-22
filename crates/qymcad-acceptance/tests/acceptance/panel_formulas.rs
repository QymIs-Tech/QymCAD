//! A FORMULA IN A FIELD OF A PANEL: `w/2` typed into a number of the properties - of an operation, of a dimension in the
//! list of a sketch - is half of the parameter `w`, and follows `w` when it changes. The parameters themselves are
//! written from one another in their table (`parameters::a_parameter_is_written_from_another_and_follows_it`).
use qymcad::{Key, Kind, Modifiers, Session, Widget};
use qymcad_acceptance::contract::fixtures::Fixture;
use qymcad_acceptance::{build, probe};

/// The field of the properties panel nearest under or after the words `label`, right of the view.
fn panel_field(s: &mut Session, label: &str) -> Widget {
    let right = s.canvas().max.x;
    let at = s.words_at().into_iter().find(|(w, r)| w == label && r.min.x > right).map(|(_, r)| r).unwrap_or_else(|| panic!("the properties name no {label:?}; on screen: {:?}", s.words()));
    s.widgets()
        .into_iter()
        .filter(|w| w.kind == Kind::TextField && w.rect.min.x > right && w.rect.center().y >= at.min.y - 2.0)
        .min_by(|a, b| (a.rect.center().y - at.center().y).abs().total_cmp(&(b.rect.center().y - at.center().y).abs()))
        .unwrap_or_else(|| panic!("no field of the properties stands by {label:?}"))
}

/// Type `text` over what field `w` holds, and Enter.
fn type_over(s: &mut Session, w: &Widget, text: &str) {
    s.click(w.rect.center()).chord(Modifiers::COMMAND, Key::A).type_text(text).key(Key::Enter);
}

/// How high the block stands.
fn height(s: &mut Session) -> f64 {
    let b = &s.document().bodies[0];
    b.max[2] - b.min[2]
}

probe! {
    /// THE HEIGHT OF AN EXTRUSION IN ITS PROPERTIES TAKES `w/2`: the block, `w = 40`, the node picked in the tree; the
    /// block stands 20 high, and 30 once `w` is 60.
    fn a_formula_in_the_properties_of_an_operation_follows_its_parameter() {
        let mut s = Fixture::Block.start();
        build::parameter(&mut s, "w", "40");
        // the tree names the node by what it made
        let node = s.word_with("feat-extrude", qymcad::Arg { name: "h", value: "10" });
        let left = s.canvas().min.x;
        let row = s.words_at().into_iter().find(|(w, r)| *w == node && r.max.x < left).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("the row {node:?}; on screen: {:?}", s.words()));
        s.click(row);
        let field = panel_field(&mut s, "height");
        type_over(&mut s, &field, "w/2");
        assert!((height(&mut s) - 20.0).abs() < 1e-6, "w/2 with w = 40: the block stands {} high", height(&mut s));
        build::set_parameter(&mut s, "w", "60");
        assert!((height(&mut s) - 30.0).abs() < 1e-6, "w changed to 60: the block stands {} high", height(&mut s));
    }
}

probe! {
    /// A DIMENSION IN THE LIST OF A SKETCH TAKES `w/2`: the width of a rectangle dimensioned 40, `w = 60`, `w/2` typed
    /// into its row; the rectangle is 30 wide, and 40 once `w` is 80.
    fn a_formula_in_the_list_of_a_sketch_follows_its_parameter() {
        let mut s = Fixture::RectangleInSketch.start();
        s.key(Key::Escape).key(Key::Escape);
        let hint = s.word("tb-dim-hint");
        s.press_hint(&hint);
        s.click_on_sketch(20.0, 0.0).click_on_sketch(20.0, -10.0);
        s.key(Key::Escape).key(Key::Escape);
        build::parameter(&mut s, "w", "60");
        let label = s.word("sk-dim");
        let field = panel_field(&mut s, &label);
        type_over(&mut s, &field, "w/2");
        let wide = |s: &mut Session| {
            let k = &s.document().sketches[0];
            k.max[0] - k.min[0]
        };
        assert!((wide(&mut s) - 30.0).abs() < 1e-6, "w/2 with w = 60: the rectangle is {} wide", wide(&mut s));
        build::set_parameter(&mut s, "w", "80");
        assert!((wide(&mut s) - 40.0).abs() < 1e-6, "w changed to 80: the rectangle is {} wide", wide(&mut s));
    }
}
