//! THE OPTIONS OF THE SKETCH TOOL IN HAND, as data for the bar of the frame: its name, its switches, its ways and its
//! numbers. What is touched comes back here (`opt_took`); the frame draws them, as it does for every tool.

use qymcad_ui_state::{AsLit, Armed, ClickTool, Did, EditTool, Limits, Mode, MoveKind, Number, Opt, OptKey, PatternKind, SketchShape, Zero};

/// The name the bar gives the tool in hand.
pub(crate) fn tool_name(armed: &Armed) -> String {
    let key = if let Some(kind) = armed.pattern() {
        match kind {
            PatternKind::Linear => "tool-lin-array",
            PatternKind::Circular => "tool-circ-array",
        }
    } else if let Some(m) = armed.moving() {
        match m {
            MoveKind::Move => "tool-move",
            MoveKind::Copy => "tool-copy",
            MoveKind::Rotate => "tool-rotate",
        }
    } else if let Some(c) = armed.click_tool() {
        match c {
            ClickTool::Trim => "tool-trim",
            ClickTool::Extend => "tool-extend",
            ClickTool::Break => "tool-break",
            ClickTool::Corner => "tool-fillet",
            ClickTool::CornerChamfer => "tool-chamfer",
            ClickTool::Project => "tool-project-body",
        }
    } else if armed.dimension().is_some() {
        "tool-dim"
    } else if armed.measuring() {
        "tool-measure"
    } else if let Some(shape) = armed.drawing() {
        match shape {
            SketchShape::Line => "tool-line",
            SketchShape::Rect => "tool-rect",
            SketchShape::Circle => "tool-circle",
            SketchShape::Arc => "tool-arc",
            SketchShape::Point => "tool-point",
            SketchShape::Polygon => "tool-polygon",
            SketchShape::Slot => "tool-slot",
            SketchShape::Ellipse => "sk-ellipse",
            SketchShape::Spline => "sk-spline",
            SketchShape::Text => "tool-text",
        }
    } else {
        match armed.modify() {
            Some(EditTool::Fillet) => "tool-fillet",
            Some(EditTool::Chamfer) => "tool-chamfer",
            Some(EditTool::Offset) => "tool-offset",
            Some(EditTool::Mirror) => "tool-mirror",
            Some(EditTool::LinearPattern) => "tool-lin-array-short",
            Some(EditTool::CircularPattern) => "tool-circ-array-short",
            _ => "tool-select",
        }
    };
    qymcad_i18n::tr(key)
}

/// A length of the bar, of `range`, in millimetres.
fn mm(range: std::ops::RangeInclusive<f64>) -> Limits {
    Limits { range, number: Number::Real, zero: Zero::Takes, unit: Some("unit-mm-suffix") }
}

/// A count of the bar: a whole number of `range`.
fn count(range: std::ops::RangeInclusive<f64>) -> Limits {
    Limits { range, number: Number::Whole, zero: Zero::Takes, unit: None }
}

/// A number of the bar under its caption key, kept under `key`.
fn number(key: OptKey, label: &'static str, value: f64, limits: Limits) -> Opt {
    Opt::Number { key, label, value, limits }
}

/// Words of the bar, said as they are.
fn said(key: &str) -> Opt {
    Opt::Readout(qymcad_i18n::tr(key))
}

/// A MODE OF A DRAWING TOOL ON ITS BAR: its word, and the hint over it.
struct ModeWord {
    word: &'static str,
    hint: Option<&'static str>,
}

/// A group of ways under `caption`, `now` in force.
fn ways(key: &'static str, caption: Option<&str>, words: &[ModeWord], now: usize) -> Opt {
    let choices = words
        .iter()
        .map(|w| match w.hint {
            Some(h) => Mode::new(qymcad_i18n::tr(w.word)).hint(qymcad_i18n::tr(h)),
            None => Mode::new(qymcad_i18n::tr(w.word)),
        })
        .collect();
    Opt::Modes { key: OptKey::of(key), caption: caption.map(qymcad_i18n::tr), choices, current: Some(now) }
}

/// THE WAYS OF A RECTANGLE: two corners, from the centre, three points of a turned one.
const RECT_WAYS: [ModeWord; 3] = [
    ModeWord { word: "opt-rect-2corners", hint: None },
    ModeWord { word: "opt-rect-centre", hint: None },
    ModeWord { word: "opt-rect-3pt", hint: Some("opt-rect-rotated") },
];

