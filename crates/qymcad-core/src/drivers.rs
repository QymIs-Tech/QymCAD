//! Drivers of a project: what may be substituted into any dimension field.
//!
//! A global parameter such as `w = 50` and a named driving dimension of a sketch are the same thing from the
//! outside: a name that can be written into a formula. The difference is that a parameter lives in the project
//! while a driver lives in a particular sketch of a particular part.
//!
//! **And here is the trouble, found by measurement.** `param_map` stores drivers under a bare name: two
//! dimensions called `len` in different parts are both accepted, but only the last one remains in scope.
//!
//! ```text
//! named: A=true, B=true; drivers in the project: 2
//! in scope, 'len' = 70.0
//!    driver 'len' in sketch A, value 20.0   <- unreachable
//!    driver 'len' in sketch B, value 70.0
//! ```
//!
//! This looks like a convenience problem — names in different assemblies, parts and sketches may coincide, so
//! a search is needed along with an indication of which part a driver belongs to — but it is a correctness
//! problem too: without a path a name is not unambiguous, and there is no way to tell which of two dimensions
//! is being moved.
//!
//! What is assembled here is a list of drivers carrying breadcrumbs (`Assembly.Part.Sketch`), a value and an
//! honest ambiguity flag, so that the completion list shows what exactly is being substituted and identically
//! named drivers do not look alike.
use crate::model::{Id, Project};

/// Where a driver comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DriverKind {
    /// A global parameter of the project, such as `w = 50`. It lives in the document and has no path.
    Parameter,
    /// A named dimension of a sketch. The path leads to the sketch through the components.
    SketchDim,
    /// A named parameter of a feature, such as an extrusion height or a fillet radius. The path leads to the
    /// feature.
    FeatDim,
}

/// A driver fit for substitution into a formula.
#[derive(Clone, Debug)]
pub struct DriverRef {
    /// The name as it is written in an expression.
    pub name: String,
    /// Breadcrumbs to the owner: `Assembly.Part.Sketch`. Empty for a global parameter.
    pub path: String,
    /// The current value. `None` means the dimension was not found, its sketch or constraint having gone.
    pub value: Option<f64>,
    pub kind: DriverKind,
    /// Another driver in the project carries the same name. A bare name in a formula is then ambiguous, and
    /// the list has to show the path rather than pretend the choice is obvious.
    pub ambiguous: bool,
}

impl DriverRef {
    /// How to show it in a list: `len — Assembly.Part.Sketch`, or simply `w` for a global parameter.
    pub fn label(&self) -> String {
        if self.path.is_empty() {
            self.name.clone()
        } else {
            format!("{} — {}", self.name, self.path)
        }
    }
}

/// Why a name is unfit for a formula. The core returns a code and the interface supplies the words: no language
/// lives here, and the name itself is user input, which must not be translated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IdentError {
    /// Empty. Not an error in itself — the dimension simply stops being a driver — but it will not become a
    /// name either.
    Empty,
    /// The first character is unfit: a name cannot begin with a digit or a sign, since `2w` parses as two
    /// times w and the formula would read it differently from the person writing it.
    BadStart(char),
    /// A foreign character inside the name: a space, a dot, an operator. Such a name falls apart inside an
    /// expression.
    BadChar(char),
}

/// Whether a name is fit for a formula: a letter or `_` first, then letters, digits and `_`. Non-Latin letters
/// are allowed, since names are written in the language of the author and the expression parser handles
/// them.
pub fn check_ident(name: &str) -> Result<(), IdentError> {
    let nm = name.trim();
    let mut cs = nm.chars();
    let Some(first) = cs.next() else { return Err(IdentError::Empty) };
    if !(first.is_alphabetic() || first == '_') {
        return Err(IdentError::BadStart(first));
    }
    match cs.find(|c| !(c.is_alphanumeric() || *c == '_')) {
        Some(c) => Err(IdentError::BadChar(c)),
        None => Ok(()),
    }
}

/// Why a rename did not go through.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenameError {
    /// The old name does not exist in the project.
    NotFound,
    /// The new name already belongs to someone.
    Taken,
    /// The new name is unfit for a formula.
    Bad(IdentError),
}

