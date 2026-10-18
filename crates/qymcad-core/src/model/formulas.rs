//! THE NAMES OF A DOCUMENT AND THE FORMULAS THAT READ THEM, as one graph: which name reads which, the chain by which a
//! formula comes back to its own name, and the order the values are worked out in.
//!
//! The global parameters and the named numbers (a sketch dimension, a feature's number) share one scope: a formula
//! reads either kind by its name, so the graph holds both.

use super::*;

/// A FORMULA TYPED FOR A NAME, as it is asked before it is taken.
pub struct NamedFormula<'a> {
    pub name: &'a str,
    pub text: &'a str,
}

/// A NUMBER OF A FEATURE OR OF A MATE: number `key` of node `node`, as `feat_dims` keys it.
pub struct FeatNumber<'a> {
    pub node: Id,
    pub key: &'a str,
}

/// WHAT TEXT TYPED FOR A FEATURE'S NUMBER IS ([`Project::check_feat_text`]).
pub enum FeatText {
    /// An empty field: no formula.
    Nothing,
    /// A plain number, as typed.
    Number(String),
    /// A formula the document takes, as typed.
    Formula(String),
}

/// WHAT A PLAIN NUMBER TYPED FOR A FEATURE'S NUMBER DOES, by where it is typed.
#[derive(Clone, Copy)]
pub enum PlainNumber {
    /// It takes the formula off, and the node's own number applies: a command writes its numbers into the node itself.
    ClearsFormula,
    /// It stands as the number, kept as typed: the formula field of the properties and the fields of a mate hold the
    /// number there and nowhere else.
    Stands,
}

/// A NAME GIVEN ANEW, `old` to `new`: the graph as it would stand after the rename, `old` read as `new` in every formula.
pub struct Renamed<'a> {
    pub old: &'a str,
    pub new: &'a str,
}

/// THE GRAPH: every name with the formula it is written as (`None` for a plain number), and what each formula reads,
/// found once a name is gone down and remembered - a pass over every name finds what each formula reads once rather
/// than once a walk.
pub(crate) struct Names {
    formulas: std::collections::BTreeMap<String, Option<String>>,
    reads: std::cell::RefCell<std::collections::BTreeMap<String, Vec<String>>>,
}

impl Names {
    /// The names of the graph that `text` reads.
    fn read_by(&self, text: &str) -> Vec<String> {
        self.formulas.keys().filter(|n| crate::expr::mentions(text, n)).cloned().collect()
    }

    /// The names the formula of `name` reads; none for a plain number or a name the graph does not hold.
    fn reads_of(&self, name: &str) -> Vec<String> {
        if let Some(r) = self.reads.borrow().get(name) {
            return r.clone();
        }
        let r = self.formulas.get(name).and_then(|f| f.as_deref()).map(|f| self.read_by(f)).unwrap_or_default();
        self.reads.borrow_mut().insert(name.to_string(), r.clone());
        r
    }

    /// THE FIRST NAME `text` READS THAT HAS NO VALUE: one in a circle, or one reading such a name further down. Worked out
    /// from the graph at the asking, over what `text` reaches only.
    fn first_without_value(&self, text: &str) -> Option<String> {
        let reads = self.read_by(text);
        let circular = Circles::from(self, &reads);
        if circular.is_empty() {
            return None;
        }
        reads.into_iter().find(|r| self.reaches_any(r, &circular))
    }

    /// Whether `from`, or a name read further down from it, is one of `set`.
    fn reaches_any(&self, from: &str, set: &std::collections::BTreeSet<String>) -> bool {
        let mut seen = std::collections::BTreeSet::new();
        let mut todo = vec![from.to_string()];
        while let Some(n) = todo.pop() {
            if set.contains(&n) {
                return true;
            }
            if seen.insert(n.clone()) {
                todo.extend(self.reads_of(&n));
            }
        }
        false
    }

    /// THE CHAIN BY WHICH `f.text` COMES BACK TO `f.name`, the name first and last (`a -> b -> a`, `s -> s`); `None` when
    /// it does not. Only what the text reaches is walked, so a field asking it on every frame costs the names it reads.
    pub(crate) fn chain_back(&self, f: &NamedFormula) -> Option<String> {
        if f.name.is_empty() {
            return None;
        }
        let mut walk = Walk { to: f.name, path: Vec::new(), seen: std::collections::BTreeSet::new() };
        if !walk.through(self, f.text, self.read_by(f.text)) {
            return None;
        }
        Some(std::iter::once(f.name).chain(walk.path.iter().map(String::as_str)).collect::<Vec<_>>().join(" -> "))
    }
}

