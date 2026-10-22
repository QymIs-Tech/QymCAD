//! WHAT IS DONE TO A BODY THAT IS ALREADY THERE: its edges rounded and cut, its inside hollowed, a hole bored, a
//! face pushed, tilted, removed, and the body split.
//!
//! The block every check starts from is 40 by 30 by 10, holding 12000 mm^3. Each check reads what the tool left: the
//! volume, the faces and how far the body reaches.
use qymcad::{Key, Session, Tone};
use qymcad_acceptance::build;
use qymcad_acceptance::probe;

/// The block of the first part, with nothing else in it.
fn a_block() -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    s
}

/// The body of the part, measured: the last one made, which is the one the tools work on.
fn body(s: &mut Session) -> qymcad::Solid {
    s.document().bodies.iter().rfind(|b| !b.consumed && !b.sheet).cloned().unwrap_or_else(|| panic!("the part holds no body"))
}

/// The block with a hole 10 across and 5 deep bored in the middle of its top face.
fn a_block_with_a_hole() -> Session {
    let mut s = a_block();
    take(&mut s, "tb-hole-hint");
    let top = s.face_at([20.0, 15.0, 10.0]);
    s.click(top);
    field(&mut s, "f-diameter", "10");
    field(&mut s, "f-depth", "5");
    s.key(Key::Enter);
    s
}

/// Take the tool whose hint is `hint`.
fn take(s: &mut Session, hint: &str) {
    let hint = s.word(hint);
    s.press_hint(&hint);
}

/// Type `text` into the field under the caption `key` names, wherever it stands.
fn field(s: &mut Session, key: &str, text: &str) {
    let caption = s.word(key);
    s.fill(&caption, text);
}

probe! {
    /// CHAMFER: the edge is cut back by its leg, and the block loses that wedge.
    fn a_chamfer_cuts_the_edge_back_by_its_leg() {
        let mut s = a_block();
        take(&mut s, "tb-chamfer-body-hint");
        let edge = s.edge_at([20.0, 0.0, 10.0]);
        s.click(edge);
        field(&mut s, "f-leg", "2");
        s.key(Key::Enter);
        let now = body(&mut s);
        // a triangle of 2 by 2 taken along the whole 40 of the edge
        assert!((12000.0 - now.volume - 80.0).abs() < 1.0, "a chamfer of 2 along a 40 edge takes 80 away, and the body holds {}", now.volume);
        assert!(now.faces == 7, "the chamfer adds one face to the six of the block: the body has {}", now.faces);
    }
}

probe! {
    /// SHELL: the body is hollowed out to the thickness given, open at the face that was clicked.
    fn a_shell_hollows_the_body_to_the_thickness_given() {
        let mut s = a_block();
        take(&mut s, "tb-shell-hint");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        field(&mut s, "cmd-thickness", "2");
        s.key(Key::Enter);
        let now = body(&mut s);
        // what is left: the block less the cavity of 36 by 26 by 8
        let want = 12000.0 - 36.0 * 26.0 * 8.0;
        assert!((now.volume - want).abs() < 1.0, "a shell of 2 leaves {want}, and the body holds {}", now.volume);
        assert!((now.max[2] - 10.0).abs() < 1e-6, "the shell changed the size of the block: it reaches {}", now.max[2]);
    }
}

probe! {
    /// HOLE: a round hole of the diameter and depth given is bored into the face that was clicked.
    fn a_hole_is_bored_to_the_diameter_and_depth_given() {
        let mut s = a_block();
        take(&mut s, "tb-hole-hint");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        field(&mut s, "f-diameter", "10");
        field(&mut s, "f-depth", "5");
        s.key(Key::Enter);
        let now = body(&mut s);
        let want = 12000.0 - std::f64::consts::PI * 25.0 * 5.0;
        assert!((now.volume - want).abs() < 2.0, "a hole 10 across and 5 deep leaves {want}, and the body holds {}", now.volume);
    }
}

probe! {
    /// PUSH FACE: the face moves by the offset and takes the body with it.
    fn a_pushed_face_carries_the_body_with_it() {
        let mut s = a_block();
        take(&mut s, "tb-push-face-hint");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        field(&mut s, "f-offset", "5");
        s.key(Key::Enter);
        let now = body(&mut s);
        assert!((now.max[2] - 15.0).abs() < 1e-3, "the top face pushed 5 out leaves the block 15 tall, and it reaches {}", now.max[2]);
        assert!((now.volume - 18000.0).abs() < 1.0, "a block 40 by 30 by 15 holds 18000, and the body holds {}", now.volume);
    }
}

