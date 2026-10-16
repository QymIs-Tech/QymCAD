//! THE PARAMETERS AND NAMED DIMENSIONS of a document: named, evaluated, the dependents marked; the solve of a sketch asked.

use super::*;

impl Project {
    /// Solve the sketch constraints (moving the points) and re-tessellate the contour.
    /// Map of global parameter names to values, used to evaluate feature and command dimension expressions
    /// through [`crate::expr::eval`], the same way sketch dimensions are evaluated. A name is a key as written: `H`
    /// and `h` are two parameters.
    pub fn param_map(&self) -> std::collections::HashMap<String, f64> {
        let mut m: std::collections::HashMap<String, f64> = self.parameters.iter().filter(|p| !p.name.is_empty()).map(|p| (p.name.clone(), p.value)).collect();
        // Named driving dimensions (the skeleton sketch of an assembly): a name resolves to a dimension
        // value, which parts then consume through expressions.
        for nd in &self.named_dims {
            if !nd.name.is_empty() {
                if let Some(v) = self.named_dim_value(nd) {
                    m.insert(nd.name.clone(), v);
                }
            }
        }
        m
    }

    /// Name a driver: either a sketch dimension or a feature parameter.
    ///
    /// One entry point for both cases, because the rules — the name is taken, an empty name clears the
    /// previous one, renaming in place — have to be identical or they drift apart.
    ///
    /// A taken name is refused rather than accepted as a second driver under the same name. A formula
    /// knows one name and the scope stores one value, so two dimensions called `len` are always a broken
    /// model in which one of them is unreachable and nothing says so. Measured: naming `len` in two
    /// sketches left only the last one (70.0) visible to formulas while the first (20.0) disappeared,
    /// showing up as two identical rows in the parameter list. Both halves of the fix apply — the name
    /// cannot be taken twice, and the path to the owning sketch is shown next to it.
    ///
    /// Global parameters are checked too: they share the same scope.
    pub fn name_dim(&mut self, name: String, target: DimTarget) -> bool {
        let nm = name.trim().to_string();
        if !nm.is_empty() && self.driver_name_taken_by(&nm, &target) {
            return false;
        }
        self.named_dims.retain(|n| n.target != target);
        if nm.is_empty() {
            return false; // An empty name clears the previous one: the dimension stops being a driver.
        }
        self.named_dims.push(NamedDim { name: nm, target });
        true
    }

    /// Name a sketch dimension. A wrapper over [`Project::name_dim`] that reads better at the call site.
    pub fn add_named_dim(&mut self, name: String, sketch: Id, refs: Vec<Id>) -> bool {
        self.name_dim(name, DimTarget::Sketch { sketch, refs: Self::dim_key(&refs) })
    }

    /// Name a feature parameter (`height`, `radius`, `thickness` and so on).
    pub fn add_named_feat_dim(&mut self, name: String, node: Id, key: &str) -> bool {
        self.name_dim(name, DimTarget::Feature { node, key: key.to_string() })
    }

    /// Whether a driver name is already taken by something else (a global parameter or another
    /// dimension). The interface uses it to explain the refusal before Enter is pressed and nothing
    /// happens.
    pub fn driver_name_taken_by(&self, name: &str, target: &DimTarget) -> bool {
        let nm = name.trim();
        if nm.is_empty() {
            return false;
        }
        self.parameters.iter().any(|p| p.name == nm) || self.named_dims.iter().any(|n| n.name == nm && n.target != *target)
    }

    /// The same for a sketch dimension, addressed by entities the way the popup addresses it.
    pub fn driver_name_taken(&self, name: &str, sketch: Id, refs: &[Id]) -> bool {
        self.driver_name_taken_by(name, &DimTarget::Sketch { sketch, refs: Self::dim_key(refs) })
    }

    /// The name given to this dimension or parameter. Empty means it is not a driver.
    pub fn name_of_target(&self, target: &DimTarget) -> String {
        self.named_dims.iter().find(|n| n.target == *target).map(|n| n.name.clone()).unwrap_or_default()
    }

    /// The same key, exposed: the interface needs it to build a dimension target and ask the model for its
    /// name without repeating the comparison rule.
    pub fn dim_key_pub(refs: &[Id]) -> Vec<Id> {
        Self::dim_key(refs)
    }

    /// Dimension key: the same entities in a stable order. A dimension is identified by the set, not by
    /// the order, or "distance A to B" and "distance B to A" would count as different dimensions.
    pub(crate) fn dim_key(refs: &[Id]) -> Vec<Id> {
        let mut v = refs.to_vec();
        v.sort_unstable();
        v
    }

