//! A CURVE OF A DRAWING WHOSE NUMBER IS NOT A FINITE NUMBER IS LEFT OUT AND NAMED, in DXF and in an IGES drawing.
//!
//! Both readers parse numbers with Rust's `f64` parse, which takes `nan`, `inf` and `1e400` (IGES `1.D400`). Reported
//! behaviour: a DXF LINE at x = nan came into the sketch as a NaN point and a line, with no word; a LINE at x = inf
//! panicked further on, in the weld of the sketch's ends. Such an entity is not drawn, and it is counted by its kind
//! among the invalid ones, as a SPLINE whose numbers make no curve is - the rest of the drawing comes in.
use qymcad_core::geom::ProfEdge;
use qymcad_io::{import_dxf, read_iges_drawing};

/// A folder for the files, under `target`: nothing goes to `/tmp`, which lives in memory.
fn file(name: &str, text: String) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/not-a-number-drawing"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join(name);
    std::fs::write(&p, text).expect("written");
    p.to_string_lossy().into_owned()
}

/// Whether every number of every curve is finite.
fn all_finite(curves: &[ProfEdge]) -> bool {
    curves
        .iter()
        .all(|c| (0..=8).filter_map(|k| c.curve_at(k as f64 / 8.0)).all(|p| p.x.is_finite() && p.y.is_finite()) && c.ends().is_none_or(|e| e.iter().all(|p| p.x.is_finite() && p.y.is_finite())))
}

/// A DXF of one sound LINE (0,0)-(10,0) and the entity `bad`, written as group codes.
fn dxf(bad: &str) -> String {
    format!("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n11\n10.0\n21\n0.0\n31\n0.0\n{bad}0\nENDSEC\n0\nEOF\n")
}

/// One case: the file's name, the entity whose number is not a number, and the kind it is named by.
struct Case {
    name: &'static str,
    bad: String,
    kind: &'static str,
}

#[test]
fn a_dxf_entity_whose_number_is_not_a_number_is_left_out_and_named() {
    let line = |x: &str| format!("0\nLINE\n8\n0\n10\n{x}\n20\n0.0\n30\n0.0\n11\n5.0\n21\n5.0\n31\n0.0\n");
    let lw = |x: &str, bulge: &str| format!("0\nLWPOLYLINE\n100\nAcDbEntity\n8\n0\n100\nAcDbPolyline\n90\n3\n70\n0\n10\n0.0\n20\n20.0\n42\n{bulge}\n10\n{x}\n20\n20.0\n10\n20.0\n20\n30.0\n");
    let cases = [
        Case { name: "line-nan", bad: line("nan"), kind: "LINE" },
        Case { name: "line-inf", bad: line("inf"), kind: "LINE" },
        Case { name: "line-1e400", bad: line("1e400"), kind: "LINE" },
        Case { name: "polyline-nan", bad: lw("nan", "0.0"), kind: "LWPOLYLINE" },
        Case { name: "polyline-bulge-nan", bad: lw("10.0", "nan"), kind: "LWPOLYLINE" },
        Case { name: "circle-nan", bad: "0\nCIRCLE\n8\n0\n10\n20.0\n20\n20.0\n30\n0.0\n40\nnan\n".into(), kind: "CIRCLE" },
        Case { name: "arc-inf", bad: "0\nARC\n8\n0\n10\ninf\n20\n0.0\n30\n0.0\n40\n5.0\n50\n0.0\n51\n90.0\n".into(), kind: "ARC" },
        Case {
            name: "ellipse-nan",
            bad: "0\nELLIPSE\n100\nAcDbEntity\n8\n0\n100\nAcDbEllipse\n10\n0.0\n20\nnan\n30\n0.0\n11\n20.0\n21\n0.0\n31\n0.0\n40\n0.5\n41\n0.0\n42\n6.283185307179586\n".into(),
            kind: "ELLIPSE",
        },
    ];
    let mut failures = Vec::new();
    for case in cases {
        let got = import_dxf(&file(&format!("{}.dxf", case.name), dxf(&case.bad))).expect("the drawing is read");
        let sound = got.curves.iter().any(|c| matches!(c, ProfEdge::Line { a, b } if a.x == 0.0 && a.y == 0.0 && b.x == 10.0 && b.y == 0.0));
        if !all_finite(&got.curves) || !sound || got.invalid != vec![(case.kind.to_string(), 1)] {
            failures.push(format!("{}: curves {:?}, invalid {:?}, not read {:?}", case.name, got.curves, got.invalid, got.skipped));
        }
    }
    assert!(failures.is_empty(), "a number that is not a number came in, or was not named:\n{}", failures.join("\n"));
}

