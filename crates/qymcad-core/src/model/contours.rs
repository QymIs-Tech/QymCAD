//! The contours of a document: four structures that have to agree, and are therefore one.
//!
//! `contours`, `contour_ids`, `contour_parent` and `contour_ents` used to sit side by side in the document, with
//! their agreement resting on discipline. Discipline did not hold: removing a contour was written in three
//! places and each cleaned its own subset — `remove_contour` and the removal of a sketch left orphaned records
//! in `ents` and `parent`, and the two places that do clean them do not agree with each other. The orphans
//! accumulated in the document and travelled into the file.
//!
//! Removing a contour without touching everything else is now impossible: the lists are private and there is one
//! way in. Reading is unaffected — a `Deref` to a slice keeps `p.contours[i]`, `.len()` and `.iter()` as they
//! were.

use serde::{Deserialize, Serialize};

use crate::geom::Contour;
use crate::model::Id;

type Map<K, V> = std::collections::HashMap<K, V>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Contours {
    list: Vec<Contour>,
    /// the stable id of each contour, parallel to `list`, always of the same length
    ids: Vec<Id>,
    /// contour id to the entities of its boundary: the provenance, used for matching on the next edit
    ents: Map<Id, Vec<Id>>,
    /// contour id to the id of the enclosing contour: the nesting
    parent: Map<Id, Id>,
}

impl std::ops::Deref for Contours {
    type Target = [Contour];
    fn deref(&self) -> &[Contour] {
        &self.list
    }
}

impl Contours {
    /// Assemble from the flat lists when reading a file. The lengths are levelled: a contour without an id
    /// means a damaged file, and it is better lost here than spread further through the document.
    pub(crate) fn from_parts(list: Vec<Contour>, ids: Vec<Id>, ents: Map<Id, Vec<Id>>, parent: Map<Id, Id>) -> Self {
        let n = list.len().min(ids.len());
        Self { list: list.into_iter().take(n).collect(), ids: ids.into_iter().take(n).collect(), ents, parent }
    }

    pub(crate) fn parts(&self) -> (&[Contour], &[Id], &Map<Id, Vec<Id>>, &Map<Id, Id>) {
        (&self.list, &self.ids, &self.ents, &self.parent)
    }

    pub fn ids(&self) -> &[Id] {
        &self.ids
    }

    /// The index for a stable id.
    pub fn index_of(&self, id: Id) -> Option<usize> {
        self.ids.iter().position(|x| *x == id)
    }

    pub fn id_at(&self, index: usize) -> Option<Id> {
        self.ids.get(index).copied()
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Contour> {
        self.list.get_mut(index)
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Contour> {
        self.list.iter_mut()
    }

    /// Add a contour with an id already allocated.
    pub fn push(&mut self, id: Id, c: Contour) {
        self.list.push(c);
        self.ids.push(id);
    }

    /// Remove a contour by index, together with its provenance and its nesting.
    ///
    /// Returns the id of the removed contour so the caller can detach its own references.
    pub fn remove_at(&mut self, index: usize) -> Option<Id> {
        if index >= self.list.len() {
            return None;
        }
        let id = self.ids[index];
        self.list.remove(index);
        self.ids.remove(index);
        self.ents.remove(&id);
        self.parent.remove(&id);
        // the children of the removed contour are no longer nested in anything
        self.parent.retain(|_, p| *p != id);
        Some(id)
    }

    pub fn clear(&mut self) {
        self.list.clear();
        self.ids.clear();
        self.ents.clear();
        self.parent.clear();
    }

    // --- provenance: which sketch entities form the boundary ---
    pub fn ents_of(&self, id: Id) -> Option<&Vec<Id>> {
        self.ents.get(&id)
    }
    pub fn set_ents(&mut self, id: Id, e: Vec<Id>) {
        self.ents.insert(id, e);
    }
    pub fn clear_ents(&mut self, id: Id) {
        self.ents.remove(&id);
    }

    // --- nesting ---
    pub fn parent_of(&self, id: Id) -> Option<Id> {
        self.parent.get(&id).copied()
    }
    pub fn set_parent(&mut self, id: Id, parent: Id) {
        self.parent.insert(id, parent);
    }
    pub fn clear_parent(&mut self, id: Id) {
        self.parent.remove(&id);
    }
    pub fn children_of(&self, id: Id) -> Vec<Id> {
        self.parent.iter().filter(|(_, p)| **p == id).map(|(c, _)| *c).collect()
    }
}
