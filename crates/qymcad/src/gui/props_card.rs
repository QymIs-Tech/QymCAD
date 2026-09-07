//! ONE HEADER FOR THE PROPERTIES CARD — the same for every kind of selection.
//!
//! The right panel showed every kind in its own way: a feature had a lineage ("what it stands on",
//! "what depends on it"), a body and a datum did not, and a face did not even have an icon. Moving
//! about the tree, a person read a new form every time and could not lean on habit: "where does it say
//! what made this" was answered by only one card out of nine.
//!
//! Here there is exactly one header, and it takes the lineage from the kernel (`Project::creator_of` /
//! `dependents_of`) rather than working it out by a walk on the spot. The walk in the UI knew only
//! about bodies — dependencies on a sketch or a datum never reached the panel at all.

pub(crate) use qymcad_ui_state::{lineage_of};
use super::*;

impl App {
}

#[cfg(test)]
mod tests {
    use super::super::Sel;

    /// EVERY CARD STARTS WITH THE SHARED HEADER.
    ///
    /// A check over the source deliberately, by the same means as the "the panel does not edit" guard
    /// (`props_readonly.rs`): a drawn frame shows that a header IS there, but not that one and the
    /// same function draws it. That is exactly how the cards can drift apart — somebody repeats the
    /// layout by hand, and a month later half the kinds have a form of their own again.
    #[test]
    fn every_properties_card_starts_with_the_shared_header() {
        let files: [(&str, &str); 3] = [
            ("panels.rs", crate::gui::panels_source::PANELS),
            ("gui.rs", include_str!("../gui.rs")),
            ("sketching.rs", crate::gui::sketch_source::SKETCH),
        ];
        // the CAD kinds of selection; the CAM cards (machine, tool, setup, operation) are not included
        // here — that module has fallen behind and will be rewritten
        let cards = ["feature_props", "mesh_props", "face_props", "contour_props", "sketch_props", "plane_props", "datum_point_props", "datum_axis_props", "component_props"];
        for card in cards {
            let (mut found, mut has_header, mut where_) = (false, false, String::new());
            for (fname, src) in &files {
                // EVERY function of that name, not the first. A card lifted out of `App` leaves a one-line
                // wrapper behind, and the wrapper is what `find` hits first: its body draws nothing, so a
                // guard looking only there reports a missing header that is in fact drawn next door.
                for (a, _) in src.match_indices(&format!("fn {card}(")) {
                    found = true;
                    where_ = (*fname).to_string();
                    let rest = &src[a..];
                    let end = ["\n    pub(super) fn ", "\n    pub(crate) fn ", "\n    fn ", "\npub(crate) fn ", "\nfn "]
                        .iter()
                        .filter_map(|m| rest.find(m))
                        .min()
                        .unwrap_or(rest.len());
                    has_header |= rest[..end].contains("props_header(");
                }
            }
            assert!(found, "the `{card}` card was not found in any file — the list of kinds has drifted from the code");
            assert!(has_header, "the `{card}` card ({where_}) does not call the shared header — it has a form of its own");
        }
    }

    /// THE LINEAGE ANSWERS ABOUT MORE THAN BODIES. While the query lived as a walk in the panel it
    /// looked at bodies: for a sketch "what depends on me" was empty, even though an extrude stands on
    /// it.
    #[test]
    fn a_sketch_knows_which_features_stand_on_it() {
        let app = super::super::screen_keys::tests::plate();
        let sid = app.project.sketches[0].id;
        let lin = qymcad_ui_state::lineage_of(&app.project, Some(sid));
        assert!(!lin.dependents.is_empty(), "a sketch an extrude stands on must have a dependent — and there are none");
        // and the other way round: for a body it is visible what created it
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body of the plate");
        assert!(!qymcad_ui_state::lineage_of(&app.project, Some(body)).built_on.is_empty(), "for a body it must be visible what created it");
    }

    /// THE HEADER REACHES THE SCREEN, AND IT SAYS WHAT EXACTLY IS SELECTED.
    ///
    /// Checked BY A FRAME rather than by a call: `ui.label` accepts anything, and "the function
    /// returned a string" says nothing about whether it reached a person — that lesson has already
    /// been paid for in this project by the `f-nominal-d` keys in the thread popup.
    #[test]
    fn the_header_says_what_is_selected_for_every_kind() {
        let cases: &[(&str, Sel, &str)] = &[
            ("body", Sel::Mesh(0), "mesh-props-title"),
            ("face", Sel::Face(0, 0), "face-props-title"),
            ("sketch", Sel::Sketch(0), "sk-props"),
            ("plane", Sel::Plane(0), "pp-title"),
            ("component", Sel::Component(0), "props-component"),
        ];
        for (what, sel, key) in cases {
            let mut app = super::super::screen_keys::tests::populated();
            app.chosen.sel = *sel;
            let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.properties_panel(c));
            let want = crate::i18n::tr(key);
            assert!(texts.iter().any(|t| t.contains(&want)), "{what}: the header does not say what is selected (\"{want}\"); on screen: {texts:?}");
        }
    }

    /// AND THE LINEAGE REACHES IT TOO. A body used to have no "what created it" at all — only a
    /// feature did.
    #[test]
    fn a_body_shows_on_screen_what_created_it() {
        let mut app = super::super::screen_keys::tests::populated();
        app.chosen.sel = Sel::Mesh(0);
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.properties_panel(c));
        let want = crate::i18n::tr("fp-built-on");
        assert!(texts.iter().any(|t| t.contains(&want)), "the body properties hold no \"{want}\" line; on screen: {texts:?}");
    }
}