//! THE INTERFACE SCALE IS STEPPED, NOT DRAGGED.
//!
//! Reported behaviour, in two rounds.
//!
//! FIRST: "in the settings, egui lets you hold the left button and drag left and right to lower and raise
//! the value. With us that breaks straight away, because the value jumps to a whole number rather than by,
//! say, 0.05." That much was arithmetic. `DragValue::speed` is how far the value moves per POINT dragged
//! and stood at 0.05 over a range of 0.5 to 3.0 - the whole range in fifty points. Measured with a real
//! pointer: a twenty-point twitch, about the smallest deliberate hand movement, took the scale from 1.00
//! to 1.70.
//!
//! THEN, with the speed slowed to 0.01 and the value snapped to steps of 0.05: "it still glitches - you
//! move the mouse, the scale changes, the window under the mouse moves away, and the scale runs off
//! further. Can this drag be turned off altogether?"
//!
//! HE IS RIGHT, AND NO SPEED WOULD HAVE FIXED IT. This setting is applied LIVE - it has to be, or one is
//! choosing a size blind - so every value the drag produces resizes the very field being dragged. The
//! pointer stays where it is while the widget travels out from under it, and the drag goes on feeding on
//! its own output. That is a loop, and the way out of a loop is to cut it: the value is stepped by two
//! buttons instead, 0.05 at a time. A click is one discrete step; the window may relayout after it and
//! nothing has run away.
//!
//! MEASURED WITH A REAL POINTER, not by reading the widget back: what is under test is what a hand gets.
#[cfg(test)]
mod tests {
    use crate::gui::App;
    use qymcad_ui_state::WinKind;

    const SCREEN: egui::Vec2 = egui::vec2(1200.0, 900.0);

    /// Everything the frame drew, with the icons stripped off.
    fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect)> {
        fn walk(s: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::epaint::Shape::Text(t) => {
                    let text: String = t.galley.text().chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect();
                    out.push((text.trim().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size())));
                }
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for cs in shapes {
            walk(&cs.shape, &mut out);
        }
        out
    }

    /// One frame of the settings window with `events` in it; returns what it painted.
    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<(String, egui::Rect)> {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)), events, ..Default::default() };
        let out = ctx.run_ui(raw, |ui| {
            let app = &mut *app;
            let mut asks = Vec::new();
            crate::gui::panels_windows::settings_window(&mut app.win_ctx(&mut asks), ui.ctx());
            app.do_win_asks(asks, ui.ctx());
        });
        texts(&out.shapes)
    }

    /// The settings window open on Appearance, with the scale row on screen and the number located.
    fn the_scale_field() -> (App, egui::Context, egui::Pos2) {
        let mut app = App::default();
        app.win.open(WinKind::Settings);
        app.scheme.section = crate::gui::settings_sections::SettingsSection::Appearance;
        app.set.ui_scale = 1.0;
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let mut drawn = Vec::new();
        for _ in 0..3 {
            drawn = frame(&mut app, &ctx, Vec::new()); // the window settles into its size
        }
        // The number itself is where a hand would have grabbed the old field.
        let at = drawn
            .iter()
            .find(|(t, _)| t == "1.00")
            .map(|(_, r)| r.center())
            .unwrap_or_else(|| panic!("the scale field is not on screen: {:?}", drawn.iter().map(|(t, _)| t).collect::<Vec<_>>()));
        (app, ctx, at)
    }

    /// Drag from `at` by `dx` points, in steps a hand would make.
    fn drag(app: &mut App, ctx: &egui::Context, at: egui::Pos2, dx: f32) {
        frame(app, ctx, vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        let steps = 10;
        for i in 1..=steps {
            let p = at + egui::vec2(dx * i as f32 / steps as f32, 0.0);
            frame(app, ctx, vec![egui::Event::PointerMoved(p)]);
        }
        let end = at + egui::vec2(dx, 0.0);
        frame(app, ctx, vec![egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
    }

    /// WHERE THE NUMBER IS, A BUTTON IS NOT: dragging over the row does nothing at all.
    ///
    /// The old field answered a drag, and that is what fed the loop. The check drags the same twenty
    /// points a hand twitches by - and now over three hundred, which under the old speed would have run
    /// the whole range twice.
    #[test]
    fn dragging_the_scale_row_does_not_move_the_scale() {
        let mut moved = Vec::new();
        for dx in [20.0f32, 300.0, -300.0] {
            let (mut app, ctx, at) = the_scale_field();
            drag(&mut app, &ctx, at, dx);
            if (app.set.ui_scale - 1.0).abs() > 1e-6 {
                moved.push(format!("{dx} points -> {:.2}", app.set.ui_scale));
            }
        }
        assert!(
            moved.is_empty(),
            "the scale still answers a drag, and a setting applied live cannot: the field resizes under the pointer and the drag feeds on its own output - {}",
            moved.join(", ")
        );
    }

    /// AND THE BUTTONS STEP IT BY 0.05, up and down.
    ///
    /// Without this the fix could be "make the control answer nothing", which is not a setting.
    #[test]
    fn the_buttons_step_the_scale_by_five_hundredths() {
        let (mut app, ctx, _) = the_scale_field();
        let plus = button(&mut app, &ctx, "+");
        click(&mut app, &ctx, plus);
        assert!((app.set.ui_scale - 1.05).abs() < 1e-4, "one press of + must raise the scale by 0.05, and it gave {:.2}", app.set.ui_scale);

        let minus = button(&mut app, &ctx, "-");
        click(&mut app, &ctx, minus);
        click(&mut app, &ctx, minus);
        assert!((app.set.ui_scale - 0.95).abs() < 1e-4, "two presses of - must take it to 0.95, and it gave {:.2}", app.set.ui_scale);
    }

    /// THE SCALE STAYS INSIDE ITS LIMITS however long the buttons are pressed.
    #[test]
    fn the_buttons_do_not_take_the_scale_past_its_limits() {
        let (mut app, ctx, _) = the_scale_field();
        for _ in 0..70 {
            let minus = button(&mut app, &ctx, "-");
            click(&mut app, &ctx, minus);
        }
        assert!(app.set.ui_scale >= 0.5 - 1e-6, "the scale went below its floor: {:.2}", app.set.ui_scale);
        assert!((app.set.ui_scale - 0.5).abs() < 1e-4, "seventy presses must reach the floor and stop there, and it stopped at {:.2}", app.set.ui_scale);
    }

    /// Where the button labelled `label` sits in the row.
    fn button(app: &mut App, ctx: &egui::Context, label: &str) -> egui::Pos2 {
        let drawn = frame(app, ctx, Vec::new());
        drawn
            .iter()
            .find(|(t, _)| t == label)
            .map(|(_, r)| r.center())
            .unwrap_or_else(|| panic!("the button \"{label}\" is not on screen: {:?}", drawn.iter().map(|(t, _)| t).collect::<Vec<_>>()))
    }

    /// One click at `at`.
    fn click(app: &mut App, ctx: &egui::Context, at: egui::Pos2) {
        frame(app, ctx, vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        frame(app, ctx, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
    }
}
