//! IGES: WRITTEN AND READ BACK, the same bodies.
//!
//! A check that only sees the writer succeed is green and blind, so every case goes out and comes back and is
//! measured: how many solids, their volumes, where they stand.
use qymcad_core::feature::PLACE_IDENTITY;
use crate::{iges_solids, import_iges, write_iges, LengthUnit, Shape};

/// A folder for the files, under `target`: nothing heavy goes to `/tmp`, which lives in memory.
fn file(name: &str) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/iges-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    dir.join(name).to_string_lossy().into_owned()
}

fn moved(dx: f64, dy: f64) -> [f64; 12] {
    let mut m = PLACE_IDENTITY;
    m[3] = dx;
    m[7] = dy;
    m
}

/// Two cuboids and a cylinder, each with its own place: flat faces and a curved one.
fn three_bodies() -> (Shape, Shape, Shape) {
    let sq = [0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0];
    (Shape::extrude(&sq, 10.0).expect("cuboid A"), Shape::extrude(&sq, 10.0).expect("cuboid B"), Shape::cylinder(5.0, 20.0).expect("cylinder"))
}

/// The volumes, smallest first, and the box around everything.
fn measure(solids: &[Shape]) -> (Vec<f64>, [f64; 6]) {
    let mut v: Vec<f64> = solids.iter().map(|s| s.volume()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).expect("a volume is a number"));
    let mut bb = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
    for s in solids {
        let b = s.bbox().expect("a solid has a box");
        for k in 0..3 {
            bb[k] = bb[k].min(b[k]);
            bb[k + 3] = bb[k + 3].max(b[k + 3]);
        }
    }
    (v, bb)
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * b.abs().max(1.0)
}

/// THREE BODIES GO OUT AND THREE SOLIDS COME BACK, with their volumes and their places.
///
/// The file holds trimmed surfaces, not solids, so coming back as solids at all is the sewing's work: without
/// it the reader hands over one bag of eighteen faces.
#[test]
fn three_bodies_come_back_as_three_solids_in_their_places() {
    let (a, b, c) = three_bodies();
    let p = file("three.igs");
    write_iges(&[(&a, PLACE_IDENTITY), (&b, moved(30.0, 0.0)), (&c, moved(0.0, 30.0))], &p, LengthUnit::Millimetre).expect("the IGES is written");
    let back = iges_solids(&p).expect("the IGES reads back");
    assert_eq!(back.len(), 3, "three bodies went out, {} came back", back.len());
    for s in &back {
        assert_eq!(s.solid_count(), 1, "a body came back as {} solids - its faces were not closed into one", s.solid_count());
        assert!(s.is_valid(), "a body came back broken");
    }
    let (v, _) = measure(&back);
    let cyl = std::f64::consts::PI * 25.0 * 20.0;
    for (got, want) in v.iter().zip([1000.0, 1000.0, cyl]) {
        assert!(close(*got, want, 5e-3), "the volumes came back as {v:?}, not [1000, 1000, {cyl:.1}]");
    }
    // WHERE EACH ONE STANDS. The cuboids are flat, and their boxes are exact. The cylinder's box is not: a box
    // around a curved surface read back from IGES is taken from the surface's control net and overshoots it,
    // unevenly (measured on a cylinder of radius 5 at x = 0: from -5.30 to 5.00), while the volume matched to
    // 0.5 % - a real bulge of 0.3 mm would add 12 %. So here the cylinder is checked by its height and by a box
    // that may be larger than it, never smaller; where its surface lies is measured on the mesh, below.
    let near = |a: f64, b: f64| (a - b).abs() < 1e-2;
    let mut cuboids = Vec::new();
    for s in &back {
        let b = s.bbox().expect("a solid has a box");
        if close(s.volume(), cyl, 5e-3) {
            assert!(near(b[5] - b[2], 20.0), "the cylinder came back {} tall", b[5] - b[2]);
            assert!(b[3] - b[0] >= 10.0 - 1e-2 && b[4] - b[1] >= 10.0 - 1e-2, "the cylinder came back narrower than its diameter: {b:?}");
        } else {
            cuboids.push(b);
        }
    }
    cuboids.sort_by(|a, b| a[0].partial_cmp(&b[0]).expect("a coordinate is a number"));
    for (b, want) in cuboids.iter().zip([[0.0, 0.0, 0.0, 10.0, 10.0, 10.0], [30.0, 0.0, 0.0, 40.0, 10.0, 10.0]]) {
        assert!((0..6).all(|k| near(b[k], want[k])), "a cuboid came back at {b:?}, not {want:?}");
    }
}

