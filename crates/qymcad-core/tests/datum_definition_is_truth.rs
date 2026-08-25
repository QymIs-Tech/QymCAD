//! A datum has one source of truth: its definition. The coordinates are derived and are not written to the file.
//!
//! `origin` and `dir` used to sit next to the definition and were saved. For anything other than a manual datum
//! they have to be recomputed during a rebuild, but if the rebuild did not reach them the field held a stale
//! truth with nothing to distinguish it from a current one: the axis was drawn and took part in construction
//! using the old coordinates.
use qymcad_core::model::{AxisDef, DatumAxis, DatumPoint, Project};

/// The coordinates of a parametric axis do not reach the file: after a round trip they are empty until the
/// resolution runs. A stale value physically cannot survive a save.
#[test]
fn resolved_coordinates_are_not_serialized() {
    let mut p = Project::default();
    p.new_document();
    let a = p.add_datum_point(DatumPoint { at: [0.0, 0.0, 0.0], ..Default::default() });
    let b = p.add_datum_point(DatumPoint { at: [0.0, 0.0, 10.0], ..Default::default() });
    let id = p.add_datum_axis(DatumAxis::from_def("Axis", AxisDef::TwoPoints { a, b }));

    // imitate the resolution: the axis received coordinates in memory
    if let Some(ax) = p.datum_axes.iter_mut().find(|d| d.id == id) {
        ax.set_resolved_for_test([1.0, 2.0, 3.0], [0.0, 0.0, 1.0]);
    }
    assert_eq!(p.datum_axes.iter().find(|d| d.id == id).unwrap().origin(), [1.0, 2.0, 3.0], "the coordinates are present in memory");

    let back = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&p).unwrap()).unwrap();
    let ax = back.datum_axes.iter().find(|d| d.id == id).expect("the axis was saved");
    assert!(matches!(ax.def, AxisDef::TwoPoints { .. }), "the definition was saved, and it is the truth");
    assert_eq!(ax.origin(), [0.0, 0.0, 0.0], "while the coordinates were not: before the resolution they honestly do not exist");
}

/// For a manual axis the coordinates are the definition, so they survive a save.
#[test]
fn manual_axis_keeps_its_coordinates_because_they_are_the_definition() {
    let mut p = Project::default();
    p.new_document();
    let id = p.add_datum_axis(DatumAxis::manual("Manual", [5.0, 6.0, 7.0], [1.0, 0.0, 0.0]));
    let back = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&p).unwrap()).unwrap();
    let ax = back.datum_axes.iter().find(|d| d.id == id).unwrap();
    assert_eq!(ax.origin(), [5.0, 6.0, 7.0], "the manual coordinates are in place");
    assert_eq!(ax.dir(), [1.0, 0.0, 0.0]);
}

/// Editing the coordinates by hand turns the axis manual rather than writing a second truth next to the
/// parametric definition.
#[test]
fn editing_coordinates_switches_the_definition_to_manual() {
    let mut p = Project::default();
    p.new_document();
    let (a, b) = (p.add_datum_point(DatumPoint { at: [0.0; 3], ..Default::default() }), p.add_datum_point(DatumPoint { at: [0.0, 0.0, 5.0], ..Default::default() }));
    let id = p.add_datum_axis(DatumAxis::from_def("Axis", AxisDef::TwoPoints { a, b }));
    let ax = p.datum_axes.iter_mut().find(|d| d.id == id).unwrap();
    ax.set_manual([9.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    assert!(matches!(ax.def, AxisDef::Manual { .. }), "the definition became manual");
    assert_eq!(ax.origin(), [9.0, 0.0, 0.0], "and the coordinates come from it");
}
