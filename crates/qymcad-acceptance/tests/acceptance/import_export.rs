//! WHAT GOES OUT AND COMES BACK: the block written in every format the program offers and brought in again, a file
//! that is not what it claims, and the units a mesh is read in.
use qymcad::{Key, Session};
use qymcad_acceptance::{build, probe};

/// The path of a file of this check's own.
fn a_path(name: &str, ext: &str) -> String {
    format!("{}/through-{name}.{ext}", std::env::temp_dir().display())
}

/// Write the block of the first part out in the format the menu calls `item`, into `path`. The mesh formats ask for
/// a quality first; the exact ones go straight to the file.
fn write_it_out(s: &mut Session, item: &str, path: &str) {
    // a file left at the same place by an earlier run would pass for the one written now
    let _ = std::fs::remove_file(path);
    let (file, export) = (s.word("menu-file"), s.word("file-export"));
    s.menu(&[&file, &export, item]);
    let quality = s.word("stl-standard");
    if s.shows(&quality) {
        s.press_word(&quality);
    }
    s.answer_file(path);
    assert!(std::path::Path::new(path).exists(), "{item} was not written to {path}: the program says {:?}", s.status());
}

/// Bring the file at `path` into a fresh project and answer the bodies it holds.
fn bring_it_in(path: &str) -> (Session, Vec<qymcad::Solid>) {
    let mut s = Session::start();
    build::into_the_first_part(&mut s);
    build::import(&mut s, path);
    let bodies = s.document().bodies.into_iter().filter(|b| !b.consumed && !b.sheet).collect();
    (s, bodies)
}

/// THE BLOCK GOES OUT IN THIS FORMAT AND COMES BACK holding what it held: 12000 mm^3, 40 by 30 by 10.
fn a_block_goes_out_and_comes_back(item: &str, ext: &str) {
    let path = a_path(ext, ext);
    let mut s = Session::start();
    build::block(&mut s);
    write_it_out(&mut s, item, &path);
    let (mut s, bodies) = bring_it_in(&path);
    assert!(bodies.len() == 1, "{ext} came back as {} bodies: {:?}", bodies.len(), bodies.iter().map(|b| (b.name.clone(), b.volume)).collect::<Vec<_>>());
    let b = &bodies[0];
    assert!((b.volume - 12000.0).abs() < 1.0, "the block written as {ext} came back holding {} instead of 12000; the program says {:?}", b.volume, s.status());
    let size = [b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]];
    assert!((size[0] - 40.0).abs() < 0.01 && (size[1] - 30.0).abs() < 0.01 && (size[2] - 10.0).abs() < 0.01, "the block written as {ext} came back {size:?} instead of 40 by 30 by 10");
}

probe! {
    /// STEP goes out and comes back.
    fn a_block_goes_through_step() {
        a_block_goes_out_and_comes_back("STEP\u{2026}", "step");
    }
}

probe! {
    /// IGES goes out and comes back.
    fn a_block_goes_through_iges() {
        a_block_goes_out_and_comes_back("IGES\u{2026}", "iges");
    }
}

probe! {
    /// STL goes out and comes back.
    fn a_block_goes_through_stl() {
        a_block_goes_out_and_comes_back("STL\u{2026}", "stl");
    }
}

probe! {
    /// OBJ goes out and comes back.
    fn a_block_goes_through_obj() {
        a_block_goes_out_and_comes_back("OBJ\u{2026}", "obj");
    }
}

probe! {
    /// PLY goes out and comes back.
    fn a_block_goes_through_ply() {
        a_block_goes_out_and_comes_back("PLY\u{2026}", "ply");
    }
}

probe! {
    /// glTF goes out and comes back.
    fn a_block_goes_through_gltf() {
        a_block_goes_out_and_comes_back("glTF\u{2026}", "glb");
    }
}

probe! {
    /// 3MF goes out and comes back.
    fn a_block_goes_through_3mf() {
        a_block_goes_out_and_comes_back("3MF\u{2026}", "3mf");
    }
}

probe! {
    /// AMF goes out and comes back.
    fn a_block_goes_through_amf() {
        a_block_goes_out_and_comes_back("AMF\u{2026}", "amf");
    }
}