/// THE WAYS OF A CIRCLE: a centre and a radius, the two ends of a diameter, touching two curves, three points.
const CIRCLE_WAYS: [ModeWord; 4] = [
    ModeWord { word: "opt-circle-centre-radius", hint: None },
    ModeWord { word: "opt-circle-2pt", hint: Some("opt-circle-diameter-ends") },
    ModeWord { word: "opt-tangent-m", hint: Some("opt-tangent-hint") },
    ModeWord { word: "opt-rect-3pt", hint: Some("opt-circle-3pt-hint") },
];

/// THE WAYS OF AN ARC: a centre, a start and an end; three points; touching the curve it starts from.
const ARC_WAYS: [ModeWord; 3] = [
    ModeWord { word: "opt-arc-cse", hint: None },
    ModeWord { word: "opt-rect-3pt", hint: Some("opt-arc-3pt") },
    ModeWord { word: "opt-tangent", hint: Some("opt-arc-smooth-hint") },
];

/// THE WAYS OF A POLYGON: inscribed in its circle, about it, by the length of an edge.
const POLYGON_WAYS: [ModeWord; 3] = [ModeWord { word: "opt-polygon-inscribed", hint: None }, ModeWord { word: "opt-polygon-circumscribed", hint: None }, ModeWord { word: "opt-by-edge", hint: None }];

/// WHAT A PROJECTION TAKES: one edge under the cursor, or the whole outline of the face the sketch lies on.
const PROJECT_WAYS: [ModeWord; 2] = [ModeWord { word: "opt-edge", hint: Some("opt-project-edge-hint") }, ModeWord { word: "opt-face-outline", hint: Some("opt-face-outline-hint") }];

/// The ways of a sketch fillet: by its radius, its chord or the length of its arc.
const FILLET_WAYS: [qymcad_core::model::FilletBy; 3] = [qymcad_core::model::FilletBy::Radius, qymcad_core::model::FilletBy::Chord, qymcad_core::model::FilletBy::ArcLength];

/// A WAY OF A SKETCH CHAMFER ON ITS BAR: the mode and its word.
struct ChamferWay {
    mode: qymcad_core::feature::ChamferMode,
    word: &'static str,
}

/// THE WAYS OF A SKETCH CHAMFER: equal legs, two legs, a leg and an angle - the words of the chamfer of a part. Reported
/// (issue #35): the sketch chamfer took one distance only, and a chamfer of 5 x 3 or of 5 at 30 deg had to be built by
/// hand.
const CHAMFER_WAYS: [ChamferWay; 3] = [
    ChamferWay { mode: qymcad_core::feature::ChamferMode::Symmetric, word: "cmd-symmetric" },
    ChamferWay { mode: qymcad_core::feature::ChamferMode::TwoDist, word: "cmd-two-distances" },
    ChamferWay { mode: qymcad_core::feature::ChamferMode::DistAngle, word: "cmd-leg-angle" },
];

/// WHICH CORNER A TOOL CUTS: a fillet or a chamfer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Corner {
    Fillet,
    Chamfer,
}

/// THE WAYS A CORNER IS GIVEN: a fillet by its radius, its chord or the length of its arc; a chamfer by equal legs, two
/// legs, or a leg and an angle.
pub(crate) fn corner_ways(sk: &qymcad_ui_state::SketchCtx, corner: Corner) -> Opt {
    let prefs = &*sk.tool_prefs;
    if corner == Corner::Chamfer {
        let words: Vec<&str> = CHAMFER_WAYS.iter().map(|w| w.word).collect();
        let now = CHAMFER_WAYS.iter().position(|w| w.mode == prefs.chamfer_mode).unwrap_or(0);
        Opt::modes(OptKey::of("chamfer_way"), None, &words, now)
    } else {
        let words: Vec<&str> = FILLET_WAYS.iter().map(|by| qymcad_ui_state::fillet_label(*by)).collect();
        let now = FILLET_WAYS.iter().position(|by| *by == prefs.fillet_by).unwrap_or(0);
        Opt::modes(OptKey::of("fillet_by"), None, &words, now)
    }
}

