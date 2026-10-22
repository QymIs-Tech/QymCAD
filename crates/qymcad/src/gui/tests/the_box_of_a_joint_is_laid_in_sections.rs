//! THE BOX OF A JOINT IS LAID IN SECTIONS: the degree at the top, a section for each part with its anchor's fields, the
//! limits folded.
//!
//! Reported behaviour: the box of a joint laid its nine fields one under another, then its switches and buttons in a
//! heap after them, and what belonged to part A, to part B and to the limits could not be read.
#[cfg(test)]
pub(crate) mod tests {
    use crate::gui::hand::Hand;
    use crate::gui::App;
    use qymcad_core::model::Id;

    /// Two hinges in an assembly, the first with both limits on its angle - a turn with one limit has no travel to run -
    /// and opened for editing, as a double click on its row opens it; the view in space.
    pub(crate) fn a_hinge_edited(app: &mut App) -> Id {
        let ([ja, _], _) = crate::gui::a_relation_is_made_by_hand::tests::two_hinges(app);
        if let Some(j) = app.project.joints.iter_mut().find(|j| j.id == ja) {
            j.limit_min[0] = Some(-10.0);
            j.limit_max[0] = Some(80.0);
        }
        crate::gui::enter_joint_edit(&mut app.side.joint, &mut app.chosen.sel, &mut app.status, ja);
        app.viewing.mode_3d = true;
        ja
    }

    /// Where `word` is written inside `area`, the last frame drawn.
    fn written_in(hand: &Hand, word: &str, area: egui::Rect) -> Option<egui::Rect> {
        hand.words_drawn().iter().filter(|(t, r)| t == word && area.contains_rect(*r)).map(|(_, r)| *r).next()
    }

    /// THE FIELDS OF PART A STAND IN ITS SECTION AND THOSE OF PART B IN THEIRS; THE LIMITS, FOLDED, SHOW NO FIELD UNTIL
    /// THEIR TITLE IS CLICKED.
    #[test]
    fn the_fields_of_each_part_stand_in_its_section_and_the_limits_unfold() {
        let mut app = App::default();
        a_hinge_edited(&mut app);
        let mut hand = Hand::new(&mut app);
        hand.frame(Vec::new()).frame(Vec::new());
        let area = hand.area_rect(egui::Id::new("joint_edit_popup")).expect("the box of the joint is drawn");
        let title = |hand: &Hand, tag: &str| {
            hand.words_drawn().iter().filter(|(t, r)| t.starts_with(&crate::i18n::tr2("box-part-of", "tag", tag, "name", "")) && area.contains_rect(*r)).map(|(_, r)| *r).next()
        };
        let (a, b) = (title(&hand, "A").expect("the section of part A is in the box"), title(&hand, "B").expect("the section of part B is in the box"));
        let limits = crate::i18n::tr("box-limits");
        let folded = written_in(&hand, &limits, area).expect("the section of the limits is in the box");
        assert!(a.top() < b.top() && b.top() < folded.top(), "the sections stand out of order: A at {a:?}, B at {b:?}, the limits at {folded:?}");
        // the offsets of each part on one line, under its title and above the next one
        let tags: Vec<egui::Rect> = hand.words_drawn().iter().filter(|(t, r)| t == "X" && area.contains_rect(*r)).map(|(_, r)| *r).collect();
        assert!(tags.iter().any(|r| r.top() > a.top() && r.top() < b.top()), "the offset of part A does not stand in its section: X at {tags:?}, A at {a:?}, B at {b:?}");
        assert!(tags.iter().any(|r| r.top() > b.top() && r.top() < folded.top()), "the offset of part B does not stand in its section: X at {tags:?}, B at {b:?}");
        let fields = hand.fields_in(area);
        assert!(fields.iter().all(|f| f.top() < folded.top()), "a field stands under the folded limits: {fields:?}, the limits at {folded:?}");
        let shown = fields.len();
        // the title unfolds its section: the field of the lower limit comes up under it
        hand.press_word(&limits, folded.center());
        hand.frame(Vec::new());
        let area = hand.area_rect(egui::Id::new("joint_edit_popup")).expect("the box of the joint is drawn");
        let fields = hand.fields_in(area);
        assert!(fields.len() > shown, "a click on the title of the limits showed no field: {shown} before, {} after", fields.len());
        assert!(fields.iter().any(|f| f.top() > folded.top()), "the field of the limit does not stand under its title: {fields:?}");
    }

    /// THE BOX ENDS AS EVERY TOOL DOES, Apply and Cancel, with the run of the angle at its foot beside "keep as it stands".
    #[test]
    fn the_box_ends_with_apply_and_cancel_and_runs_its_angle() {
        let mut app = App::default();
        a_hinge_edited(&mut app);
        let mut hand = Hand::new(&mut app);
        hand.frame(Vec::new()).frame(Vec::new());
        let area = hand.area_rect(egui::Id::new("joint_edit_popup")).expect("the box of the joint is drawn");
        let apply = written_in(&hand, &crate::i18n::tr("cmd-apply-enter"), area);
        let cancel = written_in(&hand, &crate::i18n::tr("cmd-cancel-esc"), area);
        assert!(apply.is_some() && cancel.is_some(), "the box does not end with Apply and Cancel: Apply {apply:?}, Cancel {cancel:?}");
        assert!(written_in(&hand, &crate::i18n::tr("cmd-close-esc"), area).is_none(), "the box of a joint ends with Close, where it ends with Apply and Cancel");
        let run = written_in(&hand, &crate::i18n::tr("j-anim-angle"), area).expect("the run of the angle is at the foot of the box");
        let keep = written_in(&hand, &crate::i18n::tr("j-as-built"), area).expect("\"keep as it stands\" is at the foot of the box");
        assert!((run.center().y - keep.center().y).abs() < 30.0, "the run of the angle does not stand by \"keep as it stands\": {run:?}, {keep:?}");
        // Apply and Cancel stand apart from the foot, under a line, and centred across the box
        let (apply, cancel) = (apply.unwrap_or(egui::Rect::NOTHING), cancel.unwrap_or(egui::Rect::NOTHING));
        assert!(apply.top() - keep.bottom() > 12.0, "Apply stands {:.1} px under the foot of the box, stuck to it", apply.top() - keep.bottom());
        let (middle, of_box) = ((apply.left() + cancel.right()) / 2.0, area.center().x);
        assert!((middle - of_box).abs() < 6.0, "Apply and Cancel are not centred across the box: their middle at {middle:.1}, the box's at {of_box:.1}");
    }