probe! {
    /// A FILE THAT IS NOT WHAT IT CLAIMS IS REFUSED IN WORDS, and nothing comes into the document.
    fn a_file_that_is_not_what_it_claims_is_refused() {
        let path = a_path("nonsense", "step");
        std::fs::write(&path, "this is not a STEP file at all\n").expect("the file of nonsense is written");
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let before = s.document().bodies.len();
        let (file, import) = (s.word("menu-file"), s.word("file-import"));
        s.menu(&[&file, &import]);
        s.answer_file(&path);
        let said = s.status();
        assert!(!said.is_empty() && said != s.word("io-ready"), "nothing says the file could not be read: the program says {said:?}");
        assert!(s.document().bodies.len() == before, "something came into the document out of a file of nonsense: {:?}", s.document().bodies);
    }
}

probe! {
    /// AN EMPTY FILE IS REFUSED IN WORDS as well.
    fn an_empty_file_is_refused() {
        let path = a_path("empty", "stl");
        std::fs::write(&path, "").expect("the empty file is written");
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let (file, import) = (s.word("menu-file"), s.word("file-import"));
        s.menu(&[&file, &import]);
        s.answer_file(&path);
        let said = s.status();
        assert!(!said.is_empty() && said != s.word("io-ready"), "nothing says the empty file could not be read: the program says {said:?}");
        assert!(s.document().bodies.is_empty(), "something came into the document out of an empty file: {:?}", s.document().bodies);
    }
}

probe! {
    /// THE UNITS OF A MESH ARE ASKED AND HELD TO: the same file read in centimetres is ten times the size.
    fn the_units_of_a_mesh_are_held_to() {
        let path = a_path("in-centimetres", "stl");
        let mut s = Session::start();
        build::block(&mut s);
        write_it_out(&mut s, "STL\u{2026}", &path);
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let (file, import) = (s.word("menu-file"), s.word("file-import"));
        s.menu(&[&file, &import]);
        s.answer_file(&path);
        let cm = s.word("import-unit-cm");
        s.press_word_near(&cm, qymcad::pos2(1200.0, 600.0));
        let go = s.word("import-scale-import");
        s.press_word_near(&go, qymcad::pos2(1200.0, 700.0));
        let body = s.document().bodies.into_iter().rfind(|b| !b.consumed && !b.sheet).expect("the body that came in");
        let across = body.max[0] - body.min[0];
        assert!((across - 400.0).abs() < 0.1, "a block of 40 read as centimetres is 400 across, and it is {across}");
    }
}

probe! {
    /// AN ASSEMBLY GOES OUT AND COMES BACK WITH ITS STRUCTURE: two parts, their names and the colours they were
    /// drawn in.
    fn an_assembly_goes_out_and_comes_back_with_its_structure() {
        let path = a_path("an-assembly", "step");
        let mut s = Session::start();
        build::block(&mut s);
        let assembly = s.word("wb-assembly");
        s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
        let new_part = s.word("tb-new-part-hint");
        s.press_hint(&new_part);
        build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        s.key(Key::Enter);
        s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
        let names: Vec<String> = s.document().parts.iter().map(|p| p.name.clone()).collect();
        let colours: Vec<[u8; 3]> = s.document().bodies.iter().filter(|b| !b.consumed && !b.sheet).map(|b| b.colour).collect();
        write_it_out(&mut s, "STEP\u{2026}", &path);
        let (mut s, bodies) = bring_it_in(&path);
        assert!(bodies.len() == 2, "the assembly of two parts came back as {} bodies: {:?}", bodies.len(), bodies.iter().map(|b| (b.name.clone(), b.volume)).collect::<Vec<_>>());
        let back: Vec<String> = s.document().parts.iter().map(|p| p.name.clone()).collect();
        assert!(names.iter().all(|n| back.contains(n)), "the parts came back under other names: {names:?} went out and {back:?} came back");
        let now: Vec<[u8; 3]> = bodies.iter().map(|b| b.colour).collect();
        assert!(now == colours, "the parts came back in other colours: {colours:?} went out and {now:?} came back");
    }
}

