//! WHAT A PERSON ACTUALLY SEES WHEN A NEWER VERSION EXISTS.
//!
//! Everything else about the check is asked of pure functions and recorded answers, and that kind of
//! green says nothing about what a person can reach. The badge is painted by a real frame here, and it
//! is clicked with pointer events: a direct call to the handler would skip the frame's own reading of
//! the input and prove nothing.
//!
//! WHY THE BUILD HAS TO PRETEND. Nothing compiled on this machine is a release, and by the rule these
//! windows follow - do not show what cannot work - none of this appears in an ordinary build at all. So
//! the check pretends a tag and an answer for the length of a test; see `update_ui::pretend`.
#[cfg(test)]
mod tests {
    use qymcad_update::{InstallState, Latest, Outcome, ReadyInfo};

    const SCREEN: egui::Vec2 = egui::vec2(1400.0, 900.0);

    fn raw() -> egui::RawInput {
        egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)), ..Default::default() }
    }

    fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect)> {
        fn walk(s: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::epaint::Shape::Text(t) => out.push((t.galley.text().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size()))),
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

    fn newer() -> Latest {
        Latest {
            latest: "v9.9.9-dev.20991231".into(),
            url: "https://example.invalid/releases/tag/v9.9.9-dev.20991231".into(),
            published: Some("2099-12-31".into()),
            notes: Some("a line about what changed".into()),
            ..Default::default()
        }
    }

    /// Draw the status line for one frame and hand back what it painted.
    fn status_frame(app: &mut crate::gui::App, ctx: &egui::Context, input: egui::RawInput) -> Vec<(String, egui::Rect)> {
        let out = ctx.run_ui(input, |ui| {
            crate::gui::panels_bars::status_bar(
                &mut qymcad_ui_state::StatusCtx {
                    cache: &app.cache,
                    cursor: None,
                    project: &app.project,
                    scheme: &app.scheme,
                    set: &mut app.set,
                    sketch_ses: &app.sketch_ses,
                    status: "",
                    win: &mut app.win,
                },
                ui,
            );
        });
        texts(&out.shapes)
    }

    /// THE NEWS IS ON SCREEN, AND PRESSING IT LEADS SOMEWHERE.
    ///
    /// A badge that says a new version exists and does nothing when pressed sends people hunting through
    /// menus for the thing they were just told about.
    #[test]
    fn the_badge_says_there_is_a_new_version_and_opens_the_window() {
        crate::gui::update_ui::pretend("v0.1.0-dev.20260828", Outcome::Found(newer()));
        let mut app = crate::gui::screen_keys::tests::populated();
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);

        let painted = status_frame(&mut app, &ctx, raw());
        let said = crate::i18n::tr("update-found");
        let spot = painted
            .iter()
            .find(|(t, _)| t.contains(&said))
            .map(|(_, r)| r.center())
            .unwrap_or_else(|| panic!("the status line does not say a new version exists; it painted: {:?}", painted.iter().map(|(t, _)| t).collect::<Vec<_>>()));
        assert!(painted.iter().any(|(t, _)| t.contains("v9.9.9-dev.20991231")), "the status line says there is a new version without naming it");

        assert!(!app.win.is(qymcad_ui_state::WinKind::Updates), "the window was open before anything was pressed");
        for pressed in [true, false] {
            let ev = egui::RawInput {
                events: vec![egui::Event::PointerMoved(spot), egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }],
                ..raw()
            };
            status_frame(&mut app, &ctx, ev);
        }
        assert!(app.win.is(qymcad_ui_state::WinKind::Updates), "pressing the badge led nowhere");
        crate::gui::update_ui::stop_pretending();
    }

    /// THE WINDOW NAMES BOTH VERSIONS AND OFFERS ONE BUTTON.
    #[test]
    fn the_window_names_the_versions_and_offers_the_page() {
        crate::gui::update_ui::pretend("v0.1.0-dev.20260828", Outcome::Found(newer()));
        let mut app = crate::gui::screen_keys::tests::populated();
        app.win.open(qymcad_ui_state::WinKind::Updates);
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);

        let _ = ctx.run_ui(raw(), |c| crate::gui::panels_windows::updates_dialog(&mut app.win, &app.scheme, c.ctx()));
        let out = ctx.run_ui(raw(), |c| crate::gui::panels_windows::updates_dialog(&mut app.win, &app.scheme, c.ctx()));
        let painted = texts(&out.shapes);
        let all: Vec<&String> = painted.iter().map(|(t, _)| t).collect();

        assert!(painted.iter().any(|(t, _)| t.contains("v9.9.9-dev.20991231")), "the window does not name the new version: {all:?}");
        assert!(painted.iter().any(|(t, _)| t.contains("2099-12-31")), "the window does not say when it came out: {all:?}");
        assert!(painted.iter().any(|(t, _)| t.contains("a line about what changed")), "the window does not say what changed: {all:?}");
        let button = crate::i18n::tr("update-open-page");
        assert!(painted.iter().any(|(t, _)| t.contains(&button)), "the window offers no way to the release page: {all:?}");
        crate::gui::update_ui::stop_pretending();
    }

    /// A CHECK THAT GOT NOWHERE SAYS SO, AND IS NOT DRESSED UP AS GOOD NEWS.
    ///
    /// The whole difference is invisible unless somebody looks: both outcomes leave the screen without a
    /// new version on it. Told as "no updates", a request that never left would leave a person on a
    /// six-month-old build believing they are current.
    #[test]
    fn a_failed_check_is_not_reported_as_being_up_to_date() {
        crate::gui::update_ui::pretend("v0.1.0-dev.20260828", Outcome::Unreachable);
        let mut app = crate::gui::screen_keys::tests::populated();
        app.win.open(qymcad_ui_state::WinKind::Updates);
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);

        let _ = ctx.run_ui(raw(), |c| crate::gui::panels_windows::updates_dialog(&mut app.win, &app.scheme, c.ctx()));
        let out = ctx.run_ui(raw(), |c| crate::gui::panels_windows::updates_dialog(&mut app.win, &app.scheme, c.ctx()));
        let painted = texts(&out.shapes);
        let all: Vec<&String> = painted.iter().map(|(t, _)| t).collect();

        let unreachable = crate::i18n::tr("update-unreachable");
        let none = crate::i18n::tr("update-none");
        assert!(painted.iter().any(|(t, _)| t.contains(&unreachable)), "a failed check said nothing at all: {all:?}");
        assert!(!painted.iter().any(|(t, _)| t.contains(&none)), "a failed check was reported as 'no updates', which is a lie: {all:?}");
        crate::gui::update_ui::stop_pretending();
    }

    /// THE SETTING IS IN THE WINDOW WHEN IT CAN DO SOMETHING, AND ABSENT WHEN IT CANNOT.
    ///
    /// Reported behaviour: "I don't see the update settings under General." The reporter was right and
    /// the program was too - an ordinary `cargo run` is not a release, has nothing to compare against,
    /// and by the rule this feature follows shows none of itself. Written down so that the rule is a
    /// decision on the record rather than a surprise, and so that a change to it is deliberate.
    #[test]
    fn the_setting_appears_only_where_it_can_work() {
        let label = crate::i18n::tr("settings-updates");

        let mut app = crate::gui::App::default();
        app.win.open(qymcad_ui_state::WinKind::Settings);
        let without = crate::gui::screen_keys::tests::frame_text(&mut app, |a, c| {
            let mut asks = Vec::new();
            crate::gui::panels_windows::settings_window(&mut a.win_ctx(&mut asks), c);
            a.do_win_asks(asks, c);
        });
        assert!(!without.iter().any(|t| t.contains(&label)), "a build with no release tag offers a check it has nothing to compare against");

        crate::gui::update_ui::pretend("v0.1.0-dev.20260828", Outcome::Idle);
        let mut app = crate::gui::App::default();
        app.win.open(qymcad_ui_state::WinKind::Settings);
        let with = crate::gui::screen_keys::tests::frame_text(&mut app, |a, c| {
            let mut asks = Vec::new();
            crate::gui::panels_windows::settings_window(&mut a.win_ctx(&mut asks), c);
            a.do_win_asks(asks, c);
        });
        assert!(with.iter().any(|t| t.contains(&label)), "a release build does not offer the setting at all: {with:?}");
        for u in qymcad_ui_state::UpdateCheck::ALL {
            let choice = crate::i18n::tr(u.key());
            assert!(with.iter().any(|t| t.contains(&choice)), "the choice \"{choice}\" is missing from the window: {with:?}");
        }
        crate::gui::update_ui::stop_pretending();
    }

    /// Draw one frame the way the tests below do and hand back what it painted: the texts with
    /// their spots. One name for the shape of it, so the click helper below does not spell the
    /// whole signature out.
    type PaintOneFrame = dyn Fn(&mut crate::gui::App, &egui::Context, egui::RawInput) -> Vec<(String, egui::Rect)>;

    /// Draw the updates window for one frame and hand back what it painted.
    fn updates_frame(app: &mut crate::gui::App, ctx: &egui::Context, input: egui::RawInput) -> Vec<(String, egui::Rect)> {
        // Twice, like the window tests above: the first frame warms the layout, the second is read.
        let _ = ctx.run_ui(input.clone(), |c| crate::gui::panels_windows::updates_dialog(&mut app.win, &app.scheme, c.ctx()));
        let out = ctx.run_ui(input, |c| crate::gui::panels_windows::updates_dialog(&mut app.win, &app.scheme, c.ctx()));
        texts(&out.shapes)
    }

    /// Click the middle of the given spot: a press and a release, the way the badge test does it.
    fn click(app: &mut crate::gui::App, ctx: &egui::Context, spot: egui::Pos2, paint: &PaintOneFrame) {
        for pressed in [true, false] {
            let ev = egui::RawInput {
                events: vec![egui::Event::PointerMoved(spot), egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }],
                ..raw()
            };
            paint(app, ctx, ev);
        }
    }

    /// WHILE THE PACKAGE ARRIVES, THE SCREEN SAYS SO AND NOTHING ELSE.
    ///
    /// Downloading takes a minute on a slow line; a badge that still offers the release page for a
    /// version already on its way would send the person to fetch by hand what is arriving by itself.
    #[test]
    fn while_the_package_arrives_the_screen_says_so() {
        crate::gui::update_ui::pretend("v0.1.0-dev.20260828", Outcome::Found(newer()));
        crate::gui::update_ui::pretend_auto(InstallState::Downloading);
        let mut app = crate::gui::screen_keys::tests::populated();
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);

        let said = crate::i18n::tr("update-downloading");
        let painted = status_frame(&mut app, &ctx, raw());
        assert!(painted.iter().any(|(t, _)| t.contains(&said)), "the status line does not say the update is arriving: {:?}", painted.iter().map(|(t, _)| t).collect::<Vec<_>>());

        app.win.open(qymcad_ui_state::WinKind::Updates);
        let windowed = updates_frame(&mut app, &ctx, raw());
        assert!(windowed.iter().any(|(t, _)| t.contains(&said)), "the window does not say the update is arriving: {:?}", windowed.iter().map(|(t, _)| t).collect::<Vec<_>>());
        crate::gui::update_ui::stop_pretending();
    }

    /// THE WAITING VERSION NAMES ITSELF AND OFFERS THE RESTART.
    ///
    /// The whole scenario the background update exists for: the program fetched the new version
    /// itself, and the one thing left is to restart into it. The release page button stays beside
    /// the restart one - a copy the background could not touch still tells where to go by hand.
    #[test]
    fn the_waiting_version_names_itself_and_offers_the_restart() {
        crate::gui::update_ui::pretend("v0.1.0-dev.20260828", Outcome::Found(newer()));
        crate::gui::update_ui::pretend_auto(InstallState::Ready(ReadyInfo { tag: "v9.9.9-dev.20991231".into(), staged_zip: None }));
        let mut app = crate::gui::screen_keys::tests::populated();
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        assert!(!crate::gui::update_ui::restart_asked(), "a restart was asked before anything was pressed");

        // the status line carries the news itself, and pressing it opens the window
        let painted = status_frame(&mut app, &ctx, raw());
        let said = crate::i18n::tr("update-ready");
        let spot = painted
            .iter()
            .find(|(t, _)| t.contains(&said))
            .map(|(_, r)| r.center())
            .unwrap_or_else(|| panic!("the status line does not carry the background news; it painted: {:?}", painted.iter().map(|(t, _)| t).collect::<Vec<_>>()));
        click(&mut app, &ctx, spot, &status_frame);
        assert!(app.win.is(qymcad_ui_state::WinKind::Updates), "pressing the news led nowhere");

        // the window names the version, offers the restart, and keeps the page button
        let windowed = updates_frame(&mut app, &ctx, raw());
        let all: Vec<&String> = windowed.iter().map(|(t, _)| t).collect();
        assert!(windowed.iter().any(|(t, _)| t.contains(&said)), "the window does not carry the background news: {all:?}");
        let button = crate::i18n::tr("update-restart-now");
        let at = windowed.iter().find(|(t, _)| t.contains(&button)).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("the window offers no restart: {all:?}"));
        let page = crate::i18n::tr("update-open-page");
        assert!(windowed.iter().any(|(t, _)| t.contains(&page)), "the release page button is gone with the restart one: {all:?}");

        click(&mut app, &ctx, at, &updates_frame);
        assert!(crate::gui::update_ui::restart_asked(), "pressing the restart button asked for nothing");
        crate::gui::update_ui::stop_pretending();
    }
}
