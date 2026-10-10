//! Exact binary-fraction arithmetic for the rare line crossings whose floating-point evaluation
//! cannot be exact (operands whose magnitudes differ by more than the `f64` exponent range
//! allows in a product). Every finite `f64` is a binary fraction `m * 2^e`; sums, differences
//! and products of such values are again binary fractions, held here with a small big integer.

use std::cmp::Ordering;

/// A signed big integer: little-endian 64-bit limbs without trailing zero limbs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Big {
    neg: bool,
    mag: Vec<u64>,
}

fn trim(mut v: Vec<u64>) -> Vec<u64> {
    while v.last() == Some(&0) {
        v.pop();
    }
    v
}

fn cmp_mag(a: &[u64], b: &[u64]) -> Ordering {
    a.len()
        .cmp(&b.len())
        .then_with(|| a.iter().rev().cmp(b.iter().rev()))
}

fn add_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
    let n = a.len().max(b.len());
    let mut out = Vec::with_capacity(n + 1);
    let mut carry = 0_u128;
    for i in 0..n {
        let s = u128::from(a.get(i).copied().unwrap_or(0))
            + u128::from(b.get(i).copied().unwrap_or(0))
            + carry;
        out.push(s as u64);
        carry = s >> 64;
    }
    out.push(carry as u64);
    trim(out)
}

/// `a - b` for `|a| >= |b|`.
fn sub_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = false;
    for (i, &x) in a.iter().enumerate() {
        let y = b.get(i).copied().unwrap_or(0);
        let (d, b1) = x.overflowing_sub(y);
        let (d, b2) = d.overflowing_sub(u64::from(borrow));
        out.push(d);
        borrow = b1 || b2;
    }
    trim(out)
}

fn mul_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut out = vec![0_u64; a.len() + b.len()];
    for (i, &x) in a.iter().enumerate() {
        let mut carry = 0_u128;
        for (j, &y) in b.iter().enumerate() {
            if let Some(slot) = out.get_mut(i + j) {
                let t = u128::from(x) * u128::from(y) + u128::from(*slot) + carry;
                *slot = t as u64;
                carry = t >> 64;
            }
        }
        if let Some(slot) = out.get_mut(i + b.len()) {
            *slot = carry as u64;
        }
    }
    trim(out)
}

fn shl_mag(a: &[u64], bits: u32) -> Vec<u64> {
    if a.is_empty() {
        return Vec::new();
    }
    let (limbs, rem) = ((bits / 64) as usize, bits % 64);
    let mut out = vec![0_u64; limbs];
    let mut carry = 0_u64;
    for &x in a {
        if rem == 0 {
            out.push(x);
        } else {
            out.push((x << rem) | carry);
            carry = x >> (64 - rem);
        }
    }
    out.push(carry);
    trim(out)
}

impl Big {
    fn new(neg: bool, mag: Vec<u64>) -> Big {
        let mag = trim(mag);
        Big {
            neg: neg && !mag.is_empty(),
            mag,
        }
    }

    fn add(&self, o: &Big) -> Big {
        if self.neg == o.neg {
            return Big::new(self.neg, add_mag(&self.mag, &o.mag));
        }
        match cmp_mag(&self.mag, &o.mag) {
            Ordering::Less => Big::new(o.neg, sub_mag(&o.mag, &self.mag)),
            _ => Big::new(self.neg, sub_mag(&self.mag, &o.mag)),
        }
    }

    fn mul(&self, o: &Big) -> Big {
        Big::new(self.neg != o.neg, mul_mag(&self.mag, &o.mag))
    }

    fn shl(&self, bits: u32) -> Big {
        Big::new(self.neg, shl_mag(&self.mag, bits))
    }

    /// `|self|` as `(m, e)` with `|self| = m * 2^e` to within a relative `2^-53`, `m` in
    /// `[2^63, 2^64)` (or 0).
    fn top_bits(&self) -> (f64, i64) {
        let Some(&top) = self.mag.last() else {
            return (0.0, 0);
        };
        let n = self.mag.len();
        let below = n
            .checked_sub(2)
            .and_then(|i| self.mag.get(i))
            .copied()
            .unwrap_or(0);
        let shift = top.leading_zeros();
        let bits = if shift == 0 {
            top
        } else {
            (top << shift) | (below >> (64 - shift))
        };
        (bits as f64, 64 * (n as i64 - 1) - i64::from(shift))
    }
}

/// An exact binary fraction `m * 2^e`.
#[derive(Debug, Clone)]
pub(crate) struct Dyadic {
    m: Big,
    e: i32,
}

