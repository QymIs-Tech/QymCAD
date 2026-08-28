//! THE START SCREEN DOES NOT STAND BETWEEN A PERSON AND THEIR GEOMETRY.
//!
//! A screen that can cover somebody's work is a modal that gets closed without being read, and its
//! usefulness drops to nothing on the very first day. So what is checked here is not "is it drawn" but
//! WHEN it is not drawn: on a non-empty document and on an opened file — never, even with the flag
//! raised.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// On an empty program the screen is visible — otherwise it is useless.
    #[test]
    fn a_fresh_launch_shows_it() {
        let app = App::default();
        assert!(app.start_screen_visible(), "on a blank slate the start screen must be visible");
    }

    /// GEOMETRY APPEARED, SO THE SCREEN IS GONE, even with the flag raised.
    #[test]
    fn it_never_covers_a_document_that_has_something_in_it() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        app.apply_feat_cmd();
        assert!(!app.project.timeline.is_empty(), "setup: the timeline must hold something");

        app.set_show_start_for_test(true);
        assert!(!app.start_screen_visible(), "the screen covered a document with geometry — that is how it gets closed without being read");
    }

    /// A FILE IS OPEN, SO THE SCREEN IS GONE. A person came to work, not to choose where to begin.
    #[test]
    fn an_opened_project_hides_it() {
        let mut app = App::default();
        app.set_project_path("/tmp/some-file.qcad".into());
        app.set_show_start_for_test(true);
        assert!(!app.start_screen_visible(), "with a file open the start screen has no place on the screen");
    }

    /// THE SCREEN GOES OUT ON AN ACTION AND DOES NOT COME BACK BY ITSELF.
    #[test]
    fn it_goes_on_an_action_and_stays_gone() {
        let mut app = App::default();
        assert!(app.start_screen_visible());
        app.set_show_start_for_test(false);
        assert!(!app.start_screen_visible(), "a closed screen must stay closed");
        // and on that same empty document it does not raise itself
        assert!(!app.start_screen_visible(), "the screen raised itself — that is how it becomes a modal that gets closed without being read");
    }

    /// THE MENU ITEM FOR THE START SCREEN WORKS. Reported behaviour: clicking it opens nothing.
    ///
    /// The rule "the screen is never over somebody's work" is written for the one that raises ITSELF —
    /// an unasked-for modal over somebody's part is what gets closed without being read. It has no
    /// right to cancel an explicit request: a person pressed the menu item and got NOTHING, not even a
    /// message saying why.
    #[test]
    fn asking_for_the_start_screen_from_the_menu_opens_it_even_over_work() {
        let mut app = super::super::screen_keys::tests::plate();
        assert!(!app.project.timeline.is_empty(), "setup: the document holds work");
        assert!(!app.start_screen_visible(), "setup: by itself the screen does not raise over work");

        app.ask_start_screen_for_test(); // the same thing the menu item does
        assert!(app.start_screen_visible(), "it was asked for from the menu and the screen did not open; that is the reported complaint");
    }

    /// ...AND IT CLOSES rather than sticking open over the document.
    #[test]
    fn the_asked_screen_closes_and_stays_closed() {
        let mut app = super::super::screen_keys::tests::plate();
        app.ask_start_screen_for_test();
        assert!(app.start_screen_visible());
        app.set_show_start_for_test(false);
        assert!(!app.start_screen_visible(), "a screen closed after being asked for must stay closed");
    }

    #[test]
    fn a_new_assembly_document_has_no_leftover_part() {
        let mut app = App::default();
        app.new_assembly_project_for_test();
        let root = app.project.root;
        let parts = app.project.components.iter().filter(|c| c.parent == Some(root)).count();
        assert_eq!(parts, 0, "an assembly document must start empty, and it holds {parts} part(s) to be thrown away");
        assert_eq!(app.project.active_ctx(), root, "the root of the assembly must be the active one");
    }

    /// A NEW PART, THE OTHER WAY ROUND, COMES WITH A PART: one has to draw somewhere.
    #[test]
    fn a_new_part_document_starts_inside_a_part() {
        let mut app = App::default();
        app.new_project_for_test();
        let root = app.project.root;
        let parts = app.project.components.iter().filter(|c| c.parent == Some(root)).count();
        assert_eq!(parts, 1, "a part document must start with one part, and it came out {parts}");
        assert_ne!(app.project.active_ctx(), root, "the part must be the active one rather than the root: nothing can be drawn in the root");
    }

    /// THE START SCREEN SAYS NOT A WORD ABOUT MACHINING while the module is off.
    ///
    /// A check over the source: the screen is assembled from catalogue keys, and a key of the CAM
    /// dictionary on it is exactly the innards nobody should see.
    #[test]
    fn the_start_screen_says_nothing_about_machining() {
        let src = include_str!("start_screen.rs");
        let cam_keys: Vec<String> = {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("the root of the repository").join("i18n/en/cam.ftl");
            std::fs::read_to_string(dir)
                .expect("the CAM dictionary reads")
                .lines()
                .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.to_string()))
                .filter(|k| k.starts_with(|c: char| c.is_ascii_lowercase()))
                .collect()
        };
        assert!(cam_keys.len() > 50, "suspiciously few CAM keys were collected: {}", cam_keys.len());
        for k in &cam_keys {
            assert!(!src.contains(&format!("\"{k}\"")), "the start screen shows the machining string \"{k}\" — with CAM off those are the innards");
        }
    }

    /// THE SCREEN FITS INSIDE THE WINDOW. Written from a reported screenshot.
    ///
    /// Reported behaviour: pressing the home screen menu item opened something that spilled outside
    /// the application. The window swelled to the whole screen and hung over its edges: the captions
    /// clipped, the contents scattered.
    ///
    /// The cause is of the same kind as the tree panel that used to scatter: inside stood elements
    /// that in a vertical layout demand ALL the available width, and what is available inside a
    /// horizontal row is the width of the screen. The window honestly grew to the request.
    #[test]
    fn the_start_screen_fits_inside_the_window() {
        let mut app = App::default();
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1600.0, 1000.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        // MANY FRAMES RATHER THAN TWO: a window that asks for more than it has grows ONE FRAME AT A
        // TIME — exactly the way the tree panel used to slide. Over two frames the difference is not
        // noticeable yet.
        let mut out = ctx.run_ui(input.clone(), |c| app.start_screen(c.ctx()));
        for _ in 0..30 {
            out = ctx.run_ui(input.clone(), |c| app.start_screen(c.ctx()));
        }

        // THE SIZE IS ASKED OF THE WINDOW ITSELF rather than worked out from the clips of the shapes:
        // the background of the window has a clip across the whole screen, and the first version of the
        // check measured that — so it always read "the whole screen", both before the fix and after.
        // Such a check proves nothing.
        let _ = &out;
        // READ WHAT THE CODE PUBLISHED, not egui's own identifier. Rebuilding that identifier by hand is
        // what broke this check on the upgrade: a window derives it from `Atoms::text()`, which returns an
        // `Option`, and hashing an `Option` is not hashing a string.
        let rect: egui::Rect = ctx
            .data(|d| d.get_temp(egui::Id::new(super::super::start_screen::START_RECT)))
            .expect("the start screen must be on the screen");
        let size = rect.size();
        assert!(size.x > 100.0, "the start screen was not drawn at all ({} px)", size.x);
        assert!(
            size.x <= screen.width() * 0.75,
            "the start screen took {} px on a screen of {} — it spills out of the window and the captions clip",
            size.x,
            screen.width()
        );
        assert!(size.y <= screen.height() * 0.9, "the start screen took {} px in height on a screen of {}", size.y, screen.height());
    }
}
