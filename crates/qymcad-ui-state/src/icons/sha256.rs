//! Standard SHA-256 (FIPS 180-4) hashing and QymCAD bundle trailer verification via `sha2`.

use sha2::{Digest, Sha256};

/// Streaming SHA-256 state machine based on the standard `sha2` crate.
#[derive(Clone, Default)]
pub struct Sha256Hasher {
    hasher: Sha256,
}

impl Sha256Hasher {
    pub fn new() -> Self {
        Self { hasher: Sha256::new() }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    pub fn finalize(self) -> [u8; 32] {
        self.hasher.finalize().into()
    }
}

/// Compute standard SHA-256 digest of arbitrary input data.
pub fn compute_sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// 4-byte magic signature placed at the very end of a verified QymCAD bundle file.
pub const QICONS_TRAILER_MAGIC: &[u8; 4] = b"QCAD";

/// Current version of the trailer format.
pub const QICONS_TRAILER_VERSION: u32 = 1;

/// Total size of the trailer in bytes: 32 bytes SHA-256 + 4 bytes version + 4 bytes magic.
pub const QICONS_TRAILER_LEN: usize = 40;

/// Append the 40-byte QymCAD verification trailer to raw ZIP bytes.
pub fn append_qicons_trailer(zip_bytes: &mut Vec<u8>) {
    let hash = compute_sha256(zip_bytes);
    zip_bytes.extend_from_slice(&hash);
    zip_bytes.extend_from_slice(&QICONS_TRAILER_VERSION.to_le_bytes());
    zip_bytes.extend_from_slice(QICONS_TRAILER_MAGIC);
}

/// Result of checking the integrity trailer checksum on an archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrailerCheck {
    /// Package has a valid integrity checksum matching the archive bytes.
    IntegrityOk,
    /// Integrity checksum trailer was present, but archive bytes were modified or corrupted.
    IntegrityBad,
    /// Package does not contain an integrity trailer.
    NoTrailer,
}

/// Inspect the trailing bytes of a file to check if it has a valid QymCAD bundle signature.
pub fn verify_qicons_trailer(file_bytes: &[u8]) -> TrailerCheck {
    if file_bytes.len() < QICONS_TRAILER_LEN {
        return TrailerCheck::NoTrailer;
    }

    let len = file_bytes.len();
    let magic = &file_bytes[len - 4..];
    if magic != QICONS_TRAILER_MAGIC {
        return TrailerCheck::NoTrailer;
    }

    let version_bytes: [u8; 4] = file_bytes[len - 8..len - 4].try_into().unwrap();
    let version = u32::from_le_bytes(version_bytes);
    if version != QICONS_TRAILER_VERSION {
        return TrailerCheck::IntegrityBad;
    }

    let expected_hash = &file_bytes[len - 40..len - 8];
    let actual_hash = compute_sha256(&file_bytes[..len - 40]);

    if expected_hash == actual_hash {
        TrailerCheck::IntegrityOk
    } else {
        TrailerCheck::IntegrityBad
    }
}

/// Inspect the trailing bytes of a stream/file to check if it has a valid QymCAD bundle signature,
/// streaming content through SHA-256 without loading entire archive into RAM.
pub fn verify_qicons_trailer_stream<R: std::io::Read + std::io::Seek>(reader: &mut R, total_len: u64) -> std::io::Result<TrailerCheck> {
    if total_len < QICONS_TRAILER_LEN as u64 {
        return Ok(TrailerCheck::NoTrailer);
    }

    reader.seek(std::io::SeekFrom::Start(total_len - QICONS_TRAILER_LEN as u64))?;
    let mut trailer = [0u8; QICONS_TRAILER_LEN];
    reader.read_exact(&mut trailer)?;

    let magic = &trailer[36..40];
    if magic != QICONS_TRAILER_MAGIC {
        return Ok(TrailerCheck::NoTrailer);
    }

    let version = u32::from_le_bytes(trailer[32..36].try_into().unwrap());
    if version != QICONS_TRAILER_VERSION {
        return Ok(TrailerCheck::IntegrityBad);
    }

    let expected_hash = &trailer[0..32];

    reader.seek(std::io::SeekFrom::Start(0))?;
    let content_len = total_len - QICONS_TRAILER_LEN as u64;
    let mut remaining = content_len;
    let mut buf = [0u8; 64 * 1024];
    let mut hasher = Sha256Hasher::new();

    while remaining > 0 {
        let to_read = (remaining.min(buf.len() as u64)) as usize;
        let bytes_read = reader.read(&mut buf[..to_read])?;
        if bytes_read == 0 {
            return Ok(TrailerCheck::IntegrityBad);
        }
        hasher.update(&buf[..bytes_read]);
        remaining -= bytes_read as u64;
    }

    let actual_hash = hasher.finalize();
    if expected_hash == actual_hash {
        Ok(TrailerCheck::IntegrityOk)
    } else {
        Ok(TrailerCheck::IntegrityBad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qicons_trailer_verification() {
        let mut sample = b"PK\x03\x04dummy_zip_content".to_vec();

        // Initially without trailer
        assert_eq!(verify_qicons_trailer(&sample), TrailerCheck::NoTrailer);

        // Sign it with trailer
        append_qicons_trailer(&mut sample);
        assert_eq!(verify_qicons_trailer(&sample), TrailerCheck::IntegrityOk);

        // Modify 1 byte in the content
        sample[5] ^= 0xFF;
        assert_eq!(verify_qicons_trailer(&sample), TrailerCheck::IntegrityBad);
    }
}
