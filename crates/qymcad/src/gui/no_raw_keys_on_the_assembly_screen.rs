//! NO INTERNAL CODES ON THE ASSEMBLY SCREEN — A GUARD FOR THE WHOLE CLASS.
//!
//! A screenshot had already been sent once where a dropdown read `joint-kind-rigid` instead of a word.
//! That trouble was closed, and it came back from another side: the name of a joint was stored as a
//! catalogue key and printed past the translator, and the popup held a code again. That was found BY
//! CHANCE, while working on something else.
//!
//! The reason it came back is that the existing guards look for keys in the CALLS to the translator —
//! that is, they check that every key has words. They do not see the opposite case: a key that never
//! reached the translation at all. Such a thing is visible only IN THE FRAME.
//!
//! So what is asked here is not about keys but about THE PICTURE: not one drawn string may look like
//! an internal code. That catches the whole class at once, including the places where the code is not
//! there yet.
//!
//! WHAT THIS GUARD HOLDS AND WHAT IT DOES NOT was found out by damaging the code rather than claimed.
//! It holds THE ASSEMBLY PANEL: the list of joints, relations, connectors, groups and grounding. The
//! joint POPUP does not enter that frame (it lives in a window of its own), and a code can be put back
//! into it without reddening anything here — so the popup is guarded by a separate check in
//! `a_joint_can_hold_what_it_finds.rs`, and that one does go red on this damage. Two checks for one
//! class is not a luxury: the trouble has already come back once from another side.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::{AnchorRef, JointKind, RelationKind};

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// Does the string look like an internal catalogue code: lower-case Latin with hyphens.
    ///
    /// Interface words look like that in no language: Russian ones are Cyrillic, English ones start
    /// with a capital or hold spaces. And the keys look exactly like that: `joint-kind-rigid`,
    /// `j-anchors`, `name-connector-n`, `r-fault-mate-lost`.
    fn looks_like_a_key(t: &str) -> bool {
        // A NAME IS STORED AS `key#argument`, and untranslated it reaches the frame WHOLE. The first
        // edition of the guard parsed the string as it was — the "n#1" part stopped being alphabetic
        // and the key was not recognised. The guard stayed silent on exactly the trouble it exists
        // for; it was found by measuring a frame. The argument is cut off and the key itself is
        // judged.
        //
        // WORDS ARE JUDGED RATHER THAN THE WHOLE STRING: a key arrives in the frame together with a
        // glyph, and checking the whole string did not recognise it. That was the second misfire of
        // the guard in a row, and both were found only by measuring a frame: first it did not
        // understand the argument after the hash, then the neighbouring glyph.
        let ok = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        t.split_whitespace().any(|w| {
            let w = w.split('#').next().unwrap_or("");
            w.contains('-') && w.split('-').all(ok) && w.starts_with(|c: char| c.is_ascii_lowercase())
        })
    }

    /// An assembly with everything on screen at once: a joint, a relation, a group and a standalone
    /// connector.
    fn a_screen_with_everything(app: &mut App) -> u64 {
        let ([ja, jb], [wheel_a, wheel_b]) = super::super::a_relation_is_made_by_hand::tests::two_hinges(app);
        app.project.add_relation(RelationKind::Gear, ja, 0, jb, 0, 2.0);
        app.project.add_group(&[wheel_a, wheel_b]);
        app.project.add_connector_standalone(wheel_a, AnchorRef::Origin);
        let ca = app.project.add_connector(wheel_a, AnchorRef::Origin);
        let cb = app.project.add_connector(wheel_b, AnchorRef::Origin);
        app.project.add_joint(ca, cb, JointKind::Rigid);
        ja
    }

    /// The words drawn by the assembly panel.
    fn drawn(app: &mut App, jid: u64) -> Vec<String> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        app.workbench = super::super::Workbench::Assembly;
        app.mode_3d = true;
        app.joint.edit = Some(jid);
        let mut texts = Vec::new();
        for _ in 0..2 {
            app.joint.edit = Some(jid);
            let out = ctx.run(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                egui::CentralPanel::default().show(c, |ui| app.joints_panel_for_test(ui));
            });
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }
        texts
    }

    /// A NAME FROM A DOCUMENT, NOT ONLY FROM A FRESH JOINT.
    ///
    /// The scene of the guard was assembled here and now, so every name in it was of the present shape
    /// and always translated. And then a screenshot came from a real machine where the list read
    /// "joint-kind-slider 5": the document is old, and the name in it is stored in the former shape —
    /// the key and the number separated by a SPACE. One space is enough for the string to stop being a
    /// key as a whole, the name translator gave up and printed it as it was.
    ///
    /// So the scene now holds such a name too: the check must see what a person sees when they open a
    /// document made before us.
    fn with_a_name_from_an_old_document(app: &mut App, jid: u64) {
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == jid) {
            j.name = "joint-kind-slider 5".into();
        }
    }

    #[test]
    fn nothing_on_the_assembly_screen_looks_like_a_catalogue_key() {
        let mut app = App::default();
        let jid = a_screen_with_everything(&mut app);
        with_a_name_from_an_old_document(&mut app, jid);
        let texts = drawn(&mut app, jid);
        assert!(texts.len() > 20, "GUARD: the screen drew suspiciously few lines ({}) — there was nothing to check", texts.len());

        let bad: Vec<&String> = texts.iter().filter(|t| looks_like_a_key(t)).collect();
        assert!(
            bad.is_empty(),
            "the assembly screen shows internal codes instead of words: {bad:?}\neverything drawn: {texts:?}"
        );
    }

    /// THE SAME IN THE JOINT POPUP AND IN THE TOOL BARS.
    ///
    /// The popup lives in a window of its own and does not enter the common frame; the bars are
    /// separate panels too. Each needs a frame OF ITS OWN with a fresh environment: trying to draw them
    /// together with the panel gave emptiness, and the guard would have stayed silent having checked
    /// nothing.
    #[test]
    fn neither_the_joint_popup_nor_the_tool_bars_show_catalogue_keys() {
        let mut app = App::default();
        let jid = a_screen_with_everything(&mut app);
        with_a_name_from_an_old_document(&mut app, jid);
        app.workbench = super::super::Workbench::Assembly;
        app.mode_3d = true;

        // every kind of screen in a pass of its own, with a FRESH egui environment
        let screens: [(&str, fn(&mut App)); 4] = [
            ("the joint popup", |_app: &mut App| {}),
            ("the connector bar", |app: &mut App| app.joint.conn_pick = true),
            ("the relation bar", |app: &mut App| app.joint.relation_pick = Some(Default::default())),
            ("the joint assembling bar", |app: &mut App| app.joint.pick_faces = true),
        ];
        let mut bad: Vec<String> = Vec::new();
        let mut seen = 0usize;
        for (name, arm) in screens {
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            app.joint.conn_pick = false;
            app.joint.relation_pick = None;
            app.joint.pick_faces = false;
            arm(&mut app);
            let popup = name == "the joint popup";
            let mut texts = Vec::new();
            for _ in 0..2 {
                app.joint.edit = Some(jid);
                let out = ctx.run(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                    if popup {
                        app.joint_popup_for_test(c, viewport());
                    } else {
                        app.joint_tool_bar(c);
                    }
                });
                texts.clear();
                for cs in &out.shapes {
                    super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
                }
            }
            assert!(!texts.is_empty(), "GUARD: \"{name}\" drew nothing — there was nothing to check");
            seen += 1;
            bad.extend(texts.iter().filter(|t| looks_like_a_key(t)).map(|t| format!("{name}: {t}")));
        }
        assert_eq!(seen, 4, "GUARD: four kinds of screen were listed, and {seen} were checked");
        assert!(bad.is_empty(), "internal codes are shown instead of words: {bad:?}");
    }

    /// THE SENTINEL ITSELF IS CHECKED: it must recognise a code when one appears.
    ///
    /// Without this, green would mean only that the recognition finds nothing at all.
    #[test]
    fn the_sentinel_recognises_a_key_when_it_sees_one() {
        assert!(looks_like_a_key("joint-kind-rigid"), "an obvious key must be recognised");
        assert!(looks_like_a_key("j-anchors"), "a short key must be recognised");
        assert!(looks_like_a_key("name-joint-kind-rigid-n"), "a name key must be recognised");
        // the Cyrillic below is the test data itself: an interface word must not be taken for a key
        assert!(!looks_like_a_key("Жёсткое 3"), "a Russian word is not a key");
        assert!(!looks_like_a_key("Hold as it stands"), "an English caption is not a key");
        assert!(!looks_like_a_key("0.0 mm"), "a number is not a key");
        assert!(!looks_like_a_key("A:"), "an anchor label is not a key");
        assert!(looks_like_a_key("name-joint-kind-rigid-n#3"), "a key WITH AN ARGUMENT must be recognised: that is exactly how a name reaches the frame");
        assert!(looks_like_a_key("\u{e2e2} name-joint-kind-rigid-n#3"), "a key NEXT TO A GLYPH must be recognised — that is exactly how it arrives in the frame");
    }
}
