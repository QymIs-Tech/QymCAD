//! Importing STL, both ASCII and binary, into a triangle mesh.

use qymcad_core::geom::{Mesh, Point3};

/// Load an STL file into a mesh.
pub fn import_stl(path: &str) -> Result<Mesh, String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("STL open: {e}"))?;
    let stl = stl_io::read_stl(&mut file).map_err(|e| format!("STL read: {e}"))?;

    let verts = stl
        .vertices
        .iter()
        .map(|v| Point3::new(v[0] as f64, v[1] as f64, v[2] as f64))
        .collect();
    let tris = stl
        .faces
        .iter()
        .map(|f| [f.vertices[0] as u32, f.vertices[1] as u32, f.vertices[2] as u32])
        .collect();

    Ok(Mesh { verts, tris })
}
