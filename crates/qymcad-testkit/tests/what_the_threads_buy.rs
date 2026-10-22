//! WHAT THE THREADS BUY - a measurement, not a check: the rebuild of each example whole and of its blends alone, on
//! every core but one and on one thread, in one process. The numbers the speed of the kernel is held to start here.
use qymcad_core::feature::FeatureKind;
use qymcad_core::model::Project;
use qymcad_core::time_budget::{cost_of, Doer};

/// THE DOCUMENTS OF THE EXAMPLES.
const DOCUMENTS: [&str; 2] = [concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/QymBoxPc-Case.qcad"), concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/Filter-v2.qcad")];

/// How many times each rebuild is timed; the least is told, the machine being shared.
const TIMES: usize = 3;

/// WHAT OF A DOCUMENT IS REBUILT.
#[derive(Clone, Copy)]
enum Rebuilt {
    Whole,
    /// Its fillets and chamfers, on the bodies the rebuild before them left.
    Blends,
}

/// The seconds of work of the quickest of `TIMES` rebuilds of `p` (`time_budget::Cost`, the work on threads of its own:
/// the wall less the share the threads stood ready with no processor free).
fn quickest(p: &Project, what: Rebuilt) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..TIMES {
        let mut q = p.clone();
        let shapes = qymcad_testkit::restore_import_shapes(&q);
        let (_, shapes) = qymcad_testkit::regenerate_with_shapes(&mut q, shapes);
        let rebuilt = cost_of(Doer::Process, || {
            if let Rebuilt::Blends = what {
                for n in q.timeline.iter_mut().filter(|n| matches!(n.kind, FeatureKind::Fillet { .. } | FeatureKind::Chamfer { .. })) {
                    n.dirty = true;
                }
                let _ = qymcad_testkit::regenerate_dirty_with_shapes(&mut q, shapes);
            } else {
                let shapes = qymcad_testkit::restore_import_shapes(&q);
                let _ = qymcad_testkit::regenerate_with_shapes(&mut q, shapes);
            }
        });
        best = best.min(rebuilt.cost.work.as_secs_f64());
    }
    best
}

#[test]
#[ignore = "a measurement, not a check: prints numbers"]
fn what_the_threads_buy() {
    for doc in DOCUMENTS {
        let p = qymcad_io::load_project(doc).expect("the document opens");
        let blends = p.timeline.iter().filter(|n| matches!(n.kind, FeatureKind::Fillet { .. } | FeatureKind::Chamfer { .. })).count();
        qymcad_kernel::set_parallel(true, 0);
        let (whole_many, blends_many) = (quickest(&p, Rebuilt::Whole), quickest(&p, Rebuilt::Blends));
        qymcad_kernel::set_parallel(false, 1);
        let (whole_one, blends_one) = (quickest(&p, Rebuilt::Whole), quickest(&p, Rebuilt::Blends));
        qymcad_kernel::set_parallel(true, 0);
        let name = doc.rsplit('/').next().unwrap_or(doc);
        eprintln!(
            "MEASURED {name}: whole {whole_many:.2} s on {} threads, {whole_one:.2} s on one ({:.2}x); its {blends} blends {blends_many:.2} s against {blends_one:.2} s ({:.2}x)",
            qymcad_kernel::workers().max(1),
            whole_one / whole_many,
            blends_one / blends_many
        );
    }
}
