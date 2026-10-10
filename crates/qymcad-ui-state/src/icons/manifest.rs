//! Manifest metadata and configuration for icon theme bundles.

use std::collections::BTreeMap;

/// Localized text displayed for an icon theme.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalizedThemeText {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// The requested language followed by its primary language, with duplicates removed.
pub(crate) fn locale_fallbacks(locale: &str) -> Vec<String> {
    let Ok(tag) = locale.parse::<unic_langid::LanguageIdentifier>() else {
        return Vec::new();
    };
    let exact = tag.to_string();
    let primary = tag.language.to_string();
    if exact == primary {
        vec![exact]
    } else {
        vec![exact, primary]
    }
}

/// The package type tag for future plugin system compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PackageType {
    #[default]
    IconTheme,
}

/// Maximum length of a theme bundle ID (64 characters).
pub const MAX_MANIFEST_ID_LEN: usize = 64;

/// Maximum length of a theme name (64 characters).
pub const MAX_MANIFEST_NAME_LEN: usize = 64;

/// Maximum length of a version string (32 characters).
pub const MAX_MANIFEST_VERSION_LEN: usize = 32;

/// Maximum length of an author string (64 characters).
pub const MAX_MANIFEST_AUTHOR_LEN: usize = 64;

/// Maximum length of a license identifier (64 characters).
pub const MAX_MANIFEST_LICENSE_LEN: usize = 64;

/// Maximum length of a manifest description summary (512 characters).
pub const MAX_MANIFEST_DESCRIPTION_LEN: usize = 512;

/// Maximum number of translation entries permitted in a manifest.
pub const MAX_MANIFEST_TRANSLATIONS: usize = 32;

/// Maximum length of a translation locale tag (16 characters).
pub const MAX_MANIFEST_LOCALE_LEN: usize = 16;

/// Whether a character is a Unicode combining diacritical mark or enclosing mark.
pub fn is_combining_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036F}'
            | '\u{0483}'..='\u{0489}'
            | '\u{0591}'..='\u{05BD}'
            | '\u{05BF}'
            | '\u{05C1}'..='\u{05C2}'
            | '\u{05C4}'..='\u{05C5}'
            | '\u{05C7}'
            | '\u{0610}'..='\u{061A}'
            | '\u{064B}'..='\u{065F}'
            | '\u{0670}'
            | '\u{06D6}'..='\u{06DC}'
            | '\u{06DF}'..='\u{06E4}'
            | '\u{06E7}'..='\u{06E8}'
            | '\u{06EA}'..='\u{06ED}'
            | '\u{0711}'
            | '\u{0730}'..='\u{074A}'
            | '\u{07A6}'..='\u{07B0}'
            | '\u{07EB}'..='\u{07F3}'
            | '\u{0816}'..='\u{0819}'
            | '\u{081B}'..='\u{0823}'
            | '\u{0825}'..='\u{0827}'
            | '\u{0829}'..='\u{082D}'
            | '\u{0859}'..='\u{085B}'
            | '\u{08D3}'..='\u{08E1}'
            | '\u{08E3}'..='\u{0903}'
            | '\u{093A}'..='\u{094F}'
            | '\u{0951}'..='\u{0957}'
            | '\u{0962}'..='\u{0963}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}

/// Detects Zalgo text (stacked or excessive combining diacritics that distort layouts).
pub fn has_zalgo(text: &str) -> bool {
    let mut consecutive_combining = 0;
    let mut total_combining = 0;
    let mut total_chars = 0;

    for (i, c) in text.chars().enumerate() {
        total_chars += 1;
        if is_combining_mark(c) {
            if i == 0 {
                return true;
            }
            consecutive_combining += 1;
            total_combining += 1;
            if consecutive_combining > 2 {
                return true;
            }
        } else {
            consecutive_combining = 0;
        }
    }

    if total_chars >= 4 && total_combining * 3 > total_chars {
        return true;
    }

    false
}

/// Metadata describing an icon pack bundle.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IconManifest {
    #[serde(default)]
    pub package_type: PackageType,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub translations: BTreeMap<String, LocalizedThemeText>,
    /// Whether the package is verified by QymCAD packager.
    #[serde(default)]
    pub verified: bool,
}

impl IconManifest {
    pub fn name_for_locale(&self, locale: &str) -> &str {
        for tag in locale_fallbacks(locale) {
            if let Some(text) = self.translations.get(&tag).map(|translation| translation.name.as_str()).filter(|text| !text.trim().is_empty()) {
                return text;
            }
        }
        &self.name
    }

    pub fn description_for_locale(&self, locale: &str) -> &str {
        for tag in locale_fallbacks(locale) {
            if let Some(text) = self.translations.get(&tag).map(|translation| translation.description.as_str()).filter(|text| !text.trim().is_empty()) {
                return text;
            }
        }
        &self.description
    }

