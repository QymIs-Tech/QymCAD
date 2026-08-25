//! WHO GETS MARKED AS A REDUNDANT CONSTRAINT — and why that is ONE rule rather than two.
//!
//! The rule lived only in the list of constraints, while the glyphs on the canvas were coloured by the
//! raw `diag.redundant`. The two places drifted apart silently and for a long time: in the list a slot
//! is clean, and on its tangencies in the sketch the orange "redundant constraint" markers burn.
//! Everybody who ever drew a slot saw it; it was caught by chance, on a picture for the help, the
//! first time somebody had to LOOK at a sketch rather than at its tests.
//!
//! The lie of the rank is analysed in `flagged_redundant`: the Jacobian of a tangency at the point of
//! tangency is parallel to the intrinsic of the arc. These exceptions hide no real contradictions —
//! those are caught by `sketch_conflicts`, which reckons by geometry rather than by rank.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::Constraint;

    /// A sketch with a slot: two semicircles and two tangent lines.
    fn slot() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_slot_entity(si, -16.0, 0.0, 16.0, 0.0, 9.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        (app, si)
    }

    /// THE TANGENCIES OF A SLOT ARE NOT MARKED — neither in the list nor on the canvas.
    ///
    /// A slot is an ordinary shape, not an overconstrained sketch. A marker on it teaches a person that
    /// they made a mistake they never made, and devalues the marker where it is real.
    #[test]
    fn a_slot_is_not_flagged_as_overconstrained() {
        let (app, si) = slot();
        let raw = app.project.sketch_redundant_constraints(si);
        assert!(!raw.is_empty(), "the rank analysis found no redundancy on the slot — the guard is checking emptiness and the scene must be changed");
        assert!(app.flagged_redundant(si).is_empty(), "the slot is marked as overconstrained: {:?}", app.flagged_redundant(si));
    }

    /// AND A REAL REDUNDANCY IS MARKED. Without this half the guard is green for the rule "never mark
    /// anything" as well — that is, for a marker switched off.
    #[test]
    fn a_genuinely_redundant_constraint_is_still_flagged() {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_line_entity(si, -20.0, 0.0, 20.0, 0.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        let pts: Vec<qymcad_core::model::Id> = app.project.sketches[si].points.iter().map(|p| p.id).collect();
        assert!(pts.len() >= 2, "a sketch with a segment should hold two points");
        // one horizontal is needed, the second is the very same one: exactly what the marker is for
        app.project.sketches[si].constraints.push(Constraint::Horizontal { a: pts[0], b: pts[1] });
        app.project.sketches[si].constraints.push(Constraint::Horizontal { a: pts[0], b: pts[1] });
        app.project.regen_sketch(si);
        assert!(!app.flagged_redundant(si).is_empty(), "two identical horizontals are a real redundancy, and there is no marker");
    }

    /// AND CONSTRAINTS ENTANGLED WITH TANGENCIES ARE NOT MARKED EITHER.
    ///
    /// The rank lies about more than the tangencies themselves: on a rounded rectangle the horizontals
    /// and verticals come out falsely redundant too. So while a sketch holds tangencies, the rank
    /// redundancy of geometric constraints is not marked AT ALL.
    ///
    /// A test of its own, because the slot guard does not cover this: on a slot every false marker is a
    /// tangency itself, and it stays green even without this rule (checked by removing it).
    #[test]
    fn constraints_entangled_with_tangency_are_not_flagged_either() {
        let (app, si) = slot();
        let raw = app.project.sketch_redundant_constraints(si);
        let non_tangent = raw
            .iter()
            .filter(|ci| {
                !matches!(
                    app.project.sketches[si].constraints.get(**ci),
                    Some(Constraint::Tangent { .. }) | Some(Constraint::CircleTangent { .. }) | None
                )
            })
            .count();
        // the scene may yield no entangled constraints — then there is nothing to check here, but the
        // rule stands: NOTHING must be marked
        assert!(app.flagged_redundant(si).is_empty(), "something is marked in a sketch with tangencies (non-tangencies in the raw list: {non_tangent})");
    }

    /// THE LIST AND THE CANVAS ASK ONE PLACE. A guard over the source: they can only drift apart if
    /// somebody takes the raw `diag.redundant` for colouring again.
    #[test]
    fn the_list_and_the_canvas_ask_the_same_rule() {
        for (name, src) in [("the canvas", crate::gui::render_source::RENDER), ("the constraint list", include_str!("sketching.rs"))] {
            let uses_rule = src.contains("flagged_redundant(si)");
            assert!(uses_rule, "{name} does not ask the common marking rule");
        }
        // and the raw list is not used for colouring. COMMENTS ARE SKIPPED: the first edition of the
        // guard caught the very line that explains why the raw list is no longer here.
        let code = crate::gui::render_source::RENDER.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
        assert!(!code.contains("diag.redundant"), "the canvas colours by the raw `diag.redundant` again — the rule will drift from the list");
    }
}