/// THE WAYS AND THE NUMBERS OF THE CORNERS OF THE SHAPES CHOSEN - a fillet or a chamfer: the way it is given, its first
/// value and the second value of a chamfer of two legs or of a leg and an angle.
fn corner_opts(sk: &qymcad_ui_state::SketchCtx, corner: Corner) -> Vec<Opt> {
    let hi = 10000.0;
    use qymcad_core::feature::ChamferMode;
    let prefs = &*sk.tool_prefs;
    let mut opts = vec![corner_ways(sk, corner)];
    if corner == Corner::Chamfer {
        opts.push(number(OptKey::of("sk_fillet"), qymcad_ui_state::chamfer_d1_label(prefs.chamfer_mode), prefs.fillet, mm(0.01..=hi)));
        if prefs.chamfer_mode != ChamferMode::Symmetric {
            let angle = prefs.chamfer_mode == ChamferMode::DistAngle;
            let limits = Limits { range: 0.01..=if angle { 179.99 } else { 10000.0 }, number: Number::Real, zero: Zero::Takes, unit: Some(if angle { "unit-deg-suffix" } else { "unit-mm-suffix" }) };
            opts.push(number(OptKey::of("sk_chamfer2"), qymcad_ui_state::chamfer_d2_label(prefs.chamfer_mode), prefs.chamfer_second, limits));
        }
    } else {
        opts.push(number(OptKey::of("sk_fillet"), qymcad_ui_state::fillet_label(prefs.fillet_by), prefs.fillet, mm(0.01..=hi)));
    }
    opts
}

/// The fields of a linear pattern: the count and the step.
fn linear_steps(pat: &qymcad_ui_state::SketchPattern) -> Vec<Opt> {
    vec![
        number(OptKey::of("skpat_count"), "opt-count", f64::from(pat.count), count(2.0..=200.0)),
        number(OptKey::of("skpat_dx"), "opt-step-x", pat.dx, mm(-100000.0..=100000.0)),
        number(OptKey::of("skpat_dy"), "opt-step-y", pat.dy, mm(-100000.0..=100000.0)),
    ]
}

/// The fields of a circular pattern: the count and the angle it spans.
fn circular_steps(pat: &qymcad_ui_state::SketchPattern) -> Vec<Opt> {
    let angle = Limits { range: -360.0..=360.0, number: Number::Real, zero: Zero::Takes, unit: Some("unit-deg-suffix") };
    vec![number(OptKey::of("skpat_count"), "opt-count", f64::from(pat.count), count(2.0..=200.0)), number(OptKey::of("skpat_angle"), "cmd-angle", pat.angle, angle)]
}

/// THE OPTIONS OF THE DRAWING TOOL IN HAND: construction, no ties, and what the shape takes - the text and its font, the
/// sides of a polygon, the way of a rectangle, a circle or an arc.
fn drawing_opts(sk: &qymcad_ui_state::SketchCtx, shape: SketchShape) -> Vec<Opt> {
    let prefs = &*sk.tool_prefs;
    let mut opts = vec![
        Opt::switch(OptKey::of("construction"), qymcad_i18n::tr("opt-construction-short"), sk.tool.construction.lit()).hint(qymcad_i18n::tr("opt-construction-hint")),
        Opt::switch(OptKey::of("no_ties"), qymcad_i18n::tr("opt-no-ties-short"), (sk.tool.ties == qymcad_ui_state::Ties::Free).lit()).hint(qymcad_i18n::tr("opt-no-ties-hint")),
    ];
    match shape {
        SketchShape::Text => {
            opts.push(said("tool-text"));
            opts.push(Opt::Text { key: OptKey::of("text"), text: prefs.text.clone(), placeholder: String::new(), unfit: None, hint: None });
            opts.push(number(OptKey::of("text_h"), "opt-height-short", prefs.text_h, mm(0.1..=1000.0)));
            // WHICH FONT IT WILL BE WRITTEN IN, named on the button itself. "Font..." alone said nothing: a person pressed it
            // to find out what they were about to write with.
            opts.push(Opt::press(OptKey::of("font"), qymcad_ui_state::font_label(&prefs.font, "opt-font")).hint(qymcad_i18n::tr("opt-pick-font")));
            opts.push(Opt::switch(OptKey::of("note"), qymcad_i18n::tr("opt-note"), prefs.text_note.lit()).hint(qymcad_i18n::tr("opt-text-as-note")));
        }
        SketchShape::Polygon => {
            opts.push(number(OptKey::of("poly_n"), "opt-sides", f64::from(prefs.poly_n), count(3.0..=64.0)));
            opts.push(ways("poly_mode", None, &POLYGON_WAYS, usize::from(prefs.poly_mode)));
        }
        SketchShape::Rect => opts.push(ways("rect_mode", Some("opt-rect-mode"), &RECT_WAYS, usize::from(prefs.rect_mode))),
        SketchShape::Circle => {
            // centre and radius, two points, touching two curves, three points: the modes of one tool, as the arc's
            opts.push(ways("circ_mode", Some("opt-circle-mode"), &CIRCLE_WAYS, usize::from(prefs.circ_mode)));
            if prefs.circ_mode == 2 {
                opts.push(number(OptKey::of("sk_circle_touching_r"), "opt-radius", prefs.circ_r, Limits { zero: Zero::Refused, ..mm(0.01..=10000.0) }));
            }
        }
        SketchShape::Arc => opts.push(ways("arc_mode", Some("opt-arc-mode"), &ARC_WAYS, usize::from(prefs.arc_mode))),
        _ => {}
    }
    opts
}

