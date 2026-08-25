//! "EXPAND THE SELECTION": turn a clicked face or edge into a DESCRIPTION.
//!
//! The `Oriented` / `Extreme` / `Largest` / `Between` queries have worked in the kernel from the very
//! beginning, but there was nothing to say them with from the program: the commands lay down manual
//! picks only. Here appears the place where a person says it — a context menu on the selection.
//!
//! WHY A MENU AND NOT A LIST IN THE COMMAND BAR. A person starts from a particular face anyway: they
//! click, and then want "and all the ones like it". A menu continues that gesture, while a list in the
//! bar would force a blind choice before anything was clicked. Grown-up CAD does the same: the right
//! button on a selection offers to expand it, or to select everything tangentially connected.
//!
//! ONE LIST FOR THE MENU AND FOR THE QUERIES. A menu item and the query it builds live in one table,
//! [`EXPANSIONS`]. Two places knowing one thing would drift apart at the very first new row — and this
//! project has already paid for that: both the command catalogue and the settings table were fixed in
//! exactly that way.

use qymcad_core::refs::{Axis, Query};

/// What is being expanded: the selected face or the selected edge. Their sets of meaningful items
/// differ.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Picked {
    Face(u32),
    Edge(u32),
}

/// One expand-the-selection item: the key of the caption and how a query is built from the pick.
pub(crate) struct Expansion {
    /// The key of the language catalogue — the caption of the item.
    pub key: &'static str,
    /// What the item is reckoned from — the clicked FACE or the clicked EDGE.
    pub on_face: bool,
    pub on_edge: bool,
    /// WHAT THE ITEM YIELDS: edges or faces, and how many — one or a set.
    ///
    /// Declared rather than derived from the query, and that is not duplication. The junction item
    /// holds a stub in the table (the real query is assembled after the SECOND pick), and deriving from
    /// it would lie: it would say "a face" while a junction yields edges. A lying flag is worse than a
    /// missing one — it is what the filtering of the menu goes by.
    pub gives_edges: bool,
    pub gives_many: bool,
    /// How to build the query from the clicked element and its geometry.
    pub build: fn(Picked, [f64; 3]) -> Query,
}

/// EVERY ITEM. The order runs from common to rare: first what the menu is opened for.
pub(crate) const EXPANSIONS: &[Expansion] = &[
    Expansion {
        key: "expand-face-edges",
        on_face: true,
        on_edge: false,
        gives_edges: true,
        gives_many: true,
        build: |p, _| match p {
            Picked::Face(f) => Query::Adjacent(Box::new(Query::Id(f))),
            Picked::Edge(e) => Query::Id(e),
        },
    },
    Expansion {
        key: "expand-parallel",
        on_face: true,
        on_edge: false,
        gives_edges: false,
        gives_many: true,
        // A TOLERANCE IN DEGREES RATHER THAN "EXACTLY PARALLEL". There are no exact ones in a
        // tessellated body: a cylinder is cut into strips, and "strictly the same normal" would not
        // even find the face itself.
        build: |_, n| Query::Oriented { dir: n, tol_deg: 5.0 },
    },
    Expansion {
        key: "expand-topmost",
        on_face: true,
        on_edge: false,
        gives_edges: false,
        gives_many: false,
        build: |_, _| Query::Extreme { axis: Axis::Z, max: true },
    },
    Expansion {
        key: "expand-largest",
        on_face: true,
        on_edge: false,
        gives_edges: false,
        gives_many: false,
        build: |_, _| Query::Largest,
    },
    Expansion {
        key: "expand-tangent-chain",
        on_face: false,
        on_edge: true,
        gives_edges: true,
        gives_many: true,
        // A TOLERANCE OF 5 DEGREES RATHER THAN "EXACTLY TANGENT". Real models carry noise from snaps
        // and tessellation; strict collinearity would break the chain for no reason. Grown-up CAD takes
        // the same.
        build: |p, _| match p {
            Picked::Edge(e) => Query::TangentChain { seed: Box::new(Query::Id(e)), tol_deg: 5.0 },
            Picked::Face(f) => Query::Id(f),
        },
    },
    Expansion {
        key: "expand-between",
        on_face: true,
        on_edge: false,
        gives_edges: true,
        gives_many: true,
        // A STUB: the real query is assembled AFTER the second pick — a junction has two sides, and one
        // face pointed at is not enough for it. See `apply_expansion`.
        build: |p, _| match p {
            Picked::Face(f) => Query::Id(f),
            Picked::Edge(e) => Query::Id(e),
        },
    },
    Expansion {
        key: "expand-feature-faces",
        on_face: true,
        on_edge: false,
        gives_edges: false,
        gives_many: true,
        // FILLED IN ON THE SPOT: which feature gave birth to a face is known to the name table of the
        // document, not to this table. A stub stands here, and `expansion_query` assembles the real
        // query.
        build: |p, _| match p {
            Picked::Face(f) => Query::Id(f),
            Picked::Edge(e) => Query::Id(e),
        },
    },
];

