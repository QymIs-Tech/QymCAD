//! ALL EIGHT KINDS OF JOINT ARE MADE BY HAND.
//!
//! The acceptance matrix in `qymcad-testkit` calls `add_joint` directly — that is, it checks THE
//! SOLVER and not the path a person takes. Walking that path by hand failed: a click on a part was
//! declared a miss, and a connector on an edge was born dead. Neither of those could the matrix catch
//! even in principle.
//!
//! Here every kind is created the way a person creates it: the tool button, the choice of kind, the
//! anchor switch, TWO CLICKS ON THE FRAME — and the joint must be born alive.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::JointKind;
    use qymcad_core::model::Id;

    /// ALL EIGHT KINDS — that is how many the joint bar has, and that is how many must be here.
    const KINDS: [JointKind; 8] = [
        JointKind::Rigid,
        JointKind::Revolute,
        JointKind::Slider,
        JointKind::Cylindrical,
        JointKind::Planar,
        JointKind::Ball,
        JointKind::PinSlot,
        JointKind::Parallel,
    ];

    /// Two parts in the root: a grounded one and a free one. Returns their bodies.
    fn two_parts(app: &mut App) -> (Id, Id) {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
        if let Some(o) = app.project.body_owner(mine[0]) {
            app.project.set_grounded(o, true);
        }
        (mine[0], mine[1])
    }

    /// Where to aim on the body for the chosen anchor mode: 0 face is the centre of the top one,
    /// 1 edge and 2 vertex are a point on the longest edge, 3 origin is any point of the body.
    fn aim(app: &App, body: Id, mode: u8) -> [f64; 3] {
        let wt = app.project.body_display_transform(body, app.current_ctx_id_for_test());
        let p = |q: [f64; 3]| qymcad_core::feature::apply12(&wt, q);
        if mode == 0 || mode == 3 {
            let f = app.project.regen_faces.get(&body).and_then(|fs| {
                fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).map(|f| [f.centroid.x, f.centroid.y, f.centroid.z])
            });
            return p(f.expect("the body has faces"));
        }
        let cached = app.body_edges_cached(body).expect("the body has edges in the live B-rep");
        let mut best: Option<(f64, [f64; 3])> = None;
        for (poly, id) in cached.0.iter().zip(cached.1.iter().copied()) {
            if id == 0 || poly.len() < 2 {
                continue;
            }
            let q = |v: &[f32; 3]| p([v[0] as f64, v[1] as f64, v[2] as f64]);
            let (u, v) = (q(&poly[0]), q(&poly[poly.len() - 1]));
            let d = [v[0] - u[0], v[1] - u[1], v[2] - u[2]];
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            // A VERTEX is the END of an edge, not its middle: that is where mode 2 aims.
            let at = if mode == 2 { u } else { q(&poly[poly.len() / 2]) };
            if best.map_or(true, |(bl, _)| len > bl) {
                best = Some((len, at));
            }
        }
        best.expect("the body has edges with persistent ids").1
    }

    #[test]
    fn every_one_of_the_eight_kinds_is_born_alive() {
        let mut checked = 0usize;
        let mut broken: Vec<String> = Vec::new();
        for kind in KINDS {
            // for the coaxial kinds a person points at an AXIS (an edge), for the rest at a face —
            // that is what the bar suggests too
            let mode = if matches!(kind, JointKind::Revolute | JointKind::Cylindrical | JointKind::PinSlot) { 1 } else { 0 };
            let mut app = App::default();
            let (ba, bb) = two_parts(&mut app);
            let (pa, pb) = (aim(&app, ba, mode), aim(&app, bb, mode));

            let mut hand = Hand::new(&mut app);
            hand.look_at([30.0, 10.0, 5.0], 7.0).mate(kind).anchor(mode).click(pa).click(pb);
            app.rebuild_if_dirty();

            let name = crate::i18n::tr(kind.label());
            let Some(j) = app.project.joints.last().map(|j| j.id) else {
                broken.push(format!("\"{name}\": two clicks on the frame did NOT create a joint; status: {}", app.status));
                checked += 1;
                continue;
            };
            if let Some((_, why)) = app.project.joint_faults().into_iter().find(|(id, _)| *id == j) {
                broken.push(format!("\"{name}\": the joint was born faulty — {why}"));
                checked += 1;
                continue;
            }
            // A FRESH JOINT DRIVES NOTHING — that is what the contract of the tool itself says:
            // untouched fields do not count as a setting, and a joint is born with its degrees free.
            // The contract was being broken: the angle in the bar came prefilled with 90 degrees, and
            // EVERY joint was born demanding a 90-degree turn that nobody asked for. On a slider that
            // pinned a degree it does not even have, and it made a gear relation unsatisfiable — the
            // whole mechanism froze.
            if let Some(x) = app.project.joints.iter().find(|x| x.id == j) {
                if x.drive.iter().any(|d| d.is_some()) {
                    broken.push(format!("\"{name}\": a fresh joint already DRIVES something: {:?}", x.drive));
                }
            }

            // AND IT MUST MOVE. A free degree is free for a reason: set a value and the part stirs. A
            // mechanism that does not move is no better to a person than a broken one.
            let owner = app.project.body_owner(bb).expect("the owner of the driven part");
            let free: Vec<usize> = (0..3).filter(|s| kind.free_slots().get(*s).copied().unwrap_or(false)).collect();
            for slot in free {
                // THE WHOLE PLACEMENT IS MEASURED rather than the shift of the origin: with a
                // rotational degree the origin of the part may lie ON THE AXIS and not move a hair
                // while the part has turned. The first edition measured only the translation and
                // declared four kinds out of eight broken.
                let before = app.project.world_transform(owner);
                // THE FIRST DEGREE IS AN ANGLE, the rest are travel (see `AsmJoint::primitives`); a
                // ball joint has all three rotational.
                let rot = slot == 0 || matches!(kind, JointKind::Ball);
                let v = if rot { 20.0 } else { 12.0 };
                if let Some(x) = app.project.joints.iter_mut().find(|x| x.id == j) {
                    x.drive[slot] = Some(v);
                }
                app.project.solve_joints();
                let now = app.project.world_transform(owner);
                let d = before.iter().zip(now.iter()).map(|(x, y)| (x - y).abs()).fold(0.0f64, f64::max);
                if d < 1e-6 {
                    broken.push(format!("\"{name}\": degree {slot} is free, {v} was set, and the part did not stir"));
                }
                if let Some(x) = app.project.joints.iter_mut().find(|x| x.id == j) {
                    x.drive[slot] = None;
                }
            }
            checked += 1;
        }
        assert_eq!(checked, 8, "GUARD: there are eight kinds of joint, and {checked} were checked");
        assert!(broken.is_empty(), "cannot be made by hand, or are born dead:\n{}", broken.join("\n"));
    }
}
