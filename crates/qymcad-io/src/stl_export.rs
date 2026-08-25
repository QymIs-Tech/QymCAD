//! Binary STL export.
//!
//! The triangles come from meshes already assembled in world coordinates; the caller applies the component
//! transform through `Mesh::transform`. The quality, that is how finely arcs and fillets are resolved, is chosen
//! before tessellation by the deflection used when re-tessellating the shape.
use qymcad_core::geom::Mesh;
use std::io::Write;

/// Write a set of meshes into one binary STL file. The face normals are computed from the vertices by the
/// right-hand rule. Empty input, with no triangles, is an error: there is nothing to export.
pub fn export_stl(meshes: &[Mesh], path: &str) -> Result<(), String> {
    let total: usize = meshes.iter().map(|m| m.tris.len()).sum();
    if total == 0 {
        return Err("io-stl-no-triangles".into());
    }
    if total > u32::MAX as usize {
        return Err("io-stl-too-many-triangles".into());
    }
    let mut buf: Vec<u8> = Vec::with_capacity(84 + total * 50);
    buf.extend_from_slice(&[0u8; 80]); // the header, left empty
    buf.extend_from_slice(&(total as u32).to_le_bytes());
    for m in meshes {
        for t in &m.tris {
            let a = m.verts[t[0] as usize];
            let b = m.verts[t[1] as usize];
            let c = m.verts[t[2] as usize];
            // the normal is (b−a)×(c−a), normalised; zero for a degenerate triangle
            let u = [b.x - a.x, b.y - a.y, b.z - a.z];
            let v = [c.x - a.x, c.y - a.y, c.z - a.z];
            let mut n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len > 1e-12 {
                n = [n[0] / len, n[1] / len, n[2] / len];
            }
            for f in n {
                buf.extend_from_slice(&(f as f32).to_le_bytes());
            }
            for p in [a, b, c] {
                buf.extend_from_slice(&(p.x as f32).to_le_bytes());
                buf.extend_from_slice(&(p.y as f32).to_le_bytes());
                buf.extend_from_slice(&(p.z as f32).to_le_bytes());
            }
            buf.extend_from_slice(&[0u8; 2]); // attribute byte count
        }
    }
    std::fs::File::create(path)
        .and_then(|mut f| f.write_all(&buf))
        .map_err(|e| format!("io-stl-write-failed#{e}"))
}
