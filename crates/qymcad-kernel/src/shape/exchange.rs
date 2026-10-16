//! STEP AND IGES: read and written, as solids, meshes and trees.

use super::*;

/// Read a STEP file and return a shape per solid, in parallel with [`import_step`].
pub fn step_solids(path: &str) -> Result<Vec<Shape>, String> {
    let c = CString::new(path).map_err(|e| e.to_string())?;
    unsafe {
        let l = qym_step_solids(c.as_ptr());
        if l.is_null() {
            return Err("cad-step-no-shapes".into());
        }
        let n = qym_shapelist_count(l);
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            if let Some(s) = Shape::wrap(qym_shapelist_get(l, i)) {
                out.push(s);
            }
        }
        qym_shapelist_free(l);
        Ok(out)
    }
}

/// Write bodies into a single STEP file as exact B-rep, each with its own 3×4 world transform, which for an
/// assembly comes from `body_world_transform`. The units are millimetres. `Err` on an empty set or a kernel
/// failure.
pub fn write_step(bodies: &[(&Shape, [f64; 12])], path: &str) -> Result<(), String> {
    if bodies.is_empty() {
        return Err("cad-step-nothing-to-export".into());
    }
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let ptrs: Vec<*const QymShape> = bodies.iter().map(|(s, _)| s.ptr as *const QymShape).collect();
    let mats: Vec<f64> = bodies.iter().flat_map(|(_, m)| *m).collect();
    let rc = unsafe { qym_step_write(ptrs.as_ptr(), mats.as_ptr(), ptrs.len(), c.as_ptr()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!("cad-step-write-failed#{rc}"))
    }
}

/// Read an exact file ONCE into both what is shown and what is built on: the bodies (mesh plus B-rep faces, one per
/// solid) and the live shapes, in the same order. See `qym_exact_read`: reading twice cost the whole wait twice.
pub fn read_exact(format: ExactFormat, path: &str, deflection: f64) -> Result<(Vec<Built>, Vec<Shape>), String> {
    if let Some(said) = missing_file(path) {
        return Err(said);
    }
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let defl = if deflection > 0.0 { deflection } else { 0.5 };
    let code = match format {
        ExactFormat::Step => 0,
        ExactFormat::Iges => 1,
    };
    let (mut doc, mut list): (*mut QymDoc, *mut QymShapeList) = (std::ptr::null_mut(), std::ptr::null_mut());
    let rc = unsafe { qym_exact_read(code, c.as_ptr(), defl, &mut doc, &mut list) };
    // the codes written out whole, so that a search for one finds the place that emits it
    let (read_failed, empty) = match format {
        ExactFormat::Step => ("cad-step-read-failed", "cad-step-empty-tessellation"),
        ExactFormat::Iges => ("cad-iges-read-failed", "cad-iges-empty-tessellation"),
    };
    if rc != 0 || doc.is_null() || list.is_null() {
        unsafe {
            if !doc.is_null() {
                qym_doc_free(doc);
            }
            if !list.is_null() {
                qym_shapelist_free(list);
            }
        }
        return Err(read_failed.to_string());
    }
    let bodies = unsafe { doc_to_bodies(doc) };
    let shapes = unsafe {
        let n = qym_shapelist_count(list);
        let out: Vec<Shape> = (0..n).filter_map(|i| Shape::wrap(qym_shapelist_get(list, i))).collect();
        qym_shapelist_free(list);
        out
    };
    if bodies.is_empty() {
        return Err(empty.to_string());
    }
    Ok((bodies, shapes))
}

/// A PATH WITH NO FILE BEHIND IT, said as such: `cad-file-not-found#<path>`. Asked before the kernel is, because the
/// kernel's reader answers a missing file exactly as a broken one, and "the geometry could not be read" sends a person
/// looking for a fault in a file that is not there.
pub(crate) fn missing_file(path: &str) -> Option<String> {
    (!std::path::Path::new(path).is_file()).then(|| format!("cad-file-not-found#{path}"))
}

