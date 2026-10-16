//! A SKETCH SOLVED BY PARTS: its constraints split into parts that do not touch, solved side by side.

use super::*;

/// The size of the whole sketch, which the thresholds of a solve scale with. Taken from the whole sketch even when
/// one part is solved, so a part is solved to the same thresholds as it was inside the whole.
#[derive(Clone, Copy)]
pub(crate) struct Scale {
    /// the largest coordinate, at least 1: the residual below `1e-4 * span` counts as solved
    pub(crate) span: f64,
    /// the spread of all coordinates: an axis dimension off by more than `1e-4` of it is a candidate for a side flip
    pub(crate) extent: f64,
}

impl Scale {
    pub(crate) fn of(points: &[SketchPoint]) -> Scale {
        let span = points.iter().fold(0.0_f64, |m, p| m.max(p.x.abs()).max(p.y.abs())).max(1.0);
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for p in points {
            lo = lo.min(p.x).min(p.y);
            hi = hi.max(p.x).max(p.y);
        }
        Scale { span, extent: if hi > lo { hi - lo } else { 0.0 } }
    }
}

/// How the system of a step is solved.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Algebra {
    /// Gauss-Jordan on the dense normal matrix, whatever the size
    Dense,
    /// the sparse Cholesky of the normal matrix, whatever the size
    Sparse,
    /// dense up to `DENSE_UP_TO` free unknowns, sparse past it
    BySize,
}

/// Up to this many free unknowns a step is solved dense. A sketch part of a few shapes - nearly every part there is -
/// is solved as it always was; the dense matrix of 8 000 unknowns (an array of 1 000 rectangles) is 512 MB and
/// its Gauss-Jordan 5e11 operations a step.
pub(crate) const DENSE_UP_TO: usize = 200;

/// What a solve keeps through all of its runs: the size it scales with, its algebra and the moment its time is out.
#[derive(Clone, Copy)]
pub(crate) struct Solve {
    pub(crate) scale: Scale,
    pub(crate) algebra: Algebra,
    /// past this no run takes another step, and no part is begun
    pub(crate) until: Option<std::time::Instant>,
    pub(crate) sizes: Sizes,
}

/// A part of a sketch that no constraint ties to the rest: its points, the radii of its circles and its constraints,
/// by their places in the sketch's lists.
pub(crate) struct Part {
    pub(crate) points: Vec<usize>,
    pub(crate) radii: Vec<usize>,
    pub(crate) constraints: Vec<usize>,
}

/// The points, radii and constraints of one part, copied out of the sketch's lists.
pub(crate) struct Own {
    pub(crate) points: Vec<SketchPoint>,
    pub(crate) radii: Vec<RadiusVar>,
    pub(crate) constraints: Vec<Constraint>,
}

