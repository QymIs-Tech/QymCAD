//! THE WAYS TO SUPPORT THE AUTHOR, a resource baked into the binary: `assets/support/support.ron` names each way - the
//! words of its link, its address, the picture of its QR code lying beside it. The window that shows them reads only
//! this list, so a way is added or changed in the folder, not in the code.
use include_dir::{include_dir, Dir};

/// The folder of the resource (`build.rs` watches it).
static SUPPORT: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../assets/support");

/// ONE WAY TO SUPPORT: the words of its link, its address, and the file of its QR code in the folder.
#[derive(serde::Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Way {
    pub label: String,
    pub url: String,
    pub qr: String,
}

/// The ways, in the order of the list; none when the list does not read.
pub(crate) fn ways() -> Vec<Way> {
    SUPPORT.get_file("support.ron").and_then(|f| f.contents_utf8()).and_then(|t| ron::from_str(t).ok()).unwrap_or_default()
}

/// The bytes of the picture `name` of the folder.
pub(crate) fn picture(name: &str) -> Option<&'static [u8]> {
    SUPPORT.get_file(name).map(|f| f.contents())
}
