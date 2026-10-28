//! THE DOCUMENT AS A MODEL READS IT: what a person learns from the tree, the properties and the measuring tool, as
//! JSON. Nothing here changes the document.
//!
//! Faces and edges are listed with where they stand and which way they look, because that is what a model picks them
//! by: "the top face" is the face whose normal is +Z and whose centre is highest, read off the list, not a guess at an
//! index.
use serde_json::{json, Value};

use crate::workshop::Workshop;
use qymcad_core::model::Id;

/// Coordinates are given to a micrometre: more digits are noise to a reader and cost a model tokens.
fn round(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn round3(v: [f64; 3]) -> [f64; 3] {
    [round(v[0]), round(v[1]), round(v[2])]
}

/// The kind of a node by the program's own name for it (`Extrude`, `Fillet`), the same in every language.
pub fn kind_of(kind: &qymcad_core::feature::FeatureKind) -> String {
    format!("{kind:?}").split([' ', '{', '(']).next().unwrap_or_default().to_string()
}

/// The whole document: parts, sketches, the timeline with what failed, the bodies measured, the parameters.
pub fn document(ws: &dyn Workshop) -> Value {
    let p = ws.project();
    let parts: Vec<Value> =
        p.components.iter().filter(|c| c.parent.is_some()).map(|c| json!({ "id": c.id, "name": qymcad_i18n::name(&c.name), "kind": format!("{:?}", c.kind), "parent": c.parent })).collect();
    let sketches: Vec<Value> =
        p.sketches.iter().map(|s| json!({ "id": s.id, "name": qymcad_i18n::name(&s.name), "plane": format!("{:?}", s.plane), "contours": s.contour_ids, "entities": s.entities.len() })).collect();
    let features: Vec<Value> = p
        .timeline
        .iter()
        .map(|n| {
            json!({
                "id": n.id,
                "name": qymcad_i18n::name(&n.name),
                "kind": kind_of(&n.kind),
                "suppressed": n.suppressed,
                "error": p.regen_errors.get(&n.id).map(|e| format!("{e:?}")),
                "warning": p.regen_warnings.get(&n.id).map(|e| format!("{e:?}")),
            })
        })
        .collect();
    let bodies: Vec<Value> = ws.shown_bodies().into_iter().map(|id| body_summary(ws, id)).collect();
    let parameters: Vec<Value> = p.parameters.iter().map(|q| json!({ "name": q.name, "expression": q.expr, "value": q.value })).collect();
    json!({
        "where": ws.place(),
        "path": ws.path(),
        "active_part": p.active_component,
        "parts": parts,
        "sketches": sketches,
        "features": features,
        "bodies": bodies,
        "parameters": parameters,
    })
}

/// One body measured: its volume, its box, how many faces and edges it has.
fn body_summary(ws: &dyn Workshop, id: Id) -> Value {
    let p = ws.project();
    let Some(b) = p.bodies.iter().find(|b| b.id == id) else { return json!({ "id": id }) };
    let mut mesh = b.mesh.clone();
    mesh.transform(&p.body_world_transform(id));
    let bounds = mesh.bounds();
    json!({
        "id": id,
        "name": qymcad_i18n::name(&b.name),
        "part": p.body_owner(id),
        "volume_mm3": round(ws.exact_volume(id).unwrap_or_else(|| b.mesh.volume())),
        "box_min": bounds.map(|bb| round3([bb.min.x, bb.min.y, bb.min.z])),
        "box_max": bounds.map(|bb| round3([bb.max.x, bb.max.y, bb.max.z])),
        "faces": p.regen_faces.get(&id).map_or(0, Vec::len),
        "edges": p.regen_edges.get(&id).map_or(0, Vec::len),
        "sheet": b.sheet,
    })
}

/// The faces and edges of one body by their persistent ids, with where each stands in the body's own frame - the
/// frame the operations take them in.
pub fn body(ws: &dyn Workshop, id: Id) -> Result<Value, String> {
    let p = ws.project();
    if !p.bodies.iter().any(|b| b.id == id) {
        return Err(format!("there is no body {id}; `describe_document` lists the bodies"));
    }
    let faces: Vec<Value> = p
        .regen_faces
        .get(&id)
        .map(|fs| {
            fs.iter()
                .filter(|f| f.id != 0)
                .map(|f| json!({ "id": f.id, "centre": round3([f.centroid.x, f.centroid.y, f.centroid.z]), "normal": round3(f.normal), "area_mm2": round(f.area) }))
                .collect()
        })
        .unwrap_or_default();
    let edges: Vec<Value> = p
        .regen_edges
        .get(&id)
        .map(|es| {
            es.iter()
                .filter(|e| e.id != 0)
                .map(|e| {
                    let mut row = json!({ "id": e.id, "middle": round3(e.mid), "direction": round3(e.dir), "from": round3(e.a), "to": round3(e.b) });
                    if e.radius > 1e-9 {
                        row["radius"] = json!(round(e.radius));
                        row["centre"] = json!(round3(e.center));
                        row["axis"] = json!(round3(e.axis));
                    }
                    row
                })
                .collect()
        })
        .unwrap_or_default();
    let mut answer = body_summary(ws, id);
    answer["face_list"] = json!(faces);
    answer["edge_list"] = json!(edges);
    Ok(answer)
}