    /// Entities a dimensional constraint is re-identified by. `None` means the constraint is not a
    /// dimension: it cannot become a driver and must not offer a name field.
    pub fn dim_refs(c: &Constraint) -> Option<Vec<Id>> {
        Some(match c {
            Constraint::Distance { a, b, .. } => vec![*a, *b],
            Constraint::Angle { a, b, c: c3, .. } => vec![*a, *b, *c3],
            Constraint::AngleLines { a, b, c: c3, d, .. } => vec![*a, *b, *c3, *d],
            Constraint::Diameter { c: c1, .. } => vec![*c1],
            Constraint::DistancePL { p, a, b, .. } => vec![*p, *a, *b],
            Constraint::EdgeDistance { c1, c2, .. } => vec![*c1, *c2],
            Constraint::ArcLength { c: c1, a, b, .. } => vec![*c1, *a, *b],
            _ => return None,
        })
    }

    /// Value of a dimensional constraint (length, angle, diameter). `None` when it is not a dimension.
    pub fn dim_value_of(c: &Constraint) -> Option<f64> {
        Some(match c {
            Constraint::Distance { d, .. } | Constraint::Diameter { d, .. } | Constraint::DistancePL { d, .. } | Constraint::EdgeDistance { d, .. } => *d,
            Constraint::Angle { deg, .. } | Constraint::AngleLines { deg, .. } => *deg,
            Constraint::ArcLength { len, .. } => *len,
            _ => return None,
        })
    }

    /// Set the value of whatever was named as a driver, from the parameter table.
    ///
    /// The parameter table is the one place where the whole set of numbers in a project is visible, so it
    /// has to be editable there and not read-only.
    ///
    /// Returns `true` when the value was written. The sketch is deliberately not re-solved here: the
    /// caller decides, and the caller also marks the document for rebuild, keeping one undo step per
    /// edit.
    pub fn set_dim_target_value(&mut self, target: &DimTarget, v: f64) -> bool {
        match target {
            DimTarget::Sketch { sketch, refs } => {
                let Some(si) = self.sketch_index(*sketch) else { return false };
                let want: std::collections::BTreeSet<Id> = refs.iter().copied().collect();
                let Some(ci) = self.sketches[si].constraints.iter().position(|c| Self::dim_refs(c).is_some_and(|r| r.into_iter().collect::<std::collections::BTreeSet<Id>>() == want)) else {
                    return false;
                };
                // An entered value overrides the expression: a typed number has to stay, instead of losing
                // to the previous formula on the next evaluation.
                match self.sketches[si].constraints.get_mut(ci) {
                    Some(Constraint::Distance { d, expr, .. }) | Some(Constraint::Diameter { d, expr, .. }) | Some(Constraint::EdgeDistance { d, expr, .. }) => {
                        *d = v;
                        expr.clear();
                    }
                    // In `DistancePL` the sign is the side, while the entered value is the magnitude.
                    Some(Constraint::DistancePL { d, expr, .. }) => {
                        *d = if *d < 0.0 { -v.abs() } else { v.abs() };
                        expr.clear();
                    }
                    Some(Constraint::Angle { deg, expr, .. }) | Some(Constraint::AngleLines { deg, expr, .. }) => {
                        *deg = v;
                        expr.clear();
                    }
                    Some(Constraint::ArcLength { len, expr, .. }) => {
                        *len = v;
                        expr.clear();
                    }
                    _ => return false,
                }
                true
            }
            DimTarget::Feature { node, key } => {
                if self.timeline.iter().find(|n| n.id == *node).and_then(|n| n.kind.dim(key)).is_none() {
                    return false; // This feature has no such parameter.
                }
                // For a feature the number lives in the expression: that is how the rebuild applies it
                // (`feat_dims` overrides the stored value) and how the edit stays visible in the feature
                // properties.
                self.set_feat_dim(*node, key, crate::expr::fmt_num(v));
                true
            }
        }
    }

    /// Remove a driver name.
    pub fn remove_named_dim(&mut self, name: &str) {
        self.named_dims.retain(|n| n.name != name);
    }

    /// Set the parametric expression for dimension `key` of feature `id` (`height`, `angle`, `radius` and
    /// so on). An empty expression clears it and returns to the stored number. Marks the feature dirty, so
    /// regenerate rebuilds it.
    pub fn set_feat_dim(&mut self, id: Id, key: &str, expr: String) {
        let e = expr.trim().to_string();
        if e.is_empty() {
            if let Some(m) = self.feat_dims.get_mut(&id) {
                m.remove(key);
                if m.is_empty() {
                    self.feat_dims.remove(&id);
                }
            }
        } else {
            self.feat_dims.entry(id).or_default().insert(key.to_string(), e);
        }
        self.mark_node_dirty(id);
    }

