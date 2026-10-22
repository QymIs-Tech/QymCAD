//! A SET OF ORDINARY AND CONSTRUCTION CURVES TURNS CONSTRUCTION WHOLE, whichever was chosen first, and turns ordinary
//! only once all of it is construction.
//!
//! Reported behaviour: a polygon - six sides and its construction circle - picked circle first turned all ordinary, and
//! picked side first turned all construction.
use qymcad_core::feature::Purpose;
use qymcad_core::model::Project;

#[test]
fn the_state_does_not_hang_on_the_order_of_the_pick() {
    let mut p = Project::default();
    let si = p.new_sketch("mixed");
    let circle = p.add_circle_entity(si, 0.0, 0.0, 7.0, Purpose::Construction);
    let side = p.add_line_entity(si, 7.0, 0.0, 3.5, 6.062, Purpose::Real);
    let construction = |p: &Project| p.sketches[si].entities.iter().map(|e| e.construction).collect::<Vec<bool>>();
    assert!(p.toggle_construction(si, &[circle, side]), "the circle first: said ordinary");
    assert_eq!(construction(&p), [true, true], "the circle first: not all construction");
    assert!(!p.toggle_construction(si, &[side, circle]), "all construction: said construction again");
    assert_eq!(construction(&p), [false, false], "all construction: not all ordinary after");
}