/// THE OPTIONS OF THE TOOL IN HAND - only those of the tool or operation in hand - and how many things are selected.
pub(crate) fn tool_opts(sk: &qymcad_ui_state::SketchCtx) -> Vec<Opt> {
    let mut opts = if let Some(shape) = sk.armed.drawing() {
        drawing_opts(sk, shape)
    } else if sk.armed.dimension().is_some() {
        vec![said("opt-dim-hint")]
    } else if let Some(click) = sk.armed.click_tool() {
        click_opts(sk, click)
    } else if let Some(kind) = sk.armed.pattern() {
        // the pattern: the parameters and a hint - the preview is live, Enter applies
        let mut opts = if kind == PatternKind::Linear {
            let mut v = linear_steps(sk.sk_pat);
            v.push(number(OptKey::of("skpat_count2"), "opt-rows", f64::from(sk.sk_pat.count2), count(1.0..=200.0)));
            if sk.sk_pat.count2 > 1 {
                v.push(number(OptKey::of("skpat_dx2"), "opt-row-step-x", sk.sk_pat.dx2, mm(-100000.0..=100000.0)));
                v.push(number(OptKey::of("skpat_dy2"), "opt-row-step-y", sk.sk_pat.dy2, mm(-100000.0..=100000.0)));
            }
            v
        } else {
            let mut v = circular_steps(sk.sk_pat);
            v.push(said(if sk.pat.center.is_some() { "opt-centre-set" } else { "opt-click-rotation-centre" }));
            v
        };
        opts.push(said(if sk.pat.edit.is_some() { "opt-enter-update" } else { "opt-enter-apply" }));
        opts
    } else {
        match sk.armed.modify() {
            Some(EditTool::Fillet) => corner_opts(sk, Corner::Fillet),
            Some(EditTool::Chamfer) => corner_opts(sk, Corner::Chamfer),
            Some(EditTool::Offset) => vec![number(OptKey::of("sk_offset"), "opt-distance", sk.tool_prefs.offset, Limits { zero: Zero::Refused, ..mm(-10000.0..=10000.0) })],
            Some(EditTool::LinearPattern) => linear_steps(sk.sk_pat),
            Some(EditTool::CircularPattern) => circular_steps(sk.sk_pat),
            Some(EditTool::Mirror) => vec![said("opt-mirror-axis-hint")],
            _ => vec![Opt::switch(OptKey::of("construction"), qymcad_i18n::tr("opt-construction-short"), sk.tool.construction.lit()), said("opt-select-hint")],
        }
    };
    opts.push(Opt::Readout(qymcad_i18n::tr1("opt-selected-n", "n", &sk.sel_sk.items.len().to_string())));
    opts
}

/// THE OPTIONS OF A TOOL OF CLICKS: the ways and the fields of a corner, what a projection takes, both ends of an
/// extension, and what the tool waits for.
fn click_opts(sk: &qymcad_ui_state::SketchCtx, click: ClickTool) -> Vec<Opt> {
    let mut opts = match click {
        // THE VALUE OF A CORNER CLICKED IS TYPED IN THE BOX AT THE CORNER, which the click opens (`CornerEdit`): the bar says
        // only how the corner is given
        ClickTool::Corner | ClickTool::CornerChamfer => vec![corner_ways(sk, if click == ClickTool::CornerChamfer { Corner::Chamfer } else { Corner::Fillet })],
        ClickTool::Project => vec![ways("proj", None, &PROJECT_WAYS, usize::from(sk.tool.proj_face))],
        ClickTool::Extend => vec![Opt::switch(OptKey::of("extend_both"), qymcad_i18n::tr("opt-extend-both"), sk.tool_prefs.extend_both.lit()).hint(qymcad_i18n::tr("opt-extend-both-hint"))],
        _ => Vec::new(),
    };
    let hint = match click {
        ClickTool::Trim => Some("opt-trim-hint"),
        // the line held, the tool says what it waits for next
        ClickTool::Extend if sk.tool.extend.is_some() => Some("opt-extend-to-hint"),
        ClickTool::Extend => Some("opt-extend-hint"),
        ClickTool::Break => Some("opt-break-hint"),
        ClickTool::Corner => Some("opt-fillet-hint"),
        ClickTool::CornerChamfer => Some("opt-chamfer-hint"),
        ClickTool::Project => Some("opt-project-hint"),
    };
    opts.extend(hint.map(said));
    opts
}