/// THE EXACT FORMATS: files that carry faces and edges rather than triangles, read into live solids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactFormat {
    Step,
    Iges,
}

/// The unit a file is written in. The model is in millimetres; a file in inches carries converted coordinates
/// and says so in its header, which is what a reader on the other side goes by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LengthUnit {
    Millimetre,
    Inch,
}

impl ExactFormat {
    /// The format of a file by its extension: `igs` and `iges` are IGES, anything else is taken for STEP - which
    /// is also what a source saved before its extension was kept comes back as.
    pub fn of_extension(ext: &str) -> ExactFormat {
        match ext.to_ascii_lowercase().as_str() {
            "igs" | "iges" => ExactFormat::Iges,
            _ => ExactFormat::Step,
        }
    }
}

/// Read an exact file into bodies, one per solid, each with its B-rep faces.
pub fn import_exact(format: ExactFormat, path: &str, deflection: f64) -> Result<Vec<Built>, String> {
    match format {
        ExactFormat::Step => import_step(path, deflection),
        ExactFormat::Iges => import_iges(path, deflection),
    }
}

/// Read an exact file into a live shape per solid, in the order [`import_exact`] gives its bodies.
pub fn exact_solids(format: ExactFormat, path: &str) -> Result<Vec<Shape>, String> {
    match format {
        ExactFormat::Step => step_solids(path),
        ExactFormat::Iges => iges_solids(path),
    }
}

/// One node of an imported assembly: a subassembly, or a part carrying a body.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportNode {
    /// The name the file gives it, in UTF-8; empty where it gives none.
    pub name: String,
    pub parent: Option<usize>,
    /// Where it stands in its parent, a 3x4 row-major placement.
    pub place: [f64; 12],
    /// The body it carries - an index into the bodies and solids read - or `None` for a subassembly.
    pub solid: Option<usize>,
    /// Its colour, sRGB, where the file gives one.
    pub color: Option<[f32; 3]>,
    /// For an occurrence of a product met before: the body of its first occurrence, which it repeats - the file
    /// holds the product once, and so does the reading.
    pub repeat_of: Option<usize>,
    /// Colours the file gives single faces of the body, sRGB, by the face's persistent id - the number the body's face
    /// gets when the body is wrapped (`qym_born` in the bridge).
    pub faces: Vec<(u32, [f32; 3])>,
}

/// The tree read as the document takes it: the flat list of `read_exact_tree` (each node naming its parent by place
/// in the list) nested, every body by the id `bodies` gives it in the document. A node the file leaves unnamed -
/// every one in IGES - is named after the file, `stem`, and numbered by its body when more than one body came in.
/// A node without a colour of its own takes its parent's: a STEP assembly colours an occurrence for all under it.
pub fn document_tree(nodes: &[ImportNode], bodies: &[u64], stem: &str) -> Vec<qymcad_core::model::ImportNode> {
    let mut kids = vec![Vec::new(); nodes.len()];
    let mut tops = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        match n.parent {
            Some(p) if p < nodes.len() => kids[p].push(i),
            _ => tops.push(i),
        }
    }
    fn nest(i: usize, nodes: &[ImportNode], kids: &[Vec<usize>], bodies: &[u64], stem: &str, above: Option<[f32; 3]>) -> qymcad_core::model::ImportNode {
        let n = &nodes[i];
        let tint = n.color.or(above);
        let name = match (n.name.is_empty(), n.solid) {
            (false, _) => n.name.clone(),
            (true, Some(k)) if bodies.len() > 1 => format!("{stem} {}", k + 1),
            _ => stem.to_string(),
        };
        let children = kids[i].iter().map(|&c| nest(c, nodes, kids, bodies, stem, tint)).collect();
        qymcad_core::model::ImportNode {
            name,
            place: n.place,
            body: n.solid.and_then(|k| bodies.get(k).copied()),
            solid: n.solid.unwrap_or(0) as u32,
            color: tint.map(|c| c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)),
            face_colors: n.faces.iter().map(|(id, c)| (*id, c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8))).collect(),
            tri_colors: Vec::new(),
            repeat_of: n.repeat_of.map(|k| k as u32),
            mesh: false,
            children,
        }
    }
    tops.into_iter().map(|i| nest(i, nodes, &kids, bodies, stem, None)).collect()
}