/// A WALK ALONG WHAT FORMULAS READ, looking for the way back to one name.
struct Walk<'a> {
    to: &'a str,
    path: Vec<String>,
    seen: std::collections::BTreeSet<String>,
}

impl Walk<'_> {
    /// Whether `text` reads the name looked for, itself or through a name read further down; the way there is left in
    /// `path`. The name looked for is read even when nobody holds it yet - a name about to be given. Each name is gone
    /// down once, so a walk ends however the formulas read each other.
    fn through(&mut self, names: &Names, text: &str, mut reads: Vec<String>) -> bool {
        if !reads.iter().any(|n| n == self.to) && crate::expr::mentions(text, self.to) {
            reads.push(self.to.to_string());
        }
        for n in reads {
            self.path.push(n.clone());
            if n == self.to {
                return true;
            }
            let below = names.formulas.get(&n).cloned().flatten();
            if self.seen.insert(n.clone()) && below.is_some_and(|f| self.through(names, &f, names.reads_of(&n))) {
                return true;
            }
            self.path.pop();
        }
        false
    }
}

/// THE NAMES THAT READ THEMSELVES, directly or round a circle: the strongly connected parts of the graph of more than
/// one name, and a name whose formula reads itself. Found in one pass over the graph (Tarjan), so working out the names
/// asks the chain of a name only when it is in a circle: measured on 150 parameters reading each other in a chain, an
/// evaluation took 14 ms in the dev build with a walk back from every name, 3.6 ms with this pass.
#[derive(Default)]
struct Circles {
    next: usize,
    index: std::collections::HashMap<String, usize>,
    low: std::collections::HashMap<String, usize>,
    stack: Vec<String>,
    on_stack: std::collections::HashSet<String>,
    circular: std::collections::BTreeSet<String>,
}

impl Circles {
    /// The names in a circle among those `starts` reach, the starts included.
    fn from<'a>(names: &Names, starts: impl IntoIterator<Item = &'a String>) -> std::collections::BTreeSet<String> {
        let mut c = Circles::default();
        for n in starts {
            if !c.index.contains_key(n) {
                c.visit(names, n);
            }
        }
        c.circular
    }

    fn visit(&mut self, names: &Names, v: &str) {
        self.index.insert(v.to_string(), self.next);
        self.low.insert(v.to_string(), self.next);
        self.next += 1;
        self.stack.push(v.to_string());
        self.on_stack.insert(v.to_string());
        let reads = names.reads_of(v);
        for w in &reads {
            let below = if !self.index.contains_key(w) {
                self.visit(names, w);
                self.low[w]
            } else if self.on_stack.contains(w) {
                self.index[w]
            } else {
                continue;
            };
            let low = self.low[v].min(below);
            self.low.insert(v.to_string(), low);
        }
        if self.low[v] != self.index[v] {
            return;
        }
        let mut part = Vec::new();
        while let Some(w) = self.stack.pop() {
            self.on_stack.remove(&w);
            let root = w == v;
            part.push(w);
            if root {
                break;
            }
        }
        if part.len() > 1 || reads.iter().any(|w| w == v) {
            self.circular.extend(part);
        }
    }
}

/// WHAT WORKING OUT THE NAMES GAVE: the value of every name that has one, why the others have none, and which of them
/// have no value to hand to a formula at all.
#[derive(Default)]
pub(crate) struct WorkedOut {
    pub(crate) values: std::collections::HashMap<String, f64>,
    pub(crate) faults: std::collections::HashMap<String, crate::errors::ExprError>,
    pub(crate) without_value: std::collections::BTreeSet<String>,
}

