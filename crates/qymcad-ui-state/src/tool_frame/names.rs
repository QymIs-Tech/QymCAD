//! THE NAME OF A VALUE OF A TOOL - the driver its formulas read it by - and whether it is a reference, in the row of its
//! field in the box at the geometry: the same in every workbench, whether the value is a dimension of a sketch, a number
//! of a feature or a value of a joint. The tool says which fields are named, what they are named now, what they are told
//! by and whether they can be references; the frame draws the name and the switch in the row of the field, keeps what is
//! typed and switched, judges the name, and hands the tool what was asked when the box is applied (`FieldAsk`). No tool
//! catches what is typed into a name, and none keeps a list of names of its own.

use super::*;

/// THE NAME AND THE REFERENCE OF ONE FIELD OF A BOX, as the tool gives them: the key of the field, the name it has now -
/// empty for none or for a value not laid yet - what the value is told by among the names of the document (none while
/// it is not laid), and whether it is a reference now - none where it cannot be one (a value of a feature, a joint).
#[derive(Clone, Debug, PartialEq)]
pub struct FieldName {
    pub field: String,
    pub had: String,
    pub target: Option<qymcad_core::model::DimTarget>,
    pub driven: Option<Lit>,
}

impl FieldName {
    /// Field `field` named `had`, told by `target`, no reference switch.
    pub fn of(field: &str, had: String, target: Option<qymcad_core::model::DimTarget>) -> FieldName {
        FieldName { field: field.to_string(), had, target, driven: None }
    }

    /// The same, with the switch of a reference standing at `driven`.
    pub fn with_driven(self, driven: Lit) -> FieldName {
        FieldName { driven: Some(driven), ..self }
    }
}

/// WHAT WAS ASKED OF A FIELD in the row of its name, as the box is applied: the name as typed and whether it is a
/// reference.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldAsk {
    pub field: String,
    pub name: String,
    pub driven: Lit,
    /// the formula the field holds, as typed
    pub formula: String,
}

impl FieldAsk {
    /// Was a name asked of the field other than the one it has.
    pub fn renamed(&self, had: &str) -> bool {
        self.name.trim() != had.trim()
    }
}

/// WHAT THE FRAME KEEPS OF A NAME OR A SWITCH between frames: the value, and the frame it was last drawn in - kept by a
/// box drawn the frame before, forgotten by a box opened anew.
#[derive(Clone)]
struct Kept<T> {
    value: T,
    frame: u64,
}

/// How wide the field of the name of a value is.
const NAME_WIDTH: f32 = 80.0;

/// What of a named field the frame keeps.
#[derive(Hash, Debug)]
enum Keeps {
    Name,
    Driven,
}

/// Where the frame keeps `what` of field `field` of box `b`.
fn kept_id(b: egui::Id, field: &str, what: Keeps) -> egui::Id {
    b.with(("the row of a name", field.to_string(), what))
}

/// THE FIELD OF THE NAME OF FIELD `field`, by one id whatever box holds it: one box is in hand at a time, and a gesture
/// on the screen finds the field by it.
pub fn name_field_id(field: &str) -> egui::Id {
    egui::Id::new(("the field of the name", field.to_string()))
}

/// What the frame keeps under `id`, if the box was drawn the frame before.
fn kept<T: Clone + Send + Sync + 'static>(ctx: &egui::Context, id: egui::Id) -> Option<T> {
    let now = ctx.cumulative_frame_nr();
    ctx.data(|d| d.get_temp::<Kept<T>>(id)).filter(|k| k.frame + 1 >= now).map(|k| k.value)
}

/// Keep `value` under `id` for this frame.
fn keep<T: Clone + Send + Sync + 'static>(ctx: &egui::Context, id: egui::Id, value: T) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| d.insert_temp(id, Kept { value, frame }));
}

/// The name of field `n` as it stands in box `b`: as typed, or as the tool gives it.
fn name_now(ctx: &egui::Context, b: egui::Id, n: &FieldName) -> String {
    kept::<String>(ctx, kept_id(b, &n.field, Keeps::Name)).unwrap_or_else(|| n.had.clone())
}

/// Whether field `n` stands as a reference in box `b`: as switched, or as the tool gives it.
pub(crate) fn driven_now(ctx: &egui::Context, b: egui::Id, n: &FieldName) -> Lit {
    kept::<Lit>(ctx, kept_id(b, &n.field, Keeps::Driven)).or(n.driven).unwrap_or(Lit::Off)
}

/// WHAT IS ASKED OF THE NAMED FIELDS OF BOX `b` NOW: the name and the reference of each.
pub(crate) fn asks_of(ctx: &egui::Context, b: egui::Id, names: &[FieldName], fields: &[CmdParam]) -> Vec<FieldAsk> {
    names
        .iter()
        .map(|n| {
            let formula = fields.iter().find(|p| p.key == n.field).map(|p| p.txt.clone()).unwrap_or_default();
            FieldAsk { field: n.field.clone(), name: name_now(ctx, b, n), driven: driven_now(ctx, b, n), formula }
        })
        .collect()
}