impl Dyadic {
    /// The exact value of a finite `v`; non-finite values give 0.
    pub(crate) fn from_f64(v: f64) -> Dyadic {
        if v == 0.0 || !v.is_finite() {
            return Dyadic {
                m: Big::new(false, Vec::new()),
                e: 0,
            };
        }
        let bits = v.to_bits();
        let exp = ((bits >> 52) & 0x7ff) as i32;
        let frac = bits & ((1_u64 << 52) - 1);
        let (m, e) = if exp == 0 {
            (frac, -1074)
        } else {
            (frac | (1_u64 << 52), exp - 1075)
        };
        Dyadic {
            m: Big::new(v < 0.0, vec![m]),
            e,
        }
    }

    fn align(&self, o: &Dyadic) -> (Big, Big, i32) {
        let e = self.e.min(o.e);
        (self.m.shl(self.e.abs_diff(e)), o.m.shl(o.e.abs_diff(e)), e)
    }

    /// `self + o`.
    pub(crate) fn add(&self, o: &Dyadic) -> Dyadic {
        let (a, b, e) = self.align(o);
        Dyadic { m: a.add(&b), e }
    }

    /// `self - o`.
    pub(crate) fn sub(&self, o: &Dyadic) -> Dyadic {
        self.add(&Dyadic {
            m: Big::new(!o.m.neg, o.m.mag.clone()),
            e: o.e,
        })
    }

    /// `self * o`.
    pub(crate) fn mul(&self, o: &Dyadic) -> Dyadic {
        Dyadic {
            m: self.m.mul(&o.m),
            e: self.e.saturating_add(o.e),
        }
    }

    /// `self / o` rounded to `f64` with a relative error of a few ulps; `None` when `o` is 0.
    pub(crate) fn div_to_f64(&self, o: &Dyadic) -> Option<f64> {
        let ((m1, e1), (m2, e2)) = (self.m.top_bits(), o.m.top_bits());
        if m2 == 0.0 {
            return None;
        }
        if m1 == 0.0 {
            return Some(0.0);
        }
        let shift = e1 + i64::from(self.e) - e2 - i64::from(o.e);
        let q = ldexp(m1 / m2, shift);
        Some(if self.m.neg != o.m.neg { -q } else { q })
    }
}

/// `x * 2^e`, saturating to infinity or zero.
fn ldexp(mut x: f64, mut e: i64) -> f64 {
    const STEP: i64 = 1000;
    let up = f64::from_bits(((1023 + STEP) as u64) << 52);
    let down = f64::from_bits(((1023 - STEP) as u64) << 52);
    while e > STEP {
        x *= up;
        e -= STEP;
        if x.is_infinite() {
            return x;
        }
    }
    while e < -STEP {
        x *= down;
        e += STEP;
        if x == 0.0 {
            return x;
        }
    }
    // `|e| <= 1000` here, so `2^e` is a normal `f64`.
    x * f64::from_bits(((1023 + e) as u64) << 52)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(v: f64) -> Dyadic {
        Dyadic::from_f64(v)
    }

    #[test]
    fn arithmetic_is_exact() {
        // (1e300 + 1e-300) - 1e300 is exactly 1e-300.
        let v = d(1e300).add(&d(1e-300)).sub(&d(1e300));
        assert_eq!(v.div_to_f64(&d(1.0)), Some(1e-300));
        // 3 * 7 / -2.
        assert_eq!(d(3.0).mul(&d(7.0)).div_to_f64(&d(-2.0)), Some(-10.5));
        // Products beyond the f64 range divide back exactly.
        let big = d(1e300).mul(&d(1e300));
        let q = big.div_to_f64(&d(1e300).mul(&d(3e299))).unwrap();
        assert!((q - 10.0 / 3.0).abs() < 1e-15);
        assert_eq!(d(0.0).div_to_f64(&d(5.0)), Some(0.0));
        assert_eq!(d(5.0).div_to_f64(&d(0.0)), None);
        // Subnormals.
        let tiny = f64::from_bits(1);
        assert_eq!(d(tiny).mul(&d(2.0)).div_to_f64(&d(1.0)), Some(2.0 * tiny));
    }

    #[test]
    fn ldexp_saturates() {
        assert_eq!(ldexp(1.0, 3), 8.0);
        assert_eq!(ldexp(1.5, -1074 - 10), 0.0);
        assert!(ldexp(1.0, 5000).is_infinite());
        assert_eq!(ldexp(1.0, -1074), f64::from_bits(1));
    }
}