    /// Expression for dimension `key` of feature `id`, for display and editing in the properties panel.
    pub fn feat_dim(&self, id: Id, key: &str) -> Option<&str> {
        self.feat_dims.get(&id).and_then(|m| m.get(key)).map(|s| s.as_str())
    }

    /// Value of dimension `key` of feature `id` as the rebuild takes it: the expression evaluated against `vars`
    /// when there is one, otherwise `stored`, the number kept in the node. The number in the node is what was typed
    /// last and does not follow a parameter; whatever shows a size of a built feature reads it through here.
    pub fn feat_dim_value(&self, id: Id, key: &str, stored: f64, vars: &std::collections::HashMap<String, f64>) -> f64 {
        eval_dim(self.feat_dims.get(&id), key, stored, vars)
    }

    /// Mark as dirty the features whose expressions reference parameter `name`, and their consumers.
    ///
    /// Marking every feature that has any expression means rebuilding a project with a hundred dimensions
    /// after a single keystroke, so only the expressions that actually mention the name are considered.
    pub fn mark_param_dependents_dirty_for(&mut self, name: &str) {
        let ids: Vec<Id> = self.feat_dims.iter().filter(|(_, m)| m.values().any(|e| crate::expr::mentions(e, name))).map(|(id, _)| *id).collect();
        for id in ids {
            self.mark_node_dirty(id);
        }
        // Sketch dimensions using this parameter: the sketch is re-solved and its consumers follow it down
        // the timeline.
        let sids: Vec<Id> = self.sketches.iter().filter(|s| s.constraints.iter().any(|c| c.expr().is_some_and(|e| crate::expr::mentions(e, name)))).map(|s| s.id).collect();
        for sid in sids {
            self.mark_sketch_dirty(sid);
        }
    }

    /// DOES THE PLACEMENT COUNT FROM ONE OF `names`: the angle or an offset of a mate written as a formula. A mate is no
    /// node of the timeline - its formulas stand in `feat_dims` under its own id - so there is nothing to mark; the
    /// placement pass of every rebuild solves it (`solve_joints`), and a rebuild has to be asked for.
    pub fn placement_counts_from(&self, names: &[String]) -> bool {
        self.joints.iter().filter_map(|j| self.feat_dims.get(&j.id)).any(|m| m.values().any(|e| names.iter().any(|n| crate::expr::mentions(e, n))))
    }

    /// Mark every feature that has a parametric dimension expression as dirty. Called after `parameters`
    /// changed, so the dependent bodies rebuild on the next regenerate.
    pub fn mark_param_dependents_dirty(&mut self) {
        let ids: Vec<Id> = self.feat_dims.keys().copied().collect();
        for id in ids {
            self.mark_node_dirty(id);
        }
    }