probe! {
    /// A DRAWING FROM ANOTHER PROGRAM COMES IN AS A SKETCH that is open for drawing on: the curves of a DXF placed
    /// on the table by a click.
    fn a_drawing_comes_in_as_a_sketch() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let (file, import) = (s.word("menu-file"), s.word("file-import"));
        s.menu(&[&file, &import]);
        s.answer_file(format!("{}/../../examples/plate.dxf", env!("CARGO_MANIFEST_DIR")));
        let table = s.in_space([0.0, 0.0, 0.0]);
        s.click(table);
        let sk = s.document().sketches.last().cloned().unwrap_or_else(|| panic!("the drawing came in and no sketch was made: the program says {:?}", s.status()));
        let drawn = sk.lines + sk.arcs + sk.circles + sk.ellipses + sk.splines;
        assert!(drawn > 0, "the sketch made of the drawing holds no curve at all: the program says {:?}", s.status());
        assert!(s.document().editing.as_deref() == Some(sk.name.as_str()), "the imported sketch did not open for drawing: the program edits {:?}", s.document().editing);
        let size = [sk.max[0] - sk.min[0], sk.max[1] - sk.min[1]];
        assert!(size[0] > 1.0 && size[1] > 1.0, "the drawing came in flattened to {size:?}");
    }
}

probe! {
    /// AN SVG COMES IN AT ITS OWN SIZE: a 40 by 30 rectangle drawn in another program on a sheet in millimetres is 40 by
    /// 30 in the sketch, and its four sides are four lines. (A sheet with no unit is in pixels of 1/96 inch, as the
    /// SVG specification says.)
    fn a_drawing_in_svg_comes_in_as_a_sketch() {
        let path = a_path("a-rectangle", "svg");
        std::fs::write(&path, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40mm\" height=\"30mm\" viewBox=\"0 0 40 30\"><path d=\"M 0 0 L 40 0 L 40 30 L 0 30 Z\" fill=\"none\" stroke=\"black\"/></svg>").expect("the drawing of a rectangle is written");
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let (file, import) = (s.word("menu-file"), s.word("file-import"));
        s.menu(&[&file, &import]);
        s.answer_file(&path);
        let table = s.in_space([0.0, 0.0, 0.0]);
        s.click(table);
        let sk = s.document().sketches.last().cloned().unwrap_or_else(|| panic!("the drawing came in and no sketch was made: the program says {:?}", s.status()));
        assert!(sk.lines == 4, "a rectangle of four sides came in as {} lines, {} arcs, {} splines", sk.lines, sk.arcs, sk.splines);
        let size = [sk.max[0] - sk.min[0], sk.max[1] - sk.min[1]];
        assert!((size[0] - 40.0).abs() < 0.01 && (size[1] - 30.0).abs() < 0.01, "a rectangle of 40 by 30 came in {size:?}");
    }
}

/// A glTF OF A 40 x 30 x 10 BOX WRITTEN THE WAY THE STANDARD ASKS: the scene in the text file and the numbers in a
/// `.bin` beside it, metres and +Y up. Written by hand, not by our own export - what comes out of another program
/// is a pair of files, not the single `.glb` we write.
fn a_gltf_beside_its_buffer(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("qymcad-gltf-{name}"));
    std::fs::create_dir_all(&dir).expect("the folder of the glTF is made");
    let corners = [[0.0, 0.0, 0.0], [40.0, 0.0, 0.0], [40.0, 30.0, 0.0], [0.0, 30.0, 0.0], [0.0, 0.0, 10.0], [40.0, 0.0, 10.0], [40.0, 30.0, 10.0], [0.0, 30.0, 10.0]];
    let mut bin: Vec<u8> = Vec::new();
    for p in corners {
        // ours (mm, +Z up) -> the standard's (m, +Y up)
        for v in [p[0] / 1000.0, p[2] / 1000.0, -p[1] / 1000.0] {
            bin.extend_from_slice(&(v as f32).to_le_bytes());
        }
    }
    let at = bin.len();
    let faces: [[u32; 3]; 12] = [[0, 3, 2], [0, 2, 1], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [3, 7, 6], [3, 6, 2], [0, 4, 7], [0, 7, 3], [1, 2, 6], [1, 6, 5]];
    for f in faces {
        for i in f {
            bin.extend_from_slice(&i.to_le_bytes());
        }
    }
    std::fs::write(dir.join("box.bin"), &bin).expect("the numbers of the glTF are written");
    let json = format!(
        concat!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"name":"Plate","mesh":0}}],"#,
            r#""meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"mode":4}}]}}],"#,
            r#""buffers":[{{"uri":"box.bin","byteLength":{whole}}}],"#,
            r#""bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{at},"target":34962}},"#,
            r#"{{"buffer":0,"byteOffset":{at},"byteLength":{rest},"target":34963}}],"#,
            r#""accessors":[{{"bufferView":0,"componentType":5126,"count":8,"type":"VEC3","min":[0.0,0.0,-0.03],"max":[0.04,0.01,0.0]}},"#,
            r#"{{"bufferView":1,"componentType":5125,"count":36,"type":"SCALAR"}}]}}"#
        ),
        whole = bin.len(),
        at = at,
        rest = bin.len() - at
    );
    let path = dir.join("box.gltf");
    std::fs::write(&path, json).expect("the glTF is written");
    path.display().to_string()
}