impl super::App {
    /// THE BODY A FACE BELONGS TO — by the live rebuild.
    ///
    /// The menu needs it: a person may right-click a part they have not selected yet, and then "which
    /// body" can come from nowhere except a search by the name of the face.
    pub(crate) fn body_of_face(&self, fid: u32) -> Option<qymcad_core::model::Id> {
        self.project.regen_faces.iter().find(|(_, fs)| fs.iter().any(|f| f.id == fid)).map(|(b, _)| *b)
    }

    /// THE QUERY OF AN ITEM for a particular pick — with what the table cannot know.
    ///
    /// "Every face of this feature" is derived from the name of the face, and the names live in the
    /// document. So the item keyed `expand-feature-faces` is assembled here rather than in the
    /// table.
    pub(crate) fn expansion_query(&self, e: &Expansion, picked: Picked, normal: [f64; 3]) -> Option<Query> {
        if e.key == "expand-feature-faces" {
            let desc = match picked {
                Picked::Face(f) => f,
                Picked::Edge(_) => return None,
            };
            let name = self.project.names.get(desc)?;
            return Some(Query::OfFeature { feature: name.feature, role: None });
        }
        Some((e.build)(picked, normal))
    }

    /// WHAT IS SELECTED RIGHT NOW — a face or an edge, and what its normal is (for "all parallel").
    ///
    /// The LAST selected element is taken: the menu is opened right after a click, and "the ones like
    /// this" refers to that one rather than to the first of the set.
    pub(crate) fn expansion_target(&self) -> Option<(Picked, [f64; 3])> {
        // THE EDGE UNDER THE CURSOR OUTWEIGHS THE FACE: if a person hovered an edge, they are asking
        // about the edge. One of the two rather than both at once — otherwise the menu would guess at
        // what is meant while a finger is pointing at it.
        if let Some((e, _b)) = self.gsel.last_edge {
            return Some((Picked::Edge(e), [0.0, 0.0, 1.0]));
        }
        // TWO SOURCES, AND THE SECOND IS ESSENTIAL. In the shell and the draft the face lands in
        // `faces`. But in the fillet a click on a face puts its EDGES into the selection, and the face
        // itself is left nowhere — that is exactly where the first edition of the menu stayed silent:
        // it asked `faces`, saw emptiness and did not open.
        let (f, body) = match (self.gsel.faces.iter().next().copied(), self.gsel.last_face) {
            (Some(f), _) => (f, self.gsel.faces_body),
            (None, Some((f, b))) => (f, Some(b)), // without this branch the menu stays silent in the fillet
            _ => return None,
        };
        let n = body
            .and_then(|b| self.project.regen_faces.get(&b))
            .and_then(|fs| fs.iter().find(|x| x.id == f))
            .map(|x| x.normal)
            .unwrap_or([0.0, 0.0, 1.0]);
        Some((Picked::Face(f), n))
    }

