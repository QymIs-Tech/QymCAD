//! THE DOCUMENT PROPERTIES TRAVEL WITH THE FILE.
//!
//! A document used to be a nameless collection of geometry: no author, no title, no version. These
//! are properties of THE DOCUMENT rather than of the program — send it to a colleague and they see
//! whose it is and what it is — so they live in the `.qcad` and not in the config. The rule of
//! division: if it must travel with the file when the file is sent, its place is in the document.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::{DocMeta, Project};

    fn filled() -> DocMeta {
        DocMeta { title: "Filter housing".into(), author: "Denis".into(), version: "rev. B".into(), comment: "print with a 0.4 nozzle".into(), created: "2026-08-02T10:00:00Z".into(), saved_by: String::new() }
    }

    /// THE PROPERTIES SURVIVE WRITING AND READING THE DOCUMENT.
    #[test]
    fn the_properties_travel_with_the_document() {
        let mut p = Project::default();
        p.new_document();
        p.meta = filled();
        let text = ron::ser::to_string(&p).expect("the document serialises");
        let back: Project = ron::from_str(&text).expect("and reads back");
        assert_eq!(back.meta, filled(), "the document properties must survive writing and reading");
    }

    /// A DOCUMENT WITHOUT PROPERTIES STILL LOADS — files written BEFORE this work must open.
    ///
    /// The fixture is built by CUTTING the field out of a real record rather than by typing a minimal
    /// RON by hand: the document has plenty of fields without `serde(default)`, and a handwritten stub
    /// would check the wrong thing — it would fail on the first of those rather than on the absence of
    /// the properties.
    #[test]
    fn a_document_without_properties_still_loads() {
        let mut p = Project::default();
        p.new_document();
        p.meta = filled();
        let text = ron::ser::to_string(&p).expect("the document serialises");
        let at = text.find("meta:").expect("the properties field is there");
        let end = text[at..].find("units:").or_else(|| text[at..].find("next_id:")).expect("the next field") + at;
        let without = format!("{}{}", &text[..at], &text[end..]);

        let back: Project = ron::from_str(&without).expect("a document WITHOUT properties must read");
        assert!(back.meta.is_empty(), "a document without properties must have them empty rather than filled with rubbish");
        assert!(back.meta.created.is_empty(), "and the date too");
    }

    /// THE PROPERTIES ARE THE DOCUMENT, NOT A SETTING OF THE PROGRAM.
    ///
    /// A mistake in that direction costs dearly: had the author and the title gone into the config,
    /// they would have stuck to the machine, and one and the same file would be named differently for
    /// two people.
    #[test]
    fn the_properties_are_not_a_program_setting() {
        let text = ron::ser::to_string(&super::super::Settings::default()).expect("the settings serialise");
        for field in ["title", "author", "version", "comment"] {
            assert!(!text.contains(field), "the document property \"{field}\" leaked into the program settings: {text}");
        }
    }

    /// THE DATE IS STAMPED ONCE — "when it was created" is a fact, not a property of the last write.
    #[test]
    fn the_creation_date_is_stamped_once() {
        let mut app = App::default();
        assert!(app.project.meta.created.is_empty(), "setup: the new document has not been created yet");

        let dir = std::env::temp_dir().join("qym_doc_props_test");
        std::fs::create_dir_all(&dir).expect("the directory for the check");
        let path = dir.join("props.qcad").to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);

        app.save_for_test(path.clone());
        let first = app.project.meta.created.clone();
        assert!(!first.is_empty(), "the first save must stamp the date");

        // saved again, and the date is the same
        app.project.meta.created = first.clone();
        app.save_for_test(path.clone());
        assert_eq!(app.project.meta.created, first, "a repeated save must leave the creation date as it was");
        let _ = std::fs::remove_file(&path);
    }

    /// AN AUTOSAVE DOES NOT "CREATE" THE DOCUMENT — it is a snapshot, not the birth of a file.
    #[test]
    fn an_autosave_does_not_stamp_the_date() {
        let mut app = App::default();
        let dir = std::env::temp_dir().join("qym_doc_props_test");
        std::fs::create_dir_all(&dir).expect("the directory for the check");
        let path = dir.join("auto.qcad").to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);

        app.autosave_for_test(path.clone());
        assert!(app.project.meta.created.is_empty(), "an autosave does not create the document — the date must stay empty");
        let _ = std::fs::remove_file(&path);
    }

    /// THE CALENDAR RECKONS RIGHTLY — CHECKED AGAINST KNOWN DATES.
    ///
    /// The calendar is written by hand (a date crate was not dragged in for the sake of one line), and
    /// by hand people get exactly two things wrong in it: leap years and crossing February. A range
    /// check does not catch that — "2026-02-30" passes straight through it. So the stamps taken are
    /// ones whose answer is known exactly, including 29 February and the boundaries of a day and a
    /// year.
    #[test]
    fn the_calendar_gets_known_dates_right() {
        for (secs, want) in [
            (0u64, "1970-01-01T00:00:00Z"),
            (86_399, "1970-01-01T23:59:59Z"),
            (86_400, "1970-01-02T00:00:00Z"),
            (951_782_400, "2000-02-29T00:00:00Z"),   // a leap century: 2000 is a leap year
            (1_078_012_800, "2004-02-29T00:00:00Z"), // an ordinary leap year
            (1_709_164_800, "2024-02-29T00:00:00Z"),
            (1_767_225_599, "2025-12-31T23:59:59Z"), // the boundary of a year
            (4_102_444_800, "2100-01-01T00:00:00Z"),
            // THE RULE OF CENTURIES is only exercised AFTER February: 2100 is not a leap year, and
            // without that rule the 1st of March would slide back a day. A stamp on the 1st of January
            // does not touch it at all — that is how it was written at first, and an honesty check
            // showed it.
            (4_107_542_400, "2100-03-01T00:00:00Z"),
            (4_233_772_800, "2104-03-01T00:00:00Z"), // and 2104 is a leap year — the rule must not eat that one too
        ] {
            assert_eq!(super::super::iso8601_from_unix(secs), want, "the calendar got the stamp {secs} wrong");
        }
    }

    /// THE DATE IS ISO-8601 AND READABLE. The format is deliberately a machine one: it is
    /// unambiguous and it sorts.
    #[test]
    fn the_timestamp_is_iso8601() {
        let s = super::super::now_iso8601();
        assert_eq!(s.len(), 20, "YYYY-MM-DDTHH:MM:SSZ was expected, and it came out \"{s}\"");
        assert!(s.ends_with('Z') && s.as_bytes()[10] == b'T', "the ISO-8601 layout is broken: \"{s}\"");
        let year: i32 = s[..4].parse().expect("the year as a number");
        assert!((2025..2100).contains(&year), "the year \"{year}\" is outside common sense — the calendar reckons wrongly");
        let month: u32 = s[5..7].parse().expect("the month as a number");
        let day: u32 = s[8..10].parse().expect("the day as a number");
        assert!((1..=12).contains(&month) && (1..=31).contains(&day), "month or day out of range: \"{s}\"");
    }
}