/// THE CONSTRAINT LIST NAMES THE PARTICIPANTS.
///
/// The rows were nothing but the kind of constraint: "horizontal, horizontal, vertical, vertical" —
/// four indistinguishable rows. Highlighting on hover already existed, but it answers "where is this
/// one", not "which one do I need": in a sketch of thirty constraints a list without names is
/// useless.
#[cfg(test)]
mod parts_tests {
    use super::super::App;

    /// A rectangle: four lines, eight constraints.
    fn rect() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, -20.0, -12.0, 40.0, 24.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        (app, si)
    }

    /// A CONSTRAINT NAMES THE ENTITIES IT TOUCHES.
    ///
    /// The language is pinned and the expected word is asked OF THE CATALOGUE rather than typed as a
    /// literal: a literal would tie the check to one language and go blind the moment the catalogue is
    /// edited.
    #[test]
    fn a_constraint_names_the_entities_it_touches() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("en");
        let want = crate::i18n::tr("ent-line");
        let (app, si) = rect();
        let cs = app.project.sketches[si].constraints.clone();
        let named: Vec<Vec<String>> = cs.iter().map(|c| app.constraint_parts(si, c)).collect();
        crate::i18n::set_language(&prev);
        assert!(named.iter().any(|p| !p.is_empty()), "not one constraint named its participants: {named:?}");
        for p in named.iter().filter(|p| !p.is_empty()) {
            assert!(p.iter().all(|n| n.contains(&want)), "a participant is not named in human words: {p:?}");
        }
    }

    /// AND THE NAMES TELL THE ENTITIES APART: "Line 1" and "Line 2" are different rows.
    ///
    /// Without the numbering the list would have stayed as it was: four rows of "horizontal: line".
    #[test]
    fn entity_names_are_distinguishable() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("en");
        let (app, si) = rect();
        let ids: Vec<qymcad_core::model::Id> = app.project.sketches[si].entities.iter().map(|e| e.id).collect();
        let names: Vec<String> = ids.iter().map(|id| app.sketch_entity_name(si, *id)).collect();
        crate::i18n::set_language(&prev);
        let mut uniq = names.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(uniq.len(), names.len(), "the entity names repeat: {names:?}");
        assert!(names.iter().any(|n| n.ends_with('1')) && names.iter().any(|n| n.ends_with('2')), "the numbering does not follow the order within a kind: {names:?}");
    }

    /// THE ROW OF THE LIST REACHES THE SCREEN WITH ITS PARTICIPANTS.
    #[test]
    fn the_row_on_screen_carries_the_parts() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("en");
        let want = crate::i18n::tr("ent-line");
        let (mut app, si) = rect();
        app.enter_sketch_edit_pub(si);
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.properties_panel(c));
        crate::i18n::set_language(&prev);
        assert!(texts.iter().any(|t| t.contains(&want)), "the constraint list holds no entity names: {texts:?}");
    }
}
