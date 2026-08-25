//! LONG VALUES AND A NARROW WINDOW.
//!
//! Named as uncovered by the audit itself: a part name of 60 characters, 200 features in the tree, 50
//! parameters — where does the layout go wrong; and an interface scale of 125/150 % with a narrow
//! window, where clipped captions are exactly what shows up.
//!
//! The trouble looked for here is a quiet one: the panel swells to fit the longest name and eats the
//! canvas, or a caption is clipped to death and a person stops telling the nodes apart. Neither shows
//! up as an error, only in the layout — so what has to be checked is the WIDTH, not the absence of a
//! panic.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// The width of the tree panel at a given window size and interface scale.
    fn tree_width(app: &mut App, window: egui::Vec2, zoom: f32) -> f32 {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        ctx.set_zoom_factor(zoom);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), window)),
            ..Default::default()
        };
        // two frames: the first lays out, the second shows the SETTLED width
        let _ = ctx.run(input.clone(), |c| app.tree_panel(c));
        let _ = ctx.run(input, |c| app.tree_panel(c));
        egui::panel::PanelState::load(&ctx, egui::Id::new("tree")).map(|p| p.rect.width()).unwrap_or(0.0)
    }

    fn part_with_a_body() -> (App, u64) {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.regenerate_now();
        let body = app.project.mesh_id(0).expect("the body");
        let comp = app.project.body_owner(body).expect("the owner");
        (app, comp)
    }

    /// A SIXTY-CHARACTER NAME DOES NOT WIDEN THE PANEL.
    ///
    /// A person may name a part however they like. If the panel grows to fit the name, the canvas is
    /// taken away from them — and the more carefully they named their parts, the more of it goes.
    #[test]
    fn a_sixty_character_name_does_not_widen_the_tree() {
        let (mut app, comp) = part_with_a_body();
        let window = egui::vec2(1400.0, 900.0);
        let base = tree_width(&mut app, window, 1.0);
        assert!(base > 50.0, "setup: the tree panel must have a width, and it came out {base}");

        let long = "Lower left load-bearing reinforced bracket variant two".to_string();
        assert!(long.chars().count() >= 50, "setup: the name must be a long one");
        if let Some(c) = app.project.components.iter_mut().find(|c| c.id == comp) {
            c.name = long.clone();
        }
        let after = tree_width(&mut app, window, 1.0);
        assert!(
            after <= base + 1.0,
            "a long name widened the tree panel: it was {base}, it became {after} — the canvas is taken away from a person for naming their part carefully"
        );
    }

    /// TWO HUNDRED FEATURES IN THE TIMELINE — THE PANEL IS THE SAME WIDTH.
    ///
    /// A long history is an everyday thing in a living part; the width of the tree must not depend on
    /// it.
    #[test]
    fn two_hundred_features_do_not_widen_the_tree() {
        let (mut app, comp) = part_with_a_body();
        let window = egui::vec2(1400.0, 900.0);
        let base = tree_width(&mut app, window, 1.0);

        let mut proto = app.project.timeline.first().cloned().expect("a node");
        for k in 0..200 {
            proto.id = app.project.alloc_id();
            proto.name = format!("Feature number {k}");
            proto.parent = Some(comp);
            app.project.timeline.push(proto.clone());
        }
        let after = tree_width(&mut app, window, 1.0);
        // THE WIDTH MUST BE NON-ZERO. Without this the check is green even for a panel that did not
        // lay out at all: "no wider than before" is true of nothing as well.
        assert!(base > 50.0 && after > 50.0, "setup: the panel must be laid out, and it came out {base} and {after}");
        assert!(
            after <= base + 1.0,
            "two hundred nodes widened the tree panel: it was {base}, it became {after} — a long history is paid for with canvas"
        );
    }

    /// A NARROW WINDOW: THE PANEL DOES NOT EAT IT WHOLE.
    ///
    /// On a laptop, and at an interface scale of 150 %, the window is narrow. A tree taking up half of
    /// it leaves a strip of the model — there is nothing to work with.
    #[test]
    fn on_a_narrow_window_the_tree_leaves_room_for_the_model() {
        let (mut app, comp) = part_with_a_body();
        if let Some(c) = app.project.components.iter_mut().find(|c| c.id == comp) {
            c.name = "Lower left load-bearing reinforced bracket variant two".into();
        }
        for zoom in [1.0_f32, 1.25, 1.5] {
            let window = egui::vec2(900.0, 600.0);
            let w = tree_width(&mut app, window, zoom);
            assert!(w > 50.0, "at scale {zoom} the tree panel did not lay out (width {w}) — the check below would be green for nothing");
            assert!(
                w < window.x * 0.5,
                "at scale {zoom} the tree took {w} of {} — a strip of the model is what is left",
                window.x
            );
        }
    }
}