    /// HOW AN EDIT OF A JOINT IS LEFT, after its angle was typed.
    #[derive(Clone, Copy, Debug)]
    enum Leave {
        Cancel,
        Esc,
        Apply,
    }

    /// THE DOCUMENT AND ITS HISTORY before an edit and after it: the key of the document and the steps it can undo.
    struct Left {
        key_was: u64,
        key_now: u64,
        steps_was: usize,
        steps_now: usize,
    }

    /// The hinge opened for editing, its angle typed as 37 and the edit left by `how`.
    fn the_angle_typed_and_left(how: Leave) -> Left {
        let mut app = App::default();
        a_hinge_edited(&mut app);
        let mut hand = Hand::new(&mut app);
        hand.frame(Vec::new()).frame(Vec::new());
        let (key_was, steps_was) = (qymcad_ui_state::doc_key(&hand.app.project), hand.app.disk.edits.undo.len());
        assert!(hand.type_after_word(&crate::i18n::tr("j-row-angle"), "37"), "the angle of the hinge has no field in its box");
        hand.frame(Vec::new()).frame(Vec::new());
        assert_ne!(qymcad_ui_state::doc_key(&hand.app.project), key_was, "the angle typed did not reach the joint");
        let area = hand.area_rect(egui::Id::new("joint_edit_popup")).expect("the box of the joint is drawn");
        match how {
            Leave::Cancel => assert!(hand.press_word(&crate::i18n::tr("cmd-cancel-esc"), area.center()), "no Cancel in the box"),
            Leave::Apply => assert!(hand.press_word(&crate::i18n::tr("cmd-apply-enter"), area.center()), "no Apply in the box"),
            // the first Esc lets go of the field being typed in, as in every box; the second cancels
            Leave::Esc => {
                hand.key(egui::Key::Escape).key(egui::Key::Escape);
            }
        }
        hand.frame(Vec::new()).frame(Vec::new());
        assert!(hand.app.side.joint.edit.is_none(), "{how:?} did not leave the edit of the joint");
        Left { key_was, key_now: qymcad_ui_state::doc_key(&hand.app.project), steps_was, steps_now: hand.app.disk.edits.undo.len() }
    }

    /// CANCEL AND Esc TAKE BACK WHAT THE EDIT CHANGED, and leave no step of it in the history; Apply keeps it as a step.
    #[test]
    fn cancel_takes_the_edit_of_a_joint_back_and_apply_keeps_it() {
        for how in [Leave::Cancel, Leave::Esc] {
            let left = the_angle_typed_and_left(how);
            assert_eq!(left.key_now, left.key_was, "{how:?} left the joint as it was edited");
            assert_eq!(left.steps_now, left.steps_was, "{how:?} left steps of the edit in the history");
        }
        let left = the_angle_typed_and_left(Leave::Apply);
        assert_ne!(left.key_now, left.key_was, "Apply took the edit of the joint back");
        assert!(left.steps_now > left.steps_was, "Apply left no step of the edit in the history");
    }

    /// A PICTURE OF THE BOX, for the eye: written to `target/joint-box.png` - the hinge anchored on the top faces of its
    /// parts, so its anchors have a point along their axis to choose.
    #[test]
    fn a_picture_of_the_box_of_a_joint() {
        let mut app = App::default();
        let ja = a_hinge_edited(&mut app);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let ends = app.project.joints.iter().find(|j| j.id == ja).map(|j| [j.a, j.b]).expect("the hinge");
        for cid in ends {
            let Some(owner) = app.project.connector(cid).map(|c| c.owner) else { continue };
            let top = app
                .project
                .bodies
                .iter()
                .find(|b| app.project.body_owner(b.id) == Some(owner))
                .and_then(|b| app.project.regen_faces.get(&b.id).and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.9).map(|f| (b.id, f.clone()))));
            if let Some((body, f)) = top {
                let key = qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
                app.project.set_connector_anchor(cid, owner, qymcad_core::feature::AnchorRef::FaceCenter(body, key));
            }
        }
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
        let bg = app.scheme.pal.viewport_bg();
        let img = crate::gui::help_raster::shot_ui([900, 700], bg, |ui| {
            let ctx = ui.ctx().clone();
            qymcad_ui_state::apply_theme(&mut app.scheme, &app.set, &ctx);
            qymcad_assembly::joint_popup(&mut app.joint_ctx(), &ctx, rect);
        });
        let png = crate::gui::color_image_to_png(&img).expect("the picture encodes");
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/joint-box.png");
        std::fs::write(&out, png).expect("the picture is written");
    }
}
