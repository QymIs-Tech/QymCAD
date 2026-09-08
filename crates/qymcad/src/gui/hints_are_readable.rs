//! THE HINTS ARE BIG ENOUGH TO READ.
//!
//! Reported behaviour: "the hint fonts are small - at scale 1 and at 1.2 alike. There are hints like that
//! all over the CAD, in the horizontal toolbar, in the threads for instance - very small, unreadable, and
//! raising the interface scale does not change them. Make them a bit larger everywhere, and make them
//! scale with the setting too."
//!
//! MEASURED BEFORE FIXING, because one of the two observations had to be checked. A hint took 10 points
//! against 15 for ordinary text - two thirds. The interface scale is `set_zoom_factor`, which multiplies
//! points into pixels, so it grows BOTH by the same amount: on screen the hint does get larger, but its
//! share of the text around it never changes, however far the scale is raised. That is what "they do not
//! change" is about, and it is right.
//!
//! WHY THE SHARE AND NOT THE SIZE. An absolute number is easy to satisfy without making anything readable:
//! raise the hint and the body together and the check goes green over a screen that reads exactly as badly
//! as before. The share is what a person's eye actually judges.
#[cfg(test)]
mod tests {
    /// How small a hint may be against the ordinary text beside it.
    ///
    /// The measured start was 0.67 (10 against 15), which is the reported unreadable. Two thirds is about
    /// where a line stops being read and starts being skipped.
    const LEAST_SHARE: f32 = 0.8;

    /// The height of a hint and of ordinary text, in points, laid out by a real frame.
    fn hint_and_body(zoom: f32) -> (f32, f32) {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        ctx.set_zoom_factor(zoom);
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let (mut hint, mut body) = (0.0, 0.0);
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
            hint = ui.label(egui::RichText::new("hint").weak().small()).rect.height();
            body = ui.label("ordinary").rect.height();
        });
        (hint, body)
    }

    /// A HINT IS NOT MUCH SMALLER THAN THE TEXT BESIDE IT.
    #[test]
    fn a_hint_is_readable_next_to_ordinary_text() {
        let (hint, body) = hint_and_body(1.0);
        let share = hint / body;
        assert!(
            share >= LEAST_SHARE,
            "a hint is {hint} points against {body} for ordinary text - {share:.2} of it, and below {LEAST_SHARE} a line stops being read and starts being skipped"
        );
    }

    /// AND IT STILL GROWS WITH THE INTERFACE SCALE.
    ///
    /// MEASURED IN PHYSICAL PIXELS, which is what an eye judges. Points are the wrong unit for this
    /// question twice over: they are the same at every scale by definition, and they are rounded to whole
    /// pixels, so at 1.2 an 11-point hint lays out as 13 pixels and reads back as 10.83 points. The first
    /// edition of this check compared points and failed on that rounding - a check measuring the wrong
    /// thing, which is worse than none because it looks like a finding.
    #[test]
    fn the_hint_still_follows_the_interface_scale() {
        let px = |zoom: f32| {
            let (hint, _) = hint_and_body(zoom);
            hint * zoom
        };
        let (one, big) = (px(1.0), px(1.5));
        assert!(
            big > one * 1.4,
            "raising the scale to 1.5 took the hint from {one} pixels to {big}: the size must be in POINTS, or the scale does not reach it"
        );
    }

    /// AND IT STAYS READABLE AT EVERY SCALE.
    ///
    /// The share is what the report is about: raising the scale grows the hint and the text around it
    /// together, so a hint too small at one scale is too small at all of them. Walked over several scales
    /// rather than two, because a rounding of whole pixels moves the share a little and one pair could
    /// land on a flattering pair of numbers.
    #[test]
    fn the_hint_stays_readable_at_every_scale() {
        let mut bad = Vec::new();
        for zoom in [0.75f32, 1.0, 1.2, 1.5, 2.0] {
            let (hint, body) = hint_and_body(zoom);
            let share = hint / body;
            if share < LEAST_SHARE {
                bad.push(format!("at {zoom}: {hint} against {body}, {share:.2}"));
            }
        }
        assert!(bad.is_empty(), "a hint drops below {LEAST_SHARE} of the text beside it, and no scale rescues that:\n{}", bad.join("\n"));
    }
}