probe! {
    /// REMOVE FACE: the boss is taken off again and the block is whole.
    fn a_removed_face_takes_the_boss_with_it() {
        let mut s = a_block();
        // a boss: a 10 by 10 square drawn on the top face and raised 5
        build::sketch_on_face(&mut s, [20.0, 15.0, 10.0]);
        build::draw(&mut s, "tb-rect-hint", &[(10.0, 10.0), (20.0, 20.0)]);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        field(&mut s, "f-length", "5");
        s.key(Key::Enter);
        let raised = body(&mut s).volume;
        assert!((raised - 12500.0).abs() < 1.0, "the boss was not raised: the body holds {raised}");
        take(&mut s, "tb-remove-face-hint");
        // the faces of the feature: the top of the boss and the walls that can be seen from here
        for p in [[15.0, 15.0, 15.0], [20.0, 15.0, 12.5], [15.0, 10.0, 12.5]] {
            let at = s.face_at(p);
            s.click(at);
        }
        s.key(Key::Enter);
        let now = body(&mut s);
        assert!((now.volume - 12000.0).abs() < 1.0, "taking the boss away leaves the block whole at 12000, and it holds {}", now.volume);
    }
}

probe! {
    /// THE FACES OF A HOLE CAN BE CLICKED: a click into a bored hole takes the floor of the hole, not the face it was
    /// bored through - otherwise nothing inside a hole can be picked for the tools that work on faces.
    fn the_faces_of_a_hole_can_be_clicked() {
        let mut s = a_block();
        take(&mut s, "tb-hole-hint");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        field(&mut s, "f-diameter", "10");
        field(&mut s, "f-depth", "5");
        s.key(Key::Enter);
        let holed = body(&mut s);
        assert!(holed.faces == 8, "the hole was not bored: the body has {} faces", holed.faces);
        let refused = qymcad_acceptance::refusal(|| {
            let mut s = a_block_with_a_hole();
            // the floor of a hole 10 across and 5 deep is not seen from the corner the view opens at: the line of sight
            // to its middle meets the top 7 mm off the axis, past the edge of the hole. A person looks down into it
            // first, with the TOP of the cube.
            let top = s.word("view-top");
            let side = s.cube_side(&top).unwrap_or_else(|| panic!("the navigation cube has no side that reads {top:?}"));
            s.click(side);
            let _ = s.face_at([20.0, 15.0, 5.0]);
        });
        assert!(refused.is_empty(), "the floor of the hole cannot be clicked: {refused}");
    }
}

probe! {
    /// SPLIT BODY: the cutting plane leaves two bodies where there was one, and together they hold what the one held.
    fn a_split_body_falls_into_two() {
        let mut s = a_block();
        take(&mut s, "tb-split-body-hint");
        let top = s.face_at([20.0, 15.0, 10.0]); // the face to measure the cut from: 5 below it goes through the middle
        s.click(top);
        field(&mut s, "f-offset", "-5");
        s.key(Key::Enter);
        let bodies: Vec<qymcad::Solid> = s.document().bodies.into_iter().filter(|b| !b.consumed).collect();
        assert!(bodies.len() == 2, "the cut leaves two bodies, and the part holds {}: {:?}", bodies.len(), bodies.iter().map(|b| (b.name.clone(), b.volume)).collect::<Vec<_>>());
        let together: f64 = bodies.iter().map(|b| b.volume).sum();
        assert!((together - 12000.0).abs() < 1.0, "the two pieces hold what the block held, 12000, and they hold {together}");
    }
}

probe! {
    /// SPLIT FACES: the body stays one and its faces are divided where the plane passes.
    fn a_split_face_leaves_one_body_with_more_faces() {
        let mut s = a_block();
        let before = body(&mut s);
        take(&mut s, "tb-split-face-hint");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        field(&mut s, "f-offset", "-5");
        s.key(Key::Enter);
        let now = body(&mut s);
        let bodies = s.document().bodies.into_iter().filter(|b| !b.consumed).count();
        assert!(bodies == 1, "the body was split apart instead of its faces: the part holds {bodies} bodies");
        assert!((now.volume - before.volume).abs() < 1e-6, "dividing the faces changed the body: {} became {}", before.volume, now.volume);
        assert!(now.faces > before.faces, "the faces were not divided: the body still has {}", now.faces);
    }
}

