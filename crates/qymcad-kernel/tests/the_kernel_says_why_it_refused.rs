//! WHEN THE KERNEL REFUSES, IT NAMES THE PLACE.
//!
//! An operation that fails returns nothing, and there are 250 places inside the kernel that can produce that
//! nothing. Which one it was could only be found by bisecting five thousand lines of C++, and that cost hours
//! on every geometric defect - in a project whose own rule is that a silent failure is the worst answer.
//!
//! What is checked here is not that the operations fail - they are asked for the impossible, so of course
//! they do - but that the failure ARRIVES WITH A NAME. Two kinds are covered, because they come from
//! different places: a guard inside the kernel that turns the request away, and an exception thrown by OCCT,
//! whose own words are then carried out.
use qymcad_kernel::{clear_kernel_refusal, last_kernel_refusal, Shape};
use qymcad_core::feature::{LoftBody, LoftWalls};

fn rod() -> Shape {
    Shape::cylinder(5.0, 20.0).expect("the rod")
}

/// A groove profile good enough to sweep, as (r, z) pairs.
const PROFILE: [f64; 6] = [0.0, 0.0, 0.5, 0.0, 0.5, 0.5];

/// A GUARD INSIDE THE KERNEL names itself.
///
/// A shell has five separate dead ends - nothing asked for, no such face, the offset would not take, the
/// result came out empty, the walls did not join - and every one of them used to leave as the same nothing.
/// Here the shell is asked to open a body without naming a single face to open.
#[test]
fn a_shell_with_nothing_to_open_says_which_dead_end_it_was() {
    clear_kernel_refusal();
    let out = rod().shell(-1.0, &[], &[]);
    assert!(out.is_none(), "a shell with no face to open cannot be built");
    let why = last_kernel_refusal().expect("the kernel named the place it refused at");
    assert!(why.starts_with("shell/"), "the refusal names the place inside the shell: {why}");
    assert!(why.len() > "shell/".len() + 4, "and says something about it, not just the place: {why}");
}

/// AN EXCEPTION FROM OCCT carries the kernel's own words out.
///
/// A helix along a direction of zero length: the kernel throws with a message that names the actual trouble.
/// That message used to be caught as `...` and dropped on the floor in 71 places.
#[test]
fn a_helix_along_no_direction_carries_the_kernels_own_words() {
    clear_kernel_refusal();
    let out = rod().helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 5.0, &PROFILE, 10.0, 1.0, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0);
    assert!(out.is_none(), "there is no helix along a direction of no length");
    let why = last_kernel_refusal().expect("the kernel named the place it refused at");
    assert!(why.starts_with("helical profile:"), "the refusal names the operation: {why}");
    assert!(why.to_lowercase().contains("zero norm"), "and carries what the kernel itself said: {why}");
}

/// THE COMMONEST REFUSAL IN DAILY WORK names what went stale, with the counts.
///
/// A face is opened by the name a feature above in the timeline minted for it. Edit that feature and the name
/// may belong to nothing any more - and until now that came back as plain nothing, on the very path a person
/// meets most often. The counts are part of the answer: asked for so many, the body has so many of its own,
/// which is what tells a stale name from a body carrying no names at all.
#[test]
fn a_shell_of_a_face_the_body_lacks_says_so_and_counts_them() {
    clear_kernel_refusal();
    let out = rod().shell(-1.0, &[4242], &[]);
    assert!(out.is_none(), "there is no such face to open");
    let why = last_kernel_refusal().expect("the kernel named the place it refused at");
    assert!(why.starts_with("shell/faces:"), "the refusal names the dead end: {why}");
    assert!(why.contains(" 1 named faces asked for"), "it says how many were asked for: {why}");
    assert!(why.contains("named faces of its own"), "and how many the body carries: {why}");
}

/// THE SAME ANSWER FOR THE EVERYDAY OPERATIONS, not only for the six hot ones.
///
/// A fillet keeps the names of the edges it rounds. Edit a feature above it and the names may belong to
/// nothing - the case a person meets most often after the shell's. It used to come back as plain nothing.
#[test]
fn a_fillet_of_an_edge_the_body_lacks_says_so_and_counts_them() {
    clear_kernel_refusal();
    let out = rod().fillet_edges(1.0, &[4242]);
    assert!(out.is_none(), "there is no such edge to round");
    let why = last_kernel_refusal().expect("the kernel named the place it refused at");
    assert!(why.starts_with("fillet/edges:"), "the refusal names the dead end: {why}");
    assert!(why.contains(" 1 named edges asked for"), "it says how many were asked for: {why}");
    assert!(why.contains("named edges of its own"), "and how many the body carries: {why}");
}