/// One IGES entity: the name of its case, its type, its form and its parameters without the type and the terminator.
struct Igs {
    name: &'static str,
    typ: u32,
    form: u32,
    params: &'static str,
}

/// An IGES drawing of one sound line (0,0)-(10,0) and the entity `bad`, in the fixed columns IGES is written in.
fn iges(bad: &Igs) -> String {
    let g = "1H,,1H;,1Hc,1Hc,1Hc,1Hc,32,38,6,308,15,1Hc,1.,2,2HMM;";
    let ents = [&Igs { name: "sound", typ: 110, form: 0, params: "0.,0.,0.,10.,0.,0." }, bad];
    let mut out = vec![format!("{:<72}S{:>7}", "a drawing for the check", 1), format!("{:<72}G{:>7}", g, 1)];
    let mut p = Vec::new();
    for (i, e) in ents.iter().enumerate() {
        let de = 2 * i + 1;
        out.push(format!("{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}D{:>7}", e.typ, p.len() + 1, 0, 0, 0, 0, 0, 0, "00000000", de));
        out.push(format!("{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}D{:>7}", e.typ, 0, 0, 1, e.form, "", "", "", 0, de + 1));
        p.push(format!("{:<64}{:>8}P{:>7}", format!("{},{};", e.typ, e.params), de, p.len() + 1));
    }
    let n = p.len();
    out.extend(p);
    out.push(format!("S{:>7}G{:>7}D{:>7}P{:>7}{:<40}T{:>7}", 1, 1, 2 * n, n, "", 1));
    out.join("\n") + "\n"
}

#[test]
fn an_iges_entity_whose_number_is_not_a_number_is_left_out_and_named() {
    let cases = [
        Igs { name: "line-nan", typ: 110, form: 0, params: "NaN,0.,0.,5.,5.,0." },
        Igs { name: "line-inf", typ: 110, form: 0, params: "inf,0.,0.,5.,5.,0." },
        Igs { name: "line-1d400", typ: 110, form: 0, params: "1.D400,0.,0.,5.,5.,0." },
        Igs { name: "arc-nan", typ: 100, form: 0, params: "0.,NaN,0.,5.,0.,0.,5." },
        Igs { name: "polyline-nan", typ: 106, form: 11, params: "1,3,0.,0.,20.,NaN,20.,10.,30." },
        Igs { name: "spline-nan", typ: 126, form: 0, params: "1,1,0,0,1,0,0.,0.,1.,1.,1.,1.,0.,30.,0.,NaN,40.,0.,0.,1." },
    ];
    let mut failures = Vec::new();
    for bad in &cases {
        let got = read_iges_drawing(&file(&format!("{}.igs", bad.name), iges(bad))).expect("the drawing is read");
        let sound = got.curves.iter().any(|c| matches!(c, ProfEdge::Line { a, b } if a.x == 0.0 && a.y == 0.0 && b.x == 10.0 && b.y == 0.0));
        let named = matches!(got.invalid.as_slice(), [k] if k.typ == bad.typ && k.count == 1);
        if !all_finite(&got.curves) || !sound || !named {
            failures.push(format!("{}: curves {:?}, invalid {:?}", bad.name, got.curves, got.invalid));
        }
    }
    assert!(failures.is_empty(), "a number that is not a number came in, or was not named:\n{}", failures.join("\n"));
}