impl Project {
    /// Who already carries this name. The interface needs it in order to say not merely "taken" but "taken by
    /// a dimension in Housing.Profile"; otherwise the namesake has to be hunted down by hand across the whole
    /// project.
    pub fn name_owner(&self, name: &str) -> Option<DriverRef> {
        let nm = name.trim();
        if nm.is_empty() {
            return None;
        }
        self.drivers().into_iter().find(|d| d.name.eq_ignore_ascii_case(nm))
    }

    /// Rename a driver and update the references that follow it.
    ///
    /// The name of a parameter used to be edited straight in the model on every keystroke, with no updating of
    /// references at all: renaming `w` to `shirina` left expressions such as `w*2+5` all over the project,
    /// pointing at a name that no longer existed. It broke on the very first letter, before the new name was
    /// even finished.
    ///
    /// It is now one operation: the name changes and with it every expression where that name appears as a
    /// name. The values do not change, so nothing has to be rebuilt.
    ///
    /// Returns how many expressions were corrected, for the check and for the status line.
    pub fn rename_driver(&mut self, old: &str, new: &str) -> Result<usize, RenameError> {
        let (old, new) = (old.trim().to_string(), new.trim().to_string());
        if old == new {
            return Ok(0);
        }
        check_ident(&new).map_err(RenameError::Bad)?;
        // Taken by somebody else. Changing the case of one's own name, `len` to `Len`, is not a conflict.
        if self.name_owner(&new).is_some() && !new.eq_ignore_ascii_case(&old) {
            return Err(RenameError::Taken);
        }

        let mut found = false;
        for p in self.parameters.iter_mut().filter(|p| p.name.eq_ignore_ascii_case(&old)) {
            p.name = new.clone();
            found = true;
        }
        for nd in self.named_dims.iter_mut().filter(|n| n.name.eq_ignore_ascii_case(&old)) {
            nd.name = new.clone();
            found = true;
        }
        if !found {
            return Err(RenameError::NotFound);
        }

        let mut fixed = 0usize;
        for p in self.parameters.iter_mut() {
            let s = crate::expr::rename_ident(&p.expr, &old, &new);
            if s != p.expr {
                p.expr = s;
                fixed += 1;
            }
        }
        for dims in self.feat_dims.values_mut() {
            for e in dims.values_mut() {
                let s = crate::expr::rename_ident(e, &old, &new);
                if s != *e {
                    *e = s;
                    fixed += 1;
                }
            }
        }
        for sk in self.sketches.iter_mut() {
            for c in sk.constraints.iter_mut() {
                if let Some(e) = c.expr_mut() {
                    let s = crate::expr::rename_ident(e, &old, &new);
                    if s != *e {
                        *e = s;
                        fixed += 1;
                    }
                }
            }
        }
        Ok(fixed)
    }

    /// Breadcrumbs to a component: `Assembly.Subassembly.Part`. The root of the document is left out of the
    /// path, being the same for everyone and distinguishing nothing.
    pub fn component_breadcrumbs(&self, comp: Id) -> String {
        let mut names: Vec<String> = Vec::new();
        let mut cur = Some(comp);
        // Guard against a cycle. The component tree is built by code, but a damaged document must not hang
        // the program: a path can be no longer than the number of components.
        let mut guard = self.components.len() + 1;
        while let Some(id) = cur {
            let Some(c) = self.components.iter().find(|c| c.id == id) else { break };
            if c.parent.is_none() {
                break; // the root of the document
            }
            names.push(c.name.clone());
            cur = c.parent;
            guard -= 1;
            if guard == 0 {
                break;
            }
        }
        names.reverse();
        names.join(".")
    }

    /// Breadcrumbs to whatever carries the name: `Subassembly.Part.Sketch` for a dimension and
    /// `Subassembly.Part.Feature` for a feature parameter. The two have to look the same.
    pub fn driver_path(&self, target: &crate::model::DimTarget) -> String {
        match target {
            crate::model::DimTarget::Sketch { sketch, .. } => self.sketch_breadcrumbs(*sketch),
            crate::model::DimTarget::Feature { node, .. } => self.feature_breadcrumbs(*node),
        }
    }

