//! Error-free `f64` expansion arithmetic: two-term sums and products, and an exact
//! `p * q - r * s` of two-term operands.

/// A group of operands is scaled down by a power of two when any exceeds this magnitude (see
/// [`group_scale`]), so products of scaled operands, or of their differences, stay finite.
const MAX_UNSCALED: f64 = 3.273_390_607_896_142e150; // 2^500

/// `a + b` as an unevaluated sum `(s, e)` with `s + e == a + b` exactly (Knuth's TwoSum).
pub(crate) fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    (s, (a - (s - bb)) + (b - bb))
}

/// `a * b` as an unevaluated sum `(p, e)` with `p + e == a * b` (fused multiply-add), and
/// whether that is exact. The residual `e` is exact unless it underflows, which needs the
/// operands' binary exponents to sum below about -970; such products are reported inexact.
fn two_product(a: f64, b: f64) -> ((f64, f64), bool) {
    let p = a * b;
    let exact = a == 0.0 || b == 0.0 || binary_exponent(a) + binary_exponent(b) >= -970;
    ((p, a.mul_add(b, -p)), exact)
}

/// The unbiased binary exponent of `v` (-1023 for zero and subnormals).
fn binary_exponent(v: f64) -> i32 {
    ((v.to_bits() >> 52) & 0x7ff) as i32 - 1023
}

/// `p * q - r * s` for exact two-term sums `p`, `q`, `r`, `s`: the 16 partial products are
/// accumulated without error into a nonoverlapping expansion (Shewchuk's Grow-Expansion), which
/// is then summed from its smallest component up. `None` when a partial product's residual
/// underflowed (see [`two_product`]), so the expansion would not be exact.
///
/// The final `sum()` is faithful (within about 1 ulp of the exact value), not guaranteed to be
/// correctly rounded; for example `p = (1, 2^-53)`, `q = (1, 2^-107)`, `r = s = 0` gives 1
/// instead of `1 + 2^-52`. That is far below plot resolution.
pub(crate) fn cross_difference(
    p: (f64, f64),
    q: (f64, f64),
    r: (f64, f64),
    s: (f64, f64),
) -> Option<f64> {
    let mut expansion: Vec<f64> = Vec::with_capacity(32);
    let mut grow = |v: f64| {
        let mut carry = v;
        let mut next = Vec::with_capacity(expansion.len() + 1);
        for &e in &expansion {
            let (sum, err) = two_sum(carry, e);
            if err != 0.0 {
                next.push(err);
            }
            carry = sum;
        }
        if carry != 0.0 {
            next.push(carry);
        }
        expansion = next;
    };
    for (x, y, sign) in [(p, q, 1.0), (r, s, -1.0)] {
        for u in [x.0, x.1] {
            for v in [y.0, y.1] {
                let ((hi, lo), exact) = two_product(u, v);
                if !exact {
                    return None;
                }
                grow(sign * hi);
                grow(sign * lo);
            }
        }
    }
    Some(expansion.iter().sum())
}

/// A power of two that brings `largest` (a finite magnitude) to within `2^-500..=2^500`, or 1
/// when it already is (or is 0).
pub(crate) fn group_scale(largest: f64) -> f64 {
    let e = binary_exponent(largest);
    let k = if largest > MAX_UNSCALED {
        e - 499
    } else if largest != 0.0 && e < -500 {
        e + 500
    } else {
        0
    };
    // `k` is within -575..=524, so `2^-k` is a normal `f64`.
    f64::from_bits(((1023 - k) as u64) << 52)
}
