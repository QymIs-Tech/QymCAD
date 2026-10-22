//! THE NUMBER OF SIDES OF A PRISM FOLLOWS ITS PARAMETER: typed as an expression in the field of the prism, it is a
//! dimension of the node, as the radius and the height are and as the count of an array is.
use qymcad_core::model::{Param, Project};

#[test]
fn a_prism_of_k_sides_turns_with_k() {
    let mut p = Project::default();
    p.new_document();
    p.parameters.push(Param { name: "k".into(), expr: "6".into(), value: 6.0 });
    let prism = p.add_prism(10.0, 6, 5.0);
    p.set_feat_dim(prism, "n", "k".into());
    // a regular polygon of `n` sides in a circle of 10: n / 2 * 100 * sin(2 pi / n), times the height of 5
    let volume = |n: f64| n / 2.0 * 100.0 * (2.0 * std::f64::consts::PI / n).sin() * 5.0;
    let built = |p: &mut Project| {
        let (report, shapes) = qymcad_testkit::regenerate(p);
        assert!(report.errors.is_empty(), "the prism did not build: {:?}", report.errors);
        shapes.get(&prism).map_or(0.0, |s| s.volume())
    };
    let six = built(&mut p);
    assert!((six - volume(6.0)).abs() < 0.01, "k = 6 sides hold {:.2} mm^3, not {:.2}", six, volume(6.0));
    if let Some(k) = p.parameters.iter_mut().find(|q| q.name == "k") {
        k.expr = "8".into();
        k.value = 8.0;
    }
    let eight = built(&mut p);
    assert!((eight - volume(8.0)).abs() < 0.01, "k became 8 and the prism holds {:.2} mm^3, not {:.2}", eight, volume(8.0));
}