/// A FILE IN INCHES COMES BACK THE SAME SIZE, and it really is in inches.
///
/// Half of what goes wrong between programs is a model arriving 25.4 times too large or too small. The file
/// is checked to declare inches (so the check is not of a millimetre file under another name), and the solid
/// read from it is measured.
#[test]
fn a_file_in_inches_comes_back_the_same_size() {
    let (a, _, _) = three_bodies();
    let p = file("inches.igs");
    write_iges(&[(&a, PLACE_IDENTITY)], &p, LengthUnit::Inch).expect("the IGES is written");
    let text = std::fs::read_to_string(&p).expect("the file is text");
    let global: String = text.lines().filter(|l| l.len() >= 73 && &l[72..73] == "G").map(|l| &l[..72]).collect();
    assert!(global.contains("INCH"), "the file does not say it is in inches; its global section: {global}");
    let back = iges_solids(&p).expect("the IGES reads back");
    let (v, bb) = measure(&back);
    assert!(close(v[0], 1000.0, 5e-3), "a 10 mm cube came back with a volume of {} mm^3", v[0]);
    assert!((bb[3] - 10.0).abs() < 1e-2, "a 10 mm cube came back {} mm wide", bb[3]);
}

/// THE MESH SIDE READS THE SAME FILE: a body per solid, with triangles and faces, where it was written.
///
/// The vertices of a mesh lie ON the surface, so their bounds are the one honest measure of where a curved body
/// stands: they can fall short of the surface by the sag of a chord, never beyond it. The sag is under 0.06 mm
/// here (radius 5, at most 0.3 rad between neighbouring vertices).
#[test]
fn the_same_file_reads_as_meshes_too() {
    let (a, b, c) = three_bodies();
    let p = file("meshes.igs");
    write_iges(&[(&a, PLACE_IDENTITY), (&b, moved(30.0, 0.0)), (&c, moved(0.0, 30.0))], &p, LengthUnit::Millimetre).expect("the IGES is written");
    let bodies = import_iges(&p, 0.5).expect("the IGES imports");
    assert_eq!(bodies.len(), 3, "a body per solid, in step with the live shapes: got {}", bodies.len());
    for qymcad_core::geom::Built { mesh: m, faces } in &bodies {
        assert!(m.tris.len() >= 12 && !faces.is_empty(), "a body came in with {} triangles and {} faces", m.tris.len(), faces.len());
    }
    let mut boxes: Vec<[f64; 6]> = bodies
        .iter()
        .map(|qymcad_core::geom::Built { mesh: m, .. }| {
            let b = m.bounds().expect("a mesh has bounds");
            [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z]
        })
        .collect();
    boxes.sort_by(|a, b| (a[0], a[1]).partial_cmp(&(b[0], b[1])).expect("coordinates are numbers"));
    let want = [[-5.0, 25.0, 0.0, 5.0, 35.0, 20.0], [0.0, 0.0, 0.0, 10.0, 10.0, 10.0], [30.0, 0.0, 0.0, 40.0, 10.0, 10.0]];
    for (b, w) in boxes.iter().zip(want) {
        assert!((0..6).all(|k| (b[k] - w[k]).abs() < 0.1), "a body came in at {b:?}, not {w:?}");
    }
}