impl Project {
    /// THE GRAPH OF THE NAMES, as they stand or as they would after `renamed`. A named number written over a parameter's
    /// name stands for it, as in `param_map`.
    pub(crate) fn names_graph(&self, renamed: Option<&Renamed>) -> Names {
        let read_new = |s: String| match renamed {
            Some(r) => crate::expr::rename_ident(&s, r.old, r.new),
            None => s,
        };
        let own = |n: &str| match renamed {
            Some(r) if n == r.old => r.new.to_string(),
            _ => n.to_string(),
        };
        let mut formulas = std::collections::BTreeMap::new();
        for p in self.parameters.iter().filter(|p| !p.name.is_empty()) {
            formulas.insert(own(&p.name), (!p.expr.trim().is_empty()).then(|| read_new(p.expr.clone())));
        }
        for nd in self.named_dims.iter().filter(|n| !n.name.is_empty()) {
            formulas.insert(own(&nd.name), self.dim_target_expr(&nd.target).map(read_new));
        }
        Names { formulas, reads: Default::default() }
    }

    /// WHAT A FORMULA TYPED FOR A NAME GIVES, OR WHY IT IS NOT TAKEN - the one door every field that writes a formula for a
    /// name asks: the table of parameters and its rows of named numbers, the popup of a sketch dimension, the fields of a
    /// feature's number.
    ///
    /// A formula that comes back to its own name is refused with the chain (`a -> b -> a`). Reported behaviour: `a = b+1`
    /// was taken with `b = a+1`, and the values grew on every evaluation - 28 / 29, 60 / 61, 76 / 77. A formula reading a
    /// name that has no value says which name; anything else is the formula worked out over the document's names.
    pub fn check_formula(&self, f: &NamedFormula) -> Result<f64, crate::errors::ExprError> {
        let names = self.names_graph(None);
        if let Some(chain) = names.chain_back(f) {
            return Err(crate::errors::ExprError::LeansOnItself(chain));
        }
        if let Some(n) = names.first_without_value(f.text) {
            return Err(crate::errors::ExprError::NameWithoutValue(n));
        }
        self.eval_expr(f.text)
    }

    /// WHAT TEXT TYPED FOR A FEATURE'S NUMBER IS - the rule every field of such a number keeps, in one place: nothing, a
    /// plain number, or a formula, the formula taken only once [`Project::check_formula`] takes it under the name given to
    /// that number. The fields ask it before Enter, so a refusal is said beside them and closes their Apply;
    /// [`Project::write_feat_formula`] asks it again as it writes.
    pub fn check_feat_text(&self, at: &FeatNumber, text: &str) -> Result<FeatText, crate::errors::ExprError> {
        let t = text.trim();
        if t.is_empty() {
            return Ok(FeatText::Nothing);
        }
        if t.parse::<f64>().is_ok() {
            return Ok(FeatText::Number(t.to_string()));
        }
        let name = self.name_of_target(&DimTarget::Feature { node: at.node, key: at.key.to_string() });
        self.check_formula(&NamedFormula { name: &name, text: t })?;
        Ok(FeatText::Formula(t.to_string()))
    }

    /// WRITE TEXT TYPED FOR A FEATURE'S NUMBER - the one door a formula of a feature or of a mate goes into the document
    /// by: the numbers of a command, the counts and steps of an array, the values of a mate, the formula field of the
    /// properties. A refusal ([`Project::check_feat_text`]) leaves the document as it was. Reported behaviour: a count of
    /// an array typed as `n+1` for the count named `n` was written, and `n` was left with no value.
    pub fn write_feat_formula(&mut self, at: &FeatNumber, text: &str, plain: PlainNumber) -> Result<(), crate::errors::ExprError> {
        let written = match self.check_feat_text(at, text)? {
            FeatText::Formula(f) => f,
            FeatText::Number(n) if matches!(plain, PlainNumber::Stands) => n,
            FeatText::Number(_) | FeatText::Nothing => String::new(),
        };
        self.set_feat_dim(at.node, at.key, written);
        Ok(())
    }

    /// Take the formula off a feature's number: its node's own number applies again.
    pub fn clear_feat_formula(&mut self, at: &FeatNumber) {
        self.set_feat_dim(at.node, at.key, String::new());
    }

    /// The same for the formula `text` typed for global parameter `i`.
    pub fn check_param_text(&self, i: usize, text: &str) -> Result<f64, crate::errors::ExprError> {
        let name = self.parameters.get(i).map(|p| p.name.as_str()).unwrap_or_default();
        self.check_formula(&NamedFormula { name, text })
    }