probe! {
    /// DRAFT: the face is tilted by the angle about the neutral face, and the body loses the wedge that makes.
    fn a_draft_tilts_the_face_by_the_angle() {
        let mut s = a_block();
        take(&mut s, "tb-draft-hint");
        let front = s.face_at([20.0, 0.0, 5.0]);
        s.click(front);
        let neutral = s.word("cmd-neutral-face");
        s.press_word_near(&neutral, qymcad::pos2(640.0, 24.0));
        let top = s.face_at([20.0, 15.0, 10.0]); // the face the tilt is measured from, in view from above
        s.click(top);
        field(&mut s, "cmd-angle", "10");
        s.key(Key::Enter);
        let now = body(&mut s);
        // the face leans over the whole height of 10: a wedge of 10 * 10 * tan(10 deg) / 2 along the 40 of the edge
        let wedge = 40.0 * 10.0 * 10.0 * (10.0f64).to_radians().tan() / 2.0;
        let moved = (now.volume - 12000.0).abs();
        assert!((moved - wedge).abs() < wedge * 0.05, "a draft of 10 degrees over a face 40 by 10 moves {wedge} one way or the other, and the body went from 12000 to {}", now.volume);
    }
}

probe! {
    /// THICKEN: the face grows a plate of the thickness given, and the part stays one body.
    fn a_thickened_face_grows_a_plate() {
        let mut s = a_block();
        take(&mut s, "tb-thicken-hint");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        field(&mut s, "f-thickness", "3");
        s.key(Key::Enter);
        let now = body(&mut s);
        assert!((now.volume - 15600.0).abs() < 1.0, "a plate of 3 over a face of 40 by 30 adds 3600 to the 12000, and the body holds {}", now.volume);
        assert!((now.max[2] - 13.0).abs() < 1e-3, "the plate did not go on the top: the body reaches {}", now.max[2]);
    }
}

probe! {
    /// COPY FACE: the face is taken out as a surface of its own, and the body it came from is left as it was.
    fn a_copied_face_becomes_a_surface_of_its_own() {
        let mut s = a_block();
        take(&mut s, "tb-face-copy-hint");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        s.key(Key::Enter);
        let bodies: Vec<qymcad::Solid> = s.document().bodies.into_iter().filter(|b| !b.consumed).collect();
        let sheets: Vec<&qymcad::Solid> = bodies.iter().filter(|b| b.sheet).collect();
        assert!(sheets.len() == 1, "the face was not taken out as a surface: the part holds {:?}", bodies.iter().map(|b| (b.name.clone(), b.sheet)).collect::<Vec<_>>());
        let sheet = sheets[0];
        assert!((sheet.area - 1200.0).abs() < 1.0, "the surface of the 40 by 30 face is 1200, and it is {}", sheet.area);
        let solid = bodies.iter().find(|b| !b.sheet).unwrap_or_else(|| panic!("the block is gone"));
        assert!((solid.volume - 12000.0).abs() < 1e-6, "taking a copy of the face changed the block: it holds {}", solid.volume);
    }
}

/// WHERE THE ROUND SIDE OF THE SHAFT CAN BE CLICKED: a round side is drawn as a ring of flat facets, and only the
/// facets turned towards the eye can be clicked. Which way the view looks is the program's business, so the places
/// round the shaft are tried in turn until one of them can be seen.
fn the_side_of_the_shaft(s: &mut Session, r: f64, z: f64) -> qymcad::Pos2 {
    // the corners of the facets stand on the true radius, their middles a little inside it: both are tried
    for k in 0..(72 * 3) {
        let a = (k % 72) as f64 * std::f64::consts::TAU / 72.0;
        let r = r - 0.05 * (k / 72) as f64;
        let p = [r * a.cos(), r * a.sin(), z];
        let mut seen = None;
        let message = qymcad_acceptance::refusal(|| seen = Some(s.face_at(p)));
        if message.is_empty() {
            if let Some(at) = seen {
                return at;
            }
        }
    }
    panic!("no place on the round side of the shaft can be seen from here");
}