/// THE NUMBERS OF THE BAR OF `d` NAMED as every value of a box is - the starts of a thread, the ratio of a relation:
/// their rows get a driver, the box made for them where the tool has none. Answers their keys, the values no key of the
/// model holds (`name_free_values`).
pub fn name_bar_numbers(d: &mut Described) -> Vec<&'static str> {
    let keys: Vec<&'static str> = d.bar.iter().filter_map(|o| if let Opt::Number { key, .. } = o { Some(key.name) } else { None }).collect();
    if !keys.is_empty() {
        let spec = d.boxed.get_or_insert_with(|| BoxSpec::by(Place::close_by(By::ViewCorner)));
        spec.names.extend(keys.iter().filter(|k| !spec.names.iter().any(|n| n.field == **k)).map(|k| FieldName::of(k, String::new(), None)).collect::<Vec<_>>());
    }
    keys
}

/// THE VALUES NAMED IN A BOX THAT THE MODEL HOLDS NO PLACE FOR - fields `free` - each a parameter of the project by its
/// name and its formula (`Project::name_a_value`). Answers whether one was added.
pub fn name_free_values(project: &mut Project, asks: &[FieldAsk], free: &[&str]) -> bool {
    let mut added = false;
    for a in asks.iter().filter(|a| free.contains(&a.field.as_str()) && !a.name.trim().is_empty()) {
        added |= project.name_a_value(&a.name, &a.formula);
    }
    added
}

/// THE FIRST NAME OF BOX `b` THAT WILL NOT DO, as a refusal of its field: Enter with it would leave the value unnamed.
pub(crate) fn names_refusal(ctx: &egui::Context, project: &Project, b: egui::Id, names: &[FieldName]) -> Option<Refusal> {
    names.iter().find_map(|n| {
        let u = name_unfit(project, n.target.as_ref(), name_now(ctx, b, n).trim())?;
        Some(Refusal { key: n.field.clone(), field: qymcad_i18n::tr("sk-driver-label"), why: u.said })
    })
}

/// WHAT THE CELLS OF A NAME CAME TO: why the name will not do, for the line under the row, and whether Enter was pressed
/// in its field - Enter there applies the box as it does in a field of a value.
pub(crate) struct NameSays {
    pub unfit: Option<Unfit>,
    pub enter: Enter,
}

/// THE NAME AND THE REFERENCE OF FIELD `n` drawn in its row, after the field and its unit: "Driver", the name - its frame
/// red and why under the row when it will not do - and the switch of a reference where the value can be one.
pub(crate) fn name_cells(ui: &mut egui::Ui, d: &Drawing, n: &FieldName) -> NameSays {
    let b = d.b.id;
    let ctx = ui.ctx().clone();
    let mut text = name_now(&ctx, b, n);
    ui.label(qymcad_i18n::tr("sk-driver-label")).on_hover_text(qymcad_i18n::tr("sk-driver-name-hint"));
    // AT ITS OWN WIDTH, as the field of the value is: a width asked for is cut to what is left of the box, and the name
    // at the end of the row of a thread stood three points wide, "..."
    let h = ui.spacing().interact_size.y;
    let r = ui.add_sized([NAME_WIDTH, h], egui::TextEdit::singleline(&mut text).id(name_field_id(&n.field)).hint_text(qymcad_i18n::tr("sk-name-placeholder")));
    let enter = if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) { Enter::Pressed } else { Enter::Not };
    let unfit = name_unfit(d.look.project, n.target.as_ref(), text.trim());
    if unfit.is_some() {
        refused_frame(ui, d.look.pal, r.rect);
    }
    keep(&ctx, kept_id(b, &n.field, Keeps::Name), text);
    if let Some(had) = n.driven {
        let mut on = kept::<Lit>(&ctx, kept_id(b, &n.field, Keeps::Driven)).unwrap_or(had) == Lit::On;
        ui.checkbox(&mut on, qymcad_i18n::tr("sk-ref-short")).on_hover_text(qymcad_i18n::tr("sk-driven-hint"));
        keep(&ctx, kept_id(b, &n.field, Keeps::Driven), if on { Lit::On } else { Lit::Off });
    }
    NameSays { unfit, enter }
}

/// THE FRAME OF A FIELD THAT HOLDS WHAT IT CANNOT TAKE, in the colour of a refusal: the words of why stand under the row.
pub(crate) fn refused_frame(ui: &egui::Ui, pal: &qymcad_scheme::Palette, rect: egui::Rect) {
    ui.painter().rect_stroke(rect.expand(1.0), 2.0, egui::Stroke::new(1.5, tone_colour(pal, Tone::Refusal)), egui::StrokeKind::Outside);
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
            said: qymcad_i18n::tr2("par-name-taken", "name", nm, "where", &if o.path.is_empty() { qymcad_i18n::tr("par-owner-project") } else { path_in_words(&o.path) }),
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
