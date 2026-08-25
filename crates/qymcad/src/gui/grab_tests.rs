//! THE PICK RADII ARE REDUCED TO ROLES AND OBEY THE SETTING.
//!
//! There used to be 42 thresholds hard-coded in place. They were called "different for no reason" —
//! a census made that more precise: between roles the difference is legitimate (a point is harder to
//! aim at than a line), the arbitrariness is WITHIN a role. One and the same vertex was caught from
//! 8, 9, 10, 13 and 18 pixels in six different functions.
#[cfg(test)]
mod tests {
    use super::super::grab::{precision_factor, Grab};
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// NOT ONE HARD-CODED THRESHOLD IS LEFT — and a new one will not slip through.
    ///
    /// The same means that closed the colours: while a radius is a number in place, every new tool
    /// will bring one of its own and the aiming will drift apart again. There is exactly one exception
    /// and it is named: the layout of joint glyphs spreads them 16 px apart — that is the DISTANCE
    /// BETWEEN the badges, not a pick radius.
    #[test]
    fn no_pick_radius_is_a_number_in_place() {
        let mut sins: Vec<String> = Vec::new();
        for (name, src) in [("pick.rs", include_str!("pick.rs")), ("sketching.rs", include_str!("sketching.rs"))] {
            let code = src.split("#[cfg(test)]\nmod ").next().expect("the working part");
            for (i, line) in code.lines().enumerate() {
                let t = line.trim_start();
                // THE LAYOUT OF THE JOINT BADGES IS NOT PICKING: it is the distance BETWEEN the
                // badges. The exception used to be recorded by the word `guard` appearing on the
                // line, that is by an accident of formatting, and went red the moment the lines were
                // split. It now goes by the NAME of the constant — by meaning.
                if t.starts_with("//") || line.contains("BADGE_") {
                    continue;
                }
                let mut rest = line;
                while let Some(p) = rest.find(|c| c == '<' || c == '>') {
                    let after = &rest[p..];
                    let num: String = after.trim_start_matches(['<', '>', '=', ' ']).chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                    if let Ok(v) = num.parse::<f64>() {
                        if num.contains('.') && v >= 4.0 {
                            sins.push(format!("{name}:{}: {}", i + 1, t.chars().take(90).collect::<String>()));
                        }
                    }
                    rest = &after[1..];
                }
            }
        }
        assert!(
            sins.is_empty(),
            "a pick radius is again typed as a number in place ({}) — add a role in `grab.rs` and ask `self.grab(...)`:\n{}",
            sins.len(),
            sins.join("\n")
        );
    }

    /// THE ROLES ARE ORDERED BY MEANING rather than by an accident of typing.
    #[test]
    fn the_roles_are_ordered_the_way_aiming_works() {
        assert!(Grab::Point.base() > Grab::Curve.base(), "a point is harder to aim at than a line — its radius must be the larger one");
        assert!(Grab::Label.base() >= Grab::Point.base(), "missing a caption is the most irritating of all — it cannot be narrower than a point");
        assert!(Grab::Guide.base() < Grab::Curve.base(), "a guide runs across half the screen — a generous radius would take clicks away from the geometry");
        for g in [Grab::Point, Grab::Curve, Grab::Label, Grab::Guide, Grab::Snap] {
            assert!(g.base() > 0.0 && g.base() < 40.0, "the radius of role {g:?} is outside common sense: {}", g.base());
        }
    }

    /// THE AIMING PRECISION REALLY DOES CHANGE THE RADIUS, and in the right direction.
    #[test]
    fn the_precision_setting_scales_every_role() {
        assert!(precision_factor(0) < precision_factor(1), "precise must be narrower than normal");
        assert!(precision_factor(2) > precision_factor(1), "coarse must be wider than normal");
        let mut app = App::default();
        for g in [Grab::Point, Grab::Curve, Grab::Label, Grab::Guide, Grab::Snap] {
            app.set_pick_precision_for_test(1);
            let normal = app.grab(g);
            app.set_pick_precision_for_test(0);
            assert!(app.grab(g) < normal, "precise did not narrow the radius of role {g:?}");
            app.set_pick_precision_for_test(2);
            assert!(app.grab(g) > normal, "coarse did not widen the radius of role {g:?}");
        }
    }

    /// THE SETTING REACHES THE PICKING ITSELF — checked BY A HIT, not by arithmetic.
    ///
    /// The numbers add up for a setting that was never wired in too. So a real sketch point is taken
    /// and clicked PAST at a distance lying between precise and coarse: with wide aiming the point
    /// must be caught, with narrow aiming it must not.
    #[test]
    fn the_setting_actually_reaches_the_picking() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        // a line gives two sketch POINTS — its end at the origin is what is caught
        app.project.add_line_entity(si, 0.0, 0.0, 20.0, 0.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.sel = Sel::Sketch(si);
        assert!(app.project.sketches[si].points.iter().any(|p| p.x.abs() < 1e-9 && p.y.abs() < 1e-9), "setup: there is no point at the origin");

        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let at = app.to_screen(rect, qymcad_core::geom::Point2::new(0.0, 0.0));
        // 12 px past: precise gives 7, coarse gives 15
        let miss = egui::pos2(at.x + 12.0, at.y);

        app.set_pick_precision_for_test(0);
        assert!(app.nearest_sketch_point(rect, miss, si).is_none(), "with precise aiming a click 12 px away must not catch the point (radius {})", app.grab(Grab::Point));
        app.set_pick_precision_for_test(2);
        assert!(app.nearest_sketch_point(rect, miss, si).is_some(), "with coarse aiming a click 12 px away must catch the point (radius {})", app.grab(Grab::Point));
    }
}
