//! ALL FOUR RELATIONS ARE MADE BY HAND AND PASS THE MOTION ON.
//!
//! There was exactly one live check of the relations — the gear. The other three (rack, screw,
//! linear) were checked only by the kernel, that is, "the equation is right" rather than "a person
//! assembled a mechanism and it moved". The difference between the two has already cost dearly: the
//! eight kinds of joint were green in the kernel too, and could not be created by hand at all.
//!
//! Here every relation is assembled by the same path a person takes: the mates are placed by TWO
//! CLICKS ON THE FRAME, then the relation tool is taken, the kind and the number are chosen, the
//! degrees are pointed at by clicks, Enter. And THE MAIN THING is measured: the driving degree moved,
//! so the driven one must travel its own, by the right factor.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::{JointKind, RelationKind};
    use qymcad_core::model::Id;

    /// FOUR KINDS OF RELATION — that is how many the tool bar has, and that is how many must be here.
    const KINDS: [RelationKind; 4] = [RelationKind::Gear, RelationKind::RackPinion, RelationKind::Screw, RelationKind::Linear];

    /// A point ON THE BODY that can be clicked: the centre of the topmost face.
    fn aim(app: &App, body: Id) -> [f64; 3] {
        let wt = app.project.body_display_transform(body, app.current_ctx_id_for_test());
        let f = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)))
            .expect("the body has faces");
        qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z])
    }

    /// A PAIR OF PARTS AND A MATE BETWEEN THEM, placed BY HAND. Returns the joint.
    ///
    /// The anchor is "by origins": in that mode a person simply points a finger at the part, and it is
    /// the commonest way to put together a rough mechanism.
    fn a_mate_by_hand(app: &mut App, kind: JointKind, x: f64) -> Id {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, x);
        super::super::joint_flow::tests::add_part_at(app, x + 60.0);
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
        // MOVE THE PAIR APART. The origin of every part is at zero, and the "by origins" anchor puts
        // all the hinges ON ONE AXIS: the mechanism comes out degenerate and says nothing about the
        // joint.
        for (k, b) in mine.iter().enumerate() {
            if let Some(o) = app.project.body_owner(*b) {
                if let Some(i) = app.project.component_index(o) {
                    app.project.components[i].transform = [1.0, 0.0, 0.0, x + k as f64 * 60.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
                }
                if k == 0 {
                    app.project.set_grounded(o, true);
                }
            }
        }
        app.rebuild_if_dirty();
        app.refresh_edges();
        let (pa, pb) = (aim(app, mine[0]), aim(app, mine[1]));
        let mut hand = Hand::new(app);
        hand.look_at([x + 30.0, 10.0, 5.0], 5.0).mate(kind).anchor(3).click(pa).click(pb);
        app.rebuild_if_dirty();
        app.project.joints.last().map(|j| j.id).expect("two clicks on the frame must create the joint")
    }

    /// The reading of a degree of the joint: 0 is the angle, 1 and 2 are the travel.
    fn slot_value(app: &App, joint: Id, slot: usize) -> f64 {
        let j = app.project.joints.iter().find(|x| x.id == joint).expect("the joint is there");
        match slot {
            0 => j.angle,
            1 => j.offset,
            _ => j.offset2,
        }
    }

    fn drive(app: &mut App, joint: Id, slot: usize, v: Option<f64>) {
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == joint) {
            j.drive[slot] = v;
        }
    }

    #[test]
    fn every_one_of_the_four_relations_passes_the_motion_on() {
        let mut checked = 0usize;
        let mut broken: Vec<String> = Vec::new();
        for kind in KINDS {
            let mut app = App::default();
            // A MECHANISM OF ITS OWN FOR EACH KIND: a gear ties two rotations, a rack ties a rotation
            // to a travel, a screw lives INSIDE one cylindrical joint, and a linear one ties two
            // travels.
            let (ja, jb, sa, sb, value, given, want) = match kind {
                RelationKind::Gear => {
                    let a = a_mate_by_hand(&mut app, JointKind::Revolute, 0.0);
                    let b = a_mate_by_hand(&mut app, JointKind::Revolute, 200.0);
                    (a, b, 0usize, 0usize, 2.0, 20.0, 40.0)
                }
                RelationKind::RackPinion => {
                    let a = a_mate_by_hand(&mut app, JointKind::Revolute, 0.0);
                    let b = a_mate_by_hand(&mut app, JointKind::Slider, 200.0);
                    // the number is TRAVEL PER TURN; the drive is a fraction rather than a whole turn:
                    // the relation has a PERIOD, and a whole turn legitimately arrives at an
                    // equivalent solution
                    (a, b, 0usize, 1usize, 10.0, 36.0, 1.0)
                }
                RelationKind::Screw => {
                    let a = a_mate_by_hand(&mut app, JointKind::Cylindrical, 0.0);
                    (a, a, 0usize, 1usize, 5.0, 36.0, 0.5)
                }
                RelationKind::Linear => {
                    let a = a_mate_by_hand(&mut app, JointKind::Slider, 0.0);
                    let b = a_mate_by_hand(&mut app, JointKind::Slider, 200.0);
                    (a, b, 1usize, 1usize, 2.0, 10.0, 20.0)
                }
            };
            let name = crate::i18n::tr(kind.label());

            // A CONTROL: the same driving degree must move WITHOUT the relation. If it does not move
            // here either, the joint is to blame and not the relation.
            {
                let was = slot_value(&app, ja, sa);
                drive(&mut app, ja, sa, Some(was + given));
                app.project.solve_joints();
                let moved = slot_value(&app, ja, sa) - was;
                if (moved - given).abs() > 1e-3 {
                    broken.push(format!("\"{name}\": GUARD — WITHOUT the relation the driving degree must move, and it travelled {moved:.4} of {given}"));
                }
                // BRING THE DRIVING DEGREE BACK TO ZERO. The control left it shifted, and a relation
                // TAKES ITS PHASE at the moment it is created: making it on a mechanism that has slid
                // means measuring the wrong thing. A person does the same: first sets the mechanism up
                // as it should be, then ties it together.
                drive(&mut app, ja, sa, Some(0.0));
                app.project.solve_joints();
                drive(&mut app, ja, sa, None);
                app.project.solve_joints();
            }

            // THE RELATION TOOL: the kind, the number, clicks on the mates, Enter.
            app.start_relation_pick_for_test();
            app.relation_pick_set_for_test(kind, value);
            app.relation_pick_click_for_test(ja);
            if ja != jb {
                app.relation_pick_click_for_test(jb);
            }
            app.relation_pick_confirm_for_test();
            app.rebuild_if_dirty();

            let Some(r) = app.project.relations.last().map(|r| r.id) else {
                broken.push(format!("\"{name}\": the tool did not create a relation; status: {}", app.status));
                checked += 1;
                continue;
            };
            if let Some((_, why)) = app.project.relation_faults().into_iter().find(|(id, _)| *id == r) {
                broken.push(format!("\"{name}\": the relation was born faulty — {why}"));
                checked += 1;
                continue;
            }

            // THE DRIVING DEGREE MOVED — THE DRIVEN ONE MUST TRAVEL ITS OWN.
            //
            // A DRIVE IS AN ABSOLUTE POSITION FROM THE ZERO OF THE JOINT, not an increment: the drive
            // goes from the present reading, otherwise "travelled" is reckoned from somebody else's
            // point. That was the first mistake here — the rack and the screw were declared broken
            // when all that differed was the origin of the reckoning.
            let (was_a, was_b) = (slot_value(&app, ja, sa), slot_value(&app, jb, sb));
            drive(&mut app, ja, sa, Some(was_a + given));
            app.project.solve_joints();
            let (moved_a, moved_b) = (slot_value(&app, ja, sa) - was_a, slot_value(&app, jb, sb) - was_b);
            if (moved_a - given).abs() > 1e-3 {
                broken.push(format!("\"{name}\": the driving degree was driven by {given} and it travelled {moved_a:.4}"));
            } else if (moved_b.abs() - want).abs() > 1e-3 {
                broken.push(format!("\"{name}\": the driving degree travelled {moved_a:.4}, the driven one must travel {want}, and it travelled {:.4}", moved_b.abs()));
            }
            drive(&mut app, ja, sa, None);
            checked += 1;
        }
        assert_eq!(checked, 4, "GUARD: there are four kinds of relation, and {checked} were checked");
        assert!(broken.is_empty(), "relations cannot be assembled by hand, or do not pass the motion on:\n{}", broken.join("\n"));
    }
}
