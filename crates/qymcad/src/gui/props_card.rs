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

use super::*;

/// THE NAME OF THE OBJECT in the header: editable, shown only, or absent.
///
/// The edit is returned outwards rather than written here: for a body the name is set by
/// `set_mesh_name`, for a sketch by a field of the structure, and a feature has no name in that sense
/// at all. The write paths differ in substance, and folding them into one would be a lie for the sake
/// of symmetry.
pub(crate) enum NameSlot {
    /// The stored value (may be an auto-name key — shown translated, written only if touched).
    Editable(String),
    Fixed(String),
    None,
}

/// THE LINEAGE OF AN OBJECT, ready to be shown: what created it and what depends on it.
#[derive(Default)]
pub(crate) struct Lineage {
    pub built_on: Vec<String>,
    pub dependents: Vec<String>,
}

impl App {
    /// The lineage of object `id` BY NAMES. `None` means the object has no Id of its own (a face)
    /// and has no lineage.
    pub(super) fn lineage_of(&self, id: Option<Id>) -> Lineage {
        let Some(id) = id else { return Lineage::default() };
        let name_of = |nid: Id| -> String {
            self.project.timeline.iter().find(|n| n.id == nid).map(|n| crate::i18n::name(&n.name)).unwrap_or_else(|| format!("#{nid}"))
        };
        // "WHAT CREATED IT" for a timeline node is the node itself, and there is no point repeating
        // it under its own heading: what is selected is already written there. What is shown is the
        // input it stands on.
        let built_on: Vec<String> = match self.project.timeline.iter().find(|n| n.id == id) {
            Some(node) => node.kind.inputs().iter().map(|i| self.project.creator_of(*i).map(name_of).unwrap_or_else(|| format!("#{i}"))).collect(),
            None => self.project.creator_of(id).map(name_of).into_iter().collect(),
        };
        Lineage { built_on, dependents: self.project.dependents_of(id).into_iter().map(name_of).collect() }
    }
}

/// Draw the header. Returns the new name if a person touched it.
pub(crate) fn props_header(ui: &mut egui::Ui, icon: &str, kind_key: &str, name: NameSlot, lin: &Lineage) -> Option<String> {
    ui.heading(format!("{icon} {}", crate::i18n::tr(kind_key)));
    let mut renamed = None;
    match name {
        NameSlot::Editable(mut stored) => {
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("pp-name"));
                if name_edit(ui, &mut stored).changed() {
                    renamed = Some(stored);
                }
            });
        }
        NameSlot::Fixed(s) if !s.is_empty() => {
            ui.label(egui::RichText::new(crate::i18n::name(&s)).weak().small());
        }
        _ => {}
    }
    let mut chain = |key: &str, items: &[String]| {
        if items.is_empty() {
            return;
        }
        ui.label(egui::RichText::new(&crate::i18n::tr(key)).small().weak());
        for it in items {
            ui.label(format!("  · {it}"));
        }
    };
    chain("fp-built-on", &lin.built_on);
    chain("fp-dependents", &lin.dependents);
    ui.separator();
    renamed
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
            ("sketching.rs", include_str!("sketching.rs")),
        ];
        // the CAD kinds of selection; the CAM cards (machine, tool, setup, operation) are not included
        // here — that module has fallen behind and will be rewritten
        let cards = ["feature_props", "mesh_props", "face_props", "contour_props", "sketch_props", "plane_props", "datum_point_props", "datum_axis_props", "component_props"];
        for card in cards {
            let mut found = false;
            for (fname, src) in &files {
                let Some(a) = src.find(&format!("fn {card}(")) else { continue };
                found = true;
                let b = src[a..].find("\n    pub(super) fn ").map(|i| a + i).unwrap_or(src.len());
                let b2 = src[a..].find("\n    fn ").map(|i| a + i).unwrap_or(src.len());
                let body = &src[a..b.min(b2)];
                assert!(body.contains("props_header("), "the `{card}` card ({fname}) does not call the shared header — it has a form of its own");
            }
            assert!(found, "the `{card}` card was not found in any file — the list of kinds has drifted from the code");
        }
    }

    /// THE LINEAGE ANSWERS ABOUT MORE THAN BODIES. While the query lived as a walk in the panel it
    /// looked at bodies: for a sketch "what depends on me" was empty, even though an extrude stands on
    /// it.
    #[test]
    fn a_sketch_knows_which_features_stand_on_it() {
        let app = super::super::screen_keys::tests::plate();
        let sid = app.project.sketches[0].id;
        let lin = app.lineage_of(Some(sid));
        assert!(!lin.dependents.is_empty(), "a sketch an extrude stands on must have a dependent — and there are none");
        // and the other way round: for a body it is visible what created it
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body of the plate");
        assert!(!app.lineage_of(Some(body)).built_on.is_empty(), "for a body it must be visible what created it");
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
            app.sel = *sel;
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
        app.sel = Sel::Mesh(0);
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.properties_panel(c));
        let want = crate::i18n::tr("fp-built-on");
        assert!(texts.iter().any(|t| t.contains(&want)), "the body properties hold no \"{want}\" line; on screen: {texts:?}");
    }
}
