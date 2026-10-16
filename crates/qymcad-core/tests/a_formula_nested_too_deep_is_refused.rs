//! A FORMULA NESTED TOO DEEP IS REFUSED, NOT LEFT TO RUN THE STACK OUT. The parser goes down one call per level of
//! nesting, and nothing stopped it: a formula stored in a document with a few thousand parentheses inside one
//! another ended the program as soon as it was evaluated - by the parameters window, an undo, a sketch solve or a
//! rebuild. A binary of its own: before the limit, these checks ended the process, not a test.
use qymcad_core::errors::ExprError;
use qymcad_core::expr::{eval, MAX_FORMULA_DEPTH};
use std::collections::HashMap;

/// `open` + `core` + `close`, `levels` times over.
fn nested(open: &str, core: &str, close: &str, levels: usize) -> String {
    format!("{}{core}{}", open.repeat(levels), close.repeat(levels))
}

/// Run on a thread with a 256 KiB stack: a quarter of the smallest a formula is evaluated on (the 1 MiB main thread
/// on Windows; a spawned thread gets 2 MiB), so the limit holds with the rest of that stack already in use - the
/// parameters window evaluates its formulas deep inside the frame's drawing. A stack run out aborts the whole binary
/// rather than failing here.
fn on_small_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new().stack_size(256 << 10).spawn(work).expect("a thread").join().expect("the work panicked")
}

fn eval_small_stack(src: String) -> Result<f64, ExprError> {
    on_small_stack(move || eval(&src, &HashMap::new()))
}

#[test]
fn a_formula_at_the_limit_is_read_and_one_past_it_is_refused() {
    assert_eq!(eval_small_stack(nested("(", "1", ")", MAX_FORMULA_DEPTH)), Ok(1.0));
    assert_eq!(eval_small_stack(nested("abs(", "1", ")", MAX_FORMULA_DEPTH)), Ok(1.0));
    assert_eq!(eval_small_stack(nested("(", "1", ")", MAX_FORMULA_DEPTH + 1)), Err(ExprError::TooDeep { limit: MAX_FORMULA_DEPTH }));
}

/// The depth is a depth, not a length: a long formula that does not nest is read as before.
#[test]
fn a_long_flat_formula_is_not_counted_as_deep() {
    assert_eq!(eval_small_stack(format!("{}1", "1+".repeat(100_000))), Ok(100_001.0));
}

/// A way a formula nests, and a formula nested that way.
struct Nesting {
    what: &'static str,
    src: String,
}

/// Every way a formula nests is counted, and each is refused well before the stack would be: 20,000 levels, past
/// where 2 MiB ran out (about 4,375 parentheses).
#[test]
fn every_way_of_nesting_is_refused_before_the_stack_runs_out() {
    let deep = 20_000;
    for Nesting { what, src } in [
        Nesting { what: "parentheses", src: nested("(", "1", ")", deep) },
        Nesting { what: "function calls", src: nested("abs(", "1", ")", deep) },
        Nesting { what: "signs", src: format!("{}1", "-".repeat(deep)) },
        Nesting { what: "powers", src: format!("{}1", "1^".repeat(deep)) },
    ] {
        assert_eq!(eval_small_stack(src), Err(ExprError::TooDeep { limit: MAX_FORMULA_DEPTH }), "{deep} levels of {what}");
    }
}

/// A document's parameter holding such a formula: evaluating the parameters says what is wrong with it.
#[test]
fn a_stored_parameter_nested_too_deep_is_named_not_fatal() {
    let mut p = qymcad_core::model::Project::default();
    p.new_document();
    p.parameters.push(qymcad_core::model::Param { name: "w".into(), expr: nested("(", "1", ")", 20_000), ..Default::default() });
    let errs = on_small_stack(move || p.eval_parameters());
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert_eq!(errs[0].1, ExprError::TooDeep { limit: MAX_FORMULA_DEPTH });
}