    /// Re-evaluate the expressions. Returns pairs of "what was evaluated" and an error code rather than
    /// phrases: the core knows no language, and the first half is the user's own input, which must not be
    /// translated.
    pub fn eval_parameters(&mut self) -> Vec<(String, crate::errors::ExprError)> {
        use crate::expr::eval;
        let mut errs = Vec::new();
        // THE SCOPE HOLDS BOTH KINDS OF NAME. A global parameter `w = 50` and a named dimension are the same
        // thing seen from a formula: a name that resolves to a number. Seeding from `self.parameters` alone
        // left every driver out of scope, so a dimension written as `w*2` could not be evaluated at all and
        // silently kept its previous value - which is exactly the reported behaviour, that drivers do not
        // work inside other dimensions.
        //
        // `param_map` is the one place that answers "what names does this document have", and it is used here
        // so that the scope of a rebuild and the scope of the completion list cannot drift apart.
        let mut vars = self.param_map();
        for p in &self.parameters {
            if !p.name.is_empty() {
                vars.insert(p.name.clone(), p.value); // seed with the previous value
            }
        }
        // Fixed point over the dependencies (up to eight passes).
        for _ in 0..8 {
            let mut changed = false;
            for p in &self.parameters {
                if p.name.is_empty() || p.expr.trim().is_empty() {
                    continue;
                }
                if let Ok(v) = eval(&p.expr, &vars) {
                    let key = p.name.clone();
                    if vars.get(&key).is_none_or(|o| (o - v).abs() > 1e-12) {
                        changed = true;
                    }
                    vars.insert(key, v);
                }
            }
            if !changed {
                break;
            }
        }
        for p in &mut self.parameters {
            if p.name.is_empty() {
                errs.push((String::new(), crate::errors::ExprError::UnknownFn(String::new())));
                continue;
            }
            if p.expr.trim().is_empty() {
                continue;
            }
            match eval(&p.expr, &vars) {
                Ok(v) => p.value = v,
                Err(e) => errs.push((p.name.clone(), e)),
            }
        }
        // Apply the expressions to dimensional constraints.
        for s in &mut self.sketches {
            for c in &mut s.constraints {
                match c {
                    Constraint::Distance { d, expr, .. } if !expr.trim().is_empty() => match eval(expr, &vars) {
                        Ok(v) => *d = v,
                        Err(e) => errs.push((expr.clone(), e)),
                    },
                    Constraint::Angle { deg, expr, .. } if !expr.trim().is_empty() => match eval(expr, &vars) {
                        Ok(v) => *deg = v,
                        Err(e) => errs.push((expr.clone(), e)),
                    },
                    Constraint::DistancePL { d, expr, .. } if !expr.trim().is_empty() => match eval(expr, &vars) {
                        Ok(v) => *d = if *d < 0.0 { -v.abs() } else { v.abs() }, // keep the side
                        Err(e) => errs.push((expr.clone(), e)),
                    },
                    Constraint::Diameter { d, expr, .. } if !expr.trim().is_empty() => match eval(expr, &vars) {
                        Ok(v) => *d = v,
                        Err(e) => errs.push((expr.clone(), e)),
                    },
                    Constraint::AngleLines { deg, expr, .. } if !expr.trim().is_empty() => match eval(expr, &vars) {
                        Ok(v) => *deg = v,
                        Err(e) => errs.push((expr.clone(), e)),
                    },
                    Constraint::ArcLength { len, expr, .. } if !expr.trim().is_empty() => match eval(expr, &vars) {
                        Ok(v) => *len = v,
                        Err(e) => errs.push((expr.clone(), e)),
                    },
                    _ => {}
                }
            }
        }
        errs
    }

    /// Evaluate an arbitrary expression in the context of the project parameters, for validating a
    /// dimension field in the interface. A parameter name is matched as written.
    pub fn eval_expr(&self, src: &str) -> Result<f64, crate::errors::ExprError> {
        // Going through `param_map` exposes both the global parameters and the named driving dimensions of
        // a skeleton sketch.
        crate::expr::eval(src, &self.param_map())
    }

    pub fn solve_sketch(&mut self, si: usize) -> f64 {
        self.solve_sketch_drag(si, None)
    }

    /// Solve a sketch while pinning the dragged point `drag = Some((id, x, y))` to the cursor: the point
    /// follows the pointer and constrained geometry resists.
    pub fn solve_sketch_drag(&mut self, si: usize, drag: Option<(Id, f64, f64)>) -> f64 {
        self.eval_parameters(); // Parametric dimensions become values before the solve.
        self.solve_sketch_inner(si, drag, crate::solver::Budget::FULL, sketch::Rebuild::Whole)
    }

    /// Solve a sketch within `budget`: what is left when its time is out stands as it stood and is counted in
    /// `Sketch::left_unsolved`.
    pub fn solve_sketch_within(&mut self, si: usize, budget: crate::solver::Budget) -> f64 {
        self.eval_parameters();
        self.solve_sketch_inner(si, None, budget, sketch::Rebuild::Whole)
    }

    /// Fast path for a drag frame: no `eval_parameters` (parameters are static during a drag, and the
    /// dimension values were already applied by the last full solve) and a reduced iteration budget for
    /// responsiveness. On release the interface calls the full `solve_sketch`.
    ///
    /// Without this every drag frame ran `eval_parameters`, a full Levenberg-Marquardt solve (120
    /// iterations with a numeric Jacobian) and a regenerate over every sketch, which lagged visibly on any
    /// sizeable sketch.
    pub fn solve_sketch_drag_fast(&mut self, si: usize, drag: Option<(Id, f64, f64)>) -> f64 {
        // a point dragged: through what the drag keeps from frame to frame, where the sketch takes it
        if let Some(r) = drag.and_then(|d| self.drag_frame(si, d)) {
            return r;
        }
        self.solve_sketch_drag_frame_alone(si, drag)
    }

    /// A FRAME OF A DRAG made without what the drag keeps from frame to frame: the part solved and the loops rebuilt as
    /// the frame finds them. The reference of the frames made through it (`solve_sketch_drag_fast`).
    pub fn solve_sketch_drag_frame_alone(&mut self, si: usize, drag: Option<(Id, f64, f64)>) -> f64 {
        self.solve_sketch_inner(si, drag, crate::solver::Budget::FRAME, sketch::Rebuild::Moved)
    }
}
