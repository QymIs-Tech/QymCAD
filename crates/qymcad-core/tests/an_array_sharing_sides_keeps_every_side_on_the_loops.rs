//! AN ARRAY SHARING SIDES KEEPS EVERY SIDE ON THE LOOPS: a square arrayed in copies that lie against
//! it (the step equal to its side) shares its sides with them, and a side lies on the loops of both
//! squares - the way the rebuild round a change reads a loop, by the entities of the loop (`ents_of`)
//! and by the sources of its edges (`edge_src`). Every drawn curve of the sketch is named on some
//! closed loop, and stays there when the count is edited.
//!
//! Reported behaviour: after a linear array of a square with the step equal to its side the shared
//! sides stood on no loop - lit only while picked, and no loop to take a profile by.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{Id, PatternKind, Project};

/// The drawn curves of the sketch no closed loop of it names.
fn orphans(p: &Project, si: usize) -> Vec<Id> {
    let s = &p.sketches[si];
    s.entities
        .iter()
        .filter(|e| !e.construction)
        .filter(|e| {
            !s.contour_ids.iter().any(|&cid| p.contour_index(cid).is_some_and(|ci| p.contours[ci].edge_src.contains(&e.id)) || p.contours.ents_of(cid).is_some_and(|ents| ents.contains(&e.id)))
        })
        .map(|e| e.id)
        .collect()
}

/// Whether some closed loop of the sketch names every curve of `shape`.
fn looped_together(p: &Project, si: usize, shape: &[Id]) -> bool {
    let s = &p.sketches[si];
    s.contour_ids.iter().any(|&cid| {
        p.contour_index(cid).is_some_and(|ci| {
            let c = &p.contours[ci];
            c.closed && shape.iter().all(|q| c.edge_src.contains(q) || p.contours.ents_of(cid).is_some_and(|ents| ents.contains(q)))
        })
    })
}

#[test]
fn a_square_arrayed_in_touching_copies_keeps_every_side_on_the_loops() {
    let mut p = Project::default();
    let si = p.new_sketch("squares");
    let square = p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, Purpose::Real);
    assert!(orphans(&p, si).is_empty(), "setup: the square before the array has curves on no loop");

    // the copies lie against the square and against one another: every inner side is laid twice
    let pid = p.add_pattern(si, &square, PatternKind::Linear { dx: 20.0, dy: 0.0, count: 3, dx2: 0.0, dy2: 0.0, count2: 1 });
    let left = orphans(&p, si);
    assert!(left.is_empty(), "after the array curves {left:?} are on no loop");
    for side in &square {
        let named = p.sketches[si].contour_ids.iter().any(|&cid| match p.contour_index(cid) {
            Some(ci) if p.contours[ci].closed => p.contours[ci].edge_src.contains(side) || p.contours.ents_of(cid).is_some_and(|ents| ents.contains(side)),
            _ => false,
        });
        assert!(named, "side {side} is named on no closed loop");
    }
    assert!(looped_together(&p, si, &square), "the source square keeps no closed loop of its own");

    // EDITING THE COUNT relays the copies: the shared sides are laid anew, and stay on the loops
    let pi = p.sketches[si].patterns.iter().position(|q| q.id == pid).unwrap();
    p.update_pattern(si, pi, PatternKind::Linear { dx: 20.0, dy: 0.0, count: 4, dx2: 0.0, dy2: 0.0, count2: 1 });
    let left = orphans(&p, si);
    assert!(left.is_empty(), "after the count was edited curves {left:?} are on no loop");
    assert!(looped_together(&p, si, &square), "after the count was edited the source square keeps no closed loop of its own");
}

#[test]
fn a_circle_arrayed_across_keeps_its_loops_whole() {
    // A CIRCLE THAT A COPY'S SIDE CROSSES keeps the regions of its own - the fix must not smear the
    // records of the regions it does not touch
    let mut p = Project::default();
    let si = p.new_sketch("squares");
    let square = p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, Purpose::Real);
    let circle = p.add_circle_entity(si, 40.0, 10.0, 5.0, Purpose::Real);
    p.add_pattern(si, &square, PatternKind::Linear { dx: 20.0, dy: 0.0, count: 3, dx2: 0.0, dy2: 0.0, count2: 1 });
    let left = orphans(&p, si);
    assert!(left.is_empty(), "curves {left:?} are on no loop");
    assert!(looped_together(&p, si, &[circle]), "the circle crossed by a copy's side keeps no closed loop of its own");
}
