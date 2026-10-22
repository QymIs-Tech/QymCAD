//! THE NAME OF A VALUE OF A TOOL - the driver its formulas read it by - in the box at the geometry: one row under the
//! field it names, the same in every workbench, whether the value is a dimension of a sketch, a number of a feature or a
//! value of a joint. The tool says which fields are named and what they are named so far; the frame draws the row and
//! tells the tool what was typed into it.

use super::*;

/// The option every row of a name answers by, `at` the place of the field it names among the fields of the tool.
pub const FIELD_NAME: &str = "field-name";

/// THE NAME OF ONE FIELD OF A BOX: the key of the field, the name as typed so far, and why it will not do.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldName {
    pub field: String,
    pub text: String,
    pub unfit: Option<Unfit>,
}

impl FieldName {
    /// The name of field `field` as typed so far, judged against the document: `target` is what the value is told by,
    /// none while it is not laid yet.
    pub fn judged(project: &Project, field: &str, text: String, target: Option<&qymcad_core::model::DimTarget>) -> FieldName {
        let unfit = name_unfit(project, target, text.trim());
        FieldName { field: field.to_string(), text, unfit }
    }
}

/// THE ROW OF THE NAME of field `p` - at place `at` among the fields of the tool - as `n` says it.
pub(crate) fn name_row(at: usize, n: &FieldName) -> Row {
    let text = Opt::Text {
        key: OptKey::at(FIELD_NAME, at),
        text: n.text.clone(),
        placeholder: qymcad_i18n::tr("sk-name-placeholder"),
        // why the name will not do stands on a row of its own under it (`grid`)
        unfit: None,
        hint: Some(qymcad_i18n::tr("sk-driver-name-hint")),
    };
    Row::new(qymcad_i18n::tr("sk-driver-label")).opt(text)
}

/// The gate the names of a box leave: Enter with a name refused would leave the value unnamed, so the name says why.
pub fn names_gate(names: &[FieldName]) -> Gate {
    match names.iter().find_map(|n| n.unfit.as_ref()) {
        Some(u) => Gate::Wrong(u.said.clone()),
        None => Gate::Open,
    }
}

/// WHY NAME `nm` WILL NOT DO for the value `target` is told by, none when it will. An empty name is legitimate: the value
/// simply stops being a driver. A value not laid yet (`target` none) has nobody to share a name with: any owner of the
/// name holds it already.
///
/// A TAKEN NAME IS SAID WITH WHOSE IT IS, not merely "taken": otherwise the namesake has to be hunted for across the
/// whole project by hand. A name with no owner is unusable in itself, and the line and its hint say why - the hint said
/// "taken" over `tau`, which nobody holds. A name that would close a circle of formulas says the chain.
pub fn name_unfit(project: &Project, target: Option<&qymcad_core::model::DimTarget>, nm: &str) -> Option<Unfit> {
    let circle = target.and_then(|t| project.naming_closes_circle(t, nm));
    let taken = match target {
        Some(t) => project.driver_name_taken_by(nm, t),
        None => project.name_owner(nm).is_some(),
    };
    if nm.is_empty() || (qymcad_core::drivers::check_ident(nm).is_ok() && !taken && circle.is_none()) {
        return None;
    }
    let owner = project.name_owner(nm).filter(|_| taken);
    let unfit = match circle {
        Some(chain) => qymcad_i18n::error_words::expr_error_text(&qymcad_core::errors::ExprError::LeansOnItself(chain)),
        None => qymcad_i18n::error_words::ident_refusal_text(nm),
    };
    Some(match owner {
        Some(o) => Unfit {
            said: qymcad_i18n::tr2("par-name-taken", "name", "", "where", &if o.path.is_empty() { qymcad_i18n::tr("par-owner-project") } else { path_in_words(&o.path) }),
            hint: qymcad_i18n::tr("sk-driver-name-taken-hint"),
        },
        None => Unfit { said: unfit.clone(), hint: unfit },
    })
}

/// THE VALUE `target` IS TOLD BY NAMED `nm`, AS ONE CHANGE OF THE DOCUMENT: a name given anew, a name changed - the
/// formulas reading the old name carried over to the new one, or they would stay pointed at a name that no longer exists
/// and the model would break silently - or a name taken away. A name the same as before, or one that will not do,
/// changes nothing. Answers whether the name changed.
pub fn give_name(project: &mut Project, target: qymcad_core::model::DimTarget, nm: &str) -> bool {
    let nm = nm.trim();
    let old = qymcad_i18n::name(&project.name_of_target(&target)).trim().to_string();
    if nm == old || name_unfit(project, Some(&target), nm).is_some() {
        return false;
    }
    if !old.is_empty() && !nm.is_empty() {
        let _ = project.rename_driver(&old, nm);
    } else {
        project.name_dim(nm.to_string(), target);
    }
    true
}