/// AN EXACT FILE READ WITH ITS STRUCTURE: the bodies as meshes, the live solids they come from, and the tree they
/// stand in.
pub struct ExactTree {
    pub bodies: Vec<Built>,
    pub shapes: Vec<Shape>,
    pub nodes: Vec<ImportNode>,
}

/// Read an exact file with its structure: the bodies and solids as `read_exact` gives them, and the tree they stand
/// in.
///
/// An IGES file with neither subfigures nor solids standing on their own - surfaces, sewn - comes as a node per solid,
/// unnamed, where the file puts it.
pub fn read_exact_tree(format: ExactFormat, path: &str, deflection: f64) -> Result<ExactTree, String> {
    if let Some(said) = missing_file(path) {
        return Err(said);
    }
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let defl = if deflection > 0.0 { deflection } else { 0.5 };
    let code = match format {
        ExactFormat::Step => 0,
        ExactFormat::Iges => 1,
    };
    let (mut doc, mut list, mut tree): (*mut QymDoc, *mut QymShapeList, *mut QymTree) = (std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
    let rc = unsafe { qym_exact_read_tree(code, c.as_ptr(), defl, &mut doc, &mut list, &mut tree) };
    let read_failed = match format {
        ExactFormat::Step => "cad-step-read-failed",
        ExactFormat::Iges => "cad-iges-read-failed",
    };
    if rc != 0 || doc.is_null() || list.is_null() {
        unsafe {
            if !doc.is_null() {
                qym_doc_free(doc);
            }
            if !list.is_null() {
                qym_shapelist_free(list);
            }
            if !tree.is_null() {
                qym_tree_free(tree);
            }
        }
        return Err(read_failed.to_string());
    }
    let bodies = unsafe { doc_to_bodies(doc) };
    let shapes: Vec<Shape> = unsafe {
        let n = qym_shapelist_count(list);
        let out = (0..n).filter_map(|i| Shape::wrap(qym_shapelist_get(list, i))).collect();
        qym_shapelist_free(list);
        out
    };
    let nodes = if tree.is_null() {
        (0..shapes.len())
            .map(|i| ImportNode { name: String::new(), parent: None, place: qymcad_core::feature::PLACE_IDENTITY, solid: Some(i), color: None, repeat_of: None, faces: Vec::new() })
            .collect()
    } else {
        unsafe {
            let n = qym_tree_count(tree);
            let out = (0..n)
                .map(|i| {
                    let mut place = [0.0f64; 12];
                    qym_tree_place(tree, i, place.as_mut_ptr());
                    let mut rgb = [0.0f32; 3];
                    let color = (qym_tree_color(tree, i, rgb.as_mut_ptr()) == 1).then_some(rgb);
                    let name = std::ffi::CStr::from_ptr(qym_tree_name(tree, i)).to_string_lossy().into_owned();
                    let parent = usize::try_from(qym_tree_parent(tree, i)).ok();
                    let solid = usize::try_from(qym_tree_solid(tree, i)).ok();
                    let repeat_of = usize::try_from(qym_tree_repeat_of(tree, i)).ok();
                    let n = qym_tree_face_colours(tree, i, std::ptr::null_mut(), std::ptr::null_mut(), 0);
                    let (mut ids, mut rgb) = (vec![0u32; n], vec![0.0f32; 3 * n]);
                    qym_tree_face_colours(tree, i, ids.as_mut_ptr(), rgb.as_mut_ptr(), n);
                    let faces = ids.into_iter().enumerate().map(|(k, id)| (id, [rgb[3 * k], rgb[3 * k + 1], rgb[3 * k + 2]])).collect();
                    ImportNode { name, parent, place, solid, color, repeat_of, faces }
                })
                .collect();
            qym_tree_free(tree);
            out
        }
    };
    Ok(ExactTree { bodies, shapes, nodes })
}

/// Write a tree into a STEP file as its assembly: products, their occurrences, names and colours (see
/// `ExportNode`). `shapes` gives the live body of every part.
pub fn write_step_tree(nodes: &[qymcad_core::model::ExportNode], shapes: &[(u64, &Shape)], path: &str) -> Result<(), String> {
    if nodes.is_empty() {
        return Err("cad-step-nothing-to-export".into());
    }
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let names: Vec<CString> = nodes.iter().map(|n| CString::new(n.name.replace('\0', "")).unwrap_or_default()).collect();
    let name_ptrs: Vec<*const c_char> = names.iter().map(|s| s.as_ptr()).collect();
    let parents: Vec<i64> = nodes.iter().map(|n| n.parent.map_or(-1, |p| p as i64)).collect();
    let same_as: Vec<i64> = nodes.iter().map(|n| n.same_as.map_or(-1, |p| p as i64)).collect();
    let places: Vec<f64> = nodes.iter().flat_map(|n| n.place).collect();
    let by_id: std::collections::HashMap<u64, &Shape> = shapes.iter().copied().collect();
    let shape_ptrs: Vec<*const QymShape> = nodes.iter().map(|n| n.body.and_then(|b| by_id.get(&b)).map_or(std::ptr::null(), |s| s.ptr as *const QymShape)).collect();
    let rgb: Vec<f32> = nodes.iter().flat_map(|n| n.color.map_or([0.0; 3], |c| c.map(|v| v as f32 / 255.0))).collect();
    let has_rgb: Vec<i32> = nodes.iter().map(|n| n.color.is_some() as i32).collect();
    // the colours of single faces, node after node: where each node's list starts, the faces' persistent ids, their sRGB
    let mut face_starts: Vec<usize> = Vec::with_capacity(nodes.len() + 1);
    let (mut face_ids, mut face_rgb): (Vec<u32>, Vec<f32>) = (Vec::new(), Vec::new());
    for n in nodes {
        face_starts.push(face_ids.len());
        for (id, c) in &n.face_colors {
            face_ids.push(*id);
            face_rgb.extend(c.map(|v| v as f32 / 255.0));
        }
    }
    face_starts.push(face_ids.len());
    let rc = unsafe {
        qym_step_write_tree(
            nodes.len(),
            parents.as_ptr(),
            name_ptrs.as_ptr(),
            places.as_ptr(),
            shape_ptrs.as_ptr(),
            same_as.as_ptr(),
            rgb.as_ptr(),
            has_rgb.as_ptr(),
            face_starts.as_ptr(),
            face_ids.as_ptr(),
            face_rgb.as_ptr(),
            c.as_ptr(),
        )
    };
    if rc != 0 {
        return Err(format!("cad-step-write-failed#{rc}"));
    }
    // the writer puts the names down as raw UTF-8; they are spelled over as the format holds them
    match std::fs::read_to_string(path) {
        Ok(text) if !text.is_ascii() => std::fs::write(path, step_spelled(&text)).map_err(|e| e.to_string()),
        _ => Ok(()),
    }
}

/// A STEP file's text with everything past ASCII spelled as ISO 10303-21 holds it: a run of characters of the basic
/// plane as `\X2\` and four hex digits each, one past it as `\X4\` and eight, the run closed by `\X0\`. A string in
/// the file is ISO 8859-1, and a reader that keeps to the format shows raw UTF-8 as mojibake; the other CAD systems
/// spell a Cyrillic name so. Only names carry such characters, so the whole text is gone over.
pub fn step_spelled(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    let mut run = None; // Some(wide) inside a run of four (false) or eight (true) digits
    for ch in text.chars() {
        let want = (!ch.is_ascii()).then_some(ch as u32 > 0xFFFF);
        if want != run {
            if run.is_some() {
                out.push_str("\\X0\\");
            }
            if let Some(wide) = want {
                out.push_str(if wide { "\\X4\\" } else { "\\X2\\" });
            }
            run = want;
        }
        match want {
            Some(true) => out.push_str(&format!("{:08X}", ch as u32)),
            Some(false) => out.push_str(&format!("{:04X}", ch as u32)),
            None => out.push(ch),
        }
    }
    if run.is_some() {
        out.push_str("\\X0\\");
    }
    out
}

/// Write bodies into one exact file, in millimetres.
pub fn write_exact(format: ExactFormat, bodies: &[(&Shape, [f64; 12])], path: &str) -> Result<(), String> {
    match format {
        ExactFormat::Step => write_step(bodies, path),
        ExactFormat::Iges => write_iges(bodies, path, LengthUnit::Millimetre),
    }
}

/// Read an IGES file into bodies. Faces that close are sewn into solids (see `iges_shape` in the bridge).
pub fn import_iges(path: &str, deflection: f64) -> Result<Vec<Built>, String> {
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let defl = if deflection > 0.0 { deflection } else { 0.5 };
    unsafe {
        let d = qym_occt_iges_read(c.as_ptr(), defl);
        if d.is_null() {
            return Err("cad-iges-read-failed".into());
        }
        let bodies = doc_to_bodies(d);
        if bodies.is_empty() {
            return Err("cad-iges-empty-tessellation".into());
        }
        Ok(bodies)
    }
}

/// Read an IGES file into a live shape per solid, in parallel with [`import_iges`].
pub fn iges_solids(path: &str) -> Result<Vec<Shape>, String> {
    let c = CString::new(path).map_err(|e| e.to_string())?;
    unsafe {
        let l = qym_iges_solids(c.as_ptr());
        if l.is_null() {
            return Err("cad-iges-no-shapes".into());
        }
        let n = qym_shapelist_count(l);
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            if let Some(s) = Shape::wrap(qym_shapelist_get(l, i)) {
                out.push(s);
            }
        }
        qym_shapelist_free(l);
        Ok(out)
    }
}