    /// THE CHAIN A RENAME WOULD CLOSE: a formula reading `new` while nobody holds it, and the renamed name's own formula
    /// reading that one back. `a = b+1` with `b` held by nobody, and `c = a*2` renamed to `b`, make `b -> a -> b`.
    pub fn rename_closes_circle(&self, r: &Renamed) -> Option<String> {
        let names = self.names_graph(Some(r));
        let formula = names.formulas.get(r.new).cloned().flatten()?;
        names.chain_back(&NamedFormula { name: r.new, text: &formula })
    }

    /// THE CHAIN GIVING `target` THE NAME `new` WOULD CLOSE: renamed from the name it holds, or named for the first time
    /// while a formula reads `new` and the target's own formula reads that one back - a dimension written `a+1` named `x`
    /// while `a = x*2`. `None` when no circle closes.
    pub fn naming_closes_circle(&self, target: &DimTarget, new: &str) -> Option<String> {
        let old = self.name_of_target(target);
        if old == new {
            return None;
        }
        if !old.is_empty() {
            return self.rename_closes_circle(&Renamed { old: &old, new });
        }
        let formula = self.dim_target_expr(target)?;
        self.names_graph(None).chain_back(&NamedFormula { name: new, text: &formula })
    }

    /// WORK OUT EVERY NAME of the document in the order the formulas read each other, from the formulas alone: a name is
    /// worked out after the names its formula reads, so no value from before goes into the next one. A name whose formula
    /// comes back to itself has no value, and neither has a name that reads it.
    ///
    /// Reported behaviour: the values started from the ones before and went round up to eight times - a circle grew on
    /// every evaluation, and a chain of twelve standing in reverse order stopped at its ninth name.
    pub(crate) fn work_out_names(&self) -> WorkedOut {
        let names = self.names_graph(None);
        let mut w = WorkedOut::default();
        // a name in a circle has no value from the start: its chain is said, and the names reading it learn it has none
        for n in Circles::from(&names, names.formulas.keys()) {
            let chain = names.formulas.get(&n).cloned().flatten().and_then(|f| names.chain_back(&NamedFormula { name: &n, text: &f }));
            w.faults.insert(n.clone(), crate::errors::ExprError::LeansOnItself(chain.unwrap_or_else(|| format!("{n} -> {n}"))));
            w.without_value.insert(n);
        }
        let mut done: std::collections::BTreeSet<String> = w.without_value.clone();
        for n in names.formulas.keys() {
            self.work_out(n, &names, &mut w, &mut done);
        }
        w
    }

    /// Work out `name` after what its formula reads. A formula with no value keeps the stored number in the scope, as it
    /// always has, and says why it has none.
    fn work_out(&self, name: &str, names: &Names, w: &mut WorkedOut, done: &mut std::collections::BTreeSet<String>) {
        if !done.insert(name.to_string()) {
            return;
        }
        let Some(formula) = names.formulas.get(name).cloned() else { return };
        let Some(f) = formula else {
            if let Some(v) = self.stored_value_of_name(name) {
                w.values.insert(name.to_string(), v);
            }
            return;
        };
        let reads = names.reads_of(name);
        for r in &reads {
            self.work_out(r, names, w, done);
        }
        if let Some(r) = reads.iter().find(|r| w.without_value.contains(*r)) {
            w.faults.insert(name.to_string(), crate::errors::ExprError::NameWithoutValue(r.clone()));
            w.without_value.insert(name.to_string());
            return;
        }
        match crate::expr::eval(&f, &w.values) {
            Ok(v) => {
                w.values.insert(name.to_string(), v);
            }
            Err(e) => {
                w.faults.insert(name.to_string(), e);
                if let Some(v) = self.stored_value_of_name(name) {
                    w.values.insert(name.to_string(), v);
                }
            }
        }
    }

    /// The number a name holds without its formula: a parameter's last value, a dimension's own number, a feature's
    /// number in its node. A named number stands for a parameter of its name, as in `param_map`.
    fn stored_value_of_name(&self, name: &str) -> Option<f64> {
        if let Some(nd) = self.named_dims.iter().rev().find(|n| n.name == name) {
            return match &nd.target {
                DimTarget::Feature { node, key } => self.timeline.iter().find(|n| n.id == *node)?.kind.dim(key),
                sketch => self.dim_target_value(sketch),
            };
        }
        self.parameters.iter().find(|p| p.name == name).map(|p| p.value)
    }
}