/// A cylinder of radius 10 and height 20 standing on the table, as the primitive makes it.
fn a_cylinder() -> Session {
    let mut s = Session::start();
    build::into_the_first_part(&mut s);
    take(&mut s, "tb-cylinder-hint");
    s.key(Key::Enter);
    s
}

probe! {
    /// A THREAD OF THE STANDARD is cut into the shaft: the cylinder keeps its size and loses the groove, and the
    /// timeline holds the node.
    fn a_standard_thread_is_cut_on_the_shaft() {
        let mut s = a_cylinder();
        let before = body(&mut s);
        take(&mut s, "tb-thread-hint");
        let side = the_side_of_the_shaft(&mut s, 10.0, 10.0);
        s.click(side);
        field(&mut s, "f-nominal-d", "20");
        field(&mut s, "f-length", "10");
        s.key(Key::Enter);
        let now = body(&mut s);
        assert!(now.volume < before.volume, "the thread took nothing off the shaft: it still holds {}", now.volume);
        // the groove cannot take more than the ring down to the root of an M20 thread over 10 of length
        let most = std::f64::consts::PI * (100.0 - 8.5 * 8.5) * 10.0;
        let taken = before.volume - now.volume;
        assert!(taken > most * 0.1 && taken < most, "an M20 thread over 10 of length takes between {} and {most} off, and it took {taken}", most * 0.1);
        assert!((now.max[0] - 10.0).abs() < 1e-3 && (now.max[2] - 20.0).abs() < 1e-3, "the shaft changed its size: it reaches {:?}", now.max);
        let node = s.document().features.last().cloned().unwrap_or_else(|| panic!("the timeline is empty"));
        assert!(node.error.is_none(), "the thread node is red: {node:?}");
    }
}

probe! {
    /// A THREAD OF ONE'S OWN PITCH is not the thread of the standard: the coarser pitch takes a different amount off.
    fn a_thread_of_ones_own_pitch_differs_from_the_standard() {
        let standard = {
            let mut s = a_cylinder();
            let before = body(&mut s).volume;
            take(&mut s, "tb-thread-hint");
            let side = the_side_of_the_shaft(&mut s, 10.0, 10.0);
            s.click(side);
            field(&mut s, "f-nominal-d", "20");
            field(&mut s, "f-length", "10");
            s.key(Key::Enter);
            before - body(&mut s).volume
        };
        let mut s = a_cylinder();
        let before = body(&mut s).volume;
        take(&mut s, "tb-thread-hint");
        let side = the_side_of_the_shaft(&mut s, 10.0, 10.0);
        s.click(side);
        field(&mut s, "f-nominal-d", "20");
        field(&mut s, "f-length", "10");
        field(&mut s, "f-pitch-std", "4");
        s.key(Key::Enter);
        let mine = before - body(&mut s).volume;
        assert!(mine > 0.0, "the thread of one's own pitch took nothing off");
        assert!((mine - standard).abs() > standard * 0.05, "a pitch of 4 takes the same off as the standard pitch ({standard} against {mine}) - the pitch that was typed was not used");
    }
}

/// The sheets of the part - the surfaces standing beside the body.
fn sheets(s: &mut Session) -> Vec<qymcad::Solid> {
    s.document().bodies.into_iter().filter(|b| !b.consumed && b.sheet).collect()
}

/// The block hollowed out with the top open, the walls 2 thick.
fn a_shelled_block() -> Session {
    let mut s = a_block();
    take(&mut s, "tb-shell-hint");
    let top = s.face_at([20.0, 15.0, 10.0]);
    s.click(top);
    field(&mut s, "cmd-thickness", "2");
    s.key(Key::Enter);
    s
}

probe! {
    /// PATCH: a surface is spanned across the edges that were picked, and it covers what they enclose.
    fn a_patch_spans_a_surface_across_the_edges() {
        let mut s = a_shelled_block();
        take(&mut s, "tb-patch-hint");
        for p in [[20.0, 2.0, 10.0], [20.0, 28.0, 10.0], [2.0, 15.0, 10.0], [38.0, 15.0, 10.0]] {
            let at = s.edge_at(p);
            s.click(at);
        }
        s.key(Key::Enter);
        let sheets = sheets(&mut s);
        assert!(sheets.len() == 1, "the patch is not there: the part holds the surfaces {:?}", sheets.iter().map(|b| (b.name.clone(), b.area)).collect::<Vec<_>>());
        // the opening of a box 40 by 30 with walls of 2 is 36 by 26
        assert!((sheets[0].area - 936.0).abs() < 5.0, "the patch over an opening of 36 by 26 is 936, and it is {}", sheets[0].area);
    }
}