impl Part {
    pub(crate) fn own(&self, points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> Own {
        Own {
            points: self.points.iter().map(|&i| points[i]).collect(),
            radii: self.radii.iter().map(|&j| radii[j]).collect(),
            constraints: self.constraints.iter().map(|&ci| constraints[ci].clone()).collect(),
        }
    }
}

/// The parts of a sketch: the points a constraint names are of one part, and a radius is of the part of its centre.
/// A constraint that names no point of the sketch is of no part - the solve leaves it out anyway (`cons_ok`).
pub(crate) fn parts(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> Vec<Part> {
    let idx: HashMap<Id, usize> = points.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let mut up: Vec<usize> = (0..points.len()).collect();
    fn root(up: &mut [usize], mut i: usize) -> usize {
        while up[i] != i {
            up[i] = up[up[i]];
            i = up[i];
        }
        i
    }
    let mut first_of: Vec<Option<usize>> = Vec::with_capacity(constraints.len());
    for c in constraints {
        let mut first = None;
        for id in c.points() {
            let Some(&i) = idx.get(&id) else { continue };
            match first {
                None => first = Some(i),
                Some(f) => {
                    let (ra, rb) = (root(&mut up, f), root(&mut up, i));
                    if ra != rb {
                        up[ra.max(rb)] = ra.min(rb);
                    }
                }
            }
        }
        first_of.push(first);
    }
    let mut part_of_root: HashMap<usize, usize> = HashMap::new();
    let mut out: Vec<Part> = Vec::new();
    let mut part_at = |up: &mut [usize], i: usize, out: &mut Vec<Part>| -> usize {
        let r = root(up, i);
        *part_of_root.entry(r).or_insert_with(|| {
            out.push(Part { points: Vec::new(), radii: Vec::new(), constraints: Vec::new() });
            out.len() - 1
        })
    };
    for i in 0..points.len() {
        let k = part_at(&mut up, i, &mut out);
        out[k].points.push(i);
    }
    for (j, rv) in radii.iter().enumerate() {
        if let Some(&i) = idx.get(&rv.center) {
            let k = part_at(&mut up, i, &mut out);
            out[k].radii.push(j);
        }
    }
    for (ci, first) in first_of.into_iter().enumerate() {
        if let Some(i) = first {
            let k = part_at(&mut up, i, &mut out);
            out[k].constraints.push(ci);
        }
    }
    out
}

/// Solve every part of the sketch as a system of its own. 300 separate lines are 300 systems of 4 unknowns, not one
/// of 1 200: the dense algebra of a solve costs the cube of its unknowns. A part with no constraint is not solved. A
/// drag solves only the part of the dragged point - nothing ties the others to it, and the release solves them all.
pub(crate) fn solve_by_parts(points: &mut [SketchPoint], radii: &mut [RadiusVar], constraints: &[Constraint], drag: Option<(Id, f64, f64)>, max_iter: usize, how: Solve) -> Outcome {
    let drag = drag.filter(|(id, _, _)| points.iter().any(|p| p.id == *id));
    // the parts to solve, each with the drag if it holds the dragged point
    let work: Vec<ToSolve> = parts(points, radii, constraints)
        .into_iter()
        .map(|part| {
            let drag = drag.filter(|(id, _, _)| part.points.iter().any(|&i| points[i].id == *id));
            ToSolve { part, drag }
        })
        .filter(|w| !(w.part.constraints.is_empty() && w.drag.is_none() || drag.is_some() && w.drag.is_none()))
        .collect();
    let (shared_points, shared_radii): (&[SketchPoint], &[RadiusVar]) = (points, radii);
    let out_of_time = || how.until.is_some_and(|t| std::time::Instant::now() >= t);
    let solved = each_part(&work, |w| {
        if out_of_time() {
            return None; // the time is out: the part is left as it stood
        }
        let mut own = w.part.own(shared_points, shared_radii, constraints);
        // each part guarded on its own: a part that came out a non-number is put back, the others keep their answers
        let residual = guarded(&mut own.points, &mut own.radii, |points, radii| solve_full_iter_inner(points, radii, &own.constraints, w.drag, max_iter, how));
        // a part the time ran out in the middle of keeps the best it reached, and is counted as not solved to the end
        Some(Solved { own, residual, cut: out_of_time() })
    });
    // put back and summed in the order of the parts, whatever thread solved which
    let (mut sum_sq, mut left) = (0.0, 0);
    for (w, solved) in work.iter().zip(solved) {
        let Some(Solved { own, residual, cut }) = solved else {
            left += 1;
            let own = w.part.own(points, radii, constraints);
            sum_sq += residual_per_constraint(&own.points, &own.radii, &own.constraints).iter().map(|v| v * v).sum::<f64>();
            continue;
        };
        sum_sq += residual * residual;
        left += usize::from(cut);
        for (&i, q) in w.part.points.iter().zip(own.points) {
            points[i] = q;
        }
        for (&j, rv) in w.part.radii.iter().zip(own.radii) {
            radii[j] = rv;
        }
    }
    Outcome { residual: sum_sq.sqrt(), left }
}

/// A part to solve, with the drag if it holds the dragged point.
pub(crate) struct ToSolve {
    pub(crate) part: Part,
    pub(crate) drag: Option<(Id, f64, f64)>,
}

/// A part solved: its points and radii as they came out, what is left of its constraints, and whether the time ran out
/// while it was solved.
pub(crate) struct Solved {
    pub(crate) own: Own,
    pub(crate) residual: f64,
    pub(crate) cut: bool,
}

/// Fewer parts than this are solved on the calling thread: a thread costs more to start than a few parts to solve.
pub(crate) const PARTS_FOR_A_THREAD: usize = 256;

/// `solve` over every item of `work`, the answers in the order of `work`. Spread over `threads()` threads in runs of
/// items one after another, where there are enough items for each thread to take `PARTS_FOR_A_THREAD`; each item is
/// solved alone, so the answers do not depend on how many threads there are.
pub(crate) fn each_part<W: Sync, R: Send>(work: &[W], solve: impl Fn(&W) -> R + Sync) -> Vec<R> {
    let threads = threads().min(work.len() / PARTS_FOR_A_THREAD).max(1);
    if threads == 1 {
        return work.iter().map(&solve).collect();
    }
    let run = work.len().div_ceil(threads);
    std::thread::scope(|scope| {
        let handles: Vec<_> = work.chunks(run).map(|chunk| scope.spawn(|| chunk.iter().map(&solve).collect::<Vec<R>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_else(|e| std::panic::resume_unwind(e))).collect()
    })
}

/// HOW MANY THREADS A SOLVE MAY SPREAD ITS PARTS OVER, as said by `set_threads`; zero, the start, is all the cores but
/// one.
pub(crate) static THREADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// SAY HOW MANY THREADS A SOLVE MAY TAKE: the setting of the program for the cores of a computation, handed over with
/// the kernel's own (`qymcad_kernel::set_parallel`) - one number for all of it. One is a single thread.
pub fn set_threads(n: usize) {
    THREADS.store(n, std::sync::atomic::Ordering::Relaxed);
}

/// How many threads a solve takes now (`set_threads`).
pub fn threads() -> usize {
    match THREADS.load(std::sync::atomic::Ordering::Relaxed) {
        0 => std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).saturating_sub(1).max(1),
        n => n,
    }
}
