//! THE STEP BACK OF AN OPERATION WITH A SIZE.
//!
//! OCCT fails exactly at the boundary of degeneracy. A blend whose edge lands exactly on another edge or face - a
//! chamfer of 4 on an edge 4 from a step, a fillet of a radius equal to half a 10 mm cube - is refused, while a
//! value a hair smaller builds. A person drawing to a dimension lands there on purpose, so the limiting case is
//! retried stepped back by an amount invisible in the geometry, the way mature systems build it.

/// HOW FAR AN OPERATION OF A KIND STEPS BACK, measured per kind: the first step that builds is taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retreat {
    /// Fillets and chamfers. A fillet of R5 on a 10 mm cube took at 1 - 2e-8 (4.9999999). A chamfer needs the
    /// coarser step: a leg of 1.99999999 failed where 1.99998 worked, so 1 - 1e-5.
    Blend,
}

impl Retreat {
    /// The factors on the size after the asked one, finest first.
    pub(crate) fn steps(self) -> &'static [f64] {
        match self {
            Retreat::Blend => &[1.0 - 2e-8, 1.0 - 1e-5],
        }
    }
}

/// RUN `build` AT THE ASKED SIZE, and on a refusal stepped back. `build` takes the factor on its size (1.0 first) and
/// scales every length of the operation by it - both legs of a chamfer, every radius of a variable fillet - and
/// nothing else: an angle stays an angle.
///
/// The fast path costs nothing: the steps run only after a refusal.
pub fn sized<T>(retreat: Retreat, build: impl Fn(f64) -> Option<T>) -> Option<T> {
    std::iter::once(1.0).chain(retreat.steps().iter().copied()).find_map(build)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A build refusing exactly at its size and taking anything below it: the step back builds it.
    #[test]
    fn the_limiting_case_is_built_stepped_back() {
        let asked = 4.0;
        let got = sized(Retreat::Blend, |f| (asked * f < asked).then_some(asked * f));
        assert!(got.is_some_and(|v| asked - v < 1e-6), "the limiting case is not built a hair short of the size: {got:?}");
    }

    /// A build that takes at once is asked once: the fast path does not pay for the steps.
    #[test]
    fn a_build_that_takes_is_asked_once() {
        let asked = std::cell::Cell::new(0);
        let got = sized(Retreat::Blend, |f| {
            asked.set(asked.get() + 1);
            Some(f)
        });
        assert_eq!((got, asked.get()), (Some(1.0), 1), "the fast path ran the steps back");
    }
}