    /// Breadcrumbs to a feature: the path of the part plus the name of the timeline node.
    pub fn feature_breadcrumbs(&self, node: Id) -> String {
        let Some(n) = self.timeline.iter().find(|n| n.id == node) else { return String::new() };
        let own = n.parent.map(|c| self.component_breadcrumbs(c)).unwrap_or_default();
        match (own.is_empty(), n.name.is_empty()) {
            (true, true) => String::new(),
            (true, false) => n.name.clone(),
            (false, true) => own,
            (false, false) => format!("{own}.{}", n.name),
        }
    }

    /// Breadcrumbs to a sketch: the path of the component plus the name of the sketch itself.
    ///
    /// A sketch may have no timeline node, which happens in tests and for one just created; the owner cannot
    /// then be found and the path consists of the sketch name alone. Returning an empty path silently is not
    /// acceptable: an empty path means global, and that would be untrue.
    pub fn sketch_breadcrumbs(&self, sketch: Id) -> String {
        let own = self.sketch_owner(sketch).map(|c| self.component_breadcrumbs(c)).unwrap_or_default();
        let name = self.sketches.iter().find(|s| s.id == sketch).map(|s| s.name.clone()).unwrap_or_default();
        match (own.is_empty(), name.is_empty()) {
            (true, true) => String::new(),
            (true, false) => name,
            (false, true) => own,
            (false, false) => format!("{own}.{name}"),
        }
    }

    /// Every driver of the project: global parameters and named sketch dimensions.
    ///
    /// The order is stable — parameters first, in document order, then dimensions. The list feeds completion
    /// and must not jump between frames.
    pub fn drivers(&self) -> Vec<DriverRef> {
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for p in self.parameters.iter().filter(|p| !p.name.is_empty()) {
            *seen.entry(p.name.to_lowercase()).or_insert(0) += 1;
        }
        for nd in self.named_dims.iter().filter(|n| !n.name.is_empty()) {
            *seen.entry(nd.name.to_lowercase()).or_insert(0) += 1;
        }
        let dup = |name: &str| seen.get(&name.to_lowercase()).copied().unwrap_or(0) > 1;

        let mut out: Vec<DriverRef> = self
            .parameters
            .iter()
            .filter(|p| !p.name.is_empty())
            .map(|p| DriverRef {
                name: p.name.clone(),
                path: String::new(),
                value: Some(p.value),
                kind: DriverKind::Parameter,
                ambiguous: dup(&p.name),
            })
            .collect();
        out.extend(self.named_dims.iter().filter(|n| !n.name.is_empty()).map(|nd| DriverRef {
            name: nd.name.clone(),
            path: self.driver_path(&nd.target),
            value: self.named_dim_value(nd),
            kind: match nd.target {
                crate::model::DimTarget::Sketch { .. } => DriverKind::SketchDim,
                crate::model::DimTarget::Feature { .. } => DriverKind::FeatDim,
            },
            ambiguous: dup(&nd.name),
        }));
        out
    }

    /// The drivers matching what has been typed. The search covers both the name and the path: what is
    /// remembered is either what the dimension was called or which part it sits in, and both routes have to
    /// work.
    ///
    /// An empty query returns everything, so the completion list opens before the first letter is typed.
    pub fn drivers_matching(&self, query: &str) -> Vec<DriverRef> {
        let q = query.trim().to_lowercase();
        let all = self.drivers();
        if q.is_empty() {
            return all;
        }
        // A match at the start of the name ranks higher. Otherwise typing `len` puts something like
        // `Part.Sketch.dlina` first, where the letters happened to match somewhere in the middle of the
        // path.
        let mut hit: Vec<(u8, DriverRef)> = Vec::new();
        for d in all {
            let n = d.name.to_lowercase();
            let p = d.path.to_lowercase();
            let rank = if n.starts_with(&q) {
                0
            } else if n.contains(&q) {
                1
            } else if p.contains(&q) {
                2
            } else {
                continue;
            };
            hit.push((rank, d));
        }
        hit.sort_by_key(|(r, _)| *r);
        hit.into_iter().map(|(_, d)| d).collect()
    }
}
