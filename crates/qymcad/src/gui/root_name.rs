//! THE ROOT ASSEMBLY HAS NO NAME OF ITS OWN.
//!
//! Reported behaviour: in an English build the top crumb read the root assembly in Russian — and the
//! root assembly cannot be renamed by a person at all, so why is it in one language rather than the
//! interface language?
//!
//! Right on both counts. The root always exists and there is exactly one: it is not a component
//! somebody created, it is the document itself. While its name was an ordinary string, it froze in
//! the language of the build the document was created in. A name is translated only if it is a
//! CATALOGUE KEY; the key now sits there, and is put there on every load — otherwise a document
//! whose string has already frozen would have nothing to fix it with.
#[cfg(test)]
mod tests {
    use super::super::App;
    use crate::i18n;

    /// A document whose root is named with a plain word — as in the reported file. The word stays
    /// Cyrillic on purpose: it is the frozen string the check has to catch, and an English one would
    /// make the check below vacuous.
    fn project_with_a_frozen_root_name() -> App {
        let mut app = App::default();
        let root = app.project.root;
        let ci = app.project.components.iter().position(|c| c.id == root).expect("the root is there");
        app.project.components[ci].name = "Сборка".into();
        app
    }

    /// THE POINT: in an English build there is no Cyrillic in the crumb.
    #[test]
    fn the_root_crumb_speaks_the_interface_language() {
        let prev = i18n::language();
        i18n::set_language("en");
        let mut app = project_with_a_frozen_root_name();
        app.project.ensure_document(); // the same thing that opening a file does
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.toolbar(c));
        let cyrillic: Vec<&String> = texts.iter().filter(|t| t.chars().any(|c| ('А'..='я').contains(&c))).collect();
        assert!(cyrillic.is_empty(), "an English build has Cyrillic in the breadcrumbs: {cyrillic:?}");
        let want = i18n::tr("name-assembly");
        assert!(texts.iter().any(|t| t.contains(&want)), "the root must be captioned \"{want}\"; on screen: {texts:?}");
        i18n::set_language(&prev);
    }

    /// AND SWITCHING THE LANGUAGE MOVES IT — that is, the name really comes from the catalogue
    /// rather than having merely coincided.
    #[test]
    fn switching_the_language_moves_the_root_name() {
        let prev = i18n::language();
        let mut app = project_with_a_frozen_root_name();
        app.project.ensure_document();
        let mut seen = Vec::new();
        for code in ["ru", "en"] {
            i18n::set_language(code);
            let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.toolbar(c));
            let want = i18n::tr("name-assembly");
            assert!(texts.iter().any(|t| t.contains(&want)), "{code}: the root \"{want}\" is not on the bar: {texts:?}");
            seen.push(want);
        }
        assert_ne!(seen[0], seen[1], "the root caption is the same in two languages — so it does not come from the catalogue");
        i18n::set_language(&prev);
    }

    /// RENAMING THE ROOT IS NOT OFFERED. Offering it and silently reverting on the next load is
    /// worse than not offering it: a person will decide the program is losing their edits.
    #[test]
    fn renaming_the_root_is_not_offered() {
        let src = crate::gui::panels_source::PANELS;
        // the anchor is the handler of the item on a COMPONENT, not the first occurrence of
        // "act-rename" in the file (that one belongs to a datum plane): renaming a plane is
        // legitimate and there is nothing to touch there.
        let a = src.find("RenameNode::Component(cid), name.clone()").expect("the component rename is there");
        let head = &src[..a];
        let offer = head.rfind("act-rename").expect("the rename item before the handler");
        // backwards by LINES rather than by bytes: a slice in the middle of a multibyte letter fails
        // the test for no reason at all
        let near: String = src[..offer].lines().rev().take(6).collect::<Vec<_>>().join("\n");
        assert!(near.contains("cid != self.project.root"), "the rename item is offered for the root too — and its name will come back as a key on the very first load");
    }
}
