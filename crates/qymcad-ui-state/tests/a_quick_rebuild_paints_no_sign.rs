//! A QUICK REBUILD PAINTS NO SIGN, AND A SIGN PAINTED DOES NOT FLASH OFF: the decision of the frame over the start of
//! the rebuild and the time of the frame, at the moments either side of both limits.
use qymcad_ui_state::{rebuild_sign, RebuildLook, RebuildRunning, RebuildSign, REBUILD_SIGN_AFTER, REBUILD_SIGN_HOLDS};
use std::time::{Duration, Instant};

const MS: Duration = Duration::from_millis(1);

fn quiet(started: Instant) -> RebuildRunning {
    RebuildRunning { started, look: RebuildLook::Quiet }
}

/// The sign of every frame from `t0`, one every `step`, while a rebuild runs from `t0` to `lands`, and on for as long
/// again as a sign could be up after it. Answers the frames painted, as their offsets from `t0`.
fn painted(lands: Duration) -> Vec<Duration> {
    let until = lands + REBUILD_SIGN_AFTER + 2 * REBUILD_SIGN_HOLDS;
    let t0 = Instant::now();
    let step = 16 * MS;
    let mut shown: Option<RebuildSign> = None;
    let mut out = Vec::new();
    let mut at = Duration::ZERO;
    while at <= until {
        let running = (at < lands).then(|| quiet(t0));
        shown = rebuild_sign(running.as_ref(), shown, t0 + at);
        if shown.is_some() {
            out.push(at);
        }
        at += step;
    }
    out
}

#[test]
fn a_rebuild_shorter_than_the_limit_paints_no_frame() {
    for lands in [20 * MS, 50 * MS, REBUILD_SIGN_AFTER - MS] {
        let frames = painted(lands);
        assert!(frames.is_empty(), "a rebuild of {lands:?} painted its sign at {frames:?} - the window blinks");
    }
}

#[test]
fn a_long_rebuild_is_painted_from_the_limit_on_and_held_after_it_lands() {
    let lands = REBUILD_SIGN_AFTER + 40 * MS;
    let frames = painted(lands);
    let first = *frames.first().expect("a rebuild past the limit painted nothing - a person is not told the picture is stale");
    assert!(first >= REBUILD_SIGN_AFTER && first < REBUILD_SIGN_AFTER + 16 * MS, "the sign went up at {first:?}, the limit is {REBUILD_SIGN_AFTER:?}");
    let last = *frames.last().expect("frames");
    assert!(last - first + 16 * MS >= REBUILD_SIGN_HOLDS, "the sign went up at {first:?} and down at {last:?}: it flashed off before {REBUILD_SIGN_HOLDS:?}");
    assert!(last < first + REBUILD_SIGN_HOLDS + 16 * MS, "the sign stayed to {last:?}, long after the rebuild landed at {lands:?}");
    let gaps: Vec<_> = frames.windows(2).filter(|w| w[1] - w[0] > 16 * MS).collect();
    assert!(gaps.is_empty(), "the sign went out and back on: {gaps:?}");
}

#[test]
fn a_sign_held_for_long_goes_when_its_rebuild_lands() {
    let lands = REBUILD_SIGN_AFTER + 2 * REBUILD_SIGN_HOLDS;
    let frames = painted(lands);
    let last = *frames.last().expect("frames");
    assert!(last < lands, "the sign was up {:?} and still stood {:?} after its rebuild landed", lands - REBUILD_SIGN_AFTER, last + 16 * MS - lands);
}

/// A RUN OF EDITS: a sign painted for one rebuild stays up through the next one, though that one is young.
#[test]
fn a_sign_up_stays_through_the_next_rebuild() {
    let t0 = Instant::now();
    let shown = rebuild_sign(Some(&quiet(t0)), None, t0 + REBUILD_SIGN_AFTER).expect("setup: the first rebuild is painted");
    let next = quiet(t0 + REBUILD_SIGN_AFTER + 5 * MS);
    let held = rebuild_sign(Some(&next), Some(shown.clone()), t0 + REBUILD_SIGN_AFTER + 10 * MS);
    assert!(held.is_some_and(|s| s.since == shown.since), "the sign went out between two rebuilds of a run of edits");
}