/// Write bodies into one IGES file, each with its own 3x4 world transform, in `unit`.
pub fn write_iges(bodies: &[(&Shape, [f64; 12])], path: &str, unit: LengthUnit) -> Result<(), String> {
    if bodies.is_empty() {
        return Err("cad-iges-nothing-to-export".into());
    }
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let u = CString::new(match unit {
        LengthUnit::Millimetre => "MM",
        LengthUnit::Inch => "IN",
    })
    .map_err(|e| e.to_string())?;
    let ptrs: Vec<*const QymShape> = bodies.iter().map(|(s, _)| s.ptr as *const QymShape).collect();
    let mats: Vec<f64> = bodies.iter().flat_map(|(_, m)| *m).collect();
    let rc = unsafe { qym_iges_write(ptrs.as_ptr(), mats.as_ptr(), ptrs.len(), c.as_ptr(), u.as_ptr()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!("cad-iges-write-failed#{rc}"))
    }
}

/// Copy every body out of an opaque handle and free it.
///
/// # Safety
/// `d` is a valid non-null pointer from `qym_occt_*` and is not used afterwards.
pub(crate) unsafe fn doc_to_bodies(d: *mut QymDoc) -> Vec<Built> {
    let nb = qym_doc_body_count(d);
    let mut out = Vec::with_capacity(nb);
    for i in 0..nb {
        let vc = qym_body_vert_count(d, i);
        let tc = qym_body_tri_count(d, i);
        let fc = qym_body_face_count(d, i);

        let mut vbuf = vec![0f32; vc * 3];
        let mut tbuf = vec![0u32; tc * 3];
        if vc > 0 {
            qym_body_copy_verts(d, i, vbuf.as_mut_ptr());
        }
        if tc > 0 {
            qym_body_copy_tris(d, i, tbuf.as_mut_ptr());
        }
        let verts = vbuf.as_chunks::<3>().0.iter().map(|c| Point3::new(c[0] as f64, c[1] as f64, c[2] as f64)).collect();
        let tris = tbuf.as_chunks::<3>().0.iter().map(|c| [c[0], c[1], c[2]]).collect();
        // THE SURFACE'S NORMALS, parallel to the vertices: the smooth light is shaded by them (`Mesh::norms`)
        let mut nbuf = vec![0f32; vc * 3];
        if vc > 0 {
            qym_body_copy_normals(d, i, nbuf.as_mut_ptr());
        }
        let norms = nbuf.as_chunks::<3>().0.iter().map(|c| qymcad_core::geom::pack_direction([c[0] as f64, c[1] as f64, c[2] as f64])).collect();
        let mesh = Mesh { verts, tris, norms };

        // faces from the B-rep: each is a contiguous range of triangles plus a persistent id
        let mut starts = vec![0u32; fc];
        let mut counts = vec![0u32; fc];
        let mut ids = vec![0u32; fc];
        if fc > 0 {
            qym_body_copy_face_starts(d, i, starts.as_mut_ptr());
            qym_body_copy_face_counts(d, i, counts.as_mut_ptr());
            qym_body_copy_face_ids(d, i, ids.as_mut_ptr());
        }
        let mut anchors = vec![0f64; fc * 7];
        if fc > 0 {
            qym_body_copy_face_anchors(d, i, anchors.as_mut_ptr());
        }
        let faces = starts
            .iter()
            .zip(counts.iter())
            .zip(ids.iter())
            .enumerate()
            .map(|(k, ((&s, &c), &fid))| {
                let mut f = mesh.meshface_from_triangles((s..s + c).collect());
                f.id = fid;
                // An exact anchor from the B-rep rather than from the tessellation: the normal is analytic,
                // and for a plane the centroid is projected onto the exact plane. Otherwise a sketch on a face
                // landed with an error of about 1e-5, bosses came out nearly coplanar, the unifier did not
                // merge the seams, and the result was fragmentation and red fillets.
                let a = &anchors[k * 7..k * 7 + 7];
                let n2 = a[3] * a[3] + a[4] * a[4] + a[5] * a[5];
                if n2 > 0.5 {
                    f.normal = [a[3], a[4], a[5]];
                    if a[6] > 0.5 {
                        let d = (f.centroid.x - a[0]) * a[3] + (f.centroid.y - a[1]) * a[4] + (f.centroid.z - a[2]) * a[5];
                        f.centroid.x -= d * a[3];
                        f.centroid.y -= d * a[4];
                        f.centroid.z -= d * a[5];
                    }
                }
                f
            })
            .collect();

        out.push(Built { mesh, faces });
    }
    qym_doc_free(d);
    out
}

/// Import a STEP file into bodies, one per solid, each with its B-rep faces. `deflection` is the linear
/// tessellation deflection in mm.
pub fn import_step(path: &str, deflection: f64) -> Result<Vec<Built>, String> {
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let defl = if deflection > 0.0 { deflection } else { 0.5 };
    unsafe {
        let d = qym_occt_step_read(c.as_ptr(), defl);
        if d.is_null() {
            return Err("cad-step-read-failed".into());
        }
        let bodies = doc_to_bodies(d);
        if bodies.is_empty() {
            return Err("cad-step-empty-tessellation".into());
        }
        Ok(bodies)
    }
}