probe! {
    /// A PATCH NEEDS MORE THAN ONE EDGE, and says so instead of making a surface out of nothing.
    fn a_patch_across_one_edge_is_refused_in_words() {
        let mut s = a_shelled_block();
        let before = s.document();
        take(&mut s, "tb-patch-hint");
        let at = s.edge_at([20.0, 2.0, 10.0]);
        s.click(at);
        s.key(Key::Enter);
        let why = s.word("msg-patch-needs-edges");
        let said = s.tool_says();
        assert!(said.iter().any(|n| n.tone == Tone::Refusal && n.words.contains(&why)), "nothing says one edge is not a boundary where the patch speaks: {said:?}");
        assert!(s.document().bodies == before.bodies, "a surface was made across a single edge anyway");
    }
}

probe! {
    /// A SURFACE TAKEN OUT OF A FACE CAN BE PICKED AFTERWARDS: it lies exactly on the face it came from, and every
    /// tool that works on surfaces - stitching, trimming, handing one back to the body - has to be able to take it.
    fn a_surface_copied_from_a_face_can_be_picked_afterwards() {
        let mut s = a_block();
        take(&mut s, "tb-face-copy-hint");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        s.key(Key::Enter);
        assert!(sheets(&mut s).len() == 1, "the face was not taken out as a surface");
        take(&mut s, "tb-stitch-hint");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        let said = s.status();
        assert!(said != s.word("msg-stitch-only-sheets"), "the surface lying on the face it came from cannot be picked at all: the program says {said:?}");
    }
}

/// The block with the top face and the front face taken out as two surfaces of 1200 and 400.
fn a_block_and_two_surfaces() -> Session {
    let mut s = a_block();
    for p in [[20.0, 15.0, 10.0], [20.0, 0.0, 5.0]] {
        take(&mut s, "tb-face-copy-hint");
        let at = s.face_at(p);
        s.click(at);
        s.key(Key::Enter);
    }
    s
}

probe! {
    /// STITCH: two surfaces that meet along an edge become one, holding what the two held.
    fn stitched_surfaces_become_one() {
        let mut s = a_block_and_two_surfaces();
        let apart = sheets(&mut s);
        assert!(apart.len() == 2, "the two faces were not taken out: {:?}", apart.iter().map(|b| (b.name.clone(), b.area)).collect::<Vec<_>>());
        let together: f64 = apart.iter().map(|b| b.area).sum();
        take(&mut s, "tb-stitch-hint");
        for p in [[20.0, 15.0, 10.0], [20.0, 0.0, 5.0]] {
            let at = s.face_at(p);
            s.click(at);
        }
        s.key(Key::Enter);
        let now = sheets(&mut s);
        assert!(now.len() == 1, "the surfaces were not stitched into one - the program says {:?}; the part holds {:?}", s.status(), now.iter().map(|b| (b.name.clone(), b.area)).collect::<Vec<_>>());
        assert!((now[0].area - together).abs() < 1.0, "the stitched surface holds what the two held, {together}, and it holds {}", now[0].area);
    }
}

probe! {
    /// REPLACE FACE: a surface is handed back to the body and the face takes its shape.
    fn a_surface_is_handed_back_to_the_body() {
        let mut s = a_block_and_two_surfaces();
        take(&mut s, "tb-surface-replace-hint");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top); // the face of the body that is being replaced
        let again = s.face_at([20.0, 15.0, 10.0]);
        s.click(again); // and the surface to hand back, which lies on it
        let said = s.status();
        s.key(Key::Enter);
        let now = body(&mut s);
        assert!((now.volume - 12000.0).abs() < 1.0 && s.document().features.last().is_some_and(|f| f.error.is_none()), "handing the surface back did not go through: the program says {said:?} and the body holds {}", now.volume);
    }
}