    /// Validate manifest fields to prevent UI distortion, path traversal, and resource issues.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("theme ID cannot be empty".to_string());
        }
        if self.id.chars().count() > MAX_MANIFEST_ID_LEN {
            return Err(format!("theme ID exceeds maximum length of {MAX_MANIFEST_ID_LEN} characters"));
        }
        if !self.id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
            || !self.id.ends_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
            || !self.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(format!("theme ID must be lowercase ASCII alphanumeric with hyphens or underscores, and start and end with an alphanumeric character (got {:?})", self.id));
        }

        let trimmed_name = self.name.trim();
        if trimmed_name.is_empty() {
            return Err("theme name cannot be empty".to_string());
        }
        if self.name.chars().count() > MAX_MANIFEST_NAME_LEN {
            return Err(format!("theme name exceeds maximum length of {MAX_MANIFEST_NAME_LEN} characters"));
        }
        if self.name.chars().any(|c| c == '\n' || c == '\r' || c.is_control()) {
            return Err("theme name cannot contain newlines or control characters".to_string());
        }
        if has_zalgo(&self.name) {
            return Err("theme name cannot contain Zalgo text or excessive combining marks".to_string());
        }

        if self.version.chars().count() > MAX_MANIFEST_VERSION_LEN {
            return Err(format!("version exceeds maximum length of {MAX_MANIFEST_VERSION_LEN} characters"));
        }
        if self.version.chars().any(|c| c.is_control()) {
            return Err("version cannot contain control characters".to_string());
        }
        if has_zalgo(&self.version) {
            return Err("version cannot contain Zalgo text or excessive combining marks".to_string());
        }

        if self.author.chars().count() > MAX_MANIFEST_AUTHOR_LEN {
            return Err(format!("author exceeds maximum length of {MAX_MANIFEST_AUTHOR_LEN} characters"));
        }
        if self.author.chars().any(|c| c.is_control()) {
            return Err("author cannot contain control characters".to_string());
        }
        if has_zalgo(&self.author) {
            return Err("author cannot contain Zalgo text or excessive combining marks".to_string());
        }

        if self.license.chars().count() > MAX_MANIFEST_LICENSE_LEN {
            return Err(format!("license exceeds maximum length of {MAX_MANIFEST_LICENSE_LEN} characters"));
        }
        if self.license.chars().any(|c| c.is_control()) {
            return Err("license cannot contain control characters".to_string());
        }
        if has_zalgo(&self.license) {
            return Err("license cannot contain Zalgo text or excessive combining marks".to_string());
        }

        if self.description.chars().count() > MAX_MANIFEST_DESCRIPTION_LEN {
            return Err(format!("description exceeds maximum length of {MAX_MANIFEST_DESCRIPTION_LEN} characters"));
        }
        if self.description.chars().any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t') {
            return Err("description cannot contain control characters".to_string());
        }
        if has_zalgo(&self.description) {
            return Err("description cannot contain Zalgo text or excessive combining marks".to_string());
        }

        if self.translations.len() > MAX_MANIFEST_TRANSLATIONS {
            return Err(format!("too many translation entries: {} (maximum is {MAX_MANIFEST_TRANSLATIONS})", self.translations.len()));
        }

        for (locale, text) in &self.translations {
            if locale.chars().count() > MAX_MANIFEST_LOCALE_LEN {
                return Err(format!("locale tag {locale:?} exceeds maximum length of {MAX_MANIFEST_LOCALE_LEN} characters"));
            }
            if locale.parse::<unic_langid::LanguageIdentifier>().is_err() {
                return Err(format!("invalid locale tag: {locale:?}"));
            }
            if text.name.chars().count() > MAX_MANIFEST_NAME_LEN {
                return Err(format!("translated name for {locale} exceeds maximum length of {MAX_MANIFEST_NAME_LEN} characters"));
            }
            if text.name.chars().any(|c| c == '\n' || c == '\r' || c.is_control()) {
                return Err(format!("translated name for {locale} cannot contain newlines or control characters"));
            }
            if has_zalgo(&text.name) {
                return Err(format!("translated name for {locale} cannot contain Zalgo text or excessive combining marks"));
            }
            if text.description.chars().count() > MAX_MANIFEST_DESCRIPTION_LEN {
                return Err(format!("translated description for {locale} exceeds maximum length of {MAX_MANIFEST_DESCRIPTION_LEN} characters"));
            }
            if text.description.chars().any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t') {
                return Err(format!("translated description for {locale} cannot contain control characters"));
            }
            if has_zalgo(&text.description) {
                return Err(format!("translated description for {locale} cannot contain Zalgo text or excessive combining marks"));
            }
        }

        Ok(())
    }

    pub fn parse_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
    }
}
