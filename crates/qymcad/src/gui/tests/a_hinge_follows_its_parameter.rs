//! THE ANGLE OF A HINGE WRITTEN AS A PARAMETER FOLLOWS THE PARAMETER, edited in the window of the parameters.
//!
//! A mate is no node of the timeline: its formulas stand in `feat_dims` under its own id, and the placement pass of a
//! rebuild solves it. A parameter edit that marks only the nodes counting from it left the hinge where it stood.
//! Reported behaviour: pa = 90 typed into the angle of a hinge turned the part by 90; pa made 45, the part stayed at 90.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::model::{Id, Param};

/// How far `part` stands turned from how it was built, in degrees.
fn turned(app: &App, part: Id) -> f64 {
    let t = app.project.component_transform(part);
    ((t[0] + t[5] + t[10] - 1.0) / 2.0).clamp(-1.0, 1.0).acos().to_degrees()
}

#[test]
fn a_hinge_angle_driven_by_a_parameter_follows_it() {
    let mut app = App::default();
    let ([hinge, _], [wheel, _]) = crate::gui::a_relation_is_made_by_hand::tests::two_hinges(&mut app);
    app.project.parameters.push(Param { name: "pa".into(), expr: "90".into(), value: 90.0 });
    app.project.set_feat_dim(hinge, "angle", "pa".into());
    qymcad_ui_state::regenerate_all(&mut app.rebuild_ctx());
    let at_90 = turned(&app, wheel);
    assert!((at_90 - 90.0).abs() < 0.5, "setup: the angle of the hinge is pa = 90 and the part stands turned by {at_90:.2} deg");

    // pa made 45 in the table, the edit applied as the window applies it, in a live window
    if let Some(p) = app.project.parameters.iter_mut().find(|p| p.name == "pa") {
        p.expr = "45".into();
    }
    let mut hand = Hand::new(&mut app);
    hand.frame(Vec::new());
    let mut asks = Vec::new();
    crate::gui::panels_windows::apply_param_edit(&mut hand.app.win_ctx(&mut asks));
    hand.close_window();
    let at_45 = turned(&app, wheel);
    assert!((at_45 - 45.0).abs() < 0.5, "pa became 45 and the part driven by it stands turned by {at_45:.2} deg");
}
