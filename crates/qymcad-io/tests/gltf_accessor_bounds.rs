//! A glTF ACCESSOR IS READ ONLY WHEN ITS DATA IS THERE. The count and the stride come from the file; before this check
//! the importer reserved `count` items first, so a file of a few hundred bytes claiming 4,000,000,000 points asked for
//! 96,000,000,000 bytes and the failed request aborted the whole program. Reported behaviour (issue #100).
//!
//! A binary of its own: on a tree without the check the first test aborts the process, and an abort here names this
//! file rather than every glTF check beside it.
use qymcad_io::import_gltf;

/// A folder for the files, under `target`: nothing goes to `/tmp`, which lives in memory.
fn file(name: &str, text: &str) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/gltf-accessor-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join(name);
    std::fs::write(&p, text).expect("written");
    p.to_string_lossy().into_owned()
}

fn b64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            s.push(if k <= c.len() { A[(n >> (18 - 6 * k) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

/// One triangle whose positions are read through `accessor` and `view`; its 36 bytes are the whole buffer.
fn triangle_with(accessor: serde_json::Value, view: serde_json::Value) -> String {
    let bin: Vec<u8> = [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0].iter().flat_map(|f| f.to_le_bytes()).collect();
    serde_json::json!({
        "asset": {"version": "2.0"}, "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}],
        "accessors": [accessor],
        "bufferViews": [view],
        "buffers": [{"byteLength": bin.len(), "uri": format!("data:application/octet-stream;base64,{}", b64(&bin))}]
    })
    .to_string()
}

fn refused(name: &str, accessor: serde_json::Value, view: serde_json::Value) -> Option<String> {
    import_gltf(&file(name, &triangle_with(accessor, view))).err()
}

#[test]
fn a_count_far_past_the_data_is_refused_not_reserved() {
    let a = serde_json::json!({"bufferView": 0, "componentType": 5126, "count": 4_000_000_000u64, "type": "VEC3"});
    let v = serde_json::json!({"buffer": 0, "byteLength": 36});
    assert_eq!(refused("huge-count.gltf", a, v).as_deref(), Some("io-gltf-bad-accessor"));
}

/// A STRIDE OF 0 IS NOT A WAY TO REPEAT THREE FLOATS FOUR BILLION TIMES: it reads as items side by side, so the count
/// must still fit the data.
#[test]
fn a_zero_stride_does_not_let_a_huge_count_through() {
    let a = serde_json::json!({"bufferView": 0, "componentType": 5126, "count": 4_000_000_000u64, "type": "VEC3"});
    let v = serde_json::json!({"buffer": 0, "byteLength": 36, "byteStride": 0});
    assert_eq!(refused("zero-stride.gltf", a, v).as_deref(), Some("io-gltf-bad-accessor"));
}

/// A STRIDE THAT JUMPS FAR PAST THE DATA: the second item would start four billion bytes in.
#[test]
fn a_huge_stride_is_refused() {
    let a = serde_json::json!({"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"});
    let v = serde_json::json!({"buffer": 0, "byteLength": 36, "byteStride": 4_000_000_000u64});
    assert_eq!(refused("huge-stride.gltf", a, v).as_deref(), Some("io-gltf-bad-accessor"));
}

#[test]
fn a_stride_narrower_than_one_item_is_refused() {
    let a = serde_json::json!({"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"});
    let v = serde_json::json!({"buffer": 0, "byteLength": 36, "byteStride": 4});
    assert_eq!(refused("narrow-stride.gltf", a, v).as_deref(), Some("io-gltf-bad-accessor"));
}

/// The data of an accessor lies inside its buffer view; reading on past the view's end reads another view's bytes.
#[test]
fn an_accessor_does_not_read_past_its_view() {
    let a = serde_json::json!({"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"});
    let v = serde_json::json!({"buffer": 0, "byteLength": 24});
    assert_eq!(refused("past-view.gltf", a, v).as_deref(), Some("io-gltf-bad-accessor"));
}

#[test]
fn offsets_that_overflow_are_refused() {
    let a = serde_json::json!({"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "byteOffset": u64::MAX});
    let v = serde_json::json!({"buffer": 0, "byteLength": 36});
    assert_eq!(refused("overflow.gltf", a, v).as_deref(), Some("io-gltf-bad-accessor"));
}

/// And an honest triangle still comes in, with and without a stride written out.
#[test]
fn an_honest_accessor_still_reads() {
    let a = serde_json::json!({"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"});
    for (name, v) in [("packed.gltf", serde_json::json!({"buffer": 0, "byteLength": 36})), ("strided.gltf", serde_json::json!({"buffer": 0, "byteLength": 36, "byteStride": 12}))] {
        let meshes = import_gltf(&file(name, &triangle_with(a.clone(), v))).unwrap_or_else(|e| panic!("{name} refused: {e}"));
        assert_eq!(meshes.iter().map(|m| m.mesh.tris.len()).sum::<usize>(), 1, "{name} did not come in as one triangle");
    }
}
