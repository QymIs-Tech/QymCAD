//! THE DIRECTIONS OF A PATTERN ARE DESCRIBED ONCE for every workbench (`pattern_directions`): the first open under
//! "Direction 1", every one after it under its own number, folded until its title switches it on.

use qymcad_ui_state::{DirectionSwitch, Folded, Lit, OptKey, PatternDirection, Row};

#[test]
fn the_first_direction_is_open_and_the_others_are_switched_by_their_titles() {
    let row = |f: &'static str| vec![Row::new("c").field(f)];
    let sections = qymcad_ui_state::pattern_directions(vec![
        PatternDirection { rows: row("a"), switch: None },
        PatternDirection { rows: row("b"), switch: Some(DirectionSwitch { key: OptKey::of("two"), on: Lit::Off }) },
        PatternDirection { rows: row("c"), switch: Some(DirectionSwitch { key: OptKey::of("three"), on: Lit::On }) },
    ]);
    let titles: Vec<String> = sections.iter().map(|s| s.title.clone().unwrap_or_default()).collect();
    let want: Vec<String> = (1..=3).map(|n| qymcad_i18n::tr1("box-direction-n", "n", &n.to_string())).collect();
    assert_eq!(titles, want, "the titles are not the numbers of the directions");
    assert!(sections[0].switch.is_none() && sections[0].folded == Folded::No, "the first direction is not open, or is a switch");
    assert!(sections[1].switch.is_some_and(|k| k.name == "two") && sections[1].folded == Folded::Yes, "a direction switched off is not folded under its switch");
    assert!(sections[2].folded == Folded::No, "a direction switched on is folded");
}