probe! {
    /// A glTF THAT KEEPS ITS NUMBERS IN A FILE BESIDE IT comes in whole and the right way up: 40 by 30 by 10 mm out
    /// of metres and +Y up.
    fn a_gltf_beside_its_buffer_comes_in() {
        let path = a_gltf_beside_its_buffer("a-plate");
        let (mut s, bodies) = bring_it_in(&path);
        assert!(bodies.len() == 1, "the glTF of one box came in as {} bodies; the program says {:?}", bodies.len(), s.status());
        let b = &bodies[0];
        let size = [b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]];
        assert!((size[0] - 40.0).abs() < 0.01 && (size[1] - 30.0).abs() < 0.01 && (size[2] - 10.0).abs() < 0.01, "a box of 40 by 30 by 10 came in {size:?}");
        assert!((b.volume - 12000.0).abs() < 1.0, "the box came in holding {} instead of 12000", b.volume);
    }
}

probe! {
    /// A FILE CUT OFF HALFWAY IS REFUSED IN WORDS, and nothing half-read is left in the document.
    fn a_truncated_file_is_refused() {
        let whole = a_path("whole", "step");
        let mut s = Session::start();
        build::block(&mut s);
        write_it_out(&mut s, "STEP\u{2026}", &whole);
        let bytes = std::fs::read(&whole).expect("the whole file is read back");
        let half = a_path("cut-off", "step");
        std::fs::write(&half, &bytes[..bytes.len() / 2]).expect("the cut-off file is written");
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let (file, import) = (s.word("menu-file"), s.word("file-import"));
        s.menu(&[&file, &import]);
        s.answer_file(&half);
        let said = s.status();
        assert!(!said.is_empty() && said != s.word("io-ready"), "nothing says the file was cut off: the program says {said:?}");
        assert!(s.document().bodies.is_empty(), "something came into the document out of half a file: {:?}", s.document().bodies.iter().map(|b| b.name.clone()).collect::<Vec<_>>());
    }
}

probe! {
    #[cfg(unix)]
    /// A FILE THAT MAY NOT BE READ IS REFUSED IN WORDS, not with silence.
    fn a_file_that_may_not_be_read_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let path = a_path("closed", "stl");
        let mut s = Session::start();
        build::block(&mut s);
        write_it_out(&mut s, "STL\u{2026}", &path);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("the file is closed for reading");
        let readable = std::fs::read(&path).is_ok();
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let (file, import) = (s.word("menu-file"), s.word("file-import"));
        s.menu(&[&file, &import]);
        s.answer_file(&path);
        let go = s.word("import-scale-import");
        if let Some(button) = s.find(&go, qymcad::pos2(1200.0, 700.0)) {
            s.click(button.center());
        }
        let said = s.status();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("the file is opened again");
        assert!(!readable, "the file closed for reading is still readable here, and the check would prove nothing");
        assert!(!said.is_empty() && said != s.word("io-ready"), "nothing says the file may not be read: the program says {said:?}");
        assert!(s.document().bodies.is_empty(), "something came into the document out of a file that may not be read: {:?}", s.document().bodies.iter().map(|b| b.name.clone()).collect::<Vec<_>>());
    }
}