    /// WHAT THE ACTIVE COMMAND WILL ACCEPT: (are edges wanted?, will a set be accepted?). `None` means
    /// there is NOTHING to open the menu on.
    ///
    /// A description is a way of TELLING A COMMAND what to take. Outside a command there is nowhere to
    /// record it, and the menu becomes a button leading nowhere — it was reported that it could be
    /// summoned outside any feature simply by right-clicking any face. Outside a Part all the more so:
    /// in an Assembly faces and edges belong to nobody, components are moved there rather than geometry
    /// described — it was reported that it could be summoned in assemblies too.
    ///
    /// The count is part of the answer as well. Push-face and thicken take EXACTLY ONE face: offering
    /// them "all parallel" means offering what the command cannot accept.
    pub(crate) fn expansion_accepts(&self) -> Option<(bool, bool)> {
        if !matches!(self.workbench, super::Workbench::Part) || !self.cmd.active() {
            return None;
        }
        match self.cmd.kind {
            4 | 5 => Some((true, true)),           // fillet, chamfer — a set of EDGES
            6 | 23 | 26 => Some((false, true)),    // shell, draft, remove face — a set of FACES
            25 | 28 => Some((false, false)),       // push face, thicken — EXACTLY ONE face
            _ => None,                             // the rest take no descriptions at all
        }
    }

    /// THE MENU ITEMS FOR THE CURRENT SELECTION. Empty means there is no menu to show.
    ///
    /// ONLY WHAT THE COMMAND WILL ACCEPT IS SHOWN. The first edition dumped the whole list: under the
    /// fillet it offered items about faces, under push-face it offered "all parallel" while push-face
    /// takes one. A menu that offers the unperformable does not help one choose, it forces guessing.
    ///
    /// AN ITEM ALREADY CHOSEN STAYS IN THE LIST — with a mark. The former edition HID it (since it
    /// would change nothing), and the logic was rightly found confusing: the item is there, then it is
    /// not, and what is recorded right now is visible nowhere. A menu must show state, not only offer
    /// actions.
    pub(crate) fn expansion_menu_items(&self) -> Vec<(&'static str, Query)> {
        let Some((want_edges, want_many)) = self.expansion_accepts() else { return Vec::new() };
        let Some((picked, normal)) = self.expansion_target() else { return Vec::new() };
        let (on_face, on_edge) = match picked {
            Picked::Face(_) => (true, false),
            Picked::Edge(_) => (false, true),
        };
        EXPANSIONS
            .iter()
            .filter(|e| (e.on_face && on_face) || (e.on_edge && on_edge))
            .filter(|e| e.gives_edges == want_edges)
            .filter(|e| want_many || !e.gives_many)
            .filter_map(|e| self.expansion_query(e, picked, normal).map(|q| (e.key, q)))
            .collect()
    }

    /// APPLY AN ITEM: the selection becomes a description.
    ///
    /// The selection itself is NOT touched: it stays highlighted until the feature is applied — a
    /// person needs to see what they picked. What gets recorded is a descriptive reference (see
    /// `apply_feat_cmd`).
    pub(crate) fn apply_expansion(&mut self, key: &'static str, q: Query) {
        // A JUNCTION WAITS FOR ITS SECOND SIDE. The item does not create a reference but puts the
        // command into a "now click the second face" mode: a junction has two sets, and it cannot be
        // assembled from one face pointed at.
        if key == "expand-between" {
            if let Query::Id(f) = q {
                self.gsel.between_first = Some(f);
                self.status = crate::i18n::tr("expand-between-pick-second");
                return;
            }
        }
        // SHOW THE RESULT AT ONCE. The description is recorded into the document, but what a person
        // sees is the highlight — and if it does not change, they will decide the item did not work. So
        // the query is resolved against the live body right away and the result is highlighted.
        let body = self.edges.body.or(self.gsel.faces_body).or(self.gsel.last_face.map(|(_, b)| b));
        if let Some(b) = body {
            let r = qymcad_core::refs::Ref::many(q.clone());
            if q.yields_edges() {
                if let Ok(ids) = self.project.resolve_edge_refs(b, &r, "ref-what-fillet-edge") {
                    self.gsel.edges = ids.into_iter().collect();
                }
            } else if let Ok(ids) = self.project.resolve_face_refs(b, &r, "ref-what-walls") {
                self.gsel.faces = ids.into_iter().collect();
                self.gsel.faces_body = Some(b);
            }
        }
        let n = if q.yields_edges() { self.gsel.edges.len() } else { self.gsel.faces.len() };
        self.gsel.described = Some(q);
        self.status = crate::i18n::trn("expand-applied", &[("what", &crate::i18n::tr(key)), ("n", &n.to_string())]);
    }
}

#[cfg(test)]
mod tests;
