//! The part library: the manifest of a standard part (`.qpart`).
//!
//! A library part is a self-contained component — a part or a subassembly — extracted from a project by
//! `Project::subproject_of` and packed into a `.qpart` zip bundle; the packing itself lives in `qymcad-io`.
//! The manifest carries readable metadata (name, description, tags, author) that the library window shows
//! without unpacking `document.ron`. There are no exposed parameters: a library part is copied into the project
//! and edited from the inside, like any other part or assembly.

use serde::{Deserialize, Serialize};

/// The current schema version of a `.qpart` manifest. It grows on incompatible format changes; there is no
/// backwards compatibility while the project is in development.
pub const PART_SCHEMA: u32 = 1;

/// The manifest of a library part, stored as `part.ron` inside the `.qpart` bundle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartManifest {
    /// The schema version of the bundle; see `PART_SCHEMA`.
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    /// The display name of the part, such as "Extrusion 20×20".
    pub name: String,
    /// A description of a line or two, shown on hover and in the properties.
    #[serde(default)]
    pub description: String,
    /// Tags for search and filtering.
    #[serde(default)]
    pub tags: Vec<String>,
    /// The author who drew it.
    #[serde(default)]
    pub author: String,
}

fn default_schema() -> u32 {
    PART_SCHEMA
}

impl PartManifest {
    /// A new manifest with the current schema version and no metadata beyond the name.
    pub fn new(name: impl Into<String>) -> Self {
        Self { schema_version: PART_SCHEMA, name: name.into(), description: String::new(), tags: Vec::new(), author: String::new() }
    }
}

/// Metadata of a category, stored as `category.ron` inside the category folder.
///
/// Every field is optional and written without `Some(...)`, as in `(title: "Extrusions", order: 1, icon:
/// "cube")`. An empty string means unset: with no file or no field, `title` falls back to the folder name,
/// `order` to zero so sorting is alphabetical, and `icon` to the default.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CategoryMeta {
    /// The display name; empty falls back to the folder name.
    ///
    /// It is not shown for the built-in library, which ships with the program and has to speak the language of
    /// the interface; `title_key` exists for that. What remains here is the fallback, and the name of a
    /// user-created category, which must not be translated: it is their text.
    #[serde(default)]
    pub title: String,
    /// The catalogue key for the name of a built-in category.
    ///
    /// The names arrived as data from the manifests, so the guard against a catalogue key failing to reach the
    /// screen did not see them: it checks the catalogue, and these are library files. An English interface
    /// therefore displayed category names in another language, and nothing but the eye noticed.
    #[serde(default)]
    pub title_key: String,
    /// The sort order among siblings, smaller first. Equal values sort alphabetically.
    #[serde(default)]
    pub order: i32,
    /// The name of the icon for the category node, without its prefix; empty falls back to the default.
    #[serde(default)]
    pub icon: String,
}