/// EVERY DEAD END OF THE HELIX HAS ITS OWN WORDS, not one refusal covering five different mistakes.
#[test]
fn each_impossible_helix_says_which_of_its_conditions_failed() {
    let ask = |lead: f64, prof: &[f64]| {
        clear_kernel_refusal();
        let out = rod().helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, prof, 10.0, lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0);
        assert!(out.is_none(), "the helix cannot be built");
        last_kernel_refusal().expect("the kernel named the place it refused at")
    };
    let no_lead = ask(0.0, &PROFILE);
    assert!(no_lead.contains("lead is zero"), "a helix of no pitch says exactly that: {no_lead}");
    let no_profile = ask(1.0, &[]);
    assert!(no_profile.contains("fewer than two points"), "a helix of no profile says exactly that: {no_profile}");
    assert_ne!(no_lead, no_profile, "two different mistakes do not share one answer");
}

/// A REFUSAL BEFORE OCCT IS STILL A REFUSAL: the guards on the Rust side speak into the same channel.
///
/// A sweep with no path and a loft through one section never reach the kernel at all - they are turned away
/// one layer above it. To whoever is looking for the cause that difference does not exist, and a second
/// channel for these would be one nobody thinks to read.
#[test]
fn a_request_refused_before_the_kernel_names_itself_in_the_same_channel() {
    let ident = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

    clear_kernel_refusal();
    assert!(Shape::sweep_profile(&PROFILE, &ident, &[], &ident).is_none(), "there is no sweep without a path");
    let why = last_kernel_refusal().expect("the guard named itself");
    assert!(why.starts_with("sweep/asked:"), "the refusal names the place: {why}");
    assert!(why.contains("path"), "and says what was missing: {why}");

    clear_kernel_refusal();
    assert!(Shape::loft_sections(&PROFILE, &[0, 6], &ident, LoftWalls::Smooth, LoftBody::Solid).is_none(), "one section is not a loft");
    let why = last_kernel_refusal().expect("the guard named itself");
    assert!(why.starts_with("loft/asked:"), "the refusal names the place: {why}");
    assert!(why.contains("two sections"), "and says what it needed: {why}");
}

/// A REFUSAL IS FORGOTTEN ONCE READ, so that an old one is never handed to a later failure as its reason.
#[test]
fn an_old_refusal_is_not_offered_as_the_reason_for_a_new_one() {
    clear_kernel_refusal();
    assert!(rod().shell(-1.0, &[], &[]).is_none());
    assert!(last_kernel_refusal().is_some(), "the refusal was recorded");
    clear_kernel_refusal();
    assert_eq!(last_kernel_refusal(), None, "and cleared away afterwards");
}