/// SEVERAL THREADS AT ONCE, and the program stays up.
///
/// An import runs on a worker thread, so two files can be read at the same moment - and the IGES side of
/// OCCT keeps its state in process-wide statics. Without a lock this died with SIGSEGV.
#[test]
fn several_threads_read_and_write_iges_at_once() {
    let (a, b, c) = three_bodies();
    let p = file("threads.igs");
    write_iges(&[(&a, PLACE_IDENTITY), (&b, moved(30.0, 0.0)), (&c, moved(0.0, 30.0))], &p, LengthUnit::Millimetre).expect("the IGES is written");
    let counts: Vec<usize> = std::thread::scope(|scope| {
        let mut running = Vec::new();
        for k in 0..4 {
            let p = p.clone();
            running.push(scope.spawn(move || {
                // every other thread writes a file of its own while the rest read
                if k % 2 == 1 {
                    let cube = Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("a cube");
                    write_iges(&[(&cube, PLACE_IDENTITY)], &file(&format!("thread-{k}.igs")), LengthUnit::Millimetre).expect("written");
                }
                iges_solids(&p).map(|v| v.len()).unwrap_or(0)
            }));
        }
        running.into_iter().map(|h| h.join().expect("a thread came back")).collect()
    });
    assert!(counts.iter().all(|n| *n == 3), "reading at once gave {counts:?} solids, not three each");
}

/// A FILE THAT IS NOT IGES IS REFUSED, and the program stays up.
#[test]
fn a_file_that_is_not_iges_is_refused() {
    let p = file("not-iges.igs");
    std::fs::write(&p, "this is not an IGES file\n").expect("written");
    assert!(iges_solids(&p).is_err() || iges_solids(&p).is_ok_and(|v| v.is_empty()), "rubbish was read as a shape");
    assert!(import_iges(&p, 0.5).is_err(), "rubbish was read as bodies");
}

/// ONE READING GIVES THE BODIES AND THE LIVE SOLIDS TOGETHER, as many of each and in the same order.
///
/// The door pairs them by position: the first body with the first live solid. Reading the file twice used to
/// guarantee the pairing by reading the same way twice; one reading has to guarantee it itself.
#[test]
fn one_reading_gives_bodies_and_solids_in_step() {
    let (a, b, c) = three_bodies();
    let p = file("once.igs");
    write_iges(&[(&a, PLACE_IDENTITY), (&b, moved(30.0, 0.0)), (&c, moved(0.0, 30.0))], &p, LengthUnit::Millimetre).expect("written");
    let step = file("once.step");
    crate::write_step(&[(&a, PLACE_IDENTITY), (&b, moved(30.0, 0.0)), (&c, moved(0.0, 30.0))], &step).expect("written");
    for (format, path) in [(crate::ExactFormat::Iges, &p), (crate::ExactFormat::Step, &step)] {
        let (bodies, shapes) = crate::read_exact(format, path, 0.5).expect("reads");
        assert_eq!(bodies.len(), 3, "{format:?}: {} bodies", bodies.len());
        assert_eq!(shapes.len(), bodies.len(), "{format:?}: {} bodies but {} live solids", bodies.len(), shapes.len());
        for (k, (qymcad_core::geom::Built { mesh, .. }, shape)) in bodies.iter().zip(&shapes).enumerate() {
            let mb = mesh.bounds().expect("a mesh has bounds");
            let sb = shape.bbox().expect("a solid has a box");
            // the mesh lies inside the solid's box, which may only be looser than it
            assert!(mb.min.x >= sb[0] - 1e-3 && mb.max.x <= sb[3] + 1e-3 && mb.min.y >= sb[1] - 1e-3 && mb.max.y <= sb[4] + 1e-3, "{format:?}: body {k} and live solid {k} are different parts");
        }
    }
}
