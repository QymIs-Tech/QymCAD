//! DOCUMENT TEMPLATES.
//!
//! Templates are what make DOCUMENT settings useful: without them the geometry tolerance, the author
//! and the other properties would have to be set again in every new file.
//!
//! A template is an ordinary document, not a separate entity: introducing a "template format"
//! alongside the document format would mean maintaining two formats obliged to do the same thing.
//!
//! THERE IS ONE MAIN DANGER HERE, and it is about losing work: a document created FROM a template
//! must not remember the path of the template. Otherwise the very first Save writes the work over the
//! template itself — and a person loses both the template and their confidence that their files are
//! where they left them.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::GeomQuality;

    /// A document with marks that must travel into the new file.
    fn marked_document() -> App {
        let mut app = super::super::screen_keys::tests::plate();
        app.project.meta.author = "Denis".into();
        app.project.meta.version = "rev. B".into();
        app.project.geom_quality = GeomQuality::Fine;
        app
    }

    fn tpl_path(name: &str) -> String {
        let dir = std::env::temp_dir().join("qym_templates_test");
        std::fs::create_dir_all(&dir).expect("the directory for the check");
        let p = dir.join(format!("{name}.qcad")).to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&p);
        p
    }

    /// THE POINT: a new document from a template DOES NOT REMEMBER THE PATH OF THE TEMPLATE.
    ///
    /// Were it to remember, Save would overwrite the template with the work. That is exactly what is
    /// checked, rather than "the file opened": the opening is obvious here, while overwriting the
    /// template is a quiet loss.
    #[test]
    fn a_document_made_from_a_template_forgets_where_it_came_from() {
        let path = tpl_path("forget");
        let mut author = marked_document();
        author.set_project_path(path.clone());
        author.save_project_for_test();
        author.wait_bg_for_test();

        let mut app = App::default();
        app.new_from_template(&path);
        assert!(app.project_path_for_test().is_none(), "the new document remembers the path of the template — Save will overwrite the template itself");
        assert!(!app.project.timeline.is_empty(), "the contents of the template must travel into the new document");
        let _ = std::fs::remove_file(&path);
    }

    /// THE DOCUMENT PROPERTIES COME ALONG — that is what templates were made for.
    #[test]
    fn the_document_properties_come_along() {
        let path = tpl_path("props");
        let mut author = marked_document();
        author.set_project_path(path.clone());
        author.save_project_for_test();
        author.wait_bg_for_test();

        let mut app = App::default();
        app.new_from_template(&path);
        assert_eq!(app.project.meta.author, "Denis", "the author did not come from the template");
        assert_eq!(app.project.geom_quality, GeomQuality::Fine, "the geometry tolerance did not come from the template");
        let _ = std::fs::remove_file(&path);
    }

    /// THE CREATION DATE IS CLEARED: the document is created NOW, not when the template was saved.
    #[test]
    fn the_creation_date_starts_over() {
        let path = tpl_path("date");
        let mut author = marked_document();
        author.set_project_path(path.clone());
        author.save_project_for_test(); // the first save is what sets the date
        author.wait_bg_for_test();
        assert!(!author.project.meta.created.is_empty(), "setup: the template has a creation date");

        let mut app = App::default();
        app.new_from_template(&path);
        assert!(app.project.meta.created.is_empty(), "the new document carried off the creation date of the template — it stopped being a fact about it");
        let _ = std::fs::remove_file(&path);
    }

    /// A NEW DOCUMENT FROM A TEMPLATE IS CLEAN: no edits have been made in it, and closing is not
    /// obliged to ask about unsaved work while nobody has touched anything.
    #[test]
    fn a_fresh_document_from_a_template_is_not_dirty() {
        let path = tpl_path("clean");
        let mut author = marked_document();
        author.set_project_path(path.clone());
        author.save_project_for_test();
        author.wait_bg_for_test();

        let mut app = App::default();
        app.new_from_template(&path);
        assert!(!app.is_dirty_for_test(), "a document from a template counts as dirty right away — the save question comes up for nothing");
        let _ = std::fs::remove_file(&path);
    }

    /// A TEMPLATE DOES NOT GET INTO THE RECENT FILES: it is a blank, not a file that was worked on.
    #[test]
    fn a_template_is_not_a_recent_file() {
        let path = tpl_path("recent");
        let mut author = marked_document();
        author.set_project_path(path.clone());
        author.save_project_for_test();
        author.wait_bg_for_test();

        let mut app = App::default();
        app.new_from_template(&path);
        assert!(!app.recent_for_test().iter().any(|p| p == &path), "the template got into the recent files: {:?}", app.recent_for_test());
        let _ = std::fs::remove_file(&path);
    }

    /// THE NAME OF A TEMPLATE IS WRITTEN BY A PERSON — and it has no right to become a path.
    #[test]
    fn a_template_name_can_never_escape_its_folder() {
        for evil in ["../../elsewhere", "a/b", "..", "  ", "", "C:\\\\windows"] {
            let f = crate::templates::file_name(evil);
            assert!(!f.contains('/') && !f.contains('\\'), "the name \"{evil}\" produced a path: {f}");
            assert!(!f.starts_with('.') || f == "template.qcad", "the name \"{evil}\" produced a hidden file or one that jumps upwards: {f}");
            assert!(f.ends_with(".qcad"), "the name \"{evil}\" produced something other than a document: {f}");
        }
    }

    /// DELETING A TEMPLATE STAYS INSIDE THE DIRECTORY. The path comes from the list, but the list is
    /// data, and one day it will come from somewhere else.
    #[test]
    fn removing_a_template_stays_inside_the_folder() {
        let outside = tpl_path("outside_guard");
        std::fs::write(&outside, "not a template").expect("the file writes");
        let err = crate::templates::remove(&outside);
        assert!(err.is_err(), "deleting a file OUTSIDE the template directory must be refused");
        assert!(std::path::Path::new(&outside).exists(), "a file outside the directory was deleted after all");
        let _ = std::fs::remove_file(&outside);
    }
}
