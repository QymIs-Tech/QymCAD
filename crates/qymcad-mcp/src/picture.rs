//! A PICTURE OF THE WINDOW AS AN ANSWER: the PNG laid into the answer in base64, under the key the protocol turns into
//! an image block a model looks at.
use serde_json::{json, Value};

/// The size of a picture, in pixels.
pub struct Size {
    pub width: usize,
    pub height: usize,
}

/// The answer that carries a picture of the window.
pub fn answer(png: &[u8], size: Size) -> Value {
    let mut a = json!({ "where": "window", "width": size.width, "height": size.height });
    a[crate::tools::IMAGE_KEY] = json!(base64(png));
    a
}

/// Base64 with padding, the alphabet of RFC 4648: twenty lines here rather than a package to be watched for the one
/// call a picture makes.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * k)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    /// The vectors of RFC 4648, section 10: every length of the last group, padded as the standard pads it.
    #[test]
    fn base64_gives_the_vectors_of_the_standard() {
        let vectors = [["", ""], ["f", "Zg=="], ["fo", "Zm8="], ["foo", "Zm9v"], ["foob", "Zm9vYg=="], ["fooba", "Zm9vYmE="], ["foobar", "Zm9vYmFy"]];
        for [plain, coded] in vectors {
            assert_eq!(super::base64(plain.as_bytes()), coded, "base64 of {plain:?}");
        }
    }
}