/// WHAT WAS DONE TO AN OPTION OF THE SKETCH TOOL IN HAND, done to the tool.
pub(crate) fn opt_took(sk: &mut qymcad_ui_state::SketchCtx, did: Did) {
    let to_u8 = |n: usize| u8::try_from(n).unwrap_or(0);
    match did {
        Did::Switched { key } => match key.name {
            "construction" => sk.tool.construction = !sk.tool.construction,
            "no_ties" => sk.tool.ties = if sk.tool.ties == qymcad_ui_state::Ties::Free { qymcad_ui_state::Ties::Laid } else { qymcad_ui_state::Ties::Free },
            "note" => sk.tool_prefs.text_note = !sk.tool_prefs.text_note,
            "extend_both" => sk.tool_prefs.extend_both = !sk.tool_prefs.extend_both,
            _ => {}
        },
        Did::Chose { key, index } => match key.name {
            "poly_mode" => sk.tool_prefs.poly_mode = to_u8(index),
            "rect_mode" | "arc_mode" => {
                if key.name == "rect_mode" {
                    sk.tool_prefs.rect_mode = to_u8(index);
                } else {
                    sk.tool_prefs.arc_mode = to_u8(index);
                }
                sk.tool.pts.clear();
            }
            "circ_mode" => {
                sk.tool_prefs.circ_mode = to_u8(index);
                sk.tool.pts.clear();
                sk.tool.circ_tan.clear();
            }
            "proj" => sk.tool.proj_face = index == 1,
            "fillet_by" | "chamfer_way" => {
                if key.name == "fillet_by" {
                    sk.tool_prefs.fillet_by = FILLET_WAYS[index.min(FILLET_WAYS.len() - 1)];
                } else {
                    qymcad_ui_state::set_chamfer_mode(sk.tool_prefs, CHAMFER_WAYS[index.min(CHAMFER_WAYS.len() - 1)].mode);
                }
                // the field of a corner already open takes the keys back: the click on the word took them, and the values
                // typed after choosing two distances on a chamfer of two lines chosen before the tool went nowhere
                sk.corner.focus |= sk.corner.at.is_some();
            }
            _ => {}
        },
        Did::Typed { key, value } => typed_into(sk, key.name, value),
        Did::Retyped { key, text } if key.name == "text" => sk.tool_prefs.text = text,
        Did::Pressed { key } if key.name == "font" => sk.font_cache.picker.open = true,
        _ => {}
    }
}

/// A NUMBER OF THE BAR TYPED: the value of the tool it holds.
fn typed_into(sk: &mut qymcad_ui_state::SketchCtx, key: &str, value: f64) {
    let whole = |v: f64| v.round().max(0.0) as u32;
    match key {
        "text_h" => sk.tool_prefs.text_h = value,
        "poly_n" => sk.tool_prefs.poly_n = whole(value),
        "sk_circle_touching_r" => sk.tool_prefs.circ_r = value,
        "sk_fillet" => sk.tool_prefs.fillet = value,
        "sk_chamfer2" => sk.tool_prefs.chamfer_second = value,
        "sk_offset" => sk.tool_prefs.offset = value,
        "skpat_count" => sk.sk_pat.count = whole(value),
        "skpat_count2" => sk.sk_pat.count2 = whole(value),
        "skpat_dx" => sk.sk_pat.dx = value,
        "skpat_dy" => sk.sk_pat.dy = value,
        "skpat_dx2" => sk.sk_pat.dx2 = value,
        "skpat_dy2" => sk.sk_pat.dy2 = value,
        "skpat_angle" => sk.sk_pat.angle = value,
        _ => {}
    }
}
