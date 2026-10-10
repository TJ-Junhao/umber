//! Exact arithmetic on scalar `f64` inputs, for computations whose result must not depend on
//! the operands' magnitudes, such as line crossings far outside the clip rectangle.
//!
//! Two representations are used, in order of cost:
//! - [`expansion`]: error-free `f64` arithmetic, with values held as unevaluated sums of
//!   `f64` terms. Fast, but exact only while no product residual underflows.
//! - [`dyadic`]: exact binary fractions on a small big integer, for the operands whose
//!   magnitudes are too far apart for the expansions.
//!
//! The functions here take plain numbers and know nothing about geometry; callers decide which
//! quantity to evaluate and when to fall back from one representation to the other.

mod dyadic;
mod expansion;

pub(crate) use dyadic::Dyadic;
pub(crate) use expansion::{cross_difference, group_scale, two_sum};