probe! {
    /// A PATH WITH SPACES AND LETTERS OF ANOTHER ALPHABET is written and read like any other.
    fn a_path_with_spaces_and_other_letters_is_read() {
        let dir = std::env::temp_dir().join("\u{43f}\u{430}\u{43f}\u{43a}\u{430} \u{441} \u{43f}\u{440}\u{43e}\u{431}\u{435}\u{43b}\u{430}\u{43c}\u{438}");
        std::fs::create_dir_all(&dir).expect("the folder with spaces in its name is made");
        let path = dir.join("\u{434}\u{435}\u{442}\u{430}\u{43b}\u{44c} 1.stl").display().to_string();
        let mut s = Session::start();
        build::block(&mut s);
        write_it_out(&mut s, "STL\u{2026}", &path);
        let (mut s, bodies) = bring_it_in(&path);
        assert!(bodies.len() == 1, "the file at a path with spaces came in as {} bodies; the program says {:?}", bodies.len(), s.status());
        let b = &bodies[0];
        let size = [b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]];
        assert!((size[0] - 40.0).abs() < 0.01 && (size[1] - 30.0).abs() < 0.01 && (size[2] - 10.0).abs() < 0.01, "the block came back {size:?} instead of 40 by 30 by 10");
    }
}

probe! {
    /// THE SCALE ASKED FOR AT IMPORT IS HELD TO: the same file taken at twice its size is twice as wide.
    fn the_scale_at_import_is_held_to() {
        let path = a_path("at-twice-the-size", "stl");
        let mut s = Session::start();
        build::block(&mut s);
        write_it_out(&mut s, "STL\u{2026}", &path);
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let (file, import) = (s.word("menu-file"), s.word("file-import"));
        s.menu(&[&file, &import]);
        s.answer_file(&path);
        let factor = s.word("import-scale-factor");
        s.fill(&factor, "2");
        let go = s.word("import-scale-import");
        s.press_word_near(&go, qymcad::pos2(1200.0, 700.0));
        let body = s.document().bodies.into_iter().rfind(|b| !b.consumed && !b.sheet).expect("the body that came in");
        let across = body.max[0] - body.min[0];
        assert!((across - 80.0).abs() < 0.1, "a block of 40 taken at twice its size is 80 across, and it is {across}");
    }
}

/// THE CONVERTER OF AN F3D ON THE PATH, as a file: the probe below runs only where the program is installed. A file
/// on the path is the same look the door takes before it starts a reading; whether the file runs is what the reading
/// itself then says.
fn converter_on_path() -> bool {
    std::env::var_os("PATH").map(|search| std::env::split_paths(&search).any(|dir| dir.join("cadmpeg").is_file() || dir.join("cadmpeg.exe").is_file())).unwrap_or(false)
}

probe! {
    /// AN F3D COMES IN THROUGH ITS CONVERTER: the archive is turned into STEP outside and read the way every
    /// exact file is. Both the private sample and the cadmpeg program on the path are needed; either missing,
    /// the check says so and passes over. What came in is measured, not named: the sample is a real drawing of
    /// its own.
    fn an_f3d_comes_in_through_its_converter() {
        let Some(path) = qymcad_acceptance::private_sample("through-laser.f3d") else { return };
        if !converter_on_path() {
            eprintln!("PASSED OVER: the cadmpeg program is not on the path - the check on it runs where the converter is installed");
            return;
        }
        let (mut s, bodies) = bring_it_in(&path);
        assert!(!bodies.is_empty(), "the F3D came in as no bodies: the program says {:?}", s.status());
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for b in &bodies {
            for k in 0..3 {
                lo[k] = lo[k].min(b.min[k]);
                hi[k] = hi[k].max(b.max[k]);
            }
        }
        let span: [f64; 3] = std::array::from_fn(|k| hi[k] - lo[k]);
        assert!(span.iter().copied().fold(0.0, f64::max) > 0.1, "the F3D came in smaller than any real part: {span:?}");
    }
}