/// THE REQUESTS A PERSON ACTUALLY MAKES AND THE KERNEL CANNOT GRANT, with what it answers to each.
///
/// The list is chosen by what people run into - a face or an edge that a feature above in the timeline has
/// taken away, a contour of two points, a plane that misses the body - not by walking the code. Two readers
/// use it: the survey below, which prints, and the rule below that, which demands words.
fn refusals_a_person_meets() -> Vec<(&'static str, bool, Option<String>)> {
    let mut out: Vec<(&'static str, bool, Option<String>)> = Vec::new();
    let mut say = |name: &'static str, refused: bool| out.push((name, refused, last_kernel_refusal()));
    let ident = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

    clear_kernel_refusal();
    say("a shell thicker than the body", rod().shell(-50.0, &[], &[]).is_none());

    clear_kernel_refusal();
    say("a shell of a face the body lacks", rod().shell(-1.0, &[4242], &[]).is_none());

    clear_kernel_refusal();
    say("a helix of no lead at all", rod().helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, &PROFILE, 10.0, 0.0, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0).is_none());

    clear_kernel_refusal();
    say("a helix along no direction at all", rod().helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 5.0, &PROFILE, 10.0, 1.0, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0).is_none());

    clear_kernel_refusal();
    say("a helix of no profile at all", rod().helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, &[], 10.0, 1.0, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0).is_none());

    clear_kernel_refusal();
    say("a sweep with no path", Shape::sweep_profile(&PROFILE, &ident, &[], &ident).is_none());

    clear_kernel_refusal();
    say("a loft through one section", Shape::loft_sections(&PROFILE, &[0, 6], &ident, LoftWalls::Smooth, LoftBody::Solid).is_none());

    clear_kernel_refusal();
    say("an extrude of two points", Shape::extrude(&[0.0, 0.0, 1.0, 1.0], 5.0).is_none());

    clear_kernel_refusal();
    say("a chamfer of an edge the body lacks", rod().chamfer_edges(1.0, &[4242], &[], &[], &[]).is_none());

    clear_kernel_refusal();
    say("a draft with no face named", rod().draft_faces(&[], 5.0, [0.0, 0.0, 1.0], [0.0; 3], [0.0, 0.0, 1.0], &[]).is_none());

    clear_kernel_refusal();
    say("a face removed that the body lacks", rod().remove_faces(&[4242]).is_none());

    clear_kernel_refusal();
    say("a face pushed that the body lacks", rod().push_face(4242, 1.0).is_none());

    clear_kernel_refusal();
    say("a face thickened that the body lacks", rod().thicken_face(4242, 1.0, &[], &[]).is_none());

    clear_kernel_refusal();
    say("faces split by a plane that misses the body", rod().split_faces([0.0, 0.0, 500.0], [0.0, 0.0, 1.0]).is_none());

    clear_kernel_refusal();
    say("a patch bounded by one edge that the body lacks", rod().patch(&[4242], false, 0).is_none());

    clear_kernel_refusal();
    say("a thread of no pitch", rod().thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 10.0, 0.0, 60.0, 0.5, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 0.0, 0.0).is_none());

    // NOT a refusal, and worth keeping in the survey for exactly that: the profile above is a chain of points
    // rather than the loop encoding the helix wants, and the kernel says so by name instead of failing later
    // on a body that is quietly wrong.
    clear_kernel_refusal();
    say("a helix given points that are not a loop", rod().helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, &PROFILE, 10.0, 1.0, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0).is_none());

    // THE KERNEL DOES NOT REFUSE THIS ONE, and that is the point of the line: two bodies that do not touch
    // give an EMPTY result rather than nothing at all, and emptiness is judged a layer above, where it becomes
    // `CoreError::EmptyResult`. Anyone hunting a silent refusal here would be hunting the wrong thing.
    clear_kernel_refusal();
    let far = Shape::cylinder(5.0, 20.0).and_then(|c| c.transformed(&[1.0, 0.0, 0.0, 500.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0])).expect("a rod far away");
    say("the common part of two bodies that do not touch (expected NOT to refuse)", rod().boolean(&far, 2).is_none());

    drop(say);
    out
}

/// Run by hand: what the kernel says to each of them, in its own words.
#[test]
#[ignore = "a survey: prints what every impossible request is answered with"]
fn what_the_kernel_says_when_asked_for_the_impossible() {
    for (name, refused, why) in refusals_a_person_meets() {
        eprintln!("{name}: refused={refused}, why={why:?}");
    }
}

/// NOT ONE OF THEM ANSWERS WITH SILENCE.
///
/// The survey above was a print for a long time, and a print is read by whoever remembers to run it. This is
/// the same list as a rule: every refusal a person can meet carries words, and a place that goes quiet again
/// turns this red instead of waiting to be noticed.
///
/// The one line that is expected NOT to refuse is held to that too: two bodies that do not touch give an
/// EMPTY result rather than nothing at all, and emptiness is judged a layer above. If that ever starts
/// refusing, the rule says so.
#[test]
fn every_refusal_a_person_meets_carries_words() {
    let mut silent: Vec<String> = Vec::new();
    for (name, refused, why) in refusals_a_person_meets() {
        let expected_to_refuse = !name.contains("NOT to refuse");
        if refused != expected_to_refuse {
            silent.push(format!("\"{name}\": refused={refused}, and that is not what this case is for"));
            continue;
        }
        if !refused {
            continue;
        }
        match why {
            None => silent.push(format!("\"{name}\": refused without a single word")),
            // "shell/faces" alone names the place and says nothing about it; the words come after the colon.
            Some(w) if w.split_once(": ").map_or(true, |(_, rest)| rest.trim().len() < 8) => {
                silent.push(format!("\"{name}\": the answer names a place but says nothing: {w:?}"))
            }
            Some(_) => {}
        }
    }
    assert!(silent.is_empty(), "refusals that a person meets and cannot understand:\n  {}", silent.join("\n  "));
}