probe! {
    /// TRIM SURFACE: the surface is cut along the body and what is kept is the part that was clicked.
    fn a_surface_is_trimmed_along_the_body() {
        let mut s = a_block_and_two_surfaces();
        // a boss 10 across grown 20 from the plane under the block rises 10 above its top: the body now crosses the
        // top sheet along the circle of the boss - a sheet lying on a face of its own body is not divided by it
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        build::circle(&mut s, (20.0, 15.0), (25.0, 15.0));
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        take(&mut s, "tb-extrude-hint");
        let length = s.word("f-length");
        s.fill(&length, "20").key(Key::Enter);
        let before: f64 = sheets(&mut s).iter().map(|b| b.area).sum();
        take(&mut s, "tb-trim-surface-hint");
        // the first click takes the surface and the side of it to keep, off the boss; the second, the body to cut
        // it with - the program asks for it in words ("now click the body to cut with")
        let kept = s.face_at([5.0, 5.0, 10.0]);
        s.click(kept);
        let boss = s.face_at([20.0, 15.0, 20.0]);
        s.click(boss);
        let said = s.status();
        s.key(Key::Enter);
        let after: f64 = sheets(&mut s).iter().map(|b| b.area).sum();
        let disc = std::f64::consts::PI * 25.0;
        assert!(
            (before - after - disc).abs() < 1.0,
            "the trim cuts the disc of pi x 5^2 = {disc:.1} mm^2 under the boss out of the surfaces of {before:.1}, and they hold {after:.1}: after the click the program says {said:?} and now it says {:?}",
            s.status()
        );
    }
}

probe! {
    /// A THREAD PUT ON A SHORT CYLINDER OPENS NO LONGER THAN THE CYLINDER: on a shaft 10 long the length field holds 10
    /// and Apply can be pressed at once. Reported behaviour: the length opened at one and a half diameters, 30, and was
    /// refused before anything was typed.
    fn a_thread_on_a_short_cylinder_opens_at_its_length() {
        let mut s = qymcad_acceptance::contract::fixtures::Fixture::CylinderBody.start();
        take(&mut s, "tb-thread-hint");
        let side = s.face_at([27.071, 7.929, 5.0]);
        s.click(side);
        let caption = s.word("th-length");
        let length = s.field(&caption).value;
        assert!(length.trim().parse::<f64>().is_ok_and(|v| (v - 10.0).abs() < 1e-6), "the length of a thread on a shaft 10 long opens at {length:?}");
        let apply = s.word("cmd-apply-enter");
        assert!(s.widgets().iter().any(|w| w.label == apply && w.enabled), "Apply cannot be pressed on a thread sized to its cylinder");
    }
}

probe! {
    /// A FACE REPLACED BY A SHEET STANDING OFF IT: the top of the block offset 5 up as a sheet, then the top replaced by
    /// that sheet - the sides reach it, and the block stands 15 tall, 40 x 30 x 15 = 18000.
    fn the_top_replaced_by_its_offset_makes_the_block_taller() {
        let mut s = Session::start();
        build::block(&mut s);
        let offset = s.word("tb-offset-surface-hint");
        s.press_hint(&offset);
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        let distance = s.word("f-distance");
        s.fill(&distance, "5");
        s.key(Key::Enter);
        assert!(s.document().bodies.iter().any(|b| b.sheet && !b.consumed && (b.min[2] - 15.0).abs() < 1e-3), "no sheet stands at z 15: {:?}", s.document().bodies.iter().map(|b| (&b.name, b.sheet, b.min, b.max)).collect::<Vec<_>>());
        s.key(Key::Escape);
        let replace = s.word("tb-surface-replace-hint");
        s.press_hint(&replace);
        // the sheet covers the top from above; seen from the front, the top shows just behind its front edge
        let top = s.face_at([20.0, 2.0, 10.0]);
        s.click(top);
        let sheet = s.face_at([20.0, 15.0, 15.0]);
        s.click(sheet);
        s.key(Key::Enter);
        let doc = s.document();
        let solids: Vec<_> = doc.bodies.iter().filter(|b| !b.sheet && !b.consumed && b.visible).collect();
        assert!(solids.len() == 1 && (solids[0].volume - 18000.0).abs() < 1.0, "the block with its top replaced is {:?}, 40 x 30 x 15 = 18000 was expected; the status line says {:?}", solids.iter().map(|b| (b.volume, b.min, b.max)).collect::<Vec<_>>(), s.status());
    }
}
